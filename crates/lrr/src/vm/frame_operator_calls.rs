//! Overloaded operators, frame side. rustc lowers an operator whose operands
//! are not builtin primitive scalars to a call of its lang-item trait method:
//! `a == b` is `PartialEq::eq(&a, &b)` (`!=`, `<`, `<=`, `>`, `>=` likewise
//! `ne`/`lt`/`le`/`gt`/`ge`, with autoref'd operands that are never moved),
//! `a + b` is `Add::add(a, b)` (operands moved/copied by value), `-a` is
//! `Neg::neg(a)`, `!a` is `Not::not(a)` and `a += b` is
//! `AddAssign::add_assign(&mut a, b)`.
//!
//! The same `NumericBinary`/`NumericUnary`/`CompoundAssignPlace` operation
//! serves both: builtin operands keep the closed numeric route; any other
//! operand suspends the operation here (an operator activation, resumable
//! like aggregate-copy and cleanup activations) and demands its method. The
//! dispatcher selects that method from the left operand's reached type
//! through the run's member index and enters it as an ordinary source call
//! (`engine/run_operator_calls.rs`); its result resumes the operation.
use super::*;
use crate::{
    ir::op::{BorrowAccess, NumericBinary, NumericUnary, Op, OperandRead},
    runtime::{
        place::{Place, PlaceFailure, PlaceRoot},
        reference::Access,
        shape::TypeKey,
    },
    traits::term::{RegionTerm, TyTerm},
};

/// The operator an operation dispatches to its trait method.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OperatorKind {
    Binary(NumericBinary),
    Unary(NumericUnary),
    /// `place op= right`.
    Compound(NumericBinary),
    /// A `for` loop's `IntoIterator::into_iter` / `Iterator::next`.
    Iterator(crate::ir::op::IteratorCall),
}
impl OperatorKind {
    /// The lang-item trait (`core::<module>::<Trait>`) and its method.
    pub(crate) fn lang_item(self) -> (&'static str, &'static str, &'static str) {
        use NumericBinary as B;
        match self {
            Self::Binary(operator) => match operator {
                B::Eq => ("cmp", "PartialEq", "eq"),
                B::Ne => ("cmp", "PartialEq", "ne"),
                B::Lt => ("cmp", "PartialOrd", "lt"),
                B::Le => ("cmp", "PartialOrd", "le"),
                B::Gt => ("cmp", "PartialOrd", "gt"),
                B::Ge => ("cmp", "PartialOrd", "ge"),
                B::Add => ("ops", "Add", "add"),
                B::Sub => ("ops", "Sub", "sub"),
                B::Mul => ("ops", "Mul", "mul"),
                B::Div => ("ops", "Div", "div"),
                B::Rem => ("ops", "Rem", "rem"),
                B::Shl => ("ops", "Shl", "shl"),
                B::Shr => ("ops", "Shr", "shr"),
                B::BitAnd => ("ops", "BitAnd", "bitand"),
                B::BitOr => ("ops", "BitOr", "bitor"),
                B::BitXor => ("ops", "BitXor", "bitxor"),
            },
            Self::Unary(NumericUnary::Neg) => ("ops", "Neg", "neg"),
            Self::Unary(NumericUnary::Not) => ("ops", "Not", "not"),
            Self::Compound(operator) => match operator {
                B::Add => ("ops", "AddAssign", "add_assign"),
                B::Sub => ("ops", "SubAssign", "sub_assign"),
                B::Mul => ("ops", "MulAssign", "mul_assign"),
                B::Div => ("ops", "DivAssign", "div_assign"),
                B::Rem => ("ops", "RemAssign", "rem_assign"),
                B::Shl => ("ops", "ShlAssign", "shl_assign"),
                B::Shr => ("ops", "ShrAssign", "shr_assign"),
                B::BitAnd => ("ops", "BitAndAssign", "bitand_assign"),
                B::BitOr => ("ops", "BitOrAssign", "bitor_assign"),
                B::BitXor => ("ops", "BitXorAssign", "bitxor_assign"),
                // Not a compound operator; lowering never produces it.
                B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge => ("ops", "", ""),
            },
            Self::Iterator(crate::ir::op::IteratorCall::IntoIter) => {
                ("iter", "IntoIterator", "into_iter")
            }
            Self::Iterator(crate::ir::op::IteratorCall::Next) => ("iter", "Iterator", "next"),
        }
    }
    /// Stable key of the operator (selection cache identity).
    pub(crate) fn key(self) -> u32 {
        match self {
            Self::Binary(operator) => operator.tag(),
            Self::Unary(operator) => 32 + operator.tag(),
            Self::Compound(operator) => 64 + operator.tag(),
            Self::Iterator(call) => 96 + call.tag(),
        }
    }
    fn comparison(self) -> bool {
        use NumericBinary as B;
        matches!(
            self,
            Self::Binary(B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge)
        )
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Stage {
    Demanded,
    Entered,
    Returned,
}
/// One argument of a demanded operator method, in parameter order.
#[derive(Debug, Clone)]
pub(crate) struct OperatorArgument {
    pub(crate) cell: CellId,
    pub(crate) source: TyTerm,
    pub(crate) descriptor: TypeId,
    /// The operand slot holding this argument, when it is one (a formed
    /// reference lives in a dispatcher-owned location instead).
    pub(crate) slot: Option<SlotId>,
}
/// A suspended operator operation awaiting its trait method's result.
pub(super) struct OperatorActivation {
    pc: crate::core::digest::Digest,
    operation_index: u32,
    expected: Op,
    kind: OperatorKind,
    /// The type whose impl provides the method, fixed at suspension.
    self_type: TypeId,
    destination: SlotId,
    stage: Stage,
    /// Formed once (a re-polled preparation reuses them).
    arguments: Option<Vec<OperatorArgument>>,
    /// Dispatcher-owned reference locations, retired after the call.
    owned: Vec<CellId>,
}
/// The dispatcher's view of a demanded operator method.
pub(crate) struct OperatorDemand {
    pub(crate) kind: OperatorKind,
    /// The type whose impl provides the method: the left operand's (for a
    /// comparison, the autoref'd operand's) or the updated place's.
    pub(crate) self_type: TypeId,
    /// The operation's result slot.
    pub(crate) destination: SlotId,
}
/// A builtin operand type: a sized primitive scalar or a shared reference
/// (Copy, read by value as before). Anything else selects a trait method.
fn builtin_operand(store: &GuestStore, ty: TypeId) -> Result<bool, Diagnostic> {
    Ok(match store.type_descriptor(ty)?.key.as_ref() {
        TypeKey::Primitive(_) => ty.0 != 16,
        TypeKey::Ref { mutable: false, .. } => true,
        _ => false,
    })
}
fn place_error(error: PlaceFailure) -> Diagnostic {
    match error {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => invalid("operator operand place reported a bounds panic"),
    }
}
impl Frame {
    /// Whether a comparison's place operand is a builtin value.
    fn comparison_operand_builtin(
        &self,
        place: SlotId,
        store: &GuestStore,
    ) -> Result<bool, PlaceFailure> {
        let executed = self.executed_place(place)?;
        // An unresolved numeric literal local is a builtin scalar operand.
        if let (PlaceRoot::Cell(cell), true) = (executed.root, executed.projections.is_empty())
            && let CellState::Live(value) = store.state(cell)?
            && matches!(value.kind, 11 | 12)
        {
            return Ok(true);
        }
        Ok(builtin_operand(store, store.place_type_id(executed)?)?)
    }
    pub(crate) fn preflight_comparison_operand(
        &self,
        place: SlotId,
        read: OperandRead,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), PlaceFailure> {
        if !self.comparison_operand_builtin(place, store)? {
            return self.preflight_borrow(place, BorrowAccess::Shared, destination, store);
        }
        match read {
            OperandRead::Local(binding) => {
                self.preflight_read_local(binding, destination, store)?;
            }
            OperandRead::Parameter(ordinal) => {
                self.preflight_parameter(ordinal, destination, store)?;
            }
            OperandRead::Place => self.preflight_read_place(place, destination, store)?,
        }
        Ok(())
    }
    /// `Op::ComparisonOperand`: read a builtin operand exactly as its value
    /// expression does, or autoref any other operand in place.
    pub(crate) fn comparison_operand(
        &mut self,
        place: SlotId,
        read: OperandRead,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), PlaceFailure> {
        self.preflight_comparison_operand(place, read, destination, store)?;
        if !self.comparison_operand_builtin(place, store)? {
            self.borrow(place, BorrowAccess::Shared, destination, store)?;
            if !self.autoref_operands.contains(&destination) {
                self.autoref_operands
                    .try_reserve(1)
                    .map_err(|_| crate::runtime::arena::resource())?;
                self.autoref_operands.push(destination);
            }
            return Ok(());
        }
        match read {
            OperandRead::Local(binding) => {
                self.clear_place(place)?;
                self.read_local(binding, destination, store)?;
            }
            OperandRead::Parameter(ordinal) => {
                self.clear_place(place)?;
                self.read_parameter(ordinal, destination, store)?;
            }
            OperandRead::Place => self.read_place(place, destination, store)?,
        }
        Ok(())
    }
    /// A value-slot operand that is not a builtin primitive operand. A
    /// reference is never a builtin comparison operand (`&a == &b` is
    /// core's `&A: PartialEq<&B>`); arithmetic keeps the primitive
    /// forwarding route for references to sized primitives (`&i32 + i32`).
    fn overloaded_operand(
        &self,
        slot: SlotId,
        comparison: bool,
        store: &GuestStore,
    ) -> Result<bool, Diagnostic> {
        if self.autoref_operands.contains(&slot) {
            return Ok(true);
        }
        let CellState::Live(value) = store.state(self.cell(slot)?)? else {
            return Err(invalid("operator operand not live"));
        };
        if (1..=7).contains(&value.kind) || matches!(value.kind, 11 | 12) {
            return Ok(false);
        }
        if value.kind == 9
            && !comparison
            && let TypeKey::Ref { target, .. } =
                store.type_descriptor(TypeId(value.type_id))?.key.as_ref()
            && target.0 != 16
            && matches!(
                store.type_descriptor(*target)?.key.as_ref(),
                TypeKey::Primitive(_)
            )
        {
            return Ok(false);
        }
        Ok(true)
    }
    /// The trait-method operator a reached operation dispatches to, when its
    /// operands are not builtin primitives.
    pub(crate) fn overloaded_operator(
        &self,
        op: &Op,
        store: &GuestStore,
    ) -> Result<Option<OperatorKind>, Diagnostic> {
        Ok(match op {
            Op::NumericBinary {
                operator,
                left,
                right,
                ..
            } => {
                let comparison = OperatorKind::Binary(*operator).comparison();
                (self.overloaded_operand(*left, comparison, store)?
                    || self.overloaded_operand(*right, comparison, store)?)
                .then_some(OperatorKind::Binary(*operator))
            }
            Op::NumericUnary {
                operator, source, ..
            } => self
                .overloaded_operand(*source, false, store)?
                .then_some(OperatorKind::Unary(*operator)),
            Op::CompoundAssignPlace { operator, place, .. } => {
                let ty = store
                    .place_type_id(self.executed_place(*place)?)
                    .map_err(place_error)?;
                (!matches!(
                    store.type_descriptor(ty)?.key.as_ref(),
                    TypeKey::Primitive(_)
                ) || ty.0 == 16)
                    .then_some(OperatorKind::Compound(*operator))
            }
            Op::IteratorCall { call, .. } => Some(OperatorKind::Iterator(*call)),
            _ => None,
        })
    }
    /// Suspend operation `index` (`op`) for its trait method.
    pub(crate) fn begin_operator_call(
        &mut self,
        index: usize,
        op: &Op,
        kind: OperatorKind,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.operator_activation.is_some()
            || self.copy_activation.is_some()
            || self.cleanup_activation.is_some()
        {
            return Err(invalid("operator call overlaps another activation"));
        }
        let destination = match op {
            Op::NumericBinary { destination, .. }
            | Op::NumericUnary { destination, .. }
            | Op::CompoundAssignPlace { destination, .. }
            | Op::IteratorCall { destination, .. } => *destination,
            _ => return Err(invalid("operator call requires an operator operation")),
        };
        let self_type = self.operator_self_type(op, kind, store)?;
        self.operator_activation = Some(OperatorActivation {
            pc: self.pc.fingerprint(),
            operation_index: u32::try_from(index)
                .map_err(|_| invalid("operator operation index overflow"))?,
            expected: op.clone(),
            kind,
            self_type,
            destination,
            stage: Stage::Demanded,
            arguments: None,
            owned: Vec::new(),
        });
        Ok(())
    }
    pub(crate) fn has_operator_activation(&self) -> bool {
        self.operator_activation.is_some()
    }
    pub(super) fn operator_operation_index(&self) -> Option<usize> {
        self.operator_activation
            .as_ref()
            .map(|activation| activation.operation_index as usize)
    }
    /// The resumed operation after its method returned: the operator's
    /// result is already in its slot, so the operation completes.
    pub(crate) fn complete_operator_operation(
        &mut self,
        index: usize,
        op: &Op,
    ) -> Result<(), Diagnostic> {
        let activation = self
            .operator_activation
            .as_ref()
            .ok_or_else(|| invalid("operator activation absent"))?;
        if activation.pc != self.pc.fingerprint()
            || activation.operation_index as usize != index
            || activation.expected != *op
        {
            return Err(invalid("operator call resumed at a different operation"));
        }
        if activation.stage != Stage::Returned {
            return Err(invalid("operator operation resumed before its method returned"));
        }
        self.operator_activation = None;
        Ok(())
    }
    /// The type whose impl provides `op`'s method: the left operand's (a
    /// comparison's autoref'd operand's referent) or the updated place's.
    fn operator_self_type(
        &self,
        op: &Op,
        kind: OperatorKind,
        store: &GuestStore,
    ) -> Result<TypeId, Diagnostic> {
        Ok(match op {
            Op::NumericBinary { left: source, .. } | Op::NumericUnary { source, .. } => {
                let CellState::Live(value) = store.state(self.cell(*source)?)? else {
                    return Err(invalid("operator operand not live"));
                };
                // An unconstrained literal self operand (`3 * p`) takes
                // rustc's integer/float fallback.
                let ty = match (store.effective_type(value)?, value.kind) {
                    (Some(ty), _) => ty,
                    (None, 11) => TypeId(6),
                    (None, 12) => TypeId(15),
                    (None, _) => {
                        return Err(Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "operator operand requires its reached concrete type".into(),
                        ));
                    }
                };
                if kind.comparison() && self.autoref_operands.contains(source) {
                    match store.type_descriptor(ty)?.key.as_ref() {
                        TypeKey::Ref { target, .. } => *target,
                        _ => return Err(invalid("autoref'd operand is not a reference")),
                    }
                } else {
                    ty
                }
            }
            Op::CompoundAssignPlace { place, .. } => store
                .place_type_id(self.executed_place(*place)?)
                .map_err(place_error)?,
            // The evaluated `for` head's type, or the loop's iterator's.
            Op::IteratorCall { source, .. } => {
                let CellState::Live(value) = store.state(self.cell(*source)?)? else {
                    return Err(invalid("iterator call operand not live"));
                };
                store.effective_type(value)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "`for` operand requires its reached concrete type".into(),
                    )
                })?
            }
            _ => return Err(invalid("operator activation lost its operation")),
        })
    }
    /// The demanded method, until the dispatcher enters it.
    pub(crate) fn operator_demand(&self) -> Option<OperatorDemand> {
        self.operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
            .map(|activation| OperatorDemand {
                kind: activation.kind,
                self_type: activation.self_type,
                destination: activation.destination,
            })
    }
    pub(crate) fn operator_demanded(&self) -> bool {
        self.operator_activation
            .as_ref()
            .is_some_and(|activation| activation.stage == Stage::Demanded)
    }
    /// Reference source of a location this operation forms: its pointee's
    /// view, under this operator node's local region.
    fn formed_reference_source(&self, mutable: bool, pointee: TyTerm) -> Result<TyTerm, Diagnostic> {
        let range = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("operator node absent"))?
            .range;
        Ok(TyTerm::Ref(
            RegionTerm::Local {
                definition: self.body.body().definition,
                start: range.start,
                end: range.end,
            },
            mutable,
            Box::new(pointee),
        ))
    }
    fn value_view(&self, slot: SlotId, ty: TypeId, store: &GuestStore) -> Result<TyTerm, Diagnostic> {
        match self.source_view(slot)? {
            Some(view) => Ok(view),
            None => store.descriptor_source_term(ty).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "operator operand requires its closed source type".into(),
                )
            }),
        }
    }
    /// An operand slot passed by value (moved or copied by the packet).
    fn value_argument(
        &mut self,
        slot: SlotId,
        expected: Option<TypeId>,
        store: &mut GuestStore,
    ) -> Result<OperatorArgument, Diagnostic> {
        let cell = self.cell(slot)?;
        let value = store.admit_move(cell)?;
        // An unresolved literal operand takes its parameter's type, as an
        // ordinary call argument does.
        if matches!(value.kind, 11 | 12) && store.effective_type(value)?.is_none() {
            // Without a closed parameter type, rustc's integer/float fallback.
            let expected = expected.unwrap_or(if value.kind == 11 {
                TypeId(6)
            } else {
                TypeId(15)
            });
            let ticket = store.move_from(cell)?;
            let concrete = store.materialize_ticket(ticket, expected)?;
            store.discard(ticket)?;
            store.initialize(cell, concrete)?;
        }
        let value = store.admit_move(cell)?;
        let descriptor = store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "operator operand requires its reached concrete type".into(),
            )
        })?;
        Ok(OperatorArgument {
            cell,
            source: self.value_view(slot, descriptor, store)?,
            descriptor,
            slot: Some(slot),
        })
    }
    /// A fresh dispatcher-owned location holding a reference to `place`.
    fn formed_reference(
        &mut self,
        place: &Place,
        access: Access,
        pointee: TyTerm,
        ordinal: u32,
        store: &mut GuestStore,
    ) -> Result<OperatorArgument, Diagnostic> {
        let reference = store.borrow_ticket(place, access).map_err(place_error)?;
        let descriptor = TypeId(store.peek_ticket(reference)?.type_id);
        let cell = store.new_cell(
            StorageContract::Exact(descriptor),
            false,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot: self
                    .owned_location_base()?
                    .checked_add(2 + ordinal)
                    .ok_or_else(|| invalid("operator location overflow"))?,
            },
        )?;
        store.initialize(cell, reference)?;
        self.operator_activation
            .as_mut()
            .ok_or_else(|| invalid("operator activation absent"))?
            .owned
            .push(cell);
        Ok(OperatorArgument {
            cell,
            source: self.formed_reference_source(access == Access::Mutable, pointee)?,
            descriptor,
            slot: None,
        })
    }
    /// A comparison operand passed as `&T`: an autoref'd place operand
    /// already holds it; a value operand's temporary is borrowed in place.
    fn reference_argument(
        &mut self,
        slot: SlotId,
        ordinal: u32,
        store: &mut GuestStore,
    ) -> Result<OperatorArgument, Diagnostic> {
        let cell = self.cell(slot)?;
        let value = store.admit_move(cell)?;
        let ty = store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "comparison operand requires its reached concrete type".into(),
            )
        })?;
        if let Some(position) = self.autoref_operands.iter().position(|s| *s == slot) {
            self.autoref_operands.swap_remove(position);
            return Ok(OperatorArgument {
                cell,
                source: self.value_view(slot, ty, store)?,
                descriptor: ty,
                slot: Some(slot),
            });
        }
        let pointee = self.value_view(slot, ty, store)?;
        self.formed_reference(&Place::cell(cell), Access::Shared, pointee, ordinal, store)
    }
    /// Form the demanded method's arguments in parameter order (once; a
    /// re-polled preparation reuses them). `expected` are the parameters'
    /// closed descriptors, typing unresolved literal operands.
    pub(crate) fn form_operator_arguments(
        &mut self,
        expected: &[Option<TypeId>],
        store: &mut GuestStore,
    ) -> Result<Vec<OperatorArgument>, Diagnostic> {
        self.require_store(store)?;
        let activation = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
            .ok_or_else(|| invalid("operator arguments require a demanded method"))?;
        if let Some(arguments) = &activation.arguments {
            return Ok(arguments.clone());
        }
        let (kind, op) = (activation.kind, activation.expected.clone());
        let expected = |ordinal: usize| expected.get(ordinal).copied().flatten();
        let arguments = match op {
            Op::NumericBinary { left, right, .. } if kind.comparison() => vec![
                self.reference_argument(left, 0, store)?,
                self.reference_argument(right, 1, store)?,
            ],
            Op::NumericBinary { left, right, .. } => vec![
                self.value_argument(left, expected(0), store)?,
                self.value_argument(right, expected(1), store)?,
            ],
            Op::NumericUnary { source, .. } => vec![self.value_argument(source, expected(0), store)?],
            Op::CompoundAssignPlace { place, right, .. } => {
                let executed = {
                    let executed = self.executed_place(place)?;
                    Place {
                        root: executed.root,
                        projections: executed.projections.clone(),
                    }
                };
                let ty = store.place_type_id(&executed).map_err(place_error)?;
                let pointee = match self.place_slot(place)?.source.clone() {
                    Some(view) => view,
                    None => store.descriptor_source_term(ty).ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "compound assignment place requires its closed source type".into(),
                        )
                    })?,
                };
                let receiver = self.formed_reference(&executed, Access::Mutable, pointee, 0, store)?;
                self.clear_place(place)?;
                vec![receiver, self.value_argument(right, expected(1), store)?]
            }
            Op::IteratorCall {
                call: crate::ir::op::IteratorCall::IntoIter,
                source,
                ..
            } => vec![self.value_argument(source, expected(0), store)?],
            // `Iterator::next(&mut iter)`: the loop-owned iterator in place.
            Op::IteratorCall {
                call: crate::ir::op::IteratorCall::Next,
                source,
                ..
            } => {
                let cell = self.cell(source)?;
                let ty = store.effective_type(store.admit_move(cell)?)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "`for` iterator requires its reached concrete type".into(),
                    )
                })?;
                let pointee = self.value_view(source, ty, store)?;
                vec![self.formed_reference(&Place::cell(cell), Access::Mutable, pointee, 0, store)?]
            }
            _ => return Err(invalid("operator activation lost its operation")),
        };
        self.operator_activation
            .as_mut()
            .ok_or_else(|| invalid("operator activation absent"))?
            .arguments = Some(arguments.clone());
        Ok(arguments)
    }
    /// Enter the selected method: the callee frame shares this run's step and
    /// action observation, like any callee entered from this frame.
    pub(crate) fn enter_operator_call(
        &mut self,
        body: Arc<BodyIndex>,
        entry: PcKey,
        store: &mut GuestStore,
    ) -> Result<Self, Diagnostic> {
        self.require_store(store)?;
        let activation = self
            .operator_activation
            .as_ref()
            .ok_or_else(|| invalid("operator activation absent"))?;
        if activation.stage != Stage::Demanded || activation.arguments.is_none() {
            return Err(invalid("operator call entry requires its formed arguments"));
        }
        let mut frame = Self::new(body, entry, store)?;
        frame.fuel = self.fuel.share();
        frame.actions = self.actions.share();
        self.operator_activation
            .as_mut()
            .ok_or_else(|| invalid("operator activation absent"))?
            .stage = Stage::Entered;
        Ok(frame)
    }
    /// Check the entered operator's result slot before the callee's return
    /// cleanup; this consumes nothing.
    /// The destination of the operation whose method was entered.
    pub(crate) fn operator_result_slot(&self) -> Option<SlotId> {
        self.operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Entered)
            .map(|activation| activation.destination)
    }
    pub(crate) fn preflight_operator_result(
        &self,
        ticket: TicketId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let activation = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Entered)
            .ok_or_else(|| invalid("operator result requires an entered method"))?;
        self.preflight_write_value(activation.destination, store.peek_ticket(ticket)?, store)
    }
    /// The method returned: publish its result as the operation's and
    /// retire the formed argument locations.
    pub(crate) fn finish_operator_call(
        &mut self,
        ticket: TicketId,
        source: Option<TyTerm>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_operator_result(ticket, store)?;
        let destination = self
            .operator_activation
            .as_ref()
            .ok_or_else(|| invalid("operator activation absent"))?
            .destination;
        self.write(destination, ticket, store)?;
        self.set_source_view(destination, source)?;
        let activation = self
            .operator_activation
            .as_mut()
            .ok_or_else(|| invalid("operator activation absent"))?;
        activation.stage = Stage::Returned;
        activation.arguments = None;
        for cell in std::mem::take(&mut activation.owned) {
            store.retire_cell(cell)?;
        }
        Ok(())
    }
    /// The method unwound: this operation becomes the panicking operation.
    pub(crate) fn unwind_operator_call(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        let activation = self
            .operator_activation
            .take()
            .filter(|activation| activation.stage == Stage::Entered)
            .ok_or_else(|| invalid("operator unwind requires an entered method"))?;
        for cell in activation.owned {
            store.retire_cell(cell)?;
        }
        self.status = FrameStatus::Panicked;
        Ok(())
    }
}
