//! Registered pre-entry API controls. No interpreter/main execution occurs here.
use super::preentry_loader::Packet;
use crate::core::digest::Digest;
use std::path::PathBuf;
fn observe(path: &std::path::Path, edition: u16) -> serde_json::Value {
    serde_json::from_slice(
        &Packet::observe(path, edition, Digest([7; 32]))
            .render()
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn unchanged_original_main_metadata_uses_actual_capture_and_index() {
    let root = std::env::var_os("LRR_PREENTRY_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout")
        });
    for case in [
        "tests/ui/codegen/assign-expr-unit-type.rs",
        "tests/ui/numbers-arithmetic/floatlits.rs",
        "tests/ui/numbers-arithmetic/i8-incr.rs",
        "tests/ui/cast/u8-to-char-cast-9918.rs",
        "tests/ui/cast/cast-does-fallback.rs",
        "tests/ui/numeric/type-limit-comparisons-6130.rs",
    ] {
        let path = root.join(case);
        let bytes = std::fs::read(&path).unwrap();
        let packet = observe(&path, 2015);
        assert_eq!(packet["status"], "ready", "{case}: {packet}");
        assert_eq!(packet["snapshot"]["owning_edition"], 2015);
        assert_eq!(packet["entry_body"], packet["entry_selection"]["body"]);
        assert_eq!(packet["entry_body"], packet["structural_index"]["body"]);
        assert!(
            packet["snapshot"]["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["content"] == serde_json::to_value(Digest::of_bytes(&bytes)).unwrap())
        );
        assert_eq!(packet["guest_executions"], 0);
        assert_eq!(packet["vm_dispatches"], 0);
        assert_eq!(packet["executable_thunks"], 0);
        assert_eq!(packet["compiler_tool_invocations"], 0);
        assert_eq!(packet["procedural_macro_executions"], 0);
        assert!(
            !packet["discovery"]["current_input_stamps"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(packet["structural_index"]["nodes"].as_u64().unwrap() > 0);
    }
}
fn temporary_source(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lrr-preentry-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("root.rs");
    std::fs::write(&path, text).unwrap();
    path
}
#[test]
fn structural_metadata_never_runs_panic_arithmetic_or_managed_output() {
    for (name, text) in [
        ("panic", "fn main(){panic!(\"metadata must not run\");}"),
        ("division", "fn main(){let x=1/0;assert_eq!(x,7);}"),
        (
            "output",
            "fn main(){println!(\"must not be guest output\");}",
        ),
        ("cold", "fn cold(){panic!(\"cold\");} fn main(){}"),
    ] {
        let path = temporary_source(name, text);
        let packet = observe(&path, 2015);
        assert_eq!(packet["guest_executions"], 0);
        assert_eq!(packet["vm_dispatches"], 0);
        assert_eq!(packet["compiler_tool_invocations"], 0);
        assert_eq!(packet["status"], "ready", "{name}: {packet}");
        assert_ne!(packet["schema"], "lrr/v5/standalone-runtime-capture/v1");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
#[test]
fn missing_ambiguous_or_generated_main_is_typed_frontier() {
    for (name, text) in [
        ("missing", "fn other(){}"),
        ("duplicate", "fn main(){} fn main(){}"),
        (
            "generated",
            "macro_rules! make {()=>{fn main(){}}} make!();",
        ),
    ] {
        let path = temporary_source(name, text);
        let packet = observe(&path, 2015);
        assert_eq!(packet["status"], "frontier", "{packet}");
        assert!(packet["frontier"]["code"].as_str().is_some());
        assert!(packet["entry_body"].is_null());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
#[test]
fn metadata_context_flags_are_strict_and_separate_from_run() {
    use clap::Parser;
    let context = "07".repeat(32);
    assert!(
        super::args::Args::try_parse_from([
            "lrr",
            "source-world",
            "root.rs",
            "--context",
            &context
        ])
        .is_ok()
    );
    assert!(
        super::args::Args::try_parse_from(["lrr", "run", "root.rs", "--context", &context])
            .is_err()
    );
    for value in ["A".repeat(64), "f".repeat(63), "g".repeat(64)] {
        assert!(
            super::args::Args::try_parse_from([
                "lrr",
                "source-world",
                "root.rs",
                "--context",
                &value
            ])
            .is_err()
        );
    }
}

// The first source-only helper lacked the actual std import world. This is
// the same unchanged upstream header witness, through real facade capture.
#[test]
fn unchanged_projection_impl_header_queries_retain_every_where_predicate() {
    use crate::{
        core::profile::ProfileIdentity,
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
            value::QueryValue,
        },
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        source::manifest::Namespace,
        traits::header_syntax::ImplHeaderSyntaxKey,
    };
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("actual projection header: {error}"),
            QueryPoll::NeedInput(request) => panic!("actual projection header demand: {request:?}"),
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/upstream/rustc-checkout/tests/ui/associated-types/associated-types-conditional-dispatch.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, seal, root) = prepared_impl_header_world(&path, edition);
        let outcome = ready(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(seal.clone()),
        ));
        let QueryValue::PreparedScopeImplHeaders(headers) = outcome.value.as_ref() else {
            panic!()
        };
        let own: Vec<_> = headers
            .inventory_root()
            .headers()
            .iter()
            .filter(|header| header.scope.crate_key == root.crate_key)
            .collect();
        assert_eq!(own.len(), 4);
        let mut predicates = 0;
        for raw in own {
            let outcome = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::ImplHeaderSyntax(Box::new(ImplHeaderSyntaxKey {
                    inventory: seal.clone(),
                    implementation: raw.identity,
                })),
            ));
            let QueryValue::ImplHeaderSyntax(plan) = outcome.value.as_ref() else {
                panic!()
            };
            predicates += plan.predicates().len();
            assert_eq!(plan.source().profile.edition, edition);
        }
        assert_eq!(predicates, 3);
        match db.fetch(
            &SyntaxProvider,
            QueryKey::ImplHeaderSyntax(Box::new(ImplHeaderSyntaxKey {
                inventory: seal.clone(),
                implementation: Digest::of_bytes(b"absent-impl"),
            })),
        ) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-IMPL-HEADER"),
            other => panic!("absent source header accepted: {other:?}"),
        }
    }
}

fn prepared_impl_header_world(
    path: &std::path::Path,
    edition: u16,
) -> (
    crate::db::memo::Database,
    crate::db::seals::SealKey,
    crate::syntax::outline::ScopeKey,
) {
    use crate::{
        core::profile::ProfileIdentity,
        db::memo::Database,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        source::manifest::Namespace,
    };
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(
        &path,
        false,
        edition,
        &super::crate_graph::ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )
    .unwrap();
    let (compiler, inputs, source, sources) =
        super::compile_inputs::PreparedCompileInputs::prepare(path, inputs, source, sources)
            .unwrap();
    let root = source.outline().root_scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery =
        SourceDiscovery::new(sources, root.clone(), Namespace::Value, root.crate_key, 1)
            .unwrap()
            .with_declarative_compiler(
                compiler.environment_identity(),
                crate::expand::source_declarative::engine_version(),
                compiler.catalog.clone(),
            )
            .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut db).unwrap();
                break;
            }
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!("{diagnostic}"),
            _ => panic!("actual projection header requires an unprepared producer"),
        }
    }
    (db, discovery.key().clone(), root)
}

#[test]
fn unchanged_conditional_impl_heads_use_common_types_across_origins_and_editions() {
    use crate::{
        db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::{impl_head_types::ImplHeadTypeResult, names::NameLookup},
        ir::op::PrimitiveType,
        source::manifest::Namespace,
        syntax::imports::ImportSegment,
        traits::{
            header_syntax::ImplHeaderSyntaxKey,
            term::{GenericArg, ParamKey, TyTerm},
        },
    };
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("actual impl head: {error}"),
            QueryPoll::NeedInput(input) => panic!("actual impl head needs input: {input:?}"),
        }
    }
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/methods/method-two-trait-defer-resolution-2.rs"
    );
    let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout/tests/ui/methods/method-two-trait-defer-resolution-2.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let emit = "macro_rules! emit { ($($t:tt)*) => { $($t)* }; }";
        let forms = [
            ORIGINAL.to_owned(),
            format!("{emit} emit!{{{ORIGINAL}}}"),
            format!(
                "{emit} macro_rules! wrap {{ ($($t:tt)*) => {{ emit!{{$($t)*}} }}; }} wrap!{{{ORIGINAL}}}"
            ),
        ];
        for (origin, text) in forms.iter().enumerate() {
            let path = if origin == 0 {
                original_path.clone()
            } else {
                temporary_source(&format!("impl-head-{edition}-{origin}"), text)
            };
            let (mut db, seal, root) = prepared_impl_header_world(&path, edition);
            let outcome = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedScopeImplHeaders(seal.clone()),
            ));
            let QueryValue::PreparedScopeImplHeaders(headers) = outcome.value.as_ref() else {
                panic!()
            };
            let environment = headers.inventory_root().environment().unwrap();
            let named = |name: &str| match environment
                .names()
                .lookup_source_path(
                    Namespace::Type,
                    &[ImportSegment::Name(name.into())],
                    false,
                    &root,
                    edition,
                )
                .unwrap()
            {
                NameLookup::Defined(found) => found.identity,
                other => panic!("actual source Type {name}: {other:?}"),
            };
            let (foo, copy, vec, boxed) =
                (named("Foo"), named("MyCopy"), named("Vec"), named("Box"));
            let own: Vec<_> = headers
                .inventory_root()
                .headers()
                .iter()
                .filter(|raw| raw.scope.crate_key == root.crate_key)
                .collect();
            assert_eq!(own.len(), 3);
            db.trace.detailed = true;
            let start = db.trace.events.len();
            let mut kinds = [0; 3];
            for raw in own {
                let key = QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                    inventory: seal.clone(),
                    implementation: raw.identity,
                }));
                let first = ready(db.fetch(&SyntaxProvider, key.clone()));
                let second = ready(db.fetch(&SyntaxProvider, key));
                assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
                let QueryValue::ImplHeadTypes(prepared) = first.value.as_ref() else {
                    panic!()
                };
                let ImplHeadTypeResult::Resolved {
                    self_type,
                    trait_ref: Some(trait_ref),
                } = prepared.result()
                else {
                    panic!("{edition} origin{origin}: {:?}", prepared.result())
                };
                assert_eq!(&trait_ref.self_ty, self_type);
                assert!(trait_ref.args.is_empty());
                let canonical = prepared.syntax().canonical_owner().unwrap().identity();
                if *self_type == TyTerm::Primitive(PrimitiveType::I32) {
                    assert_eq!(trait_ref.trait_key, copy);
                    kinds[0] += 1;
                } else if prepared.syntax().generics().is_empty() {
                    assert_eq!(
                        *self_type,
                        TyTerm::Nominal(
                            vec,
                            vec![GenericArg::Type(TyTerm::Nominal(
                                boxed,
                                vec![GenericArg::Type(TyTerm::Primitive(PrimitiveType::I32))]
                            ))]
                        )
                    );
                    assert_eq!(trait_ref.trait_key, foo);
                    kinds[1] += 1;
                } else {
                    assert_eq!(
                        *self_type,
                        TyTerm::Nominal(
                            vec,
                            vec![GenericArg::Type(TyTerm::Param(ParamKey {
                                definition: canonical,
                                ordinal: 0,
                            }))]
                        )
                    );
                    assert_eq!(trait_ref.trait_key, foo);
                    assert!(
                        prepared.syntax().generics()[0]
                            .bounds
                            .iter()
                            .any(|token| token.text == "MyCopy")
                    );
                    kinds[2] += 1;
                }
            }
            assert_eq!(kinds, [1, 1, 1]);
            assert!(db.trace.events[start..].iter().all(|event| !matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::FunctionSignature(_)
            )));
            let raw = headers
                .inventory_root()
                .headers()
                .iter()
                .find(|raw| raw.scope.crate_key == root.crate_key)
                .unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                db.fetch(
                    &SyntaxProvider,
                    QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                        inventory: seal,
                        implementation: raw.identity,
                    }))
                ),
                QueryPoll::Ready(Err(_))
            ));
            if origin != 0 {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn unchanged_impl_heads_keep_region_and_cold_metadata_demands_explicit() {
    use crate::{
        db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::impl_head_types::{ImplHeadPending, ImplHeadTypeResult},
        traits::header_syntax::ImplHeaderSyntaxKey,
    };
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("actual pending impl head: {error}"),
            QueryPoll::NeedInput(input) => panic!("actual pending impl head input: {input:?}"),
        }
    }
    let originals = [
        ("tests/ui/associated-types/issue-47139-2.rs", [2, 3, 0], 5),
        (
            "tests/ui/zero-sized/zero-size-type-destructors.rs",
            [0, 0, 1],
            0,
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (case, expected, expected_predicates) in originals {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/upstream/rustc-checkout")
                .join(case);
            let (mut db, seal, root) = prepared_impl_header_world(&path, edition);
            let inventory = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedScopeImplHeaders(seal.clone()),
            ));
            let QueryValue::PreparedScopeImplHeaders(headers) = inventory.value.as_ref() else {
                panic!()
            };
            let mut observed = [0; 3];
            let mut predicates = 0;
            db.trace.detailed = true;
            let start = db.trace.events.len();
            for raw in headers
                .inventory_root()
                .headers()
                .iter()
                .filter(|raw| raw.scope.crate_key == root.crate_key)
            {
                let value = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                        inventory: seal.clone(),
                        implementation: raw.identity,
                    })),
                ));
                let QueryValue::ImplHeadTypes(prepared) = value.value.as_ref() else {
                    panic!()
                };
                predicates += prepared.syntax().predicates().len();
                match prepared.result() {
                    ImplHeadTypeResult::Resolved { .. } => observed[0] += 1,
                    ImplHeadTypeResult::Pending(ImplHeadPending::SymbolicRegions) => {
                        observed[1] += 1
                    }
                    ImplHeadTypeResult::Pending(ImplHeadPending::DeclarationMetadata) => {
                        observed[2] += 1
                    }
                    other => panic!("{edition} {case}: {other:?}"),
                }
            }
            assert_eq!(observed, expected, "{edition} {case}");
            assert_eq!(predicates, expected_predicates);
            assert!(db.trace.events[start..].iter().all(|event| !matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::FunctionSignature(_)
            )));
        }
    }
}
