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
    // The existing selected-body admission consumes an actual original anchor.
    // Generated declarations without that anchor retain their previous route;
    // this adapter cannot fabricate a source owner or rewrite their BodyKey.
    // Common generated selected-body readiness remains a migration prerequisite.
    if !source
        .outline()
        .items
        .iter()
        .any(|item| item.def_key.fingerprint() == definition)
    {
        ready(db.prepare_source_local_plan(
            discovery,
            &[source],
            &callable.body().expect("body checked above").key(),
        ))?;
        return Ok(callable);
    }
    // Do not infer expansion absence from only ExpressionKind::Macro: source
    // declaration-slot calls can have other index roles. Existing scoped
    // assembly establishes readiness from all actual retained call occurrences.
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
    let body = ready(db.publish_retained_body(input, definition))?;
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

#[cfg(test)]
#[path = "retained_entry_tests.rs"]
mod tests;
