//! Actual source facts of ordinary tuple construction. Runtime tuple layout
//! checks membership only; it never supplies missing source facts.
use super::*;
use crate::{runtime::shape::TypeKey, traits::term::TyTerm};

impl Frame {
    pub(super) fn tuple_operand_bytes(operands: Option<&[SlotId]>) -> Option<usize> {
        operands.map_or(Some(0), |operands| {
            operands
                .len()
                .checked_mul(std::mem::size_of::<SlotId>())?
                .checked_add(3 * std::mem::size_of::<usize>())
        })
    }

    pub(super) fn prepare_tuple_source_operands(
        &self,
        plan: &crate::syntax::constructors::ConstructorPlan,
    ) -> Result<Option<Arc<[SlotId]>>, Diagnostic> {
        let crate::syntax::constructors::ConstructorKind::Tuple(fields) = plan.kind() else {
            return Ok(None);
        };
        let mut operands = Vec::new();
        operands
            .try_reserve_exact(fields.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        operands.extend(fields.iter().map(|field| field.value));
        Ok(Some(operands.into()))
    }

    /// Preflight is allocation free for retained source facts. The existing
    /// observed guard query may return its bounded descriptor key, exactly as
    /// ordinary constructor finish already requires.
    pub(super) fn preflight_constructor_tuple_source(
        &self,
        active: &ActiveConstruction,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<bool, Diagnostic> {
        let Some(operands) = &active.source_operands else {
            return Ok(false);
        };
        self.slot(destination)?;
        for slot in operands.iter() {
            if store.state(self.cell(*slot)?)? != CellState::Moved {
                return Err(invalid(
                    "tuple source operand was not transferred to its guard",
                ));
            }
            let Some(source) = self.source_view_ref(*slot)? else {
                return Ok(false);
            };
            if !Self::supported_structural_source(source) {
                // Nominal/member and symbolic tuple source composition still
                // require their ordinary source-proof table. Do not guess.
                return Ok(false);
            }
        }
        let key = match active.result {
            ConstructionResult::Bound(target) => {
                std::borrow::Cow::Borrowed(store.type_descriptor(target)?.key.as_ref())
            }
            ConstructionResult::Observed(_) => {
                std::borrow::Cow::Owned(store.observed_construction_key(active.guard)?)
            }
            ConstructionResult::PendingNominal => return Ok(false),
        };
        match key.as_ref() {
            TypeKey::Tuple(members) if members.len() == operands.len() => {
                for (slot, target) in operands.iter().zip(members.iter()) {
                    self.require_bound_structural_source_target(
                        self.source_view_ref(*slot)?
                            .ok_or_else(|| invalid("tuple source disappeared"))?,
                        *target,
                        store,
                    )?;
                }
            }
            TypeKey::Primitive(_)
                if operands.is_empty() && store.find_type(&key) == Some(TypeId(1)) => {}
            _ => {
                return Err(invalid(
                    "tuple source operands differ from completed descriptor",
                ));
            }
        }
        Ok(true)
    }

    pub(super) fn constructor_tuple_source(
        &self,
        active: &ActiveConstruction,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        if !self.preflight_constructor_tuple_source(active, destination, store)? {
            return Ok(None);
        }
        let operands = active
            .source_operands
            .as_ref()
            .ok_or_else(|| invalid("tuple source rows absent"))?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(operands.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for slot in operands.iter() {
            members.push(
                self.source_view_ref(*slot)?
                    .ok_or_else(|| invalid("tuple source disappeared"))?
                    .clone(),
            );
        }
        // `()` is the unit type, spelled `Primitive(Unit)` like a declared
        // unit result; only a non-empty tuple keeps its member sources.
        let source = if members.is_empty() {
            TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)
        } else {
            TyTerm::Tuple(members)
        };
        self.preflight_source_view(destination, Some(&source))?;
        Ok(Some(source))
    }
}
