// `alloc::rc`.
use core::ops::Deref;

pub struct Rc<T: ?Sized> {
    ptr: *const T,
}
pub struct Weak<T: ?Sized> {
    ptr: *const T,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Rc<T> {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::new")
    }
}
impl<T: ?Sized> Rc<T> {
    pub fn strong_count(this: &Rc<T>) -> usize {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::strong_count")
    }
    pub fn ptr_eq(this: &Rc<T>, other: &Rc<T>) -> bool {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::ptr_eq")
    }
    pub fn downgrade(this: &Rc<T>) -> Weak<T> {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::downgrade")
    }
}
impl<T: ?Sized> Clone for Rc<T> {
    fn clone(&self) -> Rc<T> {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::clone")
    }
}
impl<T: ?Sized> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        core::intrinsics::lrr_unsupported("alloc::rc::Rc::deref")
    }
}
impl<T: ?Sized> Weak<T> {
    pub fn upgrade(&self) -> Option<Rc<T>> {
        core::intrinsics::lrr_unsupported("alloc::rc::Weak::upgrade")
    }
}
