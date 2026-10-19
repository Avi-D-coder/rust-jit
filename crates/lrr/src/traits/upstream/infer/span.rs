//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::Span;
impl Span {
    pub(crate) fn pick_best(a: Span, b: Span) -> Span {
        // We prefer dummy spans to minimize the risk of false errors.
        if b.is_dummy() { b } else { a }
    }

    pub(crate) fn is_dummy(&self) -> bool {
        self.origin == crate::core::digest::Digest::default() && self.start == 0 && self.end == 0
    }
}
