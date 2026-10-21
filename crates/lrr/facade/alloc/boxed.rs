// `alloc::boxed`. A `Box<T>` owns one heap cell holding its `T`: the box
// object's single field cell. Moving a box moves that object (the cell keeps
// its identity, as a heap allocation does); dropping it drops the contents
// through ordinary drop glue and then releases the object. `*b` is the
// interpreter's builtin box dereference projection (rustc MIR's `Deref` place
// projection on a box), never a call of `Deref::deref`: it can be read,
// written, borrowed and moved out of. Method calls and field accesses
// autoderef through it like through a reference.
use core::ops::{Deref, DerefMut};

pub struct Box<T: ?Sized>(T);

// An allocation whose contents are not written yet (see `Box::new_uninit`).
pub struct UninitBox;

impl<T> Box<T> {
    pub fn new(x: T) -> Box<T> {
        Box(x)
    }
    // The allocation `vec![a, b, ..]` writes its array into. Its element
    // type is only fixed by the value written (`write_box_via_move`), which
    // the call `Box::new_uninit()` cannot know when it runs, so the
    // allocation is the untyped `UninitBox`.
    pub fn new_uninit() -> UninitBox {
        UninitBox
    }
    pub fn into_inner(boxed: Box<T>) -> T {
        *boxed
    }
    pub fn pin(x: T) -> core::pin::Pin<Box<T>> {
        core::intrinsics::lrr_unsupported("alloc::boxed::Box::pin")
    }
}

// Pinned alloc/src/boxed/convert.rs: From<T> allocates and moves T.
impl<T> core::convert::From<T> for Box<T> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

impl<T: ?Sized> Box<T> {
    pub fn leak<'a>(b: Box<T>) -> &'a mut T {
        core::intrinsics::lrr_shared_to_mut(core::intrinsics::lrr_box_leak(b))
    }
}

impl<T: ?Sized> Deref for Box<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &**self
    }
}
impl<T: ?Sized> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut **self
    }
}

// Pinned alloc/src/boxed.rs:2407-2411. Ordinary deref coercion supplies &T.
impl<T: ?Sized> core::convert::AsRef<T> for Box<T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: Clone> Clone for Box<T> {
    fn clone(&self) -> Box<T> {
        Box::new((**self).clone())
    }
}

impl<T: Default> Default for Box<T> {
    fn default() -> Box<T> {
        Box::new(T::default())
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Box<T> {
    fn eq(&self, other: &Box<T>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}
impl<T: ?Sized + Eq> Eq for Box<T> {}
impl<T: ?Sized + PartialOrd> PartialOrd for Box<T> {
    fn partial_cmp(&self, other: &Box<T>) -> Option<core::cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
    fn lt(&self, other: &Box<T>) -> bool {
        PartialOrd::lt(&**self, &**other)
    }
    fn le(&self, other: &Box<T>) -> bool {
        PartialOrd::le(&**self, &**other)
    }
    fn gt(&self, other: &Box<T>) -> bool {
        PartialOrd::gt(&**self, &**other)
    }
    fn ge(&self, other: &Box<T>) -> bool {
        PartialOrd::ge(&**self, &**other)
    }
}
impl<T: ?Sized + Ord> Ord for Box<T> {
    fn cmp(&self, other: &Box<T>) -> core::cmp::Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for Box<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}
impl<T: ?Sized + core::fmt::Display> core::fmt::Display for Box<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&**self, f)
    }
}

// `vec![a, b, ..]` support: converts the initialized boxed array to a `Vec`
// by moving its elements into the vector's element cells.
pub fn box_assume_init_into_vec_unsafe<T, const N: usize>(
    b: Box<[T; N]>,
) -> crate::vec::Vec<T> {
    crate::vec::lrr_vec_from_array(*b, crate::vec::Vec::new())
}

impl<T: ?Sized + core::hash::Hash> core::hash::Hash for Box<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::hash::Hash::hash(&**self, state)
    }
}

// `ThinBox` (unstable `thin_box`): an owning pointer that is one machine word
// for any `T`, sized or not. The interpreter has no pointer-with-header
// allocation, so the word points at a leaked cell holding the ordinary (fat)
// `Box<T>`; dereferencing follows both indirections. Dropping takes the `Box`
// out of its cell and drops it, running the contents' destructor exactly once.
pub struct ThinBox<T: ?Sized> {
    ptr: *const core::cell::UnsafeCell<Option<Box<T>>>,
}

impl<T: ?Sized> ThinBox<T> {
    fn lrr_from_box(inner: Box<T>) -> ThinBox<T> {
        let cell: &'static core::cell::UnsafeCell<Option<Box<T>>> =
            core::intrinsics::lrr_box_leak(Box::new(core::cell::UnsafeCell::new(Option::Some(inner))));
        ThinBox {
            ptr: cell as *const core::cell::UnsafeCell<Option<Box<T>>>,
        }
    }
    fn lrr_cell(&self) -> &core::cell::UnsafeCell<Option<Box<T>>> {
        unsafe { &*self.ptr }
    }
}

impl<T> ThinBox<T> {
    pub fn new(value: T) -> ThinBox<T> {
        ThinBox::lrr_from_box(Box::new(value))
    }
}

impl<Dyn: ?Sized> ThinBox<Dyn> {
    pub fn new_unsize<T>(value: T) -> ThinBox<Dyn>
    where
        T: core::marker::Unsize<Dyn>,
    {
        let inner: Box<Dyn> = Box::new(value);
        // Built here rather than through `lrr_from_box`: the interpreter does
        // not yet carry an unsized `Dyn` through that nested generic call.
        let cell: &'static core::cell::UnsafeCell<Option<Box<Dyn>>> =
            core::intrinsics::lrr_box_leak(Box::new(core::cell::UnsafeCell::new(Option::Some(inner))));
        ThinBox {
            ptr: cell as *const core::cell::UnsafeCell<Option<Box<Dyn>>>,
        }
    }
}

impl<T: ?Sized> Deref for ThinBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self.lrr_cell().lrr_get() {
            Option::Some(contents) => &**contents,
            Option::None => core::panicking::panic("ThinBox contents already dropped"),
        }
    }
}
impl<T: ?Sized> DerefMut for ThinBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        match self.lrr_cell().lrr_get_mut() {
            Option::Some(contents) => &mut **contents,
            Option::None => core::panicking::panic("ThinBox contents already dropped"),
        }
    }
}
impl<T: ?Sized> Drop for ThinBox<T> {
    fn drop(&mut self) {
        let old = self.lrr_cell().lrr_replace(Option::None);
        core::mem::drop(old);
    }
}
impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for ThinBox<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}
impl<T: ?Sized + core::fmt::Display> core::fmt::Display for ThinBox<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&**self, f)
    }
}
