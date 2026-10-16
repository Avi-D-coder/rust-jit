//! Ordinary entered member occurrence with current source ownership and origins.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::pc::{NodeId, PcKey, Phase},
    source::changes::PreparedSourceView,
    syntax::{
        body_index::BodyIndex,
        fields::NamedFieldSite,
        outline::ScopeKey,
        parse::ParsedFile,
        token_wire::{Edition, SpanRecord},
    },
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
pub(super) struct FieldSource {
    site: NamedFieldSite,
    context: Arc<InputValue>,
}
impl FieldSource {
    pub(super) fn member(&self) -> &str {
        self.site.member()
    }
    pub(super) fn scope(&self) -> Result<&ScopeKey, Diagnostic> {
        match self.context.as_ref() {
            InputValue::DeclarationContext(context) => Ok(&context.scope),
            _ => Err(fault("selected field declaration context changed")),
        }
    }
}
fn edition(edition: Edition) -> u16 {
    match edition {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    }
}
/// Source roots remain authority-bearing identities. The pinned item context
/// only proves that no opaque field adjustment is required; it never supplies
/// the caller module or grants access to a same-spelled private member.
fn require_origin(
    db: &mut Database,
    sources: &[PreparedSourceView],
    parsed: &ParsedFile,
    site: &NamedFieldSite,
    origin: &SpanRecord,
) -> Result<(), Diagnostic> {
    let facts = parsed
        .hygiene_facts()
        .ok_or_else(|| fault("field origin has no pinned hygiene facts"))?;
    let root = facts.item(origin.context)?;
    let context = facts.context(root)?;
    if Some(root) != site.root_context()
        || context.parent.is_some()
        || context.defining_unit != origin.source.unit
    {
        return Err(fault(
            "field selector source root differs from its actual token",
        ));
    }
    let mut candidates = sources.iter().filter(|source| {
        let selection = source.selection();
        selection.outline.crate_key.0 == origin.source.unit.0
            && selection.outline.parse.content.0 == origin.source.content.0
            && selection.path.as_str() == origin.source.path
    });
    let source = candidates
        .next()
        .ok_or_else(|| fault("field token original source owner absent"))?;
    if candidates.next().is_some() {
        return Err(fault("field token original source owner ambiguous"));
    }
    source.require_original_hygiene()?;
    let selection = source.selection();
    if edition(origin.edition) != selection.outline.parse.profile.edition
        || edition(context.edition) != selection.outline.parse.profile.edition
        || !matches!(db.input(&InputKey::CapturedSource(selection.location_identity())).as_deref(),Some(InputValue::CapturedSource(actual)) if actual==selection)
        || !matches!(db.input(&InputKey::CrateCfg(selection.outline.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==selection.outline.cfg)
    {
        return Err(fault("field token original source publication differs"));
    }
    let current = db
        .prepared_source(selection)?
        .ok_or_else(|| fault("field token original source lease absent"))?;
    if !source.same_publication(&current) {
        return Err(fault("field token belongs to another source publication"));
    }
    let blob = db
        .input(&InputKey::SourceBlob(Digest(origin.source.content.0)))
        .ok_or_else(|| fault("field token original source bytes absent"))?;
    let InputValue::SourceBlob(blob) = blob.as_ref() else {
        return Err(fault("field token source bytes variant differs"));
    };
    let text = blob
        .text()?
        .get(origin.start as usize..origin.end as usize)
        .ok_or_else(|| fault("field token original source range differs"))?;
    if crate::frontend::names::owned_name(text, edition(origin.edition))?.as_str()
        != crate::frontend::names::owned_name(site.member(), parsed.token_edition(site.range())?)?
            .as_str()
    {
        return Err(fault(
            "field selector differs from its actual original token",
        ));
    }
    Ok(())
}
pub(super) fn capture(
    db: &mut Database,
    sources: &[PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<FieldSource> {
    match capture_inner(db, sources, index, pc) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
fn capture_inner(
    db: &mut Database,
    sources: &[PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
) -> Result<QueryPoll<FieldSource>, Diagnostic> {
    if index.body() != &pc.body || index.pc(NodeId(pc.node), Phase::from_code(pc.phase))? != *pc {
        return Err(fault("field source entered PC differs"));
    }
    let definition = pc.body.definition;
    let locator = db
        .input(&InputKey::DefinitionSource(definition))
        .ok_or_else(|| fault("field declaration source absent"))?;
    let body = db
        .input(&InputKey::DefinitionBody(definition))
        .ok_or_else(|| fault("field declaration body absent"))?;
    let context = db
        .input(&InputKey::DeclarationContext(definition))
        .ok_or_else(|| fault("field declaration context absent"))?;
    let header = db
        .input(&InputKey::DefinitionHeader(definition))
        .ok_or_else(|| fault("field declaration header absent"))?;
    let (
        InputValue::DefinitionSource(parse),
        InputValue::DefinitionBody(handle),
        InputValue::DeclarationContext(declaration),
        InputValue::DefinitionHeader(header),
    ) = (
        locator.as_ref(),
        body.as_ref(),
        context.as_ref(),
        header.as_ref(),
    )
    else {
        return Err(fault("field declaration input variants differ"));
    };
    if handle.key() != pc.body
        || header.profile != parse.profile
        || index.profile() != &parse.profile
        || !declaration.range.contains(handle.range)
    {
        return Err(fault("field caller source revision differs from database"));
    }
    let current_index = match db.fetch(&SyntaxProvider, QueryKey::BodyStructure(pc.body.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
    };
    let QueryValue::BodyStructure(current_index) = current_index.value.as_ref() else {
        return Err(fault("field current body index outcome differs"));
    };
    if current_index.body() != index.body()
        || current_index.profile() != index.profile()
        || current_index.nodes().get(pc.node as usize) != index.nodes().get(pc.node as usize)
    {
        return Err(fault(
            "field selected index node differs from current source",
        ));
    }
    let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Err(fault("field selected parse outcome differs"));
    };
    if parsed.key() != *parse {
        return Err(fault("field selected parse key differs"));
    }
    let site = parsed.named_field_site(handle, index, NodeId(pc.node))?;
    if let Some(input) = db.input(&InputKey::RetainedSyntax(parse.content)) {
        let InputValue::RetainedSyntax(unit) = input.as_ref() else {
            return Err(fault("field retained source input differs"));
        };
        if unit.key() != parse || !unit.prepared().permits_body(definition) {
            return Err(fault(
                "field retained source belongs to another entered body",
            ));
        }
        match unit.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        }
        let mut owners = sources
            .iter()
            .filter(|source| source.selection() == unit.prepared().source());
        let owner = owners
            .next()
            .ok_or_else(|| fault("field retained original source owner absent"))?;
        if owners.next().is_some() {
            return Err(fault("field retained original source owner ambiguous"));
        }
        // The admitted immutable assembly anchors and already-published rows
        // certify this original member. Do not rebuild its complete outline at
        // every reached field operation merely to rediscover the same anchor.
        let original = owner
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == definition)
            .ok_or_else(|| fault("field retained original declaration absent"))?;
        let mut anchors = unit
            .prepared()
            .original_declarations()
            .iter()
            .filter(|anchor| anchor.original_definition() == definition);
        let anchor = anchors
            .next()
            .ok_or_else(|| fault("field retained original declaration anchor absent"))?;
        let current = db
            .prepared_source(owner.selection())?
            .ok_or_else(|| fault("field retained original source lease absent"))?;
        if anchors.next().is_some()
            || !owner.same_publication(&current)
            || anchor.original_range() != original.declaration_span
            || anchor.declaration_range() != declaration.range
            || anchor.body_range() != Some(handle.range)
            || original.scope != declaration.scope
            || original.header.fingerprint() != header.fingerprint()
            || !declaration.range.contains(anchor.header_range())
        {
            return Err(fault("field retained original declaration rows differ"));
        }
        let origins = parsed
            .retained_source_spans(site.range())?
            .ok_or_else(|| fault("field retained selector source origins absent"))?;
        let [origin] = origins.as_slice() else {
            return Err(fault("field selector needs one actual token origin"));
        };
        require_origin(db, sources, parsed, &site, origin)?;
    } else if let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) {
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            return Err(fault("field generated source input differs"));
        };
        let selected = unit
            .definitions
            .get(&definition)
            .ok_or_else(|| fault("field generated definition absent"))?;
        if unit.key != *parse
            || selected.crate_key != declaration.scope.crate_key
            || selected.module_path != declaration.scope.module_path
            || declaration.scope.crate_key != unit.scope.crate_key
            || !declaration
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
            || !unit.items.items().iter().any(|item| {
                item.key.fingerprint() == definition
                    && item.start == declaration.range.start
                    && item.end == declaration.range.end
            })
            || !matches!(db.input(&InputKey::CrateCfg(unit.scope.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==unit.cfg.fingerprint())
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==unit.seal.epoch && unit.items.epoch()==*epoch)
        {
            return Err(fault("field generated declaration/source epoch differs"));
        }
        // The canonical generated map supplies declaration identity, while the
        // ordinary memo signature proves that the current header/body/context
        // still describe that exact function in the admitted syntax tree.
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        if !matches!(signature.value.as_ref(), QueryValue::FunctionSignature(signature)
            if signature.definition == definition && signature.header == *header && signature.scope == declaration.scope)
        {
            return Err(fault("field generated current signature proof differs"));
        }
        match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(unit.seal.clone())) {
            QueryPoll::Ready(Ok(value)) if matches!(value.value.as_ref(),QueryValue::DiscoverySeal(seal) if seal.key()==unit.seal) =>
                {}
            QueryPoll::Ready(Ok(_)) => {
                return Err(fault("field generated discovery outcome differs"));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        }
        let publication = db
            .input(&InputKey::DiscoverySeal(unit.seal.clone()))
            .ok_or_else(|| fault("field generated discovery publication absent"))?;
        let InputValue::DiscoverySeal(publication) = publication.as_ref() else {
            return Err(fault("field generated discovery publication differs"));
        };
        let record = unit.items.record();
        if publication.seal.key() != unit.seal
            || !publication
                .records
                .iter()
                .any(|row| row.fingerprint() == record.descriptor().fingerprint())
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(actual)) if actual.same_record(record))
        {
            return Err(fault("field generated actual expansion record differs"));
        }
        let origins = unit.items.project_source_spans(site.range())?;
        let [origin] = origins.as_slice() else {
            return Err(fault("field selector needs one actual token origin"));
        };
        require_origin(db, sources, parsed, &site, origin)?;
    } else {
        if site.root_context().is_some() {
            return Err(fault("field original source has generated token context"));
        }
        let mut owners = sources.iter().filter(|source| {
            source.selection().outline.parse == *parse
                && source
                    .outline()
                    .items
                    .iter()
                    .any(|item| item.def_key.fingerprint() == definition)
        });
        let owner = owners
            .next()
            .ok_or_else(|| fault("field original source owner absent"))?;
        if owners.next().is_some() {
            return Err(fault("field original source owner ambiguous"));
        }
        owner.require_original_hygiene()?;
        let selection = owner.selection();
        let current = db
            .prepared_source(selection)?
            .ok_or_else(|| fault("field original source lease absent"))?;
        if !owner.same_publication(&current)
            || !owner.outline().items.iter().any(|item| {
                item.def_key.fingerprint() == definition
                    && item.header == *header
                    && item.scope == declaration.scope
                    && item.declaration_span == declaration.range
                    && item.body.as_ref() == Some(handle)
            })
            || !matches!(db.input(&InputKey::CrateCfg(selection.outline.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==selection.outline.cfg)
        {
            return Err(fault("field caller source revision differs from database"));
        }
    }
    Ok(QueryPoll::Ready(Ok(FieldSource { site, context })))
}
