// `alloc::sync`.
use core::ops::Deref;

pub struct Arc<T: ?Sized> {
    ptr: *const T,
}
pub struct Weak<T: ?Sized> {
    ptr: *const T,
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Arc<T> {
        core::intrinsics::lrr_unsupported("alloc::sync::Arc::new")
    }
}
impl<T: ?Sized> Arc<T> {
    pub fn strong_count(this: &Arc<T>) -> usize {
        core::intrinsics::lrr_unsupported("alloc::sync::Arc::strong_count")
    }
}
impl<T: ?Sized> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        core::intrinsics::lrr_unsupported("alloc::sync::Arc::clone")
    }
}
impl<T: ?Sized> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        core::intrinsics::lrr_unsupported("alloc::sync::Arc::deref")
    }
}
