//! One normal opcode activation, retaining only unpublished physical state.
use super::*;
use crate::{
    engine::constant_array_publication::AdmittedConstantUse,
    ir::{op::Op, source_constant::SourceConstantInstruction},
};
pub(super) struct BoundSourceConstant {
    pc: crate::core::digest::Digest,
    operation_index: u32,
    instruction: crate::core::digest::Digest,
    publication: ConstantPublicationActivation,
}
impl Frame {
    pub(crate) fn has_source_constant_activation(&self) -> bool {
        self.constant_activation.is_some()
    }
    pub(crate) fn source_constant_operation_index(&self) -> Option<usize> {
        self.constant_activation
            .as_ref()
            .map(|a| a.operation_index as usize)
    }
    pub(crate) fn source_constant_cleaning(&self) -> bool {
        self.constant_activation
            .as_ref()
            .is_some_and(|a| a.publication.cleaning())
    }
    pub(crate) fn check_source_constant_activation(
        &self,
        index: usize,
        op: &Op,
    ) -> Result<(), Diagnostic> {
        let Op::SourceConstant { constant, .. } = op else {
            return Err(invalid("constant activation resumed at another opcode"));
        };
        let a = self
            .constant_activation
            .as_ref()
            .ok_or_else(|| invalid("constant activation absent"))?;
        if a.operation_index as usize != index
            || a.pc != self.pc.fingerprint()
            || a.instruction != constant.identity()
            || !a.publication.charged_on_entry()
            || self.copy_activation.is_some()
            || self.cleanup_activation.is_some()
        {
            return Err(invalid(
                "source constant activation resumed at another bound instruction",
            ));
        }
        Ok(())
    }
    pub(crate) fn start_source_constant_op(
        &mut self,
        index: usize,
        constant: &SourceConstantInstruction,
        proof: &AdmittedConstantUse<'_>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.constant_activation.is_some() || !proof.matches_instruction(constant) {
            return Err(invalid(
                "source constant instruction lacks matching current use",
            ));
        }
        let index =
            u32::try_from(index).map_err(|_| invalid("constant operation index overflow"))?;
        let prepared =
            self.prepare_constant_publication(proof, constant.value(), constant.wire(), store)?;
        let publication = self.begin_constant_publication_chunks(prepared, store, |frame| {
            // The same entry/counter sequence as normal VM dispatch, once only.
            frame.charge_fuel()?;
            frame.counters.operations_started = frame
                .counters
                .operations_started
                .checked_add(1)
                .ok_or_else(|| invalid("operation counter overflow"))?;
            let count = &mut frame.counters.started_by_opcode
                [usize::from(crate::ir::op::SOURCE_CONSTANT_OPCODE)];
            *count = count
                .checked_add(1)
                .ok_or_else(|| invalid("constant opcode counter overflow"))?;
            Ok(())
        })?;
        self.constant_activation = Some(BoundSourceConstant {
            pc: self.pc.fingerprint(),
            operation_index: index,
            instruction: constant.identity(),
            publication,
        });
        Ok(())
    }
    pub(crate) fn advance_source_constant_op(
        &mut self,
        index: usize,
        op: &Op,
        proof: &AdmittedConstantUse<'_>,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        self.check_source_constant_activation(index, op)?;
        let Op::SourceConstant { constant, .. } = op else {
            return Err(invalid("source constant opcode absent"));
        };
        let mut a = self
            .constant_activation
            .take()
            .ok_or_else(|| invalid("constant activation disappeared"))?;
        let poll = if !proof.matches_instruction(constant) {
            match self.abort_constant_publication_chunks(
                &mut a.publication,
                store,
                invalid("resumed source constant proof differs"),
            ) {
                Ok(()) => self.drain_constant_publication_chunks(&mut a.publication, store),
                Err(diagnostic) => ConstantPublicationChunk::Fault {
                    diagnostic,
                    cleanup_pending: true,
                },
            }
        } else {
            self.advance_constant_publication_chunks(&mut a.publication, proof, store)
        };
        match poll {
            ConstantPublicationChunk::Published => Ok(true),
            ConstantPublicationChunk::Dispatch {
                steps,
                bytes,
                cleaning,
            } => {
                let valid =
                    steps <= 256 && bytes <= 64 << 10 && cleaning == a.publication.cleaning();
                // Even a rejected physical continuation keeps its private owner
                // attached so the coordinator can drain it before terminal fault.
                self.constant_activation = Some(a);
                if !valid {
                    return Err(invalid(
                        "constant physical continuation exceeded its bounded contract",
                    ));
                }
                Ok(false)
            }
            ConstantPublicationChunk::Fault {
                diagnostic,
                cleanup_pending,
            } => {
                if cleanup_pending {
                    self.constant_activation = Some(a);
                }
                Err(diagnostic)
            }
        }
    }
    /// No source permission is needed to drain this exact unpublished owner.
    /// An error is returned only after bounded rollback has actually completed.
    pub(crate) fn abort_source_constant_op(
        &mut self,
        store: &mut GuestStore,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        let mut a = self
            .constant_activation
            .take()
            .ok_or_else(|| invalid("constant abort activation absent"))?;
        let result = self.abort_constant_publication_chunks(&mut a.publication, store, error);
        self.constant_activation = Some(a);
        result
    }
    pub(crate) fn drain_source_constant_op(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let mut a = self
            .constant_activation
            .take()
            .ok_or_else(|| invalid("constant drain activation absent"))?;
        match self.drain_constant_publication_chunks(&mut a.publication, store) {
            ConstantPublicationChunk::Dispatch { .. } => {
                self.constant_activation = Some(a);
                Ok(false)
            }
            ConstantPublicationChunk::Fault {
                diagnostic,
                cleanup_pending,
            } => {
                if cleanup_pending {
                    self.constant_activation = Some(a);
                }
                Err(diagnostic)
            }
            ConstantPublicationChunk::Published => {
                Err(invalid("constant rollback published a value"))
            }
        }
    }
}
