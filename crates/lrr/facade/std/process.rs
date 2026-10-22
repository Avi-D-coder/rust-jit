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

impl ExitCode {
    // The status byte, for the interpreter's entry (`__lrr_report`).
    #[doc(hidden)]
    pub fn __status(&self) -> i32 {
        self.0 as i32
    }
}

pub trait Termination {
    fn report(self) -> ExitCode;
}

impl Termination for () {
    fn report(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}
impl Termination for ExitCode {
    fn report(self) -> ExitCode {
        self
    }
}
// `main() -> Result<T, E>`: `Err(e)` prints `Error: {e:?}` to stderr and fails.
impl<T: Termination, E: crate::fmt::Debug> Termination for Result<T, E> {
    fn report(self) -> ExitCode {
        match self {
            Ok(value) => value.report(),
            Err(error) => {
                // `format_args!` is not lowered in facade source: render the
                // `Debug` text through the formatter sink, as `ToString` does.
                let mut f = core::fmt::Formatter {
                    sink: core::intrinsics::lrr_fmt_sink(),
                    spec: 0,
                    fill: ' ',
                    width: 0,
                    precision: 0,
                };
                core::fmt::Debug::fmt(&error, &mut f);
                let text = core::intrinsics::lrr_fmt_take(f.lrr_sink());
                core::intrinsics::lrr_eprint("Error: ");
                core::intrinsics::lrr_eprint(text);
                core::intrinsics::lrr_eprint("\n");
                ExitCode::FAILURE
            }
        }
    }
}
// The interpreter's program entry: rustc's `lang_start` reports what `main`
// returned and exits with the status. Not part of the real `std`.
#[doc(hidden)]
pub fn __lrr_report<T: Termination>(result: T) -> i32 {
    result.report().__status()
}

pub struct Command {
    program: crate::ffi::OsString,
}
