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

// Layout queries (E-R5-012). The wrappers are ordinary generic functions, so
// their frame binds `T`; the closed intrinsics read it from the calling frame
// and answer from the interpreter's one layout algorithm.
pub const fn size_of<T>() -> usize {
    core::intrinsics::lrr_size_of()
}

// `offset_of!(Container, a.b.0)` is lowered by the adapter's builtin expander to
// a call of this wrapper, whose frame binds the container type.
pub const fn lrr_offset_of_in<T: ?Sized>(path: &str) -> usize {
    core::intrinsics::lrr_offset_of(path)
}

pub const fn size_of_val<T: ?Sized>(val: &T) -> usize {
    core::intrinsics::lrr_size_of_val(val)
}

pub const fn align_of<T>() -> usize {
    core::intrinsics::lrr_align_of()
}

pub const fn align_of_val<T: ?Sized>(val: &T) -> usize {
    core::intrinsics::lrr_align_of_val(val)
}

pub const fn needs_drop<T: ?Sized>() -> bool {
    core::intrinsics::lrr_needs_drop()
}

pub const fn variant_count<T>() -> usize {
    core::intrinsics::lrr_variant_count()
}

// Between types of equal size that have a byte image (scalars, raw pointers,
// arrays and tuples of them); anything else is refused naming the pair.
pub const unsafe fn transmute<Src, Dst>(src: Src) -> Dst {
    core::intrinsics::lrr_transmute(src)
}

pub const unsafe fn zeroed<T>() -> T {
    core::intrinsics::lrr_zeroed()
}

// The discriminant value of an enum value (0 for any other value), compared,
// hashed and printed as that value.
pub struct Discriminant<T> {
    value: isize,
    marker: crate::marker::PhantomData<T>,
}

pub const fn discriminant<T>(v: &T) -> Discriminant<T> {
    Discriminant {
        value: core::intrinsics::lrr_discriminant_value(v),
        marker: crate::marker::PhantomData,
    }
}

impl<T> crate::marker::Copy for Discriminant<T> {}
impl<T> crate::clone::Clone for Discriminant<T> {
    fn clone(&self) -> Discriminant<T> {
        *self
    }
}
impl<T> crate::cmp::PartialEq for Discriminant<T> {
    fn eq(&self, rhs: &Discriminant<T>) -> bool {
        self.value == rhs.value
    }
}
impl<T> crate::cmp::Eq for Discriminant<T> {}
impl<T> crate::hash::Hash for Discriminant<T> {
    fn hash<H: crate::hash::Hasher>(&self, state: &mut H) {
        crate::hash::Hash::hash(&self.value, state)
    }
}
impl<T> crate::fmt::Debug for Discriminant<T> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.debug_tuple("Discriminant").field(&self.value).finish()
    }
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
