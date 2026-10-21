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
use super::deref_coercion_frame::{CoercionTarget, ValueCoercionGoal, ValueRead};
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
    /// A method receiver's overloaded autoderef step: `Deref::deref`, or
    /// `DerefMut::deref_mut` when `true`.
    Deref(bool),
    /// An index expression whose base takes its own `Index<Idx>` impl:
    /// `Index::index`, or `IndexMut::index_mut` when `true`.
    Index(bool),
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
            Self::Deref(false) => ("ops", "Deref", "deref"),
            Self::Deref(true) => ("ops", "DerefMut", "deref_mut"),
            Self::Index(false) => ("ops", "Index", "index"),
            Self::Index(true) => ("ops", "IndexMut", "index_mut"),
        }
    }
    /// Stable key of the operator (selection cache identity).
    pub(crate) fn key(self) -> u32 {
        match self {
            Self::Binary(operator) => operator.tag(),
            Self::Unary(operator) => 32 + operator.tag(),
            Self::Compound(operator) => 64 + operator.tag(),
            Self::Iterator(call) => 96 + call.tag(),
            Self::Deref(mutable) => 128 + u32::from(mutable),
            Self::Index(mutable) => 130 + u32::from(mutable),
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
    /// A method receiver forming itself through overloaded `Deref` steps.
    receiver: Option<ReceiverProgress>,
    /// A place operation whose base place takes an overloaded `Deref` step
    /// first: the base place slot.
    place_deref: Option<SlotId>,
    /// The dispatcher-owned location the step's returned `&Target` lands in
    /// (no frame slot carries it: its expectations belong to the operation's
    /// own later result).
    result_cell: Option<CellId>,
    /// The returned reference's source view.
    returned_view: Option<TyTerm>,
    /// A reference read taking rustc's deref coercion step by step.
    value_coercion: Option<ValueCoercion>,
}
/// A reference value read into a destination that expects a reference to
/// another Type: the referent place of the current reference, its nominal
/// Type, the access the destination expects and the Type it expects. Each
/// `Deref` call returns the next reference, which is the read's result once
/// its pointee is the expected Type.
struct ValueCoercion {
    current: Place,
    referent_type: TypeId,
    mutable: bool,
    expected: CoercionTarget,
}
impl ValueCoercion {
    fn duplicate(&self) -> ValueCoercion {
        ValueCoercion {
            current: Place {
                root: self.current.root,
                projections: self.current.projections.clone(),
            },
            referent_type: self.referent_type,
            mutable: self.mutable,
            expected: self.expected.clone(),
        }
    }
}
/// The dispatcher's finding that the next place operation's base place needs
/// an overloaded `Deref` step (`DerefMut` when `mutable`) before the
/// operation itself: `*rc`, `rc.field`, `rc.0`, `guard[i]`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PlaceDerefDemand {
    pub(crate) base: SlotId,
    pub(crate) mutable: bool,
    /// The base takes its own `Index<Idx>` impl (`base[idx]`), not a `Deref`
    /// step: the `index` call's returned reference is the element place.
    pub(crate) index: bool,
    /// The recipe position the demand was made at.
    pub(crate) pc: crate::core::digest::Digest,
}
/// A method receiver's autoderef steps, applied in order; an overloaded step
/// suspends the receiver operation for its `Deref` call, whose returned
/// reference continues the walk.
struct ReceiverProgress {
    /// Steps not yet applied; the next is last.
    remaining: Vec<ReceiverStep>,
    mode: ReceiverMode,
    unsize: bool,
    /// The place being dereferenced and the free place slot.
    current: SlotId,
    other: SlotId,
    /// References the walk went through, kept alive until the receiver is
    /// formed (places derived from them name their referents).
    parked: Vec<TicketId>,
    /// The destination's reached expectation (the method's receiver type):
    /// it constrains the formed receiver, not the intermediate references
    /// the walk's `Deref` calls return.
    hint: Option<TypeId>,
}
/// The dispatcher's view of a demanded operator method.
pub(crate) struct OperatorDemand {
    pub(crate) kind: OperatorKind,
    /// The type whose impl provides the method: the left operand's (for a
    /// comparison, the autoref'd operand's) or the updated place's.
    pub(crate) self_type: TypeId,
    /// The operation's result slot.
    pub(crate) destination: Option<SlotId>,
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
        // rustc types an operator on a rigid operand (`T`, `<T as Tr>::Assoc`,
        // an opaque type) as the lang-item trait call; monomorphization only
        // picks the impl. The operand's static source decides, not the primitive
        // value it happens to hold in this instantiation.
        // A reference to a rigid referent (`&'a T`) is as rigid: its operator is
        // the `&'a T: Add<..>` bound's trait call, not the primitive forwarding
        // the instantiated referent would allow.
        let mut static_source = self.source_view_ref(slot)?;
        while let Some(crate::traits::term::TyTerm::Ref(_, _, referent)) = static_source {
            static_source = Some(referent.as_ref());
        }
        if let Some(
            crate::traits::term::TyTerm::Param(_)
            | crate::traits::term::TyTerm::Projection(_)
            | crate::traits::term::TyTerm::Opaque(..),
        ) = static_source
        {
            return Ok(true);
        }
        if (1..=7).contains(&value.kind) || matches!(value.kind, 11 | 12 | 17) {
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
    /// Whether the operation's result is statically a rigid Type (`T`,
    /// `<T as Tr>::Assoc`, an opaque type). A builtin primitive operator can
    /// only produce a primitive, so a rigid result means rustc typed the
    /// operation as the lang-item trait call (`&'a T: Add<&'a T, Output = T>`),
    /// whose operands' own views carry the instantiated referent.
    fn rigid_result(&self, destination: SlotId) -> Result<bool, Diagnostic> {
        Ok(matches!(
            self.source_view_ref(destination)?,
            Some(
                crate::traits::term::TyTerm::Param(_)
                    | crate::traits::term::TyTerm::Projection(_)
                    | crate::traits::term::TyTerm::Opaque(..)
            )
        ))
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
                destination,
            } => {
                let comparison = OperatorKind::Binary(*operator).comparison();
                (self.overloaded_operand(*left, comparison, store)?
                    || self.overloaded_operand(*right, comparison, store)?
                    || (!comparison && self.rigid_result(*destination)?))
                .then_some(OperatorKind::Binary(*operator))
            }
            Op::NumericUnary {
                operator,
                source,
                destination,
            } => (self.overloaded_operand(*source, false, store)?
                || self.rigid_result(*destination)?)
            .then_some(OperatorKind::Unary(*operator)),
            Op::CompoundAssignPlace {
                operator, place, ..
            } => {
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
            Op::CalleeDeref { place } => {
                let ty = self.place_type(*place, store)?;
                match store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Callable { .. } | TypeKey::Function { .. } => None,
                    // The dispatcher found the callee takes its own `Fn*`
                    // impl: the walk ends here, the call is overloaded.
                    _ if self.declined_coercion == Some((self.pc.fingerprint(), ty)) => None,
                    _ => Some(OperatorKind::Deref(false)),
                }
            }
            // A place operation whose base takes an overloaded `Deref` step.
            Op::DerefPlace {
                reference: base, ..
            }
            | Op::ProjectField { base, .. }
            | Op::ProjectTuple { base, .. }
            | Op::ProjectIndex { base, .. } => self
                .place_deref
                .filter(|demand| demand.base == *base && demand.pc == self.pc.fingerprint())
                .map(|demand| {
                    if demand.index {
                        OperatorKind::Index(demand.mutable)
                    } else {
                        OperatorKind::Deref(demand.mutable)
                    }
                }),
            // A borrow whose result expects a reference to another Type than
            // the nominal value it borrows takes rustc's deref coercion: a
            // `Deref` step, then the borrow of its referent.
            Op::Borrow {
                place,
                access,
                destination,
            } => {
                let goal = self.borrow_coercion_goal(*place, *destination, store)?;
                match goal {
                    Some(goal)
                        if self.declined_coercion
                            != Some((self.pc.fingerprint(), goal.place_type)) =>
                    {
                        Some(OperatorKind::Deref(matches!(
                            access,
                            crate::ir::op::BorrowAccess::Mutable
                        )))
                    }
                    _ => None,
                }
            }
            // A reference read into a destination expecting a reference to
            // another Type takes the same deref coercion on its referent.
            Op::ReadLocal {
                binding,
                destination,
            } => self.value_coercion_kind(ValueRead::Local(*binding), *destination, store)?,
            Op::ReadParameter {
                ordinal,
                destination,
            } => self.value_coercion_kind(ValueRead::Parameter(*ordinal), *destination, store)?,
            Op::ReadPlace { place, destination } => {
                self.value_coercion_kind(ValueRead::Place(*place), *destination, store)?
            }
            // A receiver with an overloaded autoderef step demands its
            // `Deref` method before the receiver can be formed.
            Op::MethodReceiver { site, .. } => self
                .method_receivers
                .get(&self.call_identity(site.0))
                .and_then(|adjustment| {
                    adjustment.steps.iter().find_map(|step| match step {
                        ReceiverStep::Overloaded { mutable } => Some(OperatorKind::Deref(*mutable)),
                        ReceiverStep::Deref => None,
                    })
                }),
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
        if let Op::MethodReceiver {
            site,
            place,
            scratch,
            destination,
        } = op
        {
            return self.begin_receiver_overload(
                index,
                op,
                (site.0, *place, *scratch, *destination),
                store,
            );
        }
        let mut place_deref = None;
        let mut value_coercion = None;
        let destination = match op {
            Op::NumericBinary { destination, .. }
            | Op::NumericUnary { destination, .. }
            | Op::CompoundAssignPlace { destination, .. }
            | Op::IteratorCall { destination, .. } => *destination,
            // The step's result lands in a dispatcher-owned location; the
            // base place stands for the operation.
            Op::CalleeDeref { place: base }
            | Op::DerefPlace {
                reference: base, ..
            }
            | Op::ProjectField { base, .. }
            | Op::ProjectTuple { base, .. }
            | Op::ProjectIndex { base, .. }
            | Op::Borrow { place: base, .. } => {
                place_deref = Some(*base);
                self.place_deref = None;
                *base
            }
            // The read's result is the coerced reference, written to the
            // operation's own destination when the chain ends.
            Op::ReadLocal { destination, .. }
            | Op::ReadParameter { destination, .. }
            | Op::ReadPlace { destination, .. } => {
                let goal = self
                    .read_coercion_goal(op, store)?
                    .ok_or_else(|| invalid("reference read lost its coercion goal"))?;
                value_coercion = Some(ValueCoercion {
                    current: goal.referent,
                    referent_type: goal.referent_type,
                    mutable: goal.mutable,
                    expected: goal.expected,
                });
                *destination
            }
            _ => return Err(invalid("operator call requires an operator operation")),
        };
        let self_type = match &value_coercion {
            Some(coercion) => coercion.referent_type,
            None => self.operator_self_type(op, kind, store)?,
        };
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
            receiver: None,
            place_deref,
            result_cell: None,
            returned_view: None,
            value_coercion,
        });
        Ok(())
    }
    /// Begin forming a method receiver whose adjustment has an overloaded
    /// step: apply the builtin steps before it, then demand its `Deref`.
    fn begin_receiver_overload(
        &mut self,
        index: usize,
        op: &Op,
        (site, place, scratch, destination): (u32, SlotId, SlotId, SlotId),
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let adjustment = self
            .method_receivers
            .remove(&self.call_identity(site))
            .ok_or_else(|| invalid("method receiver adjustment was not selected"))?;
        let progress = ReceiverProgress {
            remaining: adjustment.steps.into_iter().rev().collect(),
            mode: adjustment.mode,
            unsize: adjustment.unsize,
            current: place,
            other: scratch,
            parked: Vec::new(),
            hint: self.slot(destination)?.hint,
        };
        self.slots[destination.0 as usize].hint = None;
        self.walk_receiver(index, op, destination, progress, store)?;
        Ok(())
    }
    /// Apply `progress`'s builtin steps up to the next overloaded one and
    /// suspend for it (`true`), or form the receiver when none remains
    /// (`false`).
    fn walk_receiver(
        &mut self,
        index: usize,
        op: &Op,
        destination: SlotId,
        mut progress: ReceiverProgress,
        store: &GuestStore,
    ) -> Result<bool, Diagnostic> {
        while let Some(step) = progress.remaining.pop() {
            match step {
                ReceiverStep::Deref => {
                    self.deref_place(progress.current, progress.other, store)
                        .map_err(place_error)?;
                    std::mem::swap(&mut progress.current, &mut progress.other);
                }
                ReceiverStep::Overloaded { mutable } => {
                    let self_type = store
                        .place_type_id(self.executed_place(progress.current)?)
                        .map_err(place_error)?;
                    self.operator_activation = Some(OperatorActivation {
                        pc: self.pc.fingerprint(),
                        operation_index: u32::try_from(index)
                            .map_err(|_| invalid("operator operation index overflow"))?,
                        expected: op.clone(),
                        kind: OperatorKind::Deref(mutable),
                        self_type,
                        destination,
                        stage: Stage::Demanded,
                        arguments: None,
                        owned: Vec::new(),
                        receiver: Some(progress),
                        place_deref: None,
                        result_cell: None,
                        returned_view: None,
                        value_coercion: None,
                    });
                    return Ok(true);
                }
            }
        }
        Err(invalid("receiver walk found no overloaded step"))
    }
    /// A receiver's `Deref` method returned its reference (written to the
    /// operation's destination): continue the walk from its referent. `true`
    /// when the receiver is formed, `false` when another overloaded step
    /// suspended it again.
    pub(crate) fn resume_receiver_step(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let Op::MethodReceiver { destination, .. } = op else {
            return Err(invalid("receiver resumed at a different operation"));
        };
        let destination = *destination;
        let activation = self
            .operator_activation
            .take()
            .ok_or_else(|| invalid("receiver activation absent"))?;
        if activation.pc != self.pc.fingerprint()
            || activation.operation_index as usize != index
            || activation.expected != *op
            || activation.stage != Stage::Returned
        {
            return Err(invalid("receiver resumed before its Deref method returned"));
        }
        let mut progress = activation
            .receiver
            .ok_or_else(|| invalid("receiver progress absent"))?;
        let cell = self.cell(destination)?;
        let view = self.source_view(destination)?;
        let ticket = store.move_from(cell)?;
        store.reset_moved_contract(cell)?;
        let value = store.peek_ticket(ticket)?;
        if value.kind != 9 {
            return Err(invalid("a Deref method returned a non-reference"));
        }
        let reference = crate::runtime::cell::RefId::from_bits(value.payload);
        progress.parked.push(ticket);
        if progress.remaining.is_empty() && progress.mode != ReceiverMode::Value && !progress.unsize
        {
            // `&*reference` is the reference itself.
            let ticket = progress
                .parked
                .pop()
                .ok_or_else(|| invalid("parked receiver reference absent"))?;
            self.slots[destination.0 as usize].hint = progress.hint;
            self.write(destination, ticket, store)?;
            self.set_source_view(destination, view)?;
            self.finish_receiver(progress, store)?;
            return Ok(true);
        }
        // The referent becomes the place the remaining steps walk.
        let pointee = match view {
            Some(TyTerm::Ref(_, _, pointee)) => Some(*pointee),
            _ => None,
        };
        let place = Place::deref(reference);
        let bytes = place.projection_bytes()?;
        self.projection_bytes = self
            .projection_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("place projection accounting overflow"))?;
        self.slots[progress.other.0 as usize].place = Some(place);
        self.set_source_view(progress.other, pointee)?;
        std::mem::swap(&mut progress.current, &mut progress.other);
        if progress
            .remaining
            .iter()
            .any(|step| matches!(step, ReceiverStep::Overloaded { .. }))
        {
            // Walk to the next overloaded step.
            return Ok(!self.walk_receiver(index, op, destination, progress, store)?);
        }
        while let Some(step) = progress.remaining.pop() {
            debug_assert!(step == ReceiverStep::Deref);
            self.deref_place(progress.current, progress.other, store)
                .map_err(place_error)?;
            std::mem::swap(&mut progress.current, &mut progress.other);
        }
        self.slots[destination.0 as usize].hint = progress.hint;
        self.form_receiver(
            progress.current,
            destination,
            progress.mode,
            progress.unsize,
            store,
        )
        .map_err(place_error)?;
        self.finish_receiver(progress, store)?;
        Ok(true)
    }
    /// The receiver is formed: the references its walk went through are no
    /// longer named by any place.
    fn finish_receiver(
        &mut self,
        progress: ReceiverProgress,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        for ticket in progress.parked {
            store.discard(ticket)?;
        }
        Ok(())
    }
    /// Whether an operator activation is a receiver walk resumed after its
    /// `Deref` method returned.
    pub(crate) fn receiver_walk_returned(&self) -> bool {
        self.operator_activation.as_ref().is_some_and(|activation| {
            activation.receiver.is_some() && activation.stage == Stage::Returned
        })
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
            return Err(invalid(
                "operator operation resumed before its method returned",
            ));
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
            Op::CalleeDeref { place: base }
            | Op::DerefPlace {
                reference: base, ..
            }
            | Op::ProjectField { base, .. }
            | Op::ProjectTuple { base, .. }
            | Op::ProjectIndex { base, .. }
            | Op::Borrow { place: base, .. } => store
                .place_type_id(self.executed_place(*base)?)
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
                destination: (!activation.holds_result()).then_some(activation.destination),
            })
    }
    pub(crate) fn operator_demanded(&self) -> bool {
        self.operator_activation
            .as_ref()
            .is_some_and(|activation| activation.stage == Stage::Demanded)
    }
    /// The demanded binary method's right operand (`Rhs`) slot and its
    /// reached type; `None` for unary/iterator methods and an unresolved
    /// literal operand (its type is fixed by the selected parameter).
    fn operator_right(&self, store: &GuestStore) -> Result<Option<(SlotId, TypeId)>, Diagnostic> {
        let Some(activation) = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
        else {
            return Ok(None);
        };
        let right = match activation.expected {
            Op::NumericBinary { right, .. } | Op::CompoundAssignPlace { right, .. } => right,
            // `Index<Idx>`: the index operand is the trait's argument.
            Op::ProjectIndex { index, .. } if matches!(activation.kind, OperatorKind::Index(_)) => {
                index
            }
            _ => return Ok(None),
        };
        let CellState::Live(value) = store.state(self.cell(right)?)? else {
            return Err(invalid("operator operand not live"));
        };
        Ok(store.effective_type(value)?.map(|ty| (right, ty)))
    }
    /// An unresolved numeric literal right operand: `Some(true)` for an
    /// integer literal, `Some(false)` for a float literal.
    pub(crate) fn operator_right_literal(
        &self,
        store: &GuestStore,
    ) -> Result<Option<bool>, Diagnostic> {
        let Some(activation) = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
        else {
            return Ok(None);
        };
        let right = match activation.expected {
            Op::NumericBinary { right, .. } | Op::CompoundAssignPlace { right, .. } => right,
            Op::ProjectIndex { index, .. } if matches!(activation.kind, OperatorKind::Index(_)) => {
                index
            }
            _ => return Ok(None),
        };
        let CellState::Live(value) = store.state(self.cell(right)?)? else {
            return Err(invalid("operator operand not live"));
        };
        Ok(
            (matches!(value.kind, 11 | 12) && store.effective_type(value)?.is_none())
                .then_some(value.kind == 11),
        )
    }
    /// Whether the demanded method is a comparison (its `Rhs` is passed by
    /// reference).
    pub(crate) fn operator_comparison(&self) -> bool {
        self.operator_activation
            .as_ref()
            .is_some_and(|activation| activation.kind.comparison())
    }
    /// The right operand's reached type, which (with the self type) fixes
    /// the impl selected at a site.
    pub(crate) fn operator_right_type(
        &self,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        Ok(self.operator_right(store)?.map(|(_, ty)| ty))
    }
    /// The source view of the argument the right operand becomes, exactly
    /// as `form_operator_arguments` forms it (a comparison passes `&Rhs`:
    /// an autoref'd operand already holds it, any other is borrowed), for
    /// selecting among several impls by their `Rhs`.
    pub(crate) fn operator_right_view(
        &self,
        store: &GuestStore,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        let Some((slot, ty)) = self.operator_right(store)? else {
            return Ok(None);
        };
        let comparison = self
            .operator_activation
            .as_ref()
            .is_some_and(|activation| activation.kind.comparison());
        let view = self.value_view(slot, ty, store)?;
        Ok(Some(
            if comparison && !self.autoref_operands.contains(&slot) {
                self.formed_reference_source(false, view)?
            } else {
                view
            },
        ))
    }
    /// Reference source of a location this operation forms: its pointee's
    /// view, under this operator node's local region.
    pub(super) fn formed_reference_source(
        &self,
        mutable: bool,
        pointee: TyTerm,
    ) -> Result<TyTerm, Diagnostic> {
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
    /// The operand's source view; a literal operand typed here by its
    /// parameter has the Type's own view, not the stale literal view.
    fn operand_view(
        &self,
        slot: SlotId,
        ty: TypeId,
        typed: bool,
        store: &GuestStore,
    ) -> Result<TyTerm, Diagnostic> {
        if typed && let Some(view) = store.descriptor_source_term(ty) {
            return Ok(view);
        }
        self.value_view(slot, ty, store)
    }
    fn value_view(
        &self,
        slot: SlotId,
        ty: TypeId,
        store: &GuestStore,
    ) -> Result<TyTerm, Diagnostic> {
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
    /// An unresolved literal operand takes its parameter's type, as an
    /// ordinary call argument does; without a closed parameter type it takes
    /// rustc's integer/float fallback.
    fn type_literal_operand(
        cell: CellId,
        expected: Option<TypeId>,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let value = store.admit_move(cell)?;
        if matches!(value.kind, 11 | 12) && store.effective_type(value)?.is_none() {
            let expected = expected.unwrap_or(if value.kind == 11 {
                TypeId(6)
            } else {
                TypeId(15)
            });
            let ticket = store.move_from(cell)?;
            let concrete = store.materialize_ticket(ticket, expected)?;
            store.discard(ticket)?;
            store.initialize(cell, concrete)?;
            return Ok(true);
        }
        Ok(false)
    }
    /// An operand slot passed by value (moved or copied by the packet).
    fn value_argument(
        &mut self,
        slot: SlotId,
        expected: Option<TypeId>,
        store: &mut GuestStore,
    ) -> Result<OperatorArgument, Diagnostic> {
        let cell = self.cell(slot)?;
        let typed = Self::type_literal_operand(cell, expected, store)?;
        let value = store.admit_move(cell)?;
        let descriptor = store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "operator operand requires its reached concrete type".into(),
            )
        })?;
        Ok(OperatorArgument {
            cell,
            source: self.operand_view(slot, descriptor, typed, store)?,
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
        expected: Option<TypeId>,
        store: &mut GuestStore,
    ) -> Result<OperatorArgument, Diagnostic> {
        let cell = self.cell(slot)?;
        // The parameter is `&Rhs`: an unresolved literal operand takes the
        // referent's type.
        let pointee = match expected {
            Some(reference) => match store.type_descriptor(reference)?.key.as_ref() {
                TypeKey::Ref { target, .. } => Some(*target),
                _ => None,
            },
            None => None,
        };
        let typed = Self::type_literal_operand(cell, pointee, store)?;
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
        let pointee = self.operand_view(slot, ty, typed, store)?;
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
                self.reference_argument(left, 0, expected(0), store)?,
                self.reference_argument(right, 1, expected(1), store)?,
            ],
            Op::NumericBinary { left, right, .. } => vec![
                self.value_argument(left, expected(0), store)?,
                self.value_argument(right, expected(1), store)?,
            ],
            Op::NumericUnary { source, .. } => {
                vec![self.value_argument(source, expected(0), store)?]
            }
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
                let receiver =
                    self.formed_reference(&executed, Access::Mutable, pointee, 0, store)?;
                self.clear_place(place)?;
                vec![receiver, self.value_argument(right, expected(1), store)?]
            }
            Op::IteratorCall {
                call: crate::ir::op::IteratorCall::IntoIter,
                source,
                ..
            } => vec![self.value_argument(source, expected(0), store)?],
            // `Deref::deref(&place)` / `DerefMut::deref_mut(&mut place)` of the
            // receiver walk's current place.
            Op::MethodReceiver { .. }
            | Op::CalleeDeref { .. }
            | Op::DerefPlace { .. }
            | Op::ProjectField { .. }
            | Op::ProjectTuple { .. }
            | Op::ProjectIndex { .. }
            | Op::Borrow { .. }
            | Op::ReadLocal { .. }
            | Op::ReadParameter { .. }
            | Op::ReadPlace { .. } => {
                if self
                    .operator_activation
                    .as_ref()
                    .is_some_and(|activation| activation.holds_result())
                {
                    let cell = store.new_cell(
                        StorageContract::Unbound,
                        false,
                        CellOwner::FrameSlot {
                            frame: self.anchor,
                            slot: self
                                .owned_location_base()?
                                .checked_add(8)
                                .ok_or_else(|| invalid("operator location overflow"))?,
                        },
                    )?;
                    self.operator_activation
                        .as_mut()
                        .ok_or_else(|| invalid("operator activation absent"))?
                        .result_cell = Some(cell);
                }
                let coerced = self.operator_activation.as_ref().and_then(|activation| {
                    activation
                        .value_coercion
                        .as_ref()
                        .map(ValueCoercion::duplicate)
                });
                let (current, executed, ty) = match coerced {
                    // The referent of the reference being coerced.
                    Some(coercion) => (None, coercion.current, coercion.referent_type),
                    None => {
                        let current = self
                            .operator_activation
                            .as_ref()
                            .and_then(|activation| {
                                activation
                                    .receiver
                                    .as_ref()
                                    .map(|progress| progress.current)
                                    .or(activation.place_deref)
                            })
                            .ok_or_else(|| invalid("receiver walk lost its place"))?;
                        let executed = {
                            let executed = self.executed_place(current)?;
                            Place {
                                root: executed.root,
                                projections: executed.projections.clone(),
                            }
                        };
                        let ty = store.place_type_id(&executed).map_err(place_error)?;
                        (Some(current), executed, ty)
                    }
                };
                let pointee = match current.map(|slot| self.place_slot(slot)).transpose()? {
                    Some(slot) if slot.source.is_some() => {
                        slot.source.clone().expect("checked source")
                    }
                    _ => store.descriptor_source_term(ty).ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "autoderef receiver requires its closed source type".into(),
                        )
                    })?,
                };
                let access =
                    if matches!(kind, OperatorKind::Deref(true) | OperatorKind::Index(true)) {
                        Access::Mutable
                    } else {
                        Access::Shared
                    };
                let reference = self.formed_reference(&executed, access, pointee, 0, store)?;
                if let Some(current) = current {
                    self.clear_place(current)?;
                }
                match op {
                    // `Index::index(&base, idx)`: the index operand follows.
                    Op::ProjectIndex { index, .. } if matches!(kind, OperatorKind::Index(_)) => {
                        let index = self.value_argument(index, expected(1), store)?;
                        vec![reference, index]
                    }
                    _ => vec![reference],
                }
            }
            // `Iterator::next(&mut iter)`: the loop-owned iterator in place.
            Op::IteratorCall {
                call: crate::ir::op::IteratorCall::Next,
                source,
                ..
            } => {
                let cell = self.cell(source)?;
                let ty = store
                    .effective_type(store.admit_move(cell)?)?
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "`for` iterator requires its reached concrete type".into(),
                        )
                    })?;
                let pointee = self.value_view(source, ty, store)?;
                vec![self.formed_reference(
                    &Place::cell(cell),
                    Access::Mutable,
                    pointee,
                    0,
                    store,
                )?]
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
            .filter(|activation| activation.stage == Stage::Entered && !activation.holds_result())
            .map(|activation| activation.destination)
    }
    /// The entered operator method is a `Deref` step of a place operation or
    /// a reference coercion, whose result is held by the dispatcher rather
    /// than a frame slot.
    pub(crate) fn operator_is_place_deref(&self) -> bool {
        self.operator_activation.as_ref().is_some_and(|activation| {
            activation.stage == Stage::Entered && activation.holds_result()
        })
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
        if activation.holds_result() {
            return Ok(());
        }
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
        let (destination, held) = {
            let activation = self
                .operator_activation
                .as_ref()
                .ok_or_else(|| invalid("operator activation absent"))?;
            (activation.destination, activation.result_cell)
        };
        let mut source = source;
        match held {
            // A place step's reference lands in its dispatcher-owned location.
            Some(cell) => {
                store.initialize(cell, ticket)?;
                self.operator_activation
                    .as_mut()
                    .ok_or_else(|| invalid("operator activation absent"))?
                    .returned_view = source.take();
            }
            None => {
                self.write(destination, ticket, store)?;
                self.set_source_view(destination, source)?;
            }
        }
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
    pub(crate) fn unwind_operator_call(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let activation = self
            .operator_activation
            .take()
            .filter(|activation| activation.stage == Stage::Entered)
            .ok_or_else(|| invalid("operator unwind requires an entered method"))?;
        for cell in activation.owned {
            store.retire_cell(cell)?;
        }
        if let Some(cell) = activation.result_cell {
            store.retire_cell(cell)?;
        }
        self.status = FrameStatus::Panicked;
        Ok(())
    }
}

impl Frame {
    /// The dispatcher found that `base`'s place needs an overloaded `Deref`
    /// step before the next place operation (`DerefMut` when `mutable`).
    pub(crate) fn demand_place_deref(&mut self, base: SlotId, mutable: bool) {
        self.place_deref = Some(PlaceDerefDemand {
            base,
            mutable,
            index: false,
            pc: self.pc.fingerprint(),
        });
    }
    /// The dispatcher found that the index expression's base place takes its
    /// own `Index<Idx>` impl (`IndexMut` when `mutable`).
    pub(crate) fn demand_place_index(&mut self, base: SlotId, mutable: bool) {
        self.place_deref = Some(PlaceDerefDemand {
            base,
            mutable,
            index: true,
            pc: self.pc.fingerprint(),
        });
    }
    /// The dispatcher found that the index operand about to be hinted is
    /// the operand of an overloaded `Index<Idx>`.
    /// `parameter` is the operand's closed type when one impl provides the
    /// `index` method.
    pub(crate) fn note_index_operand_overloaded(
        &mut self,
        base: SlotId,
        parameter: Option<TypeId>,
    ) {
        self.index_overload = Some((self.pc.fingerprint(), base, parameter));
    }
    /// `Some(parameter type)` when the index operand of `base` is the
    /// argument of an overloaded `Index<Idx>`.
    pub(crate) fn index_operand_overloaded(&self, base: SlotId) -> Option<Option<TypeId>> {
        self.index_overload
            .filter(|(pc, slot, _)| *pc == self.pc.fingerprint() && *slot == base)
            .map(|(_, _, parameter)| parameter)
    }
    pub(crate) fn has_place_deref_demand(&self) -> bool {
        self.place_deref.is_some()
    }
    /// The operation a dereferenced base re-enters.
    pub(crate) fn rerun_operation_index(&self) -> Option<usize> {
        self.rerun_index.map(|index| index as usize)
    }
    pub(crate) fn clear_rerun(&mut self, index: usize) {
        if self
            .rerun_index
            .is_some_and(|rerun| rerun as usize == index)
        {
            self.rerun_index = None;
        }
    }
    /// The reached type of the place `slot` holds, `None` while that place
    /// has not executed.
    pub(crate) fn place_type_if_executed(
        &self,
        slot: SlotId,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        match self.place_slot(slot) {
            Ok(place_slot) => match &place_slot.place {
                Some(place) => Ok(Some(store.place_type_id(place).map_err(place_error)?)),
                None => Ok(None),
            },
            Err(_) => Ok(None),
        }
    }
    /// Whether an operator activation is a place operation's overloaded
    /// `Deref` step whose method returned.
    pub(crate) fn place_deref_returned(&self) -> bool {
        self.operator_activation.as_ref().is_some_and(|activation| {
            activation.place_deref.is_some() && activation.stage == Stage::Returned
        })
    }
    /// The place operation's `Deref` method returned its reference (held in
    /// the node value slot): the reference becomes a temporary of the
    /// operation's scope and its referent the place the operation proceeds
    /// from. `true` when the operation itself is complete (`*x`), `false`
    /// when it must run again over the dereferenced base (`x.f`).
    pub(crate) fn resume_place_deref(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let activation = self
            .operator_activation
            .take()
            .ok_or_else(|| invalid("place dereference activation absent"))?;
        let indexed = matches!(activation.kind, OperatorKind::Index(_));
        if activation.pc != self.pc.fingerprint()
            || activation.operation_index as usize != index
            || activation.expected != *op
            || activation.stage != Stage::Returned
        {
            return Err(invalid(
                "place dereference resumed before its method returned",
            ));
        }
        let base = activation
            .place_deref
            .ok_or_else(|| invalid("place dereference base absent"))?;
        let cell = activation
            .result_cell
            .ok_or_else(|| invalid("place dereference result location absent"))?;
        let view = activation.returned_view;
        let ticket = store.move_from(cell)?;
        store.retire_cell(cell)?;
        let value = store.peek_ticket(ticket)?;
        if value.kind != 9 {
            return Err(invalid("a Deref method returned a non-reference"));
        }
        let reference = crate::runtime::cell::RefId::from_bits(value.payload);
        let scope = self.place_slot(base)?.scope;
        self.deref_parked.push((scope, ticket));
        let pointee = match view {
            Some(TyTerm::Ref(_, _, pointee)) => Some(*pointee),
            _ => None,
        };
        let place = Place::deref(reference);
        let bytes = place.projection_bytes()?;
        match op {
            Op::DerefPlace { destination, .. } | Op::ProjectIndex { destination, .. }
                if indexed || matches!(op, Op::DerefPlace { .. }) =>
            {
                self.projection_bytes = self
                    .projection_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| invalid("place projection accounting overflow"))?;
                self.set_source_view(*destination, pointee)?;
                self.slots[destination.0 as usize].place = Some(place);
                self.clear_place(base)?;
                Ok(true)
            }
            _ => {
                let old = self
                    .place_slot(base)?
                    .place
                    .as_ref()
                    .map_or(Ok(0), |old| old.projection_bytes())?;
                self.projection_bytes = self
                    .projection_bytes
                    .checked_sub(old)
                    .and_then(|total| total.checked_add(bytes))
                    .ok_or_else(|| invalid("place projection accounting underflow"))?;
                self.set_source_view(base, pointee)?;
                self.slots[base.0 as usize].place = Some(place);
                self.rerun_index = Some(
                    u32::try_from(index)
                        .map_err(|_| invalid("operator operation index overflow"))?,
                );
                Ok(false)
            }
        }
    }
    /// The references overloaded place dereferences returned end with their
    /// scope, as the temporaries they are.
    pub(crate) fn release_parked_derefs(
        &mut self,
        scope: ScopeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let mut index = self.deref_parked.len();
        while index > 0 {
            index -= 1;
            if self.deref_parked[index].0 == scope {
                let (_, ticket) = self.deref_parked.remove(index);
                store.discard(ticket)?;
            }
        }
        Ok(())
    }
}

impl Frame {
    /// Replace `base`'s place by the place its reference/box chain reaches,
    /// carrying each step's pointee view.
    pub(crate) fn peel_builtin_derefs(
        &mut self,
        base: SlotId,
        store: &GuestStore,
    ) -> Result<(), PlaceFailure> {
        loop {
            let place = self.executed_place(base)?;
            let ty = store.place_type_id(place)?;
            let is_reference =
                matches!(store.type_descriptor(ty)?.key.as_ref(), TypeKey::Ref { .. });
            if !is_reference && store.box_contents_type(ty)?.is_none() {
                return Ok(());
            }
            let dereferenced = store.dereference_place(place)?;
            let released = place.projection_bytes()?;
            let view = match self.place_slot(base)?.source.as_ref() {
                Some(TyTerm::Ref(_, _, pointee)) => self.clone_source(Some(pointee.as_ref()))?,
                Some(source) => self.clone_source(Self::box_contents_source(source, store))?,
                None => None,
            };
            self.projection_bytes = self
                .projection_bytes
                .checked_sub(released)
                .and_then(|bytes| bytes.checked_add(dereferenced.projection_bytes().ok()?))
                .ok_or_else(|| invalid("place projection accounting underflow"))?;
            self.slots[base.0 as usize].place = Some(dereferenced);
            self.set_source_view(base, view)?;
        }
    }
}

impl Frame {
    /// The deref coercion goal of a demanded (not yet entered) coercion
    /// step -- a borrow's or a reference read's -- as the Type being stepped,
    /// the closed Type expected and the access; `None` for any other
    /// operator activation.
    pub(crate) fn pending_coercion_goal(
        &self,
        store: &GuestStore,
    ) -> Result<Option<(TypeId, CoercionTarget, bool)>, Diagnostic> {
        let Some(activation) = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
        else {
            return Ok(None);
        };
        if let Some(coercion) = &activation.value_coercion {
            return Ok(Some((
                coercion.referent_type,
                coercion.expected.clone(),
                coercion.mutable,
            )));
        }
        let Op::Borrow {
            place,
            access,
            destination,
        } = &activation.expected
        else {
            return Ok(None);
        };
        Ok(self
            .borrow_coercion_goal(*place, *destination, store)?
            .map(|goal| {
                (
                    goal.place_type,
                    goal.expected,
                    matches!(access, crate::ir::op::BorrowAccess::Mutable),
                )
            }))
    }
    /// The Type of the callee place whose autoderef step is demanded (not
    /// yet entered): a value call's `CalleeDeref`; `None` for any other
    /// operator activation.
    pub(crate) fn pending_callee_deref(
        &self,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let Some(activation) = self
            .operator_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
        else {
            return Ok(None);
        };
        let Op::CalleeDeref { place } = &activation.expected else {
            return Ok(None);
        };
        self.place_type(*place, store).map(Some)
    }
    /// The dispatcher found no `Deref` chain from the borrowed Type to the
    /// Type the borrow expects (a conversion other than deref coercion):
    /// drop the demand and run the borrow as written.
    pub(crate) fn decline_coercion(&mut self, place_type: TypeId) -> Result<(), Diagnostic> {
        let activation = self
            .operator_activation
            .take()
            .filter(|activation| activation.stage == Stage::Demanded)
            .ok_or_else(|| invalid("coercion decline requires a demanded step"))?;
        self.declined_coercion = Some((activation.pc, place_type));
        self.rerun_index = Some(activation.operation_index);
        Ok(())
    }
}

impl OperatorActivation {
    /// Its method's result lands in a dispatcher-owned location, not a
    /// frame slot (a place operation's `Deref` step or a reference coercion).
    fn holds_result(&self) -> bool {
        self.place_deref.is_some() || self.value_coercion.is_some()
    }
}

impl Frame {
    /// The deref coercion a reference read calls for, if any (the read is
    /// one of `ReadLocal`/`ReadParameter`/`ReadPlace`).
    fn read_coercion_goal(
        &self,
        op: &Op,
        store: &GuestStore,
    ) -> Result<Option<ValueCoercionGoal>, Diagnostic> {
        match op {
            Op::ReadLocal {
                binding,
                destination,
            } => self.value_coercion_goal(ValueRead::Local(*binding), *destination, store),
            Op::ReadParameter {
                ordinal,
                destination,
            } => self.value_coercion_goal(ValueRead::Parameter(*ordinal), *destination, store),
            Op::ReadPlace { place, destination } => {
                self.value_coercion_goal(ValueRead::Place(*place), *destination, store)
            }
            _ => Ok(None),
        }
    }
    fn value_coercion_kind(
        &self,
        read: ValueRead,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<OperatorKind>, Diagnostic> {
        Ok(match self.value_coercion_goal(read, destination, store)? {
            Some(goal)
                if self.declined_coercion != Some((self.pc.fingerprint(), goal.referent_type)) =>
            {
                Some(OperatorKind::Deref(goal.mutable))
            }
            _ => None,
        })
    }
    /// Whether an operator activation is a reference coercion step whose
    /// method returned.
    pub(crate) fn value_coercion_returned(&self) -> bool {
        self.operator_activation.as_ref().is_some_and(|activation| {
            activation.value_coercion.is_some() && activation.stage == Stage::Returned
        })
    }
    /// A coercion step returned its reference. Once its pointee is the Type
    /// the destination expects, the reference is the read's result and the
    /// operation is complete (`true`); otherwise the next `Deref` step is
    /// demanded over its referent (`false`).
    pub(crate) fn resume_value_coercion(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let activation = self
            .operator_activation
            .take()
            .ok_or_else(|| invalid("reference coercion activation absent"))?;
        if activation.pc != self.pc.fingerprint()
            || activation.operation_index as usize != index
            || activation.expected != *op
            || activation.stage != Stage::Returned
        {
            return Err(invalid(
                "reference coercion resumed before its method returned",
            ));
        }
        let coercion = activation
            .value_coercion
            .as_ref()
            .map(ValueCoercion::duplicate)
            .ok_or_else(|| invalid("reference coercion lost its goal"))?;
        let cell = activation
            .result_cell
            .ok_or_else(|| invalid("reference coercion result location absent"))?;
        let view = activation.returned_view;
        let destination = activation.destination;
        let ticket = store.move_from(cell)?;
        store.retire_cell(cell)?;
        let value = store.peek_ticket(ticket)?;
        if value.kind != 9 {
            return Err(invalid("a Deref method returned a non-reference"));
        }
        let TypeKey::Ref { target, .. } =
            store.type_descriptor(TypeId(value.type_id))?.key.as_ref()
        else {
            return Err(invalid("a Deref method returned a non-reference"));
        };
        let target = *target;
        if coercion.expected.admits_type(target, store)? {
            // The chain ends: the reference is the read's value.
            self.preflight_write_value(destination, value, store)?;
            self.prepare_write(destination, store)?;
            self.write(destination, ticket, store)?;
            self.set_source_view(destination, view)?;
            if let Op::ReadPlace { place, .. } = op {
                self.clear_place(*place)?;
            }
            return Ok(true);
        }
        // Another step: the returned reference stays alive as a temporary of
        // the destination's scope, and its referent is stepped next.
        let scope = self.slot(destination)?.scope;
        let reference = crate::runtime::cell::RefId::from_bits(value.payload);
        self.deref_parked.push((scope, ticket));
        let self_type = target;
        self.operator_activation = Some(OperatorActivation {
            pc: activation.pc,
            operation_index: activation.operation_index,
            expected: activation.expected,
            kind: activation.kind,
            self_type,
            destination,
            stage: Stage::Demanded,
            arguments: None,
            owned: Vec::new(),
            receiver: None,
            place_deref: None,
            result_cell: None,
            returned_view: None,
            value_coercion: Some(ValueCoercion {
                current: Place::deref(reference),
                referent_type: target,
                mutable: coercion.mutable,
                expected: coercion.expected,
            }),
        });
        Ok(false)
    }
}
