//! Authority controls outside the complete unchanged official numeric witness.
use super::*;
use crate::{
    db::trace::Action,
    frontend::source_parameter_goals::{ParameterGoalObservation, ParameterTypeGoal},
    ir::op::PrimitiveType,
    syntax::signatures::FunctionSignature,
    traits::term::ParamKey,
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);

#[test]
fn upstream_parameter_order_preserves_real_impl_trait_and_method_identities() {
    use crate::traits::{
        term::BoundKind,
        upstream::{
            identity::DefinitionId,
            parameters::{ParameterPending, ParameterPreparation, SignatureParameters},
            storage::InternPool,
        },
    };
    use rustc_type_ir::inherent::ParamLike;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/generics/mid-path-type-params.rs"
    );
    // Keep the original unchanged. The macro adaptations move only the crate
    // lint attribute outside the item-producing macro, where Rust permits it.
    let items = ORIGINAL.replace("#![allow(dead_code)]", "");
    let emitted = format!(
        "#![allow(dead_code)] macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{items}}}"
    );
    let wrapped = format!(
        "#![allow(dead_code)] macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{items}}}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for text in [ORIGINAL, &emitted, &wrapped] {
            let (mut db, plan) = world(text, edition, false);
            let context = plan
                .environment()
                .unwrap()
                .macros()
                .source_environment()
                .dependency()
                .fingerprint();
            let nodes = InternPool::default();
            let foreign_nodes = InternPool::default();
            let mut methods = 0;
            for (name, definition) in published_source_declarations(&plan, &["new"]) {
                if name != "new" {
                    continue;
                }
                let key = QueryKey::FunctionSignature(definition);
                let start = db.trace.events.len();
                let owner = ready(db.fetch(&SyntaxProvider, key.clone()));
                let QueryValue::FunctionSignature(signature) = owner.value.as_ref() else {
                    panic!()
                };
                let identity = DefinitionId::new(context, definition, true);
                assert_eq!(identity.world(), context);
                assert_eq!(identity.source(), definition);
                assert_ne!(
                    identity.fingerprint(),
                    DefinitionId::new(context, definition, false).fingerprint()
                );
                assert_ne!(
                    identity.fingerprint(),
                    DefinitionId::new(
                        Digest::of_bytes(b"foreign source context"),
                        definition,
                        true,
                    )
                    .fingerprint()
                );
                let ParameterPreparation::Ready(mapping) =
                    SignatureParameters::prepare(signature).unwrap()
                else {
                    panic!("actual source method must have a complete type-only binder")
                };
                let expected: &[&str] = match signature.generics.len() {
                    1 => &["U"],
                    2 => &["T", "U"],
                    3 => &["Self", "T", "U"],
                    _ => panic!("stock source generic header differs"),
                };
                assert_eq!(mapping.count() as usize, expected.len());
                assert_eq!(mapping.parent_count() as usize, expected.len() - 1);
                for (index, name) in expected.iter().enumerate() {
                    let parameter = mapping.from_upstream(index as u32).unwrap();
                    let node = nodes.intern(parameter);
                    assert_eq!(node, nodes.intern(parameter));
                    assert_ne!(node, foreign_nodes.intern(parameter));
                    assert_eq!(*node.value(), parameter);
                    assert_eq!(parameter.index(), index as u32);
                    assert_eq!(parameter.source().definition, definition);
                    assert_eq!(
                        signature.generics[parameter.source().ordinal as usize].name,
                        *name
                    );
                    assert_eq!(parameter.kind(), BoundKind::Type);
                    assert_eq!(
                        mapping
                            .from_source(parameter.source(), BoundKind::Type)
                            .unwrap(),
                        parameter
                    );
                    assert!(mapping
                        .from_source(parameter.source(), BoundKind::Region)
                        .is_err());
                    assert!(mapping
                        .from_source(
                            ParamKey {
                                definition: Digest::of_bytes(b"foreign parameter declaration"),
                                ordinal: parameter.source().ordinal,
                            },
                            BoundKind::Type
                        )
                        .is_err());
                }
                assert!(mapping.from_upstream(mapping.count()).is_err());
                let again = ready(db.fetch(&SyntaxProvider, key));
                assert!(Arc::ptr_eq(&owner.value, &again.value));
                no_child_code(&db, start);
                let mut synthetic = signature.as_ref().clone();
                synthetic.generic_source_layout = None;
                assert!(matches!(
                    SignatureParameters::prepare(&synthetic).unwrap(),
                    ParameterPreparation::Pending(ParameterPending::SourceLayout)
                ));
                let mut changed = signature.as_ref().clone();
                changed.generic_source_layout.as_mut().unwrap().own += 1;
                assert!(SignatureParameters::prepare(&changed).is_err());
                methods += 1;
            }
            assert_eq!(methods, 3, "edition {edition}, source: {text}");
        }
    }
}

#[test]
fn upstream_parameter_order_retains_const_regions_and_pending_function_binders() {
    use crate::traits::{
        term::BoundKind,
        upstream::parameters::{ParameterPending, ParameterPreparation, SignatureParameters},
    };
    use rustc_type_ir::inherent::ParamLike;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/type-dependent/issue-61936.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = world(ORIGINAL, edition, false);
        let mut methods = 0;
        for (name, definition) in
            published_source_declarations(&plan, &["next", "array_windows_example"])
        {
            if name != "next" && name != "array_windows_example" {
                continue;
            }
            let start = db.trace.events.len();
            let owner = ready(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
            let QueryValue::FunctionSignature(signature) = owner.value.as_ref() else {
                panic!()
            };
            let mapping = SignatureParameters::prepare(signature).unwrap();
            if name == "array_windows_example" {
                assert!(matches!(
                    mapping,
                    ParameterPreparation::Pending(ParameterPending::FunctionRegionBinders)
                ));
            } else {
                let ParameterPreparation::Ready(mapping) = mapping else {
                    panic!("inherited regions are early bound")
                };
                assert_eq!(mapping.parent_count(), 3);
                for (index, kind) in [BoundKind::Region, BoundKind::Type, BoundKind::Const]
                    .into_iter()
                    .enumerate()
                {
                    let parameter = mapping.from_upstream(index as u32).unwrap();
                    assert_eq!(parameter.index(), index as u32);
                    assert_eq!(parameter.kind(), kind);
                    assert_eq!(
                        mapping.from_source(parameter.source(), kind).unwrap(),
                        parameter
                    );
                }
            }
            no_child_code(&db, start);
            methods += 1;
        }
        assert_eq!(methods, 3);
    }
}
/// The completed environment owns module/associated declarations; the entered
/// body's local item facts alone are not the module's member inventory.
fn published_source_declarations(
    plan: &PreparedSourceLocalPlan,
    member_names: &[&str],
) -> Vec<(String, Digest)> {
    let environment = plan.environment().unwrap().macros().source_environment();
    let mut declarations = std::collections::BTreeMap::new();
    let mut types = std::collections::BTreeSet::new();
    for source in environment.sources() {
        for item in &source.outline().items {
            if item.def_key.item_kind == ItemKind::Function {
                declarations.insert(
                    item.def_key.fingerprint(),
                    item.def_key.declared_name.clone(),
                );
            }
            if matches!(
                item.def_key.item_kind,
                ItemKind::Struct | ItemKind::Enum | ItemKind::Trait
            ) {
                types.insert(item.def_key.fingerprint());
            }
        }
    }
    for item in environment.names().declared_item_names() {
        if item.definition().namespace == crate::source::manifest::Namespace::Value {
            declarations.insert(item.definition().identity, item.name().to_owned());
        } else if item.definition().namespace == crate::source::manifest::Namespace::Type {
            types.insert(item.definition().identity);
        }
    }
    // Generated associated declarations are owned by the actual member index,
    // not the module-level item-name inventory. Use its ordinary source IDs.
    let members = environment.member_index().unwrap();
    for owner in types {
        for name in member_names {
            for definition in members.inherent(owner, name) {
                declarations.insert(definition, (*name).to_owned());
            }
            for (_, definition) in members.trait_functions(owner, name) {
                declarations.insert(definition, (*name).to_owned());
            }
            if let Some(definition) = members.trait_declaration(owner, name) {
                declarations.insert(definition, (*name).to_owned());
            }
        }
    }
    declarations
        .into_iter()
        .map(|(definition, name)| (name, definition))
        .collect()
}
fn declarations(plan: &PreparedSourceLocalPlan) -> Vec<(String, Digest)> {
    plan.context()
        .facts()
        .items
        .iter()
        .filter(|row| row.item.def_key.item_kind == ItemKind::Function)
        .map(|row| {
            (
                row.item.def_key.declared_name.clone(),
                row.item.def_key.fingerprint(),
            )
        })
        .collect()
}
fn signature(db: &mut Database, definition: Digest) -> FunctionSignature {
    let outcome = ready(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
    let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
        panic!("actual signature query differs")
    };
    signature.as_ref().clone()
}
fn goal(value: &QueryValue) -> &ParameterGoalObservation {
    let QueryValue::DeclaredParameterGoal(goal) = value else {
        panic!("actual parameter query differs: {value:?}")
    };
    goal
}
fn no_child_code(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
        )),
        "declared header goal entered executable preparation"
    );
}
fn check_suffix(db: &mut Database, plan: &PreparedSourceLocalPlan) {
    let expected = [
        ("id_i8", PrimitiveType::I8),
        ("id_i16", PrimitiveType::I16),
        ("id_i32", PrimitiveType::I32),
        ("id_i64", PrimitiveType::I64),
        ("id_uint", PrimitiveType::Usize),
        ("id_u8", PrimitiveType::U8),
        ("id_u16", PrimitiveType::U16),
        ("id_u32", PrimitiveType::U32),
        ("id_u64", PrimitiveType::U64),
    ];
    let declarations = declarations(plan);
    assert_eq!(declarations.len(), expected.len());
    for (name, primitive) in expected {
        let definition = declarations
            .iter()
            .find(|(actual, _)| actual == name)
            .unwrap()
            .1;
        let key = QueryKey::DeclaredParameterGoal {
            definition,
            ordinal: 0,
        };
        let start = db.trace.events.len();
        let first = ready(db.fetch(&SyntaxProvider, key.clone()));
        let ParameterTypeGoal::Ready(source) = goal(first.value.as_ref()).goal() else {
            panic!(
                "unchanged parameter Type goal pending: {:?}",
                goal(first.value.as_ref()).goal()
            )
        };
        assert_eq!(source.source(), &TyTerm::Primitive(primitive));
        assert_eq!(source.definition(), definition);
        assert_eq!(source.ordinal(), 0);
        let second = ready(db.fetch(&SyntaxProvider, key.clone()));
        assert!(
            Arc::ptr_eq(&first.value, &second.value),
            "immutable goal was reconstructed"
        );
        assert_eq!(
            db.trace.events[start..]
                .iter()
                .filter(|event| event.key == key && event.action == Action::ProviderRun)
                .count(),
            1
        );
        no_child_code(db, start);
    }
}
#[test]
fn unchanged_declared_parameter_goals_are_memoized_without_child_code_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for retained in [false, true] {
            let (mut db, plan) = world(SUFFIX, edition, retained);
            check_suffix(&mut db, &plan);
        }
    }
}

#[test]
fn contextual_parameter_goals_bind_the_actual_caller_and_reject_local_receipt_substitution() {
    // LRR source-authority control over actual unchanged upstream headers.
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = world(SUFFIX, edition, false);
        let definition = declarations(&plan)[0].1;
        let body = plan.context().source().index().body().clone();
        let local_key = QueryKey::DeclaredParameterGoal {
            definition,
            ordinal: 0,
        };
        let local = ready(db.fetch(&SyntaxProvider, local_key.clone()));
        let key = QueryKey::ContextParameterGoal {
            body: body.clone(),
            definition,
            ordinal: 0,
        };
        let start = db.trace.events.len();
        let actual = ready(db.fetch(&SyntaxProvider, key.clone()));
        assert!(actual.value.matches(&key));
        assert!(!actual.value.matches(&local_key));
        assert!(!local.value.matches(&key));
        let mut foreign = body;
        foreign.exact_body_bytes = Digest::of_bytes(b"foreign caller body");
        assert!(!actual.value.matches(&QueryKey::ContextParameterGoal {
            body: foreign,
            definition,
            ordinal: 0,
        }));
        let again = ready(db.fetch(&SyntaxProvider, key.clone()));
        assert!(Arc::ptr_eq(&actual.value, &again.value));
        no_child_code(&db, start);
        let guard = ready(db.prepare_input_guard(&SyntaxProvider, &key, actual.fingerprint));
        guard.require_current(&mut db).unwrap();
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert!(guard.require_current(&mut db).is_err());
    }
}
#[test]
fn emitted_and_wrapper_parameter_headers_use_the_same_goal_and_real_epoch() {
    for edition in [2015, 2018, 2021, 2024] {
        for wrapper in [false, true] {
            let text = format!(
                "macro_rules! emit{{()=>{{{SUFFIX}}}}}{}",
                if wrapper {
                    "macro_rules! wrap{()=>{emit!();}}wrap!();"
                } else {
                    "emit!();"
                }
            );
            let (mut db, plan) = world(&text, edition, false);
            check_suffix(&mut db, &plan);
            let definition = declarations(&plan)[0].1;
            db.begin_expansion_epoch(2).unwrap();
            match db.fetch(
                &SyntaxProvider,
                QueryKey::DeclaredParameterGoal {
                    definition,
                    ordinal: 0,
                },
            ) {
                QueryPoll::Ready(Err(_)) => {}
                other => panic!("stale generated parameter authority was not refused: {other:?}"),
            }
        }
    }
}
#[test]
fn parameter_goal_rejects_wrong_ordinal_header_and_revoked_parent() {
    let (mut db, plan) = world(SUFFIX, 2024, false);
    let definition = declarations(&plan)[0].1;
    let start = db.trace.events.len();
    let actual = signature(&mut db, definition);
    match db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition,
            ordinal: 1,
        },
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
        other => panic!("wrong ordinal accepted: {other:?}"),
    }
    let mut wrong = actual.clone();
    wrong.parameters[0].ty[0].text.push_str("_different");
    refused(
        plan.prepare_parameter_type(&mut db, &wrong, 0),
        "substituted parameter header",
    );
    let key = QueryKey::DeclaredParameterGoal {
        definition,
        ordinal: 0,
    };
    ready(db.fetch(&SyntaxProvider, key.clone()));
    no_child_code(&db, start);
    db.publish_inputs(BTreeMap::new()).unwrap();
    match db.fetch(&SyntaxProvider, key) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("revoked parent authority was not refused: {other:?}"),
    }
}
#[test]
fn header_goal_uses_own_generic_identity_and_keeps_nested_type_occurrences_pending() {
    // LRR authority control appended outside the unchanged fixture. It does
    // not replace the official runtime main or assert language acceptance.
    // A compound Type's leaf names are its own retained header occurrences
    // (`&T`); syntax with no such leaf structure (`fn(T)`) stays pending.
    let end = SUFFIX.rfind('}').unwrap();
    let text = format!(
        "{}fn own<T>(n:T) {{}} fn nested<T>(n:&T) {{}} fn pointer<T>(n:fn(T)) {{}}{}",
        &SUFFIX[..end],
        &SUFFIX[end..]
    );
    let (mut db, plan) = world(&text, 2024, false);
    let declarations = declarations(&plan);
    let own = declarations
        .iter()
        .find(|(name, _)| name == "own")
        .unwrap()
        .1;
    let nested = declarations
        .iter()
        .find(|(name, _)| name == "nested")
        .unwrap()
        .1;
    let start = db.trace.events.len();
    let selected = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition: own,
            ordinal: 0,
        },
    ));
    let ParameterTypeGoal::Ready(source) = goal(selected.value.as_ref()).goal() else {
        panic!(
            "actual own generic goal pending: {:?}",
            goal(selected.value.as_ref()).goal()
        )
    };
    assert_eq!(
        source.source(),
        &TyTerm::Param(ParamKey {
            definition: own,
            ordinal: 0
        })
    );
    let compound = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition: nested,
            ordinal: 0,
        },
    ));
    let ParameterTypeGoal::Ready(source) = goal(compound.value.as_ref()).goal() else {
        panic!(
            "compound generic leaf occurrence pending: {:?}",
            goal(compound.value.as_ref()).goal()
        )
    };
    assert_eq!(
        source.source(),
        &TyTerm::Ref(
            crate::traits::term::RegionTerm::Static,
            false,
            Box::new(TyTerm::Param(ParamKey {
                definition: nested,
                ordinal: 0
            }))
        )
    );
    let pointer = declarations
        .iter()
        .find(|(name, _)| name == "pointer")
        .unwrap()
        .1;
    let pending = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition: pointer,
            ordinal: 0,
        },
    ));
    assert!(
        matches!(
            goal(pending.value.as_ref()).goal(),
            ParameterTypeGoal::NeedName(SignatureName::NeedOwnerContext(_))
        ),
        "complex generic occurrence was certified without nested hygiene: {:?}",
        goal(pending.value.as_ref()).goal()
    );
    no_child_code(&db, start);
}

#[test]
fn retained_parameter_ordinals_select_the_actual_nonuniform_header_types_without_child_code() {
    // LRR source-authority control beside the complete unchanged official
    // witness. This adds a declaration only and does not execute another suite.
    let end = SUFFIX.rfind('}').unwrap();
    let text = format!(
        "{}fn ordinals(a:i8,b:u16,c:i64){{}}{}",
        &SUFFIX[..end],
        &SUFFIX[end..]
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = world(&text, edition, false);
        let definition = declarations(&plan)
            .into_iter()
            .find(|(name, _)| name == "ordinals")
            .unwrap()
            .1;
        let start = db.trace.events.len();
        for (ordinal, primitive) in [
            (2, PrimitiveType::I64),
            (0, PrimitiveType::I8),
            (1, PrimitiveType::U16),
        ] {
            let key = QueryKey::DeclaredParameterGoal {
                definition,
                ordinal,
            };
            let first = ready(db.fetch(&SyntaxProvider, key.clone()));
            let ParameterTypeGoal::Ready(source) = goal(first.value.as_ref()).goal() else {
                panic!(
                    "actual parameter ordinal Type is pending: {:?}",
                    goal(first.value.as_ref()).goal()
                )
            };
            assert_eq!(source.source(), &TyTerm::Primitive(primitive));
            assert_eq!(source.ordinal(), ordinal);
            let second = ready(db.fetch(&SyntaxProvider, key));
            assert!(Arc::ptr_eq(&first.value, &second.value));
        }
        match db.fetch(
            &SyntaxProvider,
            QueryKey::DeclaredParameterGoal {
                definition,
                ordinal: 3,
            },
        ) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
            other => panic!("out-of-list parameter ordinal accepted: {other:?}"),
        }
        no_child_code(&db, start);
    }
}
