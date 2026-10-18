//! Immutable retained syntax admitted against actual current source/epoch/seal
//! inputs. This supplies parsing provenance, not a namespace or VM entry grant.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::SyntaxProvider,
    seals::SealKey,
    value::QueryValue,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::source_declarative::assembly::PreparedSourceSyntax,
    syntax::parse::{ParseKey, ParsedFile},
};
use std::{collections::BTreeMap, sync::Arc};

pub struct RetainedSyntaxInput {
    prepared: Arc<PreparedSourceSyntax>,
    seal: SealKey,
    publication: Digest,
    key: ParseKey,
    // The published generated unit that declares a selected macro-generated
    // function; its own record/epoch/seal are joined at admission.
    generated: Option<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>>,
    // A selected macro-generated associated function written into an original
    // impl/trait body: its actual retained declaration key and owning scope.
    associated: Option<GeneratedAssociated>,
    bytes: usize,
}
/// The actual retained declaration of a macro-generated associated function
/// (impl/trait item position) inside its original owner. Not a name grant.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct GeneratedAssociated {
    pub(crate) def_key: crate::source::manifest::DefKey,
    pub(crate) scope: crate::syntax::outline::ScopeKey,
}
impl std::fmt::Debug for RetainedSyntaxInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedSyntaxInput")
            .field("key", &self.key)
            .field("seal", &self.seal)
            .finish()
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn add(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b).ok_or_else(|| {
        Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "retained source input metadata budget".into(),
        )
    })
}
impl RetainedSyntaxInput {
    pub fn prepare(
        db: &mut Database,
        prepared: Arc<PreparedSourceSyntax>,
        seal: SealKey,
    ) -> QueryPoll<Arc<Self>> {
        if let Err(error) = db.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        let publication = match current(db, &prepared, &seal, None) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let (generated, associated) = match prepared.selected_generated() {
            None => (None, None),
            Some(anchor) => match generated_owner(db, &prepared, anchor, &seal) {
                Ok(owner) => owner,
                Err(error) => return QueryPoll::Ready(Err(error)),
            },
        };
        let result = (|| {
            let mut identity = StableEncoder::new(if generated.is_some() {
                "lrr/v1/admitted-source-syntax/generated-body"
            } else if associated.is_some() {
                "lrr/v1/admitted-source-syntax/generated-associated-body"
            } else {
                "lrr/v1/admitted-source-syntax"
            });
            identity.digest(prepared.identity());
            identity.record(&seal);
            identity.digest(publication);
            if let Some(unit) = &generated {
                identity.record(&unit.key);
            }
            if let Some(associated) = &associated {
                identity.record(&associated.def_key);
                identity.record(&associated.scope);
            }
            let key = ParseKey {
                content: identity.finish(),
                profile: prepared.source().outline.parse.profile.clone(),
            };
            let bytes = add(
                add(
                    add(
                        std::mem::size_of::<Self>(),
                        key.profile.semantics.capacity(),
                    )?,
                    prepared.metadata_excluding_snapshot()?,
                )?,
                ParsedFile::snapshot_metadata_bytes(prepared.syntax())?,
            )?;
            let input = Arc::new(Self {
                prepared,
                seal,
                publication,
                key,
                generated,
                associated,
                bytes,
            });
            // Validate the complete structural replay before admitting input.
            // The temporary tree is not retained in the input or byte source map.
            input.replay()?;
            Ok(input)
        })();
        QueryPoll::Ready(result)
    }
    pub(crate) fn seal(&self) -> &SealKey { &self.seal }
    pub fn key(&self) -> &ParseKey {
        &self.key
    }
    pub(crate) fn prepared(&self) -> &PreparedSourceSyntax {
        &self.prepared
    }
    /// The retained declaration of this input's selected generated
    /// associated function, when it was written into an original owner.
    pub(crate) fn generated_associated(&self) -> Option<&GeneratedAssociated> {
        self.associated.as_ref()
    }
    /// The generated unit declaring this input's selected generated function.
    pub(crate) fn generated_unit(
        &self,
    ) -> Option<&Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>> {
        self.generated.as_ref()
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        Ok(self.bytes)
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match current(db, &self.prepared, &self.seal, Some(self.publication)) {
            QueryPoll::Ready(Ok(_)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        if let Some(unit) = &self.generated
            && !matches!(db.input(&InputKey::GeneratedSyntax(unit.key.content)).as_deref(),
                Some(InputValue::GeneratedSyntax(current)) if current.same_unit(unit))
        {
            return QueryPoll::Ready(Err(fault(
                "retained generated body unit publication changed",
            )));
        }
        QueryPoll::Ready(Ok(()))
    }
    pub(crate) fn replay(&self) -> Result<ParsedFile, Diagnostic> {
        let mut parsed = ParsedFile::from_syntax_snapshot(
            self.prepared.syntax().clone(),
            self.key.profile.clone(),
        )?;
        // A typed structural input has its own canonical admission identity.
        // No SourceBlob is manufactured under this digest.
        parsed.content = self.key.content;
        parsed.generated_origin = Some(self.key.content);
        Ok(parsed)
    }
}
impl StableEncode for RetainedSyntaxInput {
    const DOMAIN: &'static str = "lrr/v1/retained-syntax-input";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.prepared.identity());
        e.record(&self.seal);
        e.digest(self.publication);
        e.option(&self.generated, |e, unit| e.record(unit.as_ref()));
        e.option(&self.associated, |e, associated| {
            e.record(&associated.def_key);
            e.record(&associated.scope);
        });
    }
}
/// The canonical key, module boundary and owning edition of a generated
/// definition located in a parse: from its unit, or from the retained
/// admission of an associated function written into an original owner.
pub(crate) struct GeneratedDeclarationOwner {
    pub(crate) def_key: crate::source::manifest::DefKey,
    pub(crate) boundary: crate::syntax::outline::ScopeKey,
    pub(crate) edition: u16,
}
impl Database {
    pub(crate) fn generated_declaration_owner(
        &mut self,
        parse: &ParseKey,
        definition: Digest,
    ) -> Option<GeneratedDeclarationOwner> {
        if let Some(unit) = self.generated_declaration_unit(parse) {
            return Some(GeneratedDeclarationOwner {
                def_key: unit.definitions.get(&definition)?.clone(),
                boundary: unit.scope.clone(),
                edition: unit.key.profile.edition,
            });
        }
        match self.input(&InputKey::RetainedSyntax(parse.content)).as_deref() {
            Some(InputValue::RetainedSyntax(retained))
                if retained.key() == parse
                    && retained
                        .prepared()
                        .selected_generated()
                        .is_some_and(|anchor| anchor.definition() == definition) =>
            {
                let associated = retained.generated_associated()?;
                Some(GeneratedDeclarationOwner {
                    def_key: associated.def_key.clone(),
                    boundary: crate::syntax::outline::ScopeKey {
                        owner: None,
                        ..associated.scope.clone()
                    },
                    edition: parse.profile.edition,
                })
            }
            _ => None,
        }
    }
    /// Whether `parse` locates generated declarations: a generated unit, or a
    /// retained admission of one of its functions (a body-only edit).
    pub(crate) fn locates_generated_declaration(&self, parse: &ParseKey) -> bool {
        self.published_input(&InputKey::GeneratedSyntax(parse.content))
            .is_some()
            || matches!(self.published_input(&InputKey::RetainedSyntax(parse.content)),
                Some((InputValue::RetainedSyntax(retained), _))
                    if retained.key() == parse && retained.generated_unit().is_some())
    }
    /// The generated unit declaring the definitions located in `parse`: the
    /// unit's own parse, or a retained admission of one of its functions.
    pub(crate) fn generated_declaration_unit(
        &mut self,
        parse: &ParseKey,
    ) -> Option<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>> {
        if let Some(InputValue::GeneratedSyntax(unit)) = self
            .input(&InputKey::GeneratedSyntax(parse.content))
            .as_deref()
        {
            return Some(Arc::clone(unit));
        }
        match self
            .input(&InputKey::RetainedSyntax(parse.content))
            .as_deref()
        {
            Some(InputValue::RetainedSyntax(retained)) if retained.key() == parse => {
                retained.generated_unit().cloned()
            }
            _ => None,
        }
    }
}
type GeneratedOwner = (
    Option<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>>,
    Option<GeneratedAssociated>,
);
/// The owner of a selected generated function: its published generated unit,
/// or — for an associated function written by an impl/trait-item expansion
/// into an original owner — that function's actual retained declaration.
fn generated_owner(
    db: &mut Database,
    prepared: &PreparedSourceSyntax,
    anchor: &crate::expand::source_declarative::assembly::GeneratedBodyAnchor,
    seal: &SealKey,
) -> Result<GeneratedOwner, Diagnostic> {
    use crate::expand::identity::GrammarSlot;
    let output = prepared
        .outputs()
        .iter()
        .find(|output| output.record().key().fingerprint() == anchor.record())
        .ok_or_else(|| fault("retained generated body output absent"))?;
    if !matches!(
        output.record().key().grammar,
        GrammarSlot::ImplItems | GrammarSlot::TraitItems
    ) {
        return generated_unit(db, anchor, seal).map(|unit| (Some(unit), None));
    }
    let definition = anchor.definition();
    // Associated output has no separate unit. Its locator is absent before
    // its first admission, or names an earlier retained admission of it.
    if let Some(InputValue::DefinitionSource(parse)) = db
        .input(&InputKey::DefinitionSource(definition))
        .as_deref()
        .cloned()
        && !matches!(db.input(&InputKey::RetainedSyntax(parse.content)).as_deref(),
            Some(InputValue::RetainedSyntax(previous))
                if previous.generated_associated().is_some()
                    && previous.prepared().selected_generated().is_some_and(|a| a.definition() == definition))
    {
        return Err(fault("retained generated associated locator differs"));
    }
    let source = db
        .prepared_source(prepared.source())?
        .ok_or_else(|| fault("retained generated associated source lease absent"))?;
    let mut parsed = ParsedFile::from_syntax_snapshot(
        prepared.syntax().clone(),
        prepared.source().outline.parse.profile.clone(),
    )?;
    parsed.generated_origin = Some(Digest(prepared.syntax().identity().0));
    let outline = parsed.outline(
        source.selection().outline.crate_key,
        source.selection().outline.module_path.clone(),
        source.captured_macro_cfg()?,
    )?;
    let mut candidates = outline
        .items
        .iter()
        .filter(|item| item.declaration_span == anchor.declaration_range());
    let actual = candidates
        .next()
        .ok_or_else(|| fault("retained generated associated declaration absent"))?;
    let owner = actual
        .scope
        .owner
        .ok_or_else(|| fault("retained generated associated declaration has no owner"))?;
    // The owner is an actual original impl/trait of this source.
    let original_owner = source.outline().items.iter().any(|item| match item.def_key.item_kind {
        crate::source::manifest::ItemKind::Impl => item.def_key.enclosing_impl == Some(owner),
        crate::source::manifest::ItemKind::Trait => item.def_key.fingerprint() == owner,
        _ => false,
    });
    if candidates.next().is_some()
        || !original_owner
        || !matches!(
            actual.def_key.item_kind,
            crate::source::manifest::ItemKind::Function | crate::source::manifest::ItemKind::Constant
        )
        || actual.body.as_ref().map(|body| body.range) != Some(anchor.body_range())
    {
        return Err(fault("retained generated associated declaration differs"));
    }
    Ok((
        None,
        Some(GeneratedAssociated {
            def_key: actual.def_key.clone(),
            scope: actual.scope.clone(),
        }),
    ))
}
/// The actual published unit that declares a selected generated function:
/// the declaration's current locator names that unit (or an earlier retained
/// admission of it), and the unit's record wrote the anchored declaration.
fn generated_unit(
    db: &mut Database,
    anchor: &crate::expand::source_declarative::assembly::GeneratedBodyAnchor,
    seal: &SealKey,
) -> Result<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>, Diagnostic> {
    let definition = anchor.definition();
    let Some(InputValue::DefinitionSource(parse)) = db
        .input(&InputKey::DefinitionSource(definition))
        .as_deref()
        .cloned()
    else {
        return Err(fault("retained generated body locator absent"));
    };
    let unit = match db
        .input(&InputKey::GeneratedSyntax(parse.content))
        .as_deref()
    {
        Some(InputValue::GeneratedSyntax(unit)) => Arc::clone(unit),
        _ => match db
            .input(&InputKey::RetainedSyntax(parse.content))
            .as_deref()
        {
            Some(InputValue::RetainedSyntax(previous)) => previous
                .generated_unit()
                .cloned()
                .ok_or_else(|| fault("retained generated body previous unit absent"))?,
            _ => return Err(fault("retained generated body unit absent")),
        },
    };
    let header = unit
        .items
        .items()
        .iter()
        .find(|header| header.key.fingerprint() == definition)
        .ok_or_else(|| fault("retained generated body declaration absent from its unit"))?;
    if unit.items.record().key().fingerprint() != anchor.record()
        || header.key.expansion != anchor.record()
        || (header.start, header.end) != (anchor.output_range().start, anchor.output_range().end)
        || unit.seal.epoch != seal.epoch
        || unit.seal.unit_graph != seal.unit_graph
        || !unit.definitions.contains_key(&definition)
        || !matches!(db.input(&InputKey::GeneratedSyntax(unit.key.content)).as_deref(),
            Some(InputValue::GeneratedSyntax(current)) if current.same_unit(&unit))
    {
        return Err(fault(
            "retained generated body unit differs from its anchor",
        ));
    }
    Ok(unit)
}
fn current(
    db: &mut Database,
    prepared: &PreparedSourceSyntax,
    seal: &SealKey,
    expected: Option<Digest>,
) -> QueryPoll<Digest> {
    if prepared.epoch() != seal.epoch || !prepared.ready_for_admission() {
        return QueryPoll::Ready(Err(fault(
            "retained syntax epoch differs or expansion calls remain pending",
        )));
    }
    if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(), Some(InputValue::ExpansionEpoch(epoch)) if *epoch == prepared.epoch())
    {
        return QueryPoll::Ready(Err(fault(
            "retained syntax current expansion epoch is absent or stale",
        )));
    }
    match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(seal.clone())) {
        QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(actual) if actual.key() == *seal) =>
            {}
        QueryPoll::Ready(Ok(_)) => {
            return QueryPoll::Ready(Err(fault("retained syntax discovery result differs")));
        }
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    }
    let Some(certificate) = db.input(&InputKey::DiscoverySeal(seal.clone())) else {
        return QueryPoll::Ready(Err(fault("retained syntax discovery input disappeared")));
    };
    let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
        return QueryPoll::Ready(Err(fault(
            "retained syntax discovery input variant differs",
        )));
    };
    let publication = certificate.fingerprint();
    if expected.is_some_and(|value| value != publication)
        || !input
            .sources
            .iter()
            .any(|source| source == prepared.source())
    {
        return QueryPoll::Ready(Err(fault(
            "retained syntax enclosing source publication differs",
        )));
    }
    if !matches!(db.input(&InputKey::CapturedSource(prepared.source().location_identity())).as_deref(), Some(InputValue::CapturedSource(source)) if source == prepared.source())
    {
        return QueryPoll::Ready(Err(fault("retained syntax captured caller changed")));
    }
    match db.prepared_source(prepared.source()) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return QueryPoll::Ready(Err(fault(
                "retained syntax captured caller lease is absent",
            )));
        }
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    let records = input
        .records
        .iter()
        .map(|record| (record.key.fingerprint(), record))
        .collect::<BTreeMap<_, _>>();
    if records.len() != input.records.len() {
        return QueryPoll::Ready(Err(fault("retained syntax record inventory duplicated")));
    }
    for output in prepared.outputs() {
        let actual = output.record().descriptor();
        if records
            .get(&actual.key.fingerprint())
            .is_none_or(|record| record.fingerprint() != actual.fingerprint())
            || input
                .seal
                .input_dependencies()
                .binary_search(&(actual.key.fingerprint(), actual.fingerprint()))
                .is_err()
        {
            return QueryPoll::Ready(Err(fault(
                "retained syntax record is absent from its discovery certificate",
            )));
        }
    }
    QueryPoll::Ready(Ok(publication))
}
