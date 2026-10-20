//! Frame-owned written templates use the same descriptor and shape operations
//! as closed annotations, with the actual declaration's fixed bindings.
use super::{Frame, invalid};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    ir::written_type::WrittenType,
    runtime::{cell::GuestStore, shape::TypeId},
    traits::term::ParamKey,
    vm::context::GenericEnv,
};
impl Frame {
    pub(crate) fn type_template_context(&self, definition: Digest) -> Result<&GenericEnv, Diagnostic> {
        if self.body.body().definition != definition {
            return Err(invalid("written Type template belongs to another frame"));
        }
        let context = self.context.as_deref()
            .ok_or_else(|| invalid("written Type template has no reached generic context"))?;
        if context.definition().definition != definition {
            return Err(invalid("written Type template generic owner differs"));
        }
        Ok(context)
    }
    pub(crate) fn find_written_hint(&self, written: &WrittenType, store: &GuestStore) -> Result<Option<TypeId>, Diagnostic> {
        match written {
            WrittenType::Frame { definition, template } => {
                let context = self.type_template_context(*definition)?;
                if !written.exact() && !written.is_pointer() { return Ok(None); }
                template.find_with(store, &|ordinal| context.runtime_binding(ParamKey { definition: *definition, ordinal }))
            }
            _ => written.find(store),
        }
    }
    pub(crate) fn materialize_written_hint(&self, written: &WrittenType, store: &mut GuestStore) -> Result<Option<TypeId>, Diagnostic> {
        match written {
            WrittenType::Frame { definition, template } => {
                let context = self.type_template_context(*definition)?;
                if !written.exact() && !written.is_pointer() { return Ok(None); }
                template.materialize_with(store, &|ordinal| context.runtime_binding(ParamKey { definition: *definition, ordinal }))
            }
            _ => written.materialize(store),
        }
    }
    pub(crate) fn admits_written_hint(&self, written: &WrittenType, ty: TypeId, store: &GuestStore) -> Result<bool, Diagnostic> {
        match written {
            WrittenType::Frame { definition, template } => {
                let context = self.type_template_context(*definition)?;
                template.admits_with(ty, store, &|ordinal| context.runtime_binding(ParamKey { definition: *definition, ordinal }))
            }
            _ => written.admits(ty, store),
        }
    }
    /// Snapshot the actual frame's fixed Type arguments when installing a
    /// partial hint, preserving declaration ordinals including lifetime gaps.
    pub(super) fn written_frame_arguments(&self, definition: Digest) -> Result<std::sync::Arc<[Option<TypeId>]>, Diagnostic> {
        let context = self.type_template_context(definition)?;
        let mut fixed = Vec::new();
        for parameter in &context.definition().generics {
            if parameter.kind != crate::syntax::signatures::GenericKind::Type { continue; }
            let ordinal = usize::try_from(parameter.ordinal).map_err(|_| invalid("Type parameter ordinal overflow"))?;
            let length = ordinal.checked_add(1).ok_or_else(|| invalid("Type parameter ordinal overflow"))?;
            if fixed.len() < length { fixed.resize(length, None); }
            fixed[ordinal] = context.runtime_binding(ParamKey { definition, ordinal: parameter.ordinal });
        }
        Ok(fixed.into())
    }
}
