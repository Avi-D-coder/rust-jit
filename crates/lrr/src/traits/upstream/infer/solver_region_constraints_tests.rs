//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{DbInterner, Span};
use rustc_type_ir::region_constraint::{And, LeafRegionConstraint, Or};
pub(super) fn canonicalization_preserves_only_one_ambiguity<'db>(
    _cx: DbInterner<'db>,
    first: Span,
    second: Span,
) {
    let first = LeafRegionConstraint::Ambiguity::<DbInterner<'db>, _>(first);
    let second = LeafRegionConstraint::Ambiguity::<DbInterner<'db>, _>(second);

    let c = And::new([first.clone(), second.clone()]);
    assert_eq!(c.0.len(), 1);

    let c = Or::new([And::new([first]), And::new([second])]);
    assert_eq!(c.0.len(), 1);
}
