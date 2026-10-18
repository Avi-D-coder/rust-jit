//! Promoted constant temporaries: rustc's rvalue static promotion of
//! `&<constant expression>` gives the temporary a `'static` location. The run
//! owns one such location per promoting site and value type, under a frame
//! anchor no frame retires, so references to it stay valid for the whole
//! run. Later executions of the same site reuse that location, as a compiled
//! program reuses the one promoted allocation.
use super::*;
use crate::core::digest::Digest;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct PromotedStorage {
    anchor: Option<FrameId>,
    cells: BTreeMap<(Digest, TypeId), CellId>,
}

impl GuestStore {
    /// The site's promoted location for a value of type `ty`, once installed.
    pub(crate) fn promoted_cell(&self, site: Digest, ty: TypeId) -> Option<CellId> {
        self.promoted.cells.get(&(site, ty)).copied()
    }
    /// Install the first evaluated value of `site` as its promoted location.
    pub(crate) fn install_promoted(
        &mut self,
        site: Digest,
        ty: TypeId,
        ticket: TicketId,
    ) -> Result<CellId, Diagnostic> {
        if self.promoted.cells.contains_key(&(site, ty)) {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "promoted location is already installed".into(),
            ));
        }
        let anchor = match self.promoted.anchor {
            Some(anchor) => anchor,
            None => {
                let anchor = self.new_frame_anchor()?;
                self.promoted.anchor = Some(anchor);
                anchor
            }
        };
        let slot = u32::try_from(self.promoted.cells.len()).map_err(|_| {
            Diagnostic::new("LRR-E-INTERNAL-IR", "promoted location index overflow".into())
        })?;
        let cell = self.new_cell(
            StorageContract::Exact(ty),
            false,
            CellOwner::FrameSlot {
                frame: anchor,
                slot,
            },
        )?;
        self.initialize(cell, ticket)?;
        self.promoted.cells.insert((site, ty), cell);
        Ok(cell)
    }
}

impl GuestStore {
    /// Install the run's `unsafe_cell` lang item.
    pub(crate) fn install_unsafe_cell(&mut self, identity: Option<Digest>) {
        self.unsafe_cell = identity;
    }
    /// Whether rustc promotes a borrow of this constant value to `'static`
    /// (`&CONST`): the value is plain data, with no interior mutability and no
    /// drop glue, so one shared location is indistinguishable from a fresh
    /// temporary per use. Scalars and shared references qualify; an aggregate
    /// qualifies when its nominal declaration is not `UnsafeCell`, has no
    /// `Drop` impl and owns no heap allocation, and so do all its fields.
    pub(crate) fn value_promotable(&self, root: ValueRepr) -> Result<bool, Diagnostic> {
        let mut pending = vec![root];
        while let Some(value) = pending.pop() {
            match value.kind {
                1..=7 | 11 | 12 => {}
                9 => {
                    if self.references.get(RefId::from_bits(value.payload))?.access
                        == Access::Mutable
                    {
                        return Ok(false);
                    }
                }
                8 => {
                    if let Some(ty) = self.effective_type(value)?
                        && let TypeKey::Nominal { identity, .. } =
                            self.types.descriptor(ty)?.key.as_ref()
                        && (self.unsafe_cell == Some(*identity)
                            || self.owned_box == Some(*identity)
                            || self.owned_vec == Some(*identity)
                            || self
                                .user_drop_types
                                .as_ref()
                                .is_none_or(|facts| !facts.definitely_no_drop(identity)))
                    {
                        return Ok(false);
                    }
                    let ObjectStorage::Aggregate(storage) = &self
                        .objects
                        .get(ObjectId::from_bits(value.payload))?
                        .storage
                    else {
                        return Ok(false);
                    };
                    for field in &storage.fields {
                        pending.push(self.admit_move(*field)?);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
}
