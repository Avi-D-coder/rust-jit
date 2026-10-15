//! Bounded current original-token provenance through generated macro definitions.
use super::*;
use crate::{
    expand::{
        generated_items::GeneratedHeaderKind, generated_syntax::GeneratedSyntaxInput,
        identity::MacroIdentity, record::ExpansionRecord,
    },
    syntax::{
        declaration_scan::DeclarationScanLimits,
        token_wire::{SpanRecord, Transparency},
    },
};
fn missing() -> Diagnostic {
    fault(
        "LRR-E-CALL-CONTEXT",
        "generated result requires its current macro-definition parent source chain",
    )
}
pub(super) fn is_generated_definition(db: &Database, record: &ExpansionRecord) -> bool {
    let MacroIdentity::Declaration { definition } = record.key().macro_identity else {
        return false;
    };
    let Some((InputValue::DefinitionSource(source), _)) =
        db.published_input(&InputKey::DefinitionSource(definition))
    else {
        return false;
    };
    db.published_input(&InputKey::GeneratedSyntax(source.content))
        .is_some()
}
pub(super) fn require(
    db: &Database,
    unit: &GeneratedSyntaxInput,
    publication: &crate::db::seals::DiscoveryInput,
    origins: &[&SpanRecord],
    name: &str,
) -> Result<(), Diagnostic> {
    let mut seen = Vec::new();
    seen.try_reserve_exact(16)
        .map_err(|_| crate::runtime::arena::resource())?;
    let mut bytes = 0usize;
    require_chain(db, unit, publication, origins, name, &mut seen, &mut bytes)
}
fn current_record(
    db: &Database,
    unit: &GeneratedSyntaxInput,
    publication: &crate::db::seals::DiscoveryInput,
) -> Result<(), Diagnostic> {
    let record = unit.items.record();
    if !matches!(db.published_input(&InputKey::CrateCfg(unit.scope.crate_key)), Some((InputValue::CrateCfg(cfg),_)) if cfg.fingerprint()==unit.cfg.fingerprint())
        || unit.seal != publication.seal.key()
        || unit.items.epoch() != unit.seal.epoch
        || !publication
            .records
            .iter()
            .any(|owned| owned.fingerprint() == record.descriptor().fingerprint())
        || !matches!(db.published_input(&InputKey::ExpansionRecord(record.key().fingerprint())),Some((InputValue::ExpansionRecord(actual),_)) if actual.same_record(record))
    {
        return Err(missing());
    }
    Ok(())
}
fn type_scope(
    db: &Database,
    publication: &crate::db::seals::DiscoveryInput,
    scope: &crate::syntax::outline::ScopeKey,
    profile: &crate::core::profile::ProfileIdentity,
    name: &str,
) -> Result<(), Diagnostic> {
    let mut receipts = publication
        .type_scopes
        .iter()
        .filter(|proof| proof.scope().scope == *scope && proof.scope().edition == profile.edition);
    let proof = receipts.next().ok_or_else(missing)?;
    if receipts.next().is_some() {
        return Err(missing());
    }
    proof.require_owner(scope, profile)?;
    proof.require_current(db, publication)?;
    let name = crate::expand::scopes::MacroName::parse(name, profile.edition)?;
    if proof.scope().namespace != Namespace::Type || proof.scope().names.contains_key(name.as_str())
    {
        return Err(missing());
    }
    Ok(())
}
fn require_chain(
    db: &Database,
    unit: &GeneratedSyntaxInput,
    publication: &crate::db::seals::DiscoveryInput,
    origins: &[&SpanRecord],
    name: &str,
    seen: &mut Vec<Digest>,
    bytes: &mut usize,
) -> Result<(), Diagnostic> {
    let record = unit.items.record();
    let identity = record.key().fingerprint();
    if seen.len() == 16 || seen.contains(&identity) {
        return Err(missing());
    }
    *bytes = bytes
        .checked_add(unit.metadata_bytes()?)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(crate::runtime::arena::resource)?;
    seen.push(identity);
    current_record(db, unit, publication)?;
    if let Some(view) = &unit.included_source {
        // Only the actual compiler include producer can terminate this chain.
        // Its immutable capture and every producing dependency were retained by
        // authorized discovery, separately from generated display text.
        view.require_record(record)?;
        view.require_published(db)?;
        if !matches!(record.key().macro_identity, MacroIdentity::Builtin { .. })
            || view.producer() != record.key()
            || view.location().unit != unit.scope.crate_key
        {
            return Err(missing());
        }
        for dependency in view.dependencies() {
            if publication.compiler_inputs.get(&dependency.key) != Some(&dependency.fingerprint) {
                return Err(missing());
            }
        }
        type_scope(db, publication, &unit.scope, &unit.key.profile, name)?;
        if origins.is_empty()
            || origins.iter().any(|origin| {
                origin.source.unit.0 != view.location().unit.0
                    || origin.source.content.0 != view.blob().content().0
                    || origin.source.path != view.location().path.as_str()
                    || origin.origin != view.origin()
                    || origin.context != view.caller_context()
                    || !view.tokens().spans().iter().any(|span| span == *origin)
            })
        {
            return Err(missing());
        }
        let _ = seen.pop();
        return Ok(());
    }
    let MacroIdentity::Declaration { definition } = record.key().macro_identity else {
        return Err(missing());
    };
    if let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(definition))
        && let Some((InputValue::GeneratedSyntax(parent), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
    {
        current_record(db, parent, publication)?;
        if parent.key != *parse || parent.items.record().key().fingerprint() == identity {
            return Err(missing());
        }
        let selected = parent.definitions.get(&definition).ok_or_else(missing)?;
        let header = parent
            .items
            .items()
            .iter()
            .find(|header| {
                header.key.fingerprint() == definition
                    && header.kind == GeneratedHeaderKind::MacroRules
            })
            .ok_or_else(missing)?;
        let Some((InputValue::DeclarationContext(context), _)) =
            db.published_input(&InputKey::DeclarationContext(definition))
        else {
            return Err(missing());
        };
        let Some((InputValue::DefinitionHeader(declared), _)) =
            db.published_input(&InputKey::DefinitionHeader(definition))
        else {
            return Err(missing());
        };
        if selected.item_kind != crate::source::manifest::ItemKind::Macro
            || selected.crate_key != context.scope.crate_key
            || selected.module_path != context.scope.module_path
            || context.scope.owner.is_some()
            || context.scope.crate_key != parent.scope.crate_key
            || !context
                .scope
                .module_path
                .starts_with(&parent.scope.module_path)
            || context.range.start != header.start
            || context.range.end != header.end
            || declared.profile != parent.key.profile
            || parent.items.macro_edition(definition)? != record.key().definition_edition
        {
            return Err(missing());
        }
        // The expansion insertion scope may contain an inline module whose
        // macro owns this declaration. Use its real nested Type receipt; the
        // retained outline below must still match that exact item and scope.
        type_scope(db, publication, &context.scope, &declared.profile, name)?;
        // Retained AST only: exact macro_rules token tree, no body lowering,
        // parsing generated display text, discovery or adapter expansion.
        let transient = parent
            .items
            .syntax_events()
            .len()
            .checked_mul(128)
            .and_then(|n| {
                parent
                    .items
                    .text()
                    .len()
                    .checked_mul(128)
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        bytes
            .checked_add(parent.metadata_bytes()?)
            .and_then(|n| n.checked_add(transient))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        let parsed = crate::syntax::parse::ParsedFile::from_generated_items(&parent.items)?;
        if parsed.estimated_bytes() > transient {
            return Err(crate::runtime::arena::resource());
        }
        *bytes = bytes
            .checked_add(parsed.estimated_bytes())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        if parsed.key() != *parse {
            return Err(missing());
        }
        let outline = parsed.outline(
            parent.scope.crate_key,
            parent.scope.module_path.clone(),
            &parent.cfg,
        )?;
        *bytes = bytes
            .checked_add(outline.estimated_bytes())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        if !outline.items.iter().any(|item| {
            &item.def_key == selected
                && item.scope == context.scope
                && item.declaration_span == context.range
                && item.header == *declared
        }) {
            return Err(missing());
        }
        let body = parsed.macro_token_group(context.range, true)?;
        let parent_origins = parent.items.project_source_spans(body)?;
        if parent_origins.is_empty() {
            return Err(missing());
        }
        let facts = unit.items.hygiene_facts();
        for origin in origins {
            let context = facts.context(origin.context)?;
            if context.transparency != Transparency::SemiTransparent
                || context.expansion.map(|key| key.0) != Some(identity.0)
                || context.defining_unit.0 != selected.crate_key.0
                || edition_year(context.edition) != record.key().definition_edition
                || !parent_origins.iter().any(|original| {
                    original.source == origin.source
                        && original.start == origin.start
                        && original.end == origin.end
                        && original.edition == origin.edition
                        && original.origin == origin.origin
                        && context.parent == Some(original.context)
                })
            {
                return Err(missing());
            }
        }
        require_chain(db, parent, publication, &parent_origins, name, seen, bytes)?;
    } else {
        // Terminal original macro declaration. Source and scan identities are
        // joined by actual current source/header/context and declaration range.
        let mut checked: Vec<(
            &crate::source::changes::SourceSelection,
            crate::syntax::spans::SourceRange,
        )> = Vec::new();
        checked
            .try_reserve_exact(origins.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for origin in origins {
            let facts = unit.items.hygiene_facts();
            let item = facts.context(facts.item(origin.context)?)?;
            if item.parent.is_some() || item.defining_unit != origin.source.unit {
                return Err(missing());
            }
            let mut sources = publication.sources.iter().filter(|source| {
                source.outline.crate_key.0 == origin.source.unit.0
                    && source.outline.parse.content.0 == origin.source.content.0
                    && source.path.as_str() == origin.source.path
            });
            let source = sources.next().ok_or_else(missing)?;
            if sources.next().is_some()
                || edition_year(item.edition) != source.outline.parse.profile.edition
                || edition_year(origin.edition) != source.outline.parse.profile.edition
                || record.key().definition_edition != source.outline.parse.profile.edition
            {
                return Err(missing());
            }
            let range = if let Some((_, range)) =
                checked.iter().find(|(selection, _)| *selection == source)
            {
                *range
            } else {
                let view = db.prepared_source(source)?.ok_or_else(missing)?;
                let scope = &view.outline().root_scope;
                let scan = view.declaration_inventory(DeclarationScanLimits::default())?;
                *bytes = bytes
                    .checked_add(scan.retained_bytes())
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(crate::runtime::arena::resource)?;
                let original = view.outline().items.iter().find(|item| {
                    item.def_key.fingerprint() == definition
                        && item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                        && item.scope == *scope
                });
                if let Some(item) = original
                    && (!matches!(db.published_input(&InputKey::DefinitionSource(definition)),Some((InputValue::DefinitionSource(parse),_)) if parse==&source.outline.parse)
                        || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)),Some((InputValue::DefinitionHeader(header),_)) if header==&item.header)
                        || !matches!(db.published_input(&InputKey::DeclarationContext(definition)),Some((InputValue::DeclarationContext(context),_)) if context.scope==item.scope && context.range==item.declaration_span))
                {
                    return Err(missing());
                }
                let mut candidates = scan.macro_definitions().iter().filter(|header| {
                    header.identity == definition
                        || original.is_some_and(|item| item.declaration_span == header.range)
                });
                let header = candidates.next().ok_or_else(missing)?;
                if candidates.next().is_some()
                    || !scan.lexical_scopes().iter().any(|lexical| {
                        lexical.parent.is_none() && lexical.identity == header.lexical_scope
                    })
                {
                    return Err(missing());
                }
                type_scope(db, publication, scope, &source.outline.parse.profile, name)?;
                checked.push((source, header.body));
                header.body
            };
            if origin.start > origin.end || origin.start < range.start || origin.end > range.end {
                return Err(missing());
            }
        }
    }
    let _ = seen.pop();
    Ok(())
}
