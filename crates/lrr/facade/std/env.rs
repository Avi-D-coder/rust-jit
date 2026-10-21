// `std::env`. The interpreter runs one guest program and has no argument
// vector of its own, so `args()` yields exactly the program name (`argv[0]`),
// as a process started without arguments does. The rest of the process
// environment is outside the interpreter's current profile; the names resolve.
pub struct Args {
    inner: crate::vec::IntoIter<crate::string::String>,
}
pub struct Vars {
    position: usize,
}
pub enum VarError {
    NotPresent,
    NotUnicode(crate::ffi::OsString),
}

pub fn args() -> Args {
    let mut argv: crate::vec::Vec<crate::string::String> = crate::vec::Vec::new();
    argv.push(crate::string::String::from("main"));
    Args { inner: argv.into_iter() }
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
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<crate::string::String> {
        self.inner.next_back()
    }
}
impl ExactSizeIterator for Args {}
