//! M015 component input-order regression using the unchanged official source and auxiliary.
//! This does not claim full compiletest or guest-tier acceptance.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    service::{
        expansion::{DemandContext, PhaseIdentity},
        run::MacroRun,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;
#[test]
fn rustc_original_test_attribute_counts_use_consumed_builtin_prefixes() {
    original_test_attribute_case(None, false, false);
}
#[test]
fn rustc_attribute_count_component_preserves_2015_definition_and_2024_call_sites() {
    original_test_attribute_case(Some("meow2"), false, false);
}
#[test]
fn rustc_generated_test_attribute_counts_use_consumed_builtin_prefixes() {
    original_test_attribute_case(None, true, false);
}
#[test]
fn rustc_cold_test_attribute_counts_use_consumed_builtin_prefixes() {
    original_test_attribute_case(None, false, true);
}
#[test]
fn rustc_generated_cold_test_attribute_counts_use_consumed_builtin_prefixes() {
    original_test_attribute_case(None, true, true);
}
#[test]
fn rustc_auxiliary_cold_attribute_uses_actual_block_import() {
    attribute_input_case(
        None,
        false,
        true,
        Some(
            "use test_count_attributes::assert_no_attributes as keep; #[test] #[keep] fn inner() {}",
        ),
        1,
    );
}
#[test]
fn rustc_auxiliary_generated_cold_attribute_uses_actual_block_import() {
    attribute_input_case(
        None,
        true,
        true,
        Some(
            "use test_count_attributes::assert_no_attributes as keep; #[test] #[keep] fn inner() {}",
        ),
        1,
    );
}
#[test]
fn rustc_auxiliary_cold_module_alias_glob_and_cfg_imports_select_actual_export() {
    for generated in [false, true] {
        attribute_input_case(
            None,
            generated,
            true,
            Some(
                "use test_count_attributes::{self as host}; use host::*; #[cfg(any())] use absent::attribute as assert_no_attributes; #[test] #[assert_no_attributes] fn inner() {}",
            ),
            1,
        );
    }
}

#[test]
fn rustc_original_attribute_generated_fault_keeps_adopted_source_origins() {
    attribute_input_slot_case_with_origins(None, false, false, None, 5, None, Some(0));
}
#[test]
fn rustc_wrapped_attribute_generated_fault_retains_explicit_parent_invocation() {
    // The wrapper is an LRR adaptation outside the unchanged upstream directory.
    attribute_input_slot_case_with_origins(None, true, false, None, 5, None, Some(1));
}

#[test]
fn rustc_nested_attribute_generated_fault_retains_validated_expansion_chain() {
    // Two actual macro_rules layers surround the unchanged original fixture.
    attribute_input_slot_case_with_origins(None, true, false, None, 5, None, Some(2));
}

static WORK_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn allocate_work(base: &std::path::Path) -> PathBuf {
    for _ in 0..1024 {
        let sequence = WORK_SEQUENCE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("evidence sequence exhausted");
        let path = base.join(format!(
            "m015-rustc-test-count-{}-{}-{sequence}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return path,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot allocate evidence directory: {e}"),
        }
    }
    panic!("evidence directory collision limit");
}
#[test]
fn concurrent_evidence_allocations_are_exclusive() {
    let base = std::env::temp_dir();
    let handles: Vec<_> = (0..32)
        .map(|_| {
            let base = base.clone();
            std::thread::spawn(move || allocate_work(&base))
        })
        .collect();
    let paths: std::collections::BTreeSet<_> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(paths.len(), 32);
    for path in paths {
        assert_eq!(
            fs::create_dir(&path).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        fs::remove_dir(path).unwrap();
    }
}
fn original_test_attribute_case(selected_name: Option<&str>, generated: bool, cold: bool) {
    attribute_input_case(
        selected_name,
        generated,
        cold,
        None,
        if selected_name.is_some() { 1 } else { 5 },
    );
}
fn attribute_input_case(
    selected_name: Option<&str>,
    generated: bool,
    cold: bool,
    explicit_source: Option<&str>,
    expected_invocations: u64,
) {
    attribute_input_slot_case(
        selected_name,
        generated,
        cold,
        explicit_source,
        expected_invocations,
        None,
    );
}
fn attribute_input_slot_case(
    selected_name: Option<&str>,
    generated: bool,
    cold: bool,
    explicit_source: Option<&str>,
    expected_invocations: u64,
    slot: Option<lrr::expand::identity::GrammarSlot>,
) {
    attribute_input_slot_case_with_origins(
        selected_name,
        generated,
        cold,
        explicit_source,
        expected_invocations,
        slot,
        None,
    );
}
fn attribute_input_slot_case_with_origins(
    selected_name: Option<&str>,
    generated: bool,
    cold: bool,
    explicit_source: Option<&str>,
    expected_invocations: u64,
    slot: Option<lrr::expand::identity::GrammarSlot>,
    origin_depth: Option<usize>,
) {
    service_fixture::require_current_validation_source();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = allocate_work(&root.join("artifacts/macos-first"));
    fs::create_dir_all(work.join("macro/src")).unwrap();
    let work = fs::canonicalize(work).unwrap();
    let upstream_auxiliary = include_bytes!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-count-attributes.rs"
    );
    fs::write(work.join("macro/src/lib.rs"), upstream_auxiliary).unwrap();
    fs::write(work.join("macro/Cargo.toml"),"[package]\nname=\"test_count_attributes\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/test-count-attributes.rs"
    );
    let mut selected_cfg = CfgSet::guest_default();
    selected_cfg.atoms.insert("test".into());
    let parsed = lrr::syntax::parse::ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([207; 32]), vec![], &selected_cfg)
        .unwrap();
    // Select the exact owning item from unchanged upstream source, outside the licensed directory.
    let supplied = explicit_source.unwrap_or(original);
    let cold_source = format!("fn outer() {{\n{supplied}\n}}");
    let emitted = if cold { cold_source.as_str() } else { original };
    let generated_source = match slot {
        Some(lrr::expand::identity::GrammarSlot::Expression) => format!(
            "macro_rules! emit {{ () => {{{{ {supplied} 0_i32 }}}}; }} fn outer(){{let _=emit!();}}"
        ),
        Some(lrr::expand::identity::GrammarSlot::Statements) => {
            format!("macro_rules! emit {{ () => {{ {supplied} }}; }} fn outer(){{emit!();}}")
        }
        None if origin_depth == Some(2) => format!(
            "macro_rules! inner {{ () => {{\n{emitted}\n}}; }} macro_rules! outer {{ () => {{inner!();}}; }} outer!();"
        ),
        None => format!("macro_rules! emit {{ () => {{\n{emitted}\n}}; }} emit!();"),
        Some(_) => panic!("unsupported test adaptation slot"),
    };
    let source = if generated {
        generated_source.as_str()
    } else if cold {
        cold_source.as_str()
    } else if let Some(name) = selected_name {
        let range = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap()
            .declaration_span;
        &original[range.start as usize..range.end as usize]
    } else {
        supplied
    };
    // The helper must prepare only this macro-host dependency with its original crate alias.
    let manifest = service_fixture::prepare_test_count_attributes(
        &work.join("prepared"),
        &work.join("macro"),
        source,
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), work.join("macro")]
            .into_iter()
            .map(|path| {
                lrr::project::tool_manifest::InputRoot::open(
                    lrr::project::tool_manifest::InputKind::Source,
                    &path,
                    Default::default(),
                    Default::default(),
                    true,
                    Default::default(),
                )
                .unwrap()
            })
            .collect(),
        Default::default(),
    )
    .unwrap();
    let unit = *manifest.graph.roots.first().unwrap();
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let selected = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let environment =
        lrr::expand::builtins::FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit])
            .unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap()
    .with_compile_environment(Arc::new(environment))
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selected).unwrap() else {
        panic!("original rustc source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([201; 32]),
        session: Digest([202; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_compiler(context.compile_environment, Digest([203; 32]), compiler)
    .unwrap()
    .with_procedural_catalog(catalog)
    .unwrap()
    .with_expansion_budget(context.request, run.expansion_budget())
    .unwrap();
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut captured_origins = std::collections::BTreeMap::new();
    let mut generated_call_sites = std::collections::BTreeMap::new();
    let mut sealed = false;
    for index in 0..32 {
        let phase = PhaseIdentity {
            frame: Digest([204; 32]),
            body: BodyKey {
                definition: Digest([205; 32]),
                exact_body_bytes: Digest([206; 32]),
            },
            index,
        };
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                let request = run
                    .prepare_attribute_selection(&mut db, &selection)
                    .unwrap();
                let invocation = request.request();
                let definition = invocation
                    .input
                    .spans()
                    .iter()
                    .find(|span| span.key() == invocation.definition_site)
                    .unwrap();
                let call = invocation
                    .input
                    .spans()
                    .iter()
                    .find(|span| span.key() == invocation.call_site)
                    .unwrap();
                assert_eq!(
                    definition.edition,
                    lrr_expand_adapter::token_wire::Edition::E2015
                );
                assert_eq!(call.edition, lrr_expand_adapter::token_wire::Edition::E2024);
                assert!(
                    captured_origins
                        .insert(
                            request.key().invocation,
                            (
                                call.clone(),
                                definition.source.clone(),
                                invocation.export.clone()
                            )
                        )
                        .is_none()
                );
                discovery.bind_attribute_request(request.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("original attribute request absent")
                };
                assert_eq!(request.key(), bound.key());
                // The unchanged auxiliary itself asserts the remaining attribute count.
                run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache)
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                if origin_depth == Some(2) && request.procedural_request().is_none() {
                    let parent = Arc::new(request.parent_record().clone());
                    let admitted = lrr::expand::generated_items::GeneratedItems::admit(
                        parent.key(),
                        parent.clone(),
                        1,
                    )
                    .unwrap();
                    let call = admitted
                        .calls()
                        .iter()
                        .find(|call| call.key.fingerprint() == request.key().invocation)
                        .unwrap();
                    let site = call
                        .input
                        .spans()
                        .iter()
                        .find(|span| span.key() == call.site)
                        .unwrap();
                    assert!(
                        generated_call_sites
                            .insert(request.key().fingerprint(), site.clone())
                            .is_none()
                    );
                }
                if let Some(invocation) = request.procedural_request() {
                    let definition = invocation
                        .input
                        .spans()
                        .iter()
                        .find(|span| span.key() == invocation.definition_site)
                        .unwrap();
                    let call = invocation
                        .input
                        .spans()
                        .iter()
                        .find(|span| span.key() == invocation.call_site)
                        .unwrap();
                    assert_eq!(
                        definition.edition,
                        lrr_expand_adapter::token_wire::Edition::E2015
                    );
                    assert_eq!(call.edition, lrr_expand_adapter::token_wire::Edition::E2024);
                    assert!(
                        captured_origins
                            .insert(
                                request.key().invocation,
                                (
                                    call.clone(),
                                    definition.source.clone(),
                                    invocation.export.clone()
                                )
                            )
                            .is_none()
                    );
                    run.fulfil_generated_discovery(
                        &mut db,
                        &mut discovery,
                        phase,
                        &request,
                        &mut cache,
                    )
                    .unwrap();
                } else {
                    discovery
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap();
                }
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("original test attribute order unresolved: {diagnostic:?}, {blocked:?}"),
            _ => panic!("unexpected original test attribute demand"),
        }
    }
    assert!(sealed, "original input-order traversal exceeded its bound");
    assert_eq!(run.unit_invocations(unit), expected_invocations);
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    let expected_names = if cold {
        vec![]
    } else if let Some(name) = selected_name {
        vec![name]
    } else {
        vec!["meow1", "meow2", "meow3", "meow4", "meow5"]
    };
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        expected_names
    );
    if selected_name.is_none() && !cold {
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|case| case.should_panic)
                .collect::<Vec<_>>(),
            vec![false, false, true, true, true]
        );
    }
    for _ in 0..3 {
        catalog.require_current(&mut db).unwrap();
        assert_eq!(
            discovery.test_catalog(&mut db).unwrap().cases().len(),
            expected_names.len()
        );
    }
    assert_eq!(run.unit_invocations(unit), expected_invocations);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    if let Some(depth) = origin_depth {
        use lrr::{
            db::poll::QueryPoll,
            engine::run::{RunMachine, RunPoll},
        };
        assert!(!cold && selected_name.is_none() && explicit_source.is_none());
        let case = catalog
            .cases()
            .iter()
            .find(|case| case.name == "meow1")
            .unwrap();
        assert!(case.attribute_error.is_none() && !case.should_panic);
        let expected_original_meta = (!generated).then(|| {
            let owner = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == case.name)
                .unwrap();
            let facts = parsed
                .attribute_inputs_at(owner.declaration_span, &selected_cfg)
                .unwrap();
            let (call, _, _) = captured_origins
                .values()
                .find(|(call, _, _)| {
                    owner.declaration_span.contains(
                        lrr::syntax::spans::SourceRange::checked(call.start, call.end).unwrap(),
                    )
                })
                .unwrap();
            facts
                .attributes
                .iter()
                .find(|attribute| {
                    attribute.range.contains(
                        lrr::syntax::spans::SourceRange::checked(call.start, call.end).unwrap(),
                    )
                })
                .unwrap()
                .range
        });
        let body = case.body.as_ref().unwrap().clone();
        // MacroRun already authorized and published this exact sealed body.
        // Enter its immutable published inputs directly: this zero-fuel control
        // never resolves a call, and must not attempt a second raw publication.
        db.begin_run().unwrap();
        let mut machine = match RunMachine::start(&mut db, body.clone(), 0) {
            QueryPoll::Ready(Ok(machine)) => machine,
            QueryPoll::Ready(Err(error)) => {
                panic!("actual published attribute-generated body refused entry: {error}")
            }
            QueryPoll::NeedInput(_) => panic!("sealed attribute body input absent"),
        };
        drop(discovery);
        let RunPoll::Fault(error) = machine.poll(&mut db) else {
            panic!("attribute-generated body did not exhaust zero opcode fuel");
        };
        db.finish_run().unwrap();
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(error.message, "guest opcode fuel exhausted");
        assert!(error.origin_attachment_failure.is_none(), "{error}");
        let origin = error
            .generated_origin
            .as_ref()
            .expect("actual generated attribute body fault has no owned origin");
        assert_eq!(origin.body.body_key, body);
        assert_eq!(origin.parents.len(), depth);
        if generated {
            assert_eq!(case.expansion_ancestry.len(), depth + 1);
            for (parent, expected) in origin.parents.iter().zip(&case.expansion_ancestry[..depth]) {
                assert_eq!(parent.expansion, *expected);
                assert!(parent.invocation_identity.is_some());
                let at = parent
                    .invocation
                    .as_ref()
                    .expect("actual parent invocation site absent");
                assert_eq!(at.source.unit.0, unit.0);
                assert_eq!(at.source.path, view.selection().path.as_str());
                assert_eq!(
                    at.source.content.0,
                    view.selection().outline.parse.content.0
                );
            }
            let at = origin.parents[0].invocation.as_ref().unwrap();
            assert_eq!(
                &source[at.range.start as usize..at.range.end as usize],
                if depth == 2 { "outer!();" } else { "emit!();" }
            );
            if depth == 2 {
                let parent = &origin.parents[1];
                let captured = &generated_call_sites[&parent.expansion];
                let at = parent.invocation.as_ref().unwrap();
                assert_eq!(
                    (at.range.start, at.range.end),
                    (captured.start, captured.end)
                );
                assert_eq!(at.source, captured.source);
            }
        }
        let (call, producer, export) = &captured_origins[&origin.invocation_identity];
        let invocation = origin
            .invocation
            .as_ref()
            .expect("selected attribute invocation was dropped");
        assert_eq!(invocation.source.unit.0, call.source.unit.0);
        assert_eq!(invocation.source.content.0, call.source.content.0);
        assert_eq!(invocation.source.path, call.source.path);
        let anchor = lrr::syntax::spans::SourceRange::checked(call.start, call.end).unwrap();
        assert_eq!(invocation.range, expected_original_meta.unwrap_or(anchor));
        assert!(invocation.range.contains(anchor));
        assert_eq!(invocation.source.unit.0, unit.0);
        assert!(
            source[invocation.range.start as usize..invocation.range.end as usize]
                .contains("test_count_attributes")
        );
        let definition = origin
            .definition
            .as_ref()
            .expect("exact prepared host export definition was dropped");
        assert_eq!(definition.name, *export);
        assert_eq!(definition.location.source.unit.0, producer.unit.0);
        assert_eq!(definition.location.source.content.0, producer.content.0);
        assert_eq!(definition.location.source.path, producer.path);
        assert_ne!(
            definition.location.source.unit.0, unit.0,
            "native producer and live consumer worlds were conflated"
        );
        let auxiliary = std::str::from_utf8(upstream_auxiliary).unwrap();
        assert_eq!(
            &auxiliary
                [definition.location.range.start as usize..definition.location.range.end as usize],
            format!("fn {export}")
        );
        let stack = error
            .guest_stack
            .as_ref()
            .expect("entered generated stack absent");
        assert_eq!(stack.frames.len(), 1);
        assert_eq!(stack.frames[0].body.body_key, body);
        assert!(!stack.frames[0].locations.is_empty());
        assert_eq!(
            run.unit_invocations(unit),
            expected_invocations,
            "terminal attachment reexecuted the helper"
        );
        let retained = serde_json::to_value(&error).unwrap();
        drop(machine);
        drop(run);
        drop(catalog);
        assert_eq!(
            retained,
            serde_json::to_value(&error).unwrap(),
            "owned origin changed after runtime/discovery leases dropped"
        );
        // Whole original source and auxiliary remain unchanged. This guest fuel
        // and provenance control is not a full rustc check-pass/runtime result.
    }
}

#[test]
fn rustc_auxiliary_expression_output_attribute_uses_actual_block_import() {
    attribute_input_slot_case(
        None,
        true,
        true,
        Some(
            "use test_count_attributes::assert_no_attributes as keep; #[test] #[keep] fn inner() {}",
        ),
        1,
        Some(lrr::expand::identity::GrammarSlot::Expression),
    );
}
#[test]
fn rustc_auxiliary_statement_output_attribute_uses_actual_block_import() {
    attribute_input_slot_case(
        None,
        true,
        true,
        Some(
            "use test_count_attributes::assert_no_attributes as keep; #[test] #[keep] fn inner() {}",
        ),
        1,
        Some(lrr::expand::identity::GrammarSlot::Statements),
    );
}
