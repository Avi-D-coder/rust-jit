//! Supplemental LRR route/isolation controls using actual upstream fixture IR.
//! This is storage execution, never a trait answer or an upstream main pass.
use super::{InferenceStorage, DbInterner, Ty, Span, Const, Region};
use super::unify_key::ConstVariableValue;
use crate::traits::upstream::model::{ValueConst, ScalarInt};
use rustc_type_ir::{self as ty, Interner, inherent::IntoKind};

pub(crate) fn check_actual_storage<'cx>(cx: DbInterner<'cx>, actual: Ty<'cx>, span: Span) {
    assert!(!span.is_dummy(), "actual retained declaration range supplies inference origin");
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
