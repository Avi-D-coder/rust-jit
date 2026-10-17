// `std::thread`. Threads are outside the interpreter's current profile;
// the names resolve.
pub struct JoinHandle<T> {
    marker: core::marker::PhantomData<T>,
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
    core::intrinsics::lrr_unsupported("std::thread::spawn")
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
        core::intrinsics::lrr_unsupported("std::thread::JoinHandle::join")
    }
}

pub type Result<T> = core::result::Result<T, crate::boxed::Box<dyn core::any::Any + Send + 'static>>;
