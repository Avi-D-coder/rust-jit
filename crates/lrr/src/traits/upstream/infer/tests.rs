//! Supplemental LRR route/isolation controls using actual upstream fixture IR.
//! This is storage execution, never a trait answer or an upstream main pass.
use super::{InferenceStorage, DbInterner, Ty, Span, Const, Region};
use super::unify_key::ConstVariableValue;
use crate::traits::upstream::model::{ValueConst, ScalarInt};
use rustc_type_ir::{self as ty, Interner, TypeVisitableExt, inherent::IntoKind};

pub(crate) fn check_actual_storage<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    assert!(!span.is_dummy(), "actual retained declaration range supplies inference origin");
    check_placeholder_assumptions(cx, actual, span);
    check_solver_region_transactions(cx, span);
    check_current_region_collector(cx, span);
    check_actual_leak_checker(cx, span);
    check_obligation_state(cx, actual, span);
    check_variable_resolution(cx, actual, span);
    check_inference_client(cx, actual, span);
    check_canonical_instantiation(cx, actual, span);
    check_goal_fast_path(cx, actual, span);
    check_solver_delegate(cx, actual, span);
    check_pinned_bottom_up_folder(cx, actual, span);
    check_pinned_discriminant_type(cx, actual, span);
    let storage = InferenceStorage::new();
    let root = ty::UniverseIndex::ROOT;
    let (a, b, integer, float, constant, first_region, second_region) = {
        let mut inner = storage.inner.borrow_mut();
        let a = inner.type_variables().new_var(root, span);
        let b = inner.type_variables().new_var(root, span);
        let integer = inner.int_unification_table().new_key(ty::IntVarValue::Unknown);
        let float = inner.float_unification_table().new_key(ty::FloatVarValue::Unknown);
        let constant = inner.const_unification_table().new_key(ConstVariableValue::Unknown { span, universe: root });
        let first_region = inner.unwrap_region_constraints().new_region_var(root, span);
        let second_region = inner.unwrap_region_constraints().new_region_var(root, span);
        (a, b, integer, float, constant, first_region, second_region)
    };
    let value = Const::new(cx, ty::ConstKind::Value(ValueConst {
        ty: cx.ty(ty::TyKind::Bool), tree: cx.valtree(ty::ValTreeKind::Leaf(ScalarInt { bits: 1, bytes: 1 })),
    }));
    let first = Region::new_var(cx, first_region);
    let second = Region::new_var(cx, second_region);
    let placeholder = cx.intern_region(ty::RegionKind::RePlaceholder(ty::PlaceholderRegion::new_anon(root.next_universe(), ty::BoundVar::from_u32(0))));
    let outer = storage.start_snapshot();
    assert_eq!(storage.num_open_snapshots(), 1);
    assert_ne!(storage.create_next_universe(), root);
    {
        let mut inner = storage.inner.borrow_mut();
        inner.type_variables().equate(a, b);
        inner.type_variables().instantiate(a, actual);
        assert_eq!(inner.type_variables().probe(b).known(), Some(actual));
        inner.int_unification_table().unify_var_value(integer, ty::IntVarValue::IntType(ty::IntTy::I32)).unwrap();
        inner.float_unification_table().unify_var_value(float, ty::FloatVarValue::Known(ty::FloatTy::F64)).unwrap();
        inner.const_unification_table().union_value(constant, ConstVariableValue::Known { value });
        inner.unwrap_region_constraints().make_eqregion(span, first, second, ty::solve::VisibleForLeakCheck::Yes);
    }
    assert!(storage.region_constraints_added_in_snapshot(&outer));
    // A nested unsuccessful candidate allocates and mutates state before the
    // real upstream region table reports an unnameable placeholder; commit_if_ok rolls it back.
    let failed: Result<(), ()> = storage.commit_if_ok(|_| {
        assert_eq!(storage.num_open_snapshots(), 2);
        storage.create_next_universe();
        let mut inner = storage.inner.borrow_mut();
        let fresh = inner.type_variables().new_var(root, span);
        inner.type_variables().instantiate(fresh, actual);
        inner.const_unification_table().new_key(ConstVariableValue::Known { value });
        inner.unwrap_region_constraints().new_region_var(root, span);
        let super::InferCtxtInner { region_constraint_storage, undo_log, .. } = &mut *inner;
        region_constraint_storage.as_mut().unwrap().unification_table.with_log(undo_log)
            .unify_var_value(first_region, super::unify_key::RegionVariableValue::Known { value: placeholder, span: None })
            .map_err(|_| ())
    });
    assert!(failed.is_err());
    assert_eq!(storage.universe(), root.next_universe());
    {
        let mut inner = storage.inner.borrow_mut();
        assert_eq!(inner.type_variables().num_vars(), 2);
        assert_eq!(inner.type_variables().probe(b).known(), Some(actual));
        assert_eq!(inner.const_unification_table().len(), 1);
        assert_eq!(inner.const_unification_table().probe_value(constant).known(), Some(value));
        assert_eq!(inner.unwrap_region_constraints().num_region_vars(), 2);
        assert_eq!(inner.int_unification_table().probe_value(integer), ty::IntVarValue::IntType(ty::IntTy::I32));
    }
    // Nested successful commits must still be undone by the outer snapshot.
    storage.commit_if_ok::<(), (), _>(|_| {
        storage.inner.borrow_mut().type_variables().new_var(root, span);
        Ok(())
    }).unwrap();
    storage.rollback_to(outer);
    assert_eq!(storage.universe(), root);
    assert!(!storage.in_snapshot());
    {
        let mut inner = storage.inner.borrow_mut();
        assert_eq!(inner.type_variables().num_vars(), 2);
        assert!(inner.type_variables().probe(a).is_unknown());
        assert!(inner.type_variables().probe(b).is_unknown());
        assert_ne!(inner.type_variables().root_var(a), inner.type_variables().root_var(b));
        assert_eq!(inner.int_unification_table().probe_value(integer), ty::IntVarValue::Unknown);
        assert_eq!(inner.float_unification_table().probe_value(float), ty::FloatVarValue::Unknown);
        assert!(inner.const_unification_table().probe_value(constant).known().is_none());
        let mut regions = inner.unwrap_region_constraints();
        assert!(regions.region_constraint_data().is_empty());
        assert_eq!(regions.shallow_resolve_region_var(cx, first_region), first);
        assert_eq!(regions.shallow_resolve_region_var(cx, second_region), second);
    }
    storage.commit_if_ok::<(), (), _>(|_| {
        storage.inner.borrow_mut().type_variables().instantiate(a, actual);
        Ok(())
    }).unwrap();
    assert_eq!(storage.inner.borrow_mut().type_variables().probe(a).known(), Some(actual));
    assert!(!storage.in_snapshot());
    let before = storage.inner.borrow_mut().type_variables().num_vars();
    storage.probe(|_| { storage.inner.borrow_mut().type_variables().new_var(root, span); });
    assert_eq!(storage.inner.borrow_mut().type_variables().num_vars(), before);
    assert!(cx.services().ready(cx).is_err(), "actual storage execution grants no full-fact readiness");
}

pub(crate) fn check_variable_resolution<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use crate::traits::upstream::model::{AllocateList, List};
    use rustc_type_ir::inherent::Ty as _;
    let context = super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let a = context.next_ty_vid(span); let b = context.next_ty_vid(span);
    context.inner.borrow_mut().type_variables().equate(a, b);
    let root = context.root_var(a);
    assert_eq!(context.shallow_resolve(Ty::new_var(cx, a)), Ty::new_var(cx, root));
    assert_eq!(context.shallow_resolve(Ty::new_var(cx, b)), Ty::new_var(cx, root));
    let integer = context.next_int_vid(); let other_integer = context.next_int_vid();
    context.inner.borrow_mut().int_unification_table().union(integer, other_integer);
    assert_eq!(context.shallow_resolve(Ty::new_infer(cx, ty::InferTy::IntVar(integer))),
        context.shallow_resolve(Ty::new_infer(cx, ty::InferTy::IntVar(other_integer))));
    let float = context.next_float_vid(); let other_float = context.next_float_vid();
    context.inner.borrow_mut().float_unification_table().union(float, other_float);
    assert_eq!(context.shallow_resolve(Ty::new_infer(cx, ty::InferTy::FloatVar(float))),
        context.shallow_resolve(Ty::new_infer(cx, ty::InferTy::FloatVar(other_float))));
    let c = context.next_const_vid(span); let d = context.next_const_vid(span);
    context.inner.borrow_mut().const_unification_table().union(c, d);
    assert_eq!(context.shallow_resolve_const(Const::new_var(cx, c)), context.shallow_resolve_const(Const::new_var(cx, d)));
    let snapshot = context.start_snapshot();
    context.inner.borrow_mut().type_variables().instantiate(a, Ty::new_infer(cx, ty::InferTy::IntVar(integer)));
    context.inner.borrow_mut().int_unification_table().union_value(integer, ty::IntVarValue::IntType(ty::IntTy::I32));
    context.inner.borrow_mut().float_unification_table().union_value(float, ty::FloatVarValue::Known(ty::FloatTy::F64));
    let value = Const::new(cx, ty::ConstKind::Value(ValueConst { ty: cx.ty(ty::TyKind::Uint(ty::UintTy::Usize)), tree: cx.valtree(ty::ValTreeKind::Leaf(ScalarInt { bits: 3, bytes: 8 })) }));
    context.inner.borrow_mut().const_unification_table().union_value(c, ConstVariableValue::Known { value });
    assert_eq!(context.shallow_resolve_ty_var(a), Ty::new_infer(cx, ty::InferTy::IntVar(integer)));
    assert_eq!(context.shallow_resolve(Ty::new_var(cx, b)), cx.ty(ty::TyKind::Int(ty::IntTy::I32)));
    assert_eq!(context.shallow_resolve_float_var(other_float), cx.ty(ty::TyKind::Float(ty::FloatTy::F64)));
    assert_eq!(context.shallow_resolve_const(Const::new_var(cx, d)), value);
    let region = context.next_region_var(span);
    let tuple = cx.ty(ty::TyKind::Tuple(cx.list(vec![Ty::new_var(cx, b), actual,
        cx.ty(ty::TyKind::Ref(region, Ty::new_infer(cx, ty::InferTy::FloatVar(float)), ty::Mutability::Not))])));
    let resolved = context.deeply_resolve_ignoring_regions(tuple);
    let expected = cx.ty(ty::TyKind::Tuple(cx.list(vec![cx.ty(ty::TyKind::Int(ty::IntTy::I32)), actual,
        cx.ty(ty::TyKind::Ref(region, cx.ty(ty::TyKind::Float(ty::FloatTy::F64)), ty::Mutability::Not))])));
    assert_eq!(resolved, expected);
    assert_eq!(context.deeply_resolve_ignoring_regions(resolved), resolved);
    let array = cx.ty(ty::TyKind::Array(actual, Const::new_var(cx, c)));
    assert_eq!(context.deeply_resolve_ignoring_regions(array), cx.ty(ty::TyKind::Array(actual, value)));
    context.rollback_to(snapshot);
    assert_eq!(context.shallow_resolve(Ty::new_var(cx, a)), Ty::new_var(cx, root));
    assert!(context.try_resolve_const_var(c).is_err());
    // Supplemental bound-kind transport: all three namespaces pass through
    // the actual shared BoundVarReplacer, not another substitution walker.
    let vars = cx.mk_bound_variable_kinds_from_iter([
        ty::BoundVariableKind::Ty(ty::BoundTyKind::Anon),
        ty::BoundVariableKind::Region(ty::BoundRegionKind::Anon), ty::BoundVariableKind::Const]);
    let bound_ty = Ty::new_anon_bound(cx, ty::INNERMOST, ty::BoundVar::from_u32(0));
    let bound_region = Region::new_bound(cx, ty::INNERMOST, ty::BoundRegion { var: ty::BoundVar::from_u32(1), kind: ty::BoundRegionKind::Anon });
    let bound_const = Const::new_bound(cx, ty::INNERMOST, ty::BoundConst::new(ty::BoundVar::from_u32(2)));
    let bound_value = cx.ty(ty::TyKind::Tuple(cx.list(vec![bound_ty,
        cx.ty(ty::TyKind::Ref(bound_region, actual, ty::Mutability::Not)), cx.ty(ty::TyKind::Array(actual, bound_const))])));
    let binder = ty::Binder::bind_with_vars(bound_value, vars);
    // Compiler forall uses the same shared binder folder, with rigid placeholders
    // in a new universe rather than allocating fresh inference variables.
    let universe = context.universe();
    let ty_vars = context.num_ty_vars();
    let rigid = context.enter_forall_and_leak_universe(binder);
    let rigid_universe = universe.next_universe();
    assert_eq!(context.universe(), rigid_universe);
    assert_eq!(context.num_ty_vars(), ty_vars);
    let ty::TyKind::Tuple(fields) = rigid.kind() else { panic!() };
    let ty::TyKind::Placeholder(p) = fields.items()[0].kind() else { panic!() };
    assert_eq!(p.universe, rigid_universe); assert_eq!(p.bound.var, ty::BoundVar::from_u32(0));
    let ty::TyKind::Ref(r, inner, _) = fields.items()[1].kind() else { panic!() };
    assert_eq!(inner, actual);
    let ty::RegionKind::RePlaceholder(p) = r.kind() else { panic!() };
    assert_eq!(p.universe, rigid_universe); assert_eq!(p.bound.var, ty::BoundVar::from_u32(1));
    let ty::TyKind::Array(inner, c) = fields.items()[2].kind() else { panic!() };
    assert_eq!(inner, actual);
    let ty::ConstKind::Placeholder(p) = c.kind() else { panic!() };
    assert_eq!(p.universe, rigid_universe); assert_eq!(p.bound.var, ty::BoundVar::from_u32(2));
    let entered = std::cell::Cell::new(0);
    let again = context.enter_forall(binder, |value| { entered.set(entered.get() + 1); value });
    assert_eq!(entered.get(), 1);
    assert_ne!(again, rigid);
    let last_universe = context.universe();
    assert_eq!(last_universe, rigid_universe.next_universe());
    assert_eq!(context.enter_forall(ty::Binder::dummy(actual), |value| value), actual);
    assert_eq!(context.universe(), last_universe);
    assert_eq!(context.num_ty_vars(), ty_vars);
    let before = context.num_ty_vars();
    let fresh = context.instantiate_binder_with_fresh_vars(span, super::variables::BoundRegionConversionTime::HigherRankedType, binder);
    assert_eq!(context.num_ty_vars(), before + 1);
    let ty::TyKind::Tuple(fields) = fresh.kind() else { panic!() };
    assert!(matches!(fields.items()[0].kind(), ty::TyKind::Infer(ty::InferTy::TyVar(_))));
    assert!(matches!(fields.items()[1].kind(), ty::TyKind::Ref(r, _, _) if r.is_var()));
    assert!(matches!(fields.items()[2].kind(), ty::TyKind::Array(_, c) if c.is_ct_var()));
    let plain = ty::Binder::bind_with_vars(actual, List::default());
    let before = context.num_ty_vars();
    assert_eq!(context.instantiate_binder_with_fresh_vars(span, super::variables::BoundRegionConversionTime::FnCall, plain), actual);
    assert_eq!(context.num_ty_vars(), before);
    assert_eq!(context.tainted_by_errors(), None);
    let error = cx.delay_bug("LRR inference client error-taint route control");
    let error_ty = Ty::new_error(cx, error);
    assert_eq!(context.deeply_resolve_ignoring_regions(error_ty), error_ty);
    assert_eq!(context.tainted_by_errors(), Some(error));
    assert!(cx.services().ready(cx).is_err());
}

pub(crate) fn check_actual_fresh_arguments<'cx>(cx: DbInterner<'cx>, id: crate::traits::upstream::identity::DefinitionId, span: Span) {
    use rustc_type_ir::inherent::*;
    use crate::traits::upstream::model::{GenericParameter, Args, Arg};
    let context = super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let generics = cx.generics_of(id);
    let snapshot = context.start_snapshot();
    let args = context.fresh_args_for_item(span, id);
    let again = context.fresh_args_for_item(span, id);
    assert_eq!(args.items().len(), generics.count());
    for (i, ((parameter, argument), other)) in generics.parameters.iter().zip(args.items()).zip(again.items()).enumerate() {
        assert_eq!(parameter.index(), i as u32);
        assert_ne!(argument, other, "each actual source generic argument is freshly instantiated");
        match (parameter, argument) {
            (GenericParameter::Type(_), Arg::Type(ty)) => {
                let ty::TyKind::Infer(ty::InferTy::TyVar(vid)) = ty.kind() else { panic!() };
                assert_eq!(context.type_var_span(vid), span);
                assert_eq!(context.try_resolve_ty_var(vid), Err(ty::UniverseIndex::ROOT));
            }
            (GenericParameter::Region(_), Arg::Region(region)) => assert!(region.is_var()),
            (GenericParameter::Const(_, _), Arg::Const(c)) => {
                let ty::ConstKind::Infer(ty::InferConst::Var(vid)) = c.kind() else { panic!() };
                assert_eq!(context.const_var_span(vid), Some(span));
                assert_eq!(context.try_resolve_const_var(vid), Err(ty::UniverseIndex::ROOT));
            }
            _ => panic!("fresh argument namespace differs from actual source generic table"),
        }
    }
    let identity = Args::identity_for_item(cx, id);
    assert_eq!(Args::extend_with_error(cx, id, identity.items()), identity,
        "identity and already-complete error extension share the same parameter constructor");
    context.rollback_to(snapshot);
    assert_eq!(context.num_ty_vars(), 0); assert_eq!(context.num_region_vars(), 0);
    assert_eq!(context.inner.borrow_mut().const_unification_table().len(), 0);
    assert!(cx.services().ready(cx).is_err());
}

/// Supplemental host IR controls. The source Type/Cx/span are the real fixture
/// inputs; these relation graphs are not a stock higher-ranked runtime program.
pub(crate) fn check_placeholder_assumptions<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use ty::region_constraint::{Assumptions, TransitiveRelationBuilder,
        regions_outlived_by, regions_outliving};
    let context = super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let root = context.universe();
    let empty = Assumptions::<DbInterner<'cx>>::empty();
    assert!(empty.type_outlives.is_empty());
    assert_eq!(empty.region_outlives.base_edges().count(), 0);
    assert_eq!(empty.inverse_region_outlives.base_edges().count(), 0);
    let first_universe = context.create_next_universe();
    let regions = [0, 1, 2].map(|index| cx.intern_region(ty::RegionKind::RePlaceholder(
        ty::PlaceholderRegion::new_anon(first_universe, ty::BoundVar::from_u32(index)))));
    let [first, second, third] = regions;
    let mut builder = TransitiveRelationBuilder::default();
    builder.add(first, second); builder.add(second, third);
    let original = Assumptions::new_unelaborated(
        vec![ty::Binder::dummy(ty::OutlivesClause(actual, first))], builder.freeze());
    context.insert_placeholder_assumptions(first_universe, original.clone());
    let assumptions = context.get_placeholder_assumptions(first_universe);
    assert_eq!(assumptions.type_outlives, original.type_outlives);
    assert_eq!(assumptions.region_outlives.base_edges().count(), 2);
    assert!(assumptions.region_outlives.contains(first, third));
    assert!(!assumptions.region_outlives.contains(third, first));
    assert!(assumptions.inverse_region_outlives.contains(third, first));
    assert!(!assumptions.inverse_region_outlives.contains(first, third));
    let outlived = regions_outlived_by(first, &assumptions).collect::<Vec<_>>();
    assert_eq!(outlived.len(), 3);
    assert!(regions.iter().all(|r| outlived.contains(r)));
    let outliving = regions_outliving(third, &assumptions, cx).collect::<Vec<_>>();
    assert_eq!(outliving.len(), 4);
    assert!(regions.iter().all(|r| outliving.contains(r)));
    assert!(outliving.contains(&Region::new_static(cx)));
    assert!(!context.has_placeholder_assumptions(std::range::RangeInclusive { start: root, last: first_universe }));
    assert!(context.has_placeholder_assumptions(std::range::RangeInclusive { start: first_universe, last: first_universe }));
    let second_universe = context.create_next_universe();
    assert!(!context.has_placeholder_assumptions(std::range::RangeInclusive { start: first_universe, last: second_universe }));
    context.insert_placeholder_assumptions(second_universe, empty);
    assert!(context.has_placeholder_assumptions(std::range::RangeInclusive { start: first_universe, last: second_universe }));
    // Replacing one universe's assumptions must not mutate another or clones.
    context.insert_placeholder_assumptions(first_universe, Assumptions::empty());
    assert!(context.get_placeholder_assumptions(first_universe).type_outlives.is_empty());
    assert!(assumptions.region_outlives.contains(first, third));
    assert_eq!(original.type_outlives.len(), 1);
    assert!(context.get_placeholder_assumptions(second_universe).region_outlives.base_edges().next().is_none());
    assert!(!span.is_dummy());
    assert!(cx.services().ready(cx).is_err(), "assumptions grant no source facts/root readiness");
}

/// Supplemental transaction/span controls, using actual source Cx and regions.
/// These verify the compiler's shared storage and combined undo path; they do
/// not solve an outlives goal or reproduce borrow checking.
fn check_solver_region_transactions<'cx>(cx: DbInterner<'cx>, span: Span) {
    use super::solver_region_constraints::SolverRegionConstraint;
    use ty::region_constraint::LeafRegionConstraint;
    let context = super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    assert!(context.get_solver_region_constraint().is_true());
    let first = context.next_region_var(span);
    let second = context.next_region_var(span);
    let third = context.next_region_var(span);
    let original = SolverRegionConstraint::new_leaf(
        LeafRegionConstraint::RegionOutlives(first, second, span));
    context.register_solver_region_constraint(original.clone());
    assert_eq!(context.get_solver_region_constraint(), original);
    assert!(!context.in_snapshot());
    let outer = context.start_snapshot();
    let next = SolverRegionConstraint::new_leaf(
        LeafRegionConstraint::RegionOutlives(second, third, span));
    context.register_solver_region_constraint(next);
    let combined = context.get_solver_region_constraint();
    assert_eq!(combined.and_constraint.0.len(), 2);
    assert!(combined.and_constraint.0.iter().any(|leaf| matches!(leaf,
        LeafRegionConstraint::RegionOutlives(a,b,s) if *a == first && *b == second && *s == span)));
    assert!(combined.and_constraint.0.iter().any(|leaf| matches!(leaf,
        LeafRegionConstraint::RegionOutlives(a,b,s) if *a == second && *b == third && *s == span)));
    let root = context.universe();
    let failed: Result<(), ()> = context.commit_if_ok(|_| {
        context.overwrite_solver_region_constraint(SolverRegionConstraint::new_false());
        context.register_solver_region_constraint(SolverRegionConstraint::new_ambig(span));
        context.create_next_universe();
        context.next_region_var(span);
        assert!(context.get_solver_region_constraint().is_false());
        Err(())
    });
    assert!(failed.is_err());
    assert_eq!(context.universe(), root);
    assert_eq!(context.num_region_vars(), 3);
    assert_eq!(context.get_solver_region_constraint(), combined);
    // A committed inner mutation is still owned by its enclosing snapshot.
    context.commit_if_ok::<(), (), _>(|_| {
        context.overwrite_solver_region_constraint(SolverRegionConstraint::new_false());
        Ok(())
    }).unwrap();
    assert!(context.get_solver_region_constraint().is_false());
    context.rollback_to(outer);
    assert_eq!(context.get_solver_region_constraint(), original);
    assert!(!context.in_snapshot());
    context.probe(|_| {
        context.overwrite_solver_region_constraint(SolverRegionConstraint::new_false());
    });
    assert_eq!(context.get_solver_region_constraint(), original);
    // Upstream canonicalization deduplicates a repeated leaf independent of span.
    let other_span = Span { origin: span.origin, start: span.end, end: span.end };
    context.register_solver_region_constraint(SolverRegionConstraint::new_leaf(
        LeafRegionConstraint::RegionOutlives(first, second, other_span)));
    assert_eq!(context.get_solver_region_constraint().and_constraint.0.len(), 1);
    context.register_solver_region_constraint(SolverRegionConstraint::new_leaf(
        LeafRegionConstraint::RegionOutlives(first, first, span)));
    assert_eq!(context.get_solver_region_constraint().and_constraint.0.len(), 1);
    super::solver_region_constraints_tests::canonicalization_preserves_only_one_ambiguity(cx, span, other_span);
    context.commit_if_ok::<(), (), _>(|_| {
        context.overwrite_solver_region_constraint(SolverRegionConstraint::new_ambig(span));
        Ok(())
    }).unwrap();
    assert!(context.get_solver_region_constraint().is_ambig());
    let constraint = context.get_solver_region_constraint();
    assert_eq!(constraint.and_constraint.0[0].span(), span);
    assert_eq!(constraint.clone().without_spans().with_spans(span), constraint);
    assert!(super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis)
        .get_solver_region_constraint().is_true());
    assert!(cx.services().ready(cx).is_err(), "region state grants no source facts/root readiness");
}

/// Host route/transaction witness over the real source context and source span.
/// Explicit placeholder inputs exercise the pinned collector, not a Rust parser,
/// static checker or actual trait root. No leak-check outcome is fabricated.
fn check_current_region_collector<'cx>(cx: DbInterner<'cx>, span: Span) {
    use super::region_constraints::ConstraintKind;
    use ty::solve::VisibleForLeakCheck::{Yes, No};
    let context = super::InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let first = context.next_region_var(span);
    let second = context.next_region_var(span);
    let third = context.next_region_var(span);
    let ty::RegionKind::ReVar(first_vid) = first.kind() else { panic!() };
    let ty::RegionKind::ReVar(second_vid) = second.kind() else { panic!() };
    let root = context.universe();
    let snapshot = context.start_snapshot();
    {
        let mut inner = context.inner.borrow_mut();
        let mut regions = inner.unwrap_region_constraints();
        regions.make_eqregion(span, first, second, No);
        let data = regions.data();
        assert_eq!(data.constraints.len(), 1, "one direct upstream equality, not two legacy subregion records");
        let (equality, origin) = data.constraints[0];
        assert_eq!(origin, span); assert_eq!(equality.visible_for_leak_check, No);
        assert_eq!(equality.kind, ConstraintKind::VarEqVar);
        let parts = equality.iter_outlives().collect::<Vec<_>>();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|part| part.kind == ConstraintKind::VarSubVar && part.visible_for_leak_check == No));
        assert_eq!((parts[0].sub, parts[0].sup), (first, second));
        assert_eq!((parts[1].sub, parts[1].sup), (second, first));
        assert_eq!(regions.shallow_resolve_region_var(cx, first_vid), regions.shallow_resolve_region_var(cx, second_vid));
        regions.make_subregion(span, third, first, Yes);
        regions.make_subregion(span, first, first, No);
        regions.make_subregion(span, first, Region::new_static(cx), Yes);
        assert_eq!(regions.data().constraints.len(), 2, "upstream reflexive/static-subregion no-ops");
    }
    let universe = context.create_next_universe();
    let placeholders = [0, 1].map(|index| cx.intern_region(ty::RegionKind::RePlaceholder(
        ty::PlaceholderRegion::new_anon(universe, ty::BoundVar::from_u32(index)))));
    let fresh = context.next_region_var(span);
    {
        let mut inner = context.inner.borrow_mut();
        let mut regions = inner.unwrap_region_constraints();
        regions.make_eqregion(span, placeholders[0], placeholders[1], Yes);
        regions.make_subregion(span, placeholders[0], third, No);
        regions.make_subregion(span, third, placeholders[1], Yes);
        regions.make_eqregion(span, Region::new_static(cx), placeholders[1], No);
        regions.make_eqregion(span, Region::new_static(cx), third, Yes);
        regions.make_eqregion(span, fresh, placeholders[0], Yes);
        let data = regions.data();
        assert_eq!(data.constraints.len(), 8);
        let kinds = data.constraints.iter().map(|(c, _)| c.kind).collect::<std::collections::HashSet<_>>();
        assert_eq!(kinds.len(), 7, "all actual compiler constraint kinds exercised");
        for (constraint, origin) in &data.constraints {
            assert_eq!(*origin, span);
            let parts = constraint.iter_outlives().collect::<Vec<_>>();
            assert!(parts.iter().all(|p| p.visible_for_leak_check == constraint.visible_for_leak_check));
            let equality = matches!(constraint.kind, ConstraintKind::VarEqVar | ConstraintKind::VarEqReg | ConstraintKind::RegEqReg);
            assert_eq!(parts.len(), if equality {2} else {1});
        }
        assert_eq!(data.constraints.iter().filter(|(c, _)| c.visible_for_leak_check == No).count(), 3);
        assert_eq!(data.constraints[5].0.kind, ConstraintKind::RegSubReg);
        assert_eq!(data.constraints[5].0.sub, Region::new_static(cx));
        assert_eq!(data.constraints[6].0.kind, ConstraintKind::RegSubVar);
    }
    check_region_response(&context, span);
    context.rollback_to(snapshot);
    assert_eq!(context.universe(), root);
    assert_eq!(context.num_region_vars(), 3);
    {
        let mut inner = context.inner.borrow_mut();
        let mut regions = inner.unwrap_region_constraints();
        assert!(regions.data().constraints.is_empty());
        assert_eq!(regions.shallow_resolve_region_var(cx, first_vid), first);
        assert_eq!(regions.shallow_resolve_region_var(cx, second_vid), second);
        assert_eq!(regions.try_resolve_region_var(first_vid), Err(root));
    }
    context.commit_if_ok::<(), (), _>(|_| {
        context.inner.borrow_mut().unwrap_region_constraints().make_eqregion(span, first, second, No);
        Ok(())
    }).unwrap();
    let mut inner = context.inner.borrow_mut();
    let mut regions = inner.unwrap_region_constraints();
    let collected = regions.take_and_reset_data();
    assert_eq!(collected.constraints.len(), 1);
    assert_eq!(collected.constraints[0].0.visible_for_leak_check, No);
    assert_eq!(collected.constraints[0].1, span);
    assert!(regions.data().constraints.is_empty());
    assert_eq!(regions.shallow_resolve_region_var(cx, first_vid), first);
    assert_eq!(regions.shallow_resolve_region_var(cx, second_vid), second);
    assert!(cx.services().ready(cx).is_err(), "collection grants no source facts/root or leak-check readiness");
}

/// Supplemental actual compiler leak-check controls; real Cx/Span, explicit host
/// region IR. No fake delegate/source facts and no stock HRTB runtime claim.
fn check_actual_leak_checker<'cx>(cx: DbInterner<'cx>, span: Span) {
    use super::InferenceContext;
    use ty::solve::VisibleForLeakCheck::{Yes, No};
    fn check<'cx>(context: &InferenceContext<'cx>, outer: ty::UniverseIndex,
        snapshot: Option<&super::snapshot::CombinedSnapshot>)
        -> Result<(), ty::error::TypeError<DbInterner<'cx>>> {
        context.inner.borrow_mut().unwrap_region_constraints()
            .leak_check(context.interner, outer, context.universe(), snapshot)
    }
    fn setup<'cx>(cx: DbInterner<'cx>, span: Span)
        -> (InferenceContext<'cx>, Region<'cx>, Region<'cx>, [Region<'cx>;2]) {
        let context = InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
        let outer = context.next_region_var(span);
        let universe = context.create_next_universe();
        let inner = context.next_region_var(span);
        let placeholders = [0,1].map(|index| cx.intern_region(ty::RegionKind::RePlaceholder(
            ty::PlaceholderRegion::new_anon(universe, ty::BoundVar::from_u32(index)))));
        (context, outer, inner, placeholders)
    }
    let root = ty::UniverseIndex::ROOT;
    // Old-universe region outliving a placeholder is permitted (static works),
    // while a placeholder outliving an unnameable old-universe region is not.
    let (context, outer, _, p) = setup(cx, span);
    context.inner.borrow_mut().unwrap_region_constraints().make_subregion(span, p[0], outer, Yes);
    assert!(check(&context, root, None).is_ok());
    let (context, outer, _, p) = setup(cx, span);
    context.inner.borrow_mut().unwrap_region_constraints().make_subregion(span, outer, p[0], Yes);
    let error = check(&context, root, None).unwrap_err();
    assert!(matches!(error, ty::error::TypeError::RegionsInsufficientlyPolymorphic(bound, region)
        if bound.var == ty::BoundVar::from_u32(0) && region == outer));
    // Keep the existing outside failure, but do not let it poison a snapshot-only demand.
    let snapshot = context.start_snapshot();
    assert!(check(&context, root, Some(&snapshot)).is_ok());
    context.inner.borrow_mut().unwrap_region_constraints().make_subregion(span, outer, p[0], No);
    assert!(check(&context, root, Some(&snapshot)).is_ok());
    context.inner.borrow_mut().unwrap_region_constraints().make_subregion(span, p[1], p[0], Yes);
    assert!(matches!(check(&context, root, Some(&snapshot)),
        Err(ty::error::TypeError::RegionsInsufficientlyPolymorphic(..))));
    context.rollback_to(snapshot);
    assert!(check(&context, root, None).is_err());
    // Nameable universes and self/static outlives follow exact upstream directions.
    let (context, _, inner, p) = setup(cx, span);
    {
        let mut owner = context.inner.borrow_mut(); let mut regions = owner.unwrap_region_constraints();
        regions.make_subregion(span, inner, p[0], Yes);
        regions.make_subregion(span, p[0], p[0], Yes);
        regions.make_subregion(span, p[0], Region::new_static(cx), Yes);
    }
    assert!(check(&context, root, None).is_ok());
    let (context, _, _, p) = setup(cx, span);
    context.inner.borrow_mut().unwrap_region_constraints().make_subregion(span, Region::new_static(cx), p[0], Yes);
    assert!(check(&context, root, None).is_err());
    // Direct equality and a path via another SCC must both retain genuine failures.
    let (context, outer, _, p) = setup(cx, span);
    context.inner.borrow_mut().unwrap_region_constraints().make_eqregion(span, outer, p[0], Yes);
    assert!(check(&context, root, None).is_err());
    let (context, _, _, p) = setup(cx, span);
    context.inner.borrow_mut().unwrap_region_constraints().make_eqregion(span, p[0], p[1], Yes);
    assert!(check(&context, root, None).is_err());
    let (context, outer, inner, p) = setup(cx, span);
    {
        let mut owner = context.inner.borrow_mut(); let mut regions = owner.unwrap_region_constraints();
        regions.make_subregion(span, inner, p[0], Yes);
        regions.make_subregion(span, outer, inner, Yes);
    }
    assert!(check(&context, root, None).is_err());
    let (context, outer, inner, p) = setup(cx, span);
    {
        let mut owner = context.inner.borrow_mut(); let mut regions = owner.unwrap_region_constraints();
        regions.make_subregion(span, inner, p[0], Yes);
        regions.make_subregion(span, outer, inner, No);
    }
    assert!(check(&context, root, None).is_ok());
    // Actual Verify insertion preserves source origin and shares the same undo log.
    let snapshot = context.start_snapshot();
    {
        let mut owner = context.inner.borrow_mut(); let mut regions = owner.unwrap_region_constraints();
        let kind = super::region_constraints::GenericKind::Placeholder(ty::PlaceholderType::new(
            context.universe(), ty::BoundTy {var: ty::BoundVar::from_u32(0), kind: ty::BoundTyKind::Anon}));
        regions.verify_generic_bound(span, kind.clone(), inner, super::region_constraints::VerifyBound::AllBounds(vec![]));
        assert!(regions.data().verifys.is_empty());
        regions.verify_generic_bound(span, kind, inner, super::region_constraints::VerifyBound::OutlivedBy(p[0]));
        assert_eq!(regions.data().verifys.len(), 1);
        assert_eq!(regions.data().verifys[0].origin, span);
    }
    // Calling snapshot leak-check with a Verify is an upstream invariant violation;
    // do not fake success. Its panic branch is source-reviewed, not executed here.
    context.rollback_to(snapshot);
    assert!(context.inner.borrow_mut().unwrap_region_constraints().data().verifys.is_empty());
    assert!(check(&context, root, None).is_ok());
    let fresh = InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    assert!(check(&fresh, root, None).is_ok());
    assert!(cx.services().ready(cx).is_err(), "leak checking grants no source facts/root readiness");
}

/// Host state controls with real source Type/Span, without inventing an opaque
/// declaration/key or claiming source opaque execution.
fn check_obligation_state<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use super::{InferenceContext, TypeOutlivesConstraint};
    use super::opaque_types::ProvisionalHiddenType;
    use ty::solve::TyOrConstInferVar;
    use ty::inherent::{Ty as _, OpaqueTypeStorageEntries as _};
    let context = InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let record = ProvisionalHiddenType { ty: actual, span };
    assert_eq!(record.ty, actual); assert_eq!(record.span, span);
    assert!(!context.opaque_types_storage_num_entries().needs_reevaluation(0));
    assert!(context.clone_opaque_types().is_empty());
    assert!(context.clone_opaque_types_lookup_table().is_empty());
    assert!(context.clone_duplicate_opaque_types().is_empty());
    assert!(context.clone_opaque_types_added_since(context.opaque_types_storage_num_entries()).is_empty());
    assert!(context.take_opaque_types().is_empty());
    context.reset_opaque_types();
    let sub = context.next_region_var(span);
    let inferred = context.next_ty_var(span);
    let global = cx.ty(ty::TyKind::Bool);
    assert!(global.is_global()); assert!(!inferred.is_global());
    context.register_type_outlives_constraint(global, sub, span);
    assert!(context.inner.borrow().region_obligations().is_empty());
    let snapshot = context.start_snapshot();
    context.register_type_outlives_constraint(inferred, sub, span);
    {
        let owner = context.inner.borrow(); let values = owner.region_obligations();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].sup_type, inferred);assert_eq!(values[0].sub_region, sub);assert_eq!(values[0].origin, span);
    }
    let failed: Result<(), ()> = context.commit_if_ok(|_| {
        context.register_type_outlives_constraint_inner(TypeOutlivesConstraint { sup_type: actual, sub_region: sub, origin: span });
        Err(())
    });
    assert!(failed.is_err());assert_eq!(context.inner.borrow().region_obligations().len(), 1);
    context.commit_if_ok::<(), (), _>(|_| {
        context.register_type_outlives_constraint_inner(TypeOutlivesConstraint { sup_type: actual, sub_region: sub, origin: span });
        Ok(())
    }).unwrap();
    assert_eq!(context.inner.borrow().region_obligations().len(), 2);
    context.rollback_to(snapshot);
    assert!(context.inner.borrow().region_obligations().is_empty());
    context.commit_if_ok::<(), (), _>(|_| {
        context.register_type_outlives_constraint(inferred, sub, span);
        Ok(())
    }).unwrap();
    context.probe(|_| { context.register_type_outlives_constraint_inner(TypeOutlivesConstraint { sup_type: actual, sub_region: sub, origin: span }); });
    assert_eq!(context.inner.borrow().region_obligations().len(), 1);
    assert_eq!(context.inner.borrow().region_obligations()[0].origin, span);
    // Genuine upstream wake predicate over all4table kinds and snapshot changes.
    let a = context.next_ty_vid(span); let b = context.next_ty_vid(span);
    let int = context.next_int_vid();let float = context.next_float_vid();let c = context.next_const_vid(span);
    let variables = [TyOrConstInferVar::Ty(a), TyOrConstInferVar::TyInt(int), TyOrConstInferVar::TyFloat(float), TyOrConstInferVar::Const(c)];
    assert!(variables.iter().all(|v| !context.ty_or_const_infer_var_changed(*v)));
    let snapshot = context.start_snapshot();
    let value = Const::new(cx, ty::ConstKind::Value(ValueConst {ty:global,
        tree:cx.valtree(ty::ValTreeKind::Leaf(ScalarInt {bits:1,bytes:1}))}));
    {
        let mut owner = context.inner.borrow_mut();
        owner.type_variables().instantiate(a, actual);
        owner.int_unification_table().union_value(int, ty::IntVarValue::IntType(ty::IntTy::I32));
        owner.float_unification_table().union_value(float, ty::FloatVarValue::Known(ty::FloatTy::F64));
        owner.const_unification_table().union_value(c, ConstVariableValue::Known {value});
    }
    assert!(variables.iter().all(|v| context.ty_or_const_infer_var_changed(*v)));
    context.rollback_to(snapshot);
    assert!(variables.iter().all(|v| !context.ty_or_const_infer_var_changed(*v)));
    context.inner.borrow_mut().type_variables().equate(a,b);
    let root = context.root_var(a);let redirected = if root==a {b} else {a};
    assert!(!context.ty_or_const_infer_var_changed(TyOrConstInferVar::Ty(root)));
    assert!(context.ty_or_const_infer_var_changed(TyOrConstInferVar::Ty(redirected)));
    let before = context.num_ty_vars();
    let removed = context.probe(|_| context.next_ty_vid(span));
    assert_eq!(context.num_ty_vars(), before);
    // Rolled-back handles are invalid; the upstream table may reuse their ID.
    // Only query the newly allocated live variable, never treat an absent ID
    // as the readonly probe's redirected-root case.
    let replacement = context.next_ty_vid(span);
    assert_eq!(replacement, removed);
    assert!(!context.ty_or_const_infer_var_changed(TyOrConstInferVar::Ty(replacement)));
    assert!(cx.services().ready(cx).is_err(), "data state grants no full facts, source opaques or trait root");
}

// Source-bound controls for actual shared trait dispatch and compiler generalization.
// No custom TypeRelation/provider: SolverRelating supplies the real relation.
fn check_inference_client<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use super::InferenceContext;
    use crate::traits::upstream::model::{ParamEnv, List};
    use ty::{InferCtxtLike as Client, inherent::Ty as _};
    use ty::relate::solver_relating::RelateExt;
    let context = InferenceContext::new(cx, ty::TypingMode::PostAnalysis);
    let root = ty::UniverseIndex::ROOT;
    assert_eq!(Client::cx(&context), cx);
    assert!(Client::next_trait_solver(&context));
    assert!(Client::enable_next_solver_overflow_fcw(&context));
    assert!(!Client::disable_trait_solver_fast_paths(&context));
    assert!(matches!(Client::typing_mode_raw(&context), ty::TypingMode::PostAnalysis));
    assert_eq!(Client::universe(&context), root);
    let target = context.next_ty_vid(span);
    let snapshot = context.start_snapshot();
    let young_universe = Client::create_next_universe(&context);
    let young_ty = context.next_ty_vid(span);
    let young_const = context.next_const_vid(span);
    let young_region = context.next_region_vid(span);
    assert_eq!(Client::universe_of_ty(&context, young_ty), Some(young_universe));
    assert_eq!(Client::universe_of_const(&context, young_const), Some(young_universe));
    assert_eq!(Client::universe_of_region(&context, young_region), Some(young_universe));
    let array = Ty::new_array_with_const_len(cx, Ty::new_var(cx, young_ty), Const::new_var(cx, young_const));
    let reference = Ty::new_ref(cx, Region::new_var(cx, young_region), array, ty::Mutability::Not);
    Client::instantiate_ty_var_raw(&context, target, reference);
    // The actual compiler folder pulls every referenced inference family down.
    assert_eq!(Client::universe_of_ty(&context, young_ty), Some(root));
    assert_eq!(Client::universe_of_const(&context, young_const), Some(root));
    assert_eq!(Client::universe_of_region(&context, young_region), Some(root));
    assert_eq!(Client::universe_of_ty(&context, target), None);
    assert!(Client::shallow_resolve_ty_var(&context, target).has_infer());
    context.rollback_to(snapshot);
    assert_eq!(Client::universe(&context), root);
    assert_eq!(Client::universe_of_ty(&context, target), Some(root));
    // Trait wrappers call the existing combined snapshot owner, rather than
    // recursing through the same-named trait methods.
    let before = context.num_ty_vars();
    Client::probe(&context, || { Client::next_ty_infer(&context); });
    assert_eq!(context.num_ty_vars(), before);
    let failed: Result<(), ()> = Client::commit_if_ok(&context, || {
        Client::instantiate_ty_var_raw(&context, target, actual); Err(())
    });
    assert_eq!(failed, Err(()));
    assert_eq!(Client::universe_of_ty(&context, target), Some(root));
    Client::commit_if_ok::<_, ()>(&context, || {
        Client::instantiate_ty_var_raw(&context, target, actual); Ok(())
    }).unwrap();
    assert_eq!(Client::shallow_resolve_ty_var(&context, target), actual);

    // Empty caller bounds are explicit for these structural relations. This
    // does not claim to construct the witness's trait-goal ParamEnv or root.
    let env = ParamEnv(cx.clauses(List::default()));
    let variable = context.next_ty_var(span);
    assert!(context.relate(env, variable, ty::Invariant, actual, span).unwrap().is_empty());
    assert_eq!(Client::shallow_resolve(&context, variable), actual);
    let cyclic = context.next_ty_var(span);
    let pointer = Ty::new_ptr(cx, cyclic, ty::Mutability::Not);
    assert!(matches!(context.relate(env, cyclic, ty::Invariant, pointer, span), Err(ty::error::TypeError::CyclicTy(_))));
    assert_eq!(Client::shallow_resolve(&context, cyclic), cyclic);
    // An unnameable placeholder is a real generalization failure, not a fake
    // type identity or an excuse to mark a trait goal solved.
    let older = context.next_ty_var(span);
    let younger = Client::create_next_universe(&context);
    let placeholder = Ty::new_placeholder(cx, ty::PlaceholderType::new_anon(younger, ty::BoundVar::from_u32(0)));
    assert!(context.relate(env, older, ty::Invariant, placeholder, span).is_err());
    assert_eq!(Client::shallow_resolve(&context, older), older);
    // Shared subtyping requests a fresh region and records actual outlives data.
    let subtype = context.next_ty_var(span);
    let reference = Ty::new_ref(cx, Region::new_static(cx), actual, ty::Mutability::Not);
    assert!(context.relate(env, subtype, ty::Covariant, reference, span).unwrap().is_empty());
    assert!(matches!(Client::shallow_resolve(&context, subtype).kind(), ty::Ref(..)));
    assert!(!context.inner.borrow().region_constraint_storage.as_ref().unwrap().data.constraints.is_empty());
    let entries = Client::opaque_types_storage_num_entries(&context);
    assert!(Client::clone_opaque_types_added_since(&context, entries).is_empty());
    assert!(cx.services().ready(cx).is_err(), "a complete inference client grants no full source facts or solver root");
}

// Pure canonical-IR transport through the actual pinned compiler instantiator.
// Source Type/Span payloads come from unchanged fixtures, never fake SourceFacts.
fn check_canonical_instantiation<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use super::{InferenceContext, canonical::CanonicalExt};
    use crate::traits::upstream::model::{Arg, Clause, AllocateList};
    use ty::{InferCtxtLike as Client, inherent::{Ty as _, GenericArg as _}};
    let root = ty::UniverseIndex::ROOT; let young = root.next_universe();
    let var = |n| ty::BoundVar::from_u32(n);
    let placeholder_ty = |n| ty::PlaceholderType::new_anon(young, var(n));
    let placeholder_region = |n| ty::PlaceholderRegion::new_anon(young, var(n));
    let placeholder_const = |n| ty::PlaceholderConst::new(young, ty::BoundConst::new(var(n)));
    let kinds = cx.mk_canonical_var_kinds(&[
        ty::CanonicalVarKind::Ty { ui: root, sub_root: var(0) },
        ty::CanonicalVarKind::Ty { ui: root, sub_root: var(0) },
        ty::CanonicalVarKind::Int, ty::CanonicalVarKind::Float,
        ty::CanonicalVarKind::Region(young), ty::CanonicalVarKind::Const(young),
        ty::CanonicalVarKind::PlaceholderTy(placeholder_ty(6)),
        ty::CanonicalVarKind::PlaceholderRegion(placeholder_region(7)),
        ty::CanonicalVarKind::PlaceholderConst(placeholder_const(8)),
    ]);
    let bound_ty = |n| cx.ty(ty::Bound(ty::BoundVarIndexKind::Canonical, placeholder_ty(n).bound));
    let bound_region = |n| cx.intern_region(ty::RegionKind::ReBound(ty::BoundVarIndexKind::Canonical, placeholder_region(n).bound));
    let bound_const = |n| Const::new(cx, ty::ConstKind::Bound(ty::BoundVarIndexKind::Canonical, ty::BoundConst::new(var(n))));
    let args = cx.mk_args(&[
        bound_ty(0).into(),bound_ty(1).into(),bound_ty(2).into(),bound_ty(3).into(),
        bound_region(4).into(),bound_const(5).into(),bound_ty(6).into(),
        bound_region(7).into(),bound_const(8).into(),
    ]);
    let clauses = cx.clauses(cx.list(vec![
        Clause::new(cx, ty::Binder::dummy(ty::ClauseKind::TypeOutlives(ty::OutlivesClause(bound_ty(0),bound_region(4))))),
        Clause::new(cx, ty::Binder::dummy(ty::ClauseKind::WellFormed(bound_const(5).into()))),
        Clause::new(cx, ty::Binder::dummy(ty::ClauseKind::WellFormed(bound_ty(1).into()))),
    ]));
    let ordinary_bound = Ty::new_bound(cx,ty::INNERMOST,placeholder_ty(0).bound);
    let ordinary_binder = ty::Binder::bind_with_vars(
        Ty::new_tup(cx,&[ordinary_bound,bound_ty(0)]),
        cx.mk_bound_variable_kinds_from_iter([ty::BoundVariableKind::Ty(ty::BoundTyKind::Anon)]));
    let canonical = ty::Canonical { value: (actual,args,(clauses,ordinary_binder)), max_universe: young, var_kinds: kinds };
    let input = ty::CanonicalQueryInput { canonical, typing_mode: ty::TypingModeEqWrapper(ty::TypingMode::PostAnalysis) };
    use rustc_next_trait_solver::delegate::SolverDelegate as Delegate;
    use super::delegate::SolverContext;
    let (delegate, output, values) = <SolverContext as Delegate>::build_with_canonical(cx, &input);
    let owner = &*delegate;
    assert_eq!(output.0, actual);assert_eq!(output.1.items(),values.var_values.items());
    let ty::Tuple(nested) = output.2.1.skip_binder().kind() else { panic!("real nested binder tuple absent") };
    assert_eq!(nested.items()[0],ordinary_bound,"ordinary bound variable must stay in its binder");
    assert_eq!(nested.items()[1],values.var_values.items()[0].expect_ty(),"canonical variable must be substituted inside the ordinary binder");
    assert!(!output.2.1.has_escaping_bound_vars(),"ordinary variable is genuinely bound");
    assert_eq!(values.len(),9);assert_eq!(Client::universe(owner),young);
    assert!(matches!(Client::typing_mode_raw(owner),ty::TypingMode::PostAnalysis));
    let ty_vid = |n: usize| match values.var_values.items()[n].expect_ty().kind() {ty::Infer(ty::TyVar(v))=>v,_=>panic!("expected real type inference var")};
    assert_ne!(Client::root_ty_var(owner,ty_vid(0)),Client::root_ty_var(owner,ty_vid(1)));
    assert_eq!(Client::sub_unification_table_root_var(owner,ty_vid(0)),Client::sub_unification_table_root_var(owner,ty_vid(1)));
    assert_eq!(Client::universe_of_ty(owner,ty_vid(0)),Some(root));
    assert!(matches!(values.var_values.items()[2].expect_ty().kind(),ty::Infer(ty::IntVar(_))));
    assert!(matches!(values.var_values.items()[3].expect_ty().kind(),ty::Infer(ty::FloatVar(_))));
    assert!(matches!(values.var_values.items()[4].expect_region().kind(),ty::RegionKind::ReVar(_)));
    assert!(matches!(values.var_values.items()[5].expect_const().kind(),ty::ConstKind::Infer(ty::InferConst::Var(_))));
    assert_eq!(values.var_values.items()[6].expect_ty(),Ty::new_placeholder(cx,placeholder_ty(6)));
    assert_eq!(values.var_values.items()[7].expect_region(),Region::new_placeholder(cx,placeholder_region(7)));
    assert_eq!(values.var_values.items()[8].expect_const(),Const::new_placeholder(cx,placeholder_const(8)));
    let cache = cx.instantiation_caches();
    assert_eq!(cache.highest_var_in_clauses_cache.borrow().get(&clauses),Some(&5));
    let count = cache.clauses_cache.borrow().len();
    let mut tail = values.var_values.items().to_vec();
    tail[8]=Arg::Const(Const::new_var(cx,owner.next_const_vid(span)));
    let other_values = ty::CanonicalVarValues { var_values: cx.mk_args(&tail) };
    assert_eq!(canonical.instantiate_projected(cx,&other_values,|p|p.2.0),output.2.0);
    assert_eq!(cache.clauses_cache.borrow().len(),count,"unused suffix must share exact compiler prefix cache");
    assert_eq!(canonical.instantiate(cx,&values),output);
    assert_eq!(Delegate::instantiate_canonical(&delegate,canonical,values),output);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| canonical.instantiate(cx,&ty::CanonicalVarValues::dummy()))).is_err());
    // An existing non-root inference context rebases incoming universe zero.
    let rebased = InferenceContext::new(cx,ty::TypingMode::PostAnalysis);
    let base = Client::create_next_universe(&rebased);
    let (_,mapped) = rebased.instantiate_canonical(span,&canonical);
    assert!(matches!(mapped.var_values.items()[6].expect_ty().kind(),ty::Placeholder(p) if p.universe==base.next_universe()));
    let mapped_vid = match mapped.var_values.items()[0].expect_ty().kind() {ty::Infer(ty::TyVar(v))=>v,_=>unreachable!()};
    assert_eq!(Client::universe_of_ty(&rebased,mapped_vid),Some(base));
    let empty = ty::Canonical { value: actual, max_universe: root, var_kinds: cx.mk_canonical_var_kinds(&[]) };
    let (same,none) = owner.instantiate_canonical(span,&empty);
    assert_eq!(same,actual);assert!(none.var_values.items().is_empty());
    assert!(cx.services().ready(cx).is_err(),"canonical transport cannot grant full source facts or a solver answer");
}

// Actual current compiler response conversion, non-consuming dedup and undo.
fn check_region_response<'cx>(context: &super::InferenceContext<'cx>, span: Span) {
    use super::region_response::make_query_region_constraints;
    use super::region_constraints::ConstraintKind;
    use ty::solve::VisibleForLeakCheck::{Yes,No,Unreachable};
    use ty::inherent::GenericArg as _;
    let before = context.make_deduplicated_region_constraints();
    let raw = context.with_region_constraints(|data| make_query_region_constraints(vec![],data,vec![]));
    assert_eq!(raw.constraints.len(),8);assert!(raw.assumptions.is_empty());
    context.with_region_constraints(|data| {
        for ((source,origin),response) in data.constraints.iter().zip(&raw.constraints) {
            assert_eq!(*origin,span);assert_eq!(response.origin,span);
            assert_eq!(response.visible_for_leak_check,source.visible_for_leak_check);
            match (source.kind,response.constraint) {
                (ConstraintKind::VarEqVar|ConstraintKind::VarEqReg|ConstraintKind::RegEqReg,ty::RegionConstraint::Eq(eq)) => assert_eq!((eq.0,eq.1),(source.sup,source.sub)),
                (ConstraintKind::VarSubVar|ConstraintKind::RegSubVar|ConstraintKind::VarSubReg|ConstraintKind::RegSubReg,ty::RegionConstraint::Outlives(out)) => assert_eq!((out.0.expect_region(),out.1),(source.sup,source.sub)),
                other=>panic!("equality/outlives source identity changed: {other:?}"),
            }
        }
    });
    let first = context.with_region_constraints(|data| data.constraints[0].0);
    assert_eq!(first.visible_for_leak_check,No);
    let snapshot = context.start_snapshot();
    let unknown = context.next_ty_var(span);
    let assumption = ty::OutlivesClause(unknown.into(),first.sub);
    context.register_region_assumption(assumption);
    context.register_type_outlives_constraint(unknown,first.sub,span);
    context.inner.borrow_mut().unwrap_region_constraints().make_eqregion(span,first.sub,first.sup,Yes);
    let obligations=context.inner.borrow().region_obligations().to_vec();
    let assumptions=context.inner.borrow().region_assumptions().to_vec();
    let collected = context.with_region_constraints(|data| make_query_region_constraints(obligations,data,assumptions));
    assert_eq!(collected.constraints.len(),10);assert_eq!(collected.assumptions,vec![assumption]);
    let last=collected.constraints.last().unwrap();assert_eq!(last.visible_for_leak_check,Unreachable);assert_eq!(last.origin,span);
    assert!(matches!(last.constraint,ty::RegionConstraint::Outlives(out) if out.0.expect_ty()==unknown && out.1==first.sub));
    let dedup=context.make_deduplicated_region_constraints();
    assert_eq!(dedup.len(),9);assert_eq!(dedup[0].0,before[0].0);assert_eq!(dedup[0].1,Yes);
    assert_eq!(context.make_deduplicated_region_constraints(),dedup,"collection is non-consuming and stable");
    assert_eq!(context.inner.borrow().region_obligations().len(),1);assert_eq!(context.inner.borrow().region_assumptions(),&[assumption]);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| context.take_registered_region_assumptions())).is_err());
    let probe=context.probe(|_| {
        context.register_region_assumption(assumption);
        assert_eq!(context.inner.borrow().region_assumptions().len(),2);
        context.make_deduplicated_region_constraints()
    });assert_eq!(probe,dedup);assert_eq!(context.inner.borrow().region_assumptions(),&[assumption]);
    context.rollback_to(snapshot);
    assert_eq!(context.make_deduplicated_region_constraints(),before);
    assert!(context.inner.borrow().region_obligations().is_empty());assert!(context.inner.borrow().region_assumptions().is_empty());
    let independent=super::InferenceContext::new(context.interner,ty::TypingMode::PostAnalysis);
    assert!(independent.make_deduplicated_region_constraints().is_empty());
    let a=ty::OutlivesClause(independent.next_ty_var(span).into(),independent.next_region_var(span));
    independent.register_region_assumption(a);assert_eq!(independent.take_registered_region_assumptions(),vec![a]);assert!(independent.take_registered_region_assumptions().is_empty());
}

// Actual compiler fast-path outcomes; no substitute relation or trait provider.
fn check_goal_fast_path<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    use super::{InferenceContext,delegate::SolverContext};
    use rustc_next_trait_solver::delegate::SolverDelegate;
    use crate::traits::upstream::model::{ParamEnv,List,ValueConst,ScalarInt};
    use ty::{InferCtxtLike as Client,inherent::Ty as _};
    use ty::solve::{ComputeGoalFastPathOutcome::{NoFastPath,TriviallyHolds,TriviallyStalled},Goal,TyOrConstInferVar,GoalStalledOnOpaques,SizedTraitKind};
    let owner=SolverContext::from_inference_context(InferenceContext::new(cx,ty::TypingMode::PostAnalysis));
    let env=ParamEnv(cx.clauses(List::default()));
    let goal=|kind| Goal::new(cx,env,ty::Binder::dummy(kind));
    let wf=|value: Ty<'cx>| goal(ty::PredicateKind::Clause(ty::ClauseKind::WellFormed(value.into())));
    let boolean=cx.ty(ty::Bool);
    assert!(boolean.has_trivial_sizedness(cx,SizedTraitKind::Sized));
    assert!(boolean.is_trivially_pure_clone_copy());assert!(boolean.is_trivially_wf(cx));
    let str_ty=cx.ty(ty::Str);
    assert!(!str_ty.has_trivial_sizedness(cx,SizedTraitKind::Sized));
    assert!(str_ty.has_trivial_sizedness(cx,SizedTraitKind::MetaSized));
    assert!(!str_ty.is_trivially_pure_clone_copy());assert!(str_ty.is_trivially_wf(cx));
    let reference=Ty::new_ref(cx,Region::new_static(cx),boolean,ty::Mutability::Not);
    let mutable=Ty::new_ref(cx,Region::new_static(cx),boolean,ty::Mutability::Mut);
    assert!(reference.is_trivially_pure_clone_copy());assert!(!mutable.is_trivially_pure_clone_copy());
    assert!(Ty::new_ptr(cx,boolean,ty::Mutability::Mut).is_trivially_pure_clone_copy());
    assert!(Ty::new_slice(cx,boolean).is_trivially_wf(cx));assert!(!Ty::new_slice(cx,str_ty).is_trivially_wf(cx));
    for count in 0..=4 {
        let tuple=Ty::new_tup(cx,&vec![boolean;count]);
        assert_eq!(tuple.is_trivially_pure_clone_copy(),count<=3,"compiler optimization threshold is not a trait rejection");
        assert!(tuple.has_trivial_sizedness(cx,SizedTraitKind::Sized));
        assert!(matches!(owner.compute_goal_fast_path(wf(tuple),span),NoFastPath));
    }
    let unsized_tuple=Ty::new_tup(cx,&[boolean,str_ty]);
    assert!(!unsized_tuple.has_trivial_sizedness(cx,SizedTraitKind::Sized));
    assert!(unsized_tuple.has_trivial_sizedness(cx,SizedTraitKind::MetaSized));
    assert!(matches!(owner.compute_goal_fast_path(wf(boolean),span),TriviallyHolds));
    assert!(matches!(owner.compute_goal_fast_path(wf(reference),span),TriviallyHolds));
    assert!(matches!(owner.compute_goal_fast_path(wf(owner.next_int_var()),span),TriviallyHolds));
    assert!(matches!(owner.compute_goal_fast_path(wf(owner.next_float_var()),span),TriviallyHolds));
    // Real source payload sits in an ordinary tuple that requires full WF work.
    assert!(matches!(owner.compute_goal_fast_path(wf(Ty::new_tup(cx,&[actual])),span),NoFastPath));
    let unknown=owner.next_ty_var(span);let vid=unknown.ty_vid().unwrap();
    let TriviallyStalled{stalled_on}=owner.compute_goal_fast_path(wf(unknown),span) else {panic!("unknown WF must stall")};
    assert_eq!(stalled_on.stalled_vars.as_slice(),&[TyOrConstInferVar::Ty(vid)]);
    assert!(matches!(stalled_on.opaques,GoalStalledOnOpaques::No));
    Client::instantiate_ty_var_raw(&*owner,vid,boolean);
    assert!(matches!(owner.compute_goal_fast_path(wf(unknown),span),TriviallyHolds));
    let left=owner.next_region_var(span);let right=owner.next_region_var(span);
    let outlives=goal(ty::PredicateKind::Clause(ty::ClauseKind::RegionOutlives(ty::OutlivesClause(left,right))));
    assert!(matches!(owner.compute_goal_fast_path(outlives,span),TriviallyHolds));
    owner.with_region_constraints(|data| {
        assert_eq!(data.constraints.len(),1);let (constraint,origin)=data.constraints[0];assert_eq!(origin,span);
        assert_eq!((constraint.sub,constraint.sup),(right,left));assert_eq!(constraint.visible_for_leak_check,ty::solve::VisibleForLeakCheck::No);
    });
    let bound=Region::new_bound(cx,ty::INNERMOST,ty::BoundRegion{var:ty::BoundVar::from_u32(0),kind:ty::BoundRegionKind::Anon});
    let escaping=Goal::new(cx,env,ty::Binder::bind_with_vars(
        ty::PredicateKind::Clause(ty::ClauseKind::RegionOutlives(ty::OutlivesClause(bound,right))),
        cx.mk_bound_variable_kinds_from_iter([ty::BoundVariableKind::Region(ty::BoundRegionKind::Anon)])));
    assert!(matches!(owner.compute_goal_fast_path(escaping,span),NoFastPath));
    let direct=goal(ty::PredicateKind::Clause(ty::ClauseKind::TypeOutlives(ty::OutlivesClause(boolean,right))));
    assert!(matches!(owner.compute_goal_fast_path(direct,span),TriviallyHolds));
    assert!(owner.inner.borrow().region_obligations().is_empty());
    let unknown_type=owner.next_ty_var(span);
    let pending=goal(ty::PredicateKind::Clause(ty::ClauseKind::TypeOutlives(ty::OutlivesClause(unknown_type,right))));
    let TriviallyStalled{stalled_on}=owner.compute_goal_fast_path(pending,span) else {panic!("type outlives must retain inference stall")};
    assert_eq!(stalled_on.stalled_vars.as_slice(),&[TyOrConstInferVar::Ty(unknown_type.ty_vid().unwrap())]);
    let constant=owner.next_const_var(span);let const_vid=constant.ct_vid().unwrap();
    let array=Ty::new_array_with_const_len(cx,boolean,constant);
    let pending=goal(ty::PredicateKind::Clause(ty::ClauseKind::TypeOutlives(ty::OutlivesClause(array,right))));
    let TriviallyStalled{stalled_on}=owner.compute_goal_fast_path(pending,span) else {panic!("array const inference must stall")};
    assert_eq!(stalled_on.stalled_vars.as_slice(),&[TyOrConstInferVar::Const(const_vid)]);
    let const_goal=goal(ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(constant,boolean)));
    assert!(matches!(owner.compute_goal_fast_path(const_goal,span),TriviallyStalled{..}));
    let value=Const::new(cx,ty::ConstKind::Value(ValueConst{ty:boolean,tree:cx.valtree(ty::ValTreeKind::Leaf(ScalarInt{bits:1,bytes:1}))}));
    Client::instantiate_const_var_raw(&*owner,const_vid,value);
    assert!(matches!(owner.compute_goal_fast_path(const_goal,span),NoFastPath),"known const still needs full solver validation");
    for coerce in [false,true] {
        let a=owner.next_ty_var(span);let b=owner.next_ty_var(span);
        let kind=if coerce {ty::PredicateKind::Coerce(ty::CoercePredicate{a,b})}else{ty::PredicateKind::Subtype(ty::SubtypePredicate{a_is_expected:true,a,b})};
        let TriviallyStalled{stalled_on}=owner.compute_goal_fast_path(goal(kind),span) else {panic!("two vars must stall")};
        assert_eq!(stalled_on.stalled_vars.as_slice(),&[TyOrConstInferVar::Ty(a.ty_vid().unwrap()),TyOrConstInferVar::Ty(b.ty_vid().unwrap())]);
        assert_eq!(Client::sub_unification_table_root_var(&*owner,a.ty_vid().unwrap()),Client::sub_unification_table_root_var(&*owner,b.ty_vid().unwrap()));
        assert_ne!(Client::root_ty_var(&*owner,a.ty_vid().unwrap()),Client::root_ty_var(&*owner,b.ty_vid().unwrap()));
    }
    assert!(owner.known_no_opaque_types_in_storage());
    let erased=InferenceContext::new(cx,ty::TypingMode::ErasedNotCoherence(ty::MayBeErased));
    assert!(!erased.known_no_opaque_types_in_storage(),"empty storage is not enough in erased mode");
    assert!(cx.services().ready(cx).is_err(),"actual fast-path operations are not full source facts/root readiness");
}

pub(crate) fn check_trait_fast_path<'cx>(cx: DbInterner<'cx>, reference: ty::TraitRef<DbInterner<'cx>>, span: Span) {
    use super::{InferenceContext,delegate::SolverContext};
    use rustc_next_trait_solver::delegate::SolverDelegate;
    use crate::traits::upstream::model::{ParamEnv,List};
    use ty::solve::{ComputeGoalFastPathOutcome::TriviallyStalled,Goal,GoalStalledOnOpaques,SucceededInErased,TyOrConstInferVar};
    let owner=SolverContext::from_inference_context(InferenceContext::new(cx,ty::TypingMode::PostAnalysis));
    let unknown=owner.next_ty_var(span);
    let source_ref=reference.with_replaced_self_ty(cx,unknown);
    // Actual source TraitRef/Clause traversal through the exact compiler folder.
    // This is not an opaque declaration/hidden-bounds or root-goal witness.
    use crate::traits::upstream::{infer::hidden_type_bounds::BottomUpFolder,model::Clause};
    use ty::{TypeFoldable,inherent::Clause as _};
    let clause=Clause::new(cx,ty::Binder::dummy(ty::ClauseKind::Trait(ty::TraitClause{trait_ref:source_ref,polarity:ty::ClausePolarity::Positive})));
    let rewritten=clause.fold_with(&mut BottomUpFolder{interner:cx,
        ty_op:|term| if term==unknown {reference.self_ty()}else{term},lt_op:|r|r,ct_op:|c|c});
    assert_eq!(rewritten.kind().skip_binder(),ty::ClauseKind::Trait(ty::TraitClause{trait_ref:reference,polarity:ty::ClausePolarity::Positive}));
    let env=ParamEnv(cx.clauses(List::default()));
    for polarity in [ty::ClausePolarity::Positive,ty::ClausePolarity::Negative] {
        let goal=Goal::new(cx,env,ty::Binder::dummy(ty::TraitClause{trait_ref:source_ref,polarity}));
        let TriviallyStalled{stalled_on}=owner.compute_goal_fast_path(goal,span) else {panic!("genuine source trait must stall on unresolved Self")};
        assert_eq!(stalled_on.stalled_vars.as_slice(),&[TyOrConstInferVar::Ty(unknown.ty_vid().unwrap())]);
        assert!(matches!(stalled_on.opaques,GoalStalledOnOpaques::Yes{num_opaques_in_storage:0,previously_succeeded_in_erased:SucceededInErased::No}));
    }
    assert!(cx.services().ready(cx).is_err(),"source TraitRef stall is not a proven trait/root");
}

// Actual SolverDelegate entry methods over the same source-owned context.
// Four remaining metadata-dependent clients remain uninstalled/unexecuted.
fn check_solver_delegate<'cx>(cx:DbInterner<'cx>,actual:Ty<'cx>,span:Span) {
    use super::{InferenceContext,delegate::SolverContext};
    use rustc_next_trait_solver::delegate::SolverDelegate as Delegate;
    use ty::{InferCtxtLike as Client,inherent::{GenericArg as _,Ty as _}};
    use ty::solve::VisibleForLeakCheck::Yes;
    let owner=SolverContext::from_inference_context(InferenceContext::new(cx,ty::TypingMode::PostAnalysis));
    assert_eq!(Delegate::cx(&owner),cx);
    let root=ty::UniverseIndex::ROOT;
    let young=Client::create_next_universe(&*owner);
    let ty=Delegate::fresh_var_for_kind(&owner,actual.into(),span,young).expect_ty();
    assert_eq!(Client::universe_of_ty(&*owner,ty.ty_vid().unwrap()),Some(young));
    let region=Delegate::fresh_var_for_kind(&owner,Region::new_static(cx).into(),span,young).expect_region();
    assert!(matches!(region.kind(),ty::RegionKind::ReVar(_)));
    let boolean=cx.ty(ty::Bool);
    let value=Const::new(cx,ty::ConstKind::Value(ValueConst{ty:boolean,tree:cx.valtree(ty::ValTreeKind::Leaf(ScalarInt{bits:1,bytes:1}))}));
    let constant=Delegate::fresh_var_for_kind(&owner,value.into(),span,young).expect_const();
    assert!(matches!(constant.kind(),ty::ConstKind::Infer(ty::InferConst::Var(_))));
    let arg=Delegate::instantiate_canonical_var(&owner,ty::CanonicalVarKind::Ty{ui:root,sub_root:ty::BoundVar::from_u32(0)},span,&[],|_|young).expect_ty();
    assert_eq!(Client::universe_of_ty(&*owner,arg.ty_vid().unwrap()),Some(young));
    assert!(Delegate::make_deduplicated_region_constraints(&owner).is_empty());
    assert!(Delegate::leak_check(&owner,root).is_ok());
    let snapshot=owner.start_snapshot();
    let a=Region::new_placeholder(cx,ty::PlaceholderRegion::new_anon(young,ty::BoundVar::from_u32(0)));
    let b=Region::new_placeholder(cx,ty::PlaceholderRegion::new_anon(young,ty::BoundVar::from_u32(1)));
    Client::sub_regions(&*owner,a,b,Yes,span);
    assert_eq!(Delegate::leak_check(&owner,root),Err(ty::solve::NoSolution));
    let constraints=Delegate::make_deduplicated_region_constraints(&owner);
    assert_eq!(constraints.len(),1);assert_eq!(constraints[0].1,Yes);
    assert_eq!(Delegate::make_deduplicated_region_constraints(&owner),constraints);
    owner.rollback_to(snapshot);
    assert!(Delegate::make_deduplicated_region_constraints(&owner).is_empty());
    assert!(Delegate::leak_check(&owner,root).is_ok());
    assert!(cx.services().ready(cx).is_err(),"delegate methods grant no missing source clients or root readiness");
}

// Compiler folder postorder and Type/Region/Const transport, over the genuine
// retained source Type. This does not fabricate opaque IDs or source facts.
fn check_pinned_bottom_up_folder<'cx>(cx:DbInterner<'cx>,actual:Ty<'cx>,span:Span) {
    use super::{InferenceContext,hidden_type_bounds::BottomUpFolder};
    use ty::{TypeFoldable,inherent::Ty as _};
    let owner=InferenceContext::new(cx,ty::TypingMode::PostAnalysis);
    let unknown=owner.next_ty_var(span);
    let inner=Ty::new_tup(cx,&[unknown]);
    let replaced_inner=Ty::new_tup(cx,&[actual]);
    let region=Region::new_static(cx);
    let source=Ty::new_tup(cx,&[Ty::new_ref(cx,region,inner,ty::Mutability::Not)]);
    let expected=Ty::new_tup(cx,&[Ty::new_ref(cx,region,Ty::new_slice(cx,actual),ty::Mutability::Not)]);
    let mut visited=Vec::new();let mut regions=Vec::new();
    let folded=source.fold_with(&mut BottomUpFolder { interner:cx,
        ty_op:|term| {visited.push(term);if term==unknown{actual}else if term==replaced_inner{Ty::new_slice(cx,actual)}else{term}},
        lt_op:|r|{regions.push(r);r},ct_op:|c|c });
    assert_eq!(folded,expected);
    let before=visited.iter().position(|t|*t==unknown).unwrap();
    let after=visited.iter().position(|t|*t==replaced_inner).unwrap();
    assert!(before<after,"parent callback must receive the reconstructed child");
    assert_eq!(visited.last(),Some(&expected));assert_eq!(regions,vec![region]);
    let boolean=cx.ty(ty::Bool);
    let constant=Const::new(cx,ty::ConstKind::Value(ValueConst{ty:boolean,tree:cx.valtree(ty::ValTreeKind::Leaf(ScalarInt{bits:1,bytes:1}))}));
    let mut types=Vec::new();let mut constants=Vec::new();
    let result=constant.fold_with(&mut BottomUpFolder {interner:cx,
        ty_op:|t|{types.push(t);t},lt_op:|r|r,ct_op:|c|{constants.push(c);c}});
    assert_eq!(result,constant);assert_eq!(types,vec![boolean]);assert_eq!(constants,vec![constant]);
    assert!(cx.services().ready(cx).is_err(),"folder transport grants no missing opaque source/bounds readiness");
}

// Exact pinned Type client over actual retained source IR. ADT/alias/parameter
// branches still require complete real facts; this constructs no fake lang item.
fn check_pinned_discriminant_type<'cx>(cx:DbInterner<'cx>,actual:Ty<'cx>,span:Span) {
    use ty::{inherent::Ty as _, BoundVariableKind, RegionKind, TyKind};
    use crate::traits::upstream::model::AllocateList;
    if !matches!(actual.kind(),TyKind::Adt(..)|TyKind::Param(_)|TyKind::Alias(..)
        |TyKind::Infer(ty::InferTy::TyVar(_))|TyKind::Bound(..)|TyKind::Placeholder(_)) {
        assert_eq!(actual.pinned_discriminant_ty(cx),Ty::new_u8(cx));
    }
    let bound=Region::new_bound(cx,ty::INNERMOST,ty::BoundRegion {var:ty::BoundVar::from_u32(0),kind:ty::BoundRegionKind::Anon});
    let reference=Ty::new_ref(cx,bound,actual,ty::Mutability::Not);
    let binder=ty::Binder::bind_with_vars(reference,cx.list(vec![BoundVariableKind::Region(ty::BoundRegionKind::Anon)]));
    let erased=cx.instantiate_bound_regions_with_erased(binder);
    assert!(matches!(erased.kind(),TyKind::Ref(region,inner,ty::Mutability::Not) if region.kind()==RegionKind::ReErased && inner==actual));
    assert!(matches!(reference.kind(),TyKind::Ref(region,_,_) if region==bound));
    let unsafe_binder=Ty::new_unsafe_binder(cx,binder);
    assert_eq!(unsafe_binder.pinned_discriminant_ty(cx),Ty::new_u8(cx));
    assert!(!span.is_dummy());
    assert!(cx.services().ready(cx).is_err(),"Type client cannot mint missing source facts or root readiness");
}
