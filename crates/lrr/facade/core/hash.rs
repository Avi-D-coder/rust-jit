// `core::hash`: the real `Hash`/`Hasher` protocol. Integers feed their
// little-endian bytes (this host's native order) to the hasher, `str` its
// bytes then `0xff`, sequences a `usize` length prefix, tuples and derived
// types their fields in order, enums their discriminant as an `isize`;
// `DefaultHasher` (std) is SipHash-1-3 with zero keys, so equal values
// hash equally to real std's output.
use crate::cmp::Ordering;
use crate::marker::Sized;
use crate::option::Option;
use crate::result::Result;

pub trait Hash {
    fn hash<H: Hasher>(&self, state: &mut H);
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H)
    where
        Self: Sized,
    {
        let mut i = 0;
        while i < data.len() {
            data[i].hash(state);
            i += 1;
        }
    }
}

pub trait Hasher {
    fn finish(&self) -> u64;
    fn write(&mut self, bytes: &[u8]);
    fn write_u8(&mut self, i: u8) {
        self.write(&[i]);
    }
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }
    fn write_usize(&mut self, i: usize) {
        self.write(&i.to_le_bytes());
    }
    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }
    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16);
    }
    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32);
    }
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }
    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize);
    }
    fn write_length_prefix(&mut self, len: usize) {
        self.write_usize(len);
    }
    fn write_str(&mut self, s: &str) {
        // Byte by byte: a streaming hasher sees the same byte sequence.
        let mut bytes = s.bytes();
        while let Option::Some(byte) = bytes.next() {
            self.write_u8(byte);
        }
        self.write_u8(0xff);
    }
}

impl<'a, H: Hasher + ?Sized> Hasher for &'a mut H {
    fn finish(&self) -> u64 {
        (**self).finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes)
    }
    fn write_u8(&mut self, i: u8) {
        (**self).write_u8(i)
    }
    fn write_u16(&mut self, i: u16) {
        (**self).write_u16(i)
    }
    fn write_u32(&mut self, i: u32) {
        (**self).write_u32(i)
    }
    fn write_u64(&mut self, i: u64) {
        (**self).write_u64(i)
    }
    fn write_usize(&mut self, i: usize) {
        (**self).write_usize(i)
    }
    fn write_i8(&mut self, i: i8) {
        (**self).write_i8(i)
    }
    fn write_i16(&mut self, i: i16) {
        (**self).write_i16(i)
    }
    fn write_i32(&mut self, i: i32) {
        (**self).write_i32(i)
    }
    fn write_i64(&mut self, i: i64) {
        (**self).write_i64(i)
    }
    fn write_isize(&mut self, i: isize) {
        (**self).write_isize(i)
    }
    fn write_length_prefix(&mut self, len: usize) {
        (**self).write_length_prefix(len)
    }
    fn write_str(&mut self, s: &str) {
        (**self).write_str(s)
    }
}

pub trait BuildHasher {
    type Hasher: Hasher;
    fn build_hasher(&self) -> Self::Hasher;
    fn hash_one<T: Hash>(&self, x: T) -> u64
    where
        Self: Sized,
        Self::Hasher: Hasher,
    {
        let mut hasher = self.build_hasher();
        x.hash(&mut hasher);
        hasher.finish()
    }
}

pub struct BuildHasherDefault<H> {
    marker: crate::marker::PhantomData<H>,
}
impl<H: crate::default::Default + Hasher> BuildHasher for BuildHasherDefault<H> {
    type Hasher = H;
    fn build_hasher(&self) -> H {
        H::default()
    }
}
impl<H> crate::default::Default for BuildHasherDefault<H> {
    fn default() -> BuildHasherDefault<H> {
        BuildHasherDefault {
            marker: crate::marker::PhantomData,
        }
    }
}

impl Hash for u8 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(*self)
    }
}
impl Hash for u16 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u16(*self)
    }
}
impl Hash for u32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(*self)
    }
}
impl Hash for u64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(*self)
    }
}
impl Hash for usize {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(*self)
    }
}
impl Hash for i8 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i8(*self)
    }
}
impl Hash for i16 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i16(*self)
    }
}
impl Hash for i32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(*self)
    }
}
impl Hash for i64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i64(*self)
    }
}
impl Hash for isize {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_isize(*self)
    }
}
impl Hash for bool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(*self as u8)
    }
}
impl Hash for char {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(*self as u32)
    }
}
impl Hash for () {
    fn hash<H: Hasher>(&self, state: &mut H) {}
}
impl Hash for str {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_str(self)
    }
}
impl<'a, T: ?Sized + Hash> Hash for &'a T {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&**self, state)
    }
}
impl<'a, T: ?Sized + Hash> Hash for &'a mut T {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&**self, state)
    }
}
impl<T: Hash> Hash for [T] {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_length_prefix(self.len());
        let mut i = 0;
        while i < self.len() {
            self[i].hash(state);
            i += 1;
        }
    }
}
impl<T: Hash, const N: usize> Hash for [T; N] {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self[..], state)
    }
}
impl<A: Hash> Hash for (A,) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl<A: Hash, B: Hash> Hash for (A, B) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}
impl<A: Hash, B: Hash, C: Hash> Hash for (A, B, C) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
        self.2.hash(state);
    }
}
impl<A: Hash, B: Hash, C: Hash, D: Hash> Hash for (A, B, C, D) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
        self.2.hash(state);
        self.3.hash(state);
    }
}
impl<A: Hash, B: Hash, C: Hash, D: Hash, E: Hash> Hash for (A, B, C, D, E) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
        self.2.hash(state);
        self.3.hash(state);
        self.4.hash(state);
    }
}
impl<T: Hash> Hash for Option<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Option::None => state.write_isize(0),
            Option::Some(value) => {
                state.write_isize(1);
                value.hash(state);
            }
        }
    }
}
impl<T: Hash, E: Hash> Hash for Result<T, E> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Result::Ok(value) => {
                state.write_isize(0);
                value.hash(state);
            }
            Result::Err(error) => {
                state.write_isize(1);
                error.hash(state);
            }
        }
    }
}
impl Hash for Ordering {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // `#[repr(i8)]`: the derived hash writes the `i8` discriminant.
        let discriminant: i8 = match self {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        state.write_i8(discriminant)
    }
}
