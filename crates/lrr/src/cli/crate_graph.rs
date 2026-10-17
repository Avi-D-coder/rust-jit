//! Standalone crate world: the selected root crate, rustc-style `--extern`
//! source dependencies and LRR embedded crates, captured into ONE source
//! publication with `SourceDependency { consumer, alias, provider }` rows.
//!
//! Command-line shape (rustc-compatible spelling):
//!
//! * `--extern [noprelude:]NAME=PATH` (repeatable) names a dependency crate
//!   whose root source file is PATH. Without `noprelude:` the root crate gets
//!   NAME in its extern prelude (rustc `--extern`); with it the crate is only
//!   reachable through `extern crate NAME;` (rustc `-L dependency=` search, as
//!   compiletest's `aux-build` uses it).
//! * `--extern-edition NAME=YEAR` gives that dependency crate its own owning
//!   edition. The default is the `--edition` value of the run.
//! * `--extern proc-macro:NAME=PATH` (LRR option, combinable with `noprelude`)
//!   names a procedural-macro crate root. It is not captured as interpreted
//!   source: explicit standalone preparation (`super::proc_macros`) compiles it
//!   as a host tool, exactly as compiletest's `//@ proc-macro:` directive does,
//!   and expansion runs only in the matched external macro server.
//!
//! The dependency graph is inferred like rustc's crate loading: the root
//! depends on every prelude `--extern` crate, and every captured crate (root,
//! dependency or embedded) depends on each known crate it names in an
//! `extern crate NAME` item. Each dependency crate's module tree is captured
//! exactly like the root's, under its own crate unit and owning edition.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        embedded_crates::{EmbeddedCrate, embedded_crates},
        loader::SourceLoader,
        snapshot::{CfgSet, Snapshot, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};

fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}

/// One `--extern [noprelude:]NAME=PATH` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternArg {
    pub name: String,
    pub path: PathBuf,
    /// False for `noprelude:`; the crate is then reachable only through
    /// `extern crate NAME;`.
    pub prelude: bool,
    /// `proc-macro:`; PATH is a procedural-macro crate root compiled by
    /// explicit preparation and executed only by the external macro server.
    pub proc_macro: bool,
}

fn crate_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!("crate name `{name}` must be a Rust identifier"))
    }
}

/// rustc `--extern [OPTIONS:]NAME=PATH`; OPTIONS is a comma-separated list of
/// `noprelude`, `priv`, `nounused` and `force` (the last three are inert here),
/// plus the LRR option `proc-macro` for a procedural-macro crate root.
pub(super) fn parse_extern(value: &str) -> Result<ExternArg, String> {
    let (head, path) = value
        .split_once('=')
        .ok_or_else(|| "expected `[noprelude:]NAME=PATH`".to_owned())?;
    let (options, name) = head.split_once(':').unwrap_or(("", head));
    let mut prelude = true;
    let mut proc_macro = false;
    for option in options.split(',').filter(|option| !option.is_empty()) {
        match option {
            "noprelude" => prelude = false,
            "proc-macro" => proc_macro = true,
            "priv" | "nounused" | "force" => {}
            other => return Err(format!("unknown `--extern` option `{other}`")),
        }
    }
    crate_name(name)?;
    if path.is_empty() {
        return Err(format!("`--extern {name}` requires a source path"));
    }
    Ok(ExternArg {
        name: name.into(),
        path: path.into(),
        prelude,
        proc_macro,
    })
}

/// `--extern-edition NAME=YEAR`.
pub(super) fn parse_extern_edition(value: &str) -> Result<(String, u16), String> {
    let (name, year) = value
        .split_once('=')
        .ok_or_else(|| "expected `NAME=YEAR`".to_owned())?;
    crate_name(name)?;
    let year = year
        .parse::<u16>()
        .map_err(|_| format!("invalid edition `{year}`"))?;
    Ok((name.into(), year))
}

/// Explicit dependency crates of one standalone run.
#[derive(Debug, Clone, Default)]
pub struct ExternCrates {
    pub crates: Vec<ExternArg>,
    pub editions: BTreeMap<String, u16>,
}
impl ExternCrates {
    pub(super) fn new(
        crates: Vec<ExternArg>,
        editions: Vec<(String, u16)>,
    ) -> Result<Self, Diagnostic> {
        let mut by_name = BTreeMap::new();
        for row in &crates {
            if let Some(old) = by_name.insert(row.name.as_str(), &row.path)
                && *old != row.path
            {
                return Err(fault(
                    "LRR-E-UNIT-GRAPH",
                    format!("`--extern {}` names two different sources", row.name),
                ));
            }
        }
        let mut selected = BTreeMap::new();
        for (name, edition) in editions {
            if !by_name.contains_key(name.as_str()) {
                return Err(fault(
                    "LRR-E-UNIT-GRAPH",
                    format!("`--extern-edition {name}` has no matching `--extern {name}`"),
                ));
            }
            if selected
                .insert(name.clone(), edition)
                .is_some_and(|old| old != edition)
            {
                return Err(fault(
                    "LRR-E-UNIT-GRAPH",
                    format!("`--extern-edition {name}` is given twice"),
                ));
            }
        }
        Ok(Self {
            crates,
            editions: selected,
        })
    }
}

/// The captured module tree of one crate, walked from its root file exactly
/// like rustc's module loading (`mod name;`, `#[path]`, cfg-disabled modules).
fn module_tree(
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
        let captured = files.get(&path).ok_or(fault(
            "LRR-E-INTERNAL-SNAPSHOT",
            "module source absent from captured snapshot",
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

fn embedded_units(
    embedded: &EmbeddedCrate,
    units: &mut Vec<SourceUnitInput>,
    selections: &mut Vec<SourceSelection>,
) -> Result<(), Diagnostic> {
    let profile = ProfileIdentity {
        edition: embedded.edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    if embedded
        .files
        .iter()
        .filter(|file| file.module_path.is_empty())
        .count()
        != 1
    {
        return Err(fault(
            "LRR-E-UNIT-GRAPH",
            format!(
                "embedded crate `{}` needs exactly one root file",
                embedded.name
            ),
        ));
    }
    for file in &embedded.files {
        let blob = SourceBlob::new(file.text.as_bytes().to_vec())?;
        let path = SourcePath::new(file.path)?;
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: embedded.unit,
                module_path: file.module_path.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: embedded.unit,
            module_path: file.module_path.clone(),
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    Ok(())
}

fn select(
    prepared: &PreparedSourceInputs,
    selection: &SourceSelection,
) -> Result<PreparedSourceView, Diagnostic> {
    match prepared.select(selection)? {
        SourceSelectionOutcome::Selected(view) => Ok(view),
        _ => Err(fault(
            "LRR-E-INTERNAL-SNAPSHOT",
            "captured source selection unavailable",
        )),
    }
}

pub(super) type CapturedWorld = (
    PreparedSourceInputs,
    PreparedSourceView,
    Vec<PreparedSourceView>,
    Arc<Snapshot>,
);

/// Capture the root crate, every `--extern` crate and every embedded crate in
/// one publication. Returns the root view and every captured module view
/// (root crate first, then dependencies), the root's file snapshot, and each
/// `--extern` crate's own file snapshot (keyed by its unit) for rendering.
pub(super) fn capture(
    path: &Path,
    test_mode: bool,
    edition: u16,
    externs: &ExternCrates,
) -> Result<(CapturedWorld, Vec<Arc<Snapshot>>), Diagnostic> {
    capture_keyed(path, test_mode, edition, externs, None)
}
/// `capture` whose root crate takes an explicitly prepared unit identity: a
/// standalone run with procedural-macro auxiliaries keys its root crate by the
/// prepared unit graph's consumer unit, so macro sessions, catalogs and source
/// selections name the same unit.
pub(super) fn capture_keyed(
    path: &Path,
    test_mode: bool,
    edition: u16,
    externs: &ExternCrates,
    root_unit: Option<Digest>,
) -> Result<(CapturedWorld, Vec<Arc<Snapshot>>), Diagnostic> {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    capture_with(
        path,
        test_mode,
        edition,
        externs,
        embedded_crates(&profile),
        root_unit,
    )
}
/// Re-key a captured standalone root snapshot to a prepared unit identity.
fn rekey_root(snapshot: Snapshot, unit: Digest) -> Result<Snapshot, Diagnostic> {
    let project = snapshot.project();
    let crate_key = project
        .crates
        .get(&project.selected_crate)
        .ok_or(fault("LRR-E-INTERNAL-SNAPSHOT", "captured root crate absent"))?
        .clone();
    let cfg = snapshot
        .cfg_by_crate()
        .get(&project.selected_crate)
        .ok_or(fault("LRR-E-INTERNAL-SNAPSHOT", "captured root cfg absent"))?
        .clone();
    let project = crate::source::snapshot::ProjectManifest {
        identity: project.identity.clone(),
        root_source: project.root_source.clone(),
        selected_crate: unit,
        crates: BTreeMap::from([(unit, crate_key)]),
    };
    Snapshot::new(
        project,
        snapshot.files().clone(),
        BTreeMap::from([(unit, cfg)]),
        snapshot.compile_env().clone(),
        snapshot.parent(),
    )
}
/// `capture` with an explicit embedded crate list.
pub(super) fn capture_with(
    path: &Path,
    test_mode: bool,
    edition: u16,
    externs: &ExternCrates,
    embedded: Vec<EmbeddedCrate>,
    root_unit: Option<Digest>,
) -> Result<(CapturedWorld, Vec<Arc<Snapshot>>), Diagnostic> {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let captured = SourceLoader::standalone_with_profile(path, profile.clone())?.capture()?;
    let snapshot = Arc::new(match root_unit {
        Some(unit) => rekey_root(captured, unit)?,
        None => captured,
    });
    let project = snapshot.project();
    let root_key = project.selected_crate;
    let mut cfg = snapshot
        .cfg_by_crate()
        .get(&root_key)
        .ok_or(fault("LRR-E-INTERNAL-SNAPSHOT", "captured root cfg absent"))?
        .clone();
    if test_mode {
        cfg.atoms.insert("test".into());
    }
    let mut units = Vec::new();
    let mut selections = Vec::new();
    module_tree(
        snapshot.files(),
        &project.root_source,
        root_key,
        &profile,
        &cfg,
        &mut units,
        &mut selections,
    )?;
    let root_selection = selections[0].clone();
    // Crate name -> (provider unit, in the root's extern prelude).
    let mut providers = BTreeMap::<String, (Digest, bool)>::new();
    let mut captured_paths = BTreeMap::<(PathBuf, u16), Digest>::new();
    let mut crate_roots = vec![root_key];
    let mut dependency_snapshots = Vec::new();
    for row in externs.crates.iter().filter(|row| !row.proc_macro) {
        let dependency_edition = externs.editions.get(&row.name).copied().unwrap_or(edition);
        let canonical = std::fs::canonicalize(&row.path).map_err(|error| {
            fault(
                "LRR-E-SOURCE-IO",
                format!("--extern {}={}: {error}", row.name, row.path.display()),
            )
        })?;
        let unit = match captured_paths.get(&(canonical.clone(), dependency_edition)) {
            Some(unit) => *unit,
            None => {
                let dependency_profile = ProfileIdentity {
                    edition: dependency_edition,
                    ..ProfileIdentity::current()
                };
                let captured =
                    SourceLoader::standalone_with_profile(&row.path, dependency_profile.clone())?
                        .capture()?;
                let dependency = captured.project();
                let unit = dependency.selected_crate;
                if unit == root_key {
                    return Err(fault(
                        "LRR-E-UNIT-GRAPH",
                        format!("`--extern {}` names the root crate itself", row.name),
                    ));
                }
                let dependency_cfg = captured
                    .cfg_by_crate()
                    .get(&unit)
                    .ok_or(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "captured dependency cfg absent",
                    ))?
                    .clone();
                module_tree(
                    captured.files(),
                    &dependency.root_source,
                    unit,
                    &dependency_profile,
                    &dependency_cfg,
                    &mut units,
                    &mut selections,
                )?;
                captured_paths.insert((canonical, dependency_edition), unit);
                crate_roots.push(unit);
                dependency_snapshots.push(Arc::new(captured));
                unit
            }
        };
        let prelude = providers
            .get(&row.name)
            .is_some_and(|(_, prelude)| *prelude)
            || row.prelude;
        providers.insert(row.name.clone(), (unit, prelude));
    }
    for embedded in embedded {
        // rustc: an explicit `--extern std=...` replaces the sysroot crate.
        if providers.contains_key(embedded.name) {
            continue;
        }
        if crate_roots.contains(&embedded.unit) {
            return Err(fault(
                "LRR-E-UNIT-GRAPH",
                format!(
                    "embedded crate `{}` unit is already captured",
                    embedded.name
                ),
            ));
        }
        embedded_units(&embedded, &mut units, &mut selections)?;
        crate_roots.push(embedded.unit);
        providers.insert(embedded.name.into(), (embedded.unit, false));
    }
    let mut outlined = None;
    let mut rows = Vec::new();
    if crate_roots.len() > 1 {
        // Outline every captured crate once to read its `extern crate` items;
        // the final refresh reuses those parses and outlines.
        let first = PreparedSourceInputs::refresh(None, units.clone(), Default::default())?;
        let mut edges = BTreeMap::<(Digest, String), Digest>::new();
        for (name, (unit, prelude)) in &providers {
            if *prelude {
                edges.insert((root_key, name.clone()), *unit);
            }
        }
        for selection in &selections {
            let view = select(&first, selection)?;
            let consumer = selection.outline.crate_key;
            for row in &view.outline().extern_crates {
                let name = row.dependency.strip_prefix("r#").unwrap_or(&row.dependency);
                if let Some((provider, _)) = providers.get(name)
                    && *provider != consumer
                {
                    edges.insert((consumer, name.to_owned()), *provider);
                }
            }
        }
        rows.extend(
            edges
                .into_iter()
                .map(|((consumer, alias), provider)| SourceDependency {
                    consumer,
                    alias,
                    provider,
                }),
        );
        outlined = Some(first);
    }
    let prepared = PreparedSourceInputs::refresh_with_dependencies(
        outlined.as_ref(),
        units,
        Default::default(),
        rows,
    )?;
    let views = selections
        .iter()
        .map(|selection| select(&prepared, selection))
        .collect::<Result<Vec<_>, _>>()?;
    let view = select(&prepared, &root_selection)?;
    Ok(((prepared, view, views, snapshot), dependency_snapshots))
}

#[cfg(test)]
#[path = "crate_graph_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "operator_dispatch_tests.rs"]
mod operator_dispatch_tests;
#[cfg(test)]
#[path = "box_tests.rs"]
mod box_tests;
#[cfg(test)]
#[path = "trait_path_call_tests.rs"]
mod trait_path_call_tests;
#[cfg(test)]
#[path = "numeric_constraint_facade_tests.rs"]
mod numeric_constraint_facade_tests;
#[cfg(test)]
#[path = "builtin_derive_tests.rs"]
mod builtin_derive_tests;
#[cfg(test)]
#[path = "format_trait_facade_tests.rs"]
mod format_trait_facade_tests;
