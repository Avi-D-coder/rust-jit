//! Source macro declaration occurrences, distinct from repeated token origins.
//! These rows supply resolver inputs only after ordinary retained-source admission.
use super::*;
use crate::expand::scopes::MacroDefinition;
use lrr_expand_adapter::{
    declaration_output::{DeclarationSyntax, DefinitionCapture},
    syntax_attachment::SyntaxEdit,
    token_wire::TokenForest,
};

#[derive(Clone, Debug)]
pub struct MacroDefinitionAnchor {
    definition: MacroDefinition,
    range: SourceRange,
    capture: Digest,
}
impl MacroDefinitionAnchor {
    pub fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub(super) fn encode(&self, e: &mut StableEncoder) {
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u16(self.definition.definition_edition);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.digest(self.capture);
    }
}
fn capture_identity(capture: &DefinitionCapture) -> Result<Digest, Diagnostic> {
    let mut e = StableEncoder::new("lrr/v1/source-macro-capture");
    e.digest(Digest(capture.name_span.0));
    e.u16(capture.edition);
    e.digest(receipt::roots_identity(capture.body.roots())?);
    Ok(e.finish())
}
pub(super) fn metadata(anchors: &Vec<MacroDefinitionAnchor>) -> Result<usize, Diagnostic> {
    charge(
        0,
        anchors
            .capacity()
            .checked_mul(std::mem::size_of::<MacroDefinitionAnchor>())
            .ok_or_else(|| fault("macro definition anchor metadata overflow"))?,
    )
}
fn push(
    anchors: &mut Vec<MacroDefinitionAnchor>,
    definition: MacroDefinition,
    capture: &DefinitionCapture,
    range: SourceRange,
) -> Result<(), Diagnostic> {
    definition.validate()?;
    if capture.edition != definition.definition_edition {
        return Err(fault("macro definition anchor owning edition differs"));
    }
    anchors
        .try_reserve(1)
        .map_err(|_| fault("macro definition anchor allocation"))?;
    anchors.push(MacroDefinitionAnchor {
        definition,
        range,
        capture: capture_identity(capture)?,
    });
    metadata(anchors)?;
    Ok(())
}
/// Pair only before the first expansion, using physical source-name locations.
/// A later output may copy that origin, so origin equality is never sufficient
/// to recover an occurrence after attachment.
pub(super) fn capture(
    source: &PreparedSourceView,
    scan: &DeclarationScan,
    syntax: &DeclarationSyntax,
    forest: &TokenForest,
) -> Result<Vec<MacroDefinitionAnchor>, Diagnostic> {
    let selection = source.selection();
    charge(
        0,
        scan.macro_definitions()
            .len()
            .checked_mul(128)
            .ok_or_else(|| fault("macro declaration header index metadata overflow"))?,
    )?;
    let mut headers: BTreeMap<_, _> = scan
        .macro_definitions()
        .iter()
        .map(|header| (header.range.start, header))
        .collect();
    if headers.len() != scan.macro_definitions().len() {
        return Err(fault("original macro declaration positions are ambiguous"));
    }
    let mut anchors = Vec::new();
    for capture in &syntax.definitions {
        // TokenForest validates strict span-key order. Reuse that immutable
        // index instead of rebuilding a full span map for declaration capture.
        let span = forest
            .span_position(&capture.name_span)
            .map(|position| &forest.spans()[position])
            .ok_or_else(|| fault("original macro declaration name source absent"))?;
        if span.source.unit.0 != selection.outline.crate_key.0
            || span.source.content.0 != selection.outline.parse.content.0
            || span.source.path != selection.path.as_str()
        {
            return Err(fault(
                "original macro declaration name belongs to another source",
            ));
        }
        let Some((&start, header)) = headers.range(..=span.start).next_back() else {
            continue; // The declaration scan may exclude cfg-disabled syntax.
        };
        let name = SourceRange::checked(span.start, span.end)?;
        if !header.range.contains(name) || header.body.contains(name) {
            continue;
        }
        let definition = MacroDefinition {
            identity: header.identity,
            defining_unit: scan.scope().crate_key,
            definition_edition: capture.edition,
        };
        push(
            &mut anchors,
            definition,
            capture,
            SourceRange::checked(capture.start, capture.end)?,
        )?;
        headers.remove(&start);
    }
    if !headers.is_empty() {
        return Err(fault(
            "original macro declaration anchor inventory incomplete",
        ));
    }
    Ok(anchors)
}

pub(super) fn translate(
    anchors: Vec<MacroDefinitionAnchor>,
    edit: SyntaxEdit,
) -> Result<Vec<MacroDefinitionAnchor>, Diagnostic> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(anchors.len())
        .map_err(|_| fault("translated macro declaration allocation"))?;
    for mut anchor in anchors {
        let Some((start, end)) = edit
            .map_original_range(anchor.range.start, anchor.range.end)
            .map_err(adapter)?
        else {
            continue;
        };
        anchor.range = SourceRange::checked(start, end)?;
        result.push(anchor);
    }
    metadata(&result)?;
    Ok(result)
}

/// Join exact admitted output identities to the actual declarations inserted by
/// one tree edit. Preorder distinguishes two copies with identical origin keys.
pub(super) fn append_generated(
    anchors: &mut Vec<MacroDefinitionAnchor>,
    output: &GeneratedDeclarations,
    fragment: &DeclarationSyntax,
    inserted: &[DefinitionCapture],
    _edit: SyntaxEdit,
) -> Result<(), Diagnostic> {
    // `inserted` is what the live tree holds in the edit's inserted range.
    let inserted = inserted.iter();
    if inserted.clone().count() != fragment.definitions.len() {
        return Err(fault(
            "inserted macro declaration occurrence inventory differs",
        ));
    }
    let mut headers: BTreeMap<_, _> = output
        .scan()
        .macro_definitions()
        .iter()
        .map(|header| ((header.range.start, header.range.end), header))
        .collect();
    if headers.len() != output.scan().macro_definitions().len() {
        return Err(fault("generated macro declaration positions are ambiguous"));
    }
    for (fragment, actual) in fragment.definitions.iter().zip(inserted) {
        if capture_identity(fragment)? != capture_identity(actual)? {
            return Err(fault("inserted macro declaration capture differs"));
        }
        let Some(header) = headers.remove(&(fragment.start, fragment.end)) else {
            continue; // No identity is invented for cfg-disabled declarations.
        };
        if capture_identity(output.macro_definition(header.identity)?)?
            != capture_identity(fragment)?
        {
            return Err(fault("generated macro declaration source receipt differs"));
        }
        push(
            anchors,
            MacroDefinition {
                identity: header.identity,
                defining_unit: output.scan().scope().crate_key,
                definition_edition: actual.edition,
            },
            actual,
            SourceRange::checked(actual.start, actual.end)?,
        )?;
    }
    if !headers.is_empty() {
        return Err(fault(
            "generated macro declaration anchor inventory incomplete",
        ));
    }
    Ok(())
}

pub(super) fn validate(
    anchors: &[MacroDefinitionAnchor],
    actual: &DeclarationSyntax,
) -> Result<(), Diagnostic> {
    let captures: BTreeMap<_, _> = actual
        .definitions
        .iter()
        .map(|capture| ((capture.start, capture.end), capture))
        .collect();
    if captures.len() != actual.definitions.len() {
        return Err(fault("assembled macro declaration positions are ambiguous"));
    }
    let mut seen = BTreeSet::new();
    for anchor in anchors {
        let key = (anchor.range.start, anchor.range.end);
        let capture = captures
            .get(&key)
            .ok_or_else(|| fault("assembled macro declaration anchor absent"))?;
        if !seen.insert(key) || capture_identity(capture)? != anchor.capture {
            return Err(fault("assembled macro declaration anchor capture differs"));
        }
    }
    Ok(())
}
