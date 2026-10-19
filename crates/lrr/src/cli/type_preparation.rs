//! One real cold Type fixed point before ordinary Value entry. This never
//! projects a Value seal or executes a guest expression/compiler effect itself.
use super::compile_inputs::PreparedCompileInputs;
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    expand::source_discovery::SourceDiscovery,
    source::{changes::PreparedSourceView, manifest::Namespace, snapshot::Snapshot},
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-PREPARATION", message.into())
}
pub(super) fn prepare(
    compiler: &PreparedCompileInputs,
    db: &mut Database,
    root: &PreparedSourceView,
    sources: &[PreparedSourceView],
    snapshot: &mut Arc<Snapshot>,
    mut procedural: Option<&mut super::proc_macros::ProceduralRun<'_>>,
) -> Result<SourceDiscovery, Diagnostic> {
    // This precedes discovery and any compiler capture. Query/run callers do
    // not gain authority merely by possessing a prepared compiler handle.
    db.require_source_preparation()?;
    if sources.is_empty() || root.outline().root_scope.owner.is_some() {
        return Err(fault("Type owning source count/scope differs"));
    }
    let mut owners = sources
        .iter()
        .filter(|source| source.selection() == root.selection());
    if owners
        .next()
        .is_none_or(|source| !source.same_publication(root))
        || owners.next().is_some()
    {
        return Err(fault("Type owning root is not its actual captured source"));
    }
    let current = db
        .prepared_source(root.selection())?
        .ok_or_else(|| fault("Type owning source publication absent"))?;
    if !current.same_publication(root) {
        return Err(fault("Type owning source publication changed"));
    }
    let epoch = db
        .input(&InputKey::ExpansionEpoch)
        .ok_or_else(|| fault("Type preparation requires actual published epoch"))?;
    let InputValue::ExpansionEpoch(epoch) = epoch.as_ref() else {
        return Err(fault("Type preparation epoch variant differs"));
    };
    if *epoch == 0 {
        return Err(fault("Type preparation epoch is zero"));
    }
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(sources.len())
        .map_err(|_| fault("Type source handle allocation"))?;
    let mut bytes = retained
        .capacity()
        .checked_mul(std::mem::size_of::<PreparedSourceView>())
        .ok_or_else(|| fault("Type source handle size overflow"))?;
    for source in sources {
        bytes = bytes
            .checked_add(
                source
                    .selection()
                    .metadata_bytes()?
                    .checked_mul(2)
                    .ok_or_else(|| fault("Type source selection size overflow"))?,
            )
            .ok_or_else(|| fault("Type source handle metadata limit"))?;
        retained.push(source.share());
    }
    let discovery = SourceDiscovery::new_deferred_imports(
        retained,
        root.outline().root_scope.clone(),
        Namespace::Type,
        procedural
            .as_deref()
            .map_or(root.selection().outline.crate_key, |run| run.graph()),
        *epoch,
    )?;
    let mut discovery = match procedural.as_deref() {
        Some(run) => run.configure(
            discovery.with_resolved_imports()?,
            compiler.environment_identity(),
            Arc::clone(&compiler.catalog),
        )?,
        None => discovery.with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            Arc::clone(&compiler.catalog),
        )?,
    };
    // Existing coordinator owns compiler include/env/module request adoption,
    // matched facets and diagnostic sources; procedural steps execute only
    // through the request's prepared macro run.
    compiler.discover_diagnostic_with(&mut discovery, db, snapshot,
        procedural.as_deref_mut().map(|run| run as &mut dyn crate::service::source_preparation::SourceExpansionExecutor))?;
    match procedural {
        Some(run) => run.publish(db, &mut discovery)?,
        None => discovery.publish(db)?,
    }
    Ok(discovery)
}
#[cfg(test)]
#[path = "type_preparation_tests.rs"]
mod tests;
