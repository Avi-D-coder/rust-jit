//! Common current Type-namespace proof for reached primitive name lookup.
//! Call contracts and ordinary expressions share this source-owned decision.
use super::names::NameNamespaces;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::QueryPoll,
        seals::SealDomain,
    },
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::outline::{HeaderRecipe, ScopeKey},
};
use std::collections::BTreeMap;

/// These are actual published declaration facts, never a synthetic signature.
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
    let missing = || {
        fault(
            "LRR-E-CALL-CONTEXT",
            "primitive name requires its current owning Type namespace",
        )
    };
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(declaration.definition))
    else {
        return Err(missing());
    };
    let parse = parse.clone();
    // A body-only retained edit preserves its original declaration's Type
    // namespace. Prove that bridge from actual admitted source/rows, not a
    // primitive-looking return spelling or the result's runtime layout.
    let retained_owner = if db
        .published_input(&InputKey::RetainedSyntax(parse.content))
        .is_some()
    {
        let mut owners = sources.iter().filter(|view| {
            view.outline()
                .items
                .iter()
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
            .items
            .iter()
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
    let mut source = None;
    let mut captured_count = 0usize;
    for key in stamps.keys() {
        let InputKey::CapturedSource(location) = key else {
            continue;
        };
        captured_count = captured_count
            .checked_add(1)
            .filter(|n| *n <= 4096)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "primitive result source universe count",
                )
            })?;
        let Some((InputValue::CapturedSource(selection), _)) = db.published_input(key) else {
            return Err(missing());
        };
        if selection.location_identity() != *location {
            return Err(missing());
        }
        if selection.outline.parse == *owning_parse
            && selection.outline.crate_key == declaration.scope.crate_key
            && declaration
                .scope
                .module_path
                .starts_with(&selection.outline.module_path)
        {
            if source.is_some() {
                return Err(missing());
            }
            source = Some(selection);
        }
    }
    let Some(source) = source else {
        return Err(missing());
    };
    let Some(owner) = sources.iter().find(|view| view.selection() == source) else {
        return Err(missing());
    };
    owner.require_original_hygiene()?;
    if !owner.outline().items.iter().any(|item| {
        item.def_key.fingerprint() == declaration.definition
            && item.scope == *declaration.scope
            && item.header.fingerprint() == declaration.header.fingerprint()
            && (retained_owner.is_some() || item.header == *declaration.header)
    }) {
        return Err(missing());
    }
    let name =
        crate::expand::scopes::MacroName::parse(written_name, declaration.header.profile.edition)?;
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
            || input.sources.len() > 4096
            || input.compiler_inputs.len() > 4096
            || input.records.len() > 4096
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
        if scope.names.contains_key(name.as_str()) {
            return Err(missing());
        }
        return Ok(());
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
        if receipt.scope().namespace != Namespace::Type
            || receipt.scope().names.contains_key(name.as_str())
        {
            return Err(missing());
        }
        return Ok(());
    }
    // Plain start_with_sources has no discovery certificates. Reconstruct
    // only from the complete actual current retained original source set;
    // unknown producers/imports still cannot establish namespace absence.
    if sources.is_empty() || sources.len() != captured_count || sources.len() > 4096 {
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
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "primitive result original namespace bytes",
                )
            })?;
        for (key, expected) in view.prepared_inputs() {
            rows = rows
                .checked_add(1)
                .filter(|n| *n <= 100_000)
                .ok_or_else(|| {
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
    if !resolved.pending.is_empty()
        || !resolved.missing.is_empty()
        || !resolved.inaccessible.is_empty()
        || !resolved.ambiguous.is_empty()
        || current.lookup_unqualified(
            Namespace::Type,
            name.as_str(),
            declaration.scope,
            declaration.header.profile.edition,
            None,
        )? != super::names::NameLookup::Absent
    {
        return Err(missing());
    }
    Ok(())
}
