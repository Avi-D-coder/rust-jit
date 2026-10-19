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
                let predicate_key = QueryKey::ImplPredicates(Box::new(ImplHeaderSyntaxKey {
                    inventory: seal.clone(),
                    implementation: raw.identity,
                }));
                let predicates = ready(db.fetch(&SyntaxProvider, predicate_key.clone()));
                let repeated = ready(db.fetch(&SyntaxProvider, predicate_key));
                assert!(std::sync::Arc::ptr_eq(&predicates.value, &repeated.value));
                let QueryValue::ImplPredicates(predicates) = predicates.value.as_ref() else {
                    panic!()
                };
                use crate::{
                    frontend::impl_predicates::ImplPredicateResult,
                    traits::predicate::{PredicateResult, SourcePredicateRecipe},
                };
                let ImplPredicateResult::Requirements(requirements) = predicates.result() else {
                    panic!("{:?}", predicates.result())
                };
                if prepared.syntax().generics().is_empty() {
                    assert!(requirements.is_empty());
                } else {
                    assert_eq!(requirements.len(), 2);
                    let parameter = ParamKey {
                        definition: canonical,
                        ordinal: 0,
                    };
                    assert_eq!(
                        requirements[0],
                        SourcePredicateRecipe::ResolvedSizedDefault(
                            crate::traits::term::TraitRef {
                                trait_key: match environment
                                    .names()
                                    .lookup_source_path(
                                        Namespace::Type,
                                        &[ImportSegment::Name("Sized".into())],
                                        false,
                                        &crate::syntax::outline::ScopeKey {
                                            crate_key: environment
                                                .language_item_core(
                                                    &prepared.syntax().source().profile
                                                )
                                                .unwrap(),
                                            module_path: vec!["marker".into()],
                                            owner: None,
                                        },
                                        edition,
                                    )
                                    .unwrap()
                                {
                                    NameLookup::Defined(found) => found.identity,
                                    other => panic!("{other:?}"),
                                },
                                self_ty: TyTerm::Param(parameter),
                                args: vec![],
                            }
                        )
                    );
                    let SourcePredicateRecipe::Explicit {
                        result: PredicateResult::Implemented(actual),
                        ..
                    } = &requirements[1]
                    else {
                        panic!()
                    };
                    assert_eq!(actual.trait_key, copy);
                    assert_eq!(actual.self_ty, TyTerm::Param(parameter));
                    assert!(actual.args.is_empty());
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
                        inventory: seal.clone(),
                        implementation: raw.identity,
                    }))
                ),
                QueryPoll::Ready(Err(_))
            ));
            assert!(matches!(
                db.fetch(
                    &SyntaxProvider,
                    QueryKey::ImplPredicates(Box::new(ImplHeaderSyntaxKey {
                        inventory: seal.clone(),
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
                let plan = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::ImplPredicates(Box::new(ImplHeaderSyntaxKey {
                        inventory: seal.clone(),
                        implementation: raw.identity,
                    })),
                ));
                let QueryValue::ImplPredicates(plan) = plan.value.as_ref() else {
                    panic!()
                };
                use crate::{
                    frontend::impl_predicates::ImplPredicateResult,
                    traits::predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
                };
                match (prepared.result(), plan.result()) {
                    (
                        ImplHeadTypeResult::Pending(expected),
                        ImplPredicateResult::HeadPending(actual),
                    ) => assert_eq!(expected, actual),
                    (
                        ImplHeadTypeResult::Resolved { .. },
                        ImplPredicateResult::Requirements(requirements),
                    ) => {
                        assert!(requirements.iter().any(|row| matches!(
                            row,
                            SourcePredicateRecipe::ResolvedSizedDefault(_)
                        )));
                        assert!(requirements.iter().any(|row| matches!(
                            row,
                            SourcePredicateRecipe::Explicit {
                                result: PredicateResult::Implemented(_),
                                ..
                            }
                        )));
                        if prepared.syntax().predicates().len() == 2 {
                            assert_eq!(requirements.len(), 3);
                            assert!(requirements.iter().any(|row| matches!(
                                row,
                                SourcePredicateRecipe::Explicit {
                                    result: PredicateResult::Pending(
                                        PredicatePending::AssociatedIdentity
                                    ),
                                    ..
                                }
                            )));
                        } else {
                            assert_eq!(requirements.len(), 2);
                        }
                    }
                    other => panic!("predicate preparation differs: {other:?}"),
                }
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

#[test]
fn unchanged_sized_relaxations_use_actual_language_item_across_origins_and_editions() {
    use crate::{
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        frontend::{source_types::WrittenTypeDeclaration, type_name_namespace::SourceTypeOwner},
        traits::{
            header_syntax::ImplHeaderSyntaxKey,
            predicate::{PredicateResult, SourcePredicateRecipe},
        },
    };
    use ra_ap_syntax::{AstNode, ast};
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            other => panic!(
                "{}",
                match other {
                    QueryPoll::Ready(Err(error)) => error.to_string(),
                    QueryPoll::NeedInput(input) => format!("{input:?}"),
                    _ => unreachable!(),
                }
            ),
        }
    }
    let cases = [
        (
            "tests/ui/unsized/unsized.rs",
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/unsized/unsized.rs"
            ),
            1,
        ),
        (
            "tests/ui/higher-ranked/trait-bounds/normalize-under-binder/issue-62529-2.rs",
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/higher-ranked/trait-bounds/normalize-under-binder/issue-62529-2.rs"
            ),
            2,
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (case, original, expected_relaxations) in cases {
            for origin in 0..5 {
                let shadow = origin == 3;
                let alias = origin == 4;
                if (shadow || alias) && (edition != 2024 || expected_relaxations != 1) {
                    continue;
                }
                let emit = "macro_rules! emit { ($($t:tt)*) => { $($t)* }; }";
                let path = match origin {
                    0 => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../fixtures/upstream/rustc-checkout")
                        .join(case),
                    1 => temporary_source(
                        &format!("sized-{edition}-{origin}-{expected_relaxations}"),
                        &format!("{emit} emit!{{{original}}}"),
                    ),
                    2 => temporary_source(
                        &format!("sized-{edition}-{origin}-{expected_relaxations}"),
                        &format!(
                            "{emit} macro_rules! wrap {{ ($($t:tt)*) => {{ emit!{{$($t)*}} }}; }} wrap!{{{original}}}"
                        ),
                    ),
                    3 => temporary_source(
                        "sized-user-shadow",
                        &format!("{original}\ntrait Sized {{}}"),
                    ),
                    _ => temporary_source(
                        "sized-alias-subject",
                        &format!(
                            "{original}\ntype Identity<A> = A; struct AliasOwner<A>(PhantomData<A>); impl<A> T1 for AliasOwner<A> where Identity<A>: ?Sized {{}}"
                        ),
                    ),
                };
                let (mut db, seal, root) = prepared_impl_header_world(&path, edition);
                let inventory = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::PreparedScopeImplHeaders(seal.clone()),
                ));
                let QueryValue::PreparedScopeImplHeaders(headers) = inventory.value.as_ref() else {
                    panic!()
                };
                let environment = headers.inventory_root().environment().unwrap();
                let sources: Vec<_> = environment.sources().iter().collect();
                let mut observed = 0;
                db.trace.detailed = true;
                let start = db.trace.events.len();
                for raw in headers
                    .inventory_root()
                    .headers()
                    .iter()
                    .filter(|raw| raw.scope.crate_key == root.crate_key)
                {
                    let syntax = ready(db.fetch(
                        &SyntaxProvider,
                        QueryKey::ImplHeaderSyntax(Box::new(ImplHeaderSyntaxKey {
                            inventory: seal.clone(),
                            implementation: raw.identity,
                        })),
                    ));
                    let QueryValue::ImplHeaderSyntax(syntax) = syntax.value.as_ref() else {
                        panic!()
                    };
                    // This source service does not need a resolved impl head: the
                    // lifetime-bearing head remains pending in Q58 independently.
                    let definition = syntax.canonical_owner().unwrap().identity();
                    let header = db.input(&InputKey::DefinitionHeader(definition)).unwrap();
                    let context = db.input(&InputKey::DeclarationContext(definition)).unwrap();
                    let (
                        InputValue::DefinitionHeader(header),
                        InputValue::DeclarationContext(context),
                    ) = (header.as_ref(), context.as_ref())
                    else {
                        panic!()
                    };
                    assert_eq!(header.tokens, raw.header);
                    assert_eq!(context.range, raw.range);
                    let node = syntax
                        .parsed()
                        .syntax_at(context.range, |node| ast::Impl::can_cast(node.kind()))
                        .unwrap()
                        .unwrap();
                    let declaration = WrittenTypeDeclaration::implementation(
                        SourceTypeOwner {
                            definition,
                            scope: &context.scope,
                            header,
                        },
                        syntax.generics(),
                        &node,
                        None,
                    );
                    let core = environment.language_item_core(declaration.profile());
                    let sized = ready(crate::frontend::language_items::sized(
                        &mut db,
                        &sources,
                        declaration.profile(),
                        core,
                    ))
                    .unwrap();
                    let requirements = ready(crate::frontend::source_predicates::prepare(
                        &mut db,
                        environment.names(),
                        &sources,
                        &declaration,
                        definition,
                        syntax.generics(),
                        &node,
                        core,
                    ));
                    for row in &requirements {
                        if let SourcePredicateRecipe::Explicit {
                            result:
                                PredicateResult::Relaxed {
                                    trait_ref,
                                    parameter,
                                },
                            ..
                        } = row
                        {
                            assert_eq!(trait_ref.trait_key == sized, !shadow);
                            assert!(trait_ref.args.is_empty());
                            assert_eq!(requirements.iter().any(|default| matches!(default,
                                SourcePredicateRecipe::ResolvedSizedDefault(required) if required.self_ty == trait_ref.self_ty && required.trait_key == sized)), shadow || parameter.is_none());
                            assert!(!requirements.iter().any(|default| matches!(default,
                                SourcePredicateRecipe::ImplicitSizedDefault(key) if trait_ref.self_ty == crate::traits::term::TyTerm::Param(*key))));
                            if let Some(key) = parameter {
                                assert_eq!(
                                    trait_ref.self_ty,
                                    crate::traits::term::TyTerm::Param(*key)
                                );
                            }
                            observed += 1;
                        }
                    }
                    // A source without a matched compiler facade retains its default;
                    // plain spelling lookup is insufficient to resolve the language item.
                    let absent = ready(crate::frontend::source_predicates::prepare(
                        &mut db,
                        environment.names(),
                        &sources,
                        &declaration,
                        definition,
                        syntax.generics(),
                        &node,
                        None,
                    ));
                    assert!(
                        absent.iter().any(|row| matches!(
                            row,
                            SourcePredicateRecipe::ImplicitSizedDefault(_)
                        ))
                    );
                }
                assert_eq!(
                    observed,
                    expected_relaxations + usize::from(alias),
                    "{case} edition{edition} origin{origin}"
                );
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
}

#[test]
fn unchanged_trait_declarations_share_actual_parser_and_namespace_across_origins() {
    use crate::{
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        frontend::{
            names::NameLookup,
            trait_declarations::TraitDeclarationResult,
            type_name_namespace::{SourceTypeOwner, lookup_declaration_name},
        },
        source::manifest::Namespace,
        syntax::imports::ImportSegment,
    };
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("{error}"),
            QueryPoll::NeedInput(input) => panic!("{input:?}"),
        }
    }
    const ORIGINAL: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/unsized/unsized.rs");
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..4 {
            if origin == 3 && edition != 2024 {
                continue;
            }
            let emit = "macro_rules! emit { ($($t:tt)*) => { $($t)* }; }";
            let path = match origin {
                0 => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../fixtures/upstream/rustc-checkout/tests/ui/unsized/unsized.rs"),
                1 => temporary_source(
                    &format!("trait-kind-{edition}-{origin}"),
                    &format!("{emit} emit!{{{ORIGINAL}}}"),
                ),
                2 => temporary_source(
                    &format!("trait-kind-{edition}-{origin}"),
                    &format!(
                        "{emit} macro_rules! wrap {{ ($($t:tt)*) => {{ emit!{{$($t)*}} }}; }} wrap!{{{ORIGINAL}}}"
                    ),
                ),
                _ => temporary_source(
                    "trait-kind-nontrait-bound",
                    &format!(
                        "{ORIGINAL}\ntrait BoundTarget {{}} struct Receiver<X>(std::marker::PhantomData<X>); impl<X: S1<X>> BoundTarget for Receiver<X> {{}}"
                    ),
                ),
            };
            let (mut db, seal, root) = prepared_impl_header_world(&path, edition);
            let inventory = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedScopeImplHeaders(seal.clone()),
            ));
            let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else {
                panic!()
            };
            let environment = inventory.inventory_root().environment().unwrap();
            let select = |namespace, name: &str| match environment
                .names()
                .lookup_source_path(
                    namespace,
                    &[ImportSegment::Name(name.into())],
                    false,
                    &root,
                    edition,
                )
                .unwrap()
            {
                NameLookup::Defined(selected) => selected.identity,
                other => panic!("{name}: {other:?}"),
            };
            let expected = select(Namespace::Type, "T1");
            let sources: Vec<_> = environment.sources().iter().collect();
            db.trace.detailed = true;
            let start = db.trace.events.len();
            for (name, namespace, is_trait) in [
                ("T1", Namespace::Type, true),
                ("T2", Namespace::Type, true),
                ("T3", Namespace::Type, true),
                ("T4", Namespace::Type, true),
                ("T5", Namespace::Type, true),
                ("T6", Namespace::Type, true),
                ("T7", Namespace::Type, true),
                ("T8", Namespace::Type, true),
                ("T9", Namespace::Type, true),
                ("S1", Namespace::Type, false),
                ("E", Namespace::Type, false),
                ("TT", Namespace::Type, false),
                ("f", Namespace::Value, false),
            ] {
                let definition = select(namespace, name);
                let key = QueryKey::TraitDeclaration(definition);
                let first = ready(db.fetch(&SyntaxProvider, key.clone()));
                let second = ready(db.fetch(&SyntaxProvider, key));
                assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
                let QueryValue::TraitDeclaration(declaration) = first.value.as_ref() else {
                    panic!()
                };
                assert_eq!(declaration.definition(), definition);
                assert_eq!(declaration.header().is_some(), is_trait);
                assert_eq!(
                    declaration.result(),
                    if is_trait {
                        TraitDeclarationResult::Trait
                    } else {
                        TraitDeclarationResult::NotTrait
                    },
                    "{name} {edition} origin{origin}"
                );
                if is_trait {
                    let facts = declaration.header().unwrap();
                    assert!(!facts.is_auto() && !facts.is_unsafe() && !facts.is_alias());
                    assert!(facts.predicates().is_empty());
                    assert!(facts.attributes().is_empty());
                    let expected_generics: &[&str] = match name {
                        "T1" | "T2" => &[],
                        "T3" | "T4" | "T8" | "T9" => &["X"],
                        "T6" => &["Y", "X"],
                        "T5" | "T7" => &["X", "Y"],
                        _ => unreachable!(),
                    };
                    assert_eq!(facts.generics().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), expected_generics);
                    assert_eq!(facts.generic_ranges().len(), expected_generics.len());
                    assert_eq!(facts.supertraits_range().is_some(), name == "T3");
                    assert!(facts.predicates_range().is_none());
                    assert!(facts.attribute_ranges().is_empty());
                    assert_eq!(facts.supertraits().iter().map(|t| t.text.as_str()).collect::<String>(), if name == "T3" { "T2" } else { "" });
                    if name == "T3" {
                        assert_eq!(facts.generics()[0].bounds.iter().map(|t| t.text.as_str()).collect::<String>(), "T1");
                    }
                    let parse = db.input(&InputKey::DefinitionSource(definition)).unwrap();
                    let header = db.input(&InputKey::DefinitionHeader(definition)).unwrap();
                    let context = db.input(&InputKey::DeclarationContext(definition)).unwrap();
                    let (
                        InputValue::DefinitionSource(parse),
                        InputValue::DefinitionHeader(header),
                        InputValue::DeclarationContext(context),
                    ) = (parse.as_ref(), header.as_ref(), context.as_ref())
                    else {
                        panic!()
                    };
                    assert!(declaration.matches_owner(definition, parse, header, &context.scope));
                    assert_eq!(declaration.source(), Some(parse));
                    assert_eq!(declaration.scope(), Some(&context.scope));
                    assert_eq!(declaration.parsed().unwrap().key(), *parse);
                    let lookup = ready(lookup_declaration_name(
                        &mut db,
                        SourceTypeOwner {
                            definition,
                            scope: &context.scope,
                            header,
                        },
                        &sources,
                        "T1",
                    ));
                    assert!(
                        matches!(&lookup, NameLookup::Defined(found) if found.identity == expected),
                        "{lookup:?}"
                    );
                }
            }
            if origin == 3 {
                let mut pending_bound = 0;
                for raw in inventory
                    .inventory_root()
                    .headers()
                    .iter()
                    .filter(|raw| raw.scope.crate_key == root.crate_key)
                {
                    let result = ready(db.fetch(
                        &SyntaxProvider,
                        QueryKey::ImplPredicates(Box::new(
                            crate::traits::header_syntax::ImplHeaderSyntaxKey {
                                inventory: seal.clone(),
                                implementation: raw.identity,
                            },
                        )),
                    ));
                    let QueryValue::ImplPredicates(prepared) = result.value.as_ref() else {
                        panic!()
                    };
                    let crate::frontend::impl_predicates::ImplPredicateResult::Requirements(
                        requirements,
                    ) = prepared.result()
                    else {
                        panic!("{:?}", prepared.result())
                    };
                    pending_bound += requirements
                        .iter()
                        .filter(|row| {
                            matches!(
                                row,
                                crate::traits::predicate::SourcePredicateRecipe::Explicit {
                                    result: crate::traits::predicate::PredicateResult::Pending(
                                        crate::traits::predicate::PredicatePending::TraitName
                                    ),
                                    ..
                                }
                            )
                        })
                        .count();
                }
                assert_eq!(pending_bound, 1);
            }
            let absent = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::TraitDeclaration(Digest::of_bytes(b"unpublished-trait-kind")),
            ));
            assert!(
                matches!(absent.value.as_ref(), QueryValue::TraitDeclaration(declaration) if declaration.result() == TraitDeclarationResult::MetadataPending && declaration.header().is_none())
            );
            assert!(db.trace.events[start..].iter().all(|event| !matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::FunctionSignature(_)
            )));
            db.begin_expansion_epoch(2).unwrap();
            match db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(expected)) {
                QueryPoll::Ready(Ok(value)) if origin == 0 || origin == 3 => assert!(
                    matches!(value.value.as_ref(), QueryValue::TraitDeclaration(declaration) if declaration.is_trait())
                ),
                QueryPoll::Ready(Err(_)) if origin == 1 || origin == 2 => {}
                other => panic!("trait source epoch {edition} origin{origin}: {other:?}"),
            }
        }
    }
}

#[test]
fn unchanged_auto_and_unsafe_trait_headers_keep_actual_modifiers_without_body_preparation() {
    use crate::{
        core::digest::StableEncode,
        db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::names::NameLookup,
        source::manifest::Namespace,
        syntax::imports::ImportSegment,
    };
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("{error}"),
            QueryPoll::NeedInput(input) => panic!("{input:?}"),
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        for (case, expected) in [
            ("tests/ui/auto-traits/auto-traits.rs", &[("Auto", true, false), ("AutoUnsafe", true, true)][..]),
            ("tests/ui/traits/safety-ok.rs", &[("Foo", false, true)][..]),
        ] {
            let (mut db, seal, scope) = prepared_impl_header_world(&root.join(case), edition);
            let inventory = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal)));
            let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else { panic!() };
            let environment = inventory.inventory_root().environment().unwrap();
            db.trace.detailed = true;
            let start = db.trace.events.len();
            for (name, auto_trait, unsafe_trait) in expected {
                let NameLookup::Defined(selected) = environment.names().lookup_source_path(
                    Namespace::Type, &[ImportSegment::Name((*name).into())], false, &scope, edition,
                ).unwrap() else { panic!("actual upstream trait absent") };
                let key = QueryKey::TraitDeclaration(selected.identity);
                let first = ready(db.fetch(&SyntaxProvider, key.clone()));
                let second = ready(db.fetch(&SyntaxProvider, key));
                assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
                let QueryValue::TraitDeclaration(declaration) = first.value.as_ref() else { panic!() };
                let header = declaration.header().unwrap();
                assert_eq!(header.is_auto(), *auto_trait);
                assert_eq!(header.is_unsafe(), *unsafe_trait);
                assert!(!header.is_alias());
                assert!(header.generics().is_empty());
                assert!(header.supertraits().is_empty());
                assert!(header.predicates().is_empty());
                assert_eq!(header.fingerprint(), declaration.header().unwrap().fingerprint());
            }
            assert!(db.trace.events[start..].iter().all(|event| !matches!(
                event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_)
            )));
        }
    }
}


#[test]
fn unchanged_impl_receipts_reuse_actual_parse_inputs_without_claiming_solver_readiness() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        for case in ["tests/ui/methods/method-self-arg-trait.rs", "tests/ui/traits/safety-ok.rs"] {
            let (mut db, seal, _) = prepared_impl_header_world(&root.join(case), edition);
            let count = crate::traits::upstream::check_scope_receipts(&mut db, seal);
            assert!(count > 0, "no actual impl receipts for {case}, edition{edition}");
            eprintln!("actual source receipts {case}, edition{edition}: {count}; no solver or guest execution");
        }
    }
}
