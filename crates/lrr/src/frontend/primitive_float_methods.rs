//! A reached source receiver joined to an immutable compiler member.
use crate::{
    core::diagnostic::Diagnostic,
    facade::primitive_float_methods::{
        PrimitiveFloatMethod, PrimitiveFloatMethodBody, PrimitiveFloatReceiverKind,
    },
    frontend::{call_contracts::primitive_runtime_id, runtime_macro_catalog::RuntimeMacroCatalog},
    ir::{
        op::{NumericBinary, Op, PrimitiveType},
        pc::PcKey,
    },
    runtime::cell::GuestStore,
    source::changes::PreparedSourceView,
    syntax::runtime_macro_scalars::RuntimePrimitiveMethod,
    traits::term::TyTerm,
    vm::frame::Frame,
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PrimitiveFloatAdmission {
    written: RuntimePrimitiveMethod,
    member: PrimitiveFloatMethod,
}
impl PrimitiveFloatAdmission {
    pub(crate) fn select(
        written: RuntimePrimitiveMethod,
        source: &PreparedSourceView,
        catalog: &RuntimeMacroCatalog,
        frame: &Frame,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        written.require(&source.selection().outline.parse, frame.pc())?;
        if frame.body.body() != &written.span().body_key {
            return Err(fault("primitive method owning frame differs"));
        }
        let receiver = match frame.source_view_ref(written.receiver())? {
            Some(TyTerm::Primitive(primitive @ (PrimitiveType::F32 | PrimitiveType::F64))) => {
                *primitive
            }
            Some(_) => {
                return Err(fault(
                    "primitive method requires an admitted float source receiver",
                ));
            }
            None => written
                .written_receiver()
                .ok_or_else(|| fault("primitive method receiver source type is unresolved"))?,
        };
        if written
            .written_receiver()
            .is_some_and(|primitive| primitive != receiver)
        {
            return Err(fault(
                "primitive method written and admitted receiver types differ",
            ));
        }
        let descriptor = primitive_runtime_id(receiver)?;
        let value = store.preview_scalar(frame.cell(written.receiver())?, Some(descriptor))?;
        if value.type_id != descriptor.0 {
            return Err(fault(
                "primitive method source receiver and actual scalar descriptor differ",
            ));
        }
        let member = catalog
            .primitive_float_method(receiver, "is_nan")
            .ok_or_else(|| fault("primitive method compiler member is unavailable"))?;
        member.require_profile(&source.selection().outline.parse.profile)?;
        catalog.require_primitive_float_method(member)?;
        if member.receiver() != receiver
            || member.receiver_kind() != PrimitiveFloatReceiverKind::Value
            || member.result() != PrimitiveType::Bool
            || member.body() != PrimitiveFloatMethodBody::SelfNotEqual
        {
            return Err(fault("primitive method signature/body differs"));
        }
        Ok(Self {
            written,
            member: member.clone(),
        })
    }
    pub(crate) fn operation(
        &self,
        written: &RuntimePrimitiveMethod,
        source: &PreparedSourceView,
        key: &PcKey,
    ) -> Result<Op, Diagnostic> {
        written.require(&source.selection().outline.parse, key)?;
        if written != &self.written {
            return Err(fault("primitive method admission source phase differs"));
        }
        self.member
            .require_profile(&source.selection().outline.parse.profile)?;
        Ok(Op::NumericBinary {
            operator: NumericBinary::Ne,
            left: written.receiver(),
            right: written.receiver(),
            destination: written.destination(),
        })
    }
    pub(crate) fn allocated_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.written.allocated_bytes()?)?
            .checked_add(self.member.allocated_bytes().ok()?)
    }
}
