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
    let name = request.name.clone();
    let (candidates, directory) = module_candidates(path, directory, request)?;
    let selected = match candidates.as_slice() {
        [one] if exists(one)? => one.clone(),
        [one] => {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-MISSING",
                format!("module source {} is missing", one.as_str()),
            ));
        }
        [plain, nested] => match (exists(plain)?, exists(nested)?) {
            (true, false) => plain.clone(),
            (false, true) => nested.clone(),
            (true, true) => {
                return Err(Diagnostic::new(
                    "LRR-E-MODULE-AMBIGUOUS",
                    format!("both {} and {} exist", plain.as_str(), nested.as_str()),
                ));
            }
            (false, false) => {
                return Err(Diagnostic::new(
                    "LRR-E-SOURCE-MISSING",
                    format!("module {name} has no source file"),
                ));
            }
        },
        _ => {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-MODULE",
                "module candidate count differs".into(),
            ));
        }
    };
    Ok((selected, directory))
}
/// Pure candidate layout shared by captured and dynamically discovered modules.
pub fn module_candidates(
    path: &SourcePath,
    directory: &Path,
    request: ModuleRequest,
) -> Result<(Vec<SourcePath>, PathBuf), Diagnostic> {
    let (spellings, directory) = module_candidate_spellings(path, directory, request)?;
    Ok((
        spellings
            .into_iter()
            .map(SourcePath::new)
            .collect::<Result<Vec<_>, _>>()?,
        directory,
    ))
}
/// Keep raw components for capability resolution, including symlink-before-parent semantics.
pub fn module_candidate_spellings(
    path: &SourcePath,
    directory: &Path,
    request: ModuleRequest,
) -> Result<(Vec<PathBuf>, PathBuf), Diagnostic> {
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
        let child = override_base.join(override_path);
        let child_directory = child.as_path().parent().unwrap_or(Path::new("")).to_owned();
        Ok((vec![child], child_directory))
    } else {
        let plain = base.join(format!("{}.rs", request.name));
        let nested = base.join(&request.name).join("mod.rs");
        Ok((vec![plain, nested], base.join(request.name)))
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
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "source module graph exceeds 100,000 contexts".into(),
            ));
        }
        let blob = files.get(&path).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-SOURCE-MISSING",
                format!("missing source {}", path.as_str()),
            )
        })?;
        for request in crate::syntax::modules::external_modules(blob.text()?, cfg)? {
            queue.push_back(module_target(&path, &directory, request, |p| {
                Ok(files.contains_key(p))
            })?);
        }
    }
    Ok(())
}
