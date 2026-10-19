//! One ordinary cfg-aware module tree -> immutable source input construction.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    source::{
        changes::{SourceSelection, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
};

/// The captured module tree of one crate, walked from its root file with the
/// same `mod`, `#[path]` and cfg selection used for ordinary source capture.
pub(crate) fn capture_inputs(
    files: &BTreeMap<SourcePath, SourceBlob>,
    root: &SourcePath,
    crate_key: Digest,
    profile: &ProfileIdentity,
    cfg: &CfgSet,
    units: &mut Vec<SourceUnitInput>,
    selections: &mut Vec<SourceSelection>,
) -> Result<(), Diagnostic> {
    let mut queue = VecDeque::from([(
        root.clone(),
        Path::new(root.as_str())
            .parent()
            .unwrap_or(Path::new(""))
            .to_owned(),
        Vec::<String>::new(),
    )]);
    let mut contexts = BTreeSet::new();
    while let Some((path, directory, module_path)) = queue.pop_front() {
        if !contexts.insert((path.clone(), module_path.clone())) {
            continue;
        }
        let captured = files.get(&path).ok_or(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "module source absent from captured snapshot".into(),
        ))?;
        for request in
            crate::syntax::modules::external_modules_in_profile(captured.text()?, cfg, profile)?
        {
            let mut child_scope = module_path.clone();
            child_scope.extend(request.module_path.iter().cloned());
            let (child_path, child_directory) =
                crate::source::graph::module_target(&path, &directory, request, |candidate| {
                    Ok(files.contains_key(candidate))
                })?;
            queue.push_back((child_path, child_directory, child_scope));
        }
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: captured.content(),
                    profile: profile.clone(),
                },
                crate_key,
                module_path: module_path.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key,
            module_path,
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob: captured.clone(),
        });
    }
    Ok(())
}
