//! M015 ordered attributes and doc transport from unchanged rustc source.
//! The native controls require explicit macro-only preparation and the matched server.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    expand::{
        cache::ExpansionOutputCache,
        identity::{GrammarSlot, MacroIdentity, ProceduralKind},
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
    syntax::{
        parse::ParseKey,
        token_wire::{Delimiter, TokenTree},
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
#[allow(dead_code)]
#[path = "support/macro_service.rs"]
mod service_fixture;

const UPSTREAM: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/attributes-included.rs"
);
const STDERR: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/attributes-included.stderr"
);
const AUXILIARY: &[u8] = include_bytes!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/attributes-included.rs"
);
static EVIDENCE_ID: AtomicU64 = AtomicU64::new(0);
fn component_source() -> String {
    // Preserve the complete original annotation chain, invalid body and main.
    // Only the imported crate spelling changes under the captured dependency.
    let source = UPSTREAM
        .replace(
            "extern crate attributes_included;",
            "extern crate lrr_fixture_macros;",
        )
        .replace(
            "use attributes_included::*;",
            "use lrr_fixture_macros::{bar, foo};",
        );
    format!("{source}\n#[test] fn selected() {{}}\n")
}
// Unwrap only owned invisible transport groups. Retain every visible method
// delimiter, span and token, with the same maximum depth as the wire schema.
fn method_tokens(mut roots: &[TokenTree]) -> Result<&[TokenTree], &'static str> {
    let mut remaining = 256_usize;
    loop {
        let [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] = roots
        else {
            return Ok(roots);
        };
        remaining = remaining
            .checked_sub(1)
            .ok_or("owned invisible method wrapper depth exceeds 256")?;
        roots = children;
    }
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn evidence() -> PathBuf {
    let base = root().join("artifacts/macos-first");
    fs::create_dir_all(&base).unwrap();
    loop {
        let id = EVIDENCE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("evidence counter exhausted");
        let path = base.join(format!(
            "m015-proc-attributes-included-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return fs::canonicalize(path).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive evidence directory: {error}"),
        }
    }
}
fn test_cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}

#[test]
fn unchanged_annotation_chain_and_auxiliary_are_retained() {
    assert!(
        UPSTREAM.contains("//@ edition:2015")
            && UPSTREAM.contains("//@ proc-macro: attributes-included.rs")
            && UPSTREAM.contains("//@ check-pass")
    );
    let chain = &UPSTREAM[UPSTREAM.find("#[bar]").unwrap()..UPSTREAM.find("\nfn main()").unwrap()];
    assert!(component_source().contains(chain));
    assert!(chain.contains("#[bar]\n#[inline]\n/// doc\n#[foo]\n#[inline]\n/// doc\nfn foo()"));
    assert!(chain.contains("let a: i32 = \"foo\";"));
    let auxiliary = std::str::from_utf8(AUXILIARY).unwrap();
    assert!(auxiliary.contains("pub fn bar(") && auxiliary.contains("pub fn foo("));
    assert!(auxiliary.contains("TokenTree::Literal(Literal::i32_unsuffixed(3))"));
    assert!(STDERR.contains("warning: unused variable: `a`"));
    assert_eq!(component_source().matches("#[test]").count(), 1);
}
fn chain_tokens(roots: &[TokenTree], remaining_foo: bool) {
    let tokens = method_tokens(roots).unwrap();
    let expected: &[&str] = if remaining_foo {
        &["inline", "doc", "foo", "inline", "doc"]
    } else {
        &["inline", "doc", "inline", "doc"]
    };
    assert_eq!(tokens.len(), expected.len() * 2 + 4);
    for (i, name) in expected.iter().enumerate() {
        assert!(matches!(
            &tokens[i * 2],
            TokenTree::Punct { scalar: '#', .. }
        ));
        let TokenTree::Group {
            delimiter: Delimiter::Bracket,
            children,
            ..
        } = &tokens[i * 2 + 1]
        else {
            panic!("exact attribute bracket absent")
        };
        assert!(matches!(children.first(),Some(TokenTree::Ident{text,..}) if text==name));
        if *name == "doc" {
            assert_eq!(children.len(), 3);
            assert!(matches!(
                children.get(1),
                Some(TokenTree::Punct { scalar: '=', .. })
            ));
            assert!(matches!(children.get(2), Some(TokenTree::Literal { .. })));
        } else {
            assert_eq!(children.len(), 1)
        }
    }
    let start = expected.len() * 2;
    assert!(matches!(&tokens[start],TokenTree::Ident{text,..} if text=="fn"));
    assert!(matches!(&tokens[start+1],TokenTree::Ident{text,..} if text=="foo"));
    assert!(
        matches!(&tokens[start+2],TokenTree::Group{delimiter:Delimiter::Parenthesis,children,..} if children.is_empty())
    );
    assert!(matches!(
        &tokens[start + 3],
        TokenTree::Group {
            delimiter: Delimiter::Brace,
            ..
        }
    ));
}
fn count_literal(roots: &[TokenTree], literal: &str, depth: usize) -> usize {
    assert!(depth <= 256, "owned token depth bound");
    roots
        .iter()
        .map(|token| match token {
            TokenTree::Literal { token_text, .. } => usize::from(token_text == literal),
            TokenTree::Group { children, .. } => {
                count_literal(children, literal, depth.checked_add(1).unwrap())
            }
            _ => 0,
        })
        .try_fold(0_usize, |n, amount| n.checked_add(amount))
        .expect("owned token count overflow")
}
#[test]
fn actual_bar_then_foo_transform_precedes_all_catalog_filters() {
    let work = evidence();
    let macro_source = work.join("macro");
    fs::create_dir_all(macro_source.join("src")).unwrap();
    fs::write(macro_source.join("src/lib.rs"), AUXILIARY).unwrap();
    fs::write(macro_source.join("Cargo.toml"),"[package]\nname=\"lrr_fixture_macros\"\nversion=\"0.0.0\"\nedition=\"2015\"\n[lib]\nproc-macro=true\n[workspace]\n").unwrap();
    let source = component_source();
    let manifest = service_fixture::prepare_sources(
        &work.join("prepared"),
        &macro_source,
        &[("lib.rs", &source)],
    );
    let plan = lrr::project::tool_manifest::InputPlan::new(
        [work.join("prepared/app"), macro_source]
            .into_iter()
            .map(|p| {
                lrr::project::tool_manifest::InputRoot::open(
                    lrr::project::tool_manifest::InputKind::Source,
                    &p,
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
    let cfg = test_cfg();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let selection = SourceSelection {
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
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("reemission source absent")
    };
    let foo = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "foo")
        .unwrap();
    let original = lrr::syntax::outline::attribute_producer(foo)
        .expect("real original ordinary attribute producer absent");
    let selected_definition = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "selected")
        .unwrap()
        .def_key
        .fingerprint();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let context = DemandContext {
        request: Digest([222; 32]),
        session: Digest([223; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: manifest.inputs.fingerprint(),
        native_generation: None,
        compile_environment: manifest.compile_env[&unit].fingerprint(),
    };
    let mut denied = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, false).unwrap();
    assert_eq!(
        denied.catalog(&mut db, unit).unwrap_err().code,
        "LRR-E-MACRO-PERMISSION"
    );
    assert_eq!(denied.unit_invocations(unit), 0);
    let mut run = MacroRun::begin(&mut db, context.clone(), &manifest, &plan, true).unwrap();
    let catalog = run.catalog(&mut db, unit).unwrap();
    let export_name = "bar";
    let owned_export=catalog.exports().find(|export|matches!(export.identity(),MacroIdentity::Procedural{export,..} if export==export_name)).unwrap();
    assert!(matches!(
        owned_export.identity(),
        MacroIdentity::Procedural {
            kind: ProceduralKind::Attribute,
            ..
        }
    ));
    eprintln!(
        "{}",
        serde_json::json!({"captured_export":owned_export.identity(),"owned_definition":owned_export.definition().identity.hex(),"defining_unit":owned_export.definition().defining_unit.hex(),"consumer_unit":unit.hex(),"source_content":view.selection().outline.parse.content.hex()})
    );
    let compiler = Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    );
    let discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap();
    let mut discovery = discovery
        .with_declarative_procedural(
            context.compile_environment,
            Digest([224; 32]),
            catalog.clone(),
        )
        .unwrap()
        .with_compiler(compiler)
        .unwrap()
        .with_expansion_budget(context.request, run.expansion_budget())
        .unwrap();
    assert_eq!(
        discovery.test_catalog(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
    let mut requests = Vec::new();
    let mut executed = Vec::new();
    let mut sealed = false;
    for index in 0..16 {
        let stage = match discovery.poll() {
            Ok(stage) => stage,
            Err(error) => {
                let witness = serde_json::json!({"primary_poll_error":error,"callbacks":run.unit_invocations(unit),"executed":executed,"body_queries":db.trace.events.iter().filter(|event|matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_))).count()});
                fs::write(
                    work.join("actual-prerequisite.json"),
                    serde_json::to_vec_pretty(&witness).unwrap(),
                )
                .unwrap();
                eprintln!("{witness}");
                panic!("required attribute poll refusal: {error:?}")
            }
        };
        match stage {
            SourceDiscoveryStep::NeedAttributeSelection { selection } => {
                let expected = ["bar", "foo"][requests.len()];
                let request = run
                    .prepare_attribute_selection(&mut db, &selection)
                    .unwrap();
                assert!(
                    matches!(&request.key().macro_identity,MacroIdentity::Procedural{export,kind:ProceduralKind::Attribute,..} if export==expected)
                );
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                assert_eq!(request.key().definition_edition, 2015);
                assert_eq!(request.key().call_edition, 2024);
                if requests.is_empty() {
                    assert_eq!(request.key().invocation, original.item_input);
                    assert_eq!(selection.scope(), &original.scope)
                }
                eprintln!(
                    "{}",
                    serde_json::json!({"expected_order":expected,"actual_request":request.key(),"actual_selected_scope":format!("{:?}",selection.scope()),"input_span_sources":request.request().input.spans(),"original_foo_definition":foo.def_key.fingerprint().hex(),"original_attribute_producer":original.item_input.hex(),"callbacks_before":run.unit_invocations(unit)})
                );
                let prepared_input = serde_json::json!({"actual_request":request.key(),"input_roots":request.request().input.roots(),"input_spans":request.request().input.spans(),"input_contexts":request.request().input.contexts(),"original_source":source,"source_content":view.selection().outline.parse.content.hex(),"original_attribute_producer":original.item_input.hex(),"callbacks_before":run.unit_invocations(unit),"stock_full_fixture_passes":0});
                fs::write(
                    work.join(format!("actual-{expected}-prepared-input.json")),
                    serde_json::to_vec_pretty(&prepared_input).unwrap(),
                )
                .unwrap();
                eprintln!("{prepared_input}");
                assert_eq!(run.unit_invocations(unit), requests.len() as u64);
                discovery.bind_attribute_request(request.clone()).unwrap();
                let SourceDiscoveryStep::NeedExpansion { request: bound } =
                    discovery.poll().unwrap()
                else {
                    panic!("bound attribute demand absent")
                };
                assert_eq!(bound.key(), request.key());
                let phase = PhaseIdentity {
                    frame: Digest([234; 32]),
                    body: BodyKey {
                        definition: Digest([235; 32]),
                        exact_body_bytes: Digest([236; 32]),
                    },
                    index,
                };
                if let Err(error) =
                    run.fulfil_discovery(&mut db, &mut discovery, phase, &request, &mut cache)
                {
                    let witness = serde_json::json!({"primary_fulfilment_error":error,"actual_request":request.key(),"callbacks":run.unit_invocations(unit),"executed":executed,"DB_expansion_record_present_after_failure":db.input(&InputKey::ExpansionRecord(request.key().fingerprint())).is_some(),"body_queries":db.trace.events.iter().filter(|event|matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_))).count()});
                    fs::write(
                        work.join("actual-prerequisite.json"),
                        serde_json::to_vec_pretty(&witness).unwrap(),
                    )
                    .unwrap();
                    eprintln!("{witness}");
                    panic!("actual strict upstream callback refused: {error:?}");
                }
                let adopted = db
                    .input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                    .unwrap();
                let InputValue::ExpansionRecord(record) = adopted.as_ref() else {
                    panic!("actual DB expansion record absent")
                };
                assert_eq!(record.key(), request.key());
                let output = record.tokens().unwrap();
                chain_tokens(output.roots(), expected == "bar");
                assert_eq!(
                    count_literal(output.roots(), "\"foo\"", 0),
                    usize::from(expected == "bar")
                );
                assert_eq!(
                    count_literal(output.roots(), "3", 0),
                    usize::from(expected == "foo")
                );
                executed.push(expected.to_owned());
                requests.push(request.key().clone());
                assert_eq!(run.unit_invocations(unit), requests.len() as u64);
                fs::write(work.join(format!("actual-{expected}-output.json")),serde_json::to_vec_pretty(&serde_json::json!({"record":record.key(),"tokens":output.roots(),"actual_output_span_sources":output.spans(),"actual_output_contexts":output.contexts(),"callbacks_after":run.unit_invocations(unit),"stock_full_fixture_passes":0})).unwrap()).unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                // The actual coordinator demand authorizes this continuation;
                // retain its parent record instead of inventing an invocation.
                let native = request
                    .procedural_request()
                    .expect("generated continuation is not an authorized procedural request");
                let prepared_input = serde_json::json!({"actual_stage":"NeedGeneratedExpansion","actual_request":request.key(),"actual_parent_record":request.parent_record().key(),"input_roots":native.input.roots(),"input_spans":native.input.spans(),"input_contexts":native.input.contexts(),"callbacks_before":run.unit_invocations(unit),"stock_full_fixture_passes":0});
                fs::write(
                    work.join("actual-foo-generated-prepared-input.json"),
                    serde_json::to_vec_pretty(&prepared_input).unwrap(),
                )
                .unwrap();
                eprintln!("{prepared_input}");
                assert_eq!(requests.len(), 1, "only the second callback is generated");
                assert_eq!(request.parent_record().key(), &requests[0]);
                assert_eq!(executed, ["bar"]);
                assert_eq!(run.unit_invocations(unit), 1);
                assert!(
                    matches!(&request.key().macro_identity,MacroIdentity::Procedural{export,kind:ProceduralKind::Attribute,..} if export=="foo")
                );
                assert_eq!(request.key().grammar, GrammarSlot::Items);
                assert_eq!(request.key().definition_edition, 2015);
                assert_eq!(request.key().call_edition, 2024);
                let phase = PhaseIdentity {
                    frame: Digest([234; 32]),
                    body: BodyKey {
                        definition: Digest([235; 32]),
                        exact_body_bytes: Digest([236; 32]),
                    },
                    index,
                };
                if let Err(error) = run.fulfil_generated_discovery(
                    &mut db,
                    &mut discovery,
                    phase,
                    &request,
                    &mut cache,
                ) {
                    let witness = serde_json::json!({"actual_stage":"NeedGeneratedExpansion","primary_fulfilment_error":error,"actual_request":request.key(),"actual_parent_record":request.parent_record().key(),"callbacks":run.unit_invocations(unit),"executed":executed,"DB_expansion_record_present_after_failure":db.input(&InputKey::ExpansionRecord(request.key().fingerprint())).is_some(),"body_queries":db.trace.events.iter().filter(|event|matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_))).count()});
                    fs::write(
                        work.join("actual-prerequisite.json"),
                        serde_json::to_vec_pretty(&witness).unwrap(),
                    )
                    .unwrap();
                    eprintln!("{witness}");
                    panic!("actual generated foo callback refused: {error:?}");
                }
                let adopted = db
                    .input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                    .unwrap();
                let InputValue::ExpansionRecord(record) = adopted.as_ref() else {
                    panic!("actual generatedfoo DB expansion record absent")
                };
                assert_eq!(record.key(), request.key());
                let output = record.tokens().unwrap();
                chain_tokens(output.roots(), false);
                assert_eq!(count_literal(output.roots(), "\"foo\"", 0), 0);
                assert_eq!(count_literal(output.roots(), "3", 0), 1);
                executed.push("foo".to_owned());
                requests.push(request.key().clone());
                assert_eq!(run.unit_invocations(unit), 2);
                fs::write(work.join("actual-foo-output.json"),serde_json::to_vec_pretty(&serde_json::json!({"actual_stage":"NeedGeneratedExpansion","record":record.key(),"tokens":output.roots(),"actual_output_span_sources":output.spans(),"actual_output_contexts":output.contexts(),"callbacks_after":run.unit_invocations(unit),"stock_full_fixture_passes":0})).unwrap()).unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                let witness = serde_json::json!({"primary_incomplete":diagnostic,"blocked":format!("{blocked:?}"),"callbacks":run.unit_invocations(unit),"executed":executed});
                fs::write(
                    work.join("actual-prerequisite.json"),
                    serde_json::to_vec_pretty(&witness).unwrap(),
                )
                .unwrap();
                eprintln!("{witness}");
                panic!("actual required attribute incomplete: {diagnostic:?}");
            }
            _ => panic!("unexpected effectful discovery stage"),
        }
    }
    assert!(sealed);
    assert_eq!(executed, ["bar", "foo"]);
    assert_eq!(requests.len(), 2);
    assert_eq!(run.unit_invocations(unit), 2);
    let mut records: Vec<_> = discovery.sealed_expansions().unwrap().cloned().collect();
    records.extend(discovery.sealed_declaration_expansions().unwrap());
    assert_eq!(records.len(), 2);
    for request in &requests {
        assert_eq!(
            records
                .iter()
                .filter(|record| record.key() == request)
                .count(),
            1
        )
    }
    run.publish_discovery(&mut db, &mut discovery).unwrap();
    for _ in 0..3 {
        let tests = discovery.test_catalog(&mut db).unwrap();
        tests.require_current(&mut db).unwrap();
        assert!(tests.seal().is_some());
        assert_eq!(tests.cases().len(), 1);
        let case = &tests.cases()[0];
        assert_eq!(case.name, "selected");
        assert_eq!(case.definition, selected_definition);
        assert!(case.attribute_error.is_none());
        assert!(!case.ignored && !case.should_panic && !case.unsupported_signature);
        assert_eq!(tests.select(None, false).len(), 1);
        assert_eq!(tests.select(Some("selected"), true).len(), 1);
        assert_eq!(tests.select(Some("select"), false).len(), 1);
        assert!(tests.select(Some("absent"), true).is_empty());
        assert!(tests.select(Some("absent"), false).is_empty());
    }
    assert_eq!(run.unit_invocations(unit), 2);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
    let proof = serde_json::json!({"source":source,"source_content":view.selection().outline.parse.content.hex(),"root_unit":unit.hex(),"actual_requests":requests,"actual_callback_order":executed,"callbacks":2,"records":2,"sealed_test":"selected","selected_test_definition":selected_definition.hex(),"known_exact":1,"known_substring":1,"absent_exact":0,"absent_substring":0,"body_signature_thunk_queries":0,"macro_reexecution":0,"original_invalid_body_preserved":true,"actual_output_literal_replacement":"foo string→3","stock_full_fixture_passes":0});
    fs::write(
        work.join("accepted.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    eprintln!("{proof}");
}
