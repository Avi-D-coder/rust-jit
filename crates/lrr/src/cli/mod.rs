mod args;
mod bundle;
mod compile_inputs;
pub(crate) mod macro_export_modules;
mod type_preparation;
pub use compile_inputs::capture_macro_export_facet;
mod explain;
#[cfg(test)]
mod nested_operand_prefix;
#[cfg(test)]
mod panic_family_stock_tests;
mod preentry_loader;
#[cfg(test)]
mod preentry_loader_tests;
#[cfg(test)]
mod ref_pattern_tests;
mod render;
mod retained_entry;
mod runtime_capture;
#[cfg(test)]
mod runtime_capture_tests;
use crate::core::{diagnostic::Diagnostic, profile};
use clap::Parser;
#[derive(serde::Serialize)]
#[serde(untagged)]
enum StandaloneOutcome {
    Completed(crate::engine::run::RunReport),
    Panicked(crate::engine::run::PanicReport),
    Tests(TestSummary),
    Captured(Box<runtime_capture::Packet>),
}
#[cfg(test)]
fn capture_standalone(
    path: &std::path::Path,
    test_mode: bool,
) -> Result<
    (
        crate::source::changes::PreparedSourceInputs,
        crate::source::changes::PreparedSourceView,
        Vec<crate::source::changes::PreparedSourceView>,
        std::sync::Arc<crate::source::snapshot::Snapshot>,
    ),
    Diagnostic,
> {
    capture_standalone_edition(path, test_mode, profile::LIVE_EDITION)
}
fn capture_standalone_edition(
    path: &std::path::Path,
    test_mode: bool,
    edition: u16,
) -> Result<
    (
        crate::source::changes::PreparedSourceInputs,
        crate::source::changes::PreparedSourceView,
        Vec<crate::source::changes::PreparedSourceView>,
        std::sync::Arc<crate::source::snapshot::Snapshot>,
    ),
    Diagnostic,
> {
    use crate::{
        core::digest::StableEncode,
        db::key::OutlineKey,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            loader::SourceLoader,
        },
        syntax::parse::ParseKey,
    };
    let profile = crate::core::profile::ProfileIdentity {
        edition,
        ..crate::core::profile::ProfileIdentity::current()
    };
    let snapshot = std::sync::Arc::new(
        SourceLoader::standalone_with_profile(path, profile.clone())?.capture()?,
    );
    let project = snapshot.project();
    let blob = snapshot
        .files()
        .get(&project.root_source)
        .ok_or(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "captured root source absent".into(),
        ))?
        .clone();
    let mut cfg = snapshot
        .cfg_by_crate()
        .get(&project.selected_crate)
        .ok_or(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "captured root cfg absent".into(),
        ))?
        .clone();
    if test_mode {
        cfg.atoms.insert("test".into());
    }
    let mut queue = std::collections::VecDeque::from([(
        project.root_source.clone(),
        std::path::Path::new(project.root_source.as_str())
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_owned(),
        Vec::<String>::new(),
    )]);
    let mut contexts = std::collections::BTreeSet::new();
    let mut units = Vec::new();
    let mut selections = Vec::new();
    let mut metadata_bytes = 0usize;
    let mut queued_bytes = 0usize;
    while let Some((path, directory, module_path)) = queue.pop_front() {
        if !contexts.insert((path.clone(), module_path.clone())) {
            continue;
        }
        if contexts.len() > 100_000 {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "standalone source context limit".into(),
            ));
        }
        metadata_bytes = module_path
            .iter()
            .try_fold(metadata_bytes, |n, name| {
                n.checked_add(name.len()).and_then(|n| n.checked_add(64))
            })
            .and_then(|n| n.checked_add(path.as_str().len()))
            .and_then(|n| n.checked_add(512))
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "standalone source context metadata limit".into(),
            ))?;
        let captured = snapshot.files().get(&path).ok_or(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "module source absent from captured snapshot".into(),
        ))?;
        for request in
            crate::syntax::modules::external_modules_in_profile(captured.text()?, &cfg, &profile)?
        {
            let mut child_scope = module_path.clone();
            child_scope
                .len()
                .checked_add(request.module_path.len())
                .filter(|n| *n <= 256)
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "standalone module depth limit".into(),
                ))?;
            child_scope.extend(request.module_path.iter().cloned());
            let (child_path, child_directory) =
                crate::source::graph::module_target(&path, &directory, request, |candidate| {
                    Ok(snapshot.files().contains_key(candidate))
                })?;
            queued_bytes = child_scope
                .iter()
                .try_fold(queued_bytes, |n, name| {
                    n.checked_add(name.len()).and_then(|n| n.checked_add(64))
                })
                .and_then(|n| n.checked_add(child_path.as_str().len()))
                .and_then(|n| n.checked_add(child_directory.as_os_str().len()))
                .and_then(|n| n.checked_add(256))
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "standalone queued module metadata limit".into(),
                ))?;
            if queue.len() >= 100_000 {
                return Err(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "standalone module queue count limit".into(),
                ));
            }
            queue.try_reserve(1).map_err(|_| {
                Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "standalone module queue allocation".into(),
                )
            })?;
            queue.push_back((child_path, child_directory, child_scope));
        }
        selections.try_reserve(1).map_err(|_| {
            Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "standalone selection allocation".into(),
            )
        })?;
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: captured.content(),
                    profile: profile.clone(),
                },
                crate_key: project.selected_crate,
                module_path: module_path.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.try_reserve(1).map_err(|_| {
            Diagnostic::new("LRR-E-SOURCE-LIMIT", "standalone source allocation".into())
        })?;
        units.push(SourceUnitInput {
            path,
            crate_key: project.selected_crate,
            module_path,
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob: captured.clone(),
        });
    }
    let prepared = PreparedSourceInputs::refresh(None, units, Default::default())?;
    let mut views = Vec::new();
    views.try_reserve_exact(selections.len()).map_err(|_| {
        Diagnostic::new(
            "LRR-E-SOURCE-LIMIT",
            "standalone source view allocation".into(),
        )
    })?;
    for selected in &selections {
        let SourceSelectionOutcome::Selected(view) = prepared.select(selected)? else {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "captured source selection unavailable".into(),
            ));
        };
        views.push(view);
    }
    let selection = SourceSelection {
        path: project.root_source.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile,
            },
            crate_key: project.selected_crate,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection)? else {
        return Err(Diagnostic::new(
            "LRR-E-INTERNAL-SNAPSHOT",
            "captured root selection unavailable".into(),
        ));
    };
    Ok((prepared, view, views, snapshot))
}
#[cfg(test)]
fn standalone(
    path: &std::path::Path,
    authority: Option<&bundle::Authority>,
) -> Result<StandaloneOutcome, Diagnostic> {
    standalone_edition(path, authority, profile::LIVE_EDITION)
}
#[cfg(test)]
fn standalone_edition(
    path: &std::path::Path,
    authority: Option<&bundle::Authority>,
    edition: u16,
) -> Result<StandaloneOutcome, Diagnostic> {
    standalone_edition_with_capture(path, authority, edition, false, None)
}
fn standalone_edition_with_capture(
    path: &std::path::Path,
    authority: Option<&bundle::Authority>,
    edition: u16,
    capture: bool,
    context: Option<crate::core::digest::Digest>,
) -> Result<StandaloneOutcome, Diagnostic> {
    let (prepared, view, sources, mut snapshot) =
        match capture_standalone_edition(path, false, edition) {
            Ok(inputs) => inputs,
            Err(error) if capture => {
                return runtime_capture::Packet::preparation_fault(error, context)
                    .map(|p| StandaloneOutcome::Captured(Box::new(p)));
            }
            Err(error) => return Err(error),
        };
    let outcome = standalone_captured(
        path,
        prepared,
        view,
        sources,
        &mut snapshot,
        authority,
        runtime_capture::Request {
            enabled: capture,
            context,
        },
    )
    .map_err(|error| {
        if let Some(authority) = authority {
            authority.report(&error, Some(&snapshot), None);
        }
        render::attach(error, snapshot)
    });
    match outcome {
        Err(error) if capture => runtime_capture::Packet::preparation_fault(error, context)
            .map(|p| StandaloneOutcome::Captured(Box::new(p))),
        other => other,
    }
}
fn standalone_captured(
    path: &std::path::Path,
    prepared: crate::source::changes::PreparedSourceInputs,
    view: crate::source::changes::PreparedSourceView,
    sources: Vec<crate::source::changes::PreparedSourceView>,
    snapshot: &mut std::sync::Arc<crate::source::snapshot::Snapshot>,
    authority: Option<&bundle::Authority>,
    capture: runtime_capture::Request,
) -> Result<StandaloneOutcome, Diagnostic> {
    use crate::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let owning_edition = view.selection().outline.parse.profile.edition;
    let (compiler, prepared, view, sources) =
        compile_inputs::PreparedCompileInputs::prepare(path, prepared, view, sources)?;
    let mut db = Database::default();
    prepared.publish(&mut db)?;
    db.begin_expansion_epoch(1)?;
    // Retain genuine Type closure/source roots through the entire ordinary run.
    let _type_preparation =
        type_preparation::prepare(&compiler, &mut db, &view, &sources, snapshot)?;
    let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
        sources,
        view.outline().root_scope.clone(),
        crate::source::manifest::Namespace::Value,
        view.selection().outline.crate_key,
        1,
    )?
    .with_declarative_compiler(
        compiler.environment_identity(),
        crate::expand::source_declarative::engine_version(),
        compiler.catalog.clone(),
    )?;
    compiler.discover_diagnostic(&mut discovery, &mut db, snapshot)?;
    let macros =
        crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
            &view, &discovery,
        )?;
    let contexts = macros.contexts()?;
    discovery.publish(&mut db)?;
    let callable = match discovery.entry_function(&mut db, "main") {
        QueryPoll::Ready(Ok(crate::frontend::callables::CallSelection::Source(callable))) => {
            callable
        }
        QueryPoll::Ready(Ok(crate::frontend::callables::CallSelection::Unresolved(
            crate::frontend::names::NameLookup::Ambiguous(_),
        ))) => {
            return Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                "standalone root main has multiple declaration candidates".into(),
            ));
        }
        QueryPoll::Ready(Ok(_)) => {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "standalone root main must resolve to a source function".into(),
            ));
        }
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "standalone entry requires complete declaration inputs".into(),
            ));
        }
    };
    let callable = retained_entry::prepare(&mut db, &discovery, &view, callable)?;
    callable.revalidate(&mut db)?;
    let body = callable
        .body()
        .ok_or(Diagnostic::new(
            "LRR-E-PARSE-BLOCKS-ENTRY",
            "main has no executable body".into(),
        ))?
        .key();
    let startup = RunMachine::start_with_discovery(&mut db, body, 1_000_000, discovery);
    db.begin_run()?;
    let result = match startup {
        QueryPoll::Ready(Ok(mut run)) => {
            let enabled = if capture.enabled {
                run.enable_runtime_capture()
            } else {
                Ok(())
            };
            match enabled {
                Err(error) => Err(error),
                Ok(()) => {
                    let polled = run.poll_with_runtime_macros(&mut db, &contexts);
                    if capture.enabled {
                        if let RunPoll::Fault(error) = &polled
                            && let Some(authority) = authority
                        {
                            authority.report(error, Some(snapshot), Some(&run));
                        }
                        runtime_capture::Packet::observe(
                            &run,
                            polled,
                            snapshot,
                            owning_edition,
                            capture.context,
                        )
                        .map(|p| StandaloneOutcome::Captured(Box::new(p)))
                    } else {
                        match polled {
                            RunPoll::Completed(report) => Ok(StandaloneOutcome::Completed(report)),
                            RunPoll::Panicked(report) => Ok(StandaloneOutcome::Panicked(report)),
                            RunPoll::Fault(error) => {
                                if let Some(authority) = authority {
                                    authority.report(&error, Some(snapshot), Some(&run));
                                }
                                Err(error)
                            }
                            RunPoll::NeedInput(_) => Err(Diagnostic::new(
                                "LRR-E-UNSUPPORTED",
                                "source execution requires later macro/input authority integration"
                                    .into(),
                            )),
                        }
                    }
                }
            }
        }
        QueryPoll::Ready(Err(error)) => Err(error),
        QueryPoll::NeedInput(_) => Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "body discovery requires later macro/input authority integration".into(),
        )),
    };
    db.finish_run()?;
    result
}
#[derive(serde::Serialize)]
struct TestSummary {
    live_edition: u16,
    semantics_profile: &'static str,
    schema: &'static str,
    scope: &'static str,
    selected: usize,
    filtered_out: usize,
    listed: Vec<String>,
    results: Vec<crate::engine::test_harness::TestResult>,
}
fn test_source(
    source: &std::path::Path,
    selection: TestSelection<'_>,
    authority: Option<&bundle::Authority>,
    edition: u16,
) -> Result<StandaloneOutcome, Diagnostic> {
    let (prepared, view, sources, mut snapshot) =
        capture_standalone_edition(source, true, edition)?;
    test_source_captured(
        source,
        prepared,
        view,
        sources,
        &mut snapshot,
        selection,
        authority,
    )
    .map_err(|error| {
        if let Some(authority) = authority {
            authority.report(&error, Some(&snapshot), None);
        }
        render::attach(error, snapshot)
    })
}
struct TestSelection<'a> {
    filter: Option<&'a str>,
    exact: bool,
    list: bool,
    ignored: bool,
    include_ignored: bool,
}
fn test_source_captured(
    source: &std::path::Path,
    prepared: crate::source::changes::PreparedSourceInputs,
    view: crate::source::changes::PreparedSourceView,
    sources: Vec<crate::source::changes::PreparedSourceView>,
    snapshot: &mut std::sync::Arc<crate::source::snapshot::Snapshot>,
    selection: TestSelection<'_>,
    authority: Option<&bundle::Authority>,
) -> Result<StandaloneOutcome, Diagnostic> {
    use crate::{db::memo::Database, engine::test_harness::run_sealed_test};
    let TestSelection {
        filter,
        exact,
        list,
        ignored,
        include_ignored,
    } = selection;
    let (compiler, prepared, view, sources) =
        compile_inputs::PreparedCompileInputs::prepare(source, prepared, view, sources)?;
    let mut db = Database::default();
    prepared.publish(&mut db)?;
    db.begin_expansion_epoch(1)?;
    let mut discovery = crate::expand::source_discovery::SourceDiscovery::for_tests(
        sources,
        view.outline().root_scope.clone(),
        view.selection().outline.crate_key,
        1,
    )?
    .with_declarative_compiler(
        compiler.environment_identity(),
        crate::expand::source_declarative::engine_version(),
        compiler.catalog.clone(),
    )?;
    compiler.discover_diagnostic(&mut discovery, &mut db, snapshot)?;
    discovery.publish(&mut db)?;
    let catalog = discovery.test_catalog(&mut db)?;
    let sources = discovery.completed_test_sources()?;
    catalog.require_current(&mut db)?;
    let policy = if ignored {
        crate::engine::test_catalog::IgnorePolicy::Only
    } else if include_ignored {
        crate::engine::test_catalog::IgnorePolicy::Include
    } else {
        crate::engine::test_catalog::IgnorePolicy::Exclude
    };
    let selected = catalog.select_with_ignore_policy(filter, exact, policy);
    if selected.is_empty() {
        return Err(Diagnostic::new(
            "LRR-E-TEST-NO-MATCH",
            "no final tests match the selection".into(),
        ));
    }
    let mut summary = TestSummary {
        live_edition: view.selection().outline.parse.profile.edition,
        semantics_profile: profile::SEMANTICS_PROFILE,
        schema: "lrr/v2/development-tests",
        scope: "sealed-test-catalog-component",
        selected: selected.len(),
        filtered_out: catalog.cases().len() - selected.len(),
        listed: vec![],
        results: vec![],
    };
    if list {
        summary.listed = selected.iter().map(|case| case.name.clone()).collect();
    } else {
        let macros =
            crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
                &view, &discovery,
            )?;
        let contexts = macros.contexts()?;
        for case in selected {
            let mut result =
                run_sealed_test(&mut db, &sources, case, 1_000_000, &contexts, &catalog)?;
            if let Some(authority) = authority
                && let Some(error) = &result.diagnostic
            {
                authority.report(error, Some(snapshot), None);
            }
            if let Some(error) = result.diagnostic.take() {
                result.diagnostic = Some(render::attach(error, snapshot.clone()));
            }
            summary.results.push(result);
        }
    }
    Ok(StandaloneOutcome::Tests(summary))
}
/// Current standalone scalar entry. Unimplemented commands fail closed.
pub fn entry(program: &str) -> i32 {
    if let Err(error) = profile::require_supported_host() {
        eprintln!("{error}");
        return 2;
    }
    if program != "lrr" {
        eprintln!(
            "{}",
            Diagnostic::new(
                "LRR-E-NOT-YET-IMPLEMENTED",
                format!("{program}: command awaits its ordered CLI task")
            )
        );
        return 2;
    }
    let args = match args::Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            return code;
        }
    };
    let authority = args
        .diagnostic_dir
        .as_deref()
        .map(bundle::Authority::explicit);
    let result = match args.command {
        args::Command::Explain { code } => return explain::entry(&code),
        args::Command::SourceWorld { source, context } => {
            return preentry_loader::entry(&source, args.edition, context);
        }
        args::Command::Run {
            source,
            runtime_capture,
            capture_context,
        } => standalone_edition_with_capture(
            &source,
            authority.as_ref(),
            args.edition,
            runtime_capture,
            capture_context,
        ),
        args::Command::Test {
            source,
            filter,
            exact,
            list,
            ignored,
            include_ignored,
        } => test_source(
            &source,
            TestSelection {
                filter: filter.as_deref(),
                exact,
                list,
                ignored,
                include_ignored,
            },
            authority.as_ref(),
            args.edition,
        ),
    };
    match result {
        Ok(report) => {
            let failed = matches!(&report,StandaloneOutcome::Tests(summary) if summary.results.iter().any(|result| !result.passed()));
            if let StandaloneOutcome::Captured(packet) = &report {
                use std::io::Write;
                let bytes = match packet.render() {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        eprintln!("{error}");
                        return 2;
                    }
                };
                let stdout = std::io::stdout();
                let mut transport = stdout.lock();
                if transport
                    .write_all(&bytes)
                    .and_then(|()| transport.write_all(b"\n"))
                    .and_then(|()| transport.flush())
                    .is_err()
                {
                    eprintln!("LRR-E-RUNTIME-CAPTURE: protocol transport write failed");
                    return 2;
                }
                return packet.exit_code();
            }
            let code = if failed || matches!(report, StandaloneOutcome::Panicked(_)) {
                1
            } else {
                0
            };
            match serde_json::to_string(&report) {
                Ok(json) => {
                    println!("{json}");
                    code
                }
                Err(error) => {
                    eprintln!("LRR-E-INTERNAL-IR: result serialization failed: {error}");
                    2
                }
            }
        }
        Err(error) => {
            if let Some(authority) = &authority {
                authority.report(&error, None, None);
            }
            eprintln!("{error}");
            if error.code == "LRR-E-TEST-NO-MATCH" {
                5
            } else {
                2
            }
        }
    }
}
#[cfg(test)]
mod tests {
    fn require_current_validation_source() {
        if let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") {
            let expected = std::fs::canonicalize(std::path::Path::new(&expected)).unwrap();
            let compiled = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            assert_eq!(
                std::fs::canonicalize(compiled).unwrap(),
                expected,
                "validation binary belongs to another checkout"
            );
            let embedded: &[(&str, &[u8])] = &[
                (
                    "crates/lrr/src/engine/run.rs",
                    include_bytes!("../engine/run.rs"),
                ),
                (
                    "crates/lrr/src/engine/failure.rs",
                    include_bytes!("../engine/failure.rs"),
                ),
                (
                    "crates/lrr/src/syntax/origin.rs",
                    include_bytes!("../syntax/origin.rs"),
                ),
                (
                    "crates/lrr/src/expand/generated_call_request.rs",
                    include_bytes!("../expand/generated_call_request.rs"),
                ),
                (
                    "crates/lrr/src/source/include_view.rs",
                    include_bytes!("../source/include_view.rs"),
                ),
                (
                    "crates/lrr/src/source/snapshot.rs",
                    include_bytes!("../source/snapshot.rs"),
                ),
                (
                    "crates/lrr/src/expand/include_string.rs",
                    include_bytes!("../expand/include_string.rs"),
                ),
                (
                    "crates/lrr/src/cli/compile_inputs.rs",
                    include_bytes!("compile_inputs.rs"),
                ),
                (
                    "crates/lrr/src/expand/source_discovery.rs",
                    include_bytes!("../expand/source_discovery.rs"),
                ),
            ];
            let declaration_inputs: &[(&str, &[u8])] = &[
                (
                    "crates/lrr/src/frontend/impl_index.rs",
                    include_bytes!("../frontend/impl_index.rs"),
                ),
                (
                    "crates/lrr/src/frontend/nested_compile_inputs.rs",
                    include_bytes!("../frontend/nested_compile_inputs.rs"),
                ),
                (
                    "crates/lrr/src/expand/source_declarative.rs",
                    include_bytes!("../expand/source_declarative.rs"),
                ),
                (
                    "crates/lrr/src/expand/argument_declarations.rs",
                    include_bytes!("../expand/argument_declarations.rs"),
                ),
            ];
            for (relative, bytes) in embedded.iter().chain(declaration_inputs) {
                assert_eq!(
                    *bytes,
                    std::fs::read(expected.join(relative)).unwrap(),
                    "validation embedded source differs: {relative}"
                );
            }
        }
    }

    #[test]
    fn rustc_nested_compiler_operand_failure_keeps_captured_definition_and_parent() {
        require_current_validation_source();
        use crate::source::manifest::ItemKind;
        use crate::syntax::origin::MacroFailureKind;
        let stock = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
        ));
        let original = std::fs::read(stock).unwrap();
        assert_eq!(
            original.as_slice(),
            include_bytes!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
            )
        );
        let (_, view, _, _) = super::capture_standalone(stock, false).unwrap();
        let header = view
            .outline()
            .items
            .iter()
            .find(|item| {
                item.def_key.item_kind == ItemKind::Macro
                    && item.def_key.declared_name == "one_arg_macro"
            })
            .unwrap();
        let retained = std::str::from_utf8(
            &original[header.declaration_span.start as usize..header.declaration_span.end as usize],
        )
        .unwrap();
        let temp = std::env::temp_dir().join(format!(
            "lrr-m016-nested-stock-operand-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let path = temp.join("adapter.rs");
        for prefix in ["", "\n\n"] {
            // Explicit LRR origin adaptation outside the licensed stock copy:
            // retain the exact stock macro declaration and empty invocation,
            // place it inside concat's eager operand, and exercise an edit.
            let source = format!(
                "{prefix}{retained}\nfn main() {{ assert_eq!(concat!(one_arg_macro!()), \"\"); }}\n"
            );
            std::fs::write(&path, &source).unwrap();
            let error = match super::standalone(&path, None) {
                Err(error) => error,
                Ok(_) => panic!("stock missing-argument macro unexpectedly expanded"),
            };
            assert_eq!(error.code, "LRR-E-MACRO-EXPANSION", "{error:?}");
            assert!(error.origin_attachment_failure.is_none(), "{error:?}");
            let origin = error.macro_origin.as_ref().unwrap();
            assert!(matches!(
                origin.failure_kind,
                Some(MacroFailureKind::Binding)
            ));
            let definition = origin.definition.as_ref().unwrap();
            assert_eq!(definition.name, "one_arg_macro");
            assert_eq!(
                &source[definition.location.range.start as usize
                    ..definition.location.range.end as usize],
                "macro_rules! one_arg_macro"
            );
            let invocation = origin.invocation.as_ref().unwrap();
            assert_eq!(
                &source[invocation.range.start as usize..invocation.range.end as usize],
                "one_arg_macro"
            );
            assert_eq!(origin.parents.len(), 1);
            let parent = &origin.parents[0];
            assert!(parent.invocation_identity.is_some());
            let parent_site = parent.invocation.as_ref().unwrap();
            assert_eq!(
                &source[parent_site.range.start as usize..parent_site.range.end as usize],
                "("
            );
            assert_eq!(
                parent_site.range.start as usize,
                source.find("concat!(one_arg_macro!())").unwrap() + "concat!".len()
            );
            assert_eq!(parent_site.source, invocation.source);
            assert_eq!(definition.location.source, invocation.source);
            let rendered = error.rendered_macro_origin.as_ref().unwrap();
            let call = rendered.invocation.as_ref().unwrap();
            let declared = rendered.definition.as_ref().unwrap();
            let producer = rendered.parents[0].as_ref().unwrap();
            assert_eq!(call.file.as_str(), "adapter.rs");
            assert_eq!(declared.file, call.file);
            assert_eq!(producer.file, call.file);
            assert_eq!(call.range, invocation.range);
            assert_eq!(declared.range, definition.location.range);
            assert_eq!(producer.range, parent_site.range);
            for (location, at) in [
                (call, invocation),
                (declared, &definition.location),
                (producer, parent_site),
            ] {
                assert_eq!(
                    location.start_line as usize,
                    source[..at.range.start as usize]
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        + 1
                );
                assert_eq!(location.unit.0, at.source.unit.0);
            }
            assert_eq!(std::fs::read(stock).unwrap(), original);
        }
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn rustc_original_nested_concat_expands_source_operand_from_argument_owner() {
        require_current_validation_source();
        use crate::{
            db::memo::Database,
            expand::include_arguments::{IncludeArguments, IncludeOperand},
        };
        use lrr_expand_adapter::token_wire::TokenTree;
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-deep_expansion.rs"
        ));
        let original = std::fs::read(path).unwrap();
        assert_eq!(
            original.as_slice(),
            include_bytes!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-deep_expansion.rs"
            )
        );
        let (prepared, view, sources, _) = super::capture_standalone(path, false).unwrap();
        let (compiler, prepared, view, sources) =
            super::compile_inputs::PreparedCompileInputs::prepare(path, prepared, view, sources)
                .unwrap();
        let source = crate::expand::source_declarative::SourceDeclarative::new(view.share())
            .unwrap()
            .owned_forest()
            .unwrap();
        fn assertion_groups<'a>(trees: &'a [TokenTree], result: &mut Vec<&'a TokenTree>) {
            for window in trees.windows(3) {
                if matches!(&window[0], TokenTree::Ident { text, .. } if text == "assert_eq")
                    && matches!(&window[1], TokenTree::Punct { scalar: '!', .. })
                    && matches!(&window[2], TokenTree::Group { .. })
                {
                    result.push(&window[2]);
                }
            }
            for tree in trees {
                if let TokenTree::Group { children, .. } = tree {
                    assertion_groups(children, result);
                }
            }
        }
        let mut assertions = Vec::new();
        assertion_groups(source.roots(), &mut assertions);
        assert_eq!(assertions.len(), 1);
        let assertion = lrr_expand_adapter::token_wire::TokenForest::new(
            vec![assertions[0].clone()],
            source.contexts().to_vec(),
            source.spans().to_vec(),
        )
        .unwrap();
        let operands = lrr_expand_adapter::runtime_arguments::expressions(&assertion).unwrap();
        assert_eq!(operands.len(), 2);
        let descriptor = crate::expand::compile_input_identity::descriptor("include")
            .unwrap()
            .unwrap();
        let expected =
            crate::syntax::token_wire::TokenForest::decode(&operands[1].encode().unwrap()).unwrap();
        let expected = IncludeArguments::parse(&descriptor, &expected).unwrap();
        let IncludeOperand::Literal {
            value: expected, ..
        } = expected.operand()
        else {
            panic!("stock expected operand is not a literal")
        };
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            sources,
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Value,
            view.selection().outline.crate_key,
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            compiler.catalog.clone(),
        )
        .unwrap();
        assert!(discovery.sealed_declaration_expansions().is_err());
        compiler.discover(&mut discovery, &mut db).unwrap();
        discovery.publish(&mut db).unwrap();
        let definition = compiler
            .catalog
            .definitions()
            .find(|definition| {
                compiler
                    .catalog
                    .select(definition)
                    .unwrap()
                    .is_some_and(|descriptor| descriptor.name() == "concat")
            })
            .unwrap();
        let records = discovery.sealed_declaration_expansions().unwrap();
        let matches = records.iter().filter(|record| matches!(&record.key().macro_identity, crate::expand::identity::MacroIdentity::Builtin { definition: selected } if *selected == definition.identity)).collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        let output = matches[0].tokens().unwrap();
        let produced = IncludeArguments::parse(&descriptor, &output).unwrap();
        let IncludeOperand::Literal { value, .. } = produced.operand() else {
            panic!("stock nested concat output is not a literal")
        };
        assert_eq!(value, expected);
        assert!(matches[0].key().procedural_epoch.is_none());
        assert_eq!(std::fs::read(path).unwrap(), original);
        // This checks whole-source discovery and actual macro output against the
        // original assertion operand. Guest string/assert execution and full
        // upstream run-pass acceptance remain unexercised at this milestone.
    }

    #[test]
    fn rustc_include_discovery_failure_keeps_admitted_parent_capture_and_primary_fault() {
        require_current_validation_source();
        use crate::{
            db::memo::Database,
            source::snapshot::{Snapshot, SourceBlob, SourcePath},
        };
        let upstream = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/include-single-expr-helper.rs"
        ));
        let temp = std::env::temp_dir().join(format!(
            "lrr-m016-partial-stock-include-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp.join("stock")).unwrap();
        std::fs::copy(upstream, temp.join("stock/include-single-expr-helper.rs")).unwrap();
        // This LRR origin/admission adaptation inserts the unchanged numeric helper
        // into an item slot to demand a real nested include parse failure.
        // It does not claim the original expression-slot compiletest assertion.
        std::fs::write(temp.join("main.rs"), "include!(\"bridge.rs\");\n").unwrap();
        std::fs::write(
            temp.join("bridge.rs"),
            "include!(\"stock/include-single-expr-helper.rs\");\n",
        )
        .unwrap();
        let path = temp.join("main.rs");
        let error = match super::standalone(&path, None) {
            Err(error) => error,
            Ok(_) => panic!("stock numeric include incorrectly admitted as items"),
        };
        assert_eq!(error.code, "LRR-E-INCLUDE-PARSE", "{error:?}");
        assert!(error.origin_attachment_failure.is_none(), "{error}");
        let original_message = error.message.clone();
        let origin = error.macro_origin.as_ref().unwrap();
        assert_eq!(origin.parents.len(), 1);
        assert_eq!(origin.invocation.as_ref().unwrap().source.path, "bridge.rs");
        assert_eq!(
            origin.parents[0].invocation.as_ref().unwrap().source.path,
            "main.rs"
        );
        let at = error.rendered_macro_origin.as_ref().unwrap();
        assert_eq!(at.invocation.as_ref().unwrap().file.as_str(), "bridge.rs");
        assert_eq!(at.parents[0].as_ref().unwrap().file.as_str(), "main.rs");
        assert!(
            error
                .to_string()
                .contains("stock/include-single-expr-helper.rs")
        );
        for location in [
            at.invocation.as_ref().unwrap(),
            at.parents[0].as_ref().unwrap(),
        ] {
            assert_eq!(location.start_line, 1);
            // The builtin API site is the captured opening parenthesis of
            // include!(...), not a fabricated range spanning its name.
            assert_eq!(location.start_column, 9);
            assert_eq!(location.range.start, 8);
            assert_eq!(location.range.end, 9);
        }
        // A conflicting coordinator snapshot is a separate diagnostic defect.
        // It must never conceal the already reached include parse failure.
        let (prepared, view, sources, snapshot) = super::capture_standalone(&path, false).unwrap();
        let (compiler, prepared, view, sources) =
            super::compile_inputs::PreparedCompileInputs::prepare(&path, prepared, view, sources)
                .unwrap();
        let mut files = snapshot.files().clone();
        files.insert(
            SourcePath::new("bridge.rs").unwrap(),
            SourceBlob::new(b"fn stale() {}".to_vec()).unwrap(),
        );
        let mut conflicting = std::sync::Arc::new(
            Snapshot::new(
                snapshot.project().clone(),
                files,
                snapshot.cfg_by_crate().clone(),
                snapshot.compile_env().clone(),
                Some(snapshot.id()),
            )
            .unwrap(),
        );
        let conflict_id = conflicting.id();
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            sources,
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Value,
            view.selection().outline.crate_key,
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            compiler.catalog.clone(),
        )
        .unwrap();
        let primary = compiler
            .discover_diagnostic(&mut discovery, &mut db, &mut conflicting)
            .unwrap_err();
        assert_eq!(primary.code, "LRR-E-INCLUDE-PARSE");
        assert_eq!(primary.message, original_message);
        assert_eq!(
            primary.origin_attachment_failure,
            Some("LRR-E-INTERNAL-SOURCE-MAP")
        );
        assert_eq!(
            conflicting.id(),
            conflict_id,
            "conflicting capture was silently replaced"
        );
        assert!(
            discovery.sealed_expansions().is_err(),
            "failed include acquired a discovery seal"
        );
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn rustc_include_single_expression_failure_retains_original_invocation() {
        require_current_validation_source();
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/include-single-expr.rs"
        ));
        let error = match super::standalone(path, None) {
            Err(error) => error,
            Ok(_) => panic!("stock multi-expression include incorrectly succeeded"),
        };
        assert_eq!(error.code, "LRR-E-INCLUDE-PARSE", "{error:?}");
        assert!(error.origin_attachment_failure.is_none(), "{error}");
        let origin = error
            .macro_origin
            .as_ref()
            .expect("stock include failure lost its invocation");
        assert!(origin.definition.is_none() && origin.parents.is_empty());
        let at = origin.invocation.as_ref().unwrap();
        let original = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            at.source.content.0,
            crate::core::digest::Digest::of_bytes(original.as_bytes()).0
        );
        let opening =
            u32::try_from(original.find("include!(").unwrap() + "include!".len()).unwrap();
        assert_eq!(
            at.range,
            crate::syntax::spans::SourceRange::checked(opening, opening + 1).unwrap()
        );
        assert_eq!(
            &original[at.range.start as usize..at.range.end as usize],
            "("
        );
        let rendered = error.rendered_macro_origin.as_ref().unwrap();
        assert!(rendered.invocation.is_some() && rendered.definition.is_none());
        assert!(error.to_string().contains("include-single-expr-helper.rs"));
        // Partial failure assertions only: exact rustc stderr and its second
        // include's success are not counted as a whole compiletest fixture pass.
    }

    #[test]
    fn rustc_included_function_fault_preserves_validated_include_ancestry() {
        require_current_validation_source();
        use crate::{
            core::digest::Digest,
            db::{memo::Database, poll::QueryPoll},
            engine::run::{RunMachine, RunPoll},
            frontend::callables::CallSelection,
        };
        let original = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-include-items.rs"
        ));
        let helper = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/auxiliary/macro-include-items-item.rs"
        ));
        let helper_bytes = std::fs::read(helper).unwrap();
        let temp = std::env::temp_dir().join(format!(
            "lrr-m016-stock-include-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp.join("stock/auxiliary")).unwrap();
        // Explicit LRR wrapper adaptation: all three stock inputs stay byte-for-byte original.
        std::fs::copy(original, temp.join("stock/macro-include-items.rs")).unwrap();
        for name in ["macro-include-items-item.rs", "macro-include-items-expr.rs"] {
            std::fs::copy(
                original.parent().unwrap().join("auxiliary").join(name),
                temp.join("stock/auxiliary").join(name),
            )
            .unwrap();
        }
        let wrapper = temp.join("main.rs");
        std::fs::write(&wrapper, "include!(\"stock/macro-include-items.rs\");\n").unwrap();
        for nested in [false, true] {
            let path = if nested { &wrapper } else { original };
            for fuel in [1_000_000, 0] {
                let (prepared, view, sources, snapshot) =
                    super::capture_standalone(path, false).unwrap();
                let (compiler, prepared, view, sources) =
                    super::compile_inputs::PreparedCompileInputs::prepare(
                        path, prepared, view, sources,
                    )
                    .unwrap();
                let mut db = Database::default();
                prepared.publish(&mut db).unwrap();
                let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
                    sources,
                    view.outline().root_scope.clone(),
                    crate::source::manifest::Namespace::Value,
                    view.selection().outline.crate_key,
                    1,
                )
                .unwrap()
                .with_declarative_compiler(
                    compiler.environment_identity(),
                    crate::expand::source_declarative::engine_version(),
                    compiler.catalog.clone(),
                )
                .unwrap();
                compiler.discover(&mut discovery, &mut db).unwrap();
                discovery.publish(&mut db).unwrap();
                let captured_snapshot = discovery.diagnostic_snapshot(&mut db, &snapshot).unwrap();
                // Complete discovery also captures the unchanged expression helper in main.
                assert_eq!(captured_snapshot.files().len(), if nested { 4 } else { 3 });
                // Select the original included `foo`, whose unchanged body calls original `bar`.
                // This is a selected-function observation, not a whole run-pass assertion.
                let QueryPoll::Ready(Ok(CallSelection::Source(callable))) =
                    discovery.entry_function(&mut db, "foo")
                else {
                    panic!("stock included foo did not resolve");
                };
                let body = callable.body().unwrap().key();
                let QueryPoll::Ready(Ok(mut run)) =
                    RunMachine::start_with_discovery(&mut db, body.clone(), fuel, discovery)
                else {
                    panic!("stock included function did not enter");
                };
                db.begin_run().unwrap();
                let result = run.poll(&mut db);
                if fuel != 0 {
                    let RunPoll::Completed(report) = result else {
                        panic!("stock included foo/bar did not complete");
                    };
                    assert!(matches!(
                        report.value,
                        crate::engine::run::ValueSummary::Unit
                    ));
                    assert_eq!(run.peak_guest_frames(), 2);
                } else {
                    let RunPoll::Fault(error) = result else {
                        panic!("included stock function failed to exhaust fuel");
                    };
                    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
                    assert!(error.origin_attachment_failure.is_none(), "{error}");
                    let origin = error
                        .generated_origin
                        .as_ref()
                        .expect("included function origin absent");
                    assert_eq!(origin.body.body_key, body);
                    assert!(
                        origin.definition.is_none(),
                        "compiler builtin has no source definition"
                    );
                    let invocation = origin.invocation.as_ref().expect("include anchor absent");
                    let stock = std::fs::read_to_string(original).unwrap();
                    assert_eq!(
                        invocation.source.content.0,
                        Digest::of_bytes(stock.as_bytes()).0
                    );
                    assert_eq!(
                        &stock[invocation.range.start as usize..invocation.range.end as usize],
                        "("
                    );
                    let opening =
                        u32::try_from(stock.find("include!(").unwrap() + "include!".len()).unwrap();
                    assert_eq!(
                        invocation.range,
                        crate::syntax::spans::SourceRange::checked(opening, opening + 1).unwrap()
                    );
                    assert!(!origin.tokens.is_empty());
                    assert!(origin.tokens.iter().all(|token| {
                        token.source.content.0 == Digest::of_bytes(&helper_bytes).0
                            && token
                                .source
                                .path
                                .ends_with("auxiliary/macro-include-items-item.rs")
                            && token.start <= token.end
                            && token.end as usize <= helper_bytes.len()
                    }));
                    assert_eq!(origin.parents.len(), usize::from(nested));
                    if nested {
                        let parent = &origin.parents[0];
                        assert!(parent.invocation_identity.is_some());
                        let at = parent.invocation.as_ref().unwrap();
                        assert_eq!(
                            at.source.content.0,
                            Digest::of_bytes(std::fs::read(&wrapper).unwrap().as_slice()).0
                        );
                        assert_eq!(at.range.start, 8);
                        assert_eq!(at.range.end, 9);
                    }
                    let raw = crate::core::digest::StableEncode::fingerprint(origin.as_ref());
                    let captured = super::render::attach(error.clone(), captured_snapshot.clone());
                    let at = captured.rendered_generated_origin.as_ref().unwrap();
                    assert!(at.invocation.is_some() && at.tokens.iter().all(Option::is_some));
                    assert!(at.parents.iter().all(Option::is_some));
                    assert!(captured.to_string().contains("fn foo() { bar() }"));
                    // Editing only the included helper invalidates its token labels while
                    // exact unchanged invocation and parent files remain attachable.
                    let helper_path = at.tokens[0].as_ref().unwrap().file.clone();
                    let mut changed = captured_snapshot.files().clone();
                    changed.insert(
                        helper_path.clone(),
                        crate::source::snapshot::SourceBlob::new(
                            format!("\n{}", changed[&helper_path].text().unwrap()).into_bytes(),
                        )
                        .unwrap(),
                    );
                    let edited_helper = std::sync::Arc::new(
                        crate::source::snapshot::Snapshot::new(
                            captured_snapshot.project().clone(),
                            changed,
                            captured_snapshot.cfg_by_crate().clone(),
                            captured_snapshot.compile_env().clone(),
                            Some(captured_snapshot.id()),
                        )
                        .unwrap(),
                    );
                    let rejected_helper = super::render::attach(error.clone(), edited_helper);
                    let at = rejected_helper.rendered_generated_origin.as_ref().unwrap();
                    assert!(at.invocation.is_some() && at.parents.iter().all(Option::is_some));
                    assert!(at.tokens.iter().all(Option::is_none));
                    let trace = db.trace.events.len();
                    drop(run);
                    let rendered = super::render::attach(error.clone(), snapshot.clone());
                    let attached = rendered.rendered_generated_origin.as_ref().unwrap();
                    if nested {
                        assert!(
                            attached.invocation.is_none(),
                            "unretained included file acquired a guessed location"
                        );
                        assert!(attached.parents[0].is_some());
                    } else {
                        assert!(attached.invocation.is_some());
                    }
                    assert!(
                        attached.tokens.iter().all(Option::is_none),
                        "renderer loaded included text without a captured lease"
                    );
                    assert_eq!(
                        crate::core::digest::StableEncode::fingerprint(
                            rendered.generated_origin.as_ref().unwrap().as_ref()
                        ),
                        raw
                    );
                    let mut files = snapshot.files().clone();
                    let root = snapshot.project().root_source.clone();
                    files.insert(
                        root.clone(),
                        crate::source::snapshot::SourceBlob::new(
                            format!("\n{}", files[&root].text().unwrap()).into_bytes(),
                        )
                        .unwrap(),
                    );
                    let edited = std::sync::Arc::new(
                        crate::source::snapshot::Snapshot::new(
                            snapshot.project().clone(),
                            files,
                            snapshot.cfg_by_crate().clone(),
                            snapshot.compile_env().clone(),
                            Some(snapshot.id()),
                        )
                        .unwrap(),
                    );
                    let stale = super::render::attach(error, edited);
                    let attached = stale.rendered_generated_origin.as_ref().unwrap();
                    assert!(
                        attached.invocation.is_none()
                            && attached.parents.iter().all(Option::is_none)
                    );
                    assert!(!stale.to_string().contains("\n  -->"));
                    assert_eq!(
                        db.trace.events.len(),
                        trace,
                        "rendering queried captured inputs"
                    );
                }
                db.finish_run().unwrap();
            }
        }
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn rustc_generated_module_function_executes_and_resource_fault_preserves_origins() {
        require_current_validation_source();
        use crate::{
            db::{memo::Database, poll::QueryPoll},
            engine::run::{RunMachine, RunPoll},
            frontend::callables::CallSelection,
        };
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/pub-item-inside-macro.rs"
        ));
        for fuel in [1_000_000, 0] {
            let (prepared, view, sources, snapshot) =
                super::capture_standalone(path, false).unwrap();
            let (compiler, prepared, view, sources) =
                super::compile_inputs::PreparedCompileInputs::prepare(
                    path, prepared, view, sources,
                )
                .unwrap();
            let mut db = Database::default();
            prepared.publish(&mut db).unwrap();
            let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
                sources,
                view.outline().root_scope.clone(),
                crate::source::manifest::Namespace::Value,
                view.selection().outline.crate_key,
                1,
            )
            .unwrap()
            .with_declarative_compiler(
                compiler.environment_identity(),
                crate::expand::source_declarative::engine_version(),
                compiler.catalog.clone(),
            )
            .unwrap();
            compiler.discover(&mut discovery, &mut db).unwrap();
            discovery.publish(&mut db).unwrap();
            let QueryPoll::Ready(Ok(CallSelection::Source(callable))) =
                discovery.entry_function(&mut db, "main")
            else {
                panic!("stock main did not resolve");
            };
            let body = callable.body().unwrap().key();
            let (startup, selected_body) = if fuel == 0 {
                let (sources, graph) = discovery.into_runtime_parts().unwrap();
                let borrowed = sources.iter().collect::<Vec<_>>();
                let resolver =
                    crate::frontend::callables::CallResolver::new(&graph, &borrowed).unwrap();
                let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
                    resolver.source_function(&mut db, body.definition, "bleh::bar")
                else {
                    panic!("sealed generated callee did not resolve");
                };
                callee.revalidate(&mut db).unwrap();
                let generated_body = callee.body().unwrap().key();
                assert_ne!(generated_body.definition, body.definition);
                (
                    RunMachine::start_with_sources(&mut db, generated_body.clone(), fuel, sources),
                    generated_body,
                )
            } else {
                (
                    RunMachine::start_with_discovery(&mut db, body.clone(), fuel, discovery),
                    body.clone(),
                )
            };
            let QueryPoll::Ready(Ok(mut run)) = startup else {
                panic!("stock main did not enter");
            };
            db.begin_run().unwrap();
            let result = run.poll(&mut db);
            if fuel == 1_000_000 {
                let RunPoll::Completed(report) = result else {
                    panic!(
                        "whole stock source failed: {}",
                        match result {
                            RunPoll::Fault(error) => error.to_string(),
                            _ => "unexpected execution outcome".into(),
                        }
                    );
                };
                assert!(matches!(
                    report.value,
                    crate::engine::run::ValueSummary::Unit
                ));
            } else {
                let RunPoll::Fault(error) = result else {
                    panic!("stock generated call did not exhaust opcode fuel");
                };
                assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
                assert_eq!(error.message, "guest opcode fuel exhausted");
                assert!(error.origin_attachment_failure.is_none(), "{error}");
                let origin = error
                    .generated_origin
                    .as_ref()
                    .expect("fault was not attached to the generated callee");
                assert_ne!(origin.body.body_key.definition, body.definition);
                assert_eq!(origin.body.body_key, selected_body);
                assert_eq!(origin.definition.as_ref().unwrap().name, "foo");
                assert!(!origin.tokens.is_empty());
                drop(run);
                let trace = db.trace.events.len();
                // A declared newline-only adaptation lives in a separate
                // immutable snapshot; the licensed upstream copy stays intact.
                let mut files = snapshot.files().clone();
                let root = snapshot.project().root_source.clone();
                let edited = format!("\n{}", files[&root].text().unwrap());
                files.insert(
                    root,
                    crate::source::snapshot::SourceBlob::new(edited.into_bytes()).unwrap(),
                );
                let edited_snapshot = std::sync::Arc::new(
                    crate::source::snapshot::Snapshot::new(
                        snapshot.project().clone(),
                        files,
                        snapshot.cfg_by_crate().clone(),
                        snapshot.compile_env().clone(),
                        Some(snapshot.id()),
                    )
                    .unwrap(),
                );
                let stale = super::render::attach(error.clone(), edited_snapshot);
                let stale_at = stale.rendered_generated_origin.as_ref().unwrap();
                assert!(stale_at.invocation.is_none() && stale_at.definition.is_none());
                assert!(stale_at.tokens.iter().all(Option::is_none));
                assert!(!stale.to_string().contains("\n  -->"));
                let error = super::render::attach(error, snapshot);
                let at = error.rendered_generated_origin.as_ref().unwrap();
                assert!(at.invocation.is_some() && at.definition.is_some());
                assert!(at.tokens.iter().all(Option::is_some));
                let text = error.to_string();
                assert!(text.contains("foo!();"));
                assert!(text.contains("definition of `foo`"));
                assert!(text.contains("pub fn bar() { }"));
                assert_eq!(db.trace.events.len(), trace, "rendering ran a query");
            }
            db.finish_run().unwrap();
        }
    }

    #[test]
    fn rustc_generated_module_fuel_boundaries_preserve_entered_guest_stack() {
        require_current_validation_source();
        use crate::{
            db::{memo::Database, poll::QueryPoll},
            engine::run::{RunMachine, RunPoll},
            frontend::callables::CallSelection,
        };
        // Unchanged upstream program; resource limits are LRR harness controls.
        // Every opcode boundary is exercised, measured from its successful run.
        fn enter(
            fuel: u64,
        ) -> (
            RunMachine,
            Database,
            std::sync::Arc<crate::source::snapshot::Snapshot>,
        ) {
            let path = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/pub-item-inside-macro.rs"
            ));
            let (prepared, view, sources, snapshot) =
                super::capture_standalone(path, false).unwrap();
            let (compiler, prepared, view, sources) =
                super::compile_inputs::PreparedCompileInputs::prepare(
                    path, prepared, view, sources,
                )
                .unwrap();
            let mut db = Database::default();
            prepared.publish(&mut db).unwrap();
            let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
                sources,
                view.outline().root_scope.clone(),
                crate::source::manifest::Namespace::Value,
                view.selection().outline.crate_key,
                1,
            )
            .unwrap()
            .with_declarative_compiler(
                compiler.environment_identity(),
                crate::expand::source_declarative::engine_version(),
                compiler.catalog.clone(),
            )
            .unwrap();
            compiler.discover(&mut discovery, &mut db).unwrap();
            discovery.publish(&mut db).unwrap();
            let QueryPoll::Ready(Ok(CallSelection::Source(main))) =
                discovery.entry_function(&mut db, "main")
            else {
                panic!("stock main selection failed");
            };
            let QueryPoll::Ready(Ok(run)) = RunMachine::start_with_discovery(
                &mut db,
                main.body().unwrap().key(),
                fuel,
                discovery,
            ) else {
                panic!("stock main entry failed");
            };
            db.begin_run().unwrap();
            (run, db, snapshot)
        }
        let (mut baseline, mut db, _) = enter(1_000_000);
        let RunPoll::Completed(report) = baseline.poll(&mut db) else {
            panic!("stock whole-source baseline did not complete");
        };
        assert!(matches!(
            report.value,
            crate::engine::run::ValueSummary::Unit
        ));
        let used = baseline.fuel_snapshot().used;
        assert_eq!(used, report.opcode_counters.fuel_used);
        assert!(
            (1..1024).contains(&used),
            "stock boundary sweep exceeded its harness budget"
        );
        assert_eq!(baseline.peak_guest_frames(), 2);
        db.finish_run().unwrap();
        let mut saw_suspended_caller = false;
        for fuel in 0..used {
            let (mut run, mut db, snapshot) = enter(fuel);
            let main = run.root_body().clone();
            let RunPoll::Fault(error) = run.poll(&mut db) else {
                panic!("fuel boundary {fuel} failed to exhaust");
            };
            assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
            assert_eq!(error.message, "guest opcode fuel exhausted");
            assert!(
                error.origin_attachment_failure.is_none(),
                "boundary {fuel}: {error}"
            );
            let stack = error
                .guest_stack
                .as_ref()
                .expect("entered guest stack missing");
            assert_eq!(stack.omitted_frames, 0);
            assert_eq!(stack.frames.len(), run.diagnostic_frames().count());
            for (frame, (pc, _)) in stack.frames.iter().zip(run.diagnostic_frames()) {
                assert_eq!(frame.pc, crate::core::digest::StableEncode::fingerprint(pc));
                assert_eq!(frame.body.body_key, pc.body);
                assert!(
                    !frame.locations.is_empty(),
                    "boundary {fuel} lacked an exact captured origin"
                );
            }
            assert_eq!(
                stack.frames.last().unwrap().body.body_key,
                main,
                "boundary {fuel} lost its suspended caller"
            );
            if stack.frames.len() == 2 {
                saw_suspended_caller = true;
                assert_ne!(stack.frames[0].body.body_key.definition, main.definition);
                let caller = &stack.frames[1].locations[0];
                let source = snapshot.files()[&snapshot.project().root_source]
                    .text()
                    .unwrap();
                let text = &source[caller.range.start as usize..caller.range.end as usize];
                assert!(
                    text.contains("bleh::bar"),
                    "caller location changed: {text}"
                );
            } else {
                assert_eq!(stack.frames.len(), 1);
            }
            let trace = db.trace.events.len();
            let captured = crate::engine::failure::guest_stack(&run, &mut db).unwrap();
            assert_eq!(
                crate::core::digest::StableEncode::fingerprint(stack.as_ref()),
                crate::core::digest::StableEncode::fingerprint(&captured)
            );
            assert_eq!(db.trace.events.len(), trace, "stack capture ran a query");
            drop(run);
            let mut files = snapshot.files().clone();
            let root = snapshot.project().root_source.clone();
            let edited = format!("\n{}", files[&root].text().unwrap());
            files.insert(
                root,
                crate::source::snapshot::SourceBlob::new(edited.into_bytes()).unwrap(),
            );
            let stale = std::sync::Arc::new(
                crate::source::snapshot::Snapshot::new(
                    snapshot.project().clone(),
                    files,
                    snapshot.cfg_by_crate().clone(),
                    snapshot.compile_env().clone(),
                    Some(snapshot.id()),
                )
                .unwrap(),
            );
            let rejected = super::render::attach(error.clone(), stale);
            assert!(
                rejected
                    .rendered_guest_stack
                    .as_ref()
                    .unwrap()
                    .frames
                    .iter()
                    .flatten()
                    .all(Option::is_none)
            );
            assert!(!rejected.to_string().contains("\n  -->"));
            let frame_count = stack.frames.len();
            let rendered = super::render::attach(error, snapshot);
            let attached = rendered.rendered_guest_stack.as_ref().unwrap();
            assert_eq!(attached.frames.len(), frame_count);
            assert_eq!(attached.omitted_frames, 0);
            assert_eq!(attached.omitted_locations, 0);
            assert!(
                attached
                    .frames
                    .iter()
                    .all(|frame| !frame.is_empty() && frame.iter().all(Option::is_some))
            );
            assert!(rendered.to_string().contains("guest frame 0"));
            if attached.frames.len() == 2 {
                assert!(rendered.to_string().contains("guest frame 1"));
            }
            assert_eq!(db.trace.events.len(), trace, "rendering ran a query");
            db.finish_run().unwrap();
        }
        assert!(
            saw_suspended_caller,
            "all actual call boundaries were missed"
        );
    }

    #[test]
    fn rustc_original_println_facet_retains_std_world_and_refuses_core_selection() {
        use crate::frontend::{
            macro_names::{MacroPath, PathLookup},
            runtime_macro_catalog::RuntimeMacroCatalog,
            source_macro_routes::SourceMacroRoutes,
        };
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic-arg.rs"
        ));
        let (_, view, _, _) = super::capture_standalone(path, false).unwrap();
        let profile = &view.selection().outline.parse.profile;
        let core = crate::facade::unit(profile, "core");
        let standard = crate::facade::unit(profile, "std");
        let catalog = RuntimeMacroCatalog::new(core, profile.edition)
            .unwrap()
            .with_standard_unit(standard)
            .unwrap();
        let name = crate::expand::scopes::MacroName::parse("println", profile.edition).unwrap();
        let routes = SourceMacroRoutes::candidate_graph(&[
            catalog.source_exports(standard).unwrap(),
            catalog.source_exports(core).unwrap(),
        ])
        .unwrap();
        let PathLookup::Defined(definition) = routes
            .lookup(&MacroPath::new(standard, vec![name.clone()]).unwrap())
            .unwrap()
        else {
            panic!("std println definition absent")
        };
        assert_eq!(definition.defining_unit, standard);
        assert_eq!(definition.definition_edition, profile.edition);
        assert_eq!(
            catalog.select(&definition).unwrap(),
            Some(crate::facade::macros::RuntimeMacroKind::Println)
        );
        assert_eq!(
            routes
                .lookup(&MacroPath::new(core, vec![name]).unwrap())
                .unwrap(),
            PathLookup::Absent
        );
        assert_eq!(
            RuntimeMacroCatalog::new(core, profile.edition)
                .unwrap()
                .select(&definition)
                .unwrap(),
            None
        );
    }

    #[test]
    fn rustc_original_argument_panic_cli_seals_cold_std_println_and_unwinds() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/panics/panic-arg.rs"
        ));
        let original = std::fs::read_to_string(path).unwrap();
        let pattern = original
            .lines()
            .find_map(|line| line.strip_prefix("//@ error-pattern:"))
            .unwrap();
        let outcome = super::standalone(path, None).unwrap();
        let super::StandaloneOutcome::Panicked(report) = outcome else {
            panic!("original argument panic did not survive CLI preparation")
        };
        assert_eq!(report.panic.kind, crate::vm::numeric::PanicKind::Explicit);
        assert!(report.panic.description().contains(pattern));
        assert_eq!(report.opcode_counters.started_by_opcode[22], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[19], 0);
        assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
        assert!(report.opcode_counters.panic_scopes_cleaned > 0);
    }

    #[test]
    fn rustc_generated_body_discovery_failure_reports_actual_blocked_producer() {
        use crate::{core::digest::Digest, db::memo::Database};
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
        ));
        // LRR discovery diagnostic scope only. The unchanged upstream source
        // still cannot run; this is not native run-pass or full fixture parity.
        let (prepared, view, sources, snapshot) = super::capture_standalone(path, false).unwrap();
        let (compiler, prepared, view, sources) =
            super::compile_inputs::PreparedCompileInputs::prepare(path, prepared, view, sources)
                .unwrap();
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let scan = view
            .declaration_inventory(
                crate::syntax::declaration_scan::DeclarationScanLimits::default(),
            )
            .unwrap();
        let index = crate::frontend::impl_index::DeclarationMacroIndex::new(scan.clone()).unwrap();
        for producer in scan
            .producers()
            .iter()
            .filter(|producer| producer.path.len() == 1 && producer.path[0].text == "def_fn")
        {
            let lookup = index.lookup(producer.identity).unwrap();
            assert!(
                matches!(
                    lookup,
                    crate::frontend::macro_scopes::HygienicLookup::Selection(
                        crate::frontend::macro_origins::MacroSelection::Defined(_)
                    )
                ),
                "stock def_fn lookup at {:?}: {:?}",
                producer.range,
                lookup
            );
            let provider =
                crate::expand::source_declarative::SourceDeclarative::new(view.share()).unwrap();
            let demand = provider
                .declaration_demand(
                    &index,
                    producer.identity,
                    compiler.environment_identity(),
                    crate::expand::source_declarative::engine_version(),
                )
                .unwrap();
            assert!(
                matches!(
                    demand,
                    crate::expand::source_declarative::SourceDeclarativeDemand::Ready(_)
                ),
                "stock def_fn at {:?} did not produce an exact expansion demand",
                producer.range
            );
        }
        // Diagnostic-only adaptation outside the unchanged licensed source:
        // withhold standard runtime facets to exercise a genuinely incomplete
        // catalog. Ordinary preparation now supplies the std println identity.
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            sources,
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Value,
            view.selection().outline.crate_key,
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            std::sync::Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::new(
                    crate::facade::unit(&view.selection().outline.parse.profile, "core"),
                    view.selection().outline.parse.profile.edition,
                    [crate::expand::compile_input_identity::descriptor("concat")
                        .unwrap()
                        .unwrap()],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        db.trace.detailed = true;
        let error = compiler.discover(&mut discovery, &mut db).unwrap_err();
        assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE", "{error}");
        assert!(
            error
                .message
                .contains("generated macro `println` awaits declaration inputs"),
            "{error}"
        );
        let origin = error.macro_origin.as_ref().unwrap();
        assert!(matches!(
            origin.failure_kind,
            Some(crate::syntax::origin::MacroFailureKind::UnresolvedGeneratedProducer)
        ));
        let span = origin.failure_span.as_ref().unwrap();
        assert_eq!(origin.failure_span_key, Some(span.key()));
        assert_eq!(origin.parents.len(), 1);
        assert_ne!(origin.parents[0].expansion, Digest::default());
        assert!(origin.parents[0].invocation_identity.is_some());
        let project = snapshot.project();
        let blob = &snapshot.files()[&project.root_source];
        assert_eq!(span.source.unit.0, project.selected_crate.0);
        assert_eq!(span.source.content.0, blob.content().0);
        assert_eq!(span.source.path, project.root_source.as_str());
        assert!(
            blob.text()
                .unwrap()
                .get(span.start as usize..span.end as usize)
                .unwrap()
                .contains("println")
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            crate::db::key::QueryKey::BodyStructure(_) | crate::db::key::QueryKey::Thunk(_)
        )));
        let trace = db.trace.events.len();
        let attached = super::render::attach(error, snapshot);
        let rendered = attached.rendered_macro_origin.as_ref().unwrap();
        assert!(rendered.invocation.is_some() && rendered.failure.is_some());
        let text = attached.to_string();
        assert!(text.contains("generated macro producer"));
        assert!(text.contains("println!(\"foo\")"));
        assert!(!text.contains("matcher failure"));
        assert_eq!(db.trace.events.len(), trace, "rendering ran a provider");
    }

    #[test]
    fn development_entry_fails_closed() {
        assert_eq!(super::entry("bootstrap-test"), 2)
    }
}

#[cfg(test)]
mod runtime_alias_acceptance {
    #[test]
    fn adopted_runtime_alias_discovery_preserves_macro_only_import_completion() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/macos-first/m013-module-runtime-alias-probe");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("main.rs"), "#![no_implicit_prelude]\nextern crate core;macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}").unwrap();
        std::fs::write(
            root.join("child.rs"),
            "use crate::core::assert as checked;pub fn made()->i64{checked!(true);1847_i64}",
        )
        .unwrap();
        let path = root.join("main.rs");
        let (prepared, view, sources, _snapshot) = super::capture_standalone(&path, false).unwrap();
        let (compiler, prepared, view, sources) =
            super::compile_inputs::PreparedCompileInputs::prepare(&path, prepared, view, sources)
                .unwrap();
        let mut db = crate::db::memo::Database::default();
        prepared.publish(&mut db).unwrap();
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            sources,
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Value,
            view.selection().outline.crate_key,
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            compiler.environment_identity(),
            crate::expand::source_declarative::engine_version(),
            compiler.catalog.clone(),
        )
        .unwrap();
        let result = compiler.discover(&mut discovery, &mut db);
        assert!(
            result.is_ok(),
            "{:?}\n{}",
            result,
            "macro-only import must complete discovery"
        );
    }
}
