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
