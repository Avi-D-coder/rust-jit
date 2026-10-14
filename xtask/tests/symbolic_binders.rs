//! Selected official headers and LRR binder admission properties, not compiletest passes.
use lrr::{
    core::digest::{Digest, StableEncode},
    frontend::source_view::function_predicate,
    ir::op::PrimitiveType,
    traits::{
        subst::{Substitution, shift, validate_closed},
        term::{Binder, BoundKind, ConstTerm, GenericArg, ParamKey, RegionTerm, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList},
};

const ORIGINAL: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/where-clauses/higher-ranked-fn-type.rs"
);
fn predicates(source: &str) -> Vec<Binder<TyTerm>> {
    let parsed = ast::SourceFile::parse(source, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::WherePred::cast)
        .map(|p| {
            let captured = lrr::syntax::symbolic_headers::function_predicate(&p).unwrap();
            function_predicate(captured).unwrap()
        })
        .collect()
}

#[test]
fn rustc_original_predicate_and_function_binders_keep_distinct_placement() {
    let terms = predicates(ORIGINAL);
    assert_eq!(terms.len(), 2);
    assert_eq!(terms[0].variables, vec![BoundKind::Region]);
    assert!(terms[1].variables.is_empty());
    assert_ne!(terms[0].fingerprint(), terms[1].fingerprint());
    for term in &terms {
        // Wrap the predicate binder for this type-only admission check. This
        // does not select Foo, type-check called(), or run its empty main.
        let TyTerm::Function(function) = &term.value else {
            panic!("original function pointer lost")
        };
        let TyTerm::Ref(region, _, _) = &function.value.parameters[0] else {
            panic!("original reference lost")
        };
        let expected_depth = u32::from(!term.variables.is_empty());
        assert_eq!(
            *region,
            RegionTerm::Bound {
                depth: expected_depth,
                index: 0
            }
        );
    }
}

#[test]
fn rustc_original_binder_alpha_rename_preserves_canonical_header_identity() {
    // The adaptation is outside the copied-original directory; originals stay
    // unchanged. Replace only the original declared lifetime identifiers.
    let renamed = ORIGINAL
        .replace("'a", "'renamed_a")
        .replace("'b", "'renamed_b");
    let original = predicates(ORIGINAL);
    let changed = predicates(&renamed);
    for (a, b) in original.iter().zip(&changed) {
        assert_eq!(a.fingerprint(), b.fingerprint());
    }
    let TyTerm::Function(function) = &original[1].value else {
        panic!("function lost")
    };
    let mut free = function.clone();
    let TyTerm::Ref(region, _, _) = &mut free.value.parameters[0] else {
        panic!("reference lost")
    };
    *region = RegionTerm::Static;
    assert_ne!(function.fingerprint(), free.fingerprint());
    validate_closed(&TyTerm::Function(function.clone())).unwrap();
}

#[test]
fn source_binder_substitution_shifts_without_capturing_and_preserves_namespaces() {
    let parameter = ParamKey {
        definition: Digest::of_bytes(ORIGINAL.as_bytes()),
        ordinal: 0,
    };
    let mut substitution = Substitution::default();
    let region = RegionTerm::Bound { depth: 0, index: 0 };
    substitution
        .insert(
            parameter,
            GenericArg::Type(TyTerm::Ref(
                region.clone(),
                false,
                Box::new(TyTerm::Tuple(vec![])),
            )),
        )
        .unwrap();
    substitution
        .insert(parameter, GenericArg::Region(RegionTerm::Static))
        .unwrap();
    substitution
        .insert(parameter, GenericArg::Const(ConstTerm::Value(7)))
        .unwrap();
    let mut function = match &predicates(ORIGINAL)[1].value {
        TyTerm::Function(f) => f.clone(),
        _ => panic!("function lost"),
    };
    function.value.parameters.push(TyTerm::Param(parameter));
    let applied = substitution.apply(&TyTerm::Function(function)).unwrap();
    let TyTerm::Function(function) = applied else {
        panic!("function lost")
    };
    let TyTerm::Ref(actual, _, _) = &function.value.parameters[1] else {
        panic!("inserted reference lost")
    };
    assert_eq!(*actual, RegionTerm::Bound { depth: 1, index: 0 });
    // This outer bound variable has no surrounding binder here. It must not
    // become the function's inner region just because their indices agree.
    assert!(validate_closed(&TyTerm::Function(function)).is_err());
    assert_eq!(
        substitution
            .apply(&TyTerm::Array(
                Box::new(TyTerm::Primitive(PrimitiveType::U8)),
                ConstTerm::Param(parameter)
            ))
            .unwrap(),
        TyTerm::Array(
            Box::new(TyTerm::Primitive(PrimitiveType::U8)),
            ConstTerm::Value(7)
        )
    );
    assert!(
        substitution
            .insert(parameter, GenericArg::Const(ConstTerm::Value(9)))
            .is_err()
    );
    let before = substitution.apply(&TyTerm::Param(parameter)).unwrap();
    assert!(shift(&before, u32::MAX, 0).is_err());
    assert_eq!(
        substitution.apply(&TyTerm::Param(parameter)).unwrap(),
        before
    );
}

#[test]
fn rustc_original_direct_self_reference_recipe_rejects_occurs_binding_atomically() {
    use lrr::traits::{term::InferVarKey, unify::Unifier};
    const ORIGINAL: &str = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/type-inference/direct-self-reference-occurs-check.rs"
    );
    let parsed = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let assignment = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
        .find(|b| b.op_token().is_some_and(|t| t.text() == "="))
        .unwrap();
    let lhs = assignment.lhs().unwrap().syntax().text().to_string();
    let ast::Expr::CallExpr(constructor) = assignment.rhs().unwrap() else {
        panic!("original constructor lost")
    };
    let args: Vec<_> = constructor.arg_list().unwrap().args().collect();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].syntax().text().to_string(), lhs);
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    // Source-derived recipe adaptation for occurs-check only. This identity is
    // not a bound source Box witness or execution of the original assignment.
    let nominal = Digest::of_bytes(constructor.syntax().text().to_string().as_bytes());
    let candidate = TyTerm::Nominal(nominal, vec![GenericArg::Type(TyTerm::Infer(variable))]);
    let mut unifier = Unifier::new(transaction);
    assert_eq!(
        unifier
            .unify(&TyTerm::Infer(variable), &candidate)
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(unifier.binding(variable).is_none());
    // No corrupt prefix prevents a later ordinary local constraint.
    unifier
        .unify(&TyTerm::Infer(variable), &TyTerm::Tuple(vec![]))
        .unwrap();
    assert_eq!(unifier.binding(variable), Some(&TyTerm::Tuple(vec![])));
}

#[test]
fn original_function_binder_cannot_enter_legacy_unifier_or_publish_a_binding() {
    use lrr::traits::{term::InferVarKey, unify::Unifier};
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let mut unifier = Unifier::new(transaction);
    let original = &predicates(ORIGINAL)[1].value;
    assert_eq!(
        unifier
            .unify(&TyTerm::Infer(variable), original)
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(unifier.binding(variable).is_none());
    // Deliberate refusal until universe/placeholder matching is implemented.
    // This is not the original quiet/verbose trait error or a TR002 full pass.
}

#[test]
fn original_universal_region_instantiation_cannot_leak_to_an_outer_variable() {
    use lrr::traits::{
        subst::instantiate_binder,
        term::{InferVarKey, PlaceholderKey},
        unify::Unifier,
    };
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let source = predicates(ORIGINAL);
    let instantiated = instantiate_binder(&source[0], transaction, 1).unwrap();
    let TyTerm::Function(function) = instantiated else {
        panic!("original function lost")
    };
    let reference = function.value.parameters[0].clone();
    let TyTerm::Ref(region, _, _) = &reference else {
        panic!("original reference lost")
    };
    assert_eq!(
        *region,
        RegionTerm::Placeholder(PlaceholderKey {
            transaction,
            universe: 1,
            index: 0
        })
    );
    let outer = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let inner = InferVarKey {
        transaction,
        ordinal: 1,
    };
    let mut unifier = Unifier::new(transaction);
    assert_eq!(unifier.fresh_universe().unwrap(), 1);
    unifier.register_variable(outer, 0).unwrap();
    unifier.register_variable(inner, 1).unwrap();
    // The last tuple constraint binds a local prefix before the older variable
    // encounters the original higher-ranked reference's fresh placeholder.
    assert!(
        unifier
            .unify(
                &TyTerm::Tuple(vec![TyTerm::Infer(outer), TyTerm::Infer(inner)]),
                &TyTerm::Tuple(vec![reference.clone(), TyTerm::Tuple(vec![])])
            )
            .is_err()
    );
    assert!(unifier.binding(outer).is_none() && unifier.binding(inner).is_none());
    unifier.unify(&TyTerm::Infer(inner), &reference).unwrap();
    assert_eq!(unifier.binding(inner), Some(&reference));
    assert!(
        unifier
            .unify(&TyTerm::Infer(outer), &TyTerm::Infer(inner))
            .is_err()
    );
    assert!(unifier.binding(outer).is_none());
    // The source where predicate's Foo selection and quiet/verbose errors are
    // still pending; this exercises local substitution safety only.
}

#[test]
fn lowered_local_universe_remains_restricted_after_failed_placeholder_capture() {
    use lrr::traits::{subst::instantiate_binder, term::InferVarKey, unify::Unifier};
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let TyTerm::Function(function) =
        instantiate_binder(&predicates(ORIGINAL)[0], transaction, 1).unwrap()
    else {
        panic!("function lost")
    };
    let outer = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let local = InferVarKey {
        transaction,
        ordinal: 1,
    };
    let mut unifier = Unifier::new(transaction);
    assert_eq!(unifier.fresh_universe().unwrap(), 1);
    unifier.register_variable(outer, 0).unwrap();
    unifier.register_variable(local, 1).unwrap();
    unifier
        .unify(&TyTerm::Infer(outer), &TyTerm::Infer(local))
        .unwrap();
    assert_eq!(unifier.variable_universe(local), 0);
    assert!(
        unifier
            .unify(&TyTerm::Infer(local), &function.value.parameters[0])
            .is_err()
    );
    assert!(unifier.binding(local).is_none());
    assert_eq!(unifier.variable_universe(local), 0);
    unifier
        .unify(&TyTerm::Infer(local), &TyTerm::Tuple(vec![]))
        .unwrap();
}

#[test]
fn original_where_header_param_environment_retains_unforced_trait_assumption() {
    use lrr::traits::term::{ParamEnv, TraitRef};
    let original = predicates(ORIGINAL);
    let source_owner = Digest::of_bytes(ORIGINAL.as_bytes());
    let captured = Binder {
        variables: original[0].variables.clone(),
        value: TraitRef {
            trait_key: source_owner,
            self_ty: original[0].value.clone(),
            args: vec![],
        },
    };
    // Source-derived metadata adaptation. The key is not a selected Foo impl;
    // construction validates structure and cannot fulfill a goal or execute it.
    let env = ParamEnv::new(source_owner, vec![], vec![captured.clone()]).unwrap();
    assert_eq!(env.predicates(), &[captured]);
    let invalid = Binder {
        variables: vec![BoundKind::Type],
        value: env.predicates()[0].value.clone(),
    };
    assert!(ParamEnv::new(source_owner, vec![], vec![invalid]).is_err());
    assert_eq!(env.predicates().len(), 1);
    let temporary = lrr::traits::subst::instantiate_binder(&original[0], source_owner, 1).unwrap();
    let transaction_fact = Binder {
        variables: vec![],
        value: TraitRef {
            trait_key: source_owner,
            self_ty: temporary,
            args: vec![],
        },
    };
    assert!(ParamEnv::new(source_owner, vec![], vec![transaction_fact]).is_err());
    assert_eq!(env.predicates().len(), 1);
}

#[test]
fn original_placeholder_cannot_leak_by_redeclaring_an_implicit_variable() {
    use lrr::traits::{subst::instantiate_binder, term::InferVarKey, unify::Unifier};
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let TyTerm::Function(function) =
        instantiate_binder(&predicates(ORIGINAL)[0], transaction, 1).unwrap()
    else {
        panic!("function lost")
    };
    let outer = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let nested = InferVarKey {
        transaction,
        ordinal: 1,
    };
    let mut unifier = Unifier::new(transaction);
    assert_eq!(unifier.fresh_universe().unwrap(), 1);
    unifier
        .unify(
            &TyTerm::Infer(outer),
            &TyTerm::Tuple(vec![TyTerm::Infer(nested)]),
        )
        .unwrap();
    // Both variables entered as legacy root-universe variables. The nested
    // variable must not acquire permission to capture the original universal
    // lifetime after its occurrence has already been published through outer.
    let previous = unifier.binding(outer).unwrap().clone();
    assert!(unifier.register_variable(nested, 1).is_err());
    assert_eq!(unifier.variable_universe(nested), 0);
    assert!(
        unifier
            .unify(&TyTerm::Infer(nested), &function.value.parameters[0])
            .is_err()
    );
    assert!(unifier.binding(nested).is_none());
    assert_eq!(unifier.binding(outer), Some(&previous));
}

#[test]
fn original_universal_placeholder_cannot_be_relabelled_as_root_scope() {
    use lrr::traits::{subst::instantiate_binder, term::InferVarKey, unify::Unifier};
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let TyTerm::Function(function) =
        instantiate_binder(&predicates(ORIGINAL)[0], transaction, 1).unwrap()
    else {
        panic!("function lost")
    };
    let mut reference = function.value.parameters[0].clone();
    let TyTerm::Ref(RegionTerm::Placeholder(key), _, _) = &mut reference else {
        panic!("original placeholder lost")
    };
    // An admission corruption, outside unchanged originals: a universal
    // placeholder cannot claim the root scope reserved for legacy variables.
    key.universe = 0;
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let mut unifier = Unifier::new(transaction);
    assert!(unifier.unify(&TyTerm::Infer(variable), &reference).is_err());
    assert!(unifier.binding(variable).is_none());
    unifier
        .unify(
            &TyTerm::Infer(variable),
            &TyTerm::Primitive(PrimitiveType::Unit),
        )
        .unwrap();
    assert_eq!(
        unifier.binding(variable),
        Some(&TyTerm::Primitive(PrimitiveType::Unit))
    );
}

#[test]
fn original_namespace_substitution_rejection_preserves_accumulated_budget() {
    let owner = Digest::of_bytes(ORIGINAL.as_bytes());
    let first = ParamKey {
        definition: owner,
        ordinal: 0,
    };
    let second = ParamKey {
        definition: owner,
        ordinal: 1,
    };
    let mut substitution = Substitution::default();
    substitution
        .insert(
            first,
            GenericArg::Type(TyTerm::Primitive(PrimitiveType::Unit)),
        )
        .unwrap();
    let before = substitution.apply(&TyTerm::Param(first)).unwrap();
    let overflow = TyTerm::Tuple(vec![TyTerm::Primitive(PrimitiveType::Unit); 99_998]);
    assert!(overflow.heap_bytes().is_some());
    let error = substitution
        .insert(second, GenericArg::Type(overflow))
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(substitution.apply(&TyTerm::Param(first)).unwrap(), before);
    // Failed admission must leave both namespace identity and accounting usable.
    substitution
        .insert(
            second,
            GenericArg::Type(TyTerm::Primitive(PrimitiveType::Bool)),
        )
        .unwrap();
    substitution
        .insert(first, GenericArg::Region(RegionTerm::Static))
        .unwrap();
    assert_eq!(
        substitution.apply(&TyTerm::Param(second)).unwrap(),
        TyTerm::Primitive(PrimitiveType::Bool)
    );
    assert_eq!(substitution.apply(&TyTerm::Param(first)).unwrap(), before);
}
