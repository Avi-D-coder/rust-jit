//! Real common source-goal joins, not interpreter/full-fixture acceptance.
use super::*;
use crate::{
    frontend::binding_constraints::BindingConstraintState, ir::op::PrimitiveType,
    syntax::source_constraints::FactKind,
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
const RADIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
);
#[test]
fn unchanged_suffix_shadowed_bindings_use_actual_later_parameter_source_goals() {
    let expected = [
        PrimitiveType::I8,
        PrimitiveType::I16,
        PrimitiveType::I32,
        PrimitiveType::I64,
        PrimitiveType::Usize,
        PrimitiveType::U8,
        PrimitiveType::U16,
        PrimitiveType::U32,
        PrimitiveType::U64,
    ];
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(SUFFIX, edition, false);
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let targets: Vec<_> = graph
            .facts()
            .iter()
            .enumerate()
            .filter(|(_, fact)| fact.kind() == FactKind::Let && fact.annotation().is_none())
            .map(|(id, _)| crate::ir::pc::PcKey {
                body: body.clone(),
                node: u32::try_from(id).unwrap(),
                phase: 0,
            })
            .collect();
        assert_eq!(targets.len(), expected.len());
        for (key, expected) in targets.into_iter().zip(expected) {
            let start = db.trace.events.len();
            let query = QueryKey::BindingConstraints(key);
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let QueryValue::BindingConstraints(source) = first.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                source.state(),
                BindingConstraintState::Resolved(expected),
                "real binding goal {source:?}"
            );
            let second = ready(db.fetch(&SyntaxProvider, query));
            assert!(Arc::ptr_eq(&first.value, &second.value));
            assert!(
                !db.trace.events[start..].iter().any(|event| matches!(
                    event.key,
                    QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
                ))
            );
            assert!(db.trace.events[start..].iter().all(
                |event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)
            ));
        }
        for item in local
            .context()
            .facts()
            .items
            .iter()
            .filter(|row| row.item.def_key.item_kind == ItemKind::Function)
        {
            let definition = item.item.def_key.fingerprint();
            let memo = db
                .memo(&QueryKey::FunctionSignature(definition))
                .expect("actual selected signature memo");
            assert!(!memo.dependencies.iter().any(|dependency| matches!(dependency,
                crate::db::dependency::DependencyStamp::Input { key: InputKey::DefinitionBody(owner), .. } if *owner == definition)),
                "declared parameter source must not depend on a future executable body locator");
        }
    }
}

#[test]
fn unchanged_radix_suffixed_literals_never_become_inference_variables() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(RADIX, edition, false);
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let bindings: Vec<_> = graph
            .facts()
            .iter()
            .enumerate()
            .filter(|(_, fact)| fact.kind() == FactKind::Let)
            .map(|(id, _)| id)
            .collect();
        assert_eq!(bindings.len(), 6);
        assert_eq!(
            graph
                .facts()
                .iter()
                .filter(|fact| fact.kind() == FactKind::SuffixedInteger)
                .count(),
            6
        );
        for node in bindings {
            let start = db.trace.events.len();
            let key = crate::ir::pc::PcKey {
                body: body.clone(),
                node: u32::try_from(node).unwrap(),
                phase: 0,
            };
            let result = ready(db.fetch(&SyntaxProvider, QueryKey::BindingConstraints(key)));
            let QueryValue::BindingConstraints(source) = result.value.as_ref() else {
                panic!();
            };
            assert_eq!(source.state(), BindingConstraintState::NoNumericDemand);
            assert!(!db.trace.events[start..].iter().any(|event| matches!(
                event.key,
                QueryKey::EnteredValue(_)
                    | QueryKey::DeclaredParameterGoal { .. }
                    | QueryKey::Thunk(_)
            )));
        }
    }
}

#[test]
fn ordinary_binding_phases_share_one_prepared_guard_and_reject_revoked_source() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(SUFFIX, edition, false);
        let index = local.context().source().index();
        let body = index.body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let node = graph
            .facts()
            .iter()
            .position(|fact| fact.kind() == FactKind::Let && fact.annotation().is_none())
            .unwrap();
        let key = index
            .pc(
                crate::ir::pc::NodeId(u32::try_from(node).unwrap()),
                crate::ir::pc::Phase::Entry,
            )
            .unwrap();
        let prepared = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(key.clone())));
        let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
            panic!();
        };
        assert_eq!(
            plan.constraints().state(),
            BindingConstraintState::Resolved(PrimitiveType::I8)
        );
        let guard = plan
            .guard()
            .expect("resolved ordinary plan retains its actual input closure");
        let owners = Arc::strong_count(guard);
        let mut recipes = Vec::new();
        for phase in [crate::ir::pc::Phase::Entry, crate::ir::pc::Phase::Commit] {
            let phase_key = index.pc(crate::ir::pc::NodeId(key.node), phase).unwrap();
            let value = ready(db.fetch(&SyntaxProvider, QueryKey::Thunk(phase_key)));
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                recipe.prepared_inputs_fingerprint(),
                Some(guard.fingerprint())
            );
            recipe.revalidate_namespace(&db).unwrap();
            recipes.push(value);
        }
        assert_eq!(
            Arc::strong_count(guard),
            owners + 2,
            "each ordinary phase shares the original guard Arc"
        );
        assert_eq!(
            db.trace.counts.get("PreparedBindingPlan:ProviderRun"),
            Some(&1)
        );
        assert_eq!(
            db.trace.counts.get("BindingConstraints:ProviderRun"),
            Some(&1)
        );
        assert_eq!(
            db.trace.counts.get("SourceConstraints:ProviderRun"),
            Some(&1)
        );
        db.begin_expansion_epoch(2).unwrap();
        let before = db.trace.events.len();
        for value in recipes {
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                recipe.revalidate_namespace(&db).unwrap_err().code,
                "LRR-E-INTERNAL-SNAPSHOT"
            );
        }
        assert_eq!(
            db.trace.events.len(),
            before,
            "installed guards must not fetch or replan source"
        );
    }
}

use crate::frontend::binding_constraints::ConstraintDemand;
/// The state of the first unannotated numeric-literal `let` in `main`, read
/// through the ordinary BindingConstraints query (`retained` selects the
/// published body whose actual macro output joins ordinary syntax).
fn first_literal_let(text: &str, edition: u16, retained: bool) -> BindingConstraintState {
    let (mut db, local) = world(text, edition, retained);
    let body = local.context().source().index().body().clone();
    let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
    let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
        panic!();
    };
    let node = graph
        .facts()
        .iter()
        .position(|fact| {
            fact.kind() == FactKind::Let
                && fact.annotation().is_none()
                && fact.operand(1).is_some_and(|init| {
                    matches!(
                        graph.fact(init).map(|f| f.kind()),
                        Some(FactKind::Integer | FactKind::Float | FactKind::Prefix)
                    )
                })
        })
        .expect("source has an unannotated literal let");
    let start = db.trace.events.len();
    let query = QueryKey::BindingConstraints(crate::ir::pc::PcKey {
        body: body.clone(),
        node: u32::try_from(node).unwrap(),
        phase: 0,
    });
    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
    let QueryValue::BindingConstraints(source) = first.value.as_ref() else {
        panic!();
    };
    let state = source.state();
    // Demand-driven source facts only: no executable thunk, pattern plan or
    // another body's structure is prepared to answer a Type goal.
    assert!(
        !db.trace.events[start..]
            .iter()
            .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::PatternPlan(_))),
        "{text}"
    );
    assert!(
        db.trace.events[start..]
            .iter()
            .all(|event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body))
    );
    let second = ready(db.fetch(&SyntaxProvider, query));
    assert!(Arc::ptr_eq(&first.value, &second.value));
    state
}

#[test]
fn closed_numeric_components_take_rustc_integer_and_float_fallback() {
    use BindingConstraintState::Resolved;
    let cases: &[(&str, BindingConstraintState)] = &[
        (
            "fn main() { let n = 3; if n == 4 { } }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let n = 3; let b = !(n == 4); }",
            Resolved(PrimitiveType::I32),
        ),
        // The left operand's fallback type decides the overflow (i32::MAX + 1).
        (
            "fn main() { let n = 2147483647; let m = n + 1; }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let a = 1.5; let b = a * 2.0; }",
            Resolved(PrimitiveType::F64),
        ),
        (
            "fn main() { let n = -3; let m = -(n - 1) % 2; m; }",
            Resolved(PrimitiveType::I32),
        ),
        // A shift's right operand is independent of its left/result Type.
        (
            "fn main() { let n = 3; let x: u8 = 1u8 << n; }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let mut x: u8 = 1; let n = 3; x <<= n; }",
            Resolved(PrimitiveType::I32),
        ),
        // A cast constrains its result, never its operand.
        (
            "fn main() { let n = 300; let x = n as u8; }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let n = 3; let _ = n; }",
            Resolved(PrimitiveType::I32),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition} {text}"
            );
        }
    }
}

#[test]
fn numeric_relations_select_the_connected_concrete_primitive() {
    use BindingConstraintState::Resolved;
    let cases: &[(&str, BindingConstraintState)] = &[
        (
            "fn main() { let a: u64 = 5; let n = 3; let c = a + n; }",
            Resolved(PrimitiveType::U64),
        ),
        (
            "fn main() { let n = 3; let m = n + 1u64; }",
            Resolved(PrimitiveType::U64),
        ),
        (
            "fn main() { let a = 1.5; let b = a + 2.0f32; }",
            Resolved(PrimitiveType::F32),
        ),
        // Shift: the left operand shares the result's Type.
        (
            "fn main() { let n = 3; let x: u64 = n << 2u8; }",
            Resolved(PrimitiveType::U64),
        ),
        (
            "fn main() { let mut x: u16 = 1; let n = 3; x += n; }",
            Resolved(PrimitiveType::U16),
        ),
        (
            "fn main() { let mut x: i64 = 1; let n = 3; x = n; }",
            Resolved(PrimitiveType::I64),
        ),
        (
            "fn main() { let n = 3; let x: i8 = -n; }",
            Resolved(PrimitiveType::I8),
        ),
        (
            "fn main() { let n = 3; let x: u16 = { n }; }",
            Resolved(PrimitiveType::U16),
        ),
        (
            "fn main() { let n = 3; let c = true; let x: u32 = if c { n } else { 4 }; }",
            Resolved(PrimitiveType::U32),
        ),
        // Comparison operands share one Type; `let m = n` joins the component.
        (
            "fn main() { let n = 3; let m = n; let k: usize = 2; if m < k { } }",
            Resolved(PrimitiveType::Usize),
        ),
        (
            "fn main() { let x = 7; let y: i16 = 2; let z = x * (y - 1); }",
            Resolved(PrimitiveType::I16),
        ),
        // A later shadowing binding of the same spelling is a different local.
        (
            "fn main() { let n = 3; let n: u8 = 5; let m = n + 1; }",
            Resolved(PrimitiveType::I32),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition} {text}"
            );
        }
    }
}

#[test]
fn unanswerable_numeric_relations_stay_explicit_and_never_guess() {
    for edition in [2015, 2018, 2021, 2024] {
        // Method selection is not a common source goal yet.
        let state = first_literal_let(
            "fn main() { let x: u8 = 200; let n = 100; let s = x.wrapping_add(n); }",
            edition,
            false,
        );
        assert!(
            matches!(
                state,
                BindingConstraintState::Pending {
                    demand: ConstraintDemand::MethodSelection,
                    ..
                }
            ),
            "{state:?}"
        );
        // A member reached through a structural relation (here a borrow)
        // leaves an unanswerable consumer to the existing held route.
        assert_eq!(
            first_literal_let(
                "fn main() { let n = 3; let r = &n; r.count_ones(); }",
                edition,
                false
            ),
            BindingConstraintState::Unconstrained,
            "{edition}"
        );
        // An index operand's Type follows the indexed base's selection: a
        // builtin array/slice index is `usize`; a base whose Type no common
        // goal states keeps the existing held route.
        assert_eq!(
            first_literal_let(
                "fn main() { let n = 3; let v = [1u8, 2, 3, 4]; let x = v[n]; }",
                edition,
                false
            ),
            BindingConstraintState::Resolved(PrimitiveType::Usize),
            "{edition}"
        );
        // A generic callee's result takes the Type its arguments state
        // (`T` is `[u8; 2]` here); a result no argument fixes states none.
        assert_eq!(
            first_literal_let(
                "fn f<T>(t: T) -> T { t }\nfn main() { let n = 3; let v = f([1u8, 2]); let x = v[n]; }",
                edition,
                false
            ),
            BindingConstraintState::Resolved(PrimitiveType::Usize),
            "{edition}"
        );
        assert_eq!(
            first_literal_let(
                "fn f<T>() -> T { loop {} }\nfn main() { let n = 3; let v = f(); let x = v[n]; }",
                edition,
                false
            ),
            BindingConstraintState::Unconstrained,
            "{edition}"
        );
        // An original body with an unexpanded macro cannot even classify the
        // binding; it is never resolved by fallback.
        let state = first_literal_let("fn main() { let n = 3; assert!(n == 3); }", edition, false);
        assert!(
            matches!(
                state,
                BindingConstraintState::Pending { .. } | BindingConstraintState::Unconstrained
            ),
            "{edition} {state:?}"
        );
    }
}

/// The same relation handwritten, emitted by a user macro, by a wrapper
/// macro and by the standard `assert!`/`println!` expansions joins the one
/// ordinary source-constraint route through the published body.
#[test]
fn generated_and_handwritten_numeric_relations_share_one_route() {
    let sources = [
        "fn main() { let n = 3; let m = n + 1; if !(m == 4) { panic!() } }",
        "macro_rules! check { ($e:expr) => { if !($e) { panic!() } } }\n\
         fn main() { let n = 3; let m = n + 1; check!(m == 4); }",
        "macro_rules! wrap { ($e:expr) => { assert!($e) } }\n\
         fn main() { let n = 3; let m = n + 1; wrap!(m == 4); }",
        "fn main() { let n = 3; let m = n + 1; assert!(m == 4); }",
        "fn main() { let n = 3; println!(\"{}\", n); }",
        "fn main() { let n = 3; let m = n + 1; println!(\"{m}\"); }",
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for text in sources {
            if edition == 2015 && text.contains("{m}") {
                continue; // implicit format captures are 2021+ syntax
            }
            assert_eq!(
                first_literal_let(text, edition, true),
                BindingConstraintState::Resolved(PrimitiveType::I32),
                "{edition} {text}"
            );
        }
        // Concrete Types flow through generated syntax exactly like source.
        let text = "macro_rules! sum { ($a:expr, $b:expr) => { $a + $b } }\n\
                    fn main() { let n = 3; let x: u8 = 1; let y = sum!(x, n); }";
        assert_eq!(
            first_literal_let(text, edition, true),
            BindingConstraintState::Resolved(PrimitiveType::U8),
            "{edition} {text}"
        );
        // A format count must be usize; that relation is not answered here.
        let state = first_literal_let(
            "fn main() { let n = 3; println!(\"{:1$}\", 5, n); }",
            edition,
            true,
        );
        assert!(
            matches!(state, BindingConstraintState::Pending { .. }),
            "{edition} {state:?}"
        );
    }
}

/// Every bare unsuffixed literal's own goal, in source order.
fn literal_goals(text: &str, edition: u16, retained: bool) -> Vec<BindingConstraintState> {
    let (mut db, local) = world(text, edition, retained);
    let body = local.context().source().index().body().clone();
    let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
    let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
        panic!();
    };
    let nodes: Vec<_> = graph
        .facts()
        .iter()
        .enumerate()
        .filter(|(_, fact)| matches!(fact.kind(), FactKind::Integer | FactKind::Float))
        .map(|(id, _)| u32::try_from(id).unwrap())
        .collect();
    nodes
        .into_iter()
        .map(|node| {
            let value = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::BindingConstraints(crate::ir::pc::PcKey {
                    body: body.clone(),
                    node,
                    phase: 0,
                }),
            ));
            let QueryValue::BindingConstraints(source) = value.value.as_ref() else {
                panic!();
            };
            source.state()
        })
        .collect()
}

#[test]
fn bare_literals_use_the_same_component_goal_and_castable_expectation() {
    use BindingConstraintState::Resolved;
    use PrimitiveType::*;
    let cases: &[(&str, &[BindingConstraintState])] = &[
        (
            "fn main() { let x: u8 = 3; let y = 5 == x; }",
            &[Resolved(U8), Resolved(U8)],
        ),
        (
            "fn main() { let x: u8 = 200; let y = 100 + x; }",
            &[Resolved(U8), Resolved(U8)],
        ),
        (
            "fn main() { let y = 1 + 2 * 3; }",
            &[Resolved(I32), Resolved(I32), Resolved(I32)],
        ),
        (
            "fn main() { let y = 1.5 * 2.0; }",
            &[Resolved(F64), Resolved(F64)],
        ),
        // rustc's castable expectation types a literal operand directly; a
        // named or operator operand keeps its own variable.
        ("fn main() { let c = 97 as char; }", &[Resolved(U8)]),
        ("fn main() { let c = -(5) as i8; }", &[Resolved(I8)]),
        ("fn main() { let f = 2.5 as f32; }", &[Resolved(F32)]),
        ("fn main() { let g = 5 as f64; }", &[Resolved(I32)]),
        (
            "fn main() { let h = (1 + 2) as u8; }",
            &[Resolved(I32), Resolved(I32)],
        ),
        // A shift amount is independent of the shifted value.
        (
            "fn main() { let x: u64 = 1; let y = x << 3; }",
            &[Resolved(U64), Resolved(I32)],
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                literal_goals(text, edition, false),
                *expected,
                "{edition} {text}"
            );
        }
        // Format arguments are generic formatting-trait operands.
        assert_eq!(
            literal_goals("fn main() { println!(\"{}\", -7 % 3); }", edition, true),
            [Resolved(I32), Resolved(I32)],
            "{edition}"
        );
    }
}

/// A module-level declaration answers parameter and result goals through
/// the same SourceType adapter the call contracts use, in the caller's
/// completed environment; an alias normalizes through the common service.
#[test]
fn module_declarations_answer_parameter_and_result_goals() {
    use BindingConstraintState::Resolved;
    let cases: &[(&str, BindingConstraintState)] = &[
        (
            "fn takes(x: u8) {}\nfn main() { let n = 3; takes(n); }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn f() -> u16 { 5 }\nfn main() { let n = 3; let m = n + f(); }",
            Resolved(PrimitiveType::U16),
        ),
        (
            "type B = i64;\nfn f(x: B) -> B { x }\nfn main() { let n = 3; let m = f(n); }",
            Resolved(PrimitiveType::I64),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                first_literal_let(text, edition, true),
                *expected,
                "{edition} {text}"
            );
        }
        // A type-relative associated function (`P::f`, `Self::f`) is
        // selected through the environment's shared impl member index.
        for text in [
            "struct P;\nimpl P { fn f(x: u8) -> u8 { x } }\nfn main() { let n = 3; let m = P::f(n); }",
            "struct P;\nimpl P { fn f(_x: u8) {} }\nfn main() { let n = 3; P::f(n); }",
        ] {
            assert_eq!(
                first_literal_let(text, edition, true),
                Resolved(PrimitiveType::U8),
                "{edition} {text}"
            );
        }
    }
}

/// Every local in a resolved component shares its Type, so a compound
/// assignment consumer can read any member binding from one goal.
#[test]
fn resolved_components_name_their_member_bindings() {
    let text = "fn main() { let mut t = 0; let mut k = 1; let other = 2u8; t += k; }";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(text, edition, false);
        let index = local.context().source().index();
        let body = index.body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let lets: Vec<_> = graph
            .facts()
            .iter()
            .enumerate()
            .filter(|(_, fact)| fact.kind() == FactKind::Let)
            .map(|(id, fact)| (u32::try_from(id).unwrap(), fact.operand(0).unwrap()))
            .collect();
        let [(t, t_pattern), (_, k_pattern), (_, other_pattern)] = lets[..] else {
            panic!("three lets");
        };
        let value = ready(db.fetch(
            &SyntaxProvider,
            QueryKey::BindingConstraints(crate::ir::pc::PcKey {
                body: body.clone(),
                node: t,
                phase: 0,
            }),
        ));
        let QueryValue::BindingConstraints(source) = value.value.as_ref() else {
            panic!();
        };
        assert_eq!(
            source.state(),
            BindingConstraintState::Resolved(PrimitiveType::I32)
        );
        assert_eq!(
            source.local_primitive(index, t_pattern).unwrap(),
            Some(PrimitiveType::I32)
        );
        assert_eq!(
            source.local_primitive(index, k_pattern).unwrap(),
            Some(PrimitiveType::I32)
        );
        assert_eq!(source.local_primitive(index, other_pattern).unwrap(), None);
    }
}

/// Borrows, dereferences, tuples, `match` and the pattern engine's retained
/// trees project one component variable through structured Types, as
/// rustc's unification does; arm values are the match's value.
#[test]
fn structural_relations_project_the_component_variable() {
    use BindingConstraintState::{Resolved, Unconstrained};
    let cases: &[(&str, BindingConstraintState)] = &[
        (
            "fn main() { let n = 3; let r = &n; let m = *r + 1u8; }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn main() { let n = 3; let r = &n; let s = &r; let m = **s + 1u8; }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn main() { let n = 3; let x: u64 = 7; let t = (&n, &x); }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let n = 3; let k = match n { 1 => 10u8, 2..=5 => 20, _ => 30 }; }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let n = 3; match n { 1i16 => {}, _ => {} } }",
            Resolved(PrimitiveType::I16),
        ),
        (
            "fn main() { let n = 3; let m: u8 = match 0 { 0 => n, _ => 1 }; }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn main() { let n = 3; let m = match n { k => k + 1u32 }; }",
            Resolved(PrimitiveType::U32),
        ),
        // A binding nested in a tuple pattern is the same local at its
        // position in the pattern engine's tree; a reference or tuple
        // annotation projects through its written structure.
        (
            "fn main() { let n = 3; let (a, b) = (n, 1); let c: u16 = a; }",
            Resolved(PrimitiveType::U16),
        ),
        (
            "fn main() { let n = 3; let t = (n, 1); let (a, b) = t; let c: u16 = a; }",
            Resolved(PrimitiveType::U16),
        ),
        (
            "fn main() { let n = 3; match (n, 1u8) { (a, b) => { let c: i64 = a; } } }",
            Resolved(PrimitiveType::I64),
        ),
        (
            "fn main() { let n = 3; let r: &u8 = &n; }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn main() { let n = 3; let p: (u8, u16) = (1, n); }",
            Resolved(PrimitiveType::U16),
        ),
        // Tuple positions, arrays (elements share one Type), integer-literal
        // indexing and array patterns.
        (
            "fn main() { let n = 3; let t = (n, 1); let u = t; let c: u64 = u.0; }",
            Resolved(PrimitiveType::U64),
        ),
        (
            "fn main() { let n = 3; let arr = [n, 2]; let c: i16 = arr[1]; }",
            Resolved(PrimitiveType::I16),
        ),
        (
            "fn main() { let n = 3; let [a, b] = [1u8, n]; }",
            Resolved(PrimitiveType::U8),
        ),
        (
            "fn main() { let n = 3; let a: [u32; 2] = [n, 1]; }",
            Resolved(PrimitiveType::U32),
        ),
        // A range index reads a slice of the base's elements: they share the
        // array's one Type, fixed by a later use or by the i32 fallback.
        (
            "fn main() { let n = 3; let arr = [n, 2]; let s = &arr[..]; }",
            Resolved(PrimitiveType::I32),
        ),
        (
            "fn main() { let n = 3; let arr = [n, 2]; let s: &[u8] = &arr[1..]; }",
            Resolved(PrimitiveType::U8),
        ),
        // A position after `..` needs the matched arity: the held route.
        (
            "fn main() { let n = 3; let (.., a) = (1u8, n); let c: u16 = a; }",
            Unconstrained,
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition} {text}"
            );
        }
        // The standard assertions expand to a borrowed tuple matched by a
        // tuple pattern and compared through dereferences; the cold failure
        // branch calls the closed facade `assert_failed`, whose `&T`/`&U`
        // impose nothing. Every literal is typed through the one route.
        assert_eq!(
            literal_goals(
                "fn main() { let x: u8 = 5; assert_eq!(x, 5); }",
                edition,
                true
            ),
            [Resolved(PrimitiveType::U8), Resolved(PrimitiveType::U8)],
            "{edition}"
        );
        assert_eq!(
            first_literal_let("fn main() { let x = 5; assert_eq!(x, 5); }", edition, true),
            Resolved(PrimitiveType::I32),
            "{edition}"
        );
        for text in [
            "fn main() { assert_eq!(2 + 2, 4); }",
            "fn main() { assert_ne!(1, 2); }",
            "fn main() { let x = 5; assert_eq!(x, 6, \"x was {}\", x); }",
        ] {
            let goals = literal_goals(text, edition, true);
            assert!(!goals.is_empty(), "{edition} {text}");
            assert!(
                goals
                    .iter()
                    .all(|goal| *goal == Resolved(PrimitiveType::I32)),
                "{edition} {text} {goals:?}"
            );
        }
    }
}

/// The common Value goal answers a binding nested in a tuple pattern (and
/// its reads) as the same local, and a qualified path through the
/// dispatcher's own written-path lookup: a generated `$crate` library path
/// selects its closed facade function, a module path its declaration.
#[test]
fn value_goal_answers_nested_bindings_and_qualified_paths() {
    use crate::facade::functions::LibraryFunction;
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world("fn main() { let x = 5; assert_eq!(x, 6); }", edition, true);
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let mut nested = 0;
        let mut facades = Vec::new();
        for (id, fact) in graph.facts().iter().enumerate() {
            let node = crate::ir::pc::NodeId(u32::try_from(id).unwrap());
            let goal = |db: &mut Database, node: crate::ir::pc::NodeId| {
                let value = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::EnteredValue(crate::ir::pc::PcKey {
                        body: body.clone(),
                        node: node.0,
                        phase: 0,
                    }),
                ));
                let QueryValue::EnteredValue(goal) = value.value.as_ref() else {
                    panic!();
                };
                goal.as_ref().clone()
            };
            if fact.kind() == FactKind::BindingPattern
                && graph
                    .occurrence(node)
                    .is_some_and(|occurrence| !occurrence.steps.is_empty())
            {
                let target = goal(&mut db, node);
                assert!(target.local_binding().is_some(), "{edition} {target:?}");
                nested += 1;
            }
            if fact.kind() == FactKind::Call
                && let Some(callee) = fact.operand(0)
                && let Some(function) = goal(&mut db, callee).facade()
            {
                facades.push(function);
            }
        }
        assert_eq!(nested, 2, "{edition}: left_val and right_val");
        assert_eq!(facades, [LibraryFunction::AssertFailed], "{edition}");
        // A module-qualified callee selects its declaration's parameter goal.
        assert_eq!(
            first_literal_let(
                "mod m { pub fn f(_x: u8) {} }\nfn main() { let n = 3; m::f(n); }",
                edition,
                true
            ),
            BindingConstraintState::Resolved(PrimitiveType::U8),
            "{edition}"
        );
    }
}

/// Struct/enum structure relates a numeric variable through the selected
/// declaration's fields: record and tuple-struct/variant constructors, named
/// and positional field accesses, constructor patterns and generic fields.
#[test]
fn nominal_fields_constructors_and_patterns_relate_declared_field_types() {
    use BindingConstraintState::Resolved;
    use PrimitiveType::*;
    let cases: &[(&str, &[BindingConstraintState])] = &[
        (
            "struct X { a: isize }\nfn main() { let x = X { a: 32 }; }",
            &[Resolved(Isize)],
        ),
        (
            "struct S(u8, u16);\nfn main() { let s = S{1: 10, 0: 11}; }",
            &[Resolved(U16), Resolved(U8)],
        ),
        (
            "struct S(u8, u16);\nfn main() { let s = S(1, 2); }",
            &[Resolved(U8), Resolved(U16)],
        ),
        (
            "enum E { V(u8), W }\nfn main() { let e = E::V(7); }",
            &[Resolved(U8)],
        ),
        (
            "enum F { B { b1: isize, bb1: i64 } }\nfn main() { let f = F::B { b1: 29, bb1: 100 }; }",
            &[Resolved(Isize), Resolved(I64)],
        ),
        // A generic field takes the record's own argument; unconstrained,
        // both fields share one fallback variable.
        (
            "struct Pair<T> { x: T, y: T }\nfn main() { let p = Pair { x: 10, y: 12 }; }",
            &[Resolved(I32), Resolved(I32)],
        ),
        (
            "struct Pair<T> { x: T, y: T }\nfn main() { let p = Pair { x: 10, y: 12 }; let z: u16 = p.y; }",
            &[Resolved(U16), Resolved(U16)],
        ),
        // A named field read through a borrow auto-dereferences its base.
        (
            "struct X { a: u64 }\nfn main() { let x = X { a: 1 }; let r = &x; let z = r.a == 2; }",
            &[Resolved(U64), Resolved(U64)],
        ),
        // The coordinator's witnesses: an aggregate of literals borrowed and
        // read through the borrow.
        (
            "fn main() { let a = [1, 2, 3]; let r = &a; if r[1] != 2 { panic!() } }",
            &[
                Resolved(I32),
                Resolved(I32),
                Resolved(I32),
                Resolved(Usize),
                Resolved(I32),
            ],
        ),
        (
            "fn main() { let a = (1, 2); let r = &a; if r.1 != 2 { panic!() } }",
            &[Resolved(I32), Resolved(I32), Resolved(I32)],
        ),
        (
            "fn main() { let a = [1, 2, 3]; let r = &a; let x: u8 = r[0]; }",
            &[Resolved(U8), Resolved(U8), Resolved(U8), Resolved(Usize)],
        ),
        // A compound field's written structure: its parameter is fixed by
        // another field, its concrete member by the declaration.
        (
            "struct V<T> { a: T, p: (T, u8) }\nfn main() { let v = V { a: 3u16, p: (6, 1) }; }",
            &[Resolved(U16), Resolved(U8)],
        ),
        (
            "struct V<T> { a: T, p: (T, u8) }\nfn main() { let v: V<i64> = V { a: 3, p: (6, 1) }; }",
            &[Resolved(I64), Resolved(I64), Resolved(U8)],
        ),
        // A written `_` hole states nothing: its literal takes the fallback.
        (
            "fn main() { let t: (_, u8) = (1, 2); }",
            &[Resolved(I32), Resolved(U8)],
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in cases {
            assert_eq!(
                &literal_goals(text, edition, false),
                expected,
                "{edition}: {text}"
            );
        }
    }
    let lets: &[(&str, BindingConstraintState)] = &[
        (
            "struct X { a: isize }\nfn main() { let x = X { a: 1 }; let n = 3; let m = x.a + n; }",
            Resolved(Isize),
        ),
        (
            "struct S(u8, u16);\nfn main() { let s = S(1, 2); let S(a, b) = s; let n = 5; let m = b + n; }",
            Resolved(U16),
        ),
        (
            "struct S(u8, u16);\nfn main() { let s = S{1: 10, 0: 11}; match s { S{0: a, 1: b, ..} => { let n = 4; let m = a + n; } } }",
            Resolved(U8),
        ),
        (
            "enum E { V(u8), U(u8) }\nfn main() { let e = E::V(10); match e { E::V(x) | E::U(x) => { let k = 3; let z = x + k; } } }",
            Resolved(U8),
        ),
        (
            "struct Pair<T> { x: T, y: T }\nfn main() { let p: Pair<i16> = Pair { x: 1, y: 2 }; let n = 3; let m = p.x + n; }",
            Resolved(I16),
        ),
        (
            "fn main() { let a = [1u8, 2, 3]; let i = 1; let v = a[i]; }",
            Resolved(Usize),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in lets {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition}: {text}"
            );
        }
    }
}

/// A method call selects its method with the dispatcher's own probe rule
/// over the receiver's static Type; its declared result and parameters then
/// relate like a call's.
#[test]
fn method_calls_select_through_the_shared_probe_rule() {
    use BindingConstraintState::Resolved;
    use PrimitiveType::*;
    let lets: &[(&str, BindingConstraintState)] = &[
        (
            "struct X { a: isize }\nimpl X { fn get(&self) -> u8 { 1 } }\nfn main() { let x = X { a: 1 }; let n = 3; let m = x.get() + n; }",
            Resolved(U8),
        ),
        (
            "struct X { a: isize }\nimpl X { fn put(&self, v: u32) {} }\nfn main() { let x = X { a: 1 }; let n = 3; x.put(n); }",
            Resolved(U32),
        ),
        (
            "struct X { a: isize }\ntrait Changer { fn change(self) -> Self; }\nimpl Changer for X { fn change(mut self) -> X { self } }\nfn main() { let x = X { a: 32 }; let new_x = x.change(); let n = 55; let b = new_x.a == n; }",
            Resolved(Isize),
        ),
        (
            "struct A { x: isize }\ntrait Foo { fn f(&self) -> i64; }\nimpl Foo for A { fn f(&self) -> i64 { 10 } }\nfn main() { let a = &A { x: 3 }; let n = 10; let b = a.f() == n; }",
            Resolved(I64),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in lets {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition}: {text}"
            );
        }
    }
}

/// A compound assignment to a local destructured from a `let` consumes the
/// goal of the literal its position selects (whose component holds it).
#[test]
fn destructured_locals_select_their_initializer_literal_goal() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(
            "fn main() { let (mut a, mut b) = (1, 2.5); let c: u8 = 3; a += c; b *= 2.0; }",
            edition,
            false,
        );
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let mut selected = Vec::new();
        for (id, fact) in graph.facts().iter().enumerate() {
            if fact.kind() != FactKind::Binary
                || fact.operation()
                    != Some(crate::syntax::source_constraints::SourceOperator::CompoundArithmetic)
            {
                continue;
            }
            let _ = id;
            let place = fact.operand(0).unwrap();
            let key = crate::ir::pc::PcKey {
                body: body.clone(),
                node: place.0,
                phase: 0,
            };
            let goal = ready(crate::frontend::binding_constraints::declaring_let(
                &mut db, &key,
            ))
            .expect("destructured local has a source goal");
            assert_eq!(
                graph
                    .fact(crate::ir::pc::NodeId(goal.node))
                    .map(|fact| fact.kind()),
                Some(if selected.is_empty() {
                    FactKind::Integer
                } else {
                    FactKind::Float
                }),
                "{edition}"
            );
            let value = ready(db.fetch(&SyntaxProvider, QueryKey::BindingConstraints(goal)));
            let QueryValue::BindingConstraints(source) = value.value.as_ref() else {
                panic!();
            };
            selected.push(source.state());
        }
        assert_eq!(
            selected,
            [
                BindingConstraintState::Resolved(PrimitiveType::U8),
                BindingConstraintState::Resolved(PrimitiveType::F64),
            ],
            "{edition}"
        );
    }
}

/// A callee's own type parameter relates its other positions; its bounds
/// select the variable's Type through a single applicable impl, leave it to
/// fallback (several impls), or stay a missing goal (no indexed candidate).
#[test]
fn callee_type_parameters_relate_their_positions_and_bounds() {
    use BindingConstraintState::Resolved;
    use PrimitiveType::*;
    let lets: &[(&str, BindingConstraintState)] = &[
        (
            "fn f<T>(a: T, b: T) {}\nfn main() { let n = 3; f(n, 4u8); }",
            Resolved(U8),
        ),
        (
            "fn id<T>(t: T) -> T { t }\nfn main() { let n = 3; let m: u16 = id(n); }",
            Resolved(U16),
        ),
        (
            "fn id<T>(t: T) -> T { t }\nfn main() { let n = 3; let m = id(n); }",
            Resolved(I32),
        ),
        (
            "fn pick<T>(a: &T, b: (T, bool)) {}\nfn main() { let n = 3; pick(&n, (7i64, true)); }",
            Resolved(I64),
        ),
        // Explicit generic arguments fix the parameter (never the fallback).
        (
            "fn f<T,>(_: T,) {}\nfn main() { let n = 3; f::<isize,>(n,); }",
            Resolved(Isize),
        ),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in lets {
            assert_eq!(
                first_literal_let(text, edition, false),
                *expected,
                "{edition}: {text}"
            );
        }
        // rustc's impl candidates for `{integer}: Tr`: one impl selects the
        // Type, several leave the variable to fallback, and a bound with no
        // indexed candidate stays pending.
        for (text, expected) in [
            (
                "trait Tr {}\nimpl Tr for u8 {}\nfn g<T: Tr>(t: T) {}\nfn main() { let n = 3; g(n); }",
                Resolved(U8),
            ),
            (
                "trait Tr {}\nimpl Tr for u8 {}\nimpl Tr for i32 {}\nfn g<T>(t: T) where T: Tr {}\nfn main() { let n = 3; g(n); }",
                Resolved(I32),
            ),
        ] {
            assert_eq!(first_literal_let(text, edition, false), expected, "{edition}: {text}");
        }
        let state = first_literal_let(
            "trait Tr {}\nfn g<T: Tr>(t: T) {}\nfn main() { let n = 3; g(n); }",
            edition,
            false,
        );
        assert!(
            matches!(
                state,
                BindingConstraintState::Pending {
                    demand: ConstraintDemand::ParameterType,
                    ..
                }
            ),
            "{edition} {state:?}"
        );
    }
}

/// An `if let` / `while let` clause matches its scrutinee against its pattern
/// as a `let` statement does: a binding of the clause relates the scrutinee.
#[test]
fn let_clauses_relate_their_scrutinee_and_pattern() {
    use BindingConstraintState::Resolved;
    use PrimitiveType::*;
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in [
            (
                "fn main() { let xs = [13, 1, 5]; if let [a, ..] = xs { let b: u8 = a; } }",
                U8,
            ),
            (
                "fn main() { let p = (1, 2); while let (a, _) = p { let b: i64 = a; break; } }",
                I64,
            ),
        ] {
            let goals = literal_goals(text, edition, false);
            assert_eq!(goals.first(), Some(&Resolved(expected)), "{edition}: {text} {goals:?}");
        }
    }
}

/// The type arguments of every call, method call and value path whose open
/// type arguments the common source constraints decided from later uses, in
/// source order (`None` for an argument the hint leaves to inference).
fn open_arguments(
    text: &str,
    edition: u16,
    retained: bool,
) -> Vec<Vec<Option<crate::traits::term::TyTerm>>> {
    let (mut db, local) = world(text, edition, retained);
    let body = local.context().source().index().body().clone();
    let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
    let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
        panic!();
    };
    let nodes: Vec<_> = graph
        .facts()
        .iter()
        .enumerate()
        .filter(|(_, fact)| {
            matches!(
                fact.kind(),
                FactKind::Call | FactKind::MethodCall | FactKind::Path
            )
        })
        .map(|(id, _)| u32::try_from(id).unwrap())
        .collect();
    let mut resolved = Vec::new();
    let start = db.trace.events.len();
    for node in nodes {
        let value = ready(db.fetch(
            &SyntaxProvider,
            QueryKey::BindingConstraints(crate::ir::pc::PcKey {
                body: body.clone(),
                node,
                phase: 0,
            }),
        ));
        let QueryValue::BindingConstraints(source) = value.value.as_ref() else {
            panic!();
        };
        if source.state() != BindingConstraintState::ResolvedType {
            continue;
        }
        let written = source.written().expect("resolved Type has its written form");
        let crate::ir::written_type::WrittenType::Generic { args, .. } = written else {
            panic!("expected a generic nominal hint: {written:?}");
        };
        resolved.push(args.iter().map(|arg| arg.source()).collect());
    }
    // Demand-driven source facts only: no executable thunk, pattern plan or
    // another body's structure is prepared to decide a Type.
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            &event.key,
            QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
        ) || matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)),
        "{text}"
    );
    resolved
}

#[test]
fn open_type_arguments_are_decided_by_later_uses() {
    use crate::traits::term::TyTerm::Primitive as P;
    use PrimitiveType::*;
    const DECLARATIONS: &str = "enum Maybe<T> { No, Yes(T) }\n\
        struct W<T> { a: Maybe<T>, n: u32 }\n\
        impl<T> W<T> {\n\
            fn new() -> W<T> { W { a: Maybe::No, n: 0 } }\n\
            fn set(&mut self, x: T) {}\n\
            fn count(&self) -> u32 { self.n }\n\
        }\n\
        fn takes(w: &W<i64>) {}\n\
        fn takes_mut(w: &mut W<u32>) {}\n\
        fn id<T>(t: T) -> W<T> { W { a: Maybe::No, n: 0 } }\n";
    let cases: &[(&str, &[&[Option<crate::traits::term::TyTerm>]])] = &[
        // A method whose declared signature relates the receiver's argument.
        ("let mut w = W::new(); w.set(7u8);", &[&[Some(P(U8))]]),
        // An unsuffixed literal beside no other relation takes the fallback.
        ("let mut w = W::new(); w.set(1);", &[&[Some(P(I32))]]),
        ("let mut w = W::new(); w.set(1); w.set(2u16);", &[&[Some(P(U16))]]),
        // Typed parameters, through a shared and a mutable borrow.
        ("let w = W::new(); takes(&w);", &[&[Some(P(I64))]]),
        ("let mut w = W::new(); takes_mut(&mut w);", &[&[Some(P(U32))]]),
        // An assignment gives both places one Type.
        (
            "let mut x = Maybe::No; x = Maybe::Yes(3u16);",
            &[&[Some(P(U16))]],
        ),
        // A comparison of two values of one nominal Type.
        (
            "let a = Maybe::No; let b = Maybe::Yes(5i64); let c = a == b;",
            &[&[Some(P(I64))]],
        ),
        // A field of another Type states nothing; the literal still falls back.
        (
            "let mut w = W::new(); w.set(2); let k = w.n + 1;",
            &[&[Some(P(I32))]],
        ),
        // The argument fixes the parameter: nothing is open.
        ("let w = id(7u8);", &[]),
        // No later use decides it: the explicit diagnostic stays.
        ("let w: W<u8> = W::new(); let n = w.count();", &[&[Some(P(U8))]]),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for (body, expected) in cases {
            let text = format!("{DECLARATIONS}fn main() {{ {body} }}");
            let mut found = open_arguments(&text, edition, false);
            // The annotated case also resolves its own call; ignore order.
            found.retain(|args| !args.is_empty());
            assert_eq!(&found[..], *expected, "{edition}: {body}");
        }
        // Undecided: no relation states the Type, so no hint is published.
        let text = format!("{DECLARATIONS}fn main() {{ let w: W<_> = W::new(); }}");
        assert!(open_arguments(&text, edition, false).iter().all(|args| args.iter().all(|arg| arg.is_some())));
        let text = format!("{DECLARATIONS}fn main() {{ let w = W::new(); }}");
        assert_eq!(open_arguments(&text, edition, false), Vec::<Vec<_>>::new(), "{edition}");
        let text = format!("{DECLARATIONS}fn main() {{ let w = W::new(); let n = w.count(); }}");
        assert_eq!(open_arguments(&text, edition, false), Vec::<Vec<_>>::new(), "{edition}");
    }
}

/// The same open type argument is decided identically when the call or the
/// deciding use is written by hand, emitted by a user macro, emitted through
/// a wrapper macro, or built inside a standard macro's expansion: the joined
/// published body is one ordinary source route.
#[test]
fn generated_and_handwritten_open_arguments_share_one_route() {
    use crate::traits::term::TyTerm::Primitive as P;
    const DECLARATIONS: &str = "enum Maybe<T> { No, Yes(T) }\n\
        struct W<T> { a: Maybe<T>, n: u32 }\n\
        impl<T> W<T> {\n\
            fn new() -> W<T> { W { a: Maybe::No, n: 0 } }\n\
            fn set(&mut self, x: T) {}\n\
        }\n";
    let bodies = [
        "let mut w = W::new(); w.set(7u8);",
        "macro_rules! make { () => { W::new() } }\n\
         fn main() { let mut w = make!(); w.set(7u8); }",
        "macro_rules! put { ($w:ident, $x:expr) => { $w.set($x) } }\n\
         fn main() { let mut w = W::new(); put!(w, 7u8); }",
        "macro_rules! make { () => { W::new() } }\n\
         macro_rules! outer { () => { make!() } }\n\
         fn main() { let mut w = outer!(); w.set(7u8); }",
        "macro_rules! both { ($x:expr) => { { let mut w = W::new(); w.set($x); } } }\n\
         fn main() { both!(7u8); }",
    ];
    for edition in [2015, 2018, 2021, 2024] {
        for body in bodies {
            let text = if body.starts_with("let ") {
                format!("{DECLARATIONS}fn main() {{ {body} }}")
            } else {
                format!("{DECLARATIONS}{body}")
            };
            for retained in [false, true] {
                if retained == false && !body.starts_with("let ") {
                    continue;
                }
                let found = open_arguments(&text, edition, retained);
                assert_eq!(found, vec![vec![Some(P(PrimitiveType::U8))]], "{edition} {retained}: {text}");
            }
        }
    }
}

/// Typed literals and tuples state their own Type; a value built from a
/// field Type that names the constructed value's parameters is decided by
/// that value's other relations.
#[test]
fn open_arguments_take_literal_tuple_and_dependent_types() {
    use crate::traits::term::{GenericArg, RegionTerm, TyTerm, TyTerm::Primitive as P};
    use PrimitiveType::*;
    const DECLARATIONS: &str = "enum Maybe<T> { No, Yes(T) }\n\
        struct Boxed<T>(T);\n\
        struct W<T> { a: Maybe<T>, n: u32 }\n\
        impl<T> W<T> {\n\
            fn new() -> W<T> { W { a: Maybe::No, n: 0 } }\n\
            fn set(&mut self, x: T) {}\n\
        }\n\
        struct List<T> { value: T, next: Maybe<Boxed<List<T>>> }\n";
    let text = |body: &str| format!("{DECLARATIONS}fn main() {{ {body} }}");
    for edition in [2015, 2018, 2021, 2024] {
        let found = open_arguments(&text("let mut w = W::new(); w.set(\"a\");"), edition, false);
        assert_eq!(
            found,
            vec![vec![Some(TyTerm::Ref(RegionTerm::Static, false, Box::new(TyTerm::Str)))]],
            "{edition}"
        );
        let found = open_arguments(&text("let mut w = W::new(); w.set((1u8, true));"), edition, false);
        assert_eq!(
            found,
            vec![vec![Some(TyTerm::Tuple(vec![P(U8), P(Bool)]))]],
            "{edition}"
        );
        // `next` holds a `Maybe<Boxed<List<T>>>`, where `T` is the type of
        // the literal `1` beside it: the placeholder is decided by the
        // constructed value's own relations (the integer fallback).
        let found = open_arguments(
            &text("let list = List { value: 1, next: Maybe::No }; let n = list.value;"),
            edition,
            false,
        );
        let [args] = &found[..] else {
            panic!("{edition} {found:?}");
        };
        let [Some(TyTerm::Nominal(_, boxed))] = &args[..] else {
            panic!("{edition} {args:?}");
        };
        let [GenericArg::Type(TyTerm::Nominal(_, list))] = &boxed[..] else {
            panic!("{edition} {boxed:?}");
        };
        assert_eq!(&list[..], &[GenericArg::Type(P(I32))], "{edition}");
        // The same field decided by a later method call.
        let found = open_arguments(
            &text("let mut a = W::new(); a.set(2u16); let list = List { value: 1u16, next: Maybe::No };"),
            edition,
            false,
        );
        assert!(found.len() == 2, "{edition} {found:?}");
    }
}
