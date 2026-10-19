//! Supplemental LRR route/isolation controls using actual upstream fixture IR.
//! This is storage execution, never a trait answer or an upstream main pass.
use super::{InferenceStorage, DbInterner, Ty, Span, Const, Region};
use super::unify_key::ConstVariableValue;
use crate::traits::upstream::model::{ValueConst, ScalarInt};
use rustc_type_ir::{self as ty, Interner, inherent::IntoKind};

pub(crate) fn check_actual_storage<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    assert!(!span.is_dummy(), "actual retained declaration range supplies inference origin");
    check_placeholder_assumptions(cx, actual, span);
    check_solver_region_transactions(cx, span);
    check_current_region_collector(cx, span);
    check_actual_leak_checker(cx, span);
    check_variable_resolution(cx, actual, span);
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
