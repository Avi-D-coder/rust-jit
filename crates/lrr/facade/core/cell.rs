// `core::cell`. Interior mutability is an interpreter intrinsic.
use crate::marker::Sized;
use crate::option::Option;

// Interior mutability: a value replaced through a shared reference
// (`lrr_cell_replace`), moved out of one (`lrr_cell_take`) or leased mutably
// from one (`lrr_shared_to_mut`) -- exactly the access rustc's `UnsafeCell`
// grants, which rustc (not the interpreter) checks.
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
    // The contents replaced through a shared reference.
    pub fn lrr_replace(&self, val: T) -> T {
        core::intrinsics::lrr_cell_replace(&self.value, val)
    }
}
impl<T: ?Sized> UnsafeCell<T> {
    // A shared view of the contents.
    pub fn lrr_get(&self) -> &T {
        &self.value
    }
    pub const fn get(&self) -> *mut T {
        core::intrinsics::lrr_unsupported("core::cell::UnsafeCell::get")
    }
    // `&mut` access to the contents through a shared reference (the safe
    // spelling of dereferencing `get()`).
    pub fn lrr_get_mut(&self) -> &mut T {
        core::intrinsics::lrr_shared_to_mut(&self.value)
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
        let old = core::intrinsics::lrr_cell_replace(&self.value.value, val);
        core::mem::drop(old);
    }
    pub fn replace(&self, val: T) -> T {
        core::intrinsics::lrr_cell_replace(&self.value.value, val)
    }
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}
impl<T: crate::marker::Copy> Cell<T> {
    pub fn get(&self) -> T {
        self.value.value
    }
}
impl<T: crate::default::Default> Cell<T> {
    pub fn take(&self) -> T {
        self.replace(T::default())
    }
}
impl<T: crate::default::Default> crate::default::Default for Cell<T> {
    fn default() -> Cell<T> {
        Cell::new(T::default())
    }
}
impl<T: crate::marker::Copy> crate::clone::Clone for Cell<T> {
    fn clone(&self) -> Cell<T> {
        Cell::new(self.get())
    }
}
impl<T: crate::marker::Copy + crate::fmt::Debug> crate::fmt::Debug for Cell<T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.debug_struct("Cell").field("value", &self.get()).finish()
    }
}
impl<T: crate::marker::Copy + crate::cmp::PartialEq> crate::cmp::PartialEq for Cell<T> {
    fn eq(&self, other: &Cell<T>) -> bool {
        self.get() == other.get()
    }
}

pub struct BorrowError;
pub struct BorrowMutError;
impl crate::fmt::Debug for BorrowError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("BorrowError")
    }
}
impl crate::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("already mutably borrowed")
    }
}
impl crate::fmt::Debug for BorrowMutError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("BorrowMutError")
    }
}
impl crate::fmt::Display for BorrowMutError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("already borrowed")
    }
}

// `borrow` counts the shared borrows, or is -1 while mutably borrowed.
pub struct RefCell<T: ?Sized> {
    borrow: Cell<isize>,
    value: UnsafeCell<T>,
}
pub struct Ref<'b, T: ?Sized + 'b> {
    value: &'b T,
    borrow: &'b Cell<isize>,
}
pub struct RefMut<'b, T: ?Sized + 'b> {
    value: &'b mut T,
    borrow: &'b Cell<isize>,
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
        let mut guard = self.borrow_mut();
        core::mem::replace(guard.lrr_get_mut(), t)
    }
}
impl<T: ?Sized> RefCell<T> {
    pub fn borrow(&self) -> Ref<'_, T> {
        let state: isize = self.borrow.get();
        if state < 0 {
            core::panicking::panic("already mutably borrowed: BorrowError");
        }
        self.borrow.set(state + 1);
        Ref { value: &self.value.value, borrow: &self.borrow }
    }
    pub fn try_borrow(&self) -> crate::result::Result<Ref<'_, T>, BorrowError> {
        let state: isize = self.borrow.get();
        if state < 0 {
            return crate::result::Result::Err(BorrowError);
        }
        self.borrow.set(state + 1);
        crate::result::Result::Ok(Ref { value: &self.value.value, borrow: &self.borrow })
    }
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        if self.borrow.get() != 0 {
            core::panicking::panic("already borrowed: BorrowMutError");
        }
        self.borrow.set(-1);
        RefMut { value: self.value.lrr_get_mut(), borrow: &self.borrow }
    }
    pub fn try_borrow_mut(&self) -> crate::result::Result<RefMut<'_, T>, BorrowMutError> {
        if self.borrow.get() != 0 {
            return crate::result::Result::Err(BorrowMutError);
        }
        self.borrow.set(-1);
        crate::result::Result::Ok(RefMut { value: self.value.lrr_get_mut(), borrow: &self.borrow })
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value.value
    }
}
impl<T: crate::default::Default> crate::default::Default for RefCell<T> {
    fn default() -> RefCell<T> {
        RefCell::new(T::default())
    }
}
impl<T: crate::clone::Clone> crate::clone::Clone for RefCell<T> {
    fn clone(&self) -> RefCell<T> {
        RefCell::new(self.borrow().lrr_get().clone())
    }
}
impl<T: crate::cmp::PartialEq> crate::cmp::PartialEq for RefCell<T> {
    fn eq(&self, other: &RefCell<T>) -> bool {
        self.borrow().lrr_get() == other.borrow().lrr_get()
    }
}
impl<T: crate::fmt::Debug> crate::fmt::Debug for RefCell<T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self.try_borrow() {
            crate::result::Result::Ok(guard) => f.debug_struct("RefCell").field("value", &guard).finish(),
            crate::result::Result::Err(_) => f.write_str("RefCell { value: <borrowed> }"),
        }
    }
}
impl<'b, T: ?Sized> Ref<'b, T> {
    // The contents (what `Deref::deref` returns).
    pub fn lrr_get(&self) -> &T {
        self.value
    }
}
impl<'b, T: ?Sized> RefMut<'b, T> {
    pub fn lrr_get(&self) -> &T {
        self.value
    }
    pub fn lrr_get_mut(&mut self) -> &mut T {
        self.value
    }
}
impl<'b, T: ?Sized> Drop for Ref<'b, T> {
    fn drop(&mut self) {
        let state: isize = self.borrow.get();
        self.borrow.set(state - 1);
    }
}
impl<'b, T: ?Sized> Drop for RefMut<'b, T> {
    fn drop(&mut self) {
        self.borrow.set(0);
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
impl<T: ?Sized + crate::fmt::Debug> crate::fmt::Debug for Ref<'_, T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Debug::fmt(self.value, f)
    }
}
impl<T: ?Sized + crate::fmt::Display> crate::fmt::Display for Ref<'_, T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Display::fmt(self.value, f)
    }
}
impl<T: ?Sized + crate::fmt::Debug> crate::fmt::Debug for RefMut<'_, T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Debug::fmt(&*self.value, f)
    }
}
impl<T: ?Sized + crate::fmt::Display> crate::fmt::Display for RefMut<'_, T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Display::fmt(&*self.value, f)
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
