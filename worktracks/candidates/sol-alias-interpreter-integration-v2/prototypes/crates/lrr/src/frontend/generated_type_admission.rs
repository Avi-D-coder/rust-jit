//! Readonly generated type selection; no provider lookup, descriptor or layout authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::BucketKey,
        memo::Database,
        seals::{SealDomain, SealKey},
    },
    expand::{
        generated_syntax::GeneratedSyntaxInput, identity::MacroIdentity,
        source_declarative::SourceDeclarative,
    },
    frontend::{
        generated_constant_provenance::{CurrentGeneratedConstant, GeneratedConstantForm},
        names::{NameDefinition, NameLookup},
    },
    ir::op::PrimitiveType,
    source::manifest::ItemKind,
    syntax::{
        generated_type_site::{
            GeneratedTypeRole, GeneratedTypeSegment, GeneratedTypeSegmentKind, GeneratedTypeSyntax,
            TypeKeyword, WrittenGeneratedType,
        },
        outline::{DeclarationContext, HeaderRecipe, ScopeKey},
        token_wire::{Key, Transparency},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams},
};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERATED-TYPE-CONTEXT", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated source type budget".into(),
    )
}
fn charge(bytes: &mut usize, n: usize, maximum: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= maximum)
        .ok_or_else(limit)?;
    Ok(())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum GeneratedTypePrerequisite {
    GenericOrAssociated,
    UnsupportedWrittenType,
    QualifiedModuleRoute,
    DefinitionSiteSource,
    TypeNamespace(ScopeKey),
    TypeName(NameLookup),
    PrimitiveEnvironment,
    AliasTargetHygiene,
    TraitOrForeignType,
    ReplacementOrGeneratedMember,
}
pub(crate) enum GeneratedTypeOutcome {
    Selected(SelectedGeneratedType),
    Need(GeneratedTypePrerequisite),
}
/// Alias identity stays in the proof even when its source-owned target is unit.
pub(crate) enum SelectedSourceType {
    IntrinsicUnit,
    AliasUnit(NameDefinition),
    AliasHeader(NameDefinition),
    NominalHeader(NameDefinition),
}
#[derive(Clone, Copy)]
enum RouteKind {
    CallSite,
    DefinitionSite,
    DollarCrate,
}
pub(crate) struct SelectedGeneratedType {
    parent: Digest,
    site: WrittenGeneratedType,
    result: SelectedSourceType,
    route: RouteKind,
    selected_header: Option<(HeaderRecipe, DeclarationContext)>,
    alias_site: Option<WrittenGeneratedType>,
    stamps: BTreeMap<InputKey, Digest>,
    revision: u64,
    bytes: usize,
}
fn stamp(
    db: &Database,
    out: &mut BTreeMap<InputKey, Digest>,
    key: &InputKey,
    bytes: &mut usize,
    maximum: usize,
) -> Result<(), Diagnostic> {
    let (_, value) = db
        .published_input(key)
        .ok_or_else(|| fault("generated type current published input absent"))?;
    if let Some(old) = out.get(key) {
        return if *old == value {
            Ok(())
        } else {
            Err(fault("generated type input changed"))
        };
    }
    if out.len() >= MAX_ROWS {
        return Err(limit());
    }
    charge(
        bytes,
        std::mem::size_of::<(InputKey, Digest)>()
            .checked_add(128)
            .and_then(|n| n.checked_add(key.heap_bytes()))
            .ok_or_else(limit)?,
        maximum,
    )?;
    out.insert(key.clone(), value);
    Ok(())
}
fn source_owner<'a>(
    db: &Database,
    id: Digest,
    owner: &'a SourceDeclarative,
    providers: &[&'a SourceDeclarative],
) -> Result<Option<&'a SourceDeclarative>, Diagnostic> {
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(id))
    else {
        return Ok(None);
    };
    let mut selected: Option<&SourceDeclarative> = None;
    for provider in std::iter::once(owner).chain(providers.iter().copied()) {
        let source = provider.module_source();
        if &source.selection().outline.parse != parse
            || !source
                .outline()
                .items
                .iter()
                .any(|i| i.def_key.fingerprint() == id)
        {
            continue;
        }
        source.require_original_hygiene()?;
        if !matches!(db.published_input(&InputKey::CapturedSource(source.selection().location_identity())), Some((InputValue::CapturedSource(actual), _)) if actual == source.selection())
        {
            return Err(fault(
                "generated type exact declaration owner source changed",
            ));
        }
        if let Some(old) = selected {
            if !old.module_source().same_publication(source)
                || old.module_source().selection() != source.selection()
            {
                return Err(fault("generated type retained source owner is ambiguous"));
            }
        } else {
            selected = Some(provider);
        }
    }
    Ok(selected)
}
#[allow(clippy::too_many_arguments)]
fn origin_scope(
    db: &Database,
    owner: &SourceDeclarative,
    providers: &[&SourceDeclarative],
    unit: &GeneratedSyntaxInput,
    call_scope: &ScopeKey,
    part: &GeneratedTypeSegment,
    stamps: &mut BTreeMap<InputKey, Digest>,
    bytes: &mut usize,
    maximum: usize,
) -> Result<Result<(ScopeKey, RouteKind), GeneratedTypePrerequisite>, Diagnostic> {
    let facts = unit.items.hygiene_facts();
    let call = Key(unit.items.record().key().parent_context.0);
    if facts.name_item(part.span().context)? == facts.name_item(call)? {
        charge(bytes, call_scope.heap_bytes(), maximum)?;
        return Ok(Ok((call_scope.clone(), RouteKind::CallSite)));
    }
    let normalized = facts.item(part.span().context)?;
    let mark = facts.context(normalized)?;
    let Some(expansion) = mark.expansion else {
        return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
    };
    if mark.transparency != Transparency::Opaque {
        return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
    }
    let key = InputKey::ExpansionRecord(Digest(expansion.0));
    let Some((InputValue::ExpansionRecord(record), _)) = db.published_input(&key) else {
        return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
    };
    if record.key().fingerprint().0 != expansion.0 {
        return Err(fault("generated definition-site mark record differs"));
    }
    let definition = match &record.key().macro_identity {
        MacroIdentity::Declaration { definition } => *definition,
        _ => return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource)),
    };
    let Some(provider) = source_owner(db, definition, owner, providers)? else {
        return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
    };
    let source = provider.module_source();
    let declaration = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == definition)
        .ok_or_else(|| fault("generated type definition-site declaration absent"))?;
    if declaration.def_key.item_kind != ItemKind::Macro
        || declaration.scope.owner.is_some()
        || declaration.scope.crate_key.0 != mark.defining_unit.0
        || source.outline().profile.edition != record.key().definition_edition
    {
        return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
    }
    charge(
        bytes,
        declaration
            .scope
            .heap_bytes()
            .checked_mul(2)
            .ok_or_else(limit)?,
        maximum,
    )?;
    let actual_context = DeclarationContext {
        scope: declaration.scope.clone(),
        range: declaration.declaration_span,
    };
    if !matches!(db.published_input(&InputKey::DefinitionHeader(definition)), Some((InputValue::DefinitionHeader(actual), _)) if actual == &declaration.header)
        || !matches!(db.published_input(&InputKey::DeclarationContext(definition)), Some((InputValue::DeclarationContext(actual), _)) if actual == &actual_context)
    {
        return Err(fault(
            "generated type definition-site header/context differs",
        ));
    }
    for key in [
        key,
        InputKey::DefinitionSource(definition),
        InputKey::DefinitionHeader(definition),
        InputKey::DeclarationContext(definition),
        InputKey::CapturedSource(source.selection().location_identity()),
    ] {
        stamp(db, stamps, &key, bytes, maximum)?;
    }
    Ok(Ok((declaration.scope.clone(), RouteKind::DefinitionSite)))
}

/// Borrowed actual source selected by the existing pinned generated Type origin
/// route. This does not establish primitive/prelude/alias semantics itself.
pub(crate) struct GeneratedPrimitiveOrigin<'a> {
    pub(crate) scope: ScopeKey,
    pub(crate) provider: &'a SourceDeclarative,
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn primitive_origin<'a>(
    db: &Database,
    owner: &'a SourceDeclarative,
    providers: &[&'a SourceDeclarative],
    unit: &GeneratedSyntaxInput,
    site: &WrittenGeneratedType,
    part: &GeneratedTypeSegment,
    stamps: &mut BTreeMap<InputKey, Digest>,
    bytes: &mut usize,
    maximum: usize,
) -> Result<Result<GeneratedPrimitiveOrigin<'a>, GeneratedTypePrerequisite>, Diagnostic> {
    let (scope, route) = match origin_scope(
        db,
        owner,
        providers,
        unit,
        site.scope(),
        part,
        stamps,
        bytes,
        maximum,
    )? {
        Ok(route) => route,
        Err(need) => return Ok(Err(need)),
    };
    let provider = match route {
        RouteKind::CallSite => owner,
        RouteKind::DefinitionSite => {
            let facts = unit.items.hygiene_facts();
            let normalized = facts.item(part.span().context)?;
            let Some(expansion) = facts.context(normalized)?.expansion else {
                return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
            };
            let Some((InputValue::ExpansionRecord(record), _)) =
                db.published_input(&InputKey::ExpansionRecord(Digest(expansion.0)))
            else {
                return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
            };
            let MacroIdentity::Declaration { definition } = &record.key().macro_identity else {
                return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
            };
            let Some(provider) = source_owner(db, *definition, owner, providers)? else {
                return Ok(Err(GeneratedTypePrerequisite::DefinitionSiteSource));
            };
            provider
        }
        RouteKind::DollarCrate => return Ok(Err(GeneratedTypePrerequisite::QualifiedModuleRoute)),
    };
    let source = provider.module_source();
    if source.selection().outline.crate_key != scope.crate_key
        || !scope
            .module_path
            .starts_with(&source.selection().outline.module_path)
        || scope.owner.is_some()
    {
        return Err(fault(
            "generated primitive origin actual owning source/scope differs",
        ));
    }
    Ok(Ok(GeneratedPrimitiveOrigin { scope, provider }))
}
impl SelectedGeneratedType {
    pub(crate) fn capture_current(
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        maximum: usize,
    ) -> Result<GeneratedTypeOutcome, Diagnostic> {
        Self::capture_with_providers(db, owner, parent, &[], maximum)
    }
    /// Providers are genuine retained owner evidence, never a completeness list.
    /// Their selected definition/source must independently match published facts.
    pub(crate) fn capture_with_providers(
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        providers: &[&SourceDeclarative],
        maximum: usize,
    ) -> Result<GeneratedTypeOutcome, Diagnostic> {
        if maximum > MAX_BYTES
            || providers.len() > MAX_ROWS
            || parent
                .metadata_bytes()
                .checked_mul(2)
                .is_none_or(|n| n > maximum)
        {
            return Err(limit());
        }
        parent.revalidate(db, owner)?;
        let definition = parent.retained().member().fingerprint();
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(definition))
        else {
            return Err(fault("generated written type registered source absent"));
        };
        let Some((InputValue::GeneratedSyntax(unit), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
        else {
            return Err(fault("generated written type actual syntax unit absent"));
        };
        let role = match parent.retained().form() {
            GeneratedConstantForm::Scalar { .. } => GeneratedTypeRole::Declaration,
            GeneratedConstantForm::Array { .. } => GeneratedTypeRole::ArrayElement,
        };
        let residual = maximum
            .checked_sub(parent.metadata_bytes())
            .ok_or_else(limit)?;
        let site = WrittenGeneratedType::capture(
            &unit.items,
            &unit.scope,
            &unit.cfg,
            parent.retained().member(),
            role,
            residual,
        )?;
        if site.scope().owner.is_some() {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::GenericOrAssociated,
            ));
        }
        if !unit
            .definitions
            .get(&definition)
            .is_some_and(|d| d.fingerprint() == site.declaration_fingerprint())
            || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)), Some((InputValue::DefinitionHeader(header), _)) if header.fingerprint() == site.header_fingerprint())
            || !matches!(db.published_input(&InputKey::DeclarationContext(definition)), Some((InputValue::DeclarationContext(ctx), _)) if ctx.scope == *site.scope() && ctx.range == site.declaration())
        {
            return Err(fault("generated written type actual registration changed"));
        }
        let mut bytes = parent
            .metadata_bytes()
            .checked_add(site.metadata_bytes())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= maximum)
            .ok_or_else(limit)?;
        let mut stamps = BTreeMap::new();
        for key in [
            InputKey::GeneratedSyntax(parse.content),
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DeclarationContext(definition),
            InputKey::ExpansionEpoch,
        ] {
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        let parts = match site.shape() {
            GeneratedTypeSyntax::Unit => {
                return Ok(GeneratedTypeOutcome::Selected(Self {
                    parent: parent.identity(),
                    site,
                    result: SelectedSourceType::IntrinsicUnit,
                    route: RouteKind::CallSite,
                    selected_header: None,
                    alias_site: None,
                    stamps,
                    revision: db.revision(),
                    bytes,
                }));
            }
            GeneratedTypeSyntax::GenericOrProjection => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::GenericOrAssociated,
                ));
            }
            GeneratedTypeSyntax::AbsolutePath => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::QualifiedModuleRoute,
                ));
            }
            GeneratedTypeSyntax::Unsupported => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::UnsupportedWrittenType,
                ));
            }
            GeneratedTypeSyntax::Path(parts) => parts,
        };
        let Some(last) = parts.last() else {
            return Err(fault("generated type path empty"));
        };
        let GeneratedTypeSegmentKind::Name(name) = last.kind() else {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::QualifiedModuleRoute,
            ));
        };
        if parts.len() > 2 {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::QualifiedModuleRoute,
            ));
        }
        let first = &parts[0];
        let (mut scope, mut route) = match origin_scope(
            db,
            owner,
            providers,
            unit,
            site.scope(),
            first,
            &mut stamps,
            &mut bytes,
            maximum,
        )? {
            Ok(found) => found,
            Err(need) => return Ok(GeneratedTypeOutcome::Need(need)),
        };
        if matches!(route, RouteKind::DefinitionSite)
            && site.scope() != &unit.scope
            && !matches!(
                first.kind(),
                GeneratedTypeSegmentKind::Keyword(TypeKeyword::DollarCrate)
            )
        {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::DefinitionSiteSource,
            ));
        }
        if parts.len() == 2 {
            match first.kind() {
                GeneratedTypeSegmentKind::Keyword(TypeKeyword::SelfModule) => {}
                GeneratedTypeSegmentKind::Keyword(TypeKeyword::SuperModule) => {
                    if scope.module_path.pop().is_none() {
                        return Ok(GeneratedTypeOutcome::Need(
                            GeneratedTypePrerequisite::QualifiedModuleRoute,
                        ));
                    }
                }
                GeneratedTypeSegmentKind::Keyword(TypeKeyword::Crate) => scope.module_path.clear(),
                GeneratedTypeSegmentKind::Keyword(TypeKeyword::DollarCrate) => {
                    let facts = unit.items.hygiene_facts();
                    // Requires the frozen pin-backed dollar_crate parent-chain
                    // attachment. A spelling or direct newest mark is insufficient.
                    let origin = facts.dollar_crate_origin(first.span().context)?;
                    if origin.normalized_context() != facts.local(first.span().context)? {
                        return Err(fault("dollar-crate normalized origin differs"));
                    }
                    let mark = facts.context(origin.mark())?;
                    let record_key = InputKey::ExpansionRecord(Digest(origin.expansion().0));
                    let Some((InputValue::ExpansionRecord(record), _)) =
                        db.published_input(&record_key)
                    else {
                        return Ok(GeneratedTypeOutcome::Need(
                            GeneratedTypePrerequisite::DefinitionSiteSource,
                        ));
                    };
                    if record.key().fingerprint().0 != origin.expansion().0
                        || mark.expansion != Some(origin.expansion())
                        || mark.defining_unit != origin.unit()
                    {
                        return Err(fault("dollar-crate selected mark/record/owner differs"));
                    }
                    let MacroIdentity::Declaration {
                        definition: macro_definition,
                    } = &record.key().macro_identity
                    else {
                        return Ok(GeneratedTypeOutcome::Need(
                            GeneratedTypePrerequisite::DefinitionSiteSource,
                        ));
                    };
                    let Some(definition_owner) =
                        source_owner(db, *macro_definition, owner, providers)?
                    else {
                        return Ok(GeneratedTypeOutcome::Need(
                            GeneratedTypePrerequisite::DefinitionSiteSource,
                        ));
                    };
                    let macro_source = definition_owner.module_source();
                    let macro_item = macro_source
                        .outline()
                        .items
                        .iter()
                        .find(|i| i.def_key.fingerprint() == *macro_definition)
                        .ok_or_else(|| fault("dollar-crate actual macro declaration absent"))?;
                    if macro_item.def_key.item_kind != ItemKind::Macro
                        || macro_item.scope.owner.is_some()
                        || macro_item.scope.crate_key.0 != origin.unit().0
                        || macro_source.outline().profile.edition != record.key().definition_edition
                    {
                        return Ok(GeneratedTypeOutcome::Need(
                            GeneratedTypePrerequisite::DefinitionSiteSource,
                        ));
                    }
                    charge(&mut bytes, macro_item.scope.heap_bytes(), maximum)?;
                    let macro_context = DeclarationContext {
                        scope: macro_item.scope.clone(),
                        range: macro_item.declaration_span,
                    };
                    if !matches!(db.published_input(&InputKey::DefinitionHeader(*macro_definition)), Some((InputValue::DefinitionHeader(actual), _)) if actual == &macro_item.header)
                        || !matches!(db.published_input(&InputKey::DeclarationContext(*macro_definition)), Some((InputValue::DeclarationContext(actual), _)) if actual == &macro_context)
                    {
                        return Err(fault(
                            "dollar-crate selected original header/context differs",
                        ));
                    }
                    for key in [
                        record_key,
                        InputKey::DefinitionSource(*macro_definition),
                        InputKey::DefinitionHeader(*macro_definition),
                        InputKey::DeclarationContext(*macro_definition),
                        InputKey::CapturedSource(macro_source.selection().location_identity()),
                    ] {
                        stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
                    }
                    // The context's owning edition remains independent of the
                    // caller and is checked against the actual declaration source.
                    if mark.edition
                        != crate::expand::source_declarative::edition(
                            record.key().definition_edition,
                        )?
                    {
                        return Err(fault("dollar-crate selected mark owning edition differs"));
                    }
                    scope.crate_key = macro_item.scope.crate_key;
                    scope.module_path.clear();
                    route = RouteKind::DollarCrate;
                }
                _ => {
                    return Ok(GeneratedTypeOutcome::Need(
                        GeneratedTypePrerequisite::QualifiedModuleRoute,
                    ));
                }
            }
        }
        let seal_key = SealKey {
            domain: SealDomain::Scope {
                id: scope.fingerprint(),
                namespace: crate::source::manifest::Namespace::Type,
            },
            unit_graph: unit.seal.unit_graph,
            epoch: unit.seal.epoch,
        };
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(seal_key.clone()))
        else {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::TypeNamespace(scope),
            ));
        };
        let certified = input
            .scope
            .as_ref()
            .ok_or_else(|| fault("generated selected Type scope absent"))?;
        if input.seal.key() != seal_key
            || certified.scope != scope
            || certified.namespace != crate::source::manifest::Namespace::Type
            || !matches!(db.published_input(&InputKey::ExpansionEpoch), Some((InputValue::ExpansionEpoch(epoch),_)) if *epoch==seal_key.epoch)
            || !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph),_)) if graph.fingerprint()==input.source_dependencies)
        {
            return Err(fault(
                "generated selected Type scope current world/epoch differs",
            ));
        }
        stamp(
            db,
            &mut stamps,
            &InputKey::DiscoverySeal(seal_key),
            &mut bytes,
            maximum,
        )?;
        stamp(
            db,
            &mut stamps,
            &InputKey::SourceDependencies,
            &mut bytes,
            maximum,
        )?;
        if input.compiler_inputs.len() > MAX_ROWS || input.records.len() > MAX_ROWS {
            return Err(limit());
        }
        for (key, expected) in &input.compiler_inputs {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != *expected)
            {
                return Err(fault("generated type scope compiler input changed"));
            }
            stamp(db, &mut stamps, key, &mut bytes, maximum)?;
        }
        charge(
            &mut bytes,
            scope
                .heap_bytes()
                .checked_add(name.capacity().checked_mul(2).ok_or_else(limit)?)
                .ok_or_else(limit)?,
            maximum,
        )?;
        for descriptor in &input.records {
            let key = InputKey::ExpansionRecord(descriptor.key.fingerprint());
            if !matches!(db.published_input(&key),Some((InputValue::ExpansionRecord(record),_)) if record.descriptor().fingerprint()==descriptor.fingerprint())
            {
                return Err(fault("selected Type namespace retained record changed"));
            }
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        let bucket = BucketKey {
            scope: scope.clone(),
            namespace: crate::source::manifest::Namespace::Type,
            name: name.clone(),
        };
        let found = match certified.lookup(&bucket)? {
            NameLookup::Defined(found) => found,
            NameLookup::Absent => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::PrimitiveEnvironment,
                ));
            }
            other => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::TypeName(other),
                ));
            }
        };
        let Some(provider) = source_owner(db, found.identity, owner, providers)? else {
            let comparison = unit.items.item_class(last.span().context)?;
            return Self::capture_generated_alias(
                db, parent, site, found, comparison, route, input, stamps, bytes, maximum,
            );
        };
        let source = provider.module_source();
        let actual = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.fingerprint() == found.identity)
            .ok_or_else(|| fault("generated selected source member absent"))?;
        if actual.scope.owner.is_some() {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::GenericOrAssociated,
            ));
        }
        if found.namespace != crate::source::manifest::Namespace::Type
            || found.defining_unit != actual.scope.crate_key
            || found.definition_edition != source.outline().profile.edition
            || !input.sources.contains(source.selection())
        {
            return Err(fault(
                "generated selected member owning source/profile differs",
            ));
        }
        let header = &actual.header;
        let context = DeclarationContext {
            scope: actual.scope.clone(),
            range: actual.declaration_span,
        };
        if !matches!(db.published_input(&InputKey::DefinitionHeader(found.identity)), Some((InputValue::DefinitionHeader(h), _)) if h == header)
            || !matches!(db.published_input(&InputKey::DeclarationContext(found.identity)), Some((InputValue::DeclarationContext(c),_)) if c == &context)
        {
            return Err(fault("generated selected source header/context changed"));
        }
        // A qualified route still needs the final segment's comparison class to
        // equal the genuine selected source owner's original item root class.
        let (definition_root_key, provider_facts) =
            provider.macro_index().original_root_hygiene()?;
        let definition_root = provider_facts.context(definition_root_key)?;
        if definition_root.parent.is_some()
            || definition_root.defining_unit.0 != actual.scope.crate_key.0
        {
            return Err(fault(
                "generated selected declaration original root fact differs",
            ));
        }
        let generated_facts = unit.items.hygiene_facts();
        let comparison = generated_facts.name_item(last.span().context)?;
        let comparable = match route {
            RouteKind::CallSite => comparison == provider_facts.name_item(definition_root_key)?,
            RouteKind::DefinitionSite | RouteKind::DollarCrate => {
                comparison == generated_facts.name_item(first.span().context)?
            }
        };
        if !comparable {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::DefinitionSiteSource,
            ));
        }
        charge(
            &mut bytes,
            header
                .estimated_bytes()
                .checked_add(context.scope.heap_bytes())
                .ok_or_else(limit)?,
            maximum,
        )?;
        for key in [
            InputKey::DefinitionSource(found.identity),
            InputKey::DefinitionHeader(found.identity),
            InputKey::DeclarationContext(found.identity),
            InputKey::CapturedSource(source.selection().location_identity()),
            InputKey::SourceBlob(source.selection().outline.parse.content),
            InputKey::CrateCfg(source.selection().outline.crate_key),
        ] {
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        let retained_parse = db
            .prepared_parse(&source.selection().outline.parse)
            .ok_or_else(|| fault("selected type retained original syntax prerequisite"))?;
        let retained_node = retained_parse
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find(|n| {
                crate::syntax::ast_adapter::range(n.text_range()) == context.range
                    && (ast::TypeAlias::can_cast(n.kind())
                        || ast::Struct::can_cast(n.kind())
                        || ast::Enum::can_cast(n.kind())
                        || ast::Union::can_cast(n.kind()))
            })
            .ok_or_else(|| fault("selected type exact retained header node absent"))?;
        if retained_node.children().any(|n| {
            ast::GenericParamList::can_cast(n.kind()) || ast::WhereClause::can_cast(n.kind())
        }) {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::GenericOrAssociated,
            ));
        }
        let result = match actual.def_key.item_kind {
            ItemKind::TypeAlias => {
                let parsed = db
                    .prepared_parse(&source.selection().outline.parse)
                    .ok_or_else(|| {
                        fault("generated selected alias retained original syntax prerequisite")
                    })?;
                let alias = parsed
                    .parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .filter_map(ast::TypeAlias::cast)
                    .find(|a| {
                        crate::syntax::ast_adapter::range(a.syntax().text_range()) == context.range
                    })
                    .ok_or_else(|| fault("generated selected alias exact source node absent"))?;
                if alias.generic_param_list().is_some() || alias.where_clause().is_some() {
                    return Ok(GeneratedTypeOutcome::Need(
                        GeneratedTypePrerequisite::GenericOrAssociated,
                    ));
                }
                if alias.ty().is_some_and(
                    |t| matches!(t, ast::Type::TupleType(u) if u.fields().next().is_none()),
                ) {
                    SelectedSourceType::AliasUnit(found)
                } else {
                    SelectedSourceType::AliasHeader(found)
                }
            }
            ItemKind::Struct | ItemKind::Enum | ItemKind::Union => {
                SelectedSourceType::NominalHeader(found)
            }
            _ => {
                return Ok(GeneratedTypeOutcome::Need(
                    GeneratedTypePrerequisite::TraitOrForeignType,
                ));
            }
        };
        Ok(GeneratedTypeOutcome::Selected(Self {
            parent: parent.identity(),
            site,
            result,
            route,
            selected_header: Some((header.clone(), context)),
            alias_site: None,
            stamps,
            revision: db.revision(),
            bytes,
        }))
    }
    #[allow(clippy::too_many_arguments)]
    fn capture_generated_alias(
        db: &Database,
        parent: &CurrentGeneratedConstant,
        site: WrittenGeneratedType,
        found: NameDefinition,
        comparison: crate::syntax::token_wire::NameContextKey,
        route: RouteKind,
        input: &crate::db::seals::DiscoveryInput,
        mut stamps: BTreeMap<InputKey, Digest>,
        mut bytes: usize,
        maximum: usize,
    ) -> Result<GeneratedTypeOutcome, Diagnostic> {
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(found.identity))
        else {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::ReplacementOrGeneratedMember,
            ));
        };
        let Some((InputValue::GeneratedSyntax(target), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
        else {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::ReplacementOrGeneratedMember,
            ));
        };
        if target.key != *parse
            || target.seal.epoch != input.seal.key().epoch
            || target.seal.unit_graph != input.seal.key().unit_graph
            || target.items.epoch() != target.seal.epoch
            || target.scope.crate_key != found.defining_unit
            || target.key.profile.edition != found.definition_edition
        {
            return Err(fault(
                "generated selected alias current unit/profile/world differs",
            ));
        }
        let Some(definition) = target.definitions.get(&found.identity) else {
            return Err(fault(
                "generated selected alias registered definition absent",
            ));
        };
        if definition.item_kind != ItemKind::TypeAlias {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::ReplacementOrGeneratedMember,
            ));
        }
        let header = target
            .items
            .items()
            .iter()
            .find(|h| h.key.fingerprint() == found.identity)
            .ok_or_else(|| fault("generated alias actual structural header absent"))?;
        let name = header
            .name
            .as_ref()
            .ok_or_else(|| fault("generated alias actual name origin absent"))?;
        let origin = target
            .items
            .tokens()
            .spans()
            .iter()
            .find(|s| s.key() == name.span)
            .ok_or_else(|| fault("generated alias name token span absent"))?;
        if target.items.item_class(origin.context)? != comparison {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::DefinitionSiteSource,
            ));
        }
        let descriptor = target.items.record().descriptor();
        if input
            .records
            .iter()
            .filter(|r| r.fingerprint() == descriptor.fingerprint())
            .count()
            != 1
            || input
                .seal
                .input_dependencies()
                .binary_search(&(
                    target.items.record().key().fingerprint(),
                    descriptor.fingerprint(),
                ))
                .is_err()
        {
            return Err(fault(
                "generated alias record absent from exact current Type world",
            ));
        }
        let record_key = InputKey::ExpansionRecord(target.items.record().key().fingerprint());
        if !matches!(db.published_input(&record_key), Some((InputValue::ExpansionRecord(record),_)) if record.same_record(target.items.record()))
        {
            return Err(fault("generated alias actual retained record changed"));
        }
        let residual = maximum.checked_sub(bytes).ok_or_else(limit)?;
        let alias_site = WrittenGeneratedType::capture(
            &target.items,
            &target.scope,
            &target.cfg,
            &header.key,
            GeneratedTypeRole::AliasTarget,
            residual,
        )?;
        if alias_site.scope().owner.is_some()
            || matches!(alias_site.shape(), GeneratedTypeSyntax::GenericOrProjection)
        {
            return Ok(GeneratedTypeOutcome::Need(
                GeneratedTypePrerequisite::GenericOrAssociated,
            ));
        }
        if definition.fingerprint() != alias_site.declaration_fingerprint() {
            return Err(fault("generated selected alias full declaration differs"));
        }
        let Some((InputValue::DefinitionHeader(header), _)) =
            db.published_input(&InputKey::DefinitionHeader(found.identity))
        else {
            return Err(fault("generated alias published header absent"));
        };
        let Some((InputValue::DeclarationContext(context), _)) =
            db.published_input(&InputKey::DeclarationContext(found.identity))
        else {
            return Err(fault("generated alias published context absent"));
        };
        if header.fingerprint() != alias_site.header_fingerprint()
            || context.scope != *alias_site.scope()
            || context.range != alias_site.declaration()
        {
            return Err(fault("generated alias published header/context differs"));
        }
        charge(&mut bytes, alias_site.metadata_bytes(), maximum)?;
        charge(
            &mut bytes,
            header
                .estimated_bytes()
                .checked_add(context.scope.heap_bytes())
                .ok_or_else(limit)?,
            maximum,
        )?;
        for key in [
            record_key,
            InputKey::GeneratedSyntax(parse.content),
            InputKey::DefinitionSource(found.identity),
            InputKey::DefinitionHeader(found.identity),
            InputKey::DeclarationContext(found.identity),
            InputKey::DiscoverySeal(target.seal.clone()),
        ] {
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        let result = if matches!(alias_site.shape(), GeneratedTypeSyntax::Unit) {
            SelectedSourceType::AliasUnit(found)
        } else {
            SelectedSourceType::AliasHeader(found)
        };
        Ok(GeneratedTypeOutcome::Selected(Self {
            parent: parent.identity(),
            site,
            result,
            route,
            selected_header: Some((header.clone(), context.clone())),
            alias_site: Some(alias_site),
            stamps,
            revision: db.revision(),
            bytes,
        }))
    }
    pub(crate) fn result(&self) -> &SelectedSourceType {
        &self.result
    }
    pub(crate) fn unit_primitive(&self) -> Option<PrimitiveType> {
        match &self.result {
            SelectedSourceType::IntrinsicUnit | SelectedSourceType::AliasUnit(_) => {
                Some(PrimitiveType::Unit)
            }
            _ => None,
        }
    }
    pub(crate) fn site(&self) -> &WrittenGeneratedType {
        &self.site
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn target_prerequisite(&self) -> Option<GeneratedTypePrerequisite> {
        match &self.result {
            SelectedSourceType::AliasHeader(_) => {
                Some(GeneratedTypePrerequisite::AliasTargetHygiene)
            }
            _ => None,
        }
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        providers: &[&SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        if self.revision != db.revision() || self.parent != parent.identity() {
            return Err(fault(
                "generated type publication/parent changed; remint required",
            ));
        }
        for (key, expected) in &self.stamps {
            if db
                .published_input(key)
                .is_none_or(|(_, current)| current != *expected)
            {
                return Err(fault("generated selected source type dependency changed"));
            }
        }
        let residual = MAX_BYTES.checked_sub(self.bytes).ok_or_else(limit)?;
        match Self::capture_with_providers(db, owner, parent, providers, residual)? {
            GeneratedTypeOutcome::Selected(fresh) if fresh.identity() == self.identity() => Ok(()),
            _ => Err(fault("generated selected source type replay changed")),
        }
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/generated-current-source-type");
        e.digest(self.parent);
        e.digest(self.site.identity());
        e.u8(match self.route {
            RouteKind::CallSite => 0,
            RouteKind::DefinitionSite => 1,
            RouteKind::DollarCrate => 2,
        });
        match &self.result {
            SelectedSourceType::IntrinsicUnit => e.u8(0),
            SelectedSourceType::AliasUnit(d) => {
                e.u8(1);
                e.record(d)
            }
            SelectedSourceType::AliasHeader(d) => {
                e.u8(2);
                e.record(d)
            }
            SelectedSourceType::NominalHeader(d) => {
                e.u8(3);
                e.record(d)
            }
        };
        e.option(&self.alias_site, |e, site| e.digest(site.identity()));
        e.option(&self.selected_header, |e, (h, c)| {
            e.record(h);
            e.record(&c.scope);
            e.u32(c.range.start);
            e.u32(c.range.end)
        });
        e.u64(self.stamps.len() as u64);
        for (key, value) in &self.stamps {
            e.record(key);
            e.digest(*value)
        }
        e.finish()
    }
}
#[cfg(test)]
#[path = "generated_type_admission_tests.rs"]
mod tests;
