// `core::mem`. Ownership leaves are interpreter intrinsics.
use crate::default::Default;
use crate::marker::Sized;

// Moves `_x` into this call; it is dropped when the call returns.
pub fn drop<T>(_x: T) {}

pub fn forget<T>(t: T) {
    core::intrinsics::lrr_forget(t)
}

pub fn replace<T>(dest: &mut T, src: T) -> T {
    core::intrinsics::lrr_mem_replace(dest, src)
}

pub fn swap<T>(x: &mut T, y: &mut T) {
    core::intrinsics::lrr_mem_swap(x, y)
}

pub fn take<T: Default>(dest: &mut T) -> T {
    replace(dest, T::default())
}

pub const fn size_of<T>() -> usize {
    core::intrinsics::lrr_unsupported("core::mem::size_of")
}

pub fn size_of_val<T: ?Sized>(val: &T) -> usize {
    core::intrinsics::lrr_unsupported("core::mem::size_of_val")
}

pub const fn align_of<T>() -> usize {
    core::intrinsics::lrr_unsupported("core::mem::align_of")
}

pub fn align_of_val<T: ?Sized>(val: &T) -> usize {
    core::intrinsics::lrr_unsupported("core::mem::align_of_val")
}

pub const fn needs_drop<T: ?Sized>() -> bool {
    core::intrinsics::lrr_unsupported("core::mem::needs_drop")
}

pub unsafe fn transmute<Src, Dst>(src: Src) -> Dst {
    core::intrinsics::lrr_unsupported("core::mem::transmute")
}

pub unsafe fn zeroed<T>() -> T {
    core::intrinsics::lrr_unsupported("core::mem::zeroed")
}

pub struct Discriminant<T> {
    marker: crate::marker::PhantomData<T>,
}

pub fn discriminant<T>(v: &T) -> Discriminant<T> {
    core::intrinsics::lrr_unsupported("core::mem::discriminant")
}

// Inhibits the destructor of the wrapped value.
pub struct ManuallyDrop<T: ?Sized> {
    value: T,
}

impl<T> ManuallyDrop<T> {
    pub const fn new(value: T) -> ManuallyDrop<T> {
        ManuallyDrop { value }
    }
    pub fn into_inner(slot: ManuallyDrop<T>) -> T {
        core::intrinsics::lrr_unsupported("core::mem::ManuallyDrop::into_inner")
    }
    pub unsafe fn drop(slot: &mut ManuallyDrop<T>) {
        core::intrinsics::lrr_unsupported("core::mem::ManuallyDrop::drop")
    }
    pub unsafe fn take(slot: &mut ManuallyDrop<T>) -> T {
        core::intrinsics::lrr_unsupported("core::mem::ManuallyDrop::take")
    }
}

impl<T: ?Sized> crate::ops::Deref for ManuallyDrop<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T: ?Sized> crate::ops::DerefMut for ManuallyDrop<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

pub union MaybeUninit<T> {
    uninit: (),
    value: ManuallyDrop<T>,
}

impl<T> MaybeUninit<T> {
    pub const fn new(val: T) -> MaybeUninit<T> {
        MaybeUninit {
            value: ManuallyDrop::new(val),
        }
    }
    pub const fn uninit() -> MaybeUninit<T> {
        MaybeUninit { uninit: () }
    }
}
