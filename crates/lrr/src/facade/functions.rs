//! Section 17 closed library facade: table-driven function entries.
//!
//! The real core/alloc/std macro definitions expand to ordinary calls such as
//! `$crate::panicking::panic("explicit panic")`. Those library functions are
//! not interpreted from library source; each entry here names its exact
//! library path and is implemented by a runtime intrinsic. Names resolve
//! through the ordinary value namespace (aliases and shadowing preserved);
//! selection happens only after that lookup, by definition identity.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    frontend::{
        library_macros::LibraryMacros,
        names::{NameDefinition, NameNamespaces},
    },
    source::manifest::Namespace,
    syntax::visibility::VisibilityScope,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum LibraryFunction {
    /// `core::panicking::panic(&'static str) -> !`
    Panic,
    /// `core::panicking::panic_fmt(fmt::Arguments) -> !`
    PanicFmt,
    /// `core::panicking::panic_display<T: Display>(&T) -> !`
    PanicDisplay,
    /// `core::panicking::panic_str_2015(&str) -> !`
    PanicStr2015,
    /// `core::panicking::unreachable_display<T: Display>(&T) -> !`
    UnreachableDisplay,
    /// `std::rt::begin_panic<M: Any + Send>(M) -> !`
    BeginPanic,
    /// `std::io::_print(fmt::Arguments)`
    Print,
    /// `std::io::_eprint(fmt::Arguments)`
    Eprint,
    /// `alloc::fmt::format(fmt::Arguments) -> String`
    Format,
    /// `alloc::__export::must_use<T>(T) -> T`
    MustUse,
    /// `core::intrinsics::lrr_unsupported(&'static str) -> !`: the body of an
    /// LRR facade source item the interpreter cannot execute yet. A reached
    /// call faults naming the item; it never substitutes other behavior.
    Unsupported,
    /// `core::panicking::assert_failed<T: Debug + ?Sized, U: Debug + ?Sized>(
    /// AssertKind, &T, &U, Option<fmt::Arguments<'_>>) -> !`
    AssertFailed,
    /// `core::intrinsics::lrr_slice_len<T>(&[T]) -> usize`: the element count
    /// of a slice reference's window (the body of `<[T]>::len`).
    SliceLen,
    /// `core::intrinsics::lrr_str_len(&str) -> usize`: the byte length of a
    /// str reference (the body of `str::len`).
    StrLen,
    /// `core::intrinsics::lrr_str_eq(&str, &str) -> bool`: byte equality of
    /// two str references (the body of `<str as PartialEq>::eq`).
    StrEq,
    /// `core::intrinsics::lrr_fmt_write(sink: usize, s: &str)`: append to a
    /// formatting request's output buffer (`Formatter::write_str`).
    FmtWrite,
    /// `core::intrinsics::lrr_fmt_value<T: ?Sized>(value: &T, format: u32,
    /// sink: usize, spec: u64, fill: char, width: usize, precision: usize)`:
    /// render a closed primitive/str value under a formatter's options into
    /// its buffer (the primitive `fmt` impls and `Formatter::pad`).
    FmtValue,
    /// `core::intrinsics::lrr_fmt_sink() -> usize`: a fresh output buffer.
    FmtSink,
    /// `core::intrinsics::lrr_fmt_pad_adapter(parent: usize, nested: usize)`:
    /// append the nested buffer through core's `PadAdapter` (`{:#?}`).
    FmtPadAdapter,
}

/// (function, owning library crate, path within that crate).
const ENTRIES: &[(LibraryFunction, &str, &[&str])] = &[
    (LibraryFunction::Panic, "core", &["panicking", "panic"]),
    (LibraryFunction::PanicFmt, "core", &["panicking", "panic_fmt"]),
    (LibraryFunction::PanicDisplay, "core", &["panicking", "panic_display"]),
    (LibraryFunction::PanicStr2015, "core", &["panicking", "panic_str_2015"]),
    (
        LibraryFunction::UnreachableDisplay,
        "core",
        &["panicking", "unreachable_display"],
    ),
    // std::rt reexports core's panic_display/panic_fmt.
    (LibraryFunction::PanicDisplay, "std", &["rt", "panic_display"]),
    (LibraryFunction::PanicFmt, "std", &["rt", "panic_fmt"]),
    (LibraryFunction::BeginPanic, "std", &["rt", "begin_panic"]),
    (LibraryFunction::Print, "std", &["io", "_print"]),
    (LibraryFunction::Eprint, "std", &["io", "_eprint"]),
    (LibraryFunction::Format, "alloc", &["fmt", "format"]),
    (LibraryFunction::MustUse, "alloc", &["__export", "must_use"]),
    (
        LibraryFunction::Unsupported,
        "core",
        &["intrinsics", "lrr_unsupported"],
    ),
    (LibraryFunction::AssertFailed, "core", &["panicking", "assert_failed"]),
    (
        LibraryFunction::SliceLen,
        "core",
        &["intrinsics", "lrr_slice_len"],
    ),
    (LibraryFunction::StrLen, "core", &["intrinsics", "lrr_str_len"]),
    (LibraryFunction::StrEq, "core", &["intrinsics", "lrr_str_eq"]),
    (LibraryFunction::FmtWrite, "core", &["intrinsics", "lrr_fmt_write"]),
    (LibraryFunction::FmtValue, "core", &["intrinsics", "lrr_fmt_value"]),
    (LibraryFunction::FmtSink, "core", &["intrinsics", "lrr_fmt_sink"]),
    (
        LibraryFunction::FmtPadAdapter,
        "core",
        &["intrinsics", "lrr_fmt_pad_adapter"],
    ),
];

const LIBRARY_EDITION: u16 = 2024;

impl LibraryFunction {
    fn tag(self) -> u32 {
        match self {
            Self::Panic => 0,
            Self::PanicFmt => 1,
            Self::PanicDisplay => 2,
            Self::PanicStr2015 => 3,
            Self::UnreachableDisplay => 4,
            Self::BeginPanic => 5,
            Self::Print => 6,
            Self::Eprint => 7,
            Self::Format => 8,
            Self::MustUse => 9,
            Self::Unsupported => 10,
            Self::AssertFailed => 11,
            Self::SliceLen => 12,
            Self::StrLen => 13,
            Self::StrEq => 14,
            Self::FmtWrite => 15,
            Self::FmtValue => 16,
            Self::FmtSink => 17,
            Self::FmtPadAdapter => 18,
        }
    }
    const ALL: [Self; 19] = [
        Self::Panic,
        Self::PanicFmt,
        Self::PanicDisplay,
        Self::PanicStr2015,
        Self::UnreachableDisplay,
        Self::BeginPanic,
        Self::Print,
        Self::Eprint,
        Self::Format,
        Self::MustUse,
        Self::Unsupported,
        Self::AssertFailed,
        Self::SliceLen,
        Self::StrLen,
        Self::StrEq,
        Self::FmtWrite,
        Self::FmtValue,
        Self::FmtSink,
        Self::FmtPadAdapter,
    ];
    /// Number of written arguments of the library signature.
    pub(crate) fn arity(self) -> usize {
        match self {
            Self::AssertFailed => 4,
            Self::StrEq | Self::FmtWrite | Self::FmtPadAdapter => 2,
            Self::FmtValue => 7,
            Self::FmtSink => 0,
            _ => 1,
        }
    }
    /// Whether the function diverges (`-> !`).
    pub(crate) fn diverges(self) -> bool {
        !matches!(
            self,
            Self::Print
                | Self::Eprint
                | Self::Format
                | Self::MustUse
                | Self::SliceLen
                | Self::StrLen
                | Self::StrEq
                | Self::FmtWrite
                | Self::FmtValue
                | Self::FmtSink
                | Self::FmtPadAdapter
        )
    }
    /// Identity of this function as defined in `unit`. A reexport (e.g.
    /// `std::rt::panic_display`) has its own identity selecting the same body.
    fn identity(self, unit: Digest) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/library-facade-function");
        e.digest(unit);
        e.u32(self.tag());
        e.finish()
    }
    fn definition(self, unit: Digest) -> NameDefinition {
        NameDefinition {
            namespace: Namespace::Value,
            identity: self.identity(unit),
            defining_unit: unit,
            definition_edition: LIBRARY_EDITION,
        }
    }
}

/// Add the table's value definitions to the ordinary namespace graph for the
/// library crates of this compile input.
pub(crate) fn install(graph: &mut NameNamespaces, library: &LibraryMacros) -> Result<(), Diagnostic> {
    for (function, krate, path) in ENTRIES {
        let Some(unit) = library.unit(krate) else {
            continue;
        };
        let (_, modules) = path.split_last().expect("non-empty facade path");
        for depth in 0..=modules.len() {
            graph.declare_module(
                unit,
                &modules[..depth],
                LIBRARY_EDITION,
                VisibilityScope::Public,
            )?;
        }
        graph.define(
            NameNamespaces::path(unit, path, LIBRARY_EDITION)?,
            function.definition(unit),
            VisibilityScope::Public,
        )?;
    }
    Ok(())
}

/// Select a facade function after ordinary value lookup.
pub(crate) fn select(definition: &NameDefinition) -> Option<LibraryFunction> {
    if definition.namespace != Namespace::Value || definition.definition_edition != LIBRARY_EDITION
    {
        return None;
    }
    LibraryFunction::ALL
        .into_iter()
        .find(|function| function.definition(definition.defining_unit) == *definition)
}
