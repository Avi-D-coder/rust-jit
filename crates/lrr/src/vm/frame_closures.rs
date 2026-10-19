//! Closure values and closure call frames.
//!
//! `Op::MakeClosure` forms the closure value of the closure expression at
//! the current PC: a `TypeKey::Callable` aggregate whose identity is the
//! expression (body and node) and whose fields are its captures in capture
//! order. A by-reference capture's field is an ordinary shared/unique borrow
//! of the captured binding; a by-value capture moves (or copies) the binding's
//! value into its field. A closure call frame starts at the same body index
//! with each captured binding bound to the cell its capture reaches
//! (`GuestStore::closure_capture_cell`), so the closure body's ordinary local
//! reads, places, borrows and assignments operate on the captured variables.
use super::{Frame, invalid};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    ir::{
        op::{CaptureMode, CaptureSource, ClosureCapture},
        pc::{NodeId, SlotId},
    },
    runtime::{
        cell::{CellId, GuestStore, IntrinsicRead, TicketId},
        place::{Place, PlaceFailure},
        reference::Access,
        shape::{TypeId, TypeKey},
    },
    source::manifest::BodyKey,
};
use std::sync::Arc;

/// The identity of a closure expression: its owning body and node.
pub(crate) fn closure_identity(body: &BodyKey, node: NodeId) -> Digest {
    let mut encoder = StableEncoder::new("lrr/v1/closure-expression");
    encoder.record(body);
    encoder.u32(node.0);
    encoder.finish()
}

fn place_fault(failure: PlaceFailure) -> Diagnostic {
    match failure {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => invalid("closure capture place panicked"),
    }
}

/// A capture's storage once its reached type is known.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Stored {
    Borrow(Access),
    Value,
}

impl Frame {
    /// Same actual source/application key for target registration and VM value creation.
    pub(crate) fn closure_identity(&self, node: NodeId) -> Digest {
        let source = closure_identity(self.body.body(), node);
        let Some(application) = self
            .generic_context()
            .and_then(|context| context.application_identity())
        else {
            return source;
        };
        let mut encoder = StableEncoder::new("lrr/v1/worker-closure-application");
        encoder.digest(source);
        encoder.digest(application);
        encoder.finish()
    }
    fn capture_mode(
        &self,
        capture: &ClosureCapture,
        ordinal: usize,
        count: usize,
        prepared: bool,
    ) -> Result<CaptureMode, Diagnostic> {
        if !prepared {
            return Ok(capture.mode);
        }
        match &self.prepared_capture_modes {
            Some((pc, modes)) if pc == &self.pc && modes.len() == count => Ok(modes[ordinal]),
            _ => Err(invalid(
                "closure pattern capture lacks prepared modes for this PC",
            )),
        }
    }
    pub(crate) fn set_capture_modes(&mut self, modes: Arc<[CaptureMode]>) {
        self.prepared_capture_modes = Some((self.pc.clone(), modes));
    }
    pub(crate) fn ordinary_capture_mode(
        &self,
        capture: &ClosureCapture,
        store: &GuestStore,
    ) -> Result<CaptureMode, Diagnostic> {
        let (_, stored) = self.stored_capture(capture, store, capture.mode)?;
        Ok(match stored {
            Stored::Value => CaptureMode::Value,
            Stored::Borrow(Access::Mutable) => CaptureMode::Mutable,
            Stored::Borrow(_) => CaptureMode::Shared,
        })
    }
    pub(crate) fn capture_source_type(
        &self,
        source: CaptureSource,
        store: &GuestStore,
    ) -> Result<(TypeId, crate::traits::term::TyTerm), Diagnostic> {
        let (cell, written) = match source {
            CaptureSource::Binding(binding) => {
                (self.binding(binding)?, self.make_place_source(binding))
            }
            CaptureSource::Parameter(ordinal) => (
                self.parameter(ordinal)?,
                self.parameter_sources.get(ordinal as usize),
            ),
        };
        let value = store.admit_move(cell)?;
        let ty = store
            .effective_type(value)?
            .ok_or_else(|| invalid("pattern capture root Type remains open"))?;
        let source = written
            .map(|source| self.bound_view(source, store))
            .or_else(|| store.descriptor_source_term(ty))
            .ok_or_else(|| invalid("pattern capture root source Type absent"))?;
        Ok((ty, source))
    }
    pub(crate) fn pattern_access_mode(
        &self,
        root: TypeId,
        access: &crate::frontend::pattern_access::PreparedPatternAccess,
        store: &mut GuestStore,
    ) -> Result<CaptureMode, Diagnostic> {
        use crate::frontend::pattern_access::TypeStep;
        use crate::vm::pattern::DefaultBindingMode;
        let mut result = CaptureMode::Shared;
        for binding in &access.bindings {
            let mut ty = root;
            for step in &binding.steps {
                ty = match step {
                    TypeStep::Deref => match store.type_descriptor(ty)?.key.as_ref() {
                        TypeKey::Ref { target, .. } => *target,
                        _ => return Err(invalid("capture access reference Type differs")),
                    },
                    TypeStep::Field(field) => match store.type_descriptor(ty)?.key.as_ref() {
                        TypeKey::Tuple(fields) => *fields
                            .get(*field as usize)
                            .ok_or_else(|| invalid("capture tuple field absent"))?,
                        _ => return Err(invalid("capture tuple Type differs")),
                    },
                    TypeStep::Element => match store.type_descriptor(ty)?.key.as_ref() {
                        TypeKey::Array { element, .. } | TypeKey::Slice { element } => *element,
                        _ => return Err(invalid("capture element Type differs")),
                    },
                    TypeStep::Nominal { variant, field } => {
                        let args = match store.type_descriptor(ty)?.key.as_ref() {
                            TypeKey::Nominal {
                                identity,
                                schema,
                                args,
                                ..
                            } if *identity == variant.identity() && *schema == variant.schema() => {
                                args.to_vec()
                            }
                            _ => {
                                return Err(invalid(
                                    "capture nominal access source/schema differs",
                                ));
                            }
                        };
                        variant
                            .fields()
                            .get(*field as usize)
                            .ok_or_else(|| invalid("capture nominal field absent"))?
                            .materialize(store, &args)?
                    }
                };
            }
            let mode = match binding.mode {
                DefaultBindingMode::Ref => CaptureMode::Shared,
                DefaultBindingMode::RefMut => CaptureMode::Mutable,
                DefaultBindingMode::Move => match store.intrinsic_type_read_mode(ty)? {
                    Some(IntrinsicRead::Move) => CaptureMode::Value,
                    Some(_) => CaptureMode::Shared,
                    None => {
                        return Err(Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "pattern capture binding needs its source Copy proof".into(),
                        ));
                    }
                },
            };
            result = join_capture_mode(result, mode);
        }
        Ok(result)
    }
    fn stored_capture(
        &self,
        capture: &ClosureCapture,
        store: &GuestStore,
        mode: CaptureMode,
    ) -> Result<(CellId, Stored), Diagnostic> {
        let cell = match capture.source {
            CaptureSource::Binding(binding) => self.binding(binding)?,
            CaptureSource::Parameter(ordinal) => self.parameter(ordinal)?,
        };
        let stored = match mode {
            CaptureMode::Shared => Stored::Borrow(Access::Shared),
            CaptureMode::Mutable => Stored::Borrow(Access::Mutable),
            CaptureMode::Value => Stored::Value,
            // A consuming use of a Copy value only needs to read it.
            CaptureMode::Consumed => match store.admit_owning_read(cell)? {
                IntrinsicRead::Move => Stored::Value,
                IntrinsicRead::CopyScalar | IntrinsicRead::CopyAggregate => {
                    Stored::Borrow(Access::Shared)
                }
            },
        };
        Ok((cell, stored))
    }
    pub(crate) fn preflight_make_closure(
        &self,
        captures: &[ClosureCapture],
        destination: SlotId,
        store: &GuestStore,
        prepared_modes: bool,
    ) -> Result<(), Diagnostic> {
        self.preflight_temporary_slot(destination)?;
        if !self.active_scopes.contains(&self.slot(destination)?.scope) {
            return Err(invalid("closure result scope inactive"));
        }
        for (ordinal, capture) in captures.iter().enumerate() {
            let mode = self.capture_mode(capture, ordinal, captures.len(), prepared_modes)?;
            let (cell, stored) = self.stored_capture(capture, store, mode)?;
            if let Stored::Borrow(access) = stored {
                store
                    .admit_place_borrow(&Place::cell(cell), access)
                    .map_err(place_fault)?;
            }
        }
        Ok(())
    }
    pub(crate) fn make_closure(
        &mut self,
        captures: &[ClosureCapture],
        destination: SlotId,
        store: &mut GuestStore,
        prepared_modes: bool,
    ) -> Result<(), Diagnostic> {
        self.preflight_make_closure(captures, destination, store, prepared_modes)?;
        let identity = self.closure_identity(NodeId(self.pc.node));
        let mut tickets: Vec<TicketId> = Vec::with_capacity(captures.len());
        let mut types = Vec::with_capacity(captures.len());
        let mut borrowed = Vec::with_capacity(captures.len());
        for (ordinal, capture) in captures.iter().enumerate() {
            let mode = self.capture_mode(capture, ordinal, captures.len(), prepared_modes)?;
            let (cell, stored) = self.stored_capture(capture, store, mode)?;
            let ticket = match stored {
                Stored::Borrow(access) => store
                    .borrow_ticket(&Place::cell(cell), access)
                    .map_err(place_fault)?,
                Stored::Value => {
                    let ticket = if store.admit_owning_read(cell)? == IntrinsicRead::CopyAggregate {
                        store.copy_aggregate_value(cell)?
                    } else {
                        store.read_owning(cell)?
                    };
                    let value = store.peek_ticket(ticket)?;
                    // An unresolved numeric literal takes rustc's fallback.
                    match (value.kind, store.effective_type(value)?) {
                        (11, None) => {
                            let concrete = store.materialize_ticket(ticket, TypeId(6))?;
                            store.discard(ticket)?;
                            concrete
                        }
                        (12, None) => {
                            let concrete = store.materialize_ticket(ticket, TypeId(15))?;
                            store.discard(ticket)?;
                            concrete
                        }
                        _ => ticket,
                    }
                }
            };
            let value = store.peek_ticket(ticket)?;
            let ty = store.effective_type(value)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "closure capture requires its reached concrete type".into(),
                )
            })?;
            tickets.push(ticket);
            types.push(ty);
            borrowed.push(matches!(stored, Stored::Borrow(_)));
        }
        let ty = store.intern_type(TypeKey::Callable {
            identity,
            captures: types.clone().into_boxed_slice(),
            borrowed: borrowed.into_boxed_slice(),
        })?;
        self.prepare_callable_write(destination, ty, store)?;
        let guard = store.begin_construction(&types)?;
        for (field, ticket) in tickets.into_iter().enumerate() {
            store.construction_push(
                guard,
                u32::try_from(field).map_err(|_| invalid("closure capture ordinal overflow"))?,
                ticket,
            )?;
        }
        let ticket = store.commit_construction(guard, ty)?;
        self.write(destination, ticket, store)
    }
    /// Start a closure call frame: bind each captured binding to the cell
    /// its capture reaches through the closure value at `closure`; a
    /// captured parameter of the creating frame becomes this frame's
    /// parameter `arity + ordinal`.
    pub(crate) fn bind_closure_captures(
        &mut self,
        captures: &[ClosureCapture],
        arity: usize,
        closure: &Place,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        for (field, capture) in captures.iter().enumerate() {
            let cell = store.closure_capture_cell(
                closure,
                u32::try_from(field).map_err(|_| invalid("closure capture ordinal overflow"))?,
            )?;
            match capture.source {
                CaptureSource::Binding(binding) => {
                    let slot = self
                        .bindings
                        .get_mut(binding.0 as usize)
                        .ok_or_else(|| invalid("closure capture binding absent"))?;
                    if slot.is_some() {
                        return Err(invalid("closure capture binding already bound"));
                    }
                    *slot = Some(cell);
                    self.captured.push(binding);
                    // The captured variable's view is its reached closed Type.
                    let view = store
                        .live_place_type_id(&Place::cell(cell))
                        .ok()
                        .and_then(|ty| store.descriptor_source_term(ty))
                        .filter(Self::supported_source);
                    if view.is_some() {
                        self.set_binding_source_view(binding, view)?;
                    }
                }
                CaptureSource::Parameter(ordinal) => {
                    let index = (ordinal as usize)
                        .checked_add(arity)
                        .and_then(|index| index.checked_sub(self.parameters.len()))
                        .ok_or_else(|| invalid("captured parameter precedes closure arguments"))?;
                    if self.captured_parameters.len() <= index {
                        self.captured_parameters.resize(index + 1, None);
                    }
                    if self.captured_parameters[index].is_some() {
                        return Err(invalid("closure parameter capture already bound"));
                    }
                    self.captured_parameters[index] = Some(cell);
                }
            }
        }
        Ok(())
    }
}

impl Frame {
    /// A closure parameter without a written type takes its argument's type;
    /// an argument still holding an unconstrained numeric literal takes
    /// rustc's fallback (`i32`, `f64`) before it crosses the boundary.
    pub(crate) fn materialize_fallback_argument(
        &mut self,
        slot: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.cell(slot)?;
        let value = store.admit_move(cell)?;
        if !matches!(value.kind, 11 | 12) || store.effective_type(value)?.is_some() {
            return Ok(());
        }
        let fallback = if value.kind == 11 {
            TypeId(6)
        } else {
            TypeId(15)
        };
        let ticket = store.move_from(cell)?;
        let concrete = store.materialize_ticket(ticket, fallback)?;
        store.discard(ticket)?;
        self.prepare_write(slot, store)?;
        self.write(slot, concrete, store)
    }
}

impl Frame {
    /// The frame's outermost scope: its closure scope in a closure call
    /// frame, otherwise the function body's root scope.
    pub(crate) fn root_scope(&self) -> crate::syntax::body_index::ScopeId {
        match self.active_scopes.first() {
            Some(scope)
                if self
                    .body
                    .scopes()
                    .get(scope.0 as usize)
                    .is_some_and(|descriptor| {
                        descriptor.kind == crate::syntax::body_index::ScopeKind::Closure
                    }) =>
            {
                *scope
            }
            _ => crate::syntax::body_index::ScopeId(0),
        }
    }
}

/// These modes have already resolved conditional consuming uses by Type.
pub(crate) fn join_capture_mode(a: CaptureMode, b: CaptureMode) -> CaptureMode {
    if a == CaptureMode::Value || b == CaptureMode::Value {
        CaptureMode::Value
    } else if a == CaptureMode::Mutable || b == CaptureMode::Mutable {
        CaptureMode::Mutable
    } else {
        CaptureMode::Shared
    }
}
