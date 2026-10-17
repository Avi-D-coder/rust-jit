//! T054 physical instantiation only. Descriptor agreement is not source authority.
//! The reached driver must first validate its source-selected constant/type proof.
#[path = "cell_constant_pending.rs"]
mod pending;
use super::*;
use crate::{
    ir::constant_data::{ConstValue, wire::ConstantWireType},
    runtime::shape::PrimitiveIdentity,
    vm::steps::RunSteps,
};
pub(crate) use pending::chunks::{ConstantChunkActivation, ConstantChunkPoll};
fn mismatch() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CONST-INSTANTIATION",
        "immutable constant and exact guest scalar/array layout differ".into(),
    )
}
/// Preallocated commit metadata; no arbitrary initialized-field summary can enter.
pub(super) struct ReservedArrayStorage {
    count: usize,
    fields: Vec<CellId>,
    initialized: InitializedFields,
}
impl ReservedArrayStorage {
    pub(super) fn new(count: usize) -> Result<Self, Diagnostic> {
        let mut fields = Vec::new();
        fields.try_reserve_exact(count).map_err(|_| resource())?;
        Ok(Self {
            count,
            fields,
            initialized: InitializedFields::complete(count)?,
        })
    }
    fn owned_bytes(&self) -> Result<usize, Diagnostic> {
        self.fields
            .capacity()
            .checked_mul(std::mem::size_of::<CellId>())
            .and_then(|n| n.checked_add(self.initialized.owned_bytes()?))
            .ok_or_else(resource)
    }
    pub(super) fn into_parts(
        self,
        count: usize,
    ) -> Result<(Vec<CellId>, InitializedFields), Diagnostic> {
        if count != self.count || !self.fields.is_empty() || self.fields.capacity() < count {
            return Err(ownership());
        }
        Ok((self.fields, self.initialized))
    }
}
enum Node {
    Scalar(ValueRepr),
    Array {
        ty: TypeId,
        guard: Vec<ConstructionRecord>,
        storage: ReservedArrayStorage,
        children: Vec<Node>,
    },
}
#[derive(Default)]
struct Totals {
    nodes: usize,
    arrays: usize,
    fields: usize,
}
impl Totals {
    fn node(&mut self) -> Result<(), Diagnostic> {
        self.nodes = self.nodes.checked_add(1).ok_or_else(resource)?;
        Ok(())
    }
}
/// Single-use, epoch-bound physical plan. Its fields cannot be forged, serialized
/// or placed in a constant memo. Preparing a second use yields fresh storage.
pub struct PreparedConstantInstantiation {
    session: StoreSession,
    epoch: u32,
    node: Node,
    totals: Totals,
}
impl GuestStore {
    fn constant_shape(&self, wire: &ConstantWireType, ty: TypeId) -> Result<(), Diagnostic> {
        match (wire, self.types.descriptor(ty)?.key.as_ref()) {
            (ConstantWireType::Scalar(p), TypeKey::Primitive(identity))
                if *identity == PrimitiveIdentity::from_recipe(*p)? =>
            {
                Ok(())
            }
            (
                ConstantWireType::Array {
                    element: wire,
                    length,
                },
                TypeKey::Array { element, len },
            ) if *length == u64::from(*len) => self.constant_shape(wire, *element),
            _ => Err(mismatch()),
        }
    }
    fn constant_node(
        &self,
        value: &ConstValue,
        wire: &ConstantWireType,
        ty: TypeId,
        totals: &mut Totals,
    ) -> Result<Node, Diagnostic> {
        totals.node()?;
        match wire {
            ConstantWireType::Scalar(_) => {
                let scalar = value.scalar().ok_or_else(mismatch)?;
                scalar.validate_scalar()?;
                if scalar.type_id != ty.0 || !(1..=7).contains(&scalar.kind) {
                    return Err(mismatch());
                }
                Ok(Node::Scalar(scalar))
            }
            ConstantWireType::Array {
                element: wire,
                length,
            } => {
                let TypeKey::Array { element, len } = self.types.descriptor(ty)?.key.as_ref()
                else {
                    return Err(mismatch());
                };
                let values = value.elements().ok_or_else(mismatch)?;
                if values.len() != *len as usize || *length != u64::from(*len) {
                    return Err(mismatch());
                }
                totals.arrays = totals.arrays.checked_add(1).ok_or_else(resource)?;
                totals.fields = totals
                    .fields
                    .checked_add(values.len())
                    .ok_or_else(resource)?;
                let mut children = Vec::new();
                children
                    .try_reserve_exact(values.len())
                    .map_err(|_| resource())?;
                let mut expected = Vec::new();
                expected
                    .try_reserve_exact(values.len())
                    .map_err(|_| resource())?;
                expected.resize(values.len(), *element);
                let guard = ConstructionRecord::new(&expected)?;
                let storage = ReservedArrayStorage::new(values.len())?;
                for value in values {
                    children.push(self.constant_node(value, wire, *element, totals)?);
                }
                let mut records = Vec::new();
                records.try_reserve_exact(1).map_err(|_| resource())?;
                records.push(guard);
                Ok(Node::Array {
                    ty,
                    guard: records,
                    storage,
                    children,
                })
            }
        }
    }
    /// Pure preparation from already-decoded immutable data. The caller still
    /// needs current source/type/query admission; runtime IDs supply no such proof.
    pub fn prepare_constant_instantiation(
        &self,
        value: &ConstValue,
        wire: &ConstantWireType,
        ty: TypeId,
    ) -> Result<PreparedConstantInstantiation, Diagnostic> {
        wire.fingerprint_checked()?;
        self.constant_shape(wire, ty)?;
        let mut totals = Totals::default();
        let node = self.constant_node(value, wire, ty, &mut totals)?;
        Ok(PreparedConstantInstantiation {
            session: self.pattern_binding_session(),
            epoch: self.budget.epoch(),
            node,
            totals,
        })
    }
    fn reserve_constant_instantiation(&mut self, totals: &Totals) -> Result<(), Diagnostic> {
        self.reserve_constant_instantiation_with_destination(totals, 0)
    }
    fn reserve_constant_instantiation_with_destination(
        &mut self,
        totals: &Totals,
        destination_cells: usize,
    ) -> Result<(), Diagnostic> {
        let cells = totals
            .fields
            .checked_add(destination_cells)
            .ok_or_else(resource)?;
        // Reserve arena capacity before any value ownership is published.
        self.constructions
            .reserve(totals.arrays, &mut self.budget)?;
        self.objects.reserve(totals.arrays, &mut self.budget)?;
        self.cells.reserve(cells, &mut self.budget)?;
        self.tickets.reserve(totals.nodes, &mut self.budget)
    }
    /// Physical scalar/array kernel, not a guest source admission path. Charges
    /// one operation plus each processed array field, even zero-sized Unit.
    pub fn instantiate_constant(
        &mut self,
        plan: PreparedConstantInstantiation,
        steps: &mut RunSteps,
    ) -> Result<TicketId, Diagnostic> {
        if plan.session != self.pattern_binding_session() || plan.epoch != self.budget.epoch() {
            return Err(stale());
        }
        steps.charge();
        self.instantiate_constant_admitted(plan, steps)
    }
    /// One ordinary `ValuePath` read of a constant item (or a static's
    /// initial contents): the whole value in its reached operation, charging
    /// each processed node like any aggregate construction.
    pub(crate) fn instantiate_constant_value(
        &mut self,
        plan: PreparedConstantInstantiation,
        steps: &mut RunSteps,
    ) -> Result<TicketId, Diagnostic> {
        if plan.session != self.pattern_binding_session() || plan.epoch != self.budget.epoch() {
            return Err(stale());
        }
        self.reserve_constant_instantiation(&plan.totals)?;
        self.instantiate_constant_node(plan.node, steps)
    }
    /// For an interpreter opcode whose normal dispatch already charged the op.
    pub(crate) fn instantiate_constant_admitted(
        &mut self,
        plan: PreparedConstantInstantiation,
        steps: &mut RunSteps,
    ) -> Result<TicketId, Diagnostic> {
        if plan.session != self.pattern_binding_session() || plan.epoch != self.budget.epoch() {
            return Err(stale());
        }
        // Legacy physical kernel callers are bounded small controls only. Large
        // arrays must retain an owned chunk activation; never loop it to finish
        // inside a single interpreter dispatch. (A cooperative-scheduling contract,
        // not a resource limit.)
        if plan
            .totals
            .nodes
            .checked_add(plan.totals.fields.checked_mul(4).ok_or_else(resource)?)
            .and_then(|n| n.checked_add(plan.totals.arrays.checked_mul(4)?))
            .is_none_or(|n| n > 256)
        {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "constant aggregate requires resumable chunk activation".into(),
            ));
        }
        self.reserve_constant_instantiation(&plan.totals)?;
        self.instantiate_constant_node(plan.node, steps)
    }
    fn instantiate_constant_node(
        &mut self,
        node: Node,
        steps: &mut RunSteps,
    ) -> Result<TicketId, Diagnostic> {
        match node {
            Node::Scalar(value) => self.ticket(value),
            Node::Array {
                ty,
                guard: mut records,
                storage,
                children,
            } => {
                let record = records.pop().ok_or_else(ownership)?;
                let bytes = record.owned_bytes().ok_or_else(resource)?;
                let guard = ConstructionId::from(self.constructions.allocate(
                    record,
                    bytes,
                    &mut self.budget,
                )?);
                let result = (|| {
                    for (i, child) in children.into_iter().enumerate() {
                        steps.charge();
                        let ticket = self.instantiate_constant_node(child, steps)?;
                        if let Err(error) = self.construction_push(
                            guard,
                            u32::try_from(i).map_err(|_| resource())?,
                            ticket,
                        ) {
                            self.abort_constant_ticket(ticket)?;
                            return Err(error);
                        }
                    }
                    self.commit_construction_with_storage(guard, ty, Some(storage))
                })();
                if result.is_err() {
                    self.abort_constant_guard(guard)?;
                }
                result
            }
        }
    }
    // Only privately constructed, unpublished copies reach these rollback paths.
    // They contain no reference/nominal/Drop obligations, callbacks or external
    // owners. Rollback is physical, not a new guest cleanup.
    fn abort_constant_guard(&mut self, guard: ConstructionId) -> Result<(), Diagnostic> {
        let record = self.constructions.take(guard, &mut self.budget)?;
        for i in record.completed.into_iter().rev() {
            self.abort_constant_value(record.fields[i as usize].ok_or_else(ownership)?)?;
        }
        Ok(())
    }
    fn abort_constant_ticket(&mut self, ticket: TicketId) -> Result<(), Diagnostic> {
        let value = self.tickets.take(ticket, &mut self.budget)?.value;
        self.abort_constant_value(value)
    }
    fn abort_constant_value(&mut self, value: ValueRepr) -> Result<(), Diagnostic> {
        if (1..=7).contains(&value.kind) {
            return value.validate_scalar();
        }
        if value.kind != 8
            || !matches!(
                self.types.descriptor(TypeId(value.type_id))?.key.as_ref(),
                TypeKey::Array { .. }
            )
        {
            return Err(ownership());
        }
        let object = self
            .objects
            .take(ObjectId::from_bits(value.payload), &mut self.budget)?;
        let ObjectStorage::Aggregate(storage) = object.storage else {
            return Err(ownership());
        };
        for cell in storage.fields.into_iter().rev() {
            let field = self.cells.take(cell, &mut self.budget)?;
            let CellState::Live(value) = field.state else {
                return Err(ownership());
            };
            self.abort_constant_value(value)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ir::constant_data::wire, ir::op::PrimitiveType};
    fn nested_fixture() -> (ConstValue, ConstantWireType) {
        let wire = ConstantWireType::Array {
            element: Box::new(ConstantWireType::Array {
                element: Box::new(ConstantWireType::Scalar(PrimitiveType::Unit)),
                length: 2,
            }),
            length: 2,
        };
        let mut bytes = b"LRRCNST\0".to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        for _ in 0..2 {
            bytes.push(1);
            bytes.extend_from_slice(&2u64.to_le_bytes());
        }
        bytes.push(0);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&0u64.to_le_bytes());
        }
        (wire::decode(&bytes, &wire, 1 << 20).unwrap(), wire)
    }
    #[test]
    fn private_constant_admitted_join_charges_only_fields_after_dispatch() {
        let mut s = GuestStore::for_epoch(1).unwrap();
        let child = s
            .intern_type(TypeKey::Array {
                element: TypeId(1),
                len: 2,
            })
            .unwrap();
        let ty = s
            .intern_type(TypeKey::Array {
                element: child,
                len: 2,
            })
            .unwrap();
        let (value, wire) = nested_fixture();
        let plan = s.prepare_constant_instantiation(&value, &wire, ty).unwrap();
        let mut fuel = RunSteps::default();
        fuel.charge();
        let ticket = s.instantiate_constant_admitted(plan, &mut fuel).unwrap();
        assert_eq!(fuel.snapshot().used, 7);
        assert_eq!(s.peek_ticket(ticket).unwrap().type_id, ty.0);
    }
}
