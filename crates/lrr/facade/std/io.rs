// `std::io` (`_print`/`_eprint` are closed facade functions installed
// here). Host IO beyond the guest's stdout/stderr capture is outside the
// interpreter's current profile; the names resolve.
pub type Result<T> = core::result::Result<T, Error>;

pub struct Error {
    kind: ErrorKind,
}

pub enum ErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    Other,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        core::intrinsics::lrr_unsupported("std::io::Error::kind")
    }
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
    fn flush(&mut self) -> Result<()>;
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        core::intrinsics::lrr_unsupported("std::io::Write::write_all")
    }
    fn write_fmt(&mut self, fmt: core::fmt::Arguments<'_>) -> Result<()> {
        core::intrinsics::lrr_unsupported("std::io::Write::write_fmt")
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

pub trait BufRead: Read {}

pub struct Stdout {
    _private: (),
}
pub struct Stderr {
    _private: (),
}
pub struct Stdin {
    _private: (),
}

pub fn stdout() -> Stdout {
    Stdout { _private: () }
}
pub fn stderr() -> Stderr {
    Stderr { _private: () }
}
pub fn stdin() -> Stdin {
    Stdin { _private: () }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        core::intrinsics::lrr_unsupported("std::io::Stdout::write")
    }
    fn flush(&mut self) -> Result<()> {
        core::intrinsics::lrr_unsupported("std::io::Stdout::flush")
    }
}
impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        core::intrinsics::lrr_unsupported("std::io::Stderr::write")
    }
    fn flush(&mut self) -> Result<()> {
        core::intrinsics::lrr_unsupported("std::io::Stderr::flush")
    }
}

pub mod prelude {
    pub use super::{BufRead, Read, Write};
}
