use lrr::{
    core::digest::Digest,
    ir::op::PrimitiveType as P,
    traits::{
        term::{ConstTerm, GenericArg, InferVarKey, ParamKey, RegionTerm, TyTerm as T},
        unify::Unifier,
    },
};
fn var(n: u32) -> InferVarKey {
    InferVarKey {
        transaction: Digest([142; 32]),
        ordinal: n,
    }
}
#[test]
fn structural_unification_occurs_check_rejects_direct_and_indirect_cycles() {
    let mut u = Unifier::new(var(0).transaction);
    assert_eq!(
        u.unify(&T::Infer(var(0)), &T::Slice(Box::new(T::Infer(var(0)))))
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(u.binding(var(0)).is_none());
    u.unify(&T::Infer(var(0)), &T::Tuple(vec![T::Infer(var(1))]))
        .unwrap();
    assert_eq!(
        u.unify(&T::Infer(var(1)), &T::Tuple(vec![T::Infer(var(0))]))
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(u.binding(var(1)).is_none());
}
#[test]
fn structural_unification_preserves_rigid_nominal_namespace_and_width_identity() {
    let mut u = Unifier::new(var(0).transaction);
    let p = ParamKey {
        definition: Digest([1; 32]),
        ordinal: 0,
    };
    for (a, b) in [
        (T::Param(p), T::Primitive(P::I64)),
        (
            T::Infer(var(0)),
            T::Ref(
                RegionTerm::Bound { depth: 0, index: 0 },
                false,
                Box::new(T::Primitive(P::I64)),
            ),
        ),
        (
            T::Param(p),
            T::Param(ParamKey {
                definition: Digest([2; 32]),
                ordinal: 0,
            }),
        ),
        (T::Primitive(P::Usize), T::Primitive(P::U64)),
        (T::Primitive(P::I32), T::Primitive(P::I64)),
        (
            T::Nominal(Digest([1; 32]), vec![]),
            T::Nominal(Digest([2; 32]), vec![]),
        ),
        (
            T::Nominal(
                Digest([1; 32]),
                vec![GenericArg::Const(ConstTerm::Value(1))],
            ),
            T::Nominal(
                Digest([1; 32]),
                vec![GenericArg::Region(RegionTerm::Static)],
            ),
        ),
        (
            T::Ref(
                RegionTerm::Bound { depth: 0, index: 0 },
                false,
                Box::new(T::Primitive(P::I64)),
            ),
            T::Ref(RegionTerm::Static, false, Box::new(T::Primitive(P::I64))),
        ),
    ] {
        assert_eq!(u.unify(&a, &b).unwrap_err().code, "LRR-E-GENERIC-BINDING");
    }
}
#[test]
fn structural_unification_failure_is_atomic_and_foreign_variables_are_rejected() {
    let mut u = Unifier::new(var(0).transaction);
    let a = T::Tuple(vec![T::Primitive(P::I64), T::Infer(var(0))]);
    let b = T::Tuple(vec![T::Primitive(P::U64), T::Primitive(P::I64)]);
    assert_eq!(u.unify(&a, &b).unwrap_err().code, "LRR-E-GENERIC-BINDING");
    assert!(u.binding(var(0)).is_none());
    let foreign = InferVarKey {
        transaction: Digest([143; 32]),
        ordinal: 0,
    };
    assert_eq!(
        u.unify(&T::Infer(var(0)), &T::Tuple(vec![T::Infer(foreign)]))
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(u.binding(var(0)).is_none());
    u.unify(
        &T::Infer(var(0)),
        &T::Array(Box::new(T::Primitive(P::I8)), ConstTerm::Value(3)),
    )
    .unwrap();
    assert!(matches!(
        u.binding(var(0)),
        Some(T::Array(_, ConstTerm::Value(3)))
    ));
}

#[test]
fn signature_unification_nesting_budget_rejects_before_binding() {
    let mut term = T::Primitive(P::I64);
    for _ in 0..300 {
        term = T::Slice(Box::new(term));
    }
    assert!(term.heap_bytes().is_none());
    let mut u = Unifier::new(var(0).transaction);
    assert_eq!(
        u.unify(&T::Infer(var(0)), &term).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    assert!(u.binding(var(0)).is_none());
}
