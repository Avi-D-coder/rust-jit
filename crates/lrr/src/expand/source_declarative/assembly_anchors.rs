//! Original declaration occurrences follow exact edits, never output names or
//! copied token-origin keys. These anchors are not member publication authority.
use super::*;
use crate::syntax::parse::ParsedFile;
use lrr_expand_adapter::syntax_attachment::SyntaxEdit;

#[derive(Clone, Debug)]
pub struct OriginalDeclarationAnchor {
    definition: Digest,
    original: SourceRange,
    declaration: SourceRange,
    header: SourceRange,
    body: Option<SourceRange>,
}
impl OriginalDeclarationAnchor {
    pub fn original_definition(&self) -> Digest {
        self.definition
    }
    pub fn original_range(&self) -> SourceRange {
        self.original
    }
    pub fn declaration_range(&self) -> SourceRange {
        self.declaration
    }
    pub fn header_range(&self) -> SourceRange {
        self.header
    }
    pub fn body_range(&self) -> Option<SourceRange> {
        self.body
    }
    pub(super) fn encode(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        for range in [self.original, self.declaration, self.header] {
            e.u32(range.start);
            e.u32(range.end);
        }
        e.option(&self.body, |e, range| {
            e.u32(range.start);
            e.u32(range.end);
        });
    }
}
pub(super) fn metadata(anchors: &Vec<OriginalDeclarationAnchor>) -> Result<usize, Diagnostic> {
    charge(
        0,
        anchors
            .capacity()
            .checked_mul(std::mem::size_of::<OriginalDeclarationAnchor>())
            .ok_or_else(|| fault("original declaration anchor metadata overflow"))?,
    )
}
fn outline(
    source: &PreparedSourceView,
    snapshot: Arc<SyntaxSnapshot>,
) -> Result<crate::syntax::outline::Outline, Diagnostic> {
    let mut parsed =
        ParsedFile::from_syntax_snapshot(snapshot, source.selection().outline.parse.profile.clone())?;
    // This snapshot re-parses the original captured caller before any
    // expansion; it is compared item-by-item with the original outline, so
    // original-only dialects (macro 2.0 declarations) remain admitted.
    parsed.generated_origin = None;
    parsed
        .outline(
            source.selection().outline.crate_key,
            source.selection().outline.module_path.clone(),
            source.captured_macro_cfg()?,
        )
}
pub(super) fn capture(
    source: &PreparedSourceView,
    tree: &SyntaxAttachment,
) -> Result<Vec<OriginalDeclarationAnchor>, Diagnostic> {
    let original = source.outline();
    let normalized = outline(source, tree.snapshot().map_err(adapter)?)?;
    let mut source_items = original
        .items
        .iter()
        .map(|item| (item.def_key.fingerprint(), item))
        .collect::<BTreeMap<_, _>>();
    // A block module that only an expansion wrote is hoisted by the assembled
    // tree's outline (`outline::module_segment`) and has no original
    // declaration to anchor; it is not part of the original inventory.
    let mut normalized = normalized;
    normalized.items.retain(|item| {
        source_items.contains_key(&item.def_key.fingerprint())
            || !(item.def_key.item_kind == crate::source::manifest::ItemKind::Module
                && item.def_key.declared_name.starts_with("__lrr_block_")
                || item
                    .def_key
                    .module_path
                    .iter()
                    .any(|segment| segment.starts_with("__lrr_block_")))
    });
    if source_items.len() != original.items.len() || normalized.items.len() != original.items.len()
    {
        return Err(fault(
            "original declaration normalization inventory differs",
        ));
    }
    let mut anchors = Vec::new();
    anchors
        .try_reserve_exact(original.items.len())
        .map_err(|_| fault("original declaration anchor allocation"))?;
    for item in normalized.items {
        let definition = item.def_key.fingerprint();
        // Pair only before any expansion has occurred. The tree was parsed
        // from this exact captured caller; no output can introduce a duplicate.
        let selected = source_items
            .remove(&definition)
            .ok_or_else(|| fault("original declaration normalization identity differs"))?;
        if !item.header.same_semantics(&selected.header)
            || item.body.is_some() != selected.body.is_some()
        {
            return Err(fault(
                "original declaration normalization header/body differs",
            ));
        }
        anchors.push(OriginalDeclarationAnchor {
            definition,
            original: selected.declaration_span,
            declaration: item.declaration_span,
            header: item.header_span,
            body: item.body.map(|body| body.range),
        });
    }
    anchors.sort_by_key(|anchor| anchor.definition);
    metadata(&anchors)?;
    Ok(anchors)
}
pub(super) fn translate(
    anchors: Vec<OriginalDeclarationAnchor>,
    edit: SyntaxEdit,
) -> Result<Vec<OriginalDeclarationAnchor>, Diagnostic> {
    let mut translated = Vec::new();
    translated
        .try_reserve_exact(anchors.len())
        .map_err(|_| fault("translated declaration anchor allocation"))?;
    for mut anchor in anchors {
        let map = |range: SourceRange| {
            edit.map_original_range(range.start, range.end)
                .map_err(adapter)
                .map(|range| range.map(|(start, end)| SourceRange { start, end }))
        };
        let Some(declaration) = map(anchor.declaration)? else {
            continue;
        };
        let header = map(anchor.header)?
            .ok_or_else(|| fault("surviving original declaration lost its header"))?;
        let body = match anchor.body {
            // A constant/static initializer can be exactly the replaced
            // expression. Its declaration survives while the expression changes.
            Some(range) if (range.start, range.end) == edit.removed() => {
                let (start, end) = edit.inserted();
                if start >= end {
                    return Err(fault("original initializer replacement is empty"));
                }
                Some(SourceRange { start, end })
            }
            Some(range) => Some(
                map(range)?.ok_or_else(|| fault("surviving original declaration lost its body"))?,
            ),
            None => None,
        };
        anchor.declaration = declaration;
        anchor.header = header;
        anchor.body = body;
        translated.push(anchor);
    }
    metadata(&translated)?;
    Ok(translated)
}
pub(super) fn validate(
    source: &PreparedSourceView,
    snapshot: Arc<SyntaxSnapshot>,
    anchors: &[OriginalDeclarationAnchor],
) -> Result<(), Diagnostic> {
    let normalized = outline(source, snapshot)?;
    let originals = source
        .outline()
        .items
        .iter()
        .map(|item| (item.def_key.fingerprint(), item))
        .collect::<BTreeMap<_, _>>();
    let mut locations = BTreeMap::new();
    for item in &normalized.items {
        if locations
            .insert(
                (item.declaration_span.start, item.declaration_span.end),
                item,
            )
            .is_some()
        {
            return Err(fault(
                "assembled original declaration location is ambiguous",
            ));
        }
    }
    for anchor in anchors {
        let original = originals
            .get(&anchor.definition)
            .ok_or_else(|| fault("original declaration anchor identity disappeared"))?;
        let actual = locations
            .get(&(anchor.declaration.start, anchor.declaration.end))
            .ok_or_else(|| fault("assembled original declaration anchor absent"))?;
        if actual.header_span != anchor.header
            || actual.body.as_ref().map(|body| body.range) != anchor.body
            || actual.def_key.item_kind != original.def_key.item_kind
            || actual.def_key.declared_name != original.def_key.declared_name
        {
            return Err(fault("assembled original declaration anchor shape differs"));
        }
    }
    Ok(())
}
