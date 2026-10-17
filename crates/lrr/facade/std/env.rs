// `std::env`. The process environment is outside the interpreter's
// current profile; the names resolve.
pub struct Args {
    position: usize,
}
pub struct Vars {
    position: usize,
}
pub enum VarError {
    NotPresent,
    NotUnicode(crate::ffi::OsString),
}

pub fn args() -> Args {
    core::intrinsics::lrr_unsupported("std::env::args")
}
pub fn var(key: &str) -> Result<crate::string::String, VarError> {
    core::intrinsics::lrr_unsupported("std::env::var")
}
pub fn var_os(key: &str) -> Option<crate::ffi::OsString> {
    core::intrinsics::lrr_unsupported("std::env::var_os")
}
pub fn vars() -> Vars {
    core::intrinsics::lrr_unsupported("std::env::vars")
}

impl Iterator for Args {
    type Item = crate::string::String;
    fn next(&mut self) -> Option<crate::string::String> {
        core::intrinsics::lrr_unsupported("std::env::Args::next")
    }
}
