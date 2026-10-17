// `std::process`.
pub fn exit(code: i32) -> ! {
    core::intrinsics::lrr_unsupported("std::process::exit")
}

pub fn abort() -> ! {
    core::intrinsics::lrr_unsupported("std::process::abort")
}

pub fn id() -> u32 {
    core::intrinsics::lrr_unsupported("std::process::id")
}

pub struct ExitCode(u8);

impl ExitCode {
    pub const SUCCESS: ExitCode = ExitCode(0);
    pub const FAILURE: ExitCode = ExitCode(1);
}

impl From<u8> for ExitCode {
    fn from(code: u8) -> ExitCode {
        ExitCode(code)
    }
}

pub trait Termination {
    fn report(self) -> ExitCode;
}

pub struct Command {
    program: crate::ffi::OsString,
}
