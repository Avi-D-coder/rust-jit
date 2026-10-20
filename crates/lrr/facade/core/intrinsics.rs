// Public intrinsic declarations use ordinary source/member/call preparation.
// Signature: pinned core/src/intrinsics/mod.rs:2512, ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
use crate::marker::DiscriminantKind;

pub const fn discriminant_value<T>(v: &T) -> <T as DiscriminantKind>::Discriminant {
    core::intrinsics::lrr_discriminant_value(v)
}
