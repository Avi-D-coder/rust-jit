//! Cold standalone entry joins existing source assembly and admission. No
//! expansion, expression evaluation or runtime macro recipe is performed here.
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    expand::source_discovery::SourceDiscovery,
    frontend::callables::{CallSelection, SourceCallable},
    source::changes::PreparedSourceView,
};

fn ready<T>(poll: QueryPoll<T>) -> Result<T, Diagnostic> {
    match poll {
        QueryPoll::Ready(result) => result,
        QueryPoll::NeedInput(input) => Err(Diagnostic::new(
            "LRR-E-CATALOG-INCOMPLETE",
            format!("standalone retained entry requires actual current input: {input:?}"),
        )),
    }
}

pub(crate) fn prepare(
    db: &mut Database,
    discovery: &SourceDiscovery,
    source: &PreparedSourceView,
    entry: &str,
    callable: Box<SourceCallable>,
) -> Result<Box<SourceCallable>, Diagnostic> {
    callable.revalidate(db)?;
    let definition = callable.identity();
    if callable.body().is_none() {
        return Ok(callable); // Preserve the entry service's no-body diagnostic.
    }
    // Macro-generated functions (a generated `main` included) join the SAME
    // retained admission: their owning source's plan attaches the issued
    // producer chain that wrote the declaration, then the calls their own body
    // owns. Nothing is lowered here. A body whose assembly is unavailable keeps
    // its unit publication and reports its own diagnostic only if reached.
    admit_generated_bodies(db, discovery)?;
    if !source
        .outline()
        .items
        .iter()
        .any(|item| item.def_key.fingerprint() == definition)
    {
        // A generated main: reselect it through the ordinary entry service so
        // a retained admission of its body is the one that runs.
        let selected = ready(discovery.entry_function(db, entry))?;
        let CallSelection::Source(selected) = selected else {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("standalone generated entry selection changed: {selected:?}"),
            ));
        };
        if selected.identity() != definition {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "standalone generated entry selected another declaration".into(),
            ));
        }
        selected.revalidate(db)?;
        ready(
            db.prepare_source_local_plan(
                discovery,
                &[source],
                &selected
                    .body()
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-CALL-CONTEXT",
                            "standalone generated entry body disappeared".into(),
                        )
                    })?
                    .key(),
            ),
        )?;
        return Ok(selected);
    }
    // Do not infer expansion absence from only ExpressionKind::Macro: source
    // declaration-slot calls can have other index roles. Existing scoped
    // assembly establishes readiness from all actual retained call occurrences.
    let body = admit_retained_body(db, discovery, source, definition)?;
    // Callee bodies join the same retained syntax admission before entry, so a
    // reached callee lowers its actual macro output exactly like `main`. This
    // attaches already-completed expansions only; nothing is lowered here. A
    // body whose assembly is unavailable keeps its original publication and
    // reports its own diagnostic only if it is ever reached.
    for other in macro_bearing_bodies(source)? {
        if other != definition {
            let _ = admit_retained_body(db, discovery, source, other);
        }
    }
    let selected = ready(discovery.entry_function(db, entry))?;
    let CallSelection::Source(selected) = selected else {
        return Err(Diagnostic::new(
            "LRR-E-CALL-CONTEXT",
            format!("standalone retained entry selection changed: {selected:?}"),
        ));
    };
    if selected.identity() != definition
        || selected.body().map(|handle| handle.key()).as_ref() != Some(&body)
    {
        return Err(Diagnostic::new(
            "LRR-E-CALL-CONTEXT",
            "standalone retained entry does not select its actual published body".into(),
        ));
    }
    selected.revalidate(db)?;
    ready(db.prepare_source_local_plan(discovery, &[source], &body))?;
    Ok(selected)
}

/// `lrr test`: each selected test function's body joins the same retained
/// admission as `main` (its source's macro-bearing callees and generated
/// functions included), so a test body lowers its actual macro output.
/// `None` keeps the catalog's own body (a body whose source is not an
/// original captured source, or whose assembly is unavailable, reports its
/// own diagnostic when reached).
pub(crate) fn prepare_test_bodies(
    db: &mut Database,
    discovery: &SourceDiscovery,
    sources: &[PreparedSourceView],
    tests: &[crate::core::digest::Digest],
) -> Result<std::collections::BTreeMap<crate::core::digest::Digest, crate::source::manifest::BodyKey>, Diagnostic> {
    let mut admitted = std::collections::BTreeMap::new();
    if tests.is_empty() || ready(discovery.completed_source_environment(db)).is_err() {
        return Ok(admitted);
    }
    admit_generated_bodies(db, discovery)?;
    let mut prepared_sources = std::collections::BTreeSet::new();
    for definition in tests {
        let Some(source) = sources.iter().find(|source| {
            source
                .outline()
                .items
                .iter()
                .any(|item| item.def_key.fingerprint() == *definition && item.body.is_some())
        }) else {
            continue;
        };
        if prepared_sources.insert(source.selection().path.as_str().to_owned()) {
            for other in macro_bearing_bodies(source)? {
                if !tests.contains(&other) {
                    let _ = admit_retained_body(db, discovery, source, other);
                }
            }
        }
        let Ok(body) = admit_retained_body(db, discovery, source, *definition) else {
            continue;
        };
        ready(db.prepare_source_local_plan(discovery, &[source], &body))?;
        admitted.insert(*definition, body);
    }
    Ok(admitted)
}
/// Shared retained admission for one original source function body.
fn admit_retained_body(
    db: &mut Database,
    discovery: &SourceDiscovery,
    source: &PreparedSourceView,
    definition: crate::core::digest::Digest,
) -> Result<crate::source::manifest::BodyKey, Diagnostic> {
    let completed = ready(discovery.completed_source_environment(db))?;
    let key =
        ready(db.publish_completed_source_syntax(&completed, source.selection(), definition))?;
    let prepared = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedBodySyntax(Box::new(key))))?;
    let QueryValue::PreparedBodySyntax(prepared_body) = prepared.value.as_ref() else {
        return Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "standalone retained body preparation result variant differs".into(),
        ));
    };
    if !prepared.cacheable {
        return Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "standalone retained body preparation was not memoized".into(),
        ));
    }
    let input = ready(prepared_body.admit(db))?;
    ready(db.publish_retained_syntax(input.clone()))?;
    ready(db.publish_retained_body(input, definition))
}
/// Retained admission of every published macro-generated function whose
/// own text contains a macro call, through its owning source's plan.
fn admit_generated_bodies(
    db: &mut Database,
    discovery: &SourceDiscovery,
) -> Result<(), Diagnostic> {
    let completed = ready(discovery.completed_source_environment(db))?;
    for owner in completed.sources() {
        let Ok(plan) = completed.source_syntax(owner.selection()) else {
            continue;
        };
        for definition in plan.generated_macro_bearing_functions() {
            if is_generated_function(db, definition) {
                let _ = admit_retained_body(db, discovery, owner, definition);
            }
        }
        // Associated functions written into original impls/traits have no
        // other publication: each enters through its retained admission.
        for member in plan.generated_associated_members()? {
            if member.provided {
                let _ = admit_retained_body(db, discovery, owner, member.identity);
            }
        }
    }
    Ok(())
}
/// A function declared by a published generated unit (not a body-local item).
fn is_generated_function(db: &mut Database, definition: crate::core::digest::Digest) -> bool {
    use crate::db::dependency::{InputKey, InputValue};
    let Some(InputValue::DefinitionSource(parse)) = db
        .input(&InputKey::DefinitionSource(definition))
        .as_deref()
        .cloned()
    else {
        return false;
    };
    matches!(db.input(&InputKey::GeneratedSyntax(parse.content)).as_deref(),
        Some(InputValue::GeneratedSyntax(unit)) if unit.definitions.get(&definition).is_some_and(|key| key.item_kind == crate::source::manifest::ItemKind::Function))
}
/// Original item bodies that contain at least one macro producer.
fn macro_bearing_bodies(
    source: &PreparedSourceView,
) -> Result<Vec<crate::core::digest::Digest>, Diagnostic> {
    let scan = source.declaration_inventory(Default::default())?;
    let calls: Vec<_> = scan
        .producers()
        .iter()
        .filter(|row| row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
        .map(|row| row.range)
        .collect();
    Ok(source
        .outline()
        .items
        .iter()
        .filter_map(|item| {
            let body = item.body.as_ref()?;
            calls
                .iter()
                .any(|call| body.range.start <= call.start && call.end <= body.range.end)
                .then(|| item.def_key.fingerprint())
        })
        .collect())
}

#[cfg(test)]
#[path = "retained_generated_body_tests.rs"]
mod generated_body_tests;
#[cfg(test)]
#[path = "retained_entry_tests.rs"]
mod tests;
