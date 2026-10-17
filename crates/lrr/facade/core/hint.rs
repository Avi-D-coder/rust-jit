// `core::hint`.
pub fn black_box<T>(dummy: T) -> T {
    dummy
}

pub unsafe fn unreachable_unchecked() -> ! {
    core::intrinsics::lrr_unsupported("core::hint::unreachable_unchecked")
}

pub fn spin_loop() {}

pub const fn must_use<T>(value: T) -> T {
    value
}

pub unsafe fn assert_unchecked(cond: bool) {
}
