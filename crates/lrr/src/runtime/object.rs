//! Partial aggregate ownership, separate from committed declaration-order fields.
use super::{
    arena::resource,
    cell::{CellId, TicketId},
    shape::TypeId,
    value::ValueRepr,
};
use crate::core::diagnostic::Diagnostic;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ConstructionState {
    Building,
    Cleaning,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum StructuralConstructionKind {
    Tuple,
    Array,
}
/// Completed values belong to this guard until commit or explicit cleanup.
/// Completion ordinals record evaluation order independently of field storage.
pub struct ConstructionRecord {
    /// None is an unentered structural field, not a guessed/default type.
    /// A completed observed field records its reached concrete descriptor.
    pub(super) expected: Vec<Option<TypeId>>,
    pub(super) observed_kind: Option<StructuralConstructionKind>,
    pub(super) observed_element: Option<TypeId>,
    pub(super) fields: Vec<Option<ValueRepr>>,
    pub(super) completed: Vec<u32>,
    pub(super) state: ConstructionState,
    /// Only source-checked nominal admission may fill this immutable plan.
    pub(super) nominal: Option<(TypeId, u32)>,
    /// The transferred field's exact continuation blocks the next field and
    /// guard retirement until its owned cleanup ticket has been consumed.
    pub(super) pending_cleanup: Option<(u32, TicketId, ValueRepr)>,
    /// A generic declaration's source-checked variant whose type arguments
    /// its parameter-typed fields determine as they complete.
    pub(super) pending_nominal: Option<Box<crate::frontend::types::BoundNominalVariant>>,
}
impl ConstructionRecord {
    pub(super) fn partial(expected: &[Option<TypeId>]) -> Result<Self, Diagnostic> {
        Self::allocate(expected.len(), None, |index| expected[index])
    }
    pub(super) fn new(expected: &[TypeId]) -> Result<Self, Diagnostic> {
        Self::allocate(expected.len(), None, |index| Some(expected[index]))
    }
    pub(super) fn observed(
        kind: StructuralConstructionKind,
        count: usize,
    ) -> Result<Self, Diagnostic> {
        Self::allocate(count, Some(kind), |_| None)
    }
    fn allocate(
        count: usize,
        observed_kind: Option<StructuralConstructionKind>,
        field_type: impl Fn(usize) -> Option<TypeId>,
    ) -> Result<Self, Diagnostic> {
        let mut types = Vec::new();
        let mut fields = Vec::new();
        let mut completed = Vec::new();
        types.try_reserve_exact(count).map_err(|_| resource())?;
        fields.try_reserve_exact(count).map_err(|_| resource())?;
        completed.try_reserve_exact(count).map_err(|_| resource())?;
        types.extend((0..count).map(field_type));
        fields.resize(count, None);
        Ok(Self {
            expected: types,
            observed_kind,
            observed_element: None,
            fields,
            completed,
            state: ConstructionState::Building,
            nominal: None,
            pending_cleanup: None,
            pending_nominal: None,
        })
    }
    pub(super) fn owned_bytes(&self) -> Option<usize> {
        self.expected
            .capacity()
            .checked_mul(std::mem::size_of::<Option<TypeId>>())?
            .checked_add(
                self.fields
                    .capacity()
                    .checked_mul(std::mem::size_of::<Option<ValueRepr>>())?,
            )?
            .checked_add(
                self.completed
                    .capacity()
                    .checked_mul(std::mem::size_of::<u32>())?,
            )
    }
}

pub(super) enum ObjectStorage {
    String(Vec<u8>),
    /// Invisible reservation until every field has been installed.
    Constructing,
    Aggregate(AggregateStorage),
}
pub(super) struct AggregateStorage {
    pub fields: Vec<CellId>,
    pub initialized: InitializedFields,
    /// Ordinary aggregate cleanup follows declaration order, not completion order.
    pub next_cleanup: usize,
    /// One suspended ordinary field obligation; later fields and parent release
    /// cannot overtake its exact owner/ticket completion.
    pub pending_cleanup: Option<(CellId, super::cell::TicketId)>,
    pub variant: Option<u32>,
}
/// Run-local field state; never serialized as a host layout or native pointer.
/// A set bit means the entire owning child value is initialized, including its
/// own cached aggregate summary. References terminate this relation.
pub(super) struct InitializedFields {
    bits: Vec<u64>,
    fields: usize,
    live: usize,
}
impl InitializedFields {
    pub fn empty(fields: usize) -> Result<Self, Diagnostic> {
        let words = fields.checked_add(63).ok_or_else(resource)? / 64;
        let mut bits = Vec::new();
        bits.try_reserve_exact(words).map_err(|_| resource())?;
        bits.resize(words, 0);
        Ok(Self {
            bits,
            fields,
            live: 0,
        })
    }
    pub fn complete(fields: usize) -> Result<Self, Diagnostic> {
        let words = fields.checked_add(63).ok_or_else(resource)? / 64;
        let mut bits = Vec::new();
        bits.try_reserve_exact(words).map_err(|_| resource())?;
        bits.resize(words, u64::MAX);
        let tail = fields % 64;
        if tail != 0 {
            let last = bits.last_mut().ok_or_else(super::value::invalid)?;
            *last = 1_u64.checked_shl(tail as u32).ok_or_else(resource)? - 1;
        }
        Ok(Self {
            bits,
            fields,
            live: fields,
        })
    }
    pub fn owned_bytes(&self) -> Option<usize> {
        self.bits.capacity().checked_mul(std::mem::size_of::<u64>())
    }
    pub fn complete_value(&self) -> bool {
        self.live == self.fields
    }
    /// A growable aggregate's new field slot at `index` (shifting later
    /// fields up), initialized or not.
    pub fn insert_field(&mut self, index: usize, initialized: bool) -> Result<(), Diagnostic> {
        if index > self.fields {
            return Err(super::value::invalid());
        }
        if index == self.fields {
            if self.fields % 64 == 0 {
                self.bits.push(0);
            }
            if initialized {
                self.bits[index / 64] |= 1_u64 << (index % 64);
                self.live += 1;
            }
            self.fields += 1;
            return Ok(());
        }
        let mut flags: Vec<bool> = (0..self.fields).map(|i| self.bit(i)).collect();
        flags.insert(index, initialized);
        self.rebuild(&flags);
        Ok(())
    }
    /// Remove the field slot at `index` (shifting later fields down). A
    /// removed initialized field no longer counts as live.
    pub fn remove_field(&mut self, index: usize) -> Result<(), Diagnostic> {
        if index >= self.fields {
            return Err(super::value::invalid());
        }
        if index + 1 == self.fields {
            if self.bit(index) {
                self.live -= 1;
                self.bits[index / 64] &= !(1_u64 << (index % 64));
            }
            self.fields -= 1;
            if self.fields % 64 == 0 {
                self.bits.pop();
            }
            return Ok(());
        }
        let mut flags: Vec<bool> = (0..self.fields).map(|i| self.bit(i)).collect();
        flags.remove(index);
        self.rebuild(&flags);
        Ok(())
    }
    fn bit(&self, index: usize) -> bool {
        self.bits[index / 64] & (1_u64 << (index % 64)) != 0
    }
    fn rebuild(&mut self, flags: &[bool]) {
        self.fields = flags.len();
        self.live = flags.iter().filter(|set| **set).count();
        self.bits.clear();
        self.bits.resize(flags.len().div_ceil(64), 0);
        for (index, set) in flags.iter().enumerate() {
            if *set {
                self.bits[index / 64] |= 1_u64 << (index % 64);
            }
        }
    }
    pub fn field(&self, index: usize) -> Result<bool, Diagnostic> {
        if index >= self.fields || self.live > self.fields {
            return Err(super::value::invalid());
        }
        let bits = self
            .bits
            .get(index / 64)
            .ok_or_else(super::value::invalid)?;
        Ok(bits & (1_u64 << (index % 64)) != 0)
    }
    pub fn preview(&self, index: usize, initialized: bool) -> Result<bool, Diagnostic> {
        Ok(self.changed_count(index, initialized)? == self.fields)
    }
    fn changed_count(&self, index: usize, initialized: bool) -> Result<usize, Diagnostic> {
        let previous = self.field(index)?;
        let live = if previous == initialized {
            self.live
        } else if initialized {
            self.live.checked_add(1).ok_or_else(super::value::invalid)?
        } else {
            self.live.checked_sub(1).ok_or_else(super::value::invalid)?
        };
        if live > self.fields {
            return Err(super::value::invalid());
        }
        Ok(live)
    }
    pub fn set(&mut self, index: usize, initialized: bool) -> Result<(), Diagnostic> {
        let live = self.changed_count(index, initialized)?;
        let word = self
            .bits
            .get_mut(index / 64)
            .ok_or_else(super::value::invalid)?;
        let mask = 1_u64 << (index % 64);
        if initialized {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        self.live = live;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::InitializedFields;

    /// A growable aggregate's summary tracks insertions and removals at the
    /// end and in the middle across word boundaries.
    #[test]
    fn initialized_fields_grow_and_shrink_with_the_live_count() {
        let mut fields = InitializedFields::complete(0).unwrap();
        for index in 0..130 {
            fields.insert_field(index, true).unwrap();
            assert!(fields.complete_value());
        }
        fields.insert_field(64, false).unwrap();
        assert!(!fields.complete_value());
        assert!(!fields.field(64).unwrap());
        assert!(fields.field(65).unwrap());
        assert!(fields.field(130).unwrap());
        fields.set(64, true).unwrap();
        assert!(fields.complete_value());
        fields.remove_field(10).unwrap();
        assert!(fields.complete_value());
        assert!(fields.field(129).unwrap());
        assert!(fields.field(130).is_err());
        while fields.remove_field(0).is_ok() {
            assert!(fields.complete_value());
        }
        assert!(fields.field(0).is_err());
        fields.insert_field(0, false).unwrap();
        assert!(!fields.complete_value());
        fields.remove_field(0).unwrap();
        assert!(fields.complete_value());
    }
}
