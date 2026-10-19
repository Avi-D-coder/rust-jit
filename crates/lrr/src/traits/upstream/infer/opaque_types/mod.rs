//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
pub(crate) mod table;
use super::Ty;
pub(crate) use table::{OpaqueTypeStorage, OpaqueTypeTable};
#[derive(Copy, Clone, Debug)]
pub struct OpaqueHiddenType<'db> {
    pub ty: Ty<'db>,
}
