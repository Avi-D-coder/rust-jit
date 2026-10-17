// `std::prelude`: the edition preludes with rustc's exact Type/Value
// contents (macros join through the library macro namespace). Each edition
// module lists its names explicitly (the same set rustc's globs select),
// which keeps name discovery free of glob fixed points.
pub mod v1 {
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};

    pub use crate::borrow::ToOwned;
    pub use crate::boxed::Box;
    pub use crate::string::{String, ToString};
    pub use crate::vec::Vec;
}
pub mod rust_2015 {
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};

    pub use crate::borrow::ToOwned;
    pub use crate::boxed::Box;
    pub use crate::string::{String, ToString};
    pub use crate::vec::Vec;
}
pub mod rust_2018 {
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};

    pub use crate::borrow::ToOwned;
    pub use crate::boxed::Box;
    pub use crate::string::{String, ToString};
    pub use crate::vec::Vec;
}
pub mod rust_2021 {
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};

    pub use crate::borrow::ToOwned;
    pub use crate::boxed::Box;
    pub use crate::string::{String, ToString};
    pub use crate::vec::Vec;
    pub use core::convert::{TryFrom, TryInto};
    pub use core::iter::FromIterator;
}
pub mod rust_2024 {
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::mem::{align_of, align_of_val, size_of, size_of_val};
    pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};

    pub use crate::borrow::ToOwned;
    pub use crate::boxed::Box;
    pub use crate::string::{String, ToString};
    pub use crate::vec::Vec;
    pub use core::convert::{TryFrom, TryInto};
    pub use core::iter::FromIterator;
    pub use core::future::{Future, IntoFuture};
}
