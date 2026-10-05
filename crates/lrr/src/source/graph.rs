//! The same bounded module selection validates disk capture and overlay batches.
use super::snapshot::{CfgSet, SourceBlob, SourcePath};
use crate::{core::diagnostic::Diagnostic, syntax::modules::ModuleRequest};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

pub fn module_target(
    path: &SourcePath,
    directory: &Path,
    request: ModuleRequest,
    mut exists: impl FnMut(&SourcePath) -> Result<bool, Diagnostic>,
) -> Result<(SourcePath, PathBuf), Diagnostic> {
    let source_parent = Path::new(path.as_str()).parent().unwrap_or(Path::new(""));
    let base = if request.file_relative {
        source_parent.join(request.directory.iter().collect::<PathBuf>())
    } else {
        directory.join(request.directory.iter().collect::<PathBuf>())
    };
    if let Some(override_path) = request.path_override {
        let override_base = if request.directory.is_empty() && !request.file_relative {
            source_parent
        } else {
            base.as_path()
        };
        let child = SourcePath::new(override_base.join(override_path))?;
        if !exists(&child)? {
            return Err(Diagnostic {
                code: "LRR-E-SOURCE-MISSING",
                message: format!("module source {} is missing", child.as_str()),
            });
        }
        let child_directory = Path::new(child.as_str())
            .parent()
            .unwrap_or(Path::new(""))
            .to_owned();
        Ok((child, child_directory))
    } else {
        let plain = SourcePath::new(base.join(format!("{}.rs", request.name)))?;
        let nested = SourcePath::new(base.join(&request.name).join("mod.rs"))?;
        match (exists(&plain)?, exists(&nested)?) {
            (true, true) => Err(Diagnostic {
                code: "LRR-E-MODULE-AMBIGUOUS",
                message: format!("both {} and {} exist", plain.as_str(), nested.as_str()),
            }),
            (true, false) => Ok((plain, base.join(request.name))),
            (false, true) => Ok((nested, base.join(request.name))),
            (false, false) => Err(Diagnostic {
                code: "LRR-E-SOURCE-MISSING",
                message: format!("module {} has no source file", request.name),
            }),
        }
    }
}

pub fn validate(
    root: &SourcePath,
    files: &BTreeMap<SourcePath, SourceBlob>,
    cfg: &CfgSet,
) -> Result<(), Diagnostic> {
    let directory = Path::new(root.as_str())
        .parent()
        .unwrap_or(Path::new(""))
        .to_owned();
    let mut queue = VecDeque::from([(root.clone(), directory)]);
    let mut visited = BTreeSet::new();
    while let Some((path, directory)) = queue.pop_front() {
        if !visited.insert((path.clone(), directory.clone())) {
            continue;
        }
        if visited.len() > 100_000 {
            return Err(Diagnostic {
                code: "LRR-E-SOURCE-LIMIT",
                message: "source module graph exceeds 100,000 contexts".into(),
            });
        }
        let blob = files.get(&path).ok_or_else(|| Diagnostic {
            code: "LRR-E-SOURCE-MISSING",
            message: format!("missing source {}", path.as_str()),
        })?;
        for request in crate::syntax::modules::external_modules(blob.text()?, cfg)? {
            queue.push_back(module_target(&path, &directory, request, |p| {
                Ok(files.contains_key(p))
            })?);
        }
    }
    Ok(())
}
