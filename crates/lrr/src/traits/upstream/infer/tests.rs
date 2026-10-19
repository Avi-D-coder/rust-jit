//! Supplemental LRR route/isolation controls using actual upstream fixture IR.
//! This is storage execution, never a trait answer or an upstream main pass.
use super::{InferenceStorage, DbInterner, Ty, Span, Const, Region};
use super::unify_key::ConstVariableValue;
use crate::traits::upstream::model::{ValueConst, ScalarInt};
use rustc_type_ir::{self as ty, Interner, inherent::IntoKind};

pub(crate) fn check_actual_storage<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    assert!(!span.is_dummy(), "actual retained declaration range supplies inference origin");
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
        inner.unwrap_region_constraints().make_eqregion(first, second);
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
        assert_eq!(regions.opportunistic_resolve_var(cx, first_region), first);
        assert_eq!(regions.opportunistic_resolve_var(cx, second_region), second);
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
