//! Common current Type-namespace proof for reached primitive name lookup.
//! Call contracts and ordinary expressions share this source-owned decision.
use super::names::{NameLookup, NameNamespaces};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{BucketKey, CurrentModuleNameKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::SealDomain,
        value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::outline::{HeaderRecipe, ScopeKey},
};
use std::collections::BTreeMap;

/// These are actual published declaration facts, never a synthetic signature.
#[derive(Clone, Copy)]
pub(crate) struct SourceTypeOwner<'a> {
    pub definition: Digest,
    pub scope: &'a ScopeKey,
    pub header: &'a HeaderRecipe,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub(crate) fn require_primitive_name(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    sources: &[&PreparedSourceView],
    stamps: &BTreeMap<InputKey, Digest>,
    written_name: &str,
) -> Result<(), Diagnostic> {
    require_selected_name(
        db,
        declaration,
        sources,
        stamps,
        written_name,
        &NameLookup::Absent,
    )
}
/// Exact current Type lookup, shared by primitive absence and source definitions.
pub(crate) fn require_selected_name(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    sources: &[&PreparedSourceView],
    stamps: &BTreeMap<InputKey, Digest>,
    written_name: &str,
    expected: &NameLookup,
) -> Result<(), Diagnostic> {
    if !matches!(expected, NameLookup::Absent | NameLookup::Defined(_)) {
        return Err(fault(
            "LRR-E-CALL-CONTEXT",
            "Type authority needs a completed selected lookup",
        ));
    }
    let actual = lookup_name(db, declaration, sources, stamps, written_name)?;
    if actual != *expected {
        return Err(fault(
            "LRR-E-CALL-CONTEXT",
            "primitive name requires its current owning Type namespace",
        ));
    }
    Ok(())
}

/// Shared current lookup for ordinary nominal/alias declaration contexts.
/// Generated headers enter the same origin/receipt core after actual memo proof.
pub(crate) fn lookup_declaration_name(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    sources: &[&PreparedSourceView],
    written_name: &str,
) -> QueryPoll<NameLookup> {
    let Some(source) = db.input(&InputKey::DefinitionSource(declaration.definition)) else {
        return QueryPoll::Ready(Err(fault(
            "LRR-E-CALL-CONTEXT",
            "Type declaration source absent",
        )));
    };
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return QueryPoll::Ready(Err(fault(
            "LRR-E-CALL-CONTEXT",
            "Type declaration source variant differs",
        )));
    };
    if db
        .published_input(&InputKey::GeneratedSyntax(parse.content))
        .is_some()
    {
        super::call_contracts::generated_result_namespace::lookup_type_declaration(
            db,
            declaration,
            parse,
            written_name,
        )
    } else {
        let stamps = match db.lexical_namespace_stamps() {
            Ok(stamps) => stamps,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(lookup_name(db, declaration, sources, &stamps, written_name))
    }
}

/// Ordinary declaration-local preparation and classification, shared by alias
/// targets and reached member type preparation. No synthetic function owner.
pub(crate) fn resolve_declaration_name(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    generics: &[crate::syntax::signatures::DefinitionParameter],
    sources: &[&PreparedSourceView],
    written_name: &str,
) -> QueryPoll<super::signature_names::SignatureName> {
    use super::signature_names::{declaration_type_name_local, declaration_type_name_with_lookup};
    match declaration_type_name_local(
        declaration.definition,
        declaration.scope,
        declaration.header.profile.edition,
        generics,
        written_name,
    ) {
        Ok(Some(local)) => return QueryPoll::Ready(Ok(local)),
        Ok(None) => {}
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    match lookup_declaration_name(db, declaration, sources, written_name) {
        QueryPoll::Ready(Ok(lookup)) => QueryPoll::Ready(declaration_type_name_with_lookup(
            declaration.definition,
            declaration.scope,
            declaration.header.profile.edition,
            generics,
            written_name,
            lookup,
        )),
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
    }
}

/// Obtain the actual completed lookup from this declaration's current Type
/// authority. Classification happens only after this source/namespace join.
pub(crate) fn lookup_name(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    sources: &[&PreparedSourceView],
    stamps: &BTreeMap<InputKey, Digest>,
    written_name: &str,
) -> Result<NameLookup, Diagnostic> {
    let missing = || {
        fault(
            "LRR-E-CALL-CONTEXT",
            "primitive name requires its current owning Type namespace",
        )
    };
    let Some(selected_source) = db.input(&InputKey::DefinitionSource(declaration.definition))
    else {
        return Err(missing());
    };
    let InputValue::DefinitionSource(parse) = selected_source.as_ref() else {
        return Err(missing());
    };
    let parse = parse.clone();
    // A body-only retained edit preserves its original declaration's Type
    // namespace. Prove that bridge from actual admitted source/rows, not a
    // primitive-looking return spelling or the result's runtime layout.
    // A macro-generated associated function retained in its original owner
    // (impl/trait item position) has no other declaration: macro_rules item
    // paths resolve at the invocation site, i.e. this owner's Type namespace.
    let associated_source = match db.published_input(&InputKey::RetainedSyntax(parse.content)) {
        Some((InputValue::RetainedSyntax(retained), _))
            if retained.key() == &parse
                && retained.generated_associated().is_some()
                && retained
                    .prepared()
                    .selected_generated()
                    .is_some_and(|anchor| anchor.definition() == declaration.definition) =>
        {
            Some(retained.prepared().source().clone())
        }
        _ => None,
    };
    let retained_owner = if let Some(selection) = &associated_source {
        let owner = *sources
            .iter()
            .find(|view| view.selection() == selection)
            .ok_or_else(missing)?;
        let proof = match db.retained_generated_declaration(declaration.definition) {
            QueryPoll::Ready(Ok(Some(proof))) => proof,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::Ready(Ok(None)) | QueryPoll::NeedInput(_) => return Err(missing()),
        };
        if proof.parse() != &parse
            || proof.context().scope != *declaration.scope
            || proof.header().fingerprint() != declaration.header.fingerprint()
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "retained generated associated header differs from its declaration",
            ));
        }
        Some(owner.selection())
    } else if db
        .published_input(&InputKey::RetainedSyntax(parse.content))
        .is_some()
    {
        let mut owners = sources.iter().filter(|view| {
            view.outline()
                .declaration_rows()
                .any(|item| item.def_key.fingerprint() == declaration.definition)
        });
        let owner = *owners.next().ok_or_else(missing)?;
        if owners.next().is_some() {
            return Err(missing());
        }
        let proof = match db.retained_original_declaration(owner, declaration.definition) {
            QueryPoll::Ready(Ok(Some(proof))) => proof,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::Ready(Ok(None)) | QueryPoll::NeedInput(_) => return Err(missing()),
        };
        let original = owner
            .outline()
            .declaration_rows()
            .find(|item| item.def_key.fingerprint() == declaration.definition)
            .ok_or_else(missing)?;
        if proof.parse() != &parse
            || proof.context().scope != *declaration.scope
            || proof.header().fingerprint() != declaration.header.fingerprint()
            || original.header.fingerprint() != declaration.header.fingerprint()
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "retained header changes require their own hygienic Type namespace",
            ));
        }
        Some(owner.selection())
    } else {
        None
    };
    let owning_parse = retained_owner.map_or(&parse, |source| &source.outline.parse);
    // Select the actual owner lease, not the complete published source world.
    // Retained bridges above have already proved their declaration identity.
    let mut owners = sources.iter().filter(|view| {
        let selection = view.selection();
        selection.outline.parse == *owning_parse
            && selection.outline.crate_key == declaration.scope.crate_key
            && declaration
                .scope
                .module_path
                .starts_with(&selection.outline.module_path)
    });
    let owner = *owners.next().ok_or_else(missing)?;
    if owners.next().is_some() {
        return Err(missing());
    }
    let source = owner.selection();
    owner.require_original_hygiene()?;
    if associated_source.is_none()
        && !owner.outline().declaration_rows().any(|item| {
            item.def_key.fingerprint() == declaration.definition
                && item.scope == *declaration.scope
                && item.header.fingerprint() == declaration.header.fingerprint()
                && (retained_owner.is_some() || item.header == *declaration.header)
        })
    {
        return Err(missing());
    }
    let name =
        crate::expand::scopes::MacroName::parse(written_name, declaration.header.profile.edition)?;
    if let Some(lookup) = certified_module_lookup(db, owner, declaration.scope, name.as_str())? {
        return Ok(lookup);
    }
    // Legacy unsealed/local-rib evidence below still needs its complete source
    // universe. Certified module answers above never construct that census.
    let mut captured_count = 0usize;
    for key in stamps.keys() {
        let InputKey::CapturedSource(location) = key else {
            continue;
        };
        captured_count = captured_count.checked_add(1).ok_or_else(missing)?;
        if !matches!(db.published_input(key), Some((InputValue::CapturedSource(selection), _))
            if selection.location_identity() == *location)
        {
            return Err(missing());
        }
    }
    // Prefer actual current Type discovery. Select no arbitrary world when
    // more than one current immutable scope certificate is published.
    let epoch = match db.published_input(&InputKey::ExpansionEpoch) {
        Some((InputValue::ExpansionEpoch(epoch), _)) => Some(*epoch),
        _ => None,
    };
    let mut selected = None;
    for key in stamps.keys() {
        let InputKey::DiscoverySeal(key) = key else {
            continue;
        };
        if Some(key.epoch) != epoch
            || !matches!(&key.domain,
                SealDomain::Scope {id, namespace: Namespace::Type} if *id == declaration.scope.fingerprint())
        {
            continue;
        }
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(key.clone()))
        else {
            return Err(missing());
        };
        if selected.is_some() {
            return Err(missing());
        }
        selected = Some((key, input));
    }
    if let Some((key, input)) = selected {
        key.validate()?;
        let Some(scope) = input.scope.as_ref() else {
            return Err(missing());
        };
        if input.seal.key() != *key
            || input.seal.unresolved_frontiers() != 0
            || scope.scope != *declaration.scope
            || scope.namespace != Namespace::Type
            || scope.edition != declaration.header.profile.edition
            || source.outline.parse.profile != declaration.header.profile
            || !input.sources.contains(source)
            || input.sources.len() != captured_count
            || !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph), _)) if graph.fingerprint() == input.source_dependencies)
        {
            return Err(missing());
        }
        let mut locations = std::collections::BTreeSet::new();
        for selected in &input.sources {
            if !locations.insert(selected.location_identity()) {
                return Err(missing());
            }
            if !matches!(db.published_input(&InputKey::CapturedSource(selected.location_identity())), Some((InputValue::CapturedSource(current), _)) if current == selected)
                || !matches!(db.published_input(&InputKey::CrateCfg(selected.outline.crate_key)), Some((InputValue::CrateCfg(cfg), _)) if cfg.fingerprint() == selected.outline.cfg)
            {
                return Err(missing());
            }
        }
        for (key, expected) in &input.compiler_inputs {
            if db
                .published_input(key)
                .is_none_or(|(_, current)| current != *expected)
            {
                return Err(missing());
            }
        }
        for descriptor in &input.records {
            if !matches!(db.published_input(&InputKey::ExpansionRecord(descriptor.key.fingerprint())), Some((InputValue::ExpansionRecord(record), _)) if record.descriptor().fingerprint() == descriptor.fingerprint())
            {
                return Err(missing());
            }
        }
        // Borrow this one canonical bucket instead of cloning the scope.
        return completed_bucket_lookup(&scope.names, name.as_str()).ok_or_else(missing);
    }
    // A genuine completed Value or TestCatalog publication may retain
    // independently certified Type facts. The parent domain alone never
    // establishes Type completion; the retained receipt must replay below.
    let mut nested = None;
    for key in stamps.keys() {
        let InputKey::DiscoverySeal(key) = key else {
            continue;
        };
        if Some(key.epoch) != epoch
            || !matches!(
                &key.domain,
                SealDomain::Scope {
                    namespace: Namespace::Value,
                    ..
                } | SealDomain::TestCatalog { .. }
            )
        {
            continue;
        }
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(key.clone()))
        else {
            return Err(missing());
        };
        if !input.sources.contains(source) {
            continue;
        }
        for receipt in &input.type_scopes {
            if receipt.scope().scope != *declaration.scope
                || receipt.scope().edition != declaration.header.profile.edition
            {
                continue;
            }
            if nested.is_some() {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "original primitive result has ambiguous current nested Type publications",
                ));
            }
            nested = Some((input, receipt));
        }
    }
    if let Some((input, receipt)) = nested {
        receipt.require_owner(declaration.scope, &declaration.header.profile)?;
        receipt.require_current(db, input)?;
        if receipt.scope().namespace != Namespace::Type {
            return Err(missing());
        }
        return completed_bucket_lookup(&receipt.scope().names, name.as_str()).ok_or_else(missing);
    }
    // Plain start_with_sources has no discovery certificates. Reconstruct
    // only from the complete actual current retained original source set;
    // unknown producers/imports still cannot establish namespace absence.
    if sources.is_empty() || sources.len() != captured_count {
        return Err(missing());
    }
    let mut bytes = 0usize;
    let mut rows = 0usize;
    let mut outlines = Vec::new();
    outlines
        .try_reserve_exact(sources.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    let mut locations = std::collections::BTreeSet::new();
    for view in sources {
        view.require_original_hygiene()?;
        let selection = view.selection();
        if !locations.insert(selection.location_identity())
            || !matches!(db.published_input(&InputKey::CapturedSource(selection.location_identity())), Some((InputValue::CapturedSource(current), _)) if current == selection)
        {
            return Err(missing());
        }
        bytes = bytes
            .checked_add(view.prepared_metadata_bytes()?)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "primitive result original namespace bytes",
                )
            })?;
        for (key, expected) in view.prepared_inputs() {
            rows = rows.checked_add(1).ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "primitive result original namespace input count",
                )
            })?;
            if db
                .published_input(key)
                .is_none_or(|(_, current)| current != expected.fingerprint())
            {
                return Err(missing());
            }
        }
        outlines.push(view.outline());
    }
    let mut current = NameNamespaces::from_outlines(&outlines)?;
    let resolved = current.resolve_imports()?;
    let lookup = current.lookup_unqualified(
        Namespace::Type,
        name.as_str(),
        declaration.scope,
        declaration.header.profile.edition,
        None,
    )?;
    if (!resolved.pending.is_empty()
        || !resolved.missing.is_empty()
        || !resolved.inaccessible.is_empty()
        || !resolved.ambiguous.is_empty())
        && matches!(lookup, NameLookup::Absent | NameLookup::Defined(_))
    {
        return Err(missing());
    }
    Ok(lookup)
}

/// A declaration's impl/trait is not a module namespace. Its actual source row
/// establishes the enclosing module; a function-local rib is never stripped.
/// The same retained module-name query supplies positive and negative answers.
fn certified_module_lookup(
    db: &mut Database,
    source: &PreparedSourceView,
    scope: &ScopeKey,
    name: &str,
) -> Result<Option<NameLookup>, Diagnostic> {
    let mut module_scope = scope.clone();
    if let Some(owner) = scope.owner {
        let associated_owner = source.outline().declaration_rows().find(|item| {
            item.scope.crate_key == scope.crate_key
                && item.scope.module_path == scope.module_path
                && match item.def_key.item_kind {
                    ItemKind::Impl => item.def_key.enclosing_impl == Some(owner),
                    ItemKind::Trait => item.def_key.fingerprint() == owner,
                    _ => false,
                }
        });
        let Some(associated_owner) = associated_owner else {
            return Ok(None);
        };
        // The owner's real enclosing scope can itself be a local rib. Do not
        // flatten local impl/trait declarations to the file's module.
        module_scope = associated_owner.scope.clone();
        if module_scope.owner.is_some() {
            return Ok(None);
        }
    }
    if db
        .current_scope_certificate(source.selection(), &module_scope, Namespace::Type)?
        .is_none()
    {
        return Ok(None);
    }
    let key = CurrentModuleNameKey {
        source: source.selection().outline.clone(),
        path: source.selection().path.as_str().to_owned(),
        bucket: BucketKey {
            scope: module_scope,
            namespace: Namespace::Type,
            name: name.to_owned(),
        },
    };
    let outcome = match db.fetch(
        &SyntaxProvider,
        QueryKey::CurrentModuleName(Box::new(key.clone())),
    ) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "owning module Type namespace requires preparation",
            ));
        }
    };
    match outcome.value.as_ref() {
        QueryValue::CurrentModuleName(observation) if observation.matches_request(&key) => {
            observation.lookup().cloned().map(Some).ok_or_else(|| {
                fault(
                    "LRR-E-CALL-CONTEXT",
                    "owning module Type namespace is not complete",
                )
            })
        }
        _ => Err(fault(
            "LRR-E-CALL-CONTEXT",
            "owning module Type namespace observation differs",
        )),
    }
}

/// A stored negative/incomplete row is not proof of absence. Completed scope
/// producers omit absent names and retain exact positive canonical identities.
pub(crate) fn completed_bucket_lookup(
    names: &BTreeMap<String, NameLookup>,
    name: &str,
) -> Option<NameLookup> {
    match names.get(name) {
        None => Some(NameLookup::Absent),
        Some(selected @ NameLookup::Defined(definition))
            if definition.namespace == Namespace::Type =>
        {
            Some(selected.clone())
        }
        _ => None,
    }
}

/// Preserve strict absence while allowing one exact completed positive bucket.
pub(crate) fn bucket_selects(
    names: &BTreeMap<String, NameLookup>,
    name: &str,
    expected: &NameLookup,
) -> bool {
    match expected {
        NameLookup::Absent => !names.contains_key(name),
        NameLookup::Defined(_) => names.get(name) == Some(expected),
        _ => false,
    }
}
