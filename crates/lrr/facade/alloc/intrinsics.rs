// `alloc::intrinsics`: library-internal leaves named by `vec!`.
pub fn write_box_via_move<T>(b: crate::boxed::UninitBox, x: T) -> crate::boxed::Box<T> {
    crate::boxed::Box::new(x)
}
