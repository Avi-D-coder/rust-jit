// `std::sync`.
pub use alloc_crate::sync::{Arc, Weak};
pub use core::sync::atomic;

pub struct Mutex<T: ?Sized> {
    data: core::cell::UnsafeCell<T>,
}
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}
pub struct RwLock<T: ?Sized> {
    data: core::cell::UnsafeCell<T>,
}
pub struct Once {
    state: core::sync::atomic::AtomicUsize,
}
pub struct OnceLock<T> {
    value: core::cell::UnsafeCell<Option<T>>,
}
pub struct PoisonError<T> {
    guard: T,
}
pub type LockResult<T> = Result<T, PoisonError<T>>;

impl<T> Mutex<T> {
    pub const fn new(t: T) -> Mutex<T> {
        Mutex {
            data: core::cell::UnsafeCell::new(t),
        }
    }
}
impl<T: ?Sized> Mutex<T> {
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        core::intrinsics::lrr_unsupported("std::sync::Mutex::lock")
    }
}

pub mod mpsc {
    pub struct Sender<T> {
        marker: core::marker::PhantomData<T>,
    }
    pub struct Receiver<T> {
        marker: core::marker::PhantomData<T>,
    }
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        core::intrinsics::lrr_unsupported("std::sync::mpsc::channel")
    }
}
