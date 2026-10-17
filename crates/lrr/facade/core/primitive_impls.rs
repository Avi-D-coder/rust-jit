// Trait impls of the primitive types (real core declares them with
// macros in clone.rs/cmp.rs/default.rs). Operators on primitive operands
// are built into the interpreter, so these bodies are ordinary Rust over
// those operators.
use crate::clone::Clone;
use crate::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use crate::default::Default;
use crate::marker::Copy;
use crate::option::Option;

impl Clone for i8 {
    fn clone(&self) -> i8 {
        *self
    }
}
impl Copy for i8 {}
impl PartialEq for i8 {
    fn eq(&self, other: &i8) -> bool {
        *self == *other
    }
}
impl PartialOrd for i8 {
    fn partial_cmp(&self, other: &i8) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for i8 {}
impl Ord for i8 {
    fn cmp(&self, other: &i8) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for i8 {
    fn default() -> i8 {
        0
    }
}
impl Clone for i16 {
    fn clone(&self) -> i16 {
        *self
    }
}
impl Copy for i16 {}
impl PartialEq for i16 {
    fn eq(&self, other: &i16) -> bool {
        *self == *other
    }
}
impl PartialOrd for i16 {
    fn partial_cmp(&self, other: &i16) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for i16 {}
impl Ord for i16 {
    fn cmp(&self, other: &i16) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for i16 {
    fn default() -> i16 {
        0
    }
}
impl Clone for i32 {
    fn clone(&self) -> i32 {
        *self
    }
}
impl Copy for i32 {}
impl PartialEq for i32 {
    fn eq(&self, other: &i32) -> bool {
        *self == *other
    }
}
impl PartialOrd for i32 {
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for i32 {}
impl Ord for i32 {
    fn cmp(&self, other: &i32) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for i32 {
    fn default() -> i32 {
        0
    }
}
impl Clone for i64 {
    fn clone(&self) -> i64 {
        *self
    }
}
impl Copy for i64 {}
impl PartialEq for i64 {
    fn eq(&self, other: &i64) -> bool {
        *self == *other
    }
}
impl PartialOrd for i64 {
    fn partial_cmp(&self, other: &i64) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for i64 {}
impl Ord for i64 {
    fn cmp(&self, other: &i64) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for i64 {
    fn default() -> i64 {
        0
    }
}
impl Clone for isize {
    fn clone(&self) -> isize {
        *self
    }
}
impl Copy for isize {}
impl PartialEq for isize {
    fn eq(&self, other: &isize) -> bool {
        *self == *other
    }
}
impl PartialOrd for isize {
    fn partial_cmp(&self, other: &isize) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for isize {}
impl Ord for isize {
    fn cmp(&self, other: &isize) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for isize {
    fn default() -> isize {
        0
    }
}
impl Clone for u8 {
    fn clone(&self) -> u8 {
        *self
    }
}
impl Copy for u8 {}
impl PartialEq for u8 {
    fn eq(&self, other: &u8) -> bool {
        *self == *other
    }
}
impl PartialOrd for u8 {
    fn partial_cmp(&self, other: &u8) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for u8 {}
impl Ord for u8 {
    fn cmp(&self, other: &u8) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for u8 {
    fn default() -> u8 {
        0
    }
}
impl Clone for u16 {
    fn clone(&self) -> u16 {
        *self
    }
}
impl Copy for u16 {}
impl PartialEq for u16 {
    fn eq(&self, other: &u16) -> bool {
        *self == *other
    }
}
impl PartialOrd for u16 {
    fn partial_cmp(&self, other: &u16) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for u16 {}
impl Ord for u16 {
    fn cmp(&self, other: &u16) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for u16 {
    fn default() -> u16 {
        0
    }
}
impl Clone for u32 {
    fn clone(&self) -> u32 {
        *self
    }
}
impl Copy for u32 {}
impl PartialEq for u32 {
    fn eq(&self, other: &u32) -> bool {
        *self == *other
    }
}
impl PartialOrd for u32 {
    fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for u32 {}
impl Ord for u32 {
    fn cmp(&self, other: &u32) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for u32 {
    fn default() -> u32 {
        0
    }
}
impl Clone for u64 {
    fn clone(&self) -> u64 {
        *self
    }
}
impl Copy for u64 {}
impl PartialEq for u64 {
    fn eq(&self, other: &u64) -> bool {
        *self == *other
    }
}
impl PartialOrd for u64 {
    fn partial_cmp(&self, other: &u64) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for u64 {}
impl Ord for u64 {
    fn cmp(&self, other: &u64) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for u64 {
    fn default() -> u64 {
        0
    }
}
impl Clone for usize {
    fn clone(&self) -> usize {
        *self
    }
}
impl Copy for usize {}
impl PartialEq for usize {
    fn eq(&self, other: &usize) -> bool {
        *self == *other
    }
}
impl PartialOrd for usize {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for usize {}
impl Ord for usize {
    fn cmp(&self, other: &usize) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for usize {
    fn default() -> usize {
        0
    }
}
impl Clone for f32 {
    fn clone(&self) -> f32 {
        *self
    }
}
impl Copy for f32 {}
impl PartialEq for f32 {
    fn eq(&self, other: &f32) -> bool {
        *self == *other
    }
}
impl PartialOrd for f32 {
    fn partial_cmp(&self, other: &f32) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Default for f32 {
    fn default() -> f32 {
        0.0
    }
}
impl Clone for f64 {
    fn clone(&self) -> f64 {
        *self
    }
}
impl Copy for f64 {}
impl PartialEq for f64 {
    fn eq(&self, other: &f64) -> bool {
        *self == *other
    }
}
impl PartialOrd for f64 {
    fn partial_cmp(&self, other: &f64) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Default for f64 {
    fn default() -> f64 {
        0.0
    }
}
impl Clone for bool {
    fn clone(&self) -> bool {
        *self
    }
}
impl Copy for bool {}
impl PartialEq for bool {
    fn eq(&self, other: &bool) -> bool {
        *self == *other
    }
}
impl PartialOrd for bool {
    fn partial_cmp(&self, other: &bool) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for bool {}
impl Ord for bool {
    fn cmp(&self, other: &bool) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for bool {
    fn default() -> bool {
        false
    }
}
impl Clone for char {
    fn clone(&self) -> char {
        *self
    }
}
impl Copy for char {}
impl PartialEq for char {
    fn eq(&self, other: &char) -> bool {
        *self == *other
    }
}
impl PartialOrd for char {
    fn partial_cmp(&self, other: &char) -> Option<Ordering> {
        if *self < *other {
            Option::Some(Ordering::Less)
        } else if *self > *other {
            Option::Some(Ordering::Greater)
        } else if *self == *other {
            Option::Some(Ordering::Equal)
        } else {
            Option::None
        }
    }
}
impl Eq for char {}
impl Ord for char {
    fn cmp(&self, other: &char) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl Default for char {
    fn default() -> char {
        '\x00'
    }
}
impl Clone for () {
    fn clone(&self) -> () {
        ()
    }
}
impl Copy for () {}
impl Default for () {
    fn default() -> () {
        ()
    }
}
impl<T: ?Sized> Clone for &T {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for &T {}
