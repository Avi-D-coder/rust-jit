//! T054 physical publication into the existing ordinary temporary machine.
//! Only an opaque admitted use may reach production; its producer is prerequisite.
#[path = "frame_constant_chunk_publication.rs"]
mod chunks;
use super::*;
use crate::{
    engine::constant_array_publication::AdmittedConstantUse,
    ir::constant_data::{ConstValue, wire::ConstantWireType},
    runtime::cell::constant_instantiation::PreparedConstantInstantiation,
    syntax::body_index::{ExpressionKind, NodeRole},
};
pub(crate) use chunks::{ConstantPublicationActivation, ConstantPublicationChunk};
/// No public constructor, clone, raw result ticket or stable encoding.
pub(crate) struct PreparedConstantPublication<'a, 'world> {
    admitted: &'a AdmittedConstantUse<'world>,
    frame: FrameId,
    pc: PcKey,
    destination: SlotId,
    prior: Option<CellId>,
    scope: ScopeId,
    epoch: u32,
    ty: TypeId,
    source: crate::traits::term::TyTerm,
    construction: PreparedConstantInstantiation,
}
impl Frame {
    pub(crate) fn constant_frame_anchor(&self) -> FrameId {
        self.anchor
    }
    pub(crate) fn preflight_constant_source(
        &self,
        proof: &AdmittedConstantUse<'_>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.require_store(store)?;
        proof.revalidate(store)?;
        let node = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("constant publication node absent"))?;
        if self.status != FrameStatus::Running
            || proof.frame() != self.anchor
            || proof.pc() != &self.pc
            || proof.body_index() != self.body.fingerprint()
            || self.pc.phase != crate::ir::pc::Phase::Entry.code()?
            || node.role != NodeRole::Expression(ExpressionKind::Path)
            || node.value != Some(proof.destination())
            || self.body.span(node.id)? != *proof.span()
            || self.copy_activation.is_some()
            || self.cleanup_activation.is_some()
        {
            return Err(invalid(
                "constant publication differs from exact running source path/frame/phase",
            ));
        }
        // Agreement only: the opaque source proof is mandatory and is never
        // synthesized by looking at a descriptor.
        use crate::{
            runtime::shape::{PrimitiveIdentity, TypeKey},
            traits::term::{ConstTerm, TyTerm},
        };
        let source_agrees = match (
            proof.source(),
            store.type_descriptor(proof.ty())?.key.as_ref(),
        ) {
            (TyTerm::Primitive(p), TypeKey::Primitive(actual)) => {
                *actual == PrimitiveIdentity::from_recipe(*p)?
            }
            (TyTerm::Array(source, ConstTerm::Value(length)), TypeKey::Array { element, len }) => {
                matches!(source.as_ref(),TyTerm::Primitive(p) if PrimitiveIdentity::from_recipe(*p).is_ok_and(|id|id.id()==*element))
                    && *length == u64::from(*len)
            }
            _ => false,
        };
        if !source_agrees {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "constant publication requires exact source-selected scalar/primitive-array layout"
                    .into(),
            ));
        }
        self.preflight_source_view(proof.destination(), Some(proof.source()))?;
        if self
            .source_view_ref(proof.destination())?
            .is_some_and(|s| s != proof.source())
            || self
                .hint(proof.destination())?
                .is_some_and(|t| t != proof.ty())
        {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "constant source result differs from retained destination context".into(),
            ));
        }
        self.preflight_result_type(proof.destination(), Some(proof.ty()))?;
        self.preflight_temporary_slot(proof.destination())
    }
    pub(crate) fn preflight_constant_use(
        &self,
        proof: &AdmittedConstantUse<'_>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constant_source(proof, store)?;
        self.preflight_write_type(proof.destination(), proof.ty(), store)
    }
    pub(crate) fn prepare_constant_publication<'a, 'world>(
        &self,
        proof: &'a AdmittedConstantUse<'world>,
        value: &ConstValue,
        wire: &ConstantWireType,
        store: &GuestStore,
    ) -> Result<PreparedConstantPublication<'a, 'world>, Diagnostic> {
        self.preflight_constant_use(proof, store)?;
        if !proof.admits_content(value, wire)? {
            return Err(invalid(
                "constant publication immutable content differs from admitted memo",
            ));
        }
        let construction = store.prepare_constant_instantiation(value, wire, proof.ty())?;
        let retained = Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|n| n.checked_add(self.constructor_mapping_bytes))
            .and_then(|n| n.checked_add(self.projection_bytes))
            .and_then(|n| n.checked_add(self.source_field_bytes()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<PreparedConstantPublication<'_, '_>>()))
            .and_then(|n| n.checked_add(construction.metadata_bytes()))
            .and_then(|n| n.checked_add(proof.metadata_bytes()?))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(crate::runtime::arena::resource)?;
        let _ = retained;
        store
            .preflight_owning_read_result(self.slot(proof.destination())?.cell.is_none(), false)?;
        Ok(PreparedConstantPublication {
            admitted: proof,
            frame: self.anchor,
            pc: self.pc.clone(),
            destination: proof.destination(),
            prior: self.slot(proof.destination())?.cell,
            scope: self.slot(proof.destination())?.scope,
            epoch: store.budget().epoch(),
            ty: proof.ty(),
            source: proof.source().clone(),
            construction,
        })
    }
    /// Ordinary opcode entry is already charged. No source query or authority
    /// recovery occurs here. Destination storage is secured before constructing.
    #[cfg(test)]
    pub(crate) fn publish_constant_to_temporary(
        &mut self,
        prepared: PreparedConstantPublication<'_, '_>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let proof = prepared.admitted;
        self.preflight_constant_use(proof, store)?;
        if matches!(
            store.type_descriptor(proof.ty())?.key.as_ref(),
            crate::runtime::shape::TypeKey::Array { .. }
        ) {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "constant array publication requires the resumable activation path".into(),
            ));
        }
        if prepared.frame != self.anchor
            || prepared.pc != self.pc
            || prepared.destination != proof.destination()
            || prepared.epoch != store.budget().epoch()
            || prepared.prior != self.slot(prepared.destination)?.cell
            || prepared.scope != self.slot(prepared.destination)?.scope
            || prepared.ty != proof.ty()
            || &prepared.source != proof.source()
        {
            return Err(invalid(
                "constant publication preparation is stale or foreign",
            ));
        }
        self.fuel.poll()?;
        store.reserve_owning_read_result(prepared.prior.is_none(), false)?;
        // A new Uninit FrameSlot is a normal cleanup obligation, not a value
        // publication. Its handle cannot be spent by subsequent construction.
        let cell = self.prepare_write(prepared.destination, store)?;
        let mut pending =
            store.instantiate_pending_constant(prepared.construction, &mut self.fuel)?;
        if let Err(error) = self.fuel.poll() {
            store.abort_pending_constant(pending)?;
            return Err(error);
        }
        match store.commit_pending_constant(&mut pending, cell) {
            Ok(()) => self.set_source_view(prepared.destination, Some(prepared.source)),
            Err(error) => {
                store.abort_pending_constant(pending)?;
                Err(error)
            }
        }
    }
}
#[cfg(test)]
#[path = "constant_array_transfer_tests.rs"]
mod tests;

#[cfg(test)]
impl Frame {
    pub(crate) fn constant_scope_active_for_test(&self, scope: ScopeId) -> bool {
        self.active_scopes.contains(&scope)
    }
    pub(crate) fn constant_enter_for_test(&mut self, scope: ScopeId) -> Result<(), Diagnostic> {
        self.enter(scope)
    }
}

#[cfg(test)]
impl Frame {
    pub(crate) fn constant_slot_scope_for_test(&self, slot: SlotId) -> Result<ScopeId, Diagnostic> {
        Ok(self.slot(slot)?.scope)
    }
    pub(crate) fn constant_active_scope_for_test(&self) -> Option<ScopeId> {
        self.active_scopes.last().copied()
    }
    pub(crate) fn constant_binding_for_test(&self, node: NodeId) -> Result<CellId, Diagnostic> {
        self.binding(node)
    }
}
