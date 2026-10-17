// `core::panic`. Its macros (`panic_2015`, `panic_2021`, `unreachable_2015`,
// ...) are the real library definitions, loaded separately.
use crate::ops::{Deref, DerefMut};

pub struct Location<'a> {
    file: &'a str,
    line: u32,
    col: u32,
}

impl<'a> Location<'a> {
    pub fn caller() -> &'static Location<'static> {
        core::intrinsics::lrr_unsupported("core::panic::Location::caller")
    }
    pub fn file(&self) -> &str {
        self.file
    }
    pub fn line(&self) -> u32 {
        self.line
    }
    pub fn column(&self) -> u32 {
        self.col
    }
}

pub struct PanicInfo<'a> {
    location: &'a Location<'a>,
}

pub struct PanicMessage<'a> {
    message: &'a crate::fmt::Arguments<'a>,
}

pub trait UnwindSafe {}
pub trait RefUnwindSafe {}

pub struct AssertUnwindSafe<T>(pub T);

impl<T> Deref for AssertUnwindSafe<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> DerefMut for AssertUnwindSafe<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
