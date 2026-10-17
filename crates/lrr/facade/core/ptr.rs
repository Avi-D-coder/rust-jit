// `core::ptr`. Raw pointers are outside the interpreter's current profile;
// the names resolve.
use crate::marker::Sized;

pub const fn null<T>() -> *const T {
    core::intrinsics::lrr_unsupported("core::ptr::null")
}

pub const fn null_mut<T>() -> *mut T {
    core::intrinsics::lrr_unsupported("core::ptr::null_mut")
}

pub unsafe fn read<T>(src: *const T) -> T {
    core::intrinsics::lrr_unsupported("core::ptr::read")
}

pub unsafe fn write<T>(dst: *mut T, src: T) {
    core::intrinsics::lrr_unsupported("core::ptr::write")
}

pub unsafe fn drop_in_place<T: ?Sized>(to_drop: *mut T) {
    core::intrinsics::lrr_unsupported("core::ptr::drop_in_place")
}

pub fn eq<T: ?Sized>(a: *const T, b: *const T) -> bool {
    core::intrinsics::lrr_unsupported("core::ptr::eq")
}


pub struct NonNull<T: ?Sized> {
    pointer: *const T,
}
