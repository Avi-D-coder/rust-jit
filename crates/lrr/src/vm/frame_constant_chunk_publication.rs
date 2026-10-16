//! Owned exact op data; current source authority is reborrowed on every advance.
use super::*;
use crate::runtime::cell::constant_instantiation::{ConstantChunkActivation, ConstantChunkPoll};
/// No DB/proof borrow, raw constructor, Clone, serialized authority or caller flag.
pub(crate) struct ConstantPublicationActivation {
    session: crate::runtime::cell::StoreSession,
    frame: FrameId,
    pc: PcKey,
    destination: SlotId,
    cell: CellId,
    cell_epoch: u32,
    scope: ScopeId,
    epoch: u32,
    span: crate::syntax::spans::BodySpan,
    body_index: crate::core::digest::Digest,
    source: crate::traits::term::TyTerm,
    ty: TypeId,
    content: crate::core::digest::Digest,
    source_use: crate::core::digest::Digest,
    charged_on_entry: bool,
    physical: ConstantChunkActivation,
    done: bool,
}
pub(crate) enum ConstantPublicationChunk {
    Dispatch {
        steps: u16,
        bytes: u32,
        cleaning: bool,
    },
    Published,
    Fault {
        diagnostic: Diagnostic,
        cleanup_pending: bool,
    },
}
impl ConstantPublicationActivation {
    pub(crate) fn charged_on_entry(&self) -> bool {
        self.charged_on_entry
    }
    pub(crate) fn fields_charged(&self) -> u64 {
        self.physical.fields_charged()
    }
    pub(crate) fn cleaning(&self) -> bool {
        self.physical.cleaning()
    }
}
impl Frame {
    /// Current source/metadata/table preflight precedes the single entry callback.
    /// Return the owned activation to the uniform frame OpActivation attachment.
    pub(crate) fn begin_constant_publication_chunks(
        &mut self,
        prepared: PreparedConstantPublication<'_, '_>,
        store: &mut GuestStore,
        charge: impl FnOnce(&mut Frame) -> Result<(), Diagnostic>,
    ) -> Result<ConstantPublicationActivation, Diagnostic> {
        let proof = prepared.admitted;
        self.preflight_constant_use(proof, store)?;
        if prepared.frame != self.anchor
            || prepared.pc != self.pc
            || prepared.destination != proof.destination()
            || prepared.epoch != store.budget().epoch()
            || prepared.prior != self.slot(prepared.destination)?.cell
            || prepared.scope != self.slot(prepared.destination)?.scope
            || prepared.ty != proof.ty()
            || &prepared.source != proof.source()
        {
            return Err(invalid("constant chunk preparation is stale or foreign"));
        }
        let physical = store.prepare_constant_chunks(
            prepared.construction,
            usize::from(prepared.prior.is_none()),
        )?;
        let metadata = Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|n| n.checked_add(self.constructor_mapping_bytes))
            .and_then(|n| n.checked_add(self.projection_bytes))
            .and_then(|bytes| bytes.checked_add(self.source_extra_bytes))
            .and_then(|n| n.checked_add(self.auxiliary_metadata_bytes()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<ConstantPublicationActivation>()))
            .and_then(|n| n.checked_add(physical.metadata_bytes()))
            .and_then(|n| n.checked_add(proof.metadata_bytes()?))
            .and_then(|n| n.checked_add(prepared.source.heap_bytes()?))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(crate::runtime::arena::resource)?;
        let _ = metadata;
        self.fuel.poll()?;
        let fuel_before = self.fuel.snapshot();
        charge(self)?;
        let fuel_after = self.fuel.snapshot();
        if fuel_before.used.checked_add(1) != Some(fuel_after.used)
            || fuel_before.limit != fuel_after.limit
        {
            return Err(invalid(
                "constant chunk entry must charge exactly one original opcode",
            ));
        }
        self.preflight_constant_use(proof, store)?;
        // No active guard/value exists before this callback succeeds. This cell
        // is an ordinary Uninit temporary, separately owned by its lexical scope.
        let cell = self.prepare_write(prepared.destination, store)?;
        let cell_epoch = store.location_epoch(cell)?;
        Ok(ConstantPublicationActivation {
            session: store.pattern_binding_session(),
            frame: self.anchor,
            pc: self.pc.clone(),
            destination: prepared.destination,
            cell,
            cell_epoch,
            scope: prepared.scope,
            epoch: store.budget().epoch(),
            span: proof.span().clone(),
            body_index: self.body.fingerprint(),
            source: prepared.source,
            ty: prepared.ty,
            content: proof.content_identity(),
            source_use: proof.source_use_identity(),
            charged_on_entry: true,
            physical,
            done: false,
        })
    }
    fn validate_constant_chunk_use(
        &self,
        a: &ConstantPublicationActivation,
        proof: &AdmittedConstantUse<'_>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        // This is the actual full private source/query/frame proof, not an old
        // digest or assumed completeness bit. Driver remints/reborrows it first.
        self.preflight_constant_use(proof, store)?;
        if a.session != store.pattern_binding_session()
            || a.done
            || !a.charged_on_entry
            || a.frame != self.anchor
            || a.pc != self.pc
            || a.destination != proof.destination()
            || a.epoch != store.budget().epoch()
            || a.span != *proof.span()
            || a.body_index != self.body.fingerprint()
            || a.ty != proof.ty()
            || &a.source != proof.source()
            || a.content != proof.content_identity()
            || a.source_use != proof.source_use_identity()
            || self.slot(a.destination)?.cell != Some(a.cell)
            || self.slot(a.destination)?.scope != a.scope
            || store.location_epoch(a.cell)? != a.cell_epoch
            || !matches!(
                store.state(a.cell)?,
                crate::runtime::cell::CellState::Uninit
            )
        {
            return Err(invalid(
                "constant chunk source/site/content/storage binding differs",
            ));
        }
        Ok(())
    }
    /// Authority-free abort only for this opaque activation's unpublished copies.
    /// It starts rollback, never reports that cleanup is already complete.
    pub(crate) fn abort_constant_publication_chunks(
        &mut self,
        a: &mut ConstantPublicationActivation,
        store: &mut GuestStore,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        self.require_store(store)?;
        if a.session != store.pattern_binding_session()
            || a.done
            || a.frame != self.anchor
            || a.epoch != store.budget().epoch()
        {
            return Err(invalid("constant chunk abort is stale or foreign"));
        }
        store.abort_constant_chunks(&mut a.physical, error)
    }
    /// Called only with a legitimately current full source proof. Same PC stays
    /// bound until Published; no entry charge or completed observation is repeated.
    pub(crate) fn advance_constant_publication_chunks(
        &mut self,
        a: &mut ConstantPublicationActivation,
        proof: &AdmittedConstantUse<'_>,
        store: &mut GuestStore,
    ) -> ConstantPublicationChunk {
        if a.cleaning() {
            return self.drain_constant_publication_chunks(a, store);
        }
        if let Err(error) = self.validate_constant_chunk_use(a, proof, store) {
            if let Err(diagnostic) = self.abort_constant_publication_chunks(a, store, error) {
                return ConstantPublicationChunk::Fault {
                    diagnostic,
                    cleanup_pending: true,
                };
            }
            return self.drain_constant_publication_chunks(a, store);
        }
        self.step_constant_publication_chunks(a, store)
    }
    /// Source failure may make a fresh proof unavailable. Closed private physical
    /// rollback uses only the existing opaque owner/cursor and cannot publish.
    pub(crate) fn drain_constant_publication_chunks(
        &mut self,
        a: &mut ConstantPublicationActivation,
        store: &mut GuestStore,
    ) -> ConstantPublicationChunk {
        if self.require_store(store).is_err()
            || a.session != store.pattern_binding_session()
            || !a.cleaning()
            || a.done
            || a.frame != self.anchor
            || a.epoch != store.budget().epoch()
        {
            return ConstantPublicationChunk::Fault {
                diagnostic: invalid("constant private rollback state is unavailable"),
                cleanup_pending: true,
            };
        }
        self.step_constant_publication_chunks(a, store)
    }
    fn step_constant_publication_chunks(
        &mut self,
        a: &mut ConstantPublicationActivation,
        store: &mut GuestStore,
    ) -> ConstantPublicationChunk {
        match store.advance_constant_chunks(&mut a.physical, &mut self.fuel) {
            ConstantChunkPoll::Dispatch {
                steps,
                bytes,
                cleaning,
            } => ConstantPublicationChunk::Dispatch {
                steps,
                bytes,
                cleaning,
            },
            ConstantChunkPoll::Failed(diagnostic) => {
                // A successful private drain is marked finished by the physical
                // cursor; stale/internal cleanup errors keep the activation owned.
                let pending = !a.physical.finished_for_frame();
                if !pending {
                    a.done = true;
                }
                ConstantPublicationChunk::Fault {
                    diagnostic,
                    cleanup_pending: pending,
                }
            }
            ConstantChunkPoll::Ready(mut pending) => {
                // Agreement only, after the current full source-use preflight:
                // the admitted scalar/primitive-array publication must finish
                // exactly its original per-field logical charges.
                let field_agreement = store.type_descriptor(a.ty).map(|descriptor| {
                    let expected = match descriptor.key.as_ref() {
                        crate::runtime::shape::TypeKey::Array { len, .. } => u64::from(*len),
                        _ => 0,
                    };
                    a.fields_charged() == expected
                });
                if !matches!(field_agreement, Ok(true)) {
                    let error = match field_agreement {
                        Err(error) => error,
                        _ => invalid("constant completed without exact original field charges"),
                    };
                    if let Err(diagnostic) =
                        store.cleanup_ready_constant_chunks(&mut a.physical, pending, error)
                    {
                        return ConstantPublicationChunk::Fault {
                            diagnostic,
                            cleanup_pending: true,
                        };
                    }
                    return ConstantPublicationChunk::Dispatch {
                        steps: 0,
                        bytes: 0,
                        cleaning: true,
                    };
                }
                if let Err(error) = self.preflight_source_view(a.destination, Some(&a.source)) {
                    if let Err(diagnostic) =
                        store.cleanup_ready_constant_chunks(&mut a.physical, pending, error)
                    {
                        return ConstantPublicationChunk::Fault {
                            diagnostic,
                            cleanup_pending: true,
                        };
                    }
                    return ConstantPublicationChunk::Dispatch {
                        steps: 0,
                        bytes: 0,
                        cleaning: true,
                    };
                }
                if let Err(error) = self.fuel.poll() {
                    if let Err(diagnostic) =
                        store.cleanup_ready_constant_chunks(&mut a.physical, pending, error)
                    {
                        return ConstantPublicationChunk::Fault {
                            diagnostic,
                            cleanup_pending: true,
                        };
                    }
                    return ConstantPublicationChunk::Dispatch {
                        steps: 0,
                        bytes: 0,
                        cleaning: true,
                    };
                }
                match store.commit_pending_constant(&mut pending, a.cell) {
                    Ok(()) => {
                        // Move the already-admitted source term; no late clone or
                        // array traversal occurs after the physical commit.
                        let source = std::mem::replace(
                            &mut a.source,
                            crate::traits::term::TyTerm::Primitive(
                                crate::ir::op::PrimitiveType::Unit,
                            ),
                        );
                        match self.set_source_view(a.destination, Some(source)) {
                            Ok(()) => {
                                a.done = true;
                                ConstantPublicationChunk::Published
                            }
                            Err(diagnostic) => ConstantPublicationChunk::Fault {
                                diagnostic,
                                cleanup_pending: true,
                            },
                        }
                    }
                    Err(error) => {
                        if let Err(diagnostic) =
                            store.cleanup_ready_constant_chunks(&mut a.physical, pending, error)
                        {
                            return ConstantPublicationChunk::Fault {
                                diagnostic,
                                cleanup_pending: true,
                            };
                        }
                        ConstantPublicationChunk::Dispatch {
                            steps: 0,
                            bytes: 0,
                            cleaning: true,
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "frame_constant_chunk_tests.rs"]
mod tests;
