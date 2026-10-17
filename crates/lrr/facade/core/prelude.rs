// `core::prelude`: the edition preludes, with rustc's exact Type/Value
// contents (macros join through the library macro namespace). Each edition
// module lists its names explicitly (the same set rustc's `v1::*` globs
// select), which keeps name discovery free of glob fixed points.
pub mod v1 {
    pub use crate::clone::Clone;
    pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use crate::convert::{AsMut, AsRef, From, Into};
    pub use crate::default::Default;
    pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use crate::mem::drop;
    pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
    pub use crate::option::Option::{self, None, Some};
    pub use crate::result::Result::{self, Err, Ok};
}
pub mod rust_2015 {
    pub use crate::clone::Clone;
    pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use crate::convert::{AsMut, AsRef, From, Into};
    pub use crate::default::Default;
    pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use crate::mem::drop;
    pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
    pub use crate::option::Option::{self, None, Some};
    pub use crate::result::Result::{self, Err, Ok};
}
pub mod rust_2018 {
    pub use crate::clone::Clone;
    pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use crate::convert::{AsMut, AsRef, From, Into};
    pub use crate::default::Default;
    pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use crate::mem::drop;
    pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
    pub use crate::option::Option::{self, None, Some};
    pub use crate::result::Result::{self, Err, Ok};
}
pub mod rust_2021 {
    pub use crate::clone::Clone;
    pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use crate::convert::{AsMut, AsRef, From, Into};
    pub use crate::convert::{TryFrom, TryInto};
    pub use crate::default::Default;
    pub use crate::iter::FromIterator;
    pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use crate::mem::drop;
    pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
    pub use crate::option::Option::{self, None, Some};
    pub use crate::result::Result::{self, Err, Ok};
}
pub mod rust_2024 {
    pub use crate::clone::Clone;
    pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use crate::convert::{AsMut, AsRef, From, Into};
    pub use crate::convert::{TryFrom, TryInto};
    pub use crate::default::Default;
    pub use crate::future::{Future, IntoFuture};
    pub use crate::iter::FromIterator;
    pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use crate::mem::drop;
    pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
    pub use crate::option::Option::{self, None, Some};
    pub use crate::result::Result::{self, Err, Ok};
}
