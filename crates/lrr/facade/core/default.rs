// `core::default`.
use crate::marker::Sized;

pub trait Default: Sized {
    fn default() -> Self;
}
