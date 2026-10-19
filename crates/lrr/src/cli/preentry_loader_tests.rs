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
                    assert_eq!(
                        facts
                            .generics()
                            .iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>(),
                        expected_generics
                    );
                    assert_eq!(facts.generic_ranges().len(), expected_generics.len());
                    assert_eq!(facts.supertraits_range().is_some(), name == "T3");
                    assert!(facts.predicates_range().is_none());
                    assert!(facts.attribute_ranges().is_empty());
                    assert_eq!(
                        facts
                            .supertraits()
                            .iter()
                            .map(|t| t.text.as_str())
                            .collect::<String>(),
                        if name == "T3" { "T2" } else { "" }
                    );
                    if name == "T3" {
                        assert_eq!(
                            facts.generics()[0]
                                .bounds
                                .iter()
                                .map(|t| t.text.as_str())
                                .collect::<String>(),
                            "T1"
                        );
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
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        for (case, expected) in [
            (
                "tests/ui/auto-traits/auto-traits.rs",
                &[("Auto", true, false), ("AutoUnsafe", true, true)][..],
            ),
            ("tests/ui/traits/safety-ok.rs", &[("Foo", false, true)][..]),
        ] {
            let (mut db, seal, scope) = prepared_impl_header_world(&root.join(case), edition);
            let inventory =
                ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal)));
            let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else {
                panic!()
            };
            let environment = inventory.inventory_root().environment().unwrap();
            db.trace.detailed = true;
            let start = db.trace.events.len();
            for (name, auto_trait, unsafe_trait) in expected {
                let NameLookup::Defined(selected) = environment
                    .names()
                    .lookup_source_path(
                        Namespace::Type,
                        &[ImportSegment::Name((*name).into())],
                        false,
                        &scope,
                        edition,
                    )
                    .unwrap()
                else {
                    panic!("actual upstream trait absent")
                };
                let key = QueryKey::TraitDeclaration(selected.identity);
                let first = ready(db.fetch(&SyntaxProvider, key.clone()));
                let second = ready(db.fetch(&SyntaxProvider, key));
                assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
                let QueryValue::TraitDeclaration(declaration) = first.value.as_ref() else {
                    panic!()
                };
                let header = declaration.header().unwrap();
                assert_eq!(header.is_auto(), *auto_trait);
                assert_eq!(header.is_unsafe(), *unsafe_trait);
                assert!(!header.is_alias());
                assert!(header.generics().is_empty());
                assert!(header.supertraits().is_empty());
                assert!(header.predicates().is_empty());
                assert_eq!(
                    header.fingerprint(),
                    declaration.header().unwrap().fingerprint()
                );
            }
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
fn unchanged_impl_receipts_reuse_actual_parse_inputs_without_claiming_solver_readiness() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        for case in [
            "tests/ui/methods/method-self-arg-trait.rs",
            "tests/ui/traits/safety-ok.rs",
        ] {
            let (mut db, seal, _) = prepared_impl_header_world(&root.join(case), edition);
            let count = crate::traits::upstream::check_scope_receipts(&mut db, seal);
            assert!(
                count > 0,
                "no actual impl receipts for {case}, edition{edition}"
            );
            eprintln!(
                "actual source receipts {case}, edition{edition}: {count}; no solver or guest execution"
            );
        }
    }
}

#[test]
fn unchanged_trait_root_requires_exact_captured_facade_sources_before_header_preparation() {
    use crate::{
        core::profile::ProfileIdentity,
        expand::source_discovery::{
            SourceDiscovery,
            impl_catalog::{LiveImplCatalogPreparation, MandatoryTraitFacade},
        },
        source::{
            embedded_crates::{EmbeddedFile, standard_facade_crates},
            manifest::Namespace,
        },
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    for edition in [2015, 2018, 2021, 2024] {
        for mutation in ["original", "missing", "changed", "extra"] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let mut embedded = standard_facade_crates(&profile);
            let count: usize = embedded.iter().map(|unit| unit.files.len()).sum();
            let core = embedded
                .iter_mut()
                .find(|unit| unit.name == "core")
                .unwrap();
            match mutation {
                "missing" => core
                    .files
                    .retain(|file| file.path != "<lrr-facade>/core/clone.rs"),
                "changed" => {
                    core.files
                        .iter_mut()
                        .find(|file| file.path == "<lrr-facade>/core/clone.rs")
                        .unwrap()
                        .text = "/* replaced facade source under the same identity */"
                }
                "extra" => core.files.push(EmbeddedFile {
                    module_path: vec!["unregistered".into()],
                    path: "<lrr-facade>/core/unregistered.rs",
                    text: "/* undeclared module under a reserved facade unit */",
                }),
                "original" => {}
                _ => unreachable!(),
            }
            let ((_, source, sources, _), _) = super::crate_graph::capture_with(
                &path,
                false,
                edition,
                &super::crate_graph::ExternCrates::default(),
                embedded,
                None,
            )
            .unwrap();
            let discovery = SourceDiscovery::new_deferred_imports(
                sources,
                source.outline().root_scope.clone(),
                Namespace::Value,
                source.outline().root_scope.crate_key,
                1,
            )
            .unwrap();
            let prepared = discovery.prepare_live_impl_catalog_sources();
            if matches!(mutation, "changed" | "extra") {
                match prepared {
                    Err(error) => assert_eq!(error.code, "LRR-E-IMPL-CATALOG-FACADE"),
                    Ok(_) => panic!("invalid facade source accepted: {edition} {mutation}"),
                }
                continue;
            }
            let LiveImplCatalogPreparation::NeedConfiguredWorld {
                captured: live,
                trait_facade,
            } = prepared.unwrap()
            else {
                panic!("actual root membership absent: {edition} {mutation}")
            };
            assert_eq!(live.selected_root(), source.selection());
            match (mutation, trait_facade) {
                ("original", MandatoryTraitFacade::NeedHeaders { captured }) => {
                    assert_eq!(captured.sources().len(), count);
                    assert_eq!(captured.owning_profile(), &profile);
                    assert!(
                        captured
                            .sources()
                            .iter()
                            .all(|view| view.same_publication(&source))
                    );
                    assert!(
                        captured.sources().iter().all(|view| view
                            .selection()
                            .outline
                            .parse
                            .profile
                            .edition
                            == edition)
                    );
                    assert_ne!(captured.fingerprint(), live.fingerprint());
                    eprintln!(
                        "edition{edition}: {count} actual facade files captured; header compatibility and complete catalog remain pending"
                    );
                }
                ("missing", MandatoryTraitFacade::Missing) => {}
                _ => panic!("source preparation demand differs: {edition} {mutation}"),
            }
        }
    }
}

#[test]
fn different_unchanged_roots_keep_facade_source_identity_and_distinct_world_membership() {
    use crate::{
        core::profile::ProfileIdentity,
        expand::source_discovery::{
            SourceDiscovery,
            impl_catalog::{LiveImplCatalogPreparation, MandatoryTraitFacade},
        },
        source::{embedded_crates::standard_facade_crates, manifest::Namespace},
    };
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        let mut previous = None;
        for case in [
            "tests/ui/traits/safety-ok.rs",
            "tests/ui/methods/method-self-arg-trait.rs",
        ] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let ((_, source, sources, _), _) = super::crate_graph::capture_with(
                &root.join(case),
                false,
                edition,
                &super::crate_graph::ExternCrates::default(),
                standard_facade_crates(&profile),
                None,
            )
            .unwrap();
            let discovery = SourceDiscovery::new_deferred_imports(
                sources,
                source.outline().root_scope.clone(),
                Namespace::Value,
                source.outline().root_scope.crate_key,
                1,
            )
            .unwrap();
            let LiveImplCatalogPreparation::NeedConfiguredWorld {
                captured: live,
                trait_facade: MandatoryTraitFacade::NeedHeaders { captured },
            } = discovery.prepare_live_impl_catalog_sources().unwrap()
            else {
                panic!("actual facade sources not captured: {case}, edition{edition}")
            };
            let identity = (live.fingerprint(), captured.fingerprint());
            if let Some((world, facade)) = previous {
                assert_ne!(world, identity.0);
                assert_eq!(
                    facade, identity.1,
                    "unrelated upstream root invalidates facade sources"
                );
            }
            previous = Some(identity);
        }
    }
}

#[test]
fn mixed_edition_capture_keeps_actual_facade_owning_profile_separate_from_consumers() {
    use crate::{
        core::profile::ProfileIdentity,
        expand::source_discovery::{
            SourceDiscovery,
            impl_catalog::{LiveImplCatalogPreparation, MandatoryTraitFacade},
        },
        source::{embedded_crates::standard_facade_crates, manifest::Namespace},
    };
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for (edition, dependency_edition) in [(2015, 2024), (2018, 2015), (2021, 2018), (2024, 2015)] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let external = super::crate_graph::ExternCrates::new(
            vec![super::crate_graph::ExternArg {
                name: "actual_auxiliary".into(),
                path: root.join("tests/ui/consts/auxiliary/issue-17718-aux.rs"),
                prelude: true,
                proc_macro: false,
            }],
            vec![("actual_auxiliary".into(), dependency_edition)],
        )
        .unwrap();
        let ((_, source, sources, _), _) = super::crate_graph::capture_with(
            &root.join("tests/ui/traits/safety-ok.rs"),
            false,
            edition,
            &external,
            standard_facade_crates(&profile),
            None,
        )
        .unwrap();
        let discovery = SourceDiscovery::new_deferred_imports(
            sources,
            source.outline().root_scope.clone(),
            Namespace::Value,
            source.outline().root_scope.crate_key,
            1,
        )
        .unwrap();
        let LiveImplCatalogPreparation::NeedConfiguredWorld {
            captured: live,
            trait_facade: MandatoryTraitFacade::NeedHeaders { captured },
        } = discovery.prepare_live_impl_catalog_sources().unwrap()
        else {
            panic!("actual mixed-edition sources absent")
        };
        assert_eq!(live.live_source_profiles().len(), 2);
        assert_eq!(captured.owning_profile(), &profile);
        assert!(
            captured
                .sources()
                .iter()
                .all(|view| view.selection().outline.parse.profile.edition == edition)
        );
        assert!(live.live_units().len() >= 2);
    }
}

#[test]
fn unchanged_local_derive_selects_real_output_with_canonical_insertion_and_origin() {
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{key::QueryKey, memo::Database},
        expand::{
            derive_origin::DeriveDeclarationOrigin,
            generated_declarations::GeneratedDeclarations,
            generated_items::GeneratedItems,
            source_derives::SourceDerivePoll,
            source_discovery::{SourceCompilerPreparation, SourceDiscovery},
        },
        frontend::generated_headers::GeneratedHeaderPlan,
        syntax::{declaration_scan::DeclarationProducerKind, token_wire::Key},
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    for edition in [2015, 2018, 2021, 2024] {
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
        let (compiler, inputs, source, _) =
            super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources)
                .unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        db.trace.detailed = true;
        let mut discovery = match SourceDiscovery::for_impl_catalog(&compiler, &source, 1, &db)
            .unwrap()
        {
            SourceCompilerPreparation::Ready(discovery) => discovery,
            SourceCompilerPreparation::NeedModule { .. } => panic!("actual source module pending"),
        };
        // Retain the actual original parent for the independent adoption
        // controls before the coordinator can complete its annotation group.
        let mut parent = (*discovery
            .original_declaration_parent_for_control(source.selection())
            .unwrap()).clone();
        // Drive the real configured preparation, retaining its actual result.
        // This control validates selection/admission, never treats incomplete
        // catalog discovery as a runtime or full source-contract pass.
        let catalog = compiler.discover(&mut discovery, &mut db);
        eprintln!(
            "local derive component edition{edition}: catalog preparation {}",
            match &catalog {
                Ok(()) => "ready",
                Err(error) => error.code,
            }
        );
        catalog.unwrap();
        let scan = source.declaration_inventory(Default::default()).unwrap();
        let producer = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == DeclarationProducerKind::Attribute
                    && row.unqualified_attribute.as_deref() == Some("derive")
            })
            .unwrap();
        let target = source
            .original_annotation_producer(producer.identity)
            .unwrap()
            .unwrap();
        let item = target.item();
        assert_ne!(
            item.scope, producer.scope,
            "the scan's coarse function owner is not the local item scope"
        );
        let group = source
            .original_annotation_group(producer.identity)
            .unwrap()
            .unwrap();
        group.require_source(&source).unwrap();
        assert_eq!(group.annotations(), &[producer.identity]);
        assert_eq!(group.item(), item.def_key.fingerprint());
        assert_ne!(group.call().item_input, producer.identity);
        let canonical = source
            .original_annotation_group(group.call().item_input)
            .unwrap()
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(&group, &canonical));
        let mut completed = std::collections::BTreeMap::new();
        let fresh_parent = parent.clone();
        let mut first_prefix = None;
        let mut completed_name_fragments = std::collections::BTreeMap::new();
        let mut first_declarations: Option<std::sync::Arc<GeneratedDeclarations>> = None;
        let origin = DeriveDeclarationOrigin::Source {
            selection: source.selection().clone(),
            producer: producer.identity,
            item: item.def_key.fingerprint(),
            header: item.header.fingerprint(),
        };
        origin.validate_database(&mut db).unwrap();
        let bad_header = DeriveDeclarationOrigin::Source {
            selection: source.selection().clone(),
            producer: producer.identity,
            item: item.def_key.fingerprint(),
            header: Digest([3; 32]),
        };
        assert_eq!(
            bad_header.validate_database(&mut db).unwrap_err().code,
            "LRR-E-CATALOG-INCOMPLETE"
        );
        let bad_producer = DeriveDeclarationOrigin::Source {
            selection: source.selection().clone(),
            producer: Digest([0; 32]),
            item: item.def_key.fingerprint(),
            header: item.header.fingerprint(),
        };
        assert_eq!(
            bad_producer.validate_database(&mut db).unwrap_err().code,
            "LRR-E-CATALOG-INCOMPLETE"
        );
        loop {
            let (poll, environment, engine) = discovery
                .inspect_original_derive_selection_with_completed(
                    source.selection(),
                    group.call().item_input,
                    &completed,
                )
                .unwrap();
            match poll {
                SourceDerivePoll::Ready(selection) => {
                    assert!(selection.is_builtin());
                    assert_eq!(selection.scope(), &item.scope);
                    selection.validate_database(&mut db).unwrap();
                    let request = selection.builtin_request(environment, engine).unwrap();
                    let record = std::sync::Arc::new(request.expand().unwrap());
                    let generated = GeneratedItems::admit(record.key(), record.clone(), 1).unwrap();
                    let declarations = std::sync::Arc::new(
                        GeneratedDeclarations::admit_source_derive_output(
                            &selection,
                            record.clone(),
                            1,
                            &source,
                            source.captured_macro_cfg().unwrap(),
                        )
                        .unwrap(),
                    );
                    assert_eq!(declarations.parent_inventory(), scan.fingerprint());
                    assert_eq!(declarations.parent_invocation(), producer.identity);
                    assert_eq!(declarations.scan().scope(), &item.scope);
                    let cfg = source.captured_macro_cfg().unwrap();
                    let catalog = crate::frontend::generated_macros::GeneratedMacroCatalog::for_admitted_output(
                        &generated, &declarations, cfg, 1,
                    ).unwrap();
                    assert_eq!(catalog.owner(), &item.scope);
                    assert!(catalog.require_captured_owner(&generated, &source.outline().root_scope, cfg, 1).is_err());
                    assert!(catalog.child_for_discovery(&generated).is_err(), "no actual child request is pending");
                    assert!(crate::frontend::generated_macros::GeneratedMacroCatalog::for_admitted_output(
                        &generated, &declarations, cfg, 2,
                    ).is_err(), "an old epoch cannot admit this output");
                    if let Some(first) = &first_declarations {
                        assert!(crate::frontend::generated_macros::GeneratedMacroCatalog::for_admitted_output(
                            &generated, first, cfg, 1,
                        ).is_err(), "a sibling output's receipt does not own this record");
                    } else { first_declarations = Some(declarations.clone()); }
                    assert!(
                        GeneratedHeaderPlan::prepare(
                            &generated,
                            item.scope.clone(),
                            Key(record.key().parent_context.0),
                            source.captured_macro_cfg().unwrap(),
                            1,
                        )
                        .is_err(),
                        "a bare local scope key does not supply insertion provenance"
                    );
                    let plan = GeneratedHeaderPlan::prepare_admitted(
                        &generated,
                        &declarations,
                        source.captured_macro_cfg().unwrap(),
                        1,
                    )
                    .unwrap();
                    let headers = declarations
                        .item_impl_headers(&generated, source.captured_macro_cfg().unwrap(), &plan)
                        .unwrap();
                    assert_eq!(headers.len(), 1, "actual selected derived impl header");
                    let view = parent
                        .derive_prefix_view(declarations.parent_invocation())
                        .unwrap();
                    let lexical_parent =
                        crate::frontend::impl_index::DeclarationMacroParent::Source(
                            std::sync::Arc::new(view.clone()),
                        );
                    let outer = lexical_parent
                        .outer(declarations.parent_invocation())
                        .unwrap();
                    assert_eq!(outer.before, u64::from(producer.range.start) + 1);
                    assert!(
                        !view.is_complete(),
                        "a private prefix is not source completion"
                    );
                    let child = discovery
                        .complete_original_derive_output_for_control(
                            source.selection(),
                            std::sync::Arc::new(view),
                            declarations,
                        )
                        .unwrap();
                    assert!(child.index().is_complete());
                    assert!(completed_name_fragments.insert(
                        record.key().invocation, child.clone()).is_none());
                    parent
                        .adopt_completed_child_prefix(record.key(), child.index(), false)
                        .unwrap();
                    assert!(
                        !parent.is_complete(),
                        "an appended output cannot finish the item group"
                    );
                    assert!(
                        parent
                            .adopt_completed_child_prefix(record.key(), child.index(), false)
                            .is_err(),
                        "duplicate occurrence append must fail without changing the prefix"
                    );
                    if first_prefix.is_none() {
                        first_prefix = Some(parent.clone());
                    }
                    assert!(
                        completed
                            .insert(selection.input().identity(), selection)
                            .is_none(),
                        "the same occurrence must not execute twice"
                    );
                }
                SourceDerivePoll::Complete(proof) => {
                    assert_eq!(
                        completed.len(),
                        2,
                        "both unchanged upstream derive occurrences ran"
                    );
                    proof.require_group(&group, &source).unwrap();
                    assert!(
                        fresh_parent
                            .clone()
                            .complete_original_annotation_group(&source, &group, &proof)
                            .is_err(),
                        "a proof cannot complete an index that never adopted its outputs"
                    );
                    assert!(
                        first_prefix
                            .as_ref()
                            .unwrap()
                            .clone()
                            .complete_original_annotation_group(&source, &group, &proof)
                            .is_err(),
                        "one appended output cannot complete a two-occurrence group"
                    );
                    assert!(parent.completed_original_annotation(&source, &group, &proof).is_err(),
                        "adopted outputs do not replace actual group completion");
                    assert!(fresh_parent.completed_original_annotation(&source, &group, &proof).is_err(),
                        "an unadopted parent cannot mint a declaration completion");
                    parent
                        .complete_original_annotation_group(&source, &group, &proof)
                        .unwrap();
                    assert!(parent.is_complete());
                    let receipt = parent.completed_original_annotation(&source, &group, &proof).unwrap();
                    assert!(parent.completed_original_annotation(&source, &group, &proof).unwrap()
                        .with_name_effects(&parent, &source, &Default::default()).is_err(),
                        "name completion requires the actual completed output fragments");
                    assert!(parent.completed_original_annotation(&source, &group, &proof).unwrap()
                        .with_name_effects(&fresh_parent, &source, &completed_name_fragments).is_err(),
                        "a fresh parent cannot certify no name effects from adopted outputs");
                    let names = parent.completed_original_annotation(&source, &group, &proof).unwrap()
                        .with_name_effects(&parent, &source, &completed_name_fragments).unwrap();
                    let repeat = parent.completed_original_annotation(&source, &group, &proof).unwrap()
                        .with_name_effects(&parent, &source, &completed_name_fragments).unwrap();
                    let (names, repeat) = (names.name_effects().unwrap(), repeat.name_effects().unwrap());
                    assert_eq!(names.len(), 2);
                    for (names, repeat) in names.iter().zip(repeat) {
                        assert!(std::sync::Arc::ptr_eq(names, repeat),
                            "completed fragments share immutable prepared name effects");
                    }
                    let crate::frontend::associated_declarations::AssociatedDeclarationsPreparation::Original(prepared) =
                        crate::frontend::associated_declarations::PreparedOriginalAssociatedDeclarations::prepare(&source).unwrap()
                        else { panic!("actual original declaration scope absent") };
                    let scope = prepared.declaration_scopes().find(|scope|
                        scope.facts().declarations().any(|row| row.item.def_key.fingerprint() == receipt.item())
                    ).unwrap();
                    let row = scope.facts().declarations().find(|row|
                        row.item.def_key.fingerprint() == receipt.item()).unwrap();
                    let parsed = scope.parsed().unwrap();
                    receipt.require_local_declaration(parsed, scope.body(), scope.scope(), row).unwrap();
                    let mut wrong_body = scope.body().clone();
                    wrong_body.def_key = Digest([0; 32]);
                    assert!(receipt.require_local_declaration(parsed, &wrong_body, scope.scope(), row).is_err());
                    let mut wrong_scope = scope.scope().clone();
                    wrong_scope.owner = Some(Digest([0; 32]));
                    assert!(receipt.require_local_declaration(parsed, scope.body(), &wrong_scope, row).is_err());
                    let other = scope.facts().declarations().find(|other|
                        other.item.def_key.fingerprint() != receipt.item()).unwrap();
                    assert!(receipt.require_local_declaration(parsed, scope.body(), scope.scope(), other).is_err());
                    assert!(matches!(db.publish_captured_declaration(&scope, receipt.item()),
                        crate::db::poll::QueryPoll::Ready(Ok(crate::db::memo::LocalDeclarationPublication::NeedsProducer(_)))),
                        "the shared publisher still refuses an annotated row without its actual receipt");
                    assert!(matches!(db.publish_captured_declaration_with_completion(&scope, &receipt),
                        crate::db::poll::QueryPoll::Ready(Ok(crate::db::memo::LocalDeclarationPublication::Published(id)))
                            if id == receipt.item()));
                    assert!(matches!(db.publish_captured_declaration_with_completion(&scope, &receipt),
                        crate::db::poll::QueryPoll::Ready(Ok(crate::db::memo::LocalDeclarationPublication::Published(id)))
                            if id == receipt.item()), "immutable declaration publication replays consistently");
                    assert!(parent.derive_prefix_view(producer.identity).is_err());
                    assert!(
                        parent
                            .complete_original_annotation_group(&source, &group, &proof)
                            .is_err(),
                        "group completion is exact once"
                    );
                    break;
                }
                _ => panic!("edition{edition}: actual item group has an unfinished demand"),
            }
        }
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::FunctionSignature(_)
        )));
        eprintln!(
            "local derive component edition{edition}: {} real outputs appended and the item group completed; no coordinator publication or guest execution",
            completed.len()
        );
    }
}

fn configured_impl_catalog_discovery(
    path: &std::path::Path,
    edition: u16,
) -> Result<
    (
        crate::db::memo::Database,
        crate::expand::source_discovery::SourceDiscovery,
        crate::syntax::outline::ScopeKey,
    ),
    crate::core::diagnostic::Diagnostic,
> {
    use crate::{
        core::profile::ProfileIdentity,
        db::memo::Database,
        expand::source_discovery::{SourceCompilerPreparation, SourceDiscovery},
    };
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(
        path,
        false,
        edition,
        &super::crate_graph::ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )?;
    let (compiler, inputs, source, _) =
        super::compile_inputs::PreparedCompileInputs::prepare(path, inputs, source, sources)?;
    let root = source.outline().root_scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db)?;
    db.begin_expansion_epoch(1)?;
    let mut discovery = match SourceDiscovery::for_impl_catalog(&compiler, &source, 1, &db)? {
        SourceCompilerPreparation::Ready(discovery) => discovery,
        SourceCompilerPreparation::NeedModule { demand, .. } => return Err(demand.diagnostic()),
    };
    if let Err(error) = compiler.discover(&mut discovery, &mut db) {
        for row in discovery.catalog_pending_source_evidence() {
            eprintln!("{row}");
        }
        if let Ok(crate::expand::source_discovery::SourceDiscoveryStep::Incomplete {
            blocked,
            ..
        }) = discovery.poll()
        {
            eprintln!("blocked source jobs: {blocked:?}");
        }
        return Err(error);
    }
    Ok((db, discovery, root))
}

fn configured_impl_catalog_world(
    path: &std::path::Path, edition: u16,
) -> Result<(crate::db::memo::Database, crate::db::seals::SealKey,
    crate::syntax::outline::ScopeKey), crate::core::diagnostic::Diagnostic> {
    let (mut db, discovery, root) = configured_impl_catalog_discovery(path, edition)?;
    discovery.publish(&mut db)?;
    Ok((db, discovery.key().clone(), root))
}

#[test]
fn unchanged_catalog_completed_annotations_publish_actual_local_declarations() {
    use crate::{
        core::digest::StableEncode,
        db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::associated_declarations::{AssociatedDeclarationsPreparation, PreparedOriginalAssociatedDeclarations},
        source::manifest::ItemKind,
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, seal, root_scope) = configured_impl_catalog_world(&path, edition).unwrap();
        db.trace.detailed = true;
        let start = db.trace.events.len();
        let input = db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
        let InputValue::DiscoverySeal(publication) = input.as_ref() else { panic!() };
        let selection = publication.sources.iter().find(|selection|
            selection.outline.crate_key == root_scope.crate_key).unwrap();
        let source = db.prepared_source(selection).unwrap().unwrap();
        let AssociatedDeclarationsPreparation::Original(prepared) =
            PreparedOriginalAssociatedDeclarations::prepare(&source).unwrap() else { panic!() };
        let mut declarations = 0;
        let mut annotated = 0;
        for scope in prepared.declaration_scopes() {
            for row in scope.facts().declarations() {
                let id = row.item.def_key.fingerprint();
                assert!(matches!(db.input(&InputKey::LocalDeclarationOwner(id)).as_deref(),
                    Some(InputValue::LocalDeclarationOwner(body)) if body == &scope.body().key()));
                assert!(matches!(db.input(&InputKey::DefinitionSource(id)).as_deref(),
                    Some(InputValue::DefinitionSource(parse)) if parse == &scope.parsed().unwrap().key()));
                assert!(matches!(db.input(&InputKey::DefinitionHeader(id)).as_deref(),
                    Some(InputValue::DefinitionHeader(header)) if header == &row.item.header));
                assert!(matches!(db.input(&InputKey::DeclarationContext(id)).as_deref(),
                    Some(InputValue::DeclarationContext(context))
                        if context.scope == row.item.scope && context.range == row.item.declaration_span));
                if row.attribute_input.is_some() {
                    assert_eq!(row.item.def_key.item_kind, ItemKind::Enum);
                    annotated += 1;
                    assert!(matches!(db.publish_captured_declaration(&scope, id),
                        QueryPoll::Ready(Ok(crate::db::memo::LocalDeclarationPublication::NeedsProducer(_)))),
                        "metadata alone does not acknowledge an annotation");
                }
                declarations += 1;
            }
        }
        assert!(declarations >= 4, "actual enum, impl and both methods are published");
        assert_eq!(annotated, 1);
        let key = QueryKey::PreparedScopeImplHeaders(seal.clone());
        let headers = match db.fetch(&SyntaxProvider, key) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual local catalog headers: {other:?}"),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        let mut original_heads = 0;
        for raw in headers.inventory_root().headers().iter().filter(|raw|
            raw.scope.crate_key == root_scope.crate_key
                && headers.header(raw.identity).unwrap().source().generated_origin().is_none()) {
            let key = QueryKey::ImplHeadTypes(Box::new(crate::traits::header_syntax::ImplHeaderSyntaxKey {
                inventory: seal.clone(), implementation: raw.identity,
            }));
            let first = match db.fetch(&SyntaxProvider, key.clone()) {
                QueryPoll::Ready(Ok(value)) => value.value,
                other => panic!("completed original Type demand: {other:?}"),
            };
            let QueryValue::ImplHeadTypes(head) = first.as_ref() else { panic!() };
            assert!(matches!(head.result(), crate::frontend::impl_head_types::ImplHeadTypeResult::Resolved {
                trait_ref: Some(_), .. }), "actual completed annotation name remains pending: {:?}", head.result());
            let repeat = match db.fetch(&SyntaxProvider, key) {
                QueryPoll::Ready(Ok(value)) => value.value,
                other => panic!("completed original repeat: {other:?}"),
            };
            assert!(std::sync::Arc::ptr_eq(&first, &repeat));
            original_heads += 1;
        }
        assert_eq!(original_heads, 1, "actual original PartialEq impl head, not derived output");
        let mut generated = 0;
        let mut generated_definitions = Vec::new();
        for raw in headers.inventory_root().headers().iter().filter(|raw| raw.scope.crate_key == root_scope.crate_key) {
            if headers.header(raw.identity).unwrap().source().generated_origin().is_none() { continue; }
            let definition = headers.canonical_owner(raw.identity).unwrap().identity();
            assert!(matches!(crate::frontend::source_locals::cold_declarations::declaration(&mut db, definition),
                QueryPoll::Ready(Ok(None))), "an original cold input must defer to the genuine generated source service");
            generated_definitions.push(definition);
            generated += 1;
        }
        assert_eq!(generated, 2, "both actual derived declarations retain their own source route");
        db.begin_expansion_epoch(2).unwrap();
        for definition in generated_definitions {
            assert!(matches!(crate::frontend::source_locals::cold_declarations::declaration(&mut db, definition),
                QueryPoll::Ready(Ok(None))), "an unrelated stale original receipt must not block generated source selection");
        }
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
        eprintln!("completed original annotations edition{edition}: {declarations} actual local declaration rows, {annotated} annotated enum; no body preparation or guest execution");
    }
}

#[test]
fn unchanged_roots_publish_real_global_catalogs_through_common_discovery_and_header_queries() {
    use crate::{
        core::digest::StableEncode,
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            seals::SealDomain,
            value::QueryValue,
        },
    };
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("{error}"),
            QueryPoll::NeedInput(input) => panic!("actual catalog input pending: {input:?}"),
        }
    }
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut failed = Vec::new();
    let mut passed = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for case in [
            "tests/ui/traits/safety-ok.rs",
            "tests/ui/methods/method-self-arg-trait.rs",
            "tests/ui/coherence/coherence-impl-in-fn.rs",
        ] {
            let (mut db, seal, scope) =
                match configured_impl_catalog_world(&root.join(case), edition) {
                    Ok(world) => world,
                    Err(error) => {
                        eprintln!("FAILED global source catalog {case}, edition{edition}: {error}");
                        failed.push((case, edition, error.code));
                        continue;
                    }
                };
            assert!(matches!(seal.domain, SealDomain::ImplCatalog { .. }));
            let input = db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
            let InputValue::DiscoverySeal(publication) = input.as_ref() else {
                panic!()
            };
            assert!(publication.scope.is_none());
            assert!(publication.impl_catalog.is_some());
            assert!(!publication.type_scopes.is_empty(), "actual completed module Type facets are retained");
            assert!(publication.value_scopes.is_empty(), "an impl catalog does not assert a Value fixed point");
            let mut missing_facet = publication.as_ref().clone();
            missing_facet.type_scopes.clear();
            assert!(missing_facet.require_scope_certificate_layout().is_err(),
                "catalog completion binds the actual contained namespace facts");
            publication.require_scope_certificate_layout().unwrap();
            assert_eq!(
                crate::traits::upstream::check_completed_source_catalog(publication).unwrap(),
                publication.seal.fingerprint()
            );
            assert!(publication.sources.len() >= 67);
            let mut changed = publication.as_ref().clone();
            changed.sources.pop();
            assert_eq!(
                changed.require_scope_certificate_layout().unwrap_err().code,
                "LRR-E-IMPL-CATALOG-WORLD"
            );
            let mut unproved = publication.as_ref().clone();
            unproved.impl_catalog = None;
            assert_eq!(
                unproved
                    .require_scope_certificate_layout()
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
            db.trace.detailed = true;
            let start = db.trace.events.len();
            let key = QueryKey::PreparedScopeImplHeaders(seal.clone());
            let first = ready(db.fetch(&SyntaxProvider, key.clone()));
            let second = ready(db.fetch(&SyntaxProvider, key));
            assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
            let QueryValue::PreparedScopeImplHeaders(headers) = first.value.as_ref() else {
                panic!()
            };
            assert_eq!(headers.key(), &seal);
            assert!(!headers.is_empty());
            let mut original = 0;
            let mut generated = 0;
            let mut nested = 0;
            for raw in headers.inventory_root().headers() {
                let header = headers.header(raw.identity).unwrap();
                assert!(headers.canonical_owner(raw.identity).is_some());
                if raw.scope.crate_key == scope.crate_key {
                    if header.source().generated_origin().is_some() {
                        generated += 1;
                    } else {
                        original += 1;
                    }
                    if !raw.containers.is_empty() {
                        nested += 1;
                    }
                }
            }
            assert!(original > 0, "missing actual root impl: {case}");
            if case.ends_with("method-self-arg-trait.rs")
                || case.ends_with("coherence-impl-in-fn.rs")
            {
                assert!(generated > 0, "missing actual derived impl: {case}");
            }
            if case.ends_with("coherence-impl-in-fn.rs") {
                assert!(nested > 0, "missing impl inside main");
            }
            assert!(db.trace.events[start..].iter().all(|event| !matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::FunctionSignature(_)
            )));
            passed += 1;
            eprintln!(
                "global source catalog {case}, edition{edition}: {} headers; original{original}/generated{generated}/nested{nested}; no guest execution",
                headers.len()
            );
        }
    }
    eprintln!(
        "global source matrix: {passed}/12 source worlds; failed={failed:?}; no guest runtime acceptance"
    );
    assert!(
        failed.is_empty(),
        "global source catalog obligations remain: {failed:?}"
    );
}

#[test]
fn unchanged_global_catalog_impl_heads_use_actual_source_types_without_body_preparation() {
    use ra_ap_syntax::{AstNode, ast};
    use crate::{
        db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::impl_head_types::ImplHeadTypeResult,
        traits::header_syntax::ImplHeaderSyntaxKey,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout");
    let mut pending = Vec::new();
    let mut resolved = 0;
    let mut demanded = 0;
    let mut occurrences = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for case in [
            "tests/ui/traits/safety-ok.rs",
            "tests/ui/methods/method-self-arg-trait.rs",
            "tests/ui/coherence/coherence-impl-in-fn.rs",
        ] {
            let (mut db, seal, scope) = configured_impl_catalog_world(&root.join(case), edition).unwrap();
            let inventory = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone())) {
                QueryPoll::Ready(Ok(value)) => value.value,
                other => panic!("actual global inventory is not ready: {other:?}"),
            };
            let QueryValue::PreparedScopeImplHeaders(headers) = inventory.as_ref() else { panic!() };
            db.trace.detailed = true;
            let start = db.trace.events.len();
            for raw in headers.inventory_root().headers().iter()
                .filter(|header| header.scope.crate_key == scope.crate_key) {
                demanded += 1;
                let owner = headers.canonical_owner(raw.identity).unwrap().identity();
                if headers.header(raw.identity).unwrap().source().generated_origin().is_some() {
                    let context = db.input(&InputKey::DeclarationContext(owner)).unwrap();
                    let InputValue::DeclarationContext(context) = context.as_ref() else { panic!() };
                    assert_eq!(context.scope, raw.scope, "query input retains the admitted generated insertion owner");
                    let source = db.input(&InputKey::DefinitionSource(owner)).unwrap();
                    let InputValue::DefinitionSource(parse) = source.as_ref() else { panic!() };
                    let input = db.input(&InputKey::GeneratedSyntax(parse.content)).unwrap();
                    let InputValue::GeneratedSyntax(unit) = input.as_ref() else { panic!() };
                    let first = unit.outline().unwrap();
                    assert!(std::sync::Arc::ptr_eq(&first, &unit.outline().unwrap()));
                    if unit.scope.owner.is_some() {
                        assert!(crate::expand::generated_syntax::GeneratedSyntaxInput::prepare(
                            unit.items.record_root().clone(), unit.scope.clone(), unit.cfg.clone(),
                            unit.seal.clone(), None, None,
                        ).is_err(), "a raw local scope cannot mint query-input ownership");
                    }
                }
                let key = QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                    inventory: seal.clone(), implementation: raw.identity,
                }));
                let first = match db.fetch(&SyntaxProvider, key.clone()) {
                    QueryPoll::Ready(Ok(value)) => value.value,
                    other => {
                        pending.push(format!("{case} edition{edition} {:?}: {other:?}", raw.identity));
                        continue;
                    }
                };
                let second = match db.fetch(&SyntaxProvider, key) {
                    QueryPoll::Ready(Ok(value)) => value.value,
                    other => panic!("repeat actual head demand: {other:?}"),
                };
                assert!(std::sync::Arc::ptr_eq(&first, &second));
                let QueryValue::ImplHeadTypes(head) = first.as_ref() else { panic!() };
                match head.result() {
                    ImplHeadTypeResult::Resolved { self_type, trait_ref } => {
                        if let Some(trait_ref) = trait_ref {
                            assert_eq!(trait_ref.self_ty, *self_type);
                        }
                        if head.syntax().generated_origin().is_some() {
                            let header = db.input(&InputKey::DefinitionHeader(owner)).unwrap();
                            let context = db.input(&InputKey::DeclarationContext(owner)).unwrap();
                            let (InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context)) =
                                (header.as_ref(), context.as_ref()) else { panic!() };
                            let parsed = head.syntax().parsed();
                            let ty = parsed.syntax_at(head.syntax().self_type(), |node| ast::Type::can_cast(node.kind())).unwrap().unwrap();
                            let name = ty.descendants().find_map(ast::NameRef::cast).unwrap();
                            let proof = parsed.declaration_type_name(owner, header, context, &name).unwrap();
                            proof.require(&parsed.key(), owner, header, context, name.text()).unwrap();
                            let mut foreign = context.clone();
                            foreign.scope.crate_key = Digest([0; 32]);
                            assert!(proof.require(&parsed.key(), owner, header, &foreign, name.text()).is_err());
                            assert!(proof.require(&parsed.key(), Digest([0; 32]), header, context, name.text()).is_err());
                            let mut changed = header.clone();
                            changed.tokens.clear();
                            assert!(proof.require(&parsed.key(), owner, &changed, context, name.text()).is_err());
                            let mut changed = parsed.key();
                            changed.content = Digest([0; 32]);
                            assert!(proof.require(&changed, owner, header, context, name.text()).is_err());
                            occurrences += 1;
                        }
                        resolved += 1;
                        eprintln!("actual impl head {case} edition{edition} {:?}: {self_type:?}; trait={trait_ref:?}; no guest execution", raw.identity);
                    }
                    other => pending.push(format!("{case} edition{edition} {:?}: {other:?}", raw.identity)),
                }
            }
            assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
                QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
                    | QueryKey::FunctionSignature(_))));
        }
    }
    eprintln!("actual catalog head Types: {resolved}/{demanded}; pending={pending:?}; no solver or guest execution");
    eprintln!("actual generated declaration Type occurrence controls: {occurrences}; no guest execution");
    assert!(demanded > 0);
    assert!(occurrences > 0);
    assert!(pending.is_empty(), "actual source head Types remain unprepared: {pending:?}");
}

#[test]
fn global_catalog_rejects_a_foreign_prepared_source_publication_before_expansion() {
    use crate::{core::profile::ProfileIdentity, expand::source_discovery::SourceDiscovery};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    let profile = ProfileIdentity::current();
    let capture = || {
        let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(
            &path,
            false,
            profile.edition,
            &super::crate_graph::ExternCrates::default(),
            crate::source::embedded_crates::standard_facade_crates(&profile),
            None,
        )
        .unwrap();
        super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources)
            .unwrap()
    };
    let (compiler, inputs, _, _) = capture();
    let (_, _, foreign, _) = capture();
    let mut db = crate::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    match SourceDiscovery::for_impl_catalog(&compiler, &foreign, 1, &db) {
        Err(error) => assert_eq!(error.code, "LRR-E-IMPL-CATALOG-WORLD"),
        Ok(_) => panic!("foreign prepared source accepted as this configured catalog"),
    }
}

#[test]
fn global_catalog_does_not_complete_an_unknown_cold_macro_or_prepare_cold_call_bodies() {
    use crate::{
        core::profile::ProfileIdentity,
        db::{dependency::InputKey, memo::Database},
        expand::source_discovery::{SourceCompilerPreparation, SourceDiscovery},
    };
    let original =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    for (label, addition, fails) in [
        (
            "unknown",
            "\nfn cold_catalog_frontier(){ unresolved_catalog_producer!(); }\n",
            true,
        ),
        (
            "disabled",
            "\n#[cfg(any())] fn cold_catalog_frontier(){ unresolved_catalog_producer!(); }\n",
            false,
        ),
        (
            "ordinary",
            "\nfn cold_catalog_frontier(){ unentered_unknown_function(); }\n",
            false,
        ),
    ] {
        let path = temporary_source(
            &format!("impl-catalog-{label}"),
            &format!("{original}{addition}"),
        );
        let profile = ProfileIdentity::current();
        let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(
            &path,
            false,
            profile.edition,
            &super::crate_graph::ExternCrates::default(),
            crate::source::embedded_crates::standard_facade_crates(&profile),
            None,
        )
        .unwrap();
        let (compiler, inputs, source, _) =
            super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources)
                .unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let mut discovery =
            match SourceDiscovery::for_impl_catalog(&compiler, &source, 1, &db).unwrap() {
                SourceCompilerPreparation::Ready(discovery) => discovery,
                SourceCompilerPreparation::NeedModule { demand, .. } => {
                    panic!("{}", demand.diagnostic())
                }
            };
        let key = discovery.key().clone();
        let outcome = compiler.discover(&mut discovery, &mut db);
        if fails {
            assert_eq!(outcome.unwrap_err().code, "LRR-E-CATALOG-INCOMPLETE");
            assert!(discovery.publish(&mut db).is_err());
            assert!(db.input(&InputKey::DiscoverySeal(key)).is_none());
        } else {
            outcome.unwrap();
            discovery.publish(&mut db).unwrap();
            assert!(db.input(&InputKey::DiscoverySeal(key)).is_some());
        }
        assert!(
            !db.trace
                .counts
                .keys()
                .any(|label| label.starts_with("Thunk:"))
        );
    }
}

#[test]
fn global_catalog_rejects_changed_owning_compiler_input_before_publication() {
    use crate::{
        core::profile::{GUEST_TARGET, ProfileIdentity},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
        },
        expand::{
            builtins::FrozenCompileEnvironment,
            source_discovery::{SourceCompilerPreparation, SourceDiscovery},
        },
        project::tool_manifest::CompileEnv,
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    let profile = ProfileIdentity::current();
    let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(
        &path,
        false,
        profile.edition,
        &super::crate_graph::ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )
    .unwrap();
    let (compiler, inputs, source, _) =
        super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources)
            .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = match SourceDiscovery::for_impl_catalog(&compiler, &source, 1, &db).unwrap()
    {
        SourceCompilerPreparation::Ready(discovery) => discovery,
        SourceCompilerPreparation::NeedModule { demand, .. } => panic!("{}", demand.diagnostic()),
    };
    compiler.discover(&mut discovery, &mut db).unwrap();
    let unit = source.selection().outline.crate_key;
    let changed = CompileEnv {
        unit,
        host: GUEST_TARGET.into(),
        target: GUEST_TARGET.into(),
        cfg: source.captured_macro_cfg().unwrap().clone(),
        cargo: Default::default(),
        rustc_env: std::collections::BTreeMap::from([("LRR_CATALOG_CHANGED".into(), "yes".into())]),
        declared: Default::default(),
        out_dir: None,
    };
    let changed = std::sync::Arc::new(FrozenCompileEnvironment::new(unit, &changed).unwrap());
    db.publish_inputs(std::collections::BTreeMap::from([(
        InputKey::UnitCompileEnv(unit),
        InputValue::UnitCompileEnv(changed),
    )]))
    .unwrap();
    assert_eq!(
        discovery.publish(&mut db).unwrap_err().code,
        "LRR-E-IMPL-CATALOG-WORLD"
    );
    assert!(
        db.input(&InputKey::DiscoverySeal(discovery.key().clone()))
            .is_none()
    );
}

#[test]
fn unchanged_cold_local_type_uses_shared_name_input_and_observed_preparation_all_editions() {
    use ra_ap_syntax::{AstNode, ast};
    use crate::{
        core::digest::StableEncode,
        db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll,
            provider::SyntaxProvider, value::QueryValue},
        frontend::impl_head_types::ImplHeadTypeResult,
        traits::header_syntax::ImplHeaderSyntaxKey,
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/zero-sized/zero-size-type-destructors.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, discovery, root) = configured_impl_catalog_discovery(&path, edition).unwrap();
        discovery.publish(&mut db).unwrap();
        db.trace.detailed = true;
        let start = db.trace.events.len();
        let headers = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(discovery.key().clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual cold catalog demand: {other:?}"),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        let raw = headers.inventory_root().headers().iter().find(|header|
            header.scope.crate_key == root.crate_key && !header.containers.is_empty()).unwrap();
        let owner = headers.canonical_owner(raw.identity).unwrap().identity();
        let context = db.input(&InputKey::DeclarationContext(owner)).unwrap();
        let InputValue::DeclarationContext(context) = context.as_ref() else { panic!() };
        assert!(context.scope.owner.is_some());
        let input_key = InputKey::DeclarationNames(context.scope.clone());
        let input = db.input(&input_key).unwrap();
        let InputValue::DeclarationNames(prepared) = input.as_ref() else { panic!() };
        prepared.require_current(&mut db).unwrap();
        assert!(input.matches(&input_key));
        let mut wrong = context.scope.clone(); wrong.owner = Some(Digest([0; 32]));
        assert!(!input.matches(&InputKey::DeclarationNames(wrong)));
        let revision = db.revision();
        discovery.publish(&mut db).unwrap();
        let again = db.input(&input_key).unwrap();
        let InputValue::DeclarationNames(again) = again.as_ref() else { panic!() };
        assert!(std::sync::Arc::ptr_eq(prepared, again), "same actual immutable plan is reused");
        // The cold input itself is idempotent; unrelated existing publication
        // work may still advance the coordinator's revision.
        assert!(db.revision() >= revision);
        let removed = db.take_cold_declaration_names_for_control(&context.scope).unwrap();
        let head_key = QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
            inventory: discovery.key().clone(), implementation: raw.identity,
        }));
        let missing = match db.fetch(&SyntaxProvider, head_key.clone()) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual missing name input: {other:?}"),
        };
        let QueryValue::ImplHeadTypes(missing) = missing.as_ref() else { panic!() };
        assert!(matches!(missing.result(), ImplHeadTypeResult::Pending(_)));
        let before = db.revision();
        assert!(db.publish_inputs(std::collections::BTreeMap::from([
            (input_key.clone(), removed.as_ref().clone()),
        ])).is_err(), "base inputs cannot inject a derived name receipt");
        assert_eq!(db.revision(), before);
        discovery.publish(&mut db).unwrap();
        let ready = match db.fetch(&SyntaxProvider, head_key.clone()) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual restored cold name input: {other:?}"),
        };
        let QueryValue::ImplHeadTypes(head) = ready.as_ref() else { panic!() };
        assert!(matches!(head.result(), ImplHeadTypeResult::Resolved { .. }),
            "cold local Foo/Drop head remains incomplete: {:?}", head.result());
        let repeat = match db.fetch(&SyntaxProvider, head_key) {
            QueryPoll::Ready(Ok(value)) => value.value,
            other => panic!("actual retained head: {other:?}"),
        };
        assert!(std::sync::Arc::ptr_eq(&ready, &repeat));
        let syntax = head.syntax();
        let implementation = ast::Impl::cast(syntax.parsed().syntax_at(context.range,
            |node| ast::Impl::can_cast(node.kind())).unwrap().unwrap()).unwrap();
        let name = implementation.self_ty().unwrap().syntax().descendants().find_map(ast::NameRef::cast).unwrap();
        let current = db.input(&input_key).unwrap();
        let InputValue::DeclarationNames(current) = current.as_ref() else { panic!() };
        let header = db.input(&InputKey::DefinitionHeader(owner)).unwrap();
        let InputValue::DefinitionHeader(header) = header.as_ref() else { panic!() };
        assert!(matches!(prepared.lookup(&mut db, crate::frontend::type_name_namespace::SourceTypeOwner {
            definition: owner, scope: &context.scope, header,
        }, &name), QueryPoll::Ready(Err(_))), "a replaced opaque name handle cannot authorize lookup");
        assert!(matches!(current.lookup(&mut db, crate::frontend::type_name_namespace::SourceTypeOwner {
            definition: owner, scope: &context.scope, header,
        }, &name), QueryPoll::Ready(Ok(Some(crate::frontend::names::NameLookup::Defined(_))))));
        let mut wrong_header = header.clone(); wrong_header.tokens.clear();
        assert!(matches!(current.lookup(&mut db, crate::frontend::type_name_namespace::SourceTypeOwner {
            definition: owner, scope: &context.scope, header: &wrong_header,
        }, &name), QueryPoll::Ready(Err(_))));
        db.begin_expansion_epoch(2).unwrap();
        assert!(current.require_current(&mut db).is_err(), "old epoch cannot authorize cold lookup");
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
        eprintln!("cold original Type input edition{edition}: actual local Foo/Drop head resolved after observed absence; memo reused, wrong scope/header/epoch and raw publication refused; no executable preparation or guest execution");
    }
}
