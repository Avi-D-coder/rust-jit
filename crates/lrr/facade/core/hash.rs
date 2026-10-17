// `core::hash`.
use crate::marker::Sized;

pub trait Hash {
    fn hash<H: Hasher>(&self, state: &mut H);
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H)
    where
        Self: Sized,
    {
        core::intrinsics::lrr_unsupported("core::hash::Hash::hash_slice")
    }
}

pub trait Hasher {
    fn finish(&self) -> u64;
    fn write(&mut self, bytes: &[u8]);
    fn write_u8(&mut self, i: u8) {
        core::intrinsics::lrr_unsupported("core::hash::Hasher::write_u8")
    }
    fn write_u32(&mut self, i: u32) {
        core::intrinsics::lrr_unsupported("core::hash::Hasher::write_u32")
    }
    fn write_u64(&mut self, i: u64) {
        core::intrinsics::lrr_unsupported("core::hash::Hasher::write_u64")
    }
    fn write_usize(&mut self, i: usize) {
        core::intrinsics::lrr_unsupported("core::hash::Hasher::write_usize")
    }
    fn write_isize(&mut self, i: isize) {
        core::intrinsics::lrr_unsupported("core::hash::Hasher::write_isize")
    }
}

pub trait BuildHasher {
    type Hasher: Hasher;
    fn build_hasher(&self) -> Self::Hasher;
}

pub struct BuildHasherDefault<H> {
    marker: crate::marker::PhantomData<H>,
}
