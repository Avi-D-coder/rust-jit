// `std::ffi`.
pub use core::ffi::{c_char, c_double, c_float, c_int, c_long, c_longlong, c_schar, c_short};
pub use core::ffi::{c_uchar, c_uint, c_ulong, c_ulonglong, c_ushort, c_void, CStr};

pub struct OsString {
    inner: crate::vec::Vec<u8>,
}
pub struct OsStr {
    inner: [u8],
}
pub struct CString {
    inner: crate::boxed::Box<[u8]>,
}
