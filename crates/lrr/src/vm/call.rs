//! Storage ownership for already-evaluated call arguments.
//! Source selection/substitution and dispatcher publication remain separate phases.
use crate::{
    core::diagnostic::Diagnostic,
    runtime::{
        cell::{ArgumentBatchId, CellId, GuestStore},
        shape::TypeId,
    },
};
/// The store record is the canonical once-consumed state; this wrapper is not Clone.
pub struct ArgumentPacket {
    batch: ArgumentBatchId,
}
impl ArgumentPacket {
    pub fn seal(
        store: &mut GuestStore,
        arguments: &[CellId],
        prepared_types: &[TypeId],
    ) -> Result<Self, Diagnostic> {
        store
            .seal_arguments(arguments, prepared_types)
            .map(|batch| Self { batch })
    }
    pub(crate) fn check_types(
        &self,
        store: &GuestStore,
        expected: &[TypeId],
    ) -> Result<(), Diagnostic> {
        store.argument_types(self.batch, expected)
    }
    pub fn deliver(
        &self,
        store: &mut GuestStore,
        destinations: &[CellId],
    ) -> Result<(), Diagnostic> {
        store.deliver_arguments(self.batch, destinations)
    }
    pub fn discard(&self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        store.discard_arguments(self.batch)
    }
}

/// Installed dispatcher facts for a selected call's unevaluated argument slots.
pub(crate) struct CallStartFacts {
    pub(crate) ordinary: crate::frontend::callables::OrdinaryCallWitness,
    pub(crate) site: u32,
    pub(crate) hints: Vec<(
        crate::ir::pc::SlotId,
        Option<TypeId>,
        Option<crate::traits::term::TyTerm>,
    )>,
}
