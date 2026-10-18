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
    /// `core::intrinsics::lrr_fmt_take(sink: usize) -> &'static str`: the
    /// text a formatting request's output buffer holds.
    FmtTake,
    /// `core::intrinsics::lrr_str_byte(&str, usize) -> u8`: one byte of a
    /// str reference.
    StrByte,
    /// `core::intrinsics::lrr_vec_len<T>(&Vec<T>) -> usize`.
    VecLen,
    /// `core::intrinsics::lrr_vec_push<T>(&mut Vec<T>, T)`: move a value into
    /// a new last element cell of the vector.
    VecPush,
    /// `core::intrinsics::lrr_vec_insert<T>(&mut Vec<T>, usize, T)`.
    VecInsert,
    /// `core::intrinsics::lrr_vec_remove<T>(&mut Vec<T>, usize) -> T`: move
    /// an element out of the vector, retiring its cell.
    VecRemove,
    /// `core::intrinsics::lrr_vec_as_slice<T>(&Vec<T>) -> &[T]`.
    VecAsSlice,
    /// `core::intrinsics::lrr_vec_as_mut_slice<T>(&mut Vec<T>) -> &mut [T]`.
    VecAsMutSlice,
    /// `core::intrinsics::lrr_vec_as_str(&Vec<u8>) -> &str`.
    VecAsStr,
    /// `core::intrinsics::lrr_slice_swap<T>(&mut [T], usize, usize)`: exchange
    /// the values of two element cells of a slice window.
    SliceSwap,
    /// `core::intrinsics::lrr_vec_append_array<T, const N: usize>(&mut Vec<T>,
    /// [T; N])`: move every element of an array value into the vector.
    VecAppendArray,
    /// `core::intrinsics::lrr_char_from_u32(u32) -> char`: the char of a
    /// value the caller has checked to be a Unicode scalar value.
    CharFromU32,
    /// `core::intrinsics::lrr_str_slice(&str, usize, usize) -> &str`:
    /// `&text[start..end]`, panicking like `str` indexing.
    StrSlice,
    /// `core::intrinsics::lrr_bytes_as_str(&[u8]) -> &str`: the text of a byte
    /// slice the caller has validated as UTF-8.
    BytesAsStr,
    /// `core::intrinsics::lrr_mem_replace<T>(&mut T, T) -> T`.
    MemReplace,
    /// `core::intrinsics::lrr_mem_swap<T>(&mut T, &mut T)`.
    MemSwap,
    /// `core::intrinsics::lrr_cell_replace<T>(&T, T) -> T`: replace the value
    /// at a shared reference's location (interior mutability).
    CellReplace,
    /// One indivisible atomic scalar access/RMW, with explicit operation and
    /// validated Ordering tags; serialized GuestStore execution is SeqCst.
    AtomicOp,
    /// `core::intrinsics::lrr_cell_take<T>(&T) -> T`: move the value out of a
    /// shared reference's location.
    CellTake,
    /// `core::intrinsics::lrr_shared_to_mut<T>(&T) -> &mut T`: `&mut` access to
    /// interior-mutable storage.
    SharedToMut,
    /// `core::intrinsics::lrr_box_leak<T>(Box<T>) -> &'static T`.
    BoxLeak,
    /// `core::intrinsics::lrr_ref_eq<T>(&T, &T) -> bool`: same location.
    RefEq,
    /// `core::intrinsics::lrr_forget<T>(T)`: move a value into a
    /// never-retired location, so no destructor runs (`mem::forget`).
    Forget,
    /// `core::intrinsics::lrr_ptr_null() -> *const ()`: the null pointer
    /// (E-R5-008; coercion sites retype it to the written pointee).
    PtrNull,
    /// `core::intrinsics::lrr_ptr_offset<T>(*const T, isize) -> *const T`:
    /// address arithmetic scaled by the pointee.
    PtrOffset,
    /// `core::intrinsics::lrr_ptr_offset_from<T>(*const T, *const T) -> isize`.
    PtrOffsetFrom,
    /// `core::intrinsics::lrr_ptr_read<T>(*const T) -> T`.
    PtrRead,
    /// `core::intrinsics::lrr_ptr_write<T>(*mut T, T)`.
    PtrWrite,
    /// `core::intrinsics::lrr_ptr_retype<T, U>(*const T) -> *const U`: the same
    /// address (a pointer-to-pointer cast whose target is generic).
    PtrRetype,
    /// `core::intrinsics::lrr_slice_as_ptr<T>(&[T]) -> *const T`: the first
    /// element of a slice, array or vector reference.
    SliceAsPtr,
    /// `core::intrinsics::lrr_slice_as_mut_ptr<T>(&mut [T]) -> *mut T`.
    SliceAsMutPtr,
    /// `core::intrinsics::lrr_ptr_null_mut() -> *mut ()`.
    PtrNullMut,
    /// `core::intrinsics::lrr_ptr_as_const<T>(*mut T) -> *const T`.
    PtrAsConst,
    /// `core::intrinsics::lrr_ptr_as_mut<T>(*const T) -> *mut T`.
    PtrAsMut,
    /// `core::intrinsics::lrr_f64_bits(f64) -> u64`: the IEEE bit pattern.
    F64Bits,
    /// `core::intrinsics::lrr_f64_from_bits(u64) -> f64`.
    F64FromBits,
    /// `core::intrinsics::lrr_f32_bits(f32) -> u32`.
    F32Bits,
    /// `core::intrinsics::lrr_f32_from_bits(u32) -> f32`.
    F32FromBits,
    /// `core::intrinsics::lrr_f64_math(op: u32, a: f64, b: f64, c: f64) -> f64`:
    /// the host libm/IEEE operation `op` names (`f64::sqrt`, `f64::sin`, ...);
    /// operands an operation does not use are ignored.
    F64Math,
    /// `core::intrinsics::lrr_f32_math(op: u32, a: f32, b: f32, c: f32) -> f32`.
    F32Math,
    /// `core::intrinsics::lrr_size_of() -> usize`: the size of the enclosing
    /// function's first type parameter (`mem::size_of::<T>`; E-R5-012).
    SizeOf,
    /// `core::intrinsics::lrr_align_of() -> usize`.
    AlignOf,
    /// `core::intrinsics::lrr_needs_drop() -> bool`.
    NeedsDrop,
    /// `core::intrinsics::lrr_variant_count() -> usize`.
    VariantCount,
    /// `core::intrinsics::lrr_size_of_val<T: ?Sized>(&T) -> usize`.
    SizeOfVal,
    /// `core::intrinsics::lrr_align_of_val<T: ?Sized>(&T) -> usize`.
    AlignOfVal,
    /// `core::intrinsics::lrr_discriminant_value<T>(&T) -> isize`: the
    /// discriminant of the enum value a reference designates (0 otherwise).
    DiscriminantValue,
    /// `core::intrinsics::lrr_zeroed<T>() -> T` for a type with a byte image.
    Zeroed,
    /// `core::intrinsics::lrr_transmute<Src, Dst>(Src) -> Dst` between types of
    /// equal size that both have a byte image.
    Transmute,
}

/// (function, owning library crate, path within that crate).
const ENTRIES: &[(LibraryFunction, &str, &[&str])] = &[
    (LibraryFunction::Panic, "core", &["panicking", "panic"]),
    (
        LibraryFunction::PanicFmt,
        "core",
        &["panicking", "panic_fmt"],
    ),
    (
        LibraryFunction::PanicDisplay,
        "core",
        &["panicking", "panic_display"],
    ),
    (
        LibraryFunction::PanicStr2015,
        "core",
        &["panicking", "panic_str_2015"],
    ),
    (
        LibraryFunction::UnreachableDisplay,
        "core",
        &["panicking", "unreachable_display"],
    ),
    // std::rt reexports core's panic_display/panic_fmt.
    (
        LibraryFunction::PanicDisplay,
        "std",
        &["rt", "panic_display"],
    ),
    (LibraryFunction::PanicFmt, "std", &["rt", "panic_fmt"]),
    (LibraryFunction::BeginPanic, "std", &["rt", "begin_panic"]),
    (LibraryFunction::Print, "std", &["io", "_print"]),
    (LibraryFunction::Eprint, "std", &["io", "_eprint"]),
    (LibraryFunction::MustUse, "alloc", &["__export", "must_use"]),
    (
        LibraryFunction::Unsupported,
        "core",
        &["intrinsics", "lrr_unsupported"],
    ),
    (
        LibraryFunction::AssertFailed,
        "core",
        &["panicking", "assert_failed"],
    ),
    (
        LibraryFunction::SliceLen,
        "core",
        &["intrinsics", "lrr_slice_len"],
    ),
    (
        LibraryFunction::StrLen,
        "core",
        &["intrinsics", "lrr_str_len"],
    ),
    (
        LibraryFunction::StrEq,
        "core",
        &["intrinsics", "lrr_str_eq"],
    ),
    (
        LibraryFunction::FmtWrite,
        "core",
        &["intrinsics", "lrr_fmt_write"],
    ),
    (
        LibraryFunction::FmtValue,
        "core",
        &["intrinsics", "lrr_fmt_value"],
    ),
    (
        LibraryFunction::FmtSink,
        "core",
        &["intrinsics", "lrr_fmt_sink"],
    ),
    (
        LibraryFunction::FmtPadAdapter,
        "core",
        &["intrinsics", "lrr_fmt_pad_adapter"],
    ),
    (
        LibraryFunction::FmtTake,
        "core",
        &["intrinsics", "lrr_fmt_take"],
    ),
    (
        LibraryFunction::StrByte,
        "core",
        &["intrinsics", "lrr_str_byte"],
    ),
    (
        LibraryFunction::VecLen,
        "core",
        &["intrinsics", "lrr_vec_len"],
    ),
    (
        LibraryFunction::VecPush,
        "core",
        &["intrinsics", "lrr_vec_push"],
    ),
    (
        LibraryFunction::VecInsert,
        "core",
        &["intrinsics", "lrr_vec_insert"],
    ),
    (
        LibraryFunction::VecRemove,
        "core",
        &["intrinsics", "lrr_vec_remove"],
    ),
    (
        LibraryFunction::VecAsSlice,
        "core",
        &["intrinsics", "lrr_vec_as_slice"],
    ),
    (
        LibraryFunction::VecAsMutSlice,
        "core",
        &["intrinsics", "lrr_vec_as_mut_slice"],
    ),
    (
        LibraryFunction::VecAsStr,
        "core",
        &["intrinsics", "lrr_vec_as_str"],
    ),
    (
        LibraryFunction::SliceSwap,
        "core",
        &["intrinsics", "lrr_slice_swap"],
    ),
    (
        LibraryFunction::VecAppendArray,
        "core",
        &["intrinsics", "lrr_vec_append_array"],
    ),
    (
        LibraryFunction::CharFromU32,
        "core",
        &["intrinsics", "lrr_char_from_u32"],
    ),
    (
        LibraryFunction::StrSlice,
        "core",
        &["intrinsics", "lrr_str_slice"],
    ),
    (
        LibraryFunction::BytesAsStr,
        "core",
        &["intrinsics", "lrr_bytes_as_str"],
    ),
    (
        LibraryFunction::MemReplace,
        "core",
        &["intrinsics", "lrr_mem_replace"],
    ),
    (
        LibraryFunction::MemSwap,
        "core",
        &["intrinsics", "lrr_mem_swap"],
    ),
    (
        LibraryFunction::CellReplace,
        "core",
        &["intrinsics", "lrr_cell_replace"],
    ),
    (
        LibraryFunction::AtomicOp,
        "core",
        &["intrinsics", "lrr_atomic_op"],
    ),
    (
        LibraryFunction::CellTake,
        "core",
        &["intrinsics", "lrr_cell_take"],
    ),
    (
        LibraryFunction::SharedToMut,
        "core",
        &["intrinsics", "lrr_shared_to_mut"],
    ),
    (
        LibraryFunction::BoxLeak,
        "core",
        &["intrinsics", "lrr_box_leak"],
    ),
    (
        LibraryFunction::RefEq,
        "core",
        &["intrinsics", "lrr_ref_eq"],
    ),
    (
        LibraryFunction::Forget,
        "core",
        &["intrinsics", "lrr_forget"],
    ),
    (
        LibraryFunction::PtrNull,
        "core",
        &["intrinsics", "lrr_ptr_null"],
    ),
    (
        LibraryFunction::PtrOffset,
        "core",
        &["intrinsics", "lrr_ptr_offset"],
    ),
    (
        LibraryFunction::PtrOffsetFrom,
        "core",
        &["intrinsics", "lrr_ptr_offset_from"],
    ),
    (
        LibraryFunction::PtrRead,
        "core",
        &["intrinsics", "lrr_ptr_read"],
    ),
    (
        LibraryFunction::PtrWrite,
        "core",
        &["intrinsics", "lrr_ptr_write"],
    ),
    (
        LibraryFunction::PtrRetype,
        "core",
        &["intrinsics", "lrr_ptr_retype"],
    ),
    (
        LibraryFunction::SliceAsPtr,
        "core",
        &["intrinsics", "lrr_slice_as_ptr"],
    ),
    (
        LibraryFunction::SliceAsMutPtr,
        "core",
        &["intrinsics", "lrr_slice_as_mut_ptr"],
    ),
    (
        LibraryFunction::PtrNullMut,
        "core",
        &["intrinsics", "lrr_ptr_null_mut"],
    ),
    (
        LibraryFunction::PtrAsConst,
        "core",
        &["intrinsics", "lrr_ptr_as_const"],
    ),
    (
        LibraryFunction::PtrAsMut,
        "core",
        &["intrinsics", "lrr_ptr_as_mut"],
    ),
    (
        LibraryFunction::F64Bits,
        "core",
        &["intrinsics", "lrr_f64_bits"],
    ),
    (
        LibraryFunction::F64FromBits,
        "core",
        &["intrinsics", "lrr_f64_from_bits"],
    ),
    (
        LibraryFunction::F32Bits,
        "core",
        &["intrinsics", "lrr_f32_bits"],
    ),
    (
        LibraryFunction::F32FromBits,
        "core",
        &["intrinsics", "lrr_f32_from_bits"],
    ),
    (
        LibraryFunction::F64Math,
        "core",
        &["intrinsics", "lrr_f64_math"],
    ),
    (
        LibraryFunction::F32Math,
        "core",
        &["intrinsics", "lrr_f32_math"],
    ),
    (
        LibraryFunction::SizeOf,
        "core",
        &["intrinsics", "lrr_size_of"],
    ),
    (
        LibraryFunction::AlignOf,
        "core",
        &["intrinsics", "lrr_align_of"],
    ),
    (
        LibraryFunction::NeedsDrop,
        "core",
        &["intrinsics", "lrr_needs_drop"],
    ),
    (
        LibraryFunction::VariantCount,
        "core",
        &["intrinsics", "lrr_variant_count"],
    ),
    (
        LibraryFunction::SizeOfVal,
        "core",
        &["intrinsics", "lrr_size_of_val"],
    ),
    (
        LibraryFunction::AlignOfVal,
        "core",
        &["intrinsics", "lrr_align_of_val"],
    ),
    (
        LibraryFunction::DiscriminantValue,
        "core",
        &["intrinsics", "lrr_discriminant_value"],
    ),
    (
        LibraryFunction::Zeroed,
        "core",
        &["intrinsics", "lrr_zeroed"],
    ),
    (
        LibraryFunction::Transmute,
        "core",
        &["intrinsics", "lrr_transmute"],
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
            Self::FmtTake => 19,
            Self::StrByte => 20,
            Self::VecLen => 21,
            Self::VecPush => 22,
            Self::VecInsert => 23,
            Self::VecRemove => 24,
            Self::VecAsSlice => 25,
            Self::VecAsMutSlice => 26,
            Self::VecAsStr => 27,
            Self::SliceSwap => 28,
            Self::VecAppendArray => 29,
            Self::CharFromU32 => 30,
            Self::StrSlice => 31,
            Self::BytesAsStr => 32,
            Self::MemReplace => 33,
            Self::MemSwap => 34,
            Self::CellReplace => 35,
            Self::AtomicOp => 129,
            Self::CellTake => 36,
            Self::SharedToMut => 37,
            Self::BoxLeak => 38,
            Self::RefEq => 39,
            Self::Forget => 40,
            Self::PtrNull => 70,
            Self::PtrOffset => 71,
            Self::PtrOffsetFrom => 72,
            Self::PtrRead => 73,
            Self::PtrWrite => 74,
            Self::PtrRetype => 75,
            Self::SliceAsPtr => 76,
            Self::SliceAsMutPtr => 77,
            Self::PtrNullMut => 78,
            Self::PtrAsConst => 79,
            Self::PtrAsMut => 80,
            Self::F64Bits => 100,
            Self::F64FromBits => 101,
            Self::F32Bits => 102,
            Self::F32FromBits => 103,
            Self::F64Math => 104,
            Self::F32Math => 105,
            Self::SizeOf => 120,
            Self::AlignOf => 121,
            Self::NeedsDrop => 122,
            Self::VariantCount => 123,
            Self::SizeOfVal => 124,
            Self::AlignOfVal => 125,
            Self::DiscriminantValue => 126,
            Self::Zeroed => 127,
            Self::Transmute => 128,
        }
    }
    const ALL: &'static [Self] = &[
        Self::Panic,
        Self::PanicFmt,
        Self::PanicDisplay,
        Self::PanicStr2015,
        Self::UnreachableDisplay,
        Self::BeginPanic,
        Self::Print,
        Self::Eprint,
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
        Self::FmtTake,
        Self::StrByte,
        Self::VecLen,
        Self::VecPush,
        Self::VecInsert,
        Self::VecRemove,
        Self::VecAsSlice,
        Self::VecAsMutSlice,
        Self::VecAsStr,
        Self::SliceSwap,
        Self::VecAppendArray,
        Self::CharFromU32,
        Self::StrSlice,
        Self::BytesAsStr,
        Self::MemReplace,
        Self::MemSwap,
        Self::CellReplace,
        Self::AtomicOp,
        Self::CellTake,
        Self::SharedToMut,
        Self::BoxLeak,
        Self::RefEq,
        Self::Forget,
        Self::PtrNull,
        Self::PtrOffset,
        Self::PtrOffsetFrom,
        Self::PtrRead,
        Self::PtrWrite,
        Self::PtrRetype,
        Self::SliceAsPtr,
        Self::SliceAsMutPtr,
        Self::PtrNullMut,
        Self::PtrAsConst,
        Self::PtrAsMut,
        Self::F64Bits,
        Self::F64FromBits,
        Self::F32Bits,
        Self::F32FromBits,
        Self::F64Math,
        Self::F32Math,
        Self::SizeOf,
        Self::AlignOf,
        Self::NeedsDrop,
        Self::VariantCount,
        Self::SizeOfVal,
        Self::AlignOfVal,
        Self::DiscriminantValue,
        Self::Zeroed,
        Self::Transmute,
    ];
    /// Number of written arguments of the library signature.
    pub(crate) fn arity(self) -> usize {
        match self {
            Self::AssertFailed => 4,
            Self::StrEq
            | Self::FmtWrite
            | Self::FmtPadAdapter
            | Self::StrByte
            | Self::VecPush
            | Self::VecRemove
            | Self::VecAppendArray
            | Self::MemReplace
            | Self::MemSwap
            | Self::CellReplace
            | Self::PtrOffset
            | Self::PtrOffsetFrom
            | Self::PtrWrite
            | Self::RefEq => 2,
            Self::F64Math | Self::F32Math | Self::AtomicOp => 4,
            Self::VecInsert | Self::SliceSwap | Self::StrSlice => 3,
            Self::FmtValue => 7,
            Self::FmtSink
            | Self::PtrNull
            | Self::PtrNullMut
            | Self::SizeOf
            | Self::AlignOf
            | Self::NeedsDrop
            | Self::VariantCount
            | Self::Zeroed => 0,
            _ => 1,
        }
    }
    /// Whether the function diverges (`-> !`).
    pub(crate) fn diverges(self) -> bool {
        !matches!(
            self,
            Self::Print
                | Self::Eprint
                | Self::MustUse
                | Self::SliceLen
                | Self::StrLen
                | Self::StrEq
                | Self::FmtWrite
                | Self::FmtValue
                | Self::FmtSink
                | Self::FmtPadAdapter
                | Self::FmtTake
                | Self::StrByte
                | Self::VecLen
                | Self::VecPush
                | Self::VecInsert
                | Self::VecRemove
                | Self::VecAsSlice
                | Self::VecAsMutSlice
                | Self::VecAsStr
                | Self::SliceSwap
                | Self::VecAppendArray
                | Self::CharFromU32
                | Self::StrSlice
                | Self::BytesAsStr
                | Self::MemReplace
                | Self::MemSwap
                | Self::CellReplace
                | Self::AtomicOp
                | Self::CellTake
                | Self::SharedToMut
                | Self::BoxLeak
                | Self::RefEq
                | Self::PtrNull
                | Self::PtrOffset
                | Self::PtrOffsetFrom
                | Self::PtrRead
                | Self::PtrWrite
                | Self::PtrRetype
                | Self::SliceAsPtr
                | Self::SliceAsMutPtr
                | Self::PtrNullMut
                | Self::PtrAsConst
                | Self::PtrAsMut
                | Self::Forget
                | Self::F64Bits
                | Self::F64FromBits
                | Self::F32Bits
                | Self::F32FromBits
                | Self::F64Math
                | Self::F32Math
                | Self::SizeOf
                | Self::AlignOf
                | Self::NeedsDrop
                | Self::VariantCount
                | Self::SizeOfVal
                | Self::AlignOfVal
                | Self::DiscriminantValue
                | Self::Zeroed
                | Self::Transmute
        )
    }

    /// Number of leading type parameters of the enclosing (calling) generic
    /// function this intrinsic reads, in declaration order. The wrapper
    /// `pub const fn size_of<T>() -> usize { lrr_size_of() }` is an ordinary
    /// generic function, so the frame already binds `T`; the intrinsic names no
    /// type of its own (E-R5-012).
    pub(crate) fn type_arguments(self) -> usize {
        match self {
            Self::SizeOf | Self::AlignOf | Self::NeedsDrop | Self::VariantCount | Self::Zeroed => 1,
            Self::Transmute => 2,
            _ => 0,
        }
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
pub(crate) fn install(
    graph: &mut NameNamespaces,
    library: &LibraryMacros,
) -> Result<(), Diagnostic> {
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
        .iter()
        .copied()
        .find(|function| function.definition(definition.defining_unit) == *definition)
}
