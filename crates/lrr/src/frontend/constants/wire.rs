//! Public compatibility path for shared immutable constant data; no source authority.
pub(crate) use crate::ir::constant_data::wire::admit_result_limits;
pub use crate::ir::constant_data::wire::{ConstantWireType, decode, encode};
