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
                let identity_key = QueryKey::ImplTraitIdentity(Box::new(ImplHeaderSyntaxKey {
                    inventory: seal.clone(), implementation: raw.identity,
                }));
                let identity_start = db.trace.events.len();
                let identity = match db.fetch(&SyntaxProvider, identity_key.clone()) {
                    QueryPoll::Ready(Ok(value)) => value.value,
                    other => panic!("actual trait identity demand: {other:?}"),
                };
                let repeat = match db.fetch(&SyntaxProvider, identity_key) {
                    QueryPoll::Ready(Ok(value)) => value.value,
                    other => panic!("actual repeated trait identity demand: {other:?}"),
                };
                assert!(std::sync::Arc::ptr_eq(&identity, &repeat));
                let QueryValue::ImplTraitIdentity(identity) = identity.as_ref() else { panic!() };
                assert!(db.trace.events[identity_start..].iter().all(|event| !matches!(event.key,
                    QueryKey::ImplHeadTypes(_) | QueryKey::ImplPredicates(_) | QueryKey::BodyStructure(_)
                        | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
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
                        assert_eq!(identity.result(), match trait_ref {
                            Some(reference) => crate::frontend::impl_trait_identity::ImplTraitIdentityResult::Trait(reference.trait_key),
                            None => crate::frontend::impl_trait_identity::ImplTraitIdentityResult::Inherent,
                        }, "actual trait membership matches full head; edition{edition}, {case}");
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

#[test]
fn unchanged_generated_local_heads_observe_actual_completion_input_and_epoch_all_editions() {
    use ra_ap_syntax::{AstNode, ast};
    use crate::{
        db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll,
            provider::SyntaxProvider, value::QueryValue},
        frontend::{impl_head_types::ImplHeadTypeResult, names::NameLookup,
            type_name_namespace::SourceTypeOwner},
        traits::header_syntax::ImplHeaderSyntaxKey,
    };
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            other => panic!("actual generated local Type demand: {}", match other {
                QueryPoll::Ready(Err(error)) => error.to_string(),
                QueryPoll::NeedInput(input) => format!("{input:?}"),
                _ => unreachable!(),
            }),
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, discovery, root) = configured_impl_catalog_discovery(&path, edition).unwrap();
        discovery.publish(&mut db).unwrap();
        db.trace.detailed = true;
        let start = db.trace.events.len();
        let headers = ready(db.fetch(&SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(discovery.key().clone()))).value;
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        let mut selected = 0;
        for raw in headers.inventory_root().headers().iter().filter(|raw|
            raw.scope.crate_key == root.crate_key && raw.scope.owner.is_some()
                && headers.header(raw.identity).unwrap().source().generated_origin().is_some()) {
            let definition = headers.canonical_owner(raw.identity).unwrap().identity();
            let context = db.input(&InputKey::DeclarationContext(definition)).unwrap();
            let header = db.input(&InputKey::DefinitionHeader(definition)).unwrap();
            let (InputValue::DeclarationContext(context), InputValue::DefinitionHeader(header)) =
                (context.as_ref(), header.as_ref()) else { panic!() };
            let key = QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                inventory: discovery.key().clone(), implementation: raw.identity,
            }));
            let first = ready(db.fetch(&SyntaxProvider, key.clone())).value;
            let QueryValue::ImplHeadTypes(head) = first.as_ref() else { panic!() };
            assert!(matches!(head.result(), ImplHeadTypeResult::Resolved { .. }));
            let syntax = head.syntax();
            let implementation = ast::Impl::cast(syntax.parsed().syntax_at(context.range,
                |node| ast::Impl::can_cast(node.kind())).unwrap().unwrap()).unwrap();
            let name = implementation.self_ty().unwrap().syntax().descendants()
                .find_map(ast::NameRef::cast).unwrap();
            let owner = SourceTypeOwner { definition, scope: &context.scope, header };
            let input_key = InputKey::DeclarationNames(context.scope.clone());
            let input = db.input(&input_key).unwrap();
            let InputValue::DeclarationNames(input) = input.as_ref() else { panic!() };
            assert!(matches!(ready(input.lookup(&mut db, owner, &name)), Some(NameLookup::Defined(_))));
            let mut wrong = header.clone(); wrong.tokens.clear();
            assert!(matches!(input.lookup(&mut db, SourceTypeOwner { header: &wrong, ..owner }, &name),
                QueryPoll::Ready(Err(_))));
            let mut wrong_scope = context.scope.clone(); wrong_scope.owner = Some(Digest([0; 32]));
            assert!(matches!(input.lookup(&mut db, SourceTypeOwner { scope: &wrong_scope, ..owner }, &name),
                QueryPoll::Ready(Err(_))));
            let removed = db.take_cold_declaration_names_for_control(&context.scope).unwrap();
            let missing = ready(db.fetch(&SyntaxProvider, key.clone())).value;
            assert!(matches!(missing.as_ref(), QueryValue::ImplHeadTypes(head)
                if matches!(head.result(), ImplHeadTypeResult::Pending(_))));
            assert!(matches!(input.lookup(&mut db, owner, &name), QueryPoll::Ready(Err(_))));
            assert!(db.publish_inputs(std::collections::BTreeMap::from([
                (input_key.clone(), removed.as_ref().clone())])).is_err());
            discovery.publish(&mut db).unwrap();
            let restored = ready(db.fetch(&SyntaxProvider, key.clone())).value;
            assert!(matches!(restored.as_ref(), QueryValue::ImplHeadTypes(head)
                if matches!(head.result(), ImplHeadTypeResult::Resolved { .. })));
            assert!(std::sync::Arc::ptr_eq(&restored, &ready(db.fetch(&SyntaxProvider, key)).value));
            selected += 1;
        }
        assert_eq!(selected, 2, "both real generated local headers, edition{edition}");
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(db.fetch(&SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(discovery.key().clone())), QueryPoll::Ready(Err(_))));
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
        eprintln!("generated local heads edition{edition}: exact actual output membership, header/scope, observed absence, restoration, memo and stale epoch; no body preparation or guest execution");
    }
}

#[test]
fn unchanged_reference_impl_trait_identity_does_not_prepare_self_or_trait_arguments_all_editions() {
    use crate::{db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        frontend::impl_trait_identity::ImplTraitIdentityResult,
        traits::header_syntax::ImplHeaderSyntaxKey};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/assignability-trait.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, seal, root) = configured_impl_catalog_world(&path, edition).unwrap();
        let headers = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual reference impl inventory: {other:?}"),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        db.trace.detailed = true; let start = db.trace.events.len(); let mut demanded = 0;
        for raw in headers.inventory_root().headers().iter().filter(|raw| raw.scope.crate_key == root.crate_key) {
            let identity = match db.fetch(&SyntaxProvider, QueryKey::ImplTraitIdentity(Box::new(ImplHeaderSyntaxKey {
                inventory: seal.clone(), implementation: raw.identity,
            }))) { QueryPoll::Ready(Ok(value)) => value.value, other => panic!("reference identity demand: {other:?}") };
            let QueryValue::ImplTraitIdentity(identity) = identity.as_ref() else { panic!() };
            assert!(matches!(identity.result(), ImplTraitIdentityResult::Trait(_)));
            demanded += 1;
        }
        assert_eq!(demanded, 2, "both unchanged impls, including borrowed slice; edition{edition}");
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::ImplHeadTypes(_) | QueryKey::ImplPredicates(_) | QueryKey::TypeSchema(_)
                | QueryKey::TypeAlias(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_) | QueryKey::Constant(_))));
        eprintln!("unchanged reference/generic impl identities edition{edition}:2/2; no self/argument/predicate/body preparation");
    }
}

#[test]
fn unchanged_whole_catalog_trait_membership_is_complete_without_preparing_impl_types() {
    use crate::db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll,
        provider::SyntaxProvider, value::QueryValue};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    let (mut db, seal, _) = configured_impl_catalog_world(&path, 2024).unwrap();
    db.trace.detailed = true; let start = db.trace.events.len();
    let key = QueryKey::ImplTraitCatalog(seal.clone());
    let catalog = match db.fetch(&SyntaxProvider, key.clone()) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => panic!("actual global trait membership: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual global trait membership input: {input:?}"),
    };
    let QueryValue::ImplTraitCatalog(prepared) = catalog.as_ref() else { panic!() };
    let publication = db.input(&InputKey::DiscoverySeal(seal)).unwrap();
    let InputValue::DiscoverySeal(publication) = publication.as_ref() else { panic!() };
    eprintln!("complete raw catalog:{}; pending trait memberships:{}; no typed impl preparation", publication.declaration_inventory.headers().len(), prepared.pending().len());
    assert!(prepared.pending().is_empty(), "actual whole-source memberships remain pending: {:?}", prepared.pending());
    assert!(prepared.matches_catalog(publication));
    let again = match db.fetch(&SyntaxProvider, key) { QueryPoll::Ready(Ok(value)) => value.value, _ => panic!() };
    assert!(std::sync::Arc::ptr_eq(&catalog, &again));
    assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
        QueryKey::ImplHeadTypes(_) | QueryKey::ImplPredicates(_) | QueryKey::BodyStructure(_)
            | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
    db.begin_expansion_epoch(2).unwrap();
    assert!(matches!(db.fetch(&SyntaxProvider, QueryKey::ImplTraitCatalog(prepared.key().clone())), QueryPoll::Ready(Err(_))));
}

#[test]
fn unresolved_trait_membership_cannot_certify_an_empty_candidate_bucket() {
    use crate::db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll,
        provider::SyntaxProvider, value::QueryValue};
    let original = include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    let path = temporary_source("pending-trait-membership", &format!("{original}\nimpl LrrUnresolvedTrait for i32 {{}}\n"));
    let (mut db, seal, _) = configured_impl_catalog_world(&path, 2024).unwrap();
    let catalog = match db.fetch(&SyntaxProvider, QueryKey::ImplTraitCatalog(seal.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => panic!("actual pending trait membership: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual pending membership input: {input:?}"),
    };
    let QueryValue::ImplTraitCatalog(catalog) = catalog.as_ref() else { panic!() };
    let publication = db.input(&InputKey::DiscoverySeal(seal)).unwrap();
    let InputValue::DiscoverySeal(publication) = publication.as_ref() else { panic!() };
    assert!(!catalog.pending().is_empty());
    assert!(catalog.candidates(Digest([0; 32])).is_none(), "unknown membership never proves absence");
    assert!(!catalog.matches_catalog(publication));
}

#[test]
fn unchanged_single_trait_translation_requires_its_actual_bucket_not_unrelated_impl_types_all_editions() {
    use crate::{db::{dependency::{InputKey, InputValue}, key::QueryKey, poll::QueryPoll,
        provider::SyntaxProvider, value::QueryValue}, frontend::impl_trait_identity::ImplTraitIdentityResult};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    let foreign_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    let (mut foreign_db, foreign_seal, _) = configured_impl_catalog_world(&foreign_path, 2024).unwrap();
    let foreign = foreign_db.input(&InputKey::DiscoverySeal(foreign_seal)).unwrap();
    let InputValue::DiscoverySeal(foreign) = foreign.as_ref() else { panic!() };
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, seal, root) = configured_impl_catalog_world(&path, edition).unwrap();
        db.trace.detailed = true; let start = db.trace.events.len();
        let headers = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual trait source headers: {other:?}"),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        let catalog = match db.fetch(&SyntaxProvider, QueryKey::ImplTraitCatalog(seal.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual single trait catalog: {other:?}"),
        };
        let QueryValue::ImplTraitCatalog(catalog) = catalog.as_ref() else { panic!() };
        let publication = db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
        let InputValue::DiscoverySeal(publication) = publication.as_ref() else { panic!() };
        assert!(catalog.pending().is_empty());
        let raw = headers.inventory_root().headers().iter().find(|raw| raw.scope.crate_key == root.crate_key).unwrap();
        let classified = catalog.classification(raw.identity).unwrap();
        let ImplTraitIdentityResult::Trait(trait_id) = classified.result() else { panic!() };
        let candidates = catalog.candidates(trait_id).unwrap();
        assert_eq!(candidates.len(), 1, "unchanged local Foo trait has one actual impl");
        assert!(publication.declaration_inventory.headers().len() > candidates.len());
        let mut translated = candidates.iter().copied().collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(catalog.require_translation(publication, trait_id, &translated).unwrap(), candidates);
        let canonical = translated.insert(raw.identity, Digest([0; 32])).unwrap();
        assert!(catalog.require_translation(publication, trait_id, &translated).is_err());
        translated.clear();
        assert!(catalog.require_translation(publication, trait_id, &translated).is_err());
        translated.insert(raw.identity, canonical);
        assert_eq!(catalog.require_translation(publication, trait_id, &translated).unwrap(), candidates);
        assert!(catalog.require_translation(foreign, trait_id, &translated).is_err());
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::ImplHeadTypes(_) | QueryKey::ImplPredicates(_) | QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
        eprintln!("actual Foo candidate publication edition{edition}:1/{}source declarations required in the demanded typed bucket; missing/foreign identity and wrong world refused; no typed solver context or guest execution", publication.declaration_inventory.headers().len());
    }
}

#[test]
fn unchanged_impl_source_terms_construct_actual_shared_ir_in_source_contexts_all_editions() {
    use crate::db::key::QueryKey;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut encoded = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for case in ["tests/ui/traits/safety-ok.rs", "tests/ui/methods/method-self-arg-trait.rs",
            "tests/ui/coherence/coherence-impl-in-fn.rs", "tests/ui/methods/method-two-trait-defer-resolution-2.rs",
            "tests/ui/traits/multidispatch1.rs",
            "tests/ui/generics/issue-61631-default-type-param-can-reference-self-in-trait.rs", "tests/ui/traits/issue-22019.rs"] {
            let original = root.join(case);
            let text = std::fs::read_to_string(&original).unwrap();
            let mut paths = vec![original];
            if case.ends_with("method-two-trait-defer-resolution-2.rs") || case.ends_with("multidispatch1.rs") || case.ends_with("issue-61631-default-type-param-can-reference-self-in-trait.rs") || case.ends_with("issue-22019.rs") {
                // Crate configuration belongs outside an item-emitting macro.
                // Only the temporary adaptation removes it; original stays unchanged.
                let text = text.replace("#![crate_type=\"lib\"]", "").replace("#![allow(missing_copy_implementations)]", "").replace("#![allow(unused_variables)]", "");
                paths.push(temporary_source(&format!("shared-types-user-{edition}"),
                    &format!("macro_rules! emit_types {{($($t:tt)*)=>{{$($t)*}}}} emit_types!{{{text}}}")));
                paths.push(temporary_source(&format!("shared-types-wrap-{edition}"),
                    &format!("macro_rules! emit_types {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap_types {{($($t:tt)*)=>{{emit_types!{{$($t)*}}}}}} wrap_types!{{{text}}}")));
            }
            for (origin, path) in paths.iter().enumerate() {
            let (mut db, seal, scope) = configured_impl_catalog_world(path, edition).unwrap();
            db.trace.detailed = true; let start = db.trace.events.len();
            let count = crate::traits::upstream::source_terms::check_actual_encodings(&mut db, &seal, scope.crate_key)
                .unwrap_or_else(|error| panic!("actual shared source encoding {case},edition{edition}: {error:?}"));
            assert!(count > 0); encoded += count;
            assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
                QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
            eprintln!("actual guarded shared IR {case},edition{edition},origin{origin}: {count}source heads; intern reuse, canonical/parameter/TraitRef identity and stale/foreign context guards; no solver or guest execution");
            }
        }
    }
    eprintln!("actual shared source heads encoded:{encoded}; no typed source-fact publication or upstream solving");
}

#[test]
fn unchanged_root_solver_settings_use_source_limits_features_and_observed_cfg_all_editions() {
    use crate::{db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        traits::upstream::configuration::SolverConfigurationKey};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    for edition in [2015, 2018, 2021, 2024] {
        for (case, limit, feature) in [
            ("tests/ui/traits/safety-ok.rs",128,false),
            ("tests/ui/traits/method-on-deeply-nested-struct-perf.rs",1024,false),
            ("tests/ui/const-generics/ensure_is_evaluatable.rs",128,true),
        ] {
            let path = root.join(case);
            let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(&path, false, edition,
                &super::crate_graph::ExternCrates::default(), crate::source::embedded_crates::standard_facade_crates(
                    &crate::core::profile::ProfileIdentity { edition, ..crate::core::profile::ProfileIdentity::current() }), None).unwrap();
            let (_, inputs, source, _) = super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources).unwrap();
            let mut db = crate::db::memo::Database::default(); inputs.publish(&mut db).unwrap(); db.trace.detailed = true;
            let key = SolverConfigurationKey::from_source(source.selection());
            let first = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(key.clone()))) {
                QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual source settings: {other:?}"),
            };
            let QueryValue::SolverConfiguration(config) = first.as_ref() else { panic!() };
            assert_eq!(config.recursion_limit(), limit); assert_eq!(config.features().contains("generic_const_exprs"), feature);
            let second = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(key.clone()))) {
                QueryPoll::Ready(Ok(value)) => value.value, _ => panic!(),
            };
            assert!(std::sync::Arc::ptr_eq(&first, &second));
            let mut wrong = key.clone(); wrong.source.parse.content = Digest([0;32]);
            assert!(matches!(db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(wrong))), QueryPoll::Ready(Err(_))));
            let mut wrong = key; wrong.source.cfg = Digest([0;32]);
            assert!(matches!(db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(wrong))), QueryPoll::Ready(Err(_))));
            assert!(db.trace.events.iter().all(|event| !matches!(event.key,
                QueryKey::ImplHeadTypes(_) | QueryKey::ImplPredicates(_) | QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
            eprintln!("actual root settings {case},edition{edition}:limit{limit},generic_const_exprs{feature}; source-only");
        }
    }
}

#[test]
fn unchanged_unsafe_function_alias_requires_full_source_signature_before_shared_ir() {
    use crate::db::key::QueryKey;
    let original = include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/fn-ptr-comparisons-some.rs");
    for edition in [2015, 2018, 2021, 2024] {
        // Original remains unchanged; the appended own trait tests representation
        // admission, not compiletest diagnostics or original main execution.
        let path = temporary_source(&format!("erased-function-alias-{edition}"),
            &format!("{original}\ntrait LrrAliasSignatureWitness {{}} impl LrrAliasSignatureWitness for FnPtr {{}}\n"));
        let (mut db, seal, scope) = configured_impl_catalog_world(&path, edition).unwrap();
        db.trace.detailed = true; let start = db.trace.events.len();
        crate::traits::upstream::source_terms::check_erased_function_pending(&mut db, &seal, scope.crate_key).unwrap();
        assert!(db.trace.events[start..].iter().all(|event| !matches!(event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::FunctionSignature(_))));
    }
}

#[test]
fn root_solver_configuration_uses_common_cfg_attr_without_static_feature_validation() {
    use crate::{db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
        traits::upstream::configuration::SolverConfigurationKey};
    let original = include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/safety-ok.rs");
    let path = temporary_source("solver-cfg-attributes", &format!("#![cfg_attr(all(), feature(generic_const_exprs))]\n#![cfg_attr(any(), feature(gca_const_items))]\n#![cfg_attr(all(), recursion_limit = r#\"256\"#)]\n#![feature(lrr_unrecognized_source_feature)]\n{original}"));
    let ((inputs, source, sources, _), _) = super::crate_graph::capture_with(&path, false, 2024,
        &super::crate_graph::ExternCrates::default(), crate::source::embedded_crates::standard_facade_crates(
            &crate::core::profile::ProfileIdentity::current()), None).unwrap();
    let (_, inputs, source, _) = super::compile_inputs::PreparedCompileInputs::prepare(&path, inputs, source, sources).unwrap();
    let mut db = crate::db::memo::Database::default(); inputs.publish(&mut db).unwrap();
    let prepared = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(source.selection())))) {
        QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual cfg_attr settings: {other:?}"),
    };
    let QueryValue::SolverConfiguration(prepared) = prepared.as_ref() else { panic!() };
    assert_eq!(prepared.recursion_limit(), 256);
    assert!(prepared.features().contains("generic_const_exprs"));
    assert!(!prepared.features().contains("gca_const_items"));
    assert!(prepared.features().contains("lrr_unrecognized_source_feature"));
}

#[test]
fn unchanged_const_region_declarations_supply_actual_typed_generic_facts_all_editions() {
    use crate::db::key::QueryKey;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut constants = 0; let mut regions = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, names, methods) in [
            ("tests/ui/const-generics/type-after-const-ok.rs", &["A"][..], false),
            ("tests/ui/const-generics/generic_const_exprs/unop.rs", &["Foo", "test"][..], false),
            ("tests/ui/const-generics/type-dependent/issue-61936.rs", &["ArrayWindowsExample", "SliceExt"][..], true),
        ] {
            let mut paths = vec![root.join(case)];
            if methods {
                let original = std::fs::read_to_string(&paths[0]).unwrap();
                paths.push(temporary_source(&format!("const-region-emit-{edition}"),
                    &format!("macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{original}}}")));
                paths.push(temporary_source(&format!("const-region-wrap-{edition}"),
                    &format!("macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{original}}}")));
            }
            for (origin, path) in paths.iter().enumerate() {
            let (mut db, seal, scope) = configured_impl_catalog_world(path, edition).unwrap();
            db.trace.detailed = true; let before = db.trace.events.len();
            let (rows, c, r) = crate::traits::upstream::source_generics::check_actual_declarations(&mut db, &seal, scope.crate_key, names, methods)
                .unwrap_or_else(|error| panic!("actual typed generics {case},edition{edition}: {error:?}"));
            assert!(rows > 0); constants += c; regions += r;
            assert!(db.trace.events[before..].iter().all(|event| !matches!(event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_))));
            eprintln!("actual generic Type facts {case},edition{edition},origin{origin}: {rows}rows/{c}consts/{r}regions; original main unexecuted");
            }
        }
    }
    assert!(constants > 0 && regions > 0);
}

#[test]
fn unchanged_opaque_return_headers_retain_actual_source_and_cold_bodies_all_editions() {
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    };
    use ra_ap_syntax::{AstNode, ast::HasTypeBounds};
    fn ready<T: std::fmt::Debug>(poll: QueryPoll<T>) -> T {
        match poll { QueryPoll::Ready(Ok(value)) => value, other => panic!("actual declaration query: {other:?}") }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut sites = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, targets) in [
            ("tests/ui/impl-trait/nesting.rs", &[ ("foo",1usize), ("bar",1) ][..]),
            ("tests/ui/impl-trait/impl_fn_associativity.rs", &[ ("f_debug",2usize), ("ff_debug",3), ("multi",2) ][..]),
        ] {
            let path=root.join(case);let original=std::fs::read_to_string(&path).unwrap();
            let profile=ProfileIdentity {edition,..ProfileIdentity::current()};
            let ((inputs,view,_,_),_)=super::crate_graph::capture_with(&path,false,edition,
                &super::crate_graph::ExternCrates::default(),
                crate::source::embedded_crates::standard_facade_crates(&profile),None).unwrap();
            let mut db=Database::default();inputs.publish(&mut db).unwrap();db.trace.detailed=true;
            for &(name,count) in targets {
                let item=view.outline().items.iter().find(|item|item.def_key.declared_name==name).unwrap();
                let definition=item.def_key.fingerprint();
                let value=ready(db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(definition))).value;
                let QueryValue::DeclarationGenerics(row)=value.as_ref() else {panic!()};
                let plan=row.opaque_returns().expect("real return Type");
                assert_eq!(plan.definition(),definition);assert_eq!(plan.sites().len(),count);
                assert!(plan.macro_frontiers().is_empty());
                for (index,site) in plan.sites().iter().enumerate() {
                    assert_eq!(site.position(),index as u32);
                    assert_eq!(site.parent(),index.checked_sub(1).map(|i|i as u32));
                    assert_eq!(site.edition(),edition);
                    let syntax=row.opaque_return_syntax(site.position()).unwrap().unwrap();
                    assert_eq!(crate::syntax::ast_adapter::range(syntax.syntax().text_range()),site.range());
                    let bounds=syntax.type_bound_list().unwrap();
                    assert_eq!(Some(crate::syntax::ast_adapter::range(bounds.syntax().text_range())),site.bounds_range());
                    assert!(!bounds.syntax().text().to_string().is_empty());sites+=1;
                }
                let reused=ready(db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(definition))).value;
                assert!(std::sync::Arc::ptr_eq(&value,&reused));
                crate::traits::upstream::check_opaque_return_receipt(&mut db,definition,&value,None);
                if name=="foo" {
                    // Adapt only an external temporary copy of the real source.
                    // Inventory the unexpanded return Type macro, not a false
                    // empty opaque set. No macro/guest code is executed here.
                    let ret=plan.result_range();let a=ret.start as usize;let b=ret.end as usize;
                    let mut wrapped=original.clone();
                    wrapped.replace_range(a..b,&format!("opaque_type_passthrough!{{{}}}",&original[a..b]));
                    let (_,_,context,_)=row.source_parts().unwrap();
                    wrapped.insert_str(context.range.start as usize,"macro_rules! opaque_type_passthrough {($t:ty)=>{$t}}\n");
                    let altered=temporary_source(&format!("opaque-type-frontier-{edition}"),&wrapped);
                    let ((inputs,view,_,_),_)=super::crate_graph::capture_with(&altered,false,edition,
                        &super::crate_graph::ExternCrates::default(),crate::source::embedded_crates::standard_facade_crates(&profile),None).unwrap();
                    let mut frontier_db=Database::default();inputs.publish(&mut frontier_db).unwrap();
                    let item=view.outline().items.iter().find(|i|i.def_key.declared_name==name).unwrap();
                    let frontier=ready(frontier_db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(item.def_key.fingerprint()))).value;
                    let QueryValue::DeclarationGenerics(frontier)=frontier.as_ref() else {panic!()};
                    let frontier=frontier.opaque_returns().unwrap();
                    assert!(frontier.sites().is_empty());assert_eq!(frontier.macro_frontiers().len(),1);
                }
            }
            assert!(db.trace.events.iter().all(|event| !matches!(event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::SourceLocalPlan(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_))));
        }
    }
    assert_eq!(sites,36);
}

#[test]
fn generated_opaque_headers_share_declaration_query_and_refresh_actual_source_receipts() {
    use crate::{core::{digest::StableEncode,profile::ProfileIdentity},
        db::{dependency::{InputKey,InputValue},key::QueryKey,memo::Database,poll::QueryPoll,provider::SyntaxProvider,value::QueryValue}};
    use ra_ap_syntax::AstNode;
    fn ready<T: std::fmt::Debug>(poll:QueryPoll<T>)->T {match poll {QueryPoll::Ready(Ok(value))=>value,other=>panic!("actual opaque source query: {other:?}")}}
    let original=std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout/tests/ui/impl-trait/nesting.rs")).unwrap();
    // Keep crate attributes/directives outside the macro; emit the exact
    // original function declarations/bodies through normal source admission.
    let offset=original.find("fn foo").unwrap();let prefix=&original[..offset];let declarations=&original[offset..];
    let mut worlds=0;
    for edition in [2015,2018,2021,2024] {
        for wrapper in [false,true] {
            let setup=if wrapper {"macro_rules! outer {($($t:tt)*)=>{emit!{$($t)*}}}"}else{""};
            let invoke=if wrapper {"outer"}else{"emit"};
            let source=format!("{prefix}macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} {setup} {invoke}!{{{declarations}}}");
            let path=temporary_source(&format!("opaque-emit-{edition}-{wrapper}"),&source);
            let (mut db,seal,scope)=configured_impl_catalog_world(&path,edition).unwrap();
            let input=db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
            let InputValue::DiscoverySeal(catalog)=input.as_ref() else {panic!()};
            let headers=ready(db.fetch(&SyntaxProvider,QueryKey::PreparedScopeImplHeaders(seal.clone()))).value;
            let QueryValue::PreparedScopeImplHeaders(headers)=headers.as_ref()else{panic!()};
            let names=headers.inventory_root().environment().unwrap().names();
            let definitions=["foo","bar"].into_iter().map(|name| {
                let lookup=names.lookup_source_path(crate::source::manifest::Namespace::Value,
                    &[crate::syntax::imports::ImportSegment::Name(name.into())],false,&scope,edition).unwrap();
                let crate::frontend::names::NameLookup::Defined(found)=lookup else {panic!("actual generated function {name}: {lookup:?}")};
                (found.identity,name)
            }).collect::<Vec<_>>();
            assert_eq!(definitions.len(),2);db.trace.detailed=true;let start=db.trace.events.len();
            for (definition,name) in definitions {
                let value=ready(db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(definition))).value;
                let QueryValue::DeclarationGenerics(row)=value.as_ref() else {panic!()};
                let plan=row.opaque_returns().unwrap();assert_eq!(plan.sites().len(),1,"{name}");assert!(plan.macro_frontiers().is_empty());
                assert_eq!(plan.sites()[0].edition(),edition);assert!(row.opaque_return_syntax(0).unwrap().is_some());
                crate::traits::upstream::check_opaque_return_receipt(&mut db,definition,&value,None);
            }
            assert!(db.trace.events[start..].iter().all(|event|!matches!(event.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_)|QueryKey::PatternPlan(_))));worlds+=1;
        }
        // Re-publish genuine source input snapshots, retaining the old immutable
        // plan. The body-only comment shifts bar's actual source locations.
        let path=temporary_source(&format!("opaque-refresh-{edition}"),&original);
        let profile=ProfileIdentity{edition,..ProfileIdentity::current()};
        let capture=||super::crate_graph::capture_with(&path,false,edition,&super::crate_graph::ExternCrates::default(),crate::source::embedded_crates::standard_facade_crates(&profile),None).unwrap().0;
        let (inputs,view,_,_)=capture();let mut db=Database::default();inputs.publish(&mut db).unwrap();
        let def=view.outline().items.iter().find(|i|i.def_key.declared_name=="bar").unwrap().def_key.fingerprint();
        let old=ready(db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(def))).value;
        std::fs::write(&path,original.replacen("[t]","/* retained source refresh */ [t]",1)).unwrap();
        let (inputs,view,_,_)=capture();inputs.publish(&mut db).unwrap();
        let def=view.outline().items.iter().find(|i|i.def_key.declared_name=="bar").unwrap().def_key.fingerprint();
        let new=ready(db.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(def))).value;
        assert!(!std::sync::Arc::ptr_eq(&old,&new));
        let (QueryValue::DeclarationGenerics(old_row),QueryValue::DeclarationGenerics(new_row))=(old.as_ref(),new.as_ref())else{panic!()};
        assert_ne!(old_row.source_parts().unwrap().0,new_row.source_parts().unwrap().0);
        assert_ne!(old_row.opaque_returns().unwrap().sites()[0].range(),new_row.opaque_returns().unwrap().sites()[0].range());
        assert_eq!(old_row.opaque_return_syntax(0).unwrap().unwrap().syntax().text().to_string(),new_row.opaque_return_syntax(0).unwrap().unwrap().syntax().text().to_string());
        crate::traits::upstream::check_opaque_return_receipt(&mut db,def,&new,Some(&old));
        let mut absent=Database::default();let absent=ready(absent.fetch(&SyntaxProvider,QueryKey::DeclarationGenerics(def))).value;
        let QueryValue::DeclarationGenerics(absent)=absent.as_ref()else{panic!()};
        assert!(absent.source_parts().is_none());assert!(absent.opaque_returns().is_none());
    }
    assert_eq!(worlds,8);
}

#[test]
fn unchanged_conditional_impl_requirements_construct_actual_upstream_clauses_all_editions_and_origins() {
    use crate::{db::key::QueryKey, traits::{predicate::PredicatePending,
        upstream::source_predicates::{check_actual_clauses, SourcePredicatePending}}};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut worlds = 0; let mut clauses = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, marker, expected) in [
            ("tests/ui/methods/method-two-trait-defer-resolution-2.rs", "\ntrait Foo", [3usize,2,2,0,0,0,0]),
            ("tests/ui/traits/where-clause-vs-impl.rs", "\nuse std::rc", [2,6,6,0,0,0,0]),
            ("tests/ui/traits/static-outlives-a-where-clause.rs", "\ntrait Foo", [2,2,1,1,0,0,1]),
            ("tests/ui/self/self-as-trait-call-in-method-body.rs", "\npub trait Foo", [1,2,1,0,1,0,0]),
            ("tests/ui/traits/principal-less-objects.rs", "\nuse std::sync", [2,0,0,0,0,1,0]),
        ] {
            let original = root.join(case); let text = std::fs::read_to_string(&original).unwrap();
            let at = text.find(marker).unwrap(); let (prefix, items) = text.split_at(at);
            let mut paths = vec![original];
            for wrapper in [false,true] {
                let setup = if wrapper { "macro_rules! outer {($($t:tt)*)=>{emit!{$($t)*}}}" } else { "" };
                let invoke = if wrapper { "outer" } else { "emit" };
                paths.push(temporary_source(&format!("impl-clauses-{edition}-{wrapper}-{}",case.replace('/',"-")),
                    &format!("{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} {setup} {invoke}!{{{items}}}")));
            }
            for (origin,path) in paths.iter().enumerate() {
                let (mut db,seal,root) = configured_impl_catalog_world(path,edition).unwrap();
                db.trace.detailed=true; let start=db.trace.events.len();
                let actual = check_actual_clauses(&mut db,&seal,root.crate_key)
                    .unwrap_or_else(|e|panic!("{case},edition{edition},origin{origin}: {e:?}"));
                let counts=[actual.implementations,actual.clauses,actual.traits,actual.regions,actual.types,actual.relaxations,actual.pending.len()];
                assert_eq!(counts,expected,"{case},edition{edition},origin{origin}: {actual:?}");
                assert!(actual.pending.iter().all(|p| *p==SourcePredicatePending::Bound(PredicatePending::HigherRanked)),
                    "preserve the actual unresolved higher-ranked bound, not a source-only exclusion");
                assert!(db.trace.events[start..].iter().all(|e| !matches!(e.key,
                    QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_))));
                eprintln!("actual source clauses {case},edition{edition},origin{origin}: {actual:?}; no root/guest execution");
                worlds+=1; clauses+=actual.clauses;
            }
        }
    }
    assert_eq!(worlds,60); assert_eq!(clauses,144);
}

#[test]
fn unchanged_lifetime_aliases_use_retained_target_ast_and_mixed_substitution_all_editions_and_origins() {
    use crate::{db::{key::QueryKey,poll::QueryPoll,provider::SyntaxProvider,value::QueryValue},
        source::manifest::Namespace,syntax::imports::ImportSegment,traits::term::{TyTerm,RegionTerm}};
    let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout/tests/ui/traits/issue-22019.rs");
    let text=std::fs::read_to_string(&path).unwrap();let at=text.find("\npub struct CFGNode").unwrap();let (prefix,items)=text.split_at(at);
    let mut worlds=0;
    for edition in [2015,2018,2021,2024] {
        let mut paths=vec![path.clone()];
        for wrapper in [false,true] {
            let setup=if wrapper {"macro_rules! outer {($($t:tt)*)=>{emit!{$($t)*}}}"}else{""};let invoke=if wrapper {"outer"}else{"emit"};
            paths.push(temporary_source(&format!("retained-alias-{edition}-{wrapper}"),&format!("{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} {setup} {invoke}!{{{items}}}")));
        }
        for (origin,path) in paths.iter().enumerate() {
            let (mut db,seal,scope)=configured_impl_catalog_world(path,edition).unwrap();
            let headers=match db.fetch(&SyntaxProvider,QueryKey::PreparedScopeImplHeaders(seal)) {QueryPoll::Ready(Ok(v))=>v.value,other=>panic!("actual alias catalog: {other:?}")};
            let QueryValue::PreparedScopeImplHeaders(headers)=headers.as_ref()else{panic!()};
            let environment=headers.inventory_root().environment().unwrap();
            let lookup=environment.names().lookup_source_path(Namespace::Type,&[ImportSegment::Name("Node".into())],false,&scope,edition).unwrap();
            let crate::frontend::names::NameLookup::Defined(alias)=lookup else{panic!("actual source alias: {lookup:?}")};
            let sources=environment.sources().iter().collect::<Vec<_>>();db.trace.detailed=true;let start=db.trace.events.len();
            let (explicit,omitted)=crate::frontend::source_types::check_retained_alias_arguments(&mut db,environment.names(),&sources,alias.identity).unwrap();
            let (TyTerm::Ref(RegionTerm::Static,false,explicit_target),TyTerm::Ref(RegionTerm::Local{definition,start:slot_start,end:slot_end},false,omitted_target))=(explicit,omitted)else{panic!("actual alias region transport remains unverified")};
            assert_eq!(definition,alias.identity);assert!(slot_start<slot_end);assert_eq!(explicit_target,omitted_target);assert!(matches!(*explicit_target,TyTerm::Nominal(_,ref args)if args.is_empty()));
            assert!(db.trace.events[start..].iter().all(|e|!matches!(e.key,QueryKey::FunctionSignature(_)|QueryKey::BodyStructure(_)|QueryKey::Thunk(_)|QueryKey::PatternPlan(_))));
            eprintln!("retained actual Node alias,edition{edition},origin{origin}: explicit static and held local elision; no synthetic signature/target parse/guest entry");worlds+=1;
        }
    }
    assert_eq!(worlds,12);
}

#[test]
fn unchanged_body_and_module_aliases_share_actual_normalization_all_editions_and_origins() {
    use crate::{core::digest::StableEncode,db::{dependency::{InputKey,InputValue},key::QueryKey,poll::QueryPoll,provider::SyntaxProvider,value::QueryValue},
        frontend::{source_types::NamedTypePreparation,type_aliases::AliasTarget},source::manifest::{ItemKind,Namespace},
        syntax::imports::ImportSegment,traits::term::{TyTerm,GenericArg}};
    fn ready<T:std::fmt::Debug>(p:QueryPoll<T>)->T {match p{QueryPoll::Ready(Ok(v))=>v,other=>panic!("real alias source: {other:?}")}}
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");let mut aliases=0;
    for edition in [2015,2018,2021,2024] {
        for (case,marker,local) in [
            ("tests/ui/type-alias/static-method-type-alias-11047.rs","\npub mod foo",true),
            ("tests/ui/structs-enums/struct-aliases.rs","\nuse std::mem",false),
        ] {
            let original=root.join(case);let text=std::fs::read_to_string(&original).unwrap();let at=text.find(marker).unwrap();let (prefix,items)=text.split_at(at);let mut paths=vec![original];
            for wrapper in [false,true] {let setup=if wrapper{"macro_rules! outer {($($t:tt)*)=>{emit!{$($t)*}}}"}else{""};let invoke=if wrapper{"outer"}else{"emit"};paths.push(temporary_source(&format!("alias-normalize-{edition}-{wrapper}-{local}"),&format!("{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} {setup} {invoke}!{{{items}}}")));}
            for (origin,path) in paths.iter().enumerate() {
                let (mut db,discovery,scope)=configured_impl_catalog_discovery(path,edition).unwrap();
                discovery.publish(&mut db).unwrap();let seal=discovery.key().clone();
                let headers=ready(db.fetch(&SyntaxProvider,QueryKey::PreparedScopeImplHeaders(seal))).value;
                let QueryValue::PreparedScopeImplHeaders(headers)=headers.as_ref()else{panic!()};let environment=headers.inventory_root().environment().unwrap();let sources=environment.sources().iter().collect::<Vec<_>>();
                let mut selected=Vec::new();
                if local {
                    let lookup=environment.names().lookup_source_path(Namespace::Value,&[ImportSegment::Name("main".into())],false,&scope,edition).unwrap();
                    let crate::frontend::names::NameLookup::Defined(main)=lookup else {panic!("actual main source lookup absent")};
                    let body=db.input(&InputKey::DefinitionBody(main.identity)).unwrap();let InputValue::DefinitionBody(body)=body.as_ref() else {panic!("actual main body absent")};
                    let plan=ready(db.prepare_source_local_plan(&discovery,&sources,&body.key()));
                    for row in &plan.context().facts().items {if row.item.def_key.item_kind==ItemKind::TypeAlias {selected.push((row.item.def_key.declared_name.clone(),row.item.def_key.fingerprint()));}}
                } else {
                    for name in ["S2","S4"] {let lookup=environment.names().lookup_source_path(Namespace::Type,&[ImportSegment::Name(name.into())],false,&scope,edition).unwrap();let crate::frontend::names::NameLookup::Defined(found)=lookup else{panic!("actual alias: {lookup:?}")};selected.push((name.into(),found.identity));}
                }
                let target_identity=|path:&[&str]| {
                    let segments=path.iter().map(|name|ImportSegment::Name((*name).into())).collect::<Vec<_>>();
                    let lookup=environment.names().lookup_source_path(Namespace::Type,&segments,false,&scope,edition).unwrap();
                    let crate::frontend::names::NameLookup::Defined(found)=lookup else {panic!("actual target source lookup: {lookup:?}")};found.identity
                };
                let expected=if local {vec![target_identity(&["foo","bar","baz","Qux"]),target_identity(&["Vec"]),target_identity(&["String"])]}
                    else {vec![target_identity(&["S"]),target_identity(&["S3"])]};
                assert_eq!(selected.len(),2);db.trace.detailed=true;let start=db.trace.events.len();
                for (name,identity) in &selected {
                    assert!(matches!(db.input(&InputKey::DefinitionHeader(*identity)).as_deref(),Some(InputValue::DefinitionHeader(_))),"canonical alias publisher must supply the exact identity");
                    let (result,explicit)=ready(crate::frontend::source_types::check_selected_alias_definition(&mut db,environment.names(),&sources,*identity));
                    match (name.as_str(),result,explicit) {
                        ("Ham",NamedTypePreparation::Alias(AliasTarget::Type(TyTerm::Nominal(target,args))),None)=>{assert_eq!(target,expected[0]);assert!(args.is_empty());},
                        ("StringVec",NamedTypePreparation::Alias(AliasTarget::Type(TyTerm::Nominal(target,args))),None)=>{assert_eq!(target,expected[1]);assert_eq!(args,vec![GenericArg::Type(TyTerm::Nominal(expected[2],vec![]))]);},
                        ("S2",NamedTypePreparation::Ready(source),None)=>assert_eq!(source.source(),&TyTerm::Nominal(expected[0],vec![])),
                        ("S4",NamedTypePreparation::Alias(AliasTarget::Deferred{definition,..}),Some(TyTerm::Nominal(target,args)))=>{assert_eq!(definition,*identity);assert_eq!(target,expected[1]);assert_eq!(args,vec![GenericArg::Type(TyTerm::Primitive(crate::ir::op::PrimitiveType::U16)),GenericArg::Type(TyTerm::Primitive(crate::ir::op::PrimitiveType::Char))]);},
                        (_,result,explicit)=>panic!("{case},edition{edition},origin{origin},{name}: {result:?};{explicit:?}"),
                    }
                    aliases+=1;
                }
                assert!(db.trace.events[start..].iter().all(|e|!matches!(e.key,QueryKey::BodyStructure(_)|QueryKey::FunctionSignature(_)|QueryKey::Thunk(_)|QueryKey::PatternPlan(_))));
                let saved=selected[0].1;db.publish_inputs(std::collections::BTreeMap::new()).unwrap();assert!(matches!(crate::frontend::source_types::check_selected_alias_definition(&mut db,environment.names(),&sources,saved),QueryPoll::Ready(Err(_))),"revoked real source must not normalize from retained metadata alone");
                eprintln!("actual alias targets {case},edition{edition},origin{origin}:2; no additional body preparation/guest execution");
            }
        }
    }
    assert_eq!(aliases,48);
}
