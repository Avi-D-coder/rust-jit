//! The type arguments a generic frame's function was called with, read by
//! type-argument intrinsics (E-R5-012).
//!
//! A wrapper such as `pub const fn size_of<T>() -> usize { lrr_size_of() }`
//! is an ordinary generic function whose frame binds `T` to a runtime
//! descriptor in its `GenericEnv`; the closed intrinsic it calls names no type
//! of its own and asks the calling frame for the wrapper's leading type
//! parameters by position.
use super::*;
use crate::{syntax::signatures::GenericKind, traits::term::ParamKey};

impl Frame {
    /// The runtime descriptors bound to the first `count` type parameters
    /// (lifetimes and const parameters are skipped) of this frame's function,
    /// in declaration order.
    pub(crate) fn enclosing_type_arguments(
        &self,
        count: usize,
    ) -> Result<Vec<crate::runtime::shape::TypeId>, Diagnostic> {
        let missing = || {
            invalid("a type-argument intrinsic was called outside a generic function that binds its type")
        };
        if count == 0 {
            return Ok(Vec::new());
        }
        let context = self.context.as_ref().ok_or_else(missing)?;
        let signature = context.definition();
        let mut arguments = Vec::with_capacity(count);
        for parameter in signature
            .generics
            .iter()
            .filter(|parameter| parameter.kind == GenericKind::Type)
            .take(count)
        {
            let key = ParamKey {
                definition: signature.definition,
                ordinal: parameter.ordinal,
            };
            let ty = context.runtime_binding(key).ok_or_else(|| {
                invalid("a type-argument intrinsic's type parameter has no runtime binding")
            })?;
            arguments.push(ty);
        }
        if arguments.len() != count {
            return Err(missing());
        }
        Ok(arguments)
    }
}
