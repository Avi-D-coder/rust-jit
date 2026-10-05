//! In-memory source overlays: a validated batch publishes as one revision.
use super::snapshot::{Snapshot, SourceBlob, SourcePath};
use crate::core::diagnostic::Diagnostic;
use std::collections::BTreeSet;

pub fn apply_batch(
    base: &Snapshot,
    changes: Vec<(SourcePath, Option<Vec<u8>>)>,
) -> Result<Snapshot, Diagnostic> {
    let mut files = base.files().clone();
    let mut seen = BTreeSet::new();
    for (path, bytes) in changes {
        if !seen.insert(path.clone()) {
            return Err(Diagnostic {
                code: "LRR-E-SOURCE-BATCH",
                message: format!("duplicate source change {}", path.as_str()),
            });
        }
        match bytes {
            Some(bytes) => {
                files.insert(path, SourceBlob::new(bytes)?);
            }
            None if path == base.project().root_source => {
                return Err(Diagnostic {
                    code: "LRR-E-SOURCE-BATCH",
                    message: "cannot delete selected root".into(),
                });
            }
            None => {
                files.remove(&path);
            }
        }
    }
    let cfg = base
        .cfg_by_crate()
        .get(&base.project().selected_crate)
        .ok_or_else(|| Diagnostic {
            code: "LRR-E-SOURCE-MISSING",
            message: "selected crate cfg is missing".into(),
        })?;
    super::graph::validate(&base.project().root_source, &files, cfg)?;
    Snapshot::new(
        base.project().clone(),
        files,
        base.cfg_by_crate().clone(),
        base.compile_env().clone(),
        Some(base.id()),
    )
}
