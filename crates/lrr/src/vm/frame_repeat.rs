//! `[element; count]`: the element is evaluated once; the array holds `count`
//! copies of it (Copy element), the element itself (`count == 1`), or nothing
//! (`count == 0`, the element is dropped). Aggregate copies use the same
//! resumable worker and construction guards as ordinary owning reads.
use super::{CellId, ConstructionId, Frame, TicketId, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::SlotId,
    runtime::{
        cell::{
            CellState, GuestStore, IntrinsicRead,
            aggregate_copy::{AggregateCopy, CopyProgress},
        },
        shape::{TypeId, TypeKey},
        value::ValueRepr,
    },
};

pub(super) struct RepeatActivation {
    source_slot: SlotId,
    source: CellId,
    source_epoch: u32,
    value: ValueRepr,
    count_slot: SlotId,
    length: u32,
    guard_slot: SlotId,
    guard: ConstructionId,
    destination: SlotId,
    source_view: Option<crate::traits::term::TyTerm>,
    next: u32,
    copy: Option<AggregateCopy>,
    pending: Option<TicketId>,
}

impl Frame {
    fn repeat_operands(
        &self,
        source: SlotId,
        count: SlotId,
        store: &GuestStore,
    ) -> Result<(ValueRepr, u32), Diagnostic> {
        let cell = self.cell(source)?;
        let CellState::Live(element) = store.state(cell)? else {
            return Err(invalid("repeat element not live"));
        };
        // An unconstrained numeric literal element takes rustc's i32/f64
        // fallback (a bound literal keeps its bound type).
        let element = if matches!(element.kind, 11 | 12) {
            store.read_format_scalar(cell)?
        } else {
            element
        };
        Ok((element, self.repeat_length(count, store)?))
    }
    fn repeat_length(&self, count: SlotId, store: &GuestStore) -> Result<u32, Diagnostic> {
        let CellState::Live(length) = store.state(self.cell(count)?)? else {
            return Err(invalid("repeat count not live"));
        };
        // The count is a `usize` constant.
        if length.type_id != 13 || length.kind != 5 {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "repeat count requires a usize value".into(),
            ));
        }
        let length = u32::try_from(length.payload).map_err(|_| {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "repeat count exceeds the guest array length representation".into(),
            )
        })?;
        Ok(length)
    }
    pub(crate) fn preflight_repeat_element(
        &self,
        guard: SlotId,
        source: SlotId,
        count: SlotId,
        more: SlotId,
        initialize: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_write_type(more, TypeId(2), store)?;
        let length = self.repeat_length(count, store)?;
        if initialize {
            self.preflight_constructor_start(guard)?;
            if self.hint(source)?.is_none() {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "constant repeat element requires its declared source Type".into(),
                ));
            }
        } else {
            let active = self
                .constructor_slot(guard)?
                .construction
                .as_ref()
                .filter(|active| !active.cleaning)
                .ok_or_else(|| invalid("constant repeat guard absent"))?;
            let super::ConstructionResult::Bound(ty) = active.result else {
                return Err(invalid("constant repeat requires its bound array guard"));
            };
            if !matches!(store.type_descriptor(ty)?.key.as_ref(), TypeKey::Array { len, .. } if *len == length)
            {
                return Err(invalid("constant repeat count differs from its guard"));
            }
            let field = store
                .construction_next_field(active.guard)?
                .ok_or_else(|| invalid("constant repeat is already complete"))?;
            self.preflight_constructor_field(guard, field, source, store)?;
        }
        Ok(())
    }
    pub(crate) fn repeat_element(
        &mut self,
        guard: SlotId,
        source: SlotId,
        count: SlotId,
        more: SlotId,
        initialize: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_repeat_element(guard, source, count, more, initialize, store)?;
        let length = self.repeat_length(count, store)?;
        let field = if initialize {
            let element = self
                .hint(source)?
                .ok_or_else(|| invalid("repeat source Type hint disappeared"))?;
            let ty = store.intern_type(TypeKey::Array {
                element,
                len: length,
            })?;
            let destination = self
                .body
                .constructor_node(guard)?
                .value
                .ok_or_else(|| invalid("repeat result absent"))?;
            self.preflight_write_type(destination, ty, store)?;
            self.begin_structural_constructor(guard, ty, store)?;
            if let Some(element) = self.source_view(source)? {
                self.set_source_view(
                    destination,
                    Some(crate::traits::term::TyTerm::Array(
                        Box::new(element),
                        crate::traits::term::ConstTerm::Value(u64::from(length)),
                    )),
                )?;
            }
            None
        } else {
            let active = self
                .constructor_slot(guard)?
                .construction
                .as_ref()
                .ok_or_else(|| invalid("repeat guard disappeared"))?;
            Some(
                store
                    .construction_next_field(active.guard)?
                    .ok_or_else(|| invalid("repeat already complete"))?,
            )
        };
        let next = match field {
            Some(field) => field
                .checked_add(1)
                .ok_or_else(|| invalid("repeat cursor encoding overflow"))?,
            None => 0,
        };
        // Write the private branch test before transferring the child owner.
        // A refused move cannot leave a consumed value followed by allocation.
        let test = store.ticket(ValueRepr::boolean(next < length))?;
        self.write(more, test, store)?;
        if let Some(field) = field {
            self.constructor_field(guard, field, source, store)?;
        }
        Ok(())
    }
    /// Whether one element value may be duplicated by value (scalars and
    /// shared references); aggregate Copy goes through its resumable copy.
    fn repeat_copies(element: &ValueRepr, store: &GuestStore) -> Result<bool, Diagnostic> {
        Ok((1..=7).contains(&element.kind)
            || (element.kind == 9
                && matches!(
                    store.type_descriptor(TypeId(element.type_id))?.key.as_ref(),
                    TypeKey::Ref { mutable: false, .. }
                )))
    }
    pub(crate) fn preflight_array_repeat(
        &self,
        source: SlotId,
        count: SlotId,
        _destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let (element, length) = self.repeat_operands(source, count, store)?;
        if length > 1
            && !Self::repeat_copies(&element, store)?
            && !(element.kind == 8
                && store.admit_owning_read(self.cell(source)?)? == IntrinsicRead::CopyAggregate)
        {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "repeating a non-scalar element requires its aggregate Copy".into(),
            ));
        }
        Ok(())
    }
    /// Only actual Copy aggregates enter this activation. A non-Copy const
    /// requires its retained const-evaluation policy, not a Copy exception.
    pub(super) fn begin_repeat_copy(
        &mut self,
        source: SlotId,
        count: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<Option<RepeatActivation>, Diagnostic> {
        self.preflight_array_repeat(source, count, destination, store)?;
        let (value, length) = self.repeat_operands(source, count, store)?;
        if length <= 1 || value.kind != 8 {
            return Ok(None);
        }
        let node = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .filter(|node| node.value == Some(destination))
            .ok_or_else(|| invalid("array repeat owning expression differs"))?;
        let guard_slot = node
            .constructor_guard
            .ok_or_else(|| invalid("array repeat construction guard absent"))?;
        self.preflight_constructor_start(guard_slot)?;
        let ty = store.intern_type(TypeKey::Array {
            element: TypeId(value.type_id),
            len: length,
        })?;
        self.preflight_write_type(destination, ty, store)?;
        let source_view = self.source_view(source)?.map(|element| {
            crate::traits::term::TyTerm::Array(
                Box::new(element),
                crate::traits::term::ConstTerm::Value(u64::from(length)),
            )
        });
        self.preflight_source_view(destination, source_view.as_ref())?;
        let source_cell = self.cell(source)?;
        let source_epoch = store.location_epoch(source_cell)?;
        self.begin_structural_constructor(guard_slot, ty, store)?;
        let guard = self
            .constructor_slot(guard_slot)?
            .construction
            .as_ref()
            .ok_or_else(|| invalid("array repeat construction absent"))?
            .guard;
        Ok(Some(RepeatActivation {
            source_slot: source,
            source: source_cell,
            source_epoch,
            value,
            count_slot: count,
            length,
            guard_slot,
            guard,
            destination,
            source_view,
            next: 0,
            copy: None,
            pending: None,
        }))
    }
    pub(super) fn advance_repeat_copy(
        &mut self,
        repeat: &mut RepeatActivation,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        self.constructor_slot(repeat.guard_slot)?
            .construction
            .as_ref()
            .filter(|active| active.guard == repeat.guard && !active.cleaning)
            .ok_or_else(|| invalid("array repeat construction changed"))?;
        let length = store.state(self.cell(repeat.count_slot)?)?;
        if length != CellState::Live(ValueRepr::unsigned(TypeId(13), u64::from(repeat.length))?) {
            return Err(invalid("array repeat count changed during copying"));
        }
        // The original stays live until transferred into the final element.
        // Its epoch also protects the gaps between completed child copies.
        if repeat.next < repeat.length
            && !(repeat.next + 1 == repeat.length && repeat.pending.is_some())
            && (self.cell(repeat.source_slot)? != repeat.source
                || store.location_epoch(repeat.source)? != repeat.source_epoch
                || store.state(repeat.source)? != CellState::Live(repeat.value))
        {
            return Err(Diagnostic::new(
                "LRR-E-STALE-HANDLE",
                "array repeat operand changed during copying".into(),
            ));
        }
        if let Some(ticket) = repeat.pending {
            store.construction_push(repeat.guard, repeat.next, ticket)?;
            repeat.pending = None;
            repeat.next = repeat
                .next
                .checked_add(1)
                .ok_or_else(|| invalid("array repeat field cursor overflow"))?;
            return Ok(false);
        }
        if repeat.next == repeat.length {
            self.finish_constructor(repeat.guard_slot, repeat.destination, store)?;
            if let Some(view) = repeat.source_view.clone() {
                self.set_source_view(repeat.destination, Some(view))?;
            }
            return Ok(true);
        }
        if repeat.next + 1 == repeat.length {
            repeat.pending = Some(store.move_from(repeat.source)?);
        } else if let Some(copy) = repeat.copy.as_mut() {
            if let CopyProgress::Complete(ticket) = self.advance_copy_work(copy, store)? {
                repeat.pending = Some(ticket);
                repeat.copy = None;
            }
        } else {
            repeat.copy = Some(AggregateCopy::begin(store, repeat.source)?);
        }
        Ok(false)
    }
    pub(crate) fn array_repeat(
        &mut self,
        source: SlotId,
        count: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_array_repeat(source, count, destination, store)?;
        let literal = matches!(
            store.state(self.cell(source)?)?,
            CellState::Live(value) if matches!(value.kind, 11 | 12)
        );
        let (element, length) = self.repeat_operands(source, count, store)?;
        if length > 1 && element.kind == 8 {
            return Err(crate::runtime::cell::aggregate_copy::requires_activation());
        }
        let element_type = TypeId(element.type_id);
        let array = store.intern_type(TypeKey::Array {
            element: element_type,
            len: length,
        })?;
        let moved = store.move_from(self.cell(source)?)?;
        let guard = store.begin_construction(&vec![element_type; length as usize])?;
        match length {
            0 => store.discard(moved)?,
            1 if !literal => store.construction_push(guard, 0, moved)?,
            _ => {
                for field in 0..length {
                    let copy = store.ticket(element)?;
                    store.construction_push(guard, field, copy)?;
                }
                store.discard(moved)?;
            }
        }
        let ticket = store.commit_construction(guard, array)?;
        let element_view = self.source_view(source)?;
        self.write(destination, ticket, store)?;
        if let Some(element) = element_view {
            self.set_source_view(
                destination,
                Some(crate::traits::term::TyTerm::Array(
                    Box::new(element),
                    crate::traits::term::ConstTerm::Value(u64::from(length)),
                )),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        ir::{op::ARRAY_REPEAT_OPCODE, pc::Phase, recipe::VerifiedRecipe},
        runtime::place::{Place, Projection},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{constructors::ConstructorKind, parse::ParsedFile},
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/repeated-vector-syntax.rs"
    );

    fn context() -> (Frame, GuestStore, VerifiedRecipe, SlotId, SlotId) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition: 2015,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let cfg = CfgSet::guest_default();
        let outline = parsed
            .outline(Digest::of_bytes(b"official-repeat"), vec![], &cfg)
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let plan = body.nodes().iter().find_map(|node| {
            let plan = parsed.constructor_plan(handle, &body, node.id, &cfg).ok()??;
            matches!(plan.kind(), ConstructorKind::Repeat { length, .. } if length.iter().any(|t| t.token.text == "512")).then_some(plan)
        }).unwrap();
        let ConstructorKind::Repeat { element, .. } = plan.kind() else {
            unreachable!()
        };
        let node = &body.nodes()[plan.node().0 as usize];
        let count_operand = node
            .operands
            .iter()
            .position(|n| *n != element.node)
            .unwrap();
        let pc = body
            .pc(
                node.id,
                Phase::Operand(u16::try_from(count_operand).unwrap()),
            )
            .unwrap();
        let recipe = parsed.constructor_thunk(handle, &body, &pc, &plan).unwrap();
        let [
            crate::ir::op::Op::ArrayRepeat {
                source,
                count,
                destination,
            },
        ] = recipe.recipe().operations.as_ref()
        else {
            panic!("original repeat operation absent")
        };
        let (source, count, destination) = (*source, *count, *destination);
        let scope = node.scope;
        let mut store = GuestStore::for_epoch(1).unwrap();
        let mut frame = Frame::new(body, pc, &mut store).unwrap();
        let mut scopes = vec![];
        let mut next = Some(scope);
        while let Some(id) = next {
            scopes.push(id);
            next = frame.body.scopes()[id.0 as usize].parent;
        }
        for id in scopes.into_iter().rev() {
            frame.enter(id).unwrap();
        }
        let element = store
            .intern_type(TypeKey::Array {
                element: TypeId(2),
                len: 1,
            })
            .unwrap();
        let guard = store.begin_construction(&[TypeId(2)]).unwrap();
        let value = store.ticket(ValueRepr::boolean(true)).unwrap();
        store.construction_push(guard, 0, value).unwrap();
        let value = store.commit_construction(guard, element).unwrap();
        frame.write(source, value, &mut store).unwrap();
        let count_value = store
            .ticket(ValueRepr::unsigned(TypeId(13), 512).unwrap())
            .unwrap();
        frame.write(count, count_value, &mut store).unwrap();
        (frame, store, recipe, source, destination)
    }

    #[test]
    fn official_nested_repeat_resumes_one_opcode_and_owns_distinct_elements() {
        let (mut frame, mut store, recipe, source, destination) = context();
        let source_cell = frame.cell(source).unwrap();
        let pc = frame.pc.clone();
        assert!(matches!(
            crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap(),
            crate::vm::step::StepOutcome::Dispatch
        ));
        assert_eq!(frame.pc, pc);
        assert_eq!(frame.counters.operations_started, 1);
        assert_eq!(frame.counters.operations_completed, 0);
        assert!(frame.slot(destination).unwrap().cell.is_none());
        loop {
            match crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap() {
                crate::vm::step::StepOutcome::Dispatch => {}
                crate::vm::step::StepOutcome::Next(_) => break,
                _ => panic!("repeat returned an unrelated transition"),
            }
        }
        assert_eq!(frame.counters.operations_started, 1);
        assert_eq!(frame.counters.operations_completed, 1);
        assert_eq!(
            frame.counters.started_by_opcode[usize::from(ARRAY_REPEAT_OPCODE)],
            1
        );
        assert_eq!(store.state(source_cell).unwrap(), CellState::Moved);
        let destination = frame.cell(destination).unwrap();
        let mut objects = std::collections::BTreeSet::new();
        let mut fields = std::collections::BTreeSet::new();
        for n in 0..512 {
            let array = Place::cell(destination)
                .projected(Projection::Index(n))
                .unwrap();
            let cell = store.admit_owning_place_read(&array).unwrap().0;
            let CellState::Live(value) = store.state(cell).unwrap() else {
                panic!("repeated element absent")
            };
            assert!(objects.insert(value.payload));
            let place = array.projected(Projection::Index(0)).unwrap();
            let cell = store.admit_owning_place_read(&place).unwrap().0;
            assert!(fields.insert(cell));
            assert_eq!(store.read_scalar(cell).unwrap(), ValueRepr::boolean(true));
        }
    }

    #[test]
    fn official_repeat_changed_source_epoch_refuses_without_repeating_or_publishing() {
        let (mut frame, mut store, recipe, source, destination) = context();
        assert!(matches!(
            crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap(),
            crate::vm::step::StepOutcome::Dispatch
        ));
        let source = frame.cell(source).unwrap();
        let value = store.move_from(source).unwrap();
        store.initialize(source, value).unwrap();
        let guard = frame.body.nodes()[frame.pc.node as usize]
            .constructor_guard
            .unwrap();
        let owned = frame
            .constructor_slot(guard)
            .unwrap()
            .construction
            .as_ref()
            .unwrap()
            .guard;
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(frame.counters.operations_started, 1);
        assert_eq!(frame.counters.operations_completed, 0);
        assert!(frame.slot(destination).unwrap().cell.is_none());
        assert_eq!(
            frame
                .constructor_slot(guard)
                .unwrap()
                .construction
                .as_ref()
                .unwrap()
                .guard,
            owned
        );
        assert!(frame.has_copy_activation());
    }
}
