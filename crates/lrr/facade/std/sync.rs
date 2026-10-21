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

// `std::sync::mpsc`. The interpreter runs guest code on one thread, so a
// channel is one shared FIFO queue and its endpoints are handles on it.
// `send` appends and returns at once. A `sync_channel` bound would hold the
// sender until a receiver (necessarily another thread) made room, which can
// never happen here, so only `try_send` observes the bound. `recv` takes the
// oldest message; on an empty queue it returns `Err(RecvError)` once every
// sender is dropped, and while a sender is alive nothing can ever send, so
// the real call would block forever and this one panics with that diagnosis.
// `recv_timeout` has no time to wait out and reports `Timeout` instead. The
// queue is touched only inside one call that runs no guest code, so it needs no
// borrow flag.
pub mod mpsc {
    use alloc_crate::collections::VecDeque;
    use alloc_crate::sync::Arc;
    use core::cell::{Cell, UnsafeCell};

    struct Shared<T> {
        queue: UnsafeCell<VecDeque<T>>,
        senders: Cell<usize>,
        receiver: Cell<bool>,
        bound: Option<usize>,
    }

    pub struct Sender<T> {
        shared: Arc<Shared<T>>,
    }
    pub struct SyncSender<T> {
        shared: Arc<Shared<T>>,
    }
    pub struct Receiver<T> {
        shared: Arc<Shared<T>>,
    }
    pub struct SendError<T>(pub T);
    pub struct RecvError;
    pub enum TryRecvError {
        Empty,
        Disconnected,
    }
    pub enum RecvTimeoutError {
        Timeout,
        Disconnected,
    }
    pub enum TrySendError<T> {
        Full(T),
        Disconnected(T),
    }
    pub struct Iter<'a, T: 'a> {
        rx: &'a Receiver<T>,
    }
    pub struct TryIter<'a, T: 'a> {
        rx: &'a Receiver<T>,
    }
    pub struct IntoIter<T> {
        rx: Receiver<T>,
    }

    unsafe impl<T: Send> Send for Sender<T> {}
    unsafe impl<T: Send> Sync for Sender<T> {}
    unsafe impl<T: Send> Send for SyncSender<T> {}
    unsafe impl<T: Send> Sync for SyncSender<T> {}
    unsafe impl<T: Send> Send for Receiver<T> {}

    fn shared<T>(bound: Option<usize>) -> Arc<Shared<T>> {
        Arc::new(Shared {
            queue: UnsafeCell::new(VecDeque::new()),
            senders: Cell::new(1),
            receiver: Cell::new(true),
            bound: bound,
        })
    }

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let state = shared::<T>(Option::None);
        (Sender { shared: state.clone() }, Receiver { shared: state })
    }

    pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
        let state = shared::<T>(Option::Some(bound));
        (SyncSender { shared: state.clone() }, Receiver { shared: state })
    }

    impl<T> Sender<T> {
        pub fn send(&self, t: T) -> Result<(), SendError<T>> {
            if !self.shared.receiver.get() {
                return Result::Err(SendError(t));
            }
            self.shared.queue.lrr_get_mut().push_back(t);
            Result::Ok(())
        }
    }
    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Sender<T> {
            self.shared.senders.set(self.shared.senders.get() + 1);
            Sender { shared: self.shared.clone() }
        }
    }
    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.shared.senders.set(self.shared.senders.get() - 1);
        }
    }
    impl<T> core::fmt::Debug for Sender<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("Sender").finish_non_exhaustive()
        }
    }

    impl<T> SyncSender<T> {
        pub fn send(&self, t: T) -> Result<(), SendError<T>> {
            if !self.shared.receiver.get() {
                return Result::Err(SendError(t));
            }
            self.shared.queue.lrr_get_mut().push_back(t);
            Result::Ok(())
        }
        pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
            if !self.shared.receiver.get() {
                return Result::Err(TrySendError::Disconnected(t));
            }
            let full = match self.shared.bound {
                Option::Some(bound) => self.shared.queue.lrr_get().len() >= bound,
                Option::None => false,
            };
            if full {
                return Result::Err(TrySendError::Full(t));
            }
            self.shared.queue.lrr_get_mut().push_back(t);
            Result::Ok(())
        }
    }
    impl<T> Clone for SyncSender<T> {
        fn clone(&self) -> SyncSender<T> {
            self.shared.senders.set(self.shared.senders.get() + 1);
            SyncSender { shared: self.shared.clone() }
        }
    }
    impl<T> Drop for SyncSender<T> {
        fn drop(&mut self) {
            self.shared.senders.set(self.shared.senders.get() - 1);
        }
    }
    impl<T> core::fmt::Debug for SyncSender<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("SyncSender").finish_non_exhaustive()
        }
    }

    impl<T> Receiver<T> {
        pub fn try_recv(&self) -> Result<T, TryRecvError> {
            let next = self.shared.queue.lrr_get_mut().pop_front();
            match next {
                Option::Some(t) => Result::Ok(t),
                Option::None => {
                    if self.shared.senders.get() == 0 {
                        Result::Err(TryRecvError::Disconnected)
                    } else {
                        Result::Err(TryRecvError::Empty)
                    }
                }
            }
        }
        pub fn recv(&self) -> Result<T, RecvError> {
            let next = self.shared.queue.lrr_get_mut().pop_front();
            match next {
                Option::Some(t) => Result::Ok(t),
                Option::None => {
                    if self.shared.senders.get() == 0 {
                        Result::Err(RecvError)
                    } else {
                        core::panicking::panic(
                            "deadlock: recv on an empty channel whose senders are alive; no other thread can send",
                        )
                    }
                }
            }
        }
        pub fn recv_timeout(&self, timeout: core::time::Duration) -> Result<T, RecvTimeoutError> {
            let next = self.shared.queue.lrr_get_mut().pop_front();
            match next {
                Option::Some(t) => Result::Ok(t),
                Option::None => {
                    if self.shared.senders.get() == 0 {
                        Result::Err(RecvTimeoutError::Disconnected)
                    } else {
                        Result::Err(RecvTimeoutError::Timeout)
                    }
                }
            }
        }
        pub fn iter(&self) -> Iter<'_, T> {
            Iter { rx: self }
        }
        pub fn try_iter(&self) -> TryIter<'_, T> {
            TryIter { rx: self }
        }
    }
    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.shared.receiver.set(false);
        }
    }
    impl<T> core::fmt::Debug for Receiver<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("Receiver").finish_non_exhaustive()
        }
    }

    impl<'a, T> Iterator for Iter<'a, T> {
        type Item = T;
        fn next(&mut self) -> Option<T> {
            self.rx.recv().ok()
        }
    }
    impl<'a, T> Iterator for TryIter<'a, T> {
        type Item = T;
        fn next(&mut self) -> Option<T> {
            self.rx.try_recv().ok()
        }
    }
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<T> {
            self.rx.recv().ok()
        }
    }
    impl<'a, T> IntoIterator for &'a Receiver<T> {
        type Item = T;
        type IntoIter = Iter<'a, T>;
        fn into_iter(self) -> Iter<'a, T> {
            self.iter()
        }
    }
    impl<T> IntoIterator for Receiver<T> {
        type Item = T;
        type IntoIter = IntoIter<T>;
        fn into_iter(self) -> IntoIter<T> {
            IntoIter { rx: self }
        }
    }

    impl<T> core::fmt::Debug for SendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("SendError").finish_non_exhaustive()
        }
    }
    impl<T> core::fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("sending on a closed channel")
        }
    }
    impl<T> Clone for SendError<T> where T: Clone {
        fn clone(&self) -> SendError<T> {
            SendError(self.0.clone())
        }
    }
    impl<T> Copy for SendError<T> where T: Copy {}
    impl<T> PartialEq for SendError<T> where T: PartialEq {
        fn eq(&self, other: &SendError<T>) -> bool {
            self.0 == other.0
        }
    }
    impl<T> Eq for SendError<T> where T: Eq {}

    impl core::fmt::Debug for RecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("RecvError")
        }
    }
    impl core::fmt::Display for RecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("receiving on a closed channel")
        }
    }
    impl Clone for RecvError {
        fn clone(&self) -> RecvError {
            RecvError
        }
    }
    impl Copy for RecvError {}
    impl PartialEq for RecvError {
        fn eq(&self, other: &RecvError) -> bool {
            true
        }
    }
    impl Eq for RecvError {}

    impl core::fmt::Debug for TryRecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TryRecvError::Empty => f.write_str("Empty"),
                TryRecvError::Disconnected => f.write_str("Disconnected"),
            }
        }
    }
    impl core::fmt::Display for TryRecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TryRecvError::Empty => f.write_str("receiving on an empty channel"),
                TryRecvError::Disconnected => f.write_str("receiving on a closed channel"),
            }
        }
    }
    impl Clone for TryRecvError {
        fn clone(&self) -> TryRecvError {
            match self {
                TryRecvError::Empty => TryRecvError::Empty,
                TryRecvError::Disconnected => TryRecvError::Disconnected,
            }
        }
    }
    impl Copy for TryRecvError {}
    impl PartialEq for TryRecvError {
        fn eq(&self, other: &TryRecvError) -> bool {
            match (self, other) {
                (TryRecvError::Empty, TryRecvError::Empty) => true,
                (TryRecvError::Disconnected, TryRecvError::Disconnected) => true,
                _ => false,
            }
        }
    }
    impl Eq for TryRecvError {}

    impl core::fmt::Debug for RecvTimeoutError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                RecvTimeoutError::Timeout => f.write_str("Timeout"),
                RecvTimeoutError::Disconnected => f.write_str("Disconnected"),
            }
        }
    }
    impl core::fmt::Display for RecvTimeoutError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                RecvTimeoutError::Timeout => f.write_str("timed out waiting on channel"),
                RecvTimeoutError::Disconnected => f.write_str("channel is empty and sending half is closed"),
            }
        }
    }
    impl Clone for RecvTimeoutError {
        fn clone(&self) -> RecvTimeoutError {
            match self {
                RecvTimeoutError::Timeout => RecvTimeoutError::Timeout,
                RecvTimeoutError::Disconnected => RecvTimeoutError::Disconnected,
            }
        }
    }
    impl Copy for RecvTimeoutError {}
    impl PartialEq for RecvTimeoutError {
        fn eq(&self, other: &RecvTimeoutError) -> bool {
            match (self, other) {
                (RecvTimeoutError::Timeout, RecvTimeoutError::Timeout) => true,
                (RecvTimeoutError::Disconnected, RecvTimeoutError::Disconnected) => true,
                _ => false,
            }
        }
    }
    impl Eq for RecvTimeoutError {}

    impl<T> core::fmt::Debug for TrySendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TrySendError::Full(_) => f.write_str("Full(..)"),
                TrySendError::Disconnected(_) => f.write_str("Disconnected(..)"),
            }
        }
    }
    impl<T> core::fmt::Display for TrySendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TrySendError::Full(_) => f.write_str("sending on a full channel"),
                TrySendError::Disconnected(_) => f.write_str("sending on a closed channel"),
            }
        }
    }
}
