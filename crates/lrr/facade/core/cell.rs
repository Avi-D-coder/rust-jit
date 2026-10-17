// `core::cell`. Interior mutability is an interpreter intrinsic.
use crate::marker::Sized;
use crate::option::Option;

pub struct UnsafeCell<T: ?Sized> {
    value: T,
}
impl<T> UnsafeCell<T> {
    pub const fn new(value: T) -> UnsafeCell<T> {
        UnsafeCell { value }
    }
    pub fn into_inner(self) -> T {
        self.value
    }
}
impl<T: ?Sized> UnsafeCell<T> {
    pub const fn get(&self) -> *mut T {
        core::intrinsics::lrr_unsupported("core::cell::UnsafeCell::get")
    }
}

pub struct Cell<T: ?Sized> {
    value: UnsafeCell<T>,
}
impl<T> Cell<T> {
    pub const fn new(value: T) -> Cell<T> {
        Cell {
            value: UnsafeCell::new(value),
        }
    }
    pub fn set(&self, val: T) {
        core::intrinsics::lrr_unsupported("core::cell::Cell::set")
    }
    pub fn replace(&self, val: T) -> T {
        core::intrinsics::lrr_unsupported("core::cell::Cell::replace")
    }
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}
impl<T: crate::marker::Copy> Cell<T> {
    pub fn get(&self) -> T {
        core::intrinsics::lrr_unsupported("core::cell::Cell::get")
    }
}
impl<T: crate::default::Default> Cell<T> {
    pub fn take(&self) -> T {
        self.replace(T::default())
    }
}

pub struct BorrowError;
pub struct BorrowMutError;

pub struct RefCell<T: ?Sized> {
    borrow: Cell<isize>,
    value: UnsafeCell<T>,
}
pub struct Ref<'b, T: ?Sized + 'b> {
    value: &'b T,
}
pub struct RefMut<'b, T: ?Sized + 'b> {
    value: &'b mut T,
}
impl<T> RefCell<T> {
    pub const fn new(value: T) -> RefCell<T> {
        RefCell {
            borrow: Cell::new(0),
            value: UnsafeCell::new(value),
        }
    }
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
    pub fn replace(&self, t: T) -> T {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::replace")
    }
}
impl<T: ?Sized> RefCell<T> {
    pub fn borrow(&self) -> Ref<'_, T> {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::borrow")
    }
    pub fn try_borrow(&self) -> crate::result::Result<Ref<'_, T>, BorrowError> {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::try_borrow")
    }
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::borrow_mut")
    }
    pub fn try_borrow_mut(&self) -> crate::result::Result<RefMut<'_, T>, BorrowMutError> {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::try_borrow_mut")
    }
    pub fn get_mut(&mut self) -> &mut T {
        core::intrinsics::lrr_unsupported("core::cell::RefCell::get_mut")
    }
}
impl<T: ?Sized> crate::ops::Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}
impl<T: ?Sized> crate::ops::Deref for RefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}
impl<T: ?Sized> crate::ops::DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

pub struct OnceCell<T> {
    inner: UnsafeCell<Option<T>>,
}
impl<T> OnceCell<T> {
    pub const fn new() -> OnceCell<T> {
        OnceCell {
            inner: UnsafeCell::new(Option::None),
        }
    }
    pub fn get(&self) -> Option<&T> {
        core::intrinsics::lrr_unsupported("core::cell::OnceCell::get")
    }
    pub fn set(&self, value: T) -> crate::result::Result<(), T> {
        core::intrinsics::lrr_unsupported("core::cell::OnceCell::set")
    }
}
