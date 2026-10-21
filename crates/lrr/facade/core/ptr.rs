// `core::ptr` (E-R5-008). A raw pointer is a `Copy` value over a synthetic
// address (see `runtime/cell_pointers.rs`); null is address 0. The inherent
// methods are ordinary Rust over `usize` addresses, with the
// `core::intrinsics::lrr_ptr_*` intrinsics only where the interpreter's cell
// store is needed (address arithmetic scaled by the pointee, reads, writes).
use crate::marker::Sized;

pub const fn null<T>() -> *const T {
    core::intrinsics::lrr_ptr_null()
}

pub const fn null_mut<T>() -> *mut T {
    core::intrinsics::lrr_ptr_null_mut()
}

pub unsafe fn read<T>(src: *const T) -> T {
    core::intrinsics::lrr_ptr_read(src)
}

pub unsafe fn write<T>(dst: *mut T, src: T) {
    core::intrinsics::lrr_ptr_write(dst, src)
}

pub unsafe fn drop_in_place<T: ?Sized>(to_drop: *mut T) {
    core::intrinsics::lrr_unsupported("core::ptr::drop_in_place")
}

pub fn eq<T: ?Sized>(a: *const T, b: *const T) -> bool {
    (a as usize) == (b as usize)
}

impl<T: ?Sized> *const T {
    pub fn is_null(self) -> bool {
        (self as usize) == 0
    }
    pub fn addr(self) -> usize {
        self as usize
    }
    pub const fn cast<U>(self) -> *const U {
        core::intrinsics::lrr_ptr_retype(self)
    }
    pub const fn cast_mut(self) -> *mut T {
        core::intrinsics::lrr_ptr_as_mut(self)
    }
    pub unsafe fn as_ref<'a>(self) -> crate::option::Option<&'a T> {
        if (self as usize) == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&*self)
        }
    }
    pub unsafe fn offset(self, count: isize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, count)
    }
    pub unsafe fn add(self, count: usize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, count as isize)
    }
    pub unsafe fn sub(self, count: usize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, -(count as isize))
    }
    pub fn wrapping_offset(self, count: isize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, count)
    }
    pub fn wrapping_add(self, count: usize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, count as isize)
    }
    pub fn wrapping_sub(self, count: usize) -> *const T {
        core::intrinsics::lrr_ptr_offset(self, -(count as isize))
    }
    pub unsafe fn offset_from(self, origin: *const T) -> isize {
        core::intrinsics::lrr_ptr_offset_from(self, origin)
    }
    pub unsafe fn read(self) -> T {
        core::intrinsics::lrr_ptr_read(self)
    }
}

impl<T: ?Sized> *mut T {
    pub fn is_null(self) -> bool {
        (self as usize) == 0
    }
    pub fn addr(self) -> usize {
        self as usize
    }
    pub const fn cast<U>(self) -> *mut U {
        core::intrinsics::lrr_ptr_retype(self)
    }
    pub const fn cast_const(self) -> *const T {
        core::intrinsics::lrr_ptr_as_const(self)
    }
    pub unsafe fn as_ref<'a>(self) -> crate::option::Option<&'a T> {
        if (self as usize) == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&*self)
        }
    }
    pub unsafe fn as_mut<'a>(self) -> crate::option::Option<&'a mut T> {
        if (self as usize) == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(&mut *self)
        }
    }
    pub unsafe fn offset(self, count: isize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, count)
    }
    pub unsafe fn add(self, count: usize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, count as isize)
    }
    pub unsafe fn sub(self, count: usize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, -(count as isize))
    }
    pub fn wrapping_offset(self, count: isize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, count)
    }
    pub fn wrapping_add(self, count: usize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, count as isize)
    }
    pub fn wrapping_sub(self, count: usize) -> *mut T {
        core::intrinsics::lrr_ptr_offset(self, -(count as isize))
    }
    pub unsafe fn offset_from(self, origin: *const T) -> isize {
        core::intrinsics::lrr_ptr_offset_from(self, origin)
    }
    pub unsafe fn read(self) -> T {
        core::intrinsics::lrr_ptr_read(self)
    }
    pub unsafe fn write(self, value: T) {
        core::intrinsics::lrr_ptr_write(self, value)
    }
}

impl<T: ?Sized> crate::clone::Clone for *const T {
    fn clone(&self) -> *const T {
        *self
    }
}
impl<T: ?Sized> crate::marker::Copy for *const T {}
impl<T: ?Sized> crate::clone::Clone for *mut T {
    fn clone(&self) -> *mut T {
        *self
    }
}
impl<T: ?Sized> crate::marker::Copy for *mut T {}

impl<T: ?Sized> crate::cmp::PartialEq for *const T {
    fn eq(&self, other: &*const T) -> bool {
        (*self as usize) == (*other as usize)
    }
}
impl<T: ?Sized> crate::cmp::Eq for *const T {}
impl<T: ?Sized> crate::cmp::PartialEq for *mut T {
    fn eq(&self, other: &*mut T) -> bool {
        (*self as usize) == (*other as usize)
    }
}
impl<T: ?Sized> crate::cmp::Eq for *mut T {}

// Raw pointers order by address (real core compares the pointers with the
// built-in operators, which order by address).
impl<T: ?Sized> crate::cmp::PartialOrd for *const T {
    fn partial_cmp(&self, other: &*const T) -> crate::option::Option<crate::cmp::Ordering> {
        crate::option::Option::Some(crate::cmp::Ord::cmp(&(*self as usize), &(*other as usize)))
    }
}
impl<T: ?Sized> crate::cmp::Ord for *const T {
    fn cmp(&self, other: &*const T) -> crate::cmp::Ordering {
        crate::cmp::Ord::cmp(&(*self as usize), &(*other as usize))
    }
}
impl<T: ?Sized> crate::cmp::PartialOrd for *mut T {
    fn partial_cmp(&self, other: &*mut T) -> crate::option::Option<crate::cmp::Ordering> {
        crate::option::Option::Some(crate::cmp::Ord::cmp(&(*self as usize), &(*other as usize)))
    }
}
impl<T: ?Sized> crate::cmp::Ord for *mut T {
    fn cmp(&self, other: &*mut T) -> crate::cmp::Ordering {
        crate::cmp::Ord::cmp(&(*self as usize), &(*other as usize))
    }
}

// Every fn pointer type selects these impls by the reserved fn-pointer
// identity; the bodies compare code addresses and so serve any signature.
impl crate::cmp::PartialEq for fn() {
    fn eq(&self, other: &fn()) -> bool {
        (*self as usize) == (*other as usize)
    }
}
impl crate::cmp::Eq for fn() {}
impl crate::clone::Clone for fn() {
    fn clone(&self) -> fn() {
        *self
    }
}

pub struct NonNull<T: ?Sized> {
    pointer: *const T,
}

impl<T: ?Sized> NonNull<T> {
    pub unsafe fn new_unchecked(ptr: *mut T) -> NonNull<T> {
        NonNull { pointer: ptr as *const T }
    }
    pub fn new(ptr: *mut T) -> crate::option::Option<NonNull<T>> {
        if (ptr as usize) == 0 {
            crate::option::Option::None
        } else {
            crate::option::Option::Some(NonNull { pointer: ptr as *const T })
        }
    }
    pub const fn as_ptr(self) -> *mut T {
        core::intrinsics::lrr_ptr_as_mut(self.pointer)
    }
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        &*self.pointer
    }
    pub unsafe fn as_mut<'a>(&mut self) -> &'a mut T {
        &mut *self.as_ptr()
    }
    pub const fn cast<U>(self) -> NonNull<U> {
        NonNull { pointer: core::intrinsics::lrr_ptr_retype(self.pointer) }
    }
    pub fn addr(self) -> usize {
        self.pointer as usize
    }
}
impl<T> NonNull<T> {
    pub const fn dangling() -> NonNull<T> {
        NonNull { pointer: core::intrinsics::lrr_ptr_retype(core::mem::align_of::<T>() as *const u8) }
    }
}
impl<T: ?Sized> crate::clone::Clone for NonNull<T> {
    fn clone(&self) -> NonNull<T> {
        NonNull { pointer: self.pointer }
    }
}
impl<T: ?Sized> crate::marker::Copy for NonNull<T> {}
impl<T: ?Sized> crate::cmp::PartialEq for NonNull<T> {
    fn eq(&self, other: &NonNull<T>) -> bool {
        (self.pointer as usize) == (other.pointer as usize)
    }
}
impl<T: ?Sized> crate::cmp::Eq for NonNull<T> {}
impl<T: ?Sized> crate::fmt::Debug for NonNull<T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Debug::fmt(&(self.pointer as usize), f)
    }
}
