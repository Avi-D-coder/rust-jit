// `alloc::intrinsics`: library-internal leaves named by `vec!`.
pub fn write_box_via_move<T>(
    b: crate::boxed::Box<core::mem::MaybeUninit<T>>,
    x: T,
) -> crate::boxed::Box<core::mem::MaybeUninit<T>> {
    core::intrinsics::lrr_unsupported("alloc::intrinsics::write_box_via_move")
}
