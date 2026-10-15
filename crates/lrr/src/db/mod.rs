//! Pure, single-threaded dependency-tracked query storage.
pub mod dependency;
pub mod key;
pub mod memo;
pub mod poll;
pub mod provider;
pub mod trace;
pub mod value;

pub mod retained_syntax;
pub mod seals;
