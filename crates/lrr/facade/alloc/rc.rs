// `alloc::rc`: shared ownership over a leaked reference-counted allocation.
// The allocation lives for the rest of the run (`Box::leak`); the counts
// are `Cell`s and the value is dropped, by the last strong owner, out of its
// `Option` slot. Sized contents only.
use crate::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use core::ops::Deref;

struct RcBox<T> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: UnsafeCell<Option<T>>,
}

pub struct Rc<T> {
    ptr: &'static RcBox<T>,
}
pub struct Weak<T> {
    ptr: Option<&'static RcBox<T>>,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Rc<T> {
        let shared = Box::new(RcBox {
            strong: Cell::new(1),
            weak: Cell::new(0),
            value: UnsafeCell::new(Some(value)),
        });
        Rc {
            ptr: core::intrinsics::lrr_box_leak(shared),
        }
    }
    // The contents (what `Deref::deref` returns).
    pub fn lrr_get(&self) -> &T {
        match self.ptr.value.lrr_get() {
            Option::Some(value) => value,
            Option::None => core::panicking::panic("Rc contents already dropped"),
        }
    }
    pub fn strong_count(this: &Rc<T>) -> usize {
        this.ptr.strong.get()
    }
    pub fn weak_count(this: &Rc<T>) -> usize {
        this.ptr.weak.get()
    }
    pub fn ptr_eq(this: &Rc<T>, other: &Rc<T>) -> bool {
        core::intrinsics::lrr_ref_eq(this.ptr, other.ptr)
    }
    pub fn downgrade(this: &Rc<T>) -> Weak<T> {
        let weak: usize = this.ptr.weak.get();
        this.ptr.weak.set(weak + 1);
        Weak { ptr: Option::Some(this.ptr) }
    }
    pub fn get_mut(this: &mut Rc<T>) -> Option<&mut T> {
        if this.ptr.strong.get() == 1 && this.ptr.weak.get() == 0 {
            match this.ptr.value.lrr_get_mut() {
                Option::Some(value) => Option::Some(value),
                Option::None => Option::None,
            }
        } else {
            Option::None
        }
    }
    pub fn try_unwrap(this: Rc<T>) -> Result<T, Rc<T>> {
        if this.ptr.strong.get() == 1 {
            this.ptr.strong.set(0);
            let taken = this.ptr.value.lrr_replace(Option::None);
            core::mem::forget(this);
            match taken {
                Option::Some(value) => Result::Ok(value),
                Option::None => core::panicking::panic("Rc contents already taken"),
            }
        } else {
            Result::Err(this)
        }
    }
    pub fn into_inner(this: Rc<T>) -> Option<T> {
        match Rc::try_unwrap(this) {
            Result::Ok(value) => Option::Some(value),
            Result::Err(_) => Option::None,
        }
    }
}
impl<T: Clone> Rc<T> {
    pub fn make_mut(this: &mut Rc<T>) -> &mut T {
        if this.ptr.strong.get() != 1 || this.ptr.weak.get() != 0 {
            let copy = this.lrr_get().clone();
            *this = Rc::new(copy);
        }
        match this.ptr.value.lrr_get_mut() {
            Option::Some(value) => value,
            Option::None => core::panicking::panic("Rc contents already taken"),
        }
    }
}
impl<T> Clone for Rc<T> {
    fn clone(&self) -> Rc<T> {
        let strong: usize = self.ptr.strong.get();
        self.ptr.strong.set(strong + 1);
        Rc { ptr: self.ptr }
    }
}
impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        let strong: usize = self.ptr.strong.get();
        self.ptr.strong.set(strong - 1);
        if strong == 1 {
            let old = self.ptr.value.lrr_replace(Option::None);
            core::mem::drop(old);
        }
    }
}
impl<T> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lrr_get()
    }
}
impl<T> AsRef<T> for Rc<T> {
    fn as_ref(&self) -> &T {
        self.lrr_get()
    }
}
impl<T> From<T> for Rc<T> {
    fn from(value: T) -> Rc<T> {
        Rc::new(value)
    }
}
impl<T: Default> Default for Rc<T> {
    fn default() -> Rc<T> {
        Rc::new(T::default())
    }
}
impl<T: PartialEq> PartialEq for Rc<T> {
    fn eq(&self, other: &Rc<T>) -> bool {
        self.lrr_get() == other.lrr_get()
    }
}
impl<T: Eq> Eq for Rc<T> {}
impl<T: PartialOrd> PartialOrd for Rc<T> {
    fn partial_cmp(&self, other: &Rc<T>) -> Option<core::cmp::Ordering> {
        self.lrr_get().partial_cmp(other.lrr_get())
    }
}
impl<T: Ord> Ord for Rc<T> {
    fn cmp(&self, other: &Rc<T>) -> core::cmp::Ordering {
        self.lrr_get().cmp(other.lrr_get())
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Rc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.lrr_get(), f)
    }
}
impl<T: core::fmt::Display> core::fmt::Display for Rc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.lrr_get(), f)
    }
}

impl<T> Weak<T> {
    pub fn new() -> Weak<T> {
        Weak { ptr: Option::None }
    }
    pub fn upgrade(&self) -> Option<Rc<T>> {
        match self.ptr {
            Option::Some(ptr) => {
                let strong: usize = ptr.strong.get();
                if strong == 0 {
                    Option::None
                } else {
                    ptr.strong.set(strong + 1);
                    Option::Some(Rc { ptr: ptr })
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
