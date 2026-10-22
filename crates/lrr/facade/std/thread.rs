// `std::thread`. The interpreter runs guest code on one thread, so `spawn`
// runs the closure to completion before it returns and `join` hands back its
// result. A spawned thread that waits on another thread (an empty channel
// whose sender is alive, a lock held elsewhere) can never be woken: that is a
// deadlock the facade reports, where real threads would simply block. A panic
// in a spawned closure ends the run instead of reaching `join`, and
// `current`, `sleep` and other thread identity or time services stay
// unsupported.
pub struct JoinHandle<T> {
    result: Option<T>,
}
pub struct Thread {
    id: u64,
}
pub struct Builder {
    name: Option<crate::string::String>,
}

pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let value = f();
    JoinHandle {
        result: Option::Some(value),
    }
}

pub fn sleep(dur: core::time::Duration) {
    core::intrinsics::lrr_unsupported("std::thread::sleep")
}

pub fn current() -> Thread {
    core::intrinsics::lrr_unsupported("std::thread::current")
}

pub fn yield_now() {}

impl<T> JoinHandle<T> {
    pub fn join(self) -> crate::thread::Result<T> {
        match self.result {
            Option::Some(value) => core::result::Result::<T, crate::boxed::Box<dyn core::any::Any + Send + 'static>>::Ok(value),
            Option::None => core::panicking::panic("joined thread has no result"),
        }
    }
    pub fn is_finished(&self) -> bool {
        true
    }
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            name: Option::None,
        }
    }
    pub fn name(self, name: crate::string::String) -> Builder {
        Builder {
            name: Option::Some(name),
        }
    }
    pub fn stack_size(self, size: usize) -> Builder {
        self
    }
    pub fn spawn<F, T>(self, f: F) -> crate::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        core::result::Result::Ok(spawn(f))
    }
}

pub type Result<T> = core::result::Result<T, crate::boxed::Box<dyn core::any::Any + Send + 'static>>;

// `std::thread::LocalKey`, the key `thread_local!` declares. Guest code runs on
// one thread, so a key has one slot per `static` its expansion declares; this is
// the shape of std's `no_threads` implementation (`sys/thread_local`), whose
// `thread_local_inner!` macro the library definitions supply unchanged. The
// storage holds the value in an `Option`: initialisation is lazy and happens at
// the first access, and the value lives until the program ends.
pub struct LocalKey<T: 'static> {
    // The accessor yields the slot by reference. std's accessor returns a raw
    // pointer to the same slot; the facade keeps the reference the accessor
    // body already holds (`&STORAGE.value` or `STORAGE.get(..)`).
    inner: fn(*mut Option<T>) -> &'static T,
}
pub struct AccessError;

impl core::fmt::Debug for AccessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AccessError")
    }
}
impl core::fmt::Display for AccessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("already destroyed")
    }
}
impl Clone for AccessError {
    fn clone(&self) -> AccessError {
        AccessError
    }
}
impl Copy for AccessError {}
impl PartialEq for AccessError {
    fn eq(&self, other: &AccessError) -> bool {
        true
    }
}
impl Eq for AccessError {}
impl<T: 'static> core::fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("LocalKey { .. }")
    }
}

fn panic_access_error() -> ! {
    core::panicking::panic("cannot access a Thread Local Storage value during or after destruction: AccessError")
}

impl<T: 'static> LocalKey<T> {
    pub const unsafe fn new(inner: fn(*mut Option<T>) -> &'static T) -> LocalKey<T> {
        LocalKey { inner: inner }
    }
    // The slot, initializing it on first access.
    fn slot(&'static self) -> &'static T {
        let init: *mut Option<T> = core::ptr::null_mut();
        (self.inner)(init)
    }
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let value: &'static T = self.slot();
        f(value)
    }
    pub fn try_with<F, R>(&'static self, f: F) -> core::result::Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        let value: &'static T = self.slot();
        core::result::Result::Ok(f(value))
    }
    fn initialize_with<F, R>(&'static self, init: T, f: F) -> R
    where
        F: FnOnce(Option<T>, &T) -> R,
    {
        let mut init = Option::Some(init);
        let provided: *mut Option<T> = &mut init;
        let reference: &'static T = (self.inner)(provided);
        f(init, reference)
    }
}

impl<T: 'static> LocalKey<core::cell::Cell<T>> {
    pub fn set(&'static self, value: T) {
        self.initialize_with(core::cell::Cell::new(value), |value, cell| {
            // The cell was already initialized, so `value` was not used to
            // initialize it: overwrite the current value instead.
            if let Option::Some(value) = value {
                cell.set(value.into_inner());
            }
        });
    }
    pub fn get(&'static self) -> T
    where
        T: Copy,
    {
        let cell: &'static core::cell::Cell<T> = self.slot();
        cell.get()
    }
    pub fn take(&'static self) -> T
    where
        T: Default,
    {
        let cell: &'static core::cell::Cell<T> = self.slot();
        cell.take()
    }
    pub fn replace(&'static self, value: T) -> T {
        let cell: &'static core::cell::Cell<T> = self.slot();
        cell.replace(value)
    }
}

impl<T: 'static> LocalKey<core::cell::RefCell<T>> {
    pub fn with_borrow<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let cell = self.slot();
        f(&*cell.borrow())
    }
    pub fn with_borrow_mut<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let cell = self.slot();
        f(&mut *cell.borrow_mut())
    }
    pub fn set(&'static self, value: T) {
        self.initialize_with(core::cell::RefCell::new(value), |value, cell| {
            if let Option::Some(value) = value {
                *cell.borrow_mut() = value.into_inner();
            }
        });
    }
    pub fn take(&'static self) -> T
    where
        T: Default,
    {
        let cell = self.slot();
        cell.replace(T::default())
    }
    pub fn replace(&'static self, value: T) -> T {
        let cell = self.slot();
        cell.replace(value)
    }
}

// The storage the `no_threads` `thread_local_inner!` expansion declares as a
// `static` inside the key's accessor.
pub mod local_impl {
    pub struct EagerStorage<T> {
        pub value: T,
    }
    unsafe impl<T> Sync for EagerStorage<T> {}

    pub struct LazyStorage<T> {
        value: core::cell::UnsafeCell<Option<T>>,
    }
    impl<T> LazyStorage<T> {
        pub const fn new() -> LazyStorage<T> {
            LazyStorage {
                value: core::cell::UnsafeCell::new(Option::None),
            }
        }
        // `i` is the value `LocalKey::set` offers for the initial one, or null.
        pub fn get(&'static self, i: *mut Option<T>, f: impl FnOnce() -> T) -> &'static T {
            let slot = self.value.lrr_get_mut();
            if slot.is_none() {
                let provided = if i.is_null() {
                    Option::None
                } else {
                    unsafe { (*i).take() }
                };
                let value = match provided {
                    Option::Some(value) => value,
                    Option::None => f(),
                };
                *slot = Option::Some(value);
            }
            match slot {
                Option::Some(value) => unsafe { &*(value as *const T) },
                Option::None => panic_access_error(),
            }
        }
    }
    unsafe impl<T> Sync for LazyStorage<T> {}
}
