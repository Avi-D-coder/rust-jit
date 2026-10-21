// `alloc::sync`: atomically reference-counted shared ownership. The guest
// store serializes every access, so the counts are `Cell`s exactly like
// `alloc::rc` (an atomic read-modify-write cannot be torn here); `Arc` is
// `Send`/`Sync` for `Send + Sync` contents as in real alloc. The allocation
// lives for the rest of the run (`Box::leak`) and the value is dropped, by the
// last strong owner, out of its `Option` slot. Sized contents only.
use crate::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use core::ops::Deref;

struct ArcInner<T> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: UnsafeCell<Option<T>>,
}

pub struct Arc<T> {
    ptr: &'static ArcInner<T>,
}
pub struct Weak<T> {
    ptr: Option<&'static ArcInner<T>>,
}

unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}
unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Arc<T> {
        let shared = Box::new(ArcInner {
            strong: Cell::new(1),
            weak: Cell::new(0),
            value: UnsafeCell::new(Some(data)),
        });
        Arc {
            ptr: core::intrinsics::lrr_box_leak(shared),
        }
    }
    // The contents (what `Deref::deref` returns).
    pub fn lrr_get(&self) -> &T {
        match self.ptr.value.lrr_get() {
            Option::Some(value) => value,
            Option::None => core::panicking::panic("Arc contents already dropped"),
        }
    }
    pub fn strong_count(this: &Arc<T>) -> usize {
        this.ptr.strong.get()
    }
    pub fn weak_count(this: &Arc<T>) -> usize {
        this.ptr.weak.get()
    }
    pub fn ptr_eq(this: &Arc<T>, other: &Arc<T>) -> bool {
        core::intrinsics::lrr_ref_eq(this.ptr, other.ptr)
    }
    pub fn downgrade(this: &Arc<T>) -> Weak<T> {
        let weak: usize = this.ptr.weak.get();
        this.ptr.weak.set(weak + 1);
        Weak { ptr: Option::Some(this.ptr) }
    }
    pub fn get_mut(this: &mut Arc<T>) -> Option<&mut T> {
        if this.ptr.strong.get() == 1 && this.ptr.weak.get() == 0 {
            this.ptr.value.lrr_get_mut().as_mut()
        } else {
            Option::None
        }
    }
    pub fn try_unwrap(this: Arc<T>) -> Result<T, Arc<T>> {
        if this.ptr.strong.get() == 1 {
            this.ptr.strong.set(0);
            let taken = this.ptr.value.lrr_replace(Option::None);
            core::mem::forget(this);
            match taken {
                Option::Some(value) => Result::Ok(value),
                Option::None => core::panicking::panic("Arc contents already taken"),
            }
        } else {
            Result::Err(this)
        }
    }
    pub fn into_inner(this: Arc<T>) -> Option<T> {
        match Arc::try_unwrap(this) {
            Result::Ok(value) => Option::Some(value),
            Result::Err(_) => Option::None,
        }
    }
}
impl<T: Clone> Arc<T> {
    pub fn make_mut(this: &mut Arc<T>) -> &mut T {
        if this.ptr.strong.get() != 1 || this.ptr.weak.get() != 0 {
            let copy = this.lrr_get().clone();
            *this = Arc::new(copy);
        }
        match this.ptr.value.lrr_get_mut() {
            Option::Some(value) => value,
            Option::None => core::panicking::panic("Arc contents already taken"),
        }
    }
    pub fn unwrap_or_clone(this: Arc<T>) -> T {
        match Arc::try_unwrap(this) {
            Result::Ok(value) => value,
            Result::Err(shared) => shared.lrr_get().clone(),
        }
    }
}
impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        let strong: usize = self.ptr.strong.get();
        self.ptr.strong.set(strong + 1);
        Arc { ptr: self.ptr }
    }
}
impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let strong: usize = self.ptr.strong.get();
        self.ptr.strong.set(strong - 1);
        if strong == 1 {
            let old = self.ptr.value.lrr_replace(Option::None);
            core::mem::drop(old);
        }
    }
}
impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lrr_get()
    }
}
impl<T> AsRef<T> for Arc<T> {
    fn as_ref(&self) -> &T {
        self.lrr_get()
    }
}
impl<T> core::borrow::Borrow<T> for Arc<T> {
    fn borrow(&self) -> &T {
        self.lrr_get()
    }
}
impl<T> From<T> for Arc<T> {
    fn from(value: T) -> Arc<T> {
        Arc::new(value)
    }
}
impl<T: Default> Default for Arc<T> {
    fn default() -> Arc<T> {
        Arc::new(T::default())
    }
}
impl<T: PartialEq> PartialEq for Arc<T> {
    fn eq(&self, other: &Arc<T>) -> bool {
        self.lrr_get() == other.lrr_get()
    }
}
impl<T: Eq> Eq for Arc<T> {}
impl<T: PartialOrd> PartialOrd for Arc<T> {
    fn partial_cmp(&self, other: &Arc<T>) -> Option<core::cmp::Ordering> {
        self.lrr_get().partial_cmp(other.lrr_get())
    }
}
impl<T: Ord> Ord for Arc<T> {
    fn cmp(&self, other: &Arc<T>) -> core::cmp::Ordering {
        self.lrr_get().cmp(other.lrr_get())
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Arc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.lrr_get(), f)
    }
}
impl<T: core::fmt::Display> core::fmt::Display for Arc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.lrr_get(), f)
    }
}
impl<T: core::hash::Hash> core::hash::Hash for Arc<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::hash::Hash::hash(self.lrr_get(), state)
    }
}

impl<T> Weak<T> {
    pub fn new() -> Weak<T> {
        Weak { ptr: Option::None }
    }
    pub fn upgrade(&self) -> Option<Arc<T>> {
        match self.ptr {
            Option::Some(ptr) => {
                let strong: usize = ptr.strong.get();
                if strong == 0 {
                    Option::None
                } else {
                    ptr.strong.set(strong + 1);
                    Option::Some(Arc { ptr: ptr })
                }
            }
            Option::None => Option::None,
        }
    }
    pub fn strong_count(&self) -> usize {
        match self.ptr {
            Option::Some(ptr) => ptr.strong.get(),
            Option::None => 0,
        }
    }
    pub fn weak_count(&self) -> usize {
        match self.ptr {
            Option::Some(ptr) => {
                if ptr.strong.get() == 0 {
                    0
                } else {
                    ptr.weak.get()
                }
            }
            Option::None => 0,
        }
    }
    pub fn ptr_eq(&self, other: &Weak<T>) -> bool {
        match self.ptr {
            Option::Some(a) => match other.ptr {
                Option::Some(b) => core::intrinsics::lrr_ref_eq(a, b),
                Option::None => false,
            },
            Option::None => match other.ptr {
                Option::Some(_) => false,
                Option::None => true,
            },
        }
    }
}
impl<T> Clone for Weak<T> {
    fn clone(&self) -> Weak<T> {
        match self.ptr {
            Option::Some(ptr) => {
                let weak: usize = ptr.weak.get();
                ptr.weak.set(weak + 1);
                Weak { ptr: Option::Some(ptr) }
            }
            Option::None => Weak { ptr: Option::None },
        }
    }
}
impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if let Option::Some(ptr) = self.ptr {
            let weak: usize = ptr.weak.get();
            ptr.weak.set(weak - 1);
        }
    }
}
impl<T> Default for Weak<T> {
    fn default() -> Weak<T> {
        Weak::new()
    }
}
impl<T> core::fmt::Debug for Weak<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("(Weak)")
    }
}

impl<T> core::clone::Share for Arc<T> {}
