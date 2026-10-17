// `alloc::fmt` (`format` is a closed facade function installed here).
pub use core::fmt::{
    write, Alignment, Arguments, Binary, Debug, DebugList, DebugMap, DebugSet, DebugStruct,
    DebugTuple, Display, Error, Formatter, LowerExp, LowerHex, Octal, Pointer, Result, UpperExp,
    UpperHex, Write,
};

// `format!` renders its template eagerly (`Arguments` is the rendered text),
// so the owned string is that text.
pub fn format(args: Arguments<'_>) -> crate::string::String {
    crate::string::String::from(args)
}
