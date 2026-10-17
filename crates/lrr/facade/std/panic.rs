// `std::panic`. Unwinding is outside the interpreter's current profile; the
// names resolve. The `panic_2015`/`panic_2021` macros are the real library
// definitions, loaded separately.
use crate::boxed::Box;
pub use core::panic::{AssertUnwindSafe, Location, RefUnwindSafe, UnwindSafe};

pub struct PanicHookInfo<'a> {
    location: &'a Location<'a>,
}

pub fn catch_unwind<F: FnOnce() -> R + UnwindSafe, R>(f: F) -> crate::thread::Result<R> {
    core::intrinsics::lrr_unsupported("std::panic::catch_unwind")
}

pub fn resume_unwind(payload: Box<dyn core::any::Any + Send>) -> ! {
    core::intrinsics::lrr_unsupported("std::panic::resume_unwind")
}

pub fn panic_any<M: 'static + core::any::Any + Send>(msg: M) -> ! {
    core::intrinsics::lrr_unsupported("std::panic::panic_any")
}

pub fn set_hook(hook: Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>) {
    core::intrinsics::lrr_unsupported("std::panic::set_hook")
}

pub fn take_hook() -> Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static> {
    core::intrinsics::lrr_unsupported("std::panic::take_hook")
}
