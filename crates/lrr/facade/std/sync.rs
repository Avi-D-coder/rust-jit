// `std::sync`.
pub use alloc_crate::sync::{Arc, Weak};
pub use core::sync::atomic;

// `Mutex` and `RwLock`. The interpreter runs guest code on one thread, so a
// lock is a flag/counter the guards maintain: acquiring a lock that this
// thread already holds incompatibly can never succeed (real std deadlocks or
// panics there), so `lock`/`read`/`write` panic with that diagnosis and
// `try_*` return `WouldBlock`. Poisoning needs unwinding through a held guard,
// which the interpreter reports as a terminal fault, so no lock is ever
// poisoned; `is_poisoned` is always `false`.
struct LrrLocked;
impl core::fmt::Debug for LrrLocked {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("<locked>")
    }
}

pub struct Mutex<T: ?Sized> {
    poisoned: core::cell::Cell<bool>,
    locked: core::cell::Cell<bool>,
    data: core::cell::UnsafeCell<T>,
}
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}
pub struct RwLock<T: ?Sized> {
    poisoned: core::cell::Cell<bool>,
    // -1 while write-locked, otherwise the number of live read guards.
    state: core::cell::Cell<isize>,
    data: core::cell::UnsafeCell<T>,
}
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
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
pub enum TryLockError<T> {
    Poisoned(PoisonError<T>),
    WouldBlock,
}
pub type LockResult<T> = Result<T, PoisonError<T>>;
pub type TryLockResult<T> = Result<T, TryLockError<T>>;

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}
unsafe impl<'a, T: ?Sized + Sync> Sync for MutexGuard<'a, T> {}
unsafe impl<'a, T: ?Sized + Sync> Sync for RwLockReadGuard<'a, T> {}
unsafe impl<'a, T: ?Sized + Sync> Sync for RwLockWriteGuard<'a, T> {}

impl<T> PoisonError<T> {
    pub fn new(guard: T) -> PoisonError<T> {
        PoisonError { guard: guard }
    }
    pub fn into_inner(self) -> T {
        self.guard
    }
    pub fn get_ref(&self) -> &T {
        &self.guard
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}
impl<T> core::fmt::Debug for PoisonError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PoisonError { .. }")
    }
}
impl<T> core::fmt::Display for PoisonError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("poisoned lock: another task failed inside")
    }
}
impl<T> core::fmt::Debug for TryLockError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TryLockError::Poisoned(_) => f.write_str("Poisoned(..)"),
            TryLockError::WouldBlock => f.write_str("WouldBlock"),
        }
    }
}
impl<T> core::fmt::Display for TryLockError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TryLockError::Poisoned(_) => f.write_str("poisoned lock: another task failed inside"),
            TryLockError::WouldBlock => f.write_str("try_lock failed because the operation would block"),
        }
    }
}

impl<T> Mutex<T> {
    pub const fn new(t: T) -> Mutex<T> {
        Mutex {
            poisoned: core::cell::Cell::new(false),
            locked: core::cell::Cell::new(false),
            data: core::cell::UnsafeCell::new(t),
        }
    }
    pub fn into_inner(self) -> LockResult<T> {
        Result::Ok(self.data.into_inner())
    }
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        if self.locked.get() {
            core::panicking::panic("deadlock: this thread already holds the Mutex");
        }
        self.locked.set(true);
        Result::Ok(MutexGuard { lock: self })
    }
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        if self.locked.get() {
            return Result::Err(TryLockError::WouldBlock);
        }
        self.locked.set(true);
        Result::Ok(MutexGuard { lock: self })
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.get()
    }
    pub fn clear_poison(&self) {
        self.poisoned.set(false);
    }
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Result::Ok(self.data.lrr_get_mut())
    }
}
impl<T: Default> Default for Mutex<T> {
    fn default() -> Mutex<T> {
        Mutex::new(T::default())
    }
}
impl<T> From<T> for Mutex<T> {
    fn from(t: T) -> Mutex<T> {
        Mutex::new(t)
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("Mutex");
        if self.locked.get() {
            d.field("data", &LrrLocked);
        } else {
            d.field("data", self.data.lrr_get());
        }
        d.field("poisoned", &self.poisoned.get());
        d.finish_non_exhaustive()
    }
}
impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lock.data.lrr_get()
    }
}
impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.lock.data.lrr_get_mut()
    }
}
impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.locked.set(false);
    }
}
impl<'a, T: core::fmt::Debug> core::fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.lock.data.lrr_get(), f)
    }
}
impl<'a, T: core::fmt::Display> core::fmt::Display for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.lock.data.lrr_get(), f)
    }
}

impl<T> RwLock<T> {
    pub const fn new(t: T) -> RwLock<T> {
        RwLock {
            poisoned: core::cell::Cell::new(false),
            state: core::cell::Cell::new(0),
            data: core::cell::UnsafeCell::new(t),
        }
    }
    pub fn into_inner(self) -> LockResult<T> {
        Result::Ok(self.data.into_inner())
    }
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        let state: isize = self.state.get();
        if state < 0 {
            core::panicking::panic("rwlock read lock would result in deadlock");
        }
        self.state.set(state + 1);
        Result::Ok(RwLockReadGuard { lock: self })
    }
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        let state: isize = self.state.get();
        if state < 0 {
            return Result::Err(TryLockError::WouldBlock);
        }
        self.state.set(state + 1);
        Result::Ok(RwLockReadGuard { lock: self })
    }
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        if self.state.get() != 0 {
            core::panicking::panic("rwlock write lock would result in deadlock");
        }
        self.state.set(-1);
        Result::Ok(RwLockWriteGuard { lock: self })
    }
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        if self.state.get() != 0 {
            return Result::Err(TryLockError::WouldBlock);
        }
        self.state.set(-1);
        Result::Ok(RwLockWriteGuard { lock: self })
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.get()
    }
    pub fn clear_poison(&self) {
        self.poisoned.set(false);
    }
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Result::Ok(self.data.lrr_get_mut())
    }
}
impl<T: Default> Default for RwLock<T> {
    fn default() -> RwLock<T> {
        RwLock::new(T::default())
    }
}
impl<T> From<T> for RwLock<T> {
    fn from(t: T) -> RwLock<T> {
        RwLock::new(t)
    }
}
impl<T: core::fmt::Debug> core::fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("RwLock");
        if self.state.get() < 0 {
            d.field("data", &LrrLocked);
        } else {
            d.field("data", self.data.lrr_get());
        }
        d.field("poisoned", &self.poisoned.get());
        d.finish_non_exhaustive()
    }
}
impl<'a, T> core::ops::Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lock.data.lrr_get()
    }
}
impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.state.set(self.lock.state.get() - 1);
    }
}
impl<'a, T> core::ops::Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lock.data.lrr_get()
    }
}
impl<'a, T> core::ops::DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.lock.data.lrr_get_mut()
    }
}
impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.state.set(0);
    }
}
impl<'a, T: core::fmt::Debug> core::fmt::Debug for RwLockReadGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.lock.data.lrr_get(), f)
    }
}
impl<'a, T: core::fmt::Debug> core::fmt::Debug for RwLockWriteGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.lock.data.lrr_get(), f)
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
