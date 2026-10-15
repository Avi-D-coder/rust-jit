//! Physical frame state. Syntax and query providers never mutate it.
#[path = "frame_aggregate_copy.rs"]
mod aggregate_copy_frame;
#[cfg(test)]
#[path = "frame_call_scope_tests.rs"]
mod call_scope_tests;
#[path = "frame_constant_arrays.rs"]
mod constant_array_frame;
#[path = "frame_source_constant.rs"]
mod source_constant_frame;
pub(crate) use constant_array_frame::{ConstantPublicationActivation, ConstantPublicationChunk};
#[cfg(test)]
#[path = "integer_literal_decode_tests.rs"]
mod integer_literal_decode_tests;
#[path = "frame_scope_cleanup.rs"]
mod scope_cleanup_frame;
#[path = "frame_temporary_scopes.rs"]
mod temporary_scope_frame;
use crate::core::digest::StableEncode;
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{NodeId, PcKey, SlotId, SlotKind},
    runtime::{
        cell::{
            CellId, CellOwner, CellState, ConstructionId, FrameId, GuestStore, StorageContract,
            TicketId,
        },
        shape::TypeId,
        value::ValueRepr,
    },
    syntax::body_index::{BodyIndex, ScopeId},
};
use serde::Serialize;

pub(crate) fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FrameStatus {
    Running,
    Suspended,
    Returned,
    Panicked,
    PanicCleaned,
    Faulted,
}
#[derive(Debug, Clone, Serialize)]
pub struct OpcodeCounters {
    pub fuel_used: u64,
    pub cleanup_elements_completed: u64,
    pub operations_started: u64,
    pub operations_completed: u64,
    pub exits_completed: u64,
    pub backedges_completed: u64,
    pub backedge_polls: u64,
    /// Literal, Unit, SetHint, ResultTransfer, ScopeEnter, ScopeLeave, Unsupported, BindLocal, ReadLocal, Discard, NumericBinary, NumericUnary, PrimitiveCast, NumericHint, DeclareLocal, LocalHint, AssignLocal, BranchHint, CheckUnit, CallSeal, CheckParameters, ReadParameter, CallPrepare, CallAdvance, Declaration, Panic, Assert, ConstructStart, ConstructField, ConstructFinish, MakePlace, Borrow, DerefPlace, ReadPlace, AssignPlace, ProjectTuple, ProjectIndex, ProjectField, ValuePlace, PlaceTransfer, CompoundHint.
    #[serde(serialize_with = "serialize_opcodes")]
    pub started_by_opcode: crate::ir::op::OpcodeCounters,
    pub return_scopes_cleaned: u64,
    pub panic_scopes_cleaned: u64,
}
fn serialize_opcodes<S: serde::Serializer>(
    counts: &crate::ir::op::OpcodeCounters,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(counts.as_slice(), serializer)
}
impl Default for OpcodeCounters {
    fn default() -> Self {
        Self {
            fuel_used: 0,
            cleanup_elements_completed: 0,
            operations_started: 0,
            operations_completed: 0,
            exits_completed: 0,
            backedges_completed: 0,
            backedge_polls: 0,
            started_by_opcode: [0; crate::ir::op::OPCODE_COUNT],
            return_scopes_cleaned: 0,
            panic_scopes_cleaned: 0,
        }
    }
}
impl OpcodeCounters {
    pub(crate) fn accumulate(&mut self, other: &Self) -> Result<(), Diagnostic> {
        macro_rules! add { ($($field:ident),*) => { $(self.$field = self.$field.checked_add(other.$field).ok_or_else(|| invalid("run counter overflow"))?;)* }; }
        add!(
            cleanup_elements_completed,
            operations_started,
            operations_completed,
            exits_completed,
            backedges_completed,
            backedge_polls,
            return_scopes_cleaned,
            panic_scopes_cleaned
        );
        self.fuel_used = self.fuel_used.max(other.fuel_used);
        for (a, b) in self
            .started_by_opcode
            .iter_mut()
            .zip(other.started_by_opcode)
        {
            *a = a
                .checked_add(b)
                .ok_or_else(|| invalid("run opcode counter overflow"))?;
        }
        Ok(())
    }
}
struct Slot {
    cell: Option<CellId>,
    hint: Option<TypeId>,
    strict_hint: bool,
    scope: ScopeId,
    source: Option<crate::traits::term::TyTerm>,
    conditional_extension: Option<NodeId>,
    construction: Option<ActiveConstruction>,
    place: Option<crate::runtime::place::Place>,
}
#[derive(Clone)]
struct ActiveConstruction {
    guard: ConstructionId,
    result: ConstructionResult,
    cleaning: bool,
    mapping: Option<std::sync::Arc<[u32]>>,
}
#[derive(Clone, Copy)]
enum ConstructionResult {
    Bound(TypeId),
    Observed(crate::runtime::object::StructuralConstructionKind),
}
impl ActiveConstruction {
    fn mapping_bytes(&self) -> Option<usize> {
        self.mapping.as_ref().map_or(Some(0), |map| {
            map.len()
                .checked_mul(std::mem::size_of::<u32>())?
                .checked_add(3 * std::mem::size_of::<usize>())
        })
    }
    fn storage_field(&self, written: u32) -> Result<u32, Diagnostic> {
        self.mapping.as_ref().map_or(Ok(written), |map| {
            map.get(written as usize)
                .copied()
                .ok_or_else(|| invalid("written constructor field absent from bound source map"))
        })
    }
}
enum ConstructorBinding {
    Structural(TypeId),
    Observed {
        kind: crate::runtime::object::StructuralConstructionKind,
        fields: usize,
    },
    Nominal(Box<crate::frontend::types::BoundNominalConstructor>),
}
struct PreparedConstructor {
    pc: PcKey,
    guard: SlotId,
    binding: ConstructorBinding,
    hints: Vec<(SlotId, TypeId)>,
}
/// Per-operation preparation owns only checked guest identities. It is discarded
/// on a fuel refusal and committed immediately after this exact operation starts.
struct PreparedSourceField {
    pc: PcKey,
    base: SlotId,
    destination: SlotId,
    binding: crate::frontend::types::BoundNominalField,
}
pub(crate) struct PreparedProjection {
    base: SlotId,
    destination: SlotId,
    place: crate::runtime::place::Place,
    source: Option<crate::traits::term::TyTerm>,
    retained_bytes: usize,
}
#[cfg(test)]
impl PreparedProjection {
    pub(crate) fn place_for_test(&self) -> &crate::runtime::place::Place {
        &self.place
    }
}
pub struct Frame {
    pub(crate) body: BodyIndex,
    pub(crate) pc: PcKey,
    pub(crate) status: FrameStatus,
    pub(crate) counters: OpcodeCounters,
    pub(crate) fuel: super::fuel::RunFuel,
    pub(crate) actions: super::action::RunActions,
    pub(crate) call_ready: bool,
    /// Exact verified Call exit retained until its one successful resumption.
    pub(crate) call_continuation: Option<(PcKey, SlotId)>,
    pub(crate) call_start: Option<super::call::CallStartFacts>,
    pub(crate) runtime_call:
        Option<std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>,
    constructor_start: Option<PreparedConstructor>,
    source_field: Option<PreparedSourceField>,
    constructor_mapping_bytes: usize,
    projection_bytes: usize,
    constant_activation: Option<source_constant_frame::BoundSourceConstant>,
    copy_activation: Option<aggregate_copy_frame::OpActivation>,
    cleanup_activation: Option<scope_cleanup_frame::Activation>,
    call_progress: Vec<(super::call::CallIdentity, (u32, u32))>,
    ordinary_calls: Box<[Option<crate::core::digest::Digest>]>,
    pub(crate) call_cells: Vec<CellId>,
    pub(crate) call_types: Vec<TypeId>,
    pub(crate) call_arguments: Option<super::call::ArgumentPacket>,
    context: Option<super::context::GenericEnv>,
    parameters: Vec<Option<CellId>>,
    parameters_initialized: bool,
    parameter_sources: Vec<crate::traits::term::TyTerm>,
    binding_sources: Vec<Option<crate::traits::term::TyTerm>>,
    pub(crate) return_source: Option<crate::traits::term::TyTerm>,
    store_session: crate::runtime::cell::StoreSession,
    anchor: FrameId,
    slots: Vec<Slot>,
    active_scopes: Vec<ScopeId>,
    bindings: Vec<Option<CellId>>,
}
impl Frame {
    #[cfg(test)]
    pub(crate) fn store_session(&self) -> crate::runtime::cell::StoreSession {
        self.store_session
    }
    pub(crate) fn require_store(&self, store: &GuestStore) -> Result<(), Diagnostic> {
        if self.store_session != store.pattern_binding_session() {
            return Err(crate::runtime::arena::stale());
        }
        store.require_frame(self.anchor)
    }
    pub(crate) fn metadata_bytes(body: &BodyIndex, parameters: usize) -> Option<usize> {
        // supported_source admits at most two owned boxed nodes: & [primitive; N].
        // Reserve that complete heap bound for each retained slot/binding view
        // and the return view. Abstract/nested views need later table accounting.
        let source_heap = std::mem::size_of::<crate::traits::term::TyTerm>().checked_mul(2)?;
        (body.slot_count() as usize)
            .checked_mul(
                std::mem::size_of::<Slot>().checked_add(source_heap)?,
            )
            .and_then(|bytes| {
                body.nodes()
                    .len()
                    .checked_mul(
                        std::mem::size_of::<Option<CellId>>()
                            .checked_add(std::mem::size_of::<Option<crate::traits::term::TyTerm>>())?
                            .checked_add(source_heap)?
                            .checked_add(std::mem::size_of::<Option<crate::core::digest::Digest>>())?,
                    )
                    .and_then(|n| bytes.checked_add(n))
            })
            .and_then(|bytes| {
                parameters
                    .checked_mul(
                        std::mem::size_of::<Option<CellId>>()
                            .checked_add(std::mem::size_of::<crate::traits::term::TyTerm>())?
                            .checked_add(source_heap)?,
                    )
                    .and_then(|n| bytes.checked_add(n))
            })
            .and_then(|bytes| {
                body.scopes()
                    .len()
                    .checked_mul(std::mem::size_of::<ScopeId>())
                    .and_then(|n| bytes.checked_add(n))
            })
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<PreparedConstructor>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<(PcKey, SlotId)>>()))
            .and_then(|bytes| bytes.checked_add(source_heap))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<super::call::CallStartFacts>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<source_constant_frame::BoundSourceConstant>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<aggregate_copy_frame::OpActivation>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<scope_cleanup_frame::Activation>>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<crate::runtime::cell::StoreSession>()))
    }
    pub fn new(body: BodyIndex, entry: PcKey, store: &mut GuestStore) -> Result<Self, Diagnostic> {
        if entry.body != *body.body() {
            return Err(invalid("frame entry belongs to another body"));
        }
        body.pc(
            crate::ir::pc::NodeId(entry.node),
            crate::ir::pc::Phase::from_code(entry.phase),
        )?;
        if Self::metadata_bytes(&body, 0).is_none_or(|bytes| bytes > 32 * 1024 * 1024) {
            return Err(crate::runtime::arena::resource());
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(body.slot_count() as usize)
            .map_err(|_| invalid("frame slot allocation failed"))?;
        for _ in 0..body.slot_count() {
            slots.push(Slot {
                cell: None,
                hint: None,
                strict_hint: true,
                scope: ScopeId(0),
                source: None,
                conditional_extension: None,
                construction: None,
                place: None,
            });
        }
        for node in body.nodes() {
            if let Some(id) = node.value {
                if node
                    .extended_to
                    .as_ref()
                    .is_some_and(|extension| !extension.constructors.is_empty())
                {
                    slots[id.0 as usize].conditional_extension = Some(node.id);
                }
                // A block's result belongs to its enclosing expression scope.
                // Inner temporaries and bindings remain in the entered block.
                slots[id.0 as usize].scope = if let Some(extension) = node
                    .extended_to
                    .as_ref()
                    .filter(|e| e.constructors.is_empty())
                {
                    extension.scope
                } else if let Some(clause) = node.boolean_scope.or(node.clause_scope) {
                    body.scopes()[clause.0 as usize]
                        .parent
                        .unwrap_or(node.scope)
                } else if matches!(
                    node.role,
                    crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Block
                            | crate::syntax::body_index::ExpressionKind::While
                            | crate::syntax::body_index::ExpressionKind::Loop
                    )
                ) {
                    body.scopes()[node.scope.0 as usize]
                        .parent
                        .unwrap_or(node.scope)
                } else {
                    node.scope
                };
            }
            for id in node.runtime_temporaries.into_iter().flatten() {
                // An initialized ref pattern moves this short-lived reference
                // result into its block-owned binding. Its referent independently
                // follows the indexed extending-pattern annotation above.
                let scope = if node.role == crate::syntax::body_index::NodeRole::Statement {
                    node.statement_scope.unwrap_or(node.scope)
                } else {
                    node.scope
                };
                slots[id.0 as usize].scope = scope;
                let place = SlotId(
                    id.0.checked_add(1)
                        .ok_or_else(|| invalid("runtime temporary place slot overflow"))?,
                );
                if body.slot_kind(place)? != SlotKind::Place {
                    return Err(invalid("runtime temporary lacks its reserved place pair"));
                }
                slots[place.0 as usize].scope = scope;
            }
            for id in &node.runtime_scalar_slots {
                slots[id.0 as usize].scope = node.scope;
                let place = SlotId(
                    id.0.checked_add(1)
                        .ok_or_else(|| invalid("runtime scalar place slot overflow"))?,
                );
                if body.slot_kind(place)? != SlotKind::Place {
                    return Err(invalid("runtime scalar lacks its reserved place pair"));
                }
                slots[place.0 as usize].scope = node.scope;
            }
            if let Some(id) = node.constructor_guard {
                slots[id.0 as usize].scope = node.scope;
            }
            if let Some(id) = node.place {
                slots[id.0 as usize].scope = node.scope;
            }
        }
        let mut active_scopes = Vec::new();
        active_scopes
            .try_reserve_exact(body.scopes().len())
            .map_err(|_| invalid("frame scope allocation failed"))?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(body.nodes().len())
            .map_err(|_| crate::runtime::arena::resource())?;
        bindings.resize(body.nodes().len(), None);
        let mut ordinary_calls = Vec::new();
        ordinary_calls
            .try_reserve_exact(body.nodes().len())
            .map_err(|_| crate::runtime::arena::resource())?;
        ordinary_calls.resize(body.nodes().len(), None);
        let ordinary_calls = ordinary_calls.into_boxed_slice();
        let anchor = store.new_frame_anchor()?;
        Ok(Self {
            body,
            pc: entry,
            status: FrameStatus::Running,
            counters: OpcodeCounters::default(),
            fuel: super::fuel::RunFuel::new(super::fuel::DEFAULT_FUEL)?,
            actions: super::action::RunActions::new(),
            call_ready: false,
            call_continuation: None,
            call_start: None,
            runtime_call: None,
            constructor_start: None,
            source_field: None,
            constructor_mapping_bytes: 0,
            projection_bytes: 0,
            constant_activation: None,
            copy_activation: None,
            cleanup_activation: None,
            call_progress: Default::default(),
            ordinary_calls,
            call_cells: vec![],
            call_types: vec![],
            call_arguments: None,
            context: None,
            parameters: Vec::new(),
            parameters_initialized: false,
            parameter_sources: Vec::new(),
            binding_sources: vec![None; bindings.len()],
            return_source: None,
            store_session: store.pattern_binding_session(),
            anchor,
            slots,
            active_scopes,
            bindings,
        })
    }
    /// Create a callee in the same run without replenishing fuel or interruption state.
    /// The dispatcher must supply the caller's GuestStore and accept the call first.
    pub fn new_in_run(
        body: BodyIndex,
        entry: PcKey,
        store: &mut GuestStore,
        caller: &mut Self,
    ) -> Result<Self, Diagnostic> {
        if caller.status != FrameStatus::Running
            && !(caller.status == FrameStatus::Suspended
                && caller.call_ready
                && caller.call_arguments.is_some())
        {
            return Err(invalid("callee creation requires a running caller"));
        }
        caller.require_store(store)?;
        caller.fuel.require_credit()?;
        let mut frame = Self::new(body, entry, store)?;
        frame.fuel = caller.fuel.share();
        frame.actions = caller.actions.share();
        Ok(frame)
    }
    pub(crate) fn resume_call(
        &mut self,
        resume: PcKey,
        result: SlotId,
        ticket: TicketId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_resume_call(&resume, result, ticket, store)?;
        self.write(result, ticket, store)?;
        self.pc = resume;
        self.call_ready = false;
        self.call_continuation = None;
        self.status = FrameStatus::Running;
        Ok(())
    }
    /// Check the suspended caller before any callee cleanup may invoke guest
    /// code. This neither consumes the returned owner nor publishes the caller.
    pub(crate) fn preflight_resume_call(
        &self,
        resume: &PcKey,
        result: SlotId,
        ticket: TicketId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Suspended
            || !self.call_ready
            || self.call_arguments.is_some()
        {
            return Err(invalid("call continuation is absent or already consumed"));
        }
        store.require_frame(self.anchor)?;
        if resume.body != *self.body.body()
            || self.body.pc(
                NodeId(resume.node),
                crate::ir::pc::Phase::from_code(resume.phase),
            )? != *resume
            || self
                .call_continuation
                .as_ref()
                .is_none_or(|(bound_resume, bound_result)| {
                    bound_resume != resume || *bound_result != result
                })
        {
            return Err(invalid(
                "call return destination or resume belongs to another continuation",
            ));
        }
        let value = store.peek_ticket(ticket)?;
        self.preflight_write_value(result, value, store)?;
        Ok(())
    }
    /// Reserve the exact still-uninitialized caller result cell before cleanup.
    /// The returned ticket stays owned by the return continuation until resumption.
    pub(crate) fn reserve_resume_call(
        &mut self,
        resume: &PcKey,
        result: SlotId,
        ticket: TicketId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_resume_call(resume, result, ticket, store)?;
        let cell = self.prepare_write(result, store)?;
        store.validate_initialization_value(cell, store.peek_ticket(ticket)?)
    }
    /// Dispatcher initialization before executing the accepted callee body.
    pub fn initialize_parameters(
        &mut self,
        packet: &super::call::ArgumentPacket,
        contracts: &[(TypeId, bool)],
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Running
            || self.parameters_initialized
            || !self.active_scopes.is_empty()
            || self.counters.operations_started != 0
        {
            return Err(invalid(
                "parameter initialization requires a fresh callee frame",
            ));
        }
        if contracts.len() > 100_000 {
            return Err(crate::runtime::arena::resource());
        }
        if !self.parameter_sources.is_empty() && self.parameter_sources.len() != contracts.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "parameter source inventory differs from the accepted signature".into(),
            ));
        }
        let types: Vec<_> = contracts.iter().map(|(ty, _)| *ty).collect();
        packet.check_types(store, &types)?;
        for (ordinal, (ty, _)) in contracts.iter().enumerate() {
            self.preflight_parameter_source(
                u32::try_from(ordinal).map_err(|_| crate::runtime::arena::resource())?,
                *ty,
                store,
            )?;
        }
        self.fuel.require_credit()?;
        let base = self
            .body
            .slot_count()
            .checked_add(
                u32::try_from(self.body.nodes().len())
                    .map_err(|_| invalid("parameter slot range overflow"))?,
            )
            .ok_or_else(|| invalid("parameter slot range overflow"))?;
        self.parameters
            .try_reserve_exact(contracts.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        if Self::metadata_bytes(
            &self.body,
            self.parameters
                .capacity()
                .max(self.parameter_sources.capacity()),
        )
        .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        let cells = store.new_parameter_cells(self.anchor, base, contracts)?;
        packet.deliver(store, &cells)?;
        self.parameters.extend(cells.into_iter().map(Some));
        self.parameters_initialized = true;
        Ok(())
    }
    pub(crate) fn check_parameters(&self, count: u32) -> Result<(), Diagnostic> {
        if count != 0 && (!self.parameters_initialized || self.parameters.len() != count as usize) {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "entry requires an accepted call parameter batch".into(),
            ));
        }
        Ok(())
    }
    fn parameter(&self, ordinal: u32) -> Result<CellId, Diagnostic> {
        if !self.parameters_initialized || !self.active_scopes.contains(&ScopeId(0)) {
            return Err(invalid("parameter read outside its entered function scope"));
        }
        self.parameters
            .get(ordinal as usize)
            .and_then(|c| *c)
            .ok_or_else(|| invalid("parameter cell absent or retired"))
    }
    fn parameter_metadata_count(&self) -> usize {
        self.parameters
            .capacity()
            .max(self.parameter_sources.capacity())
    }
    fn preflight_parameter_source(
        &self,
        ordinal: u32,
        ty: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::{runtime::shape::TypeKey, traits::term::TyTerm};
        match self.parameter_sources.get(ordinal as usize) {
            Some(TyTerm::Primitive(_)) => Self::require_borrow_source_target(
                self.parameter_sources.get(ordinal as usize),
                ty,
                store,
            ),
            Some(TyTerm::Ref(_, source_mutable, target)) => {
                let TypeKey::Ref {
                    mutable,
                    target: actual_target,
                } = store.type_descriptor(ty)?.key.as_ref()
                else {
                    return Err(Diagnostic::new(
                        "LRR-E-OWNERSHIP",
                        "parameter reference source differs from its descriptor".into(),
                    ));
                };
                if mutable != source_mutable {
                    return Err(Diagnostic::new(
                        "LRR-E-OWNERSHIP",
                        "parameter reference access differs from its source view".into(),
                    ));
                }
                Self::require_borrow_source_target(Some(target.as_ref()), *actual_target, store)
            }
            // Rigid parameters retain their identities under the existing
            // prepared definition environment; no incidental method is selected.
            Some(TyTerm::Param(_)) | None => Ok(()),
            _ => Err(invalid("unsupported retained parameter source view")),
        }
    }
    pub(crate) fn preflight_parameter(
        &self,
        ordinal: u32,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.parameter(ordinal)?;
        store.admit_owning_read(cell)?;
        let value = store.admit_move(cell)?;
        let ty = store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "parameter owning read requires its concrete signature context".into(),
            )
        })?;
        self.preflight_parameter_source(ordinal, ty, store)?;
        self.preflight_write_value(destination, value, store)
    }
    pub(crate) fn read_parameter(
        &mut self,
        ordinal: u32,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_parameter(ordinal, destination, store)?;
        let cell = self.parameter(ordinal)?;
        store.reserve_owning_read_result(self.slot(destination)?.cell.is_none(), false)?;
        self.prepare_write(destination, store)?;
        let ticket = store.read_owning(cell)?;
        self.write(destination, ticket, store)?;
        self.set_source_view(
            destination,
            self.parameter_sources.get(ordinal as usize).cloned(),
        )
    }
    pub(crate) fn call_identity(&self, site: u32) -> super::call::CallIdentity {
        (site, self.runtime_call.as_ref().map(|call| call.ordinal()))
    }
    pub(crate) fn preflight_call_start(
        &self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        let Some(super::call::CallStartFacts {
            site: ready,
            hints,
            ordinary,
        }) = &self.call_start
        else {
            return Err(invalid("call start lacks selected identity"));
        };
        if *ready != site
            || self.pc.node != site
            || hints.len() != arguments.len()
            || self
                .call_progress
                .iter()
                .any(|(key, _)| *key == self.call_identity(site))
        {
            return Err(invalid("call start context mismatch"));
        }
        ordinary.require(self.body.body(), NodeId(site))?;
        if let Some(call) = &self.runtime_call {
            call.require(
                self.body.body(),
                &self.pc,
                crate::syntax::runtime_macro_scalars::RuntimeMacroCallPhase::Prepare,
            )?;
            if call.arguments() != arguments {
                return Err(invalid(
                    "macro call argument slots differ from source witness",
                ));
            }
        } else if self.pc.phase != crate::ir::pc::Phase::Entry.code()?
            || self.body.nodes().get(site as usize).is_none_or(|node| {
                node.role
                    != crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Call,
                    )
            })
        {
            return Err(invalid(
                "ordinary call scope witness requires an indexed call",
            ));
        }
        for ((slot, _, source), argument) in hints.iter().zip(arguments) {
            if slot != argument {
                return Err(invalid("call hint slot mismatch"));
            }
            if source
                .as_ref()
                .is_some_and(|source| !Self::supported_source(source))
            {
                return Err(invalid(
                    "composite frame source view requires table integration",
                ));
            }
            self.preflight_hint(*slot)?;
        }
        Ok(())
    }
    pub(crate) fn start_call(&mut self, site: u32, arguments: &[SlotId]) -> Result<(), Diagnostic> {
        self.preflight_call_start(site, arguments)?;
        let identity = self.call_identity(site);
        self.call_progress
            .len()
            .checked_add(1)
            .and_then(|count| {
                count.checked_mul(std::mem::size_of::<(super::call::CallIdentity, (u32, u32))>())
            })
            .and_then(|bytes| {
                Self::metadata_bytes(&self.body, self.parameters.len())?.checked_add(bytes)
            })
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        self.call_progress
            .try_reserve_exact(1)
            .map_err(|_| crate::runtime::arena::resource())?;
        if self
            .call_progress
            .capacity()
            .checked_mul(std::mem::size_of::<(super::call::CallIdentity, (u32, u32))>())
            .and_then(|bytes| {
                Self::metadata_bytes(&self.body, self.parameters.len())?.checked_add(bytes)
            })
            .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        let super::call::CallStartFacts {
            hints, ordinary, ..
        } = self
            .call_start
            .take()
            .ok_or_else(|| invalid("call start consumed"))?;
        let definition = ordinary.require(self.body.body(), NodeId(site))?;
        // Opaque macro operand calls do not certify an indexed temporary-scope
        // barrier. Only ordinary AST calls may populate ordinary_calls.
        if self.runtime_call.is_none() {
            self.ordinary_calls[site as usize] = Some(definition);
        }
        for (slot, hint, source) in hints {
            self.set_source_view(slot, source)?;
            self.set_hint(slot, hint)?;
            self.slots[slot.0 as usize].strict_hint = false;
        }
        self.call_progress.push((
            identity,
            (
                0,
                u32::try_from(arguments.len()).map_err(|_| invalid("argument count overflow"))?,
            ),
        ));
        Ok(())
    }
    pub(crate) fn preflight_call_advance(
        &self,
        site: u32,
        argument: u32,
    ) -> Result<(), Diagnostic> {
        if let Some(call) = &self.runtime_call {
            call.require(
                self.body.body(),
                &self.pc,
                crate::syntax::runtime_macro_scalars::RuntimeMacroCallPhase::Advance(argument),
            )?;
        }
        let Some((_, (next, total))) = self
            .call_progress
            .iter()
            .find(|(key, _)| *key == self.call_identity(site))
        else {
            return Err(invalid("call progress absent"));
        };
        if *next != argument || argument >= *total {
            return Err(invalid("argument continuation repeated or out of order"));
        }
        Ok(())
    }
    pub(crate) fn advance_call(&mut self, site: u32, argument: u32) -> Result<(), Diagnostic> {
        self.preflight_call_advance(site, argument)?;
        let identity = self.call_identity(site);
        self.call_progress
            .iter_mut()
            .find(|(key, _)| *key == identity)
            .ok_or_else(|| invalid("call progress absent"))?
            .1
            .0 += 1;
        Ok(())
    }
    pub(crate) fn finish_call_progress(&mut self, site: u32) -> Result<(), Diagnostic> {
        if let Some(call) = &self.runtime_call {
            call.require(
                self.body.body(),
                &self.pc,
                crate::syntax::runtime_macro_scalars::RuntimeMacroCallPhase::Seal,
            )?;
        }
        let identity = self.call_identity(site);
        if let Some(index) = self
            .call_progress
            .iter()
            .position(|(key, _)| *key == identity)
        {
            let (_, (next, total)) = self.call_progress[index];
            if next != total {
                return Err(invalid("call sealed before all arguments completed"));
            }
            self.call_progress.remove(index);
        } else if self.runtime_call.is_some() {
            return Err(invalid("macro call sealed without its source preparation"));
        }
        Ok(())
    }
    /// Only an admitted deferred declared result may override its printed root hint.
    pub(crate) fn normalized_root_return_hint(
        &self,
        child: SlotId,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<Option<(crate::ir::op::PrimitiveType, TypeId)>, Diagnostic> {
        if self.pc.node != 0
            || self.pc.phase != crate::ir::pc::Phase::Entry.code()?
            || self.body.nodes()[0].value != Some(child)
        {
            return Ok(None);
        }
        let Some(context) = &self.context else {
            return Ok(None);
        };
        if context.definition().definition != self.body.body().definition {
            return Err(invalid("normalized root result owner differs"));
        }
        let Some((primitive, descriptor)) = context.normalized_root_hint(hint)? else {
            return Ok(None);
        };
        let source = crate::traits::term::TyTerm::Primitive(primitive);
        if self
            .source_view(child)?
            .is_some_and(|current| current != source)
            || self
                .hint(child)?
                .is_some_and(|current| current != descriptor)
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "root result context differs from its certified source".into(),
            ));
        }
        self.preflight_source_view(child, Some(&source))?;
        Ok(Some((primitive, descriptor)))
    }
    /// A checked declaration promise can supply an absent closed literal source,
    /// but must never erase a retained generic/rigid/nominal source at equal layout.
    pub(crate) fn certified_return_source(
        &self,
        value: SlotId,
        store: &GuestStore,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        let actual = self.source_view(value)?;
        let Some(context) = &self.context else {
            return Ok(actual);
        };
        let Some((primitive, descriptor)) = context.normalized_result() else {
            return Ok(actual);
        };
        if context.definition().definition != self.body.body().definition {
            return Err(invalid("certified return context owner differs"));
        }
        let expected = crate::traits::term::TyTerm::Primitive(primitive);
        if actual.is_none() {
            use crate::syntax::body_index::{ExpressionKind, NodeRole};
            let closed_literal = self.body.nodes().iter().any(|node| {
                node.value == Some(value)
                    && node.role == NodeRole::Expression(ExpressionKind::Literal)
            });
            if !closed_literal {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "certified return cannot invent a missing caller or variable source".into(),
                ));
            }
        }
        if actual.as_ref().is_some_and(|actual| actual != &expected) {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "returned source differs from certified deferred declaration".into(),
            ));
        }
        let cell = self.cell(value)?;
        let scalar = store.preview_scalar(cell, None)?;
        if scalar.type_id != descriptor.0 {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "returned value differs from certified deferred descriptor".into(),
            ));
        }
        Ok(Some(expected))
    }
    pub(crate) fn certified_root_transfer_source(
        &self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        if self.body.nodes()[0].value == Some(destination) {
            let source_view = self.certified_return_source(source, store)?;
            if self
                .context
                .as_ref()
                .and_then(|context| context.normalized_result())
                .is_some()
                && self
                    .source_view(destination)?
                    .as_ref()
                    .is_some_and(|current| Some(current) != source_view.as_ref())
            {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "root destination source differs from certified return".into(),
                ));
            }
            Ok(source_view)
        } else {
            self.source_view(source)
        }
    }
    pub(crate) fn install_context(
        &mut self,
        context: super::context::GenericEnv,
    ) -> Result<(), Diagnostic> {
        if self.counters.operations_started != 0
            || !self.active_scopes.is_empty()
            || self.context.is_some()
            || context.definition().definition != self.body.body().definition
        {
            return Err(invalid(
                "definition environment requires its fresh accepted frame",
            ));
        }
        self.context = Some(context);
        Ok(())
    }
    /// Current closed-source component also retains primitive-reference views.
    /// Abstract/composite parameter tables still require later source integration.
    pub(crate) fn install_parameter_sources(
        &mut self,
        sources: Vec<crate::traits::term::TyTerm>,
    ) -> Result<(), Diagnostic> {
        if self.parameters_initialized
            || !self.parameter_sources.is_empty()
            || sources.len() > 100_000
            || sources.iter().any(|source| {
                !Self::supported_source(source)
                    || (!matches!(
                        source,
                        crate::traits::term::TyTerm::Primitive(_)
                            | crate::traits::term::TyTerm::Param(_)
                    ) && !matches!(source, crate::traits::term::TyTerm::Ref(_, _, target)
                    if matches!(target.as_ref(), crate::traits::term::TyTerm::Primitive(_))))
            })
        {
            return Err(invalid(
                "parameter source views require fresh closed-source frame",
            ));
        }
        if sources.iter().any(|source| {
            matches!(source, crate::traits::term::TyTerm::Param(key)
            if key.definition != self.body.body().definition || key.ordinal >= 100_000)
        }) {
            return Err(invalid("parameter source identity mismatch"));
        }
        if Self::metadata_bytes(&self.body, sources.capacity())
            .and_then(|bytes| bytes.checked_add(self.constructor_mapping_bytes))
            .and_then(|bytes| bytes.checked_add(self.projection_bytes))
            .and_then(|bytes| bytes.checked_add(self.source_field_bytes()?))
            .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        self.parameter_sources = sources;
        Ok(())
    }
    pub(crate) fn source_view(
        &self,
        slot: SlotId,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        self.source_view_ref(slot).map(|source| source.cloned())
    }
    /// Immutable admission compares the retained source without allocating a clone.
    pub(crate) fn source_view_ref(
        &self,
        slot: SlotId,
    ) -> Result<Option<&crate::traits::term::TyTerm>, Diagnostic> {
        self.slots
            .get(slot.0 as usize)
            .map(|slot| slot.source.as_ref())
            .ok_or_else(|| invalid("source view slot absent"))
    }
    /// Literal storage contracts are strict. A propagated call/return hint
    /// describes expected context in an empty temporary; the intrinsic source
    /// of a Bool/Char value is checked at its reached call/return boundary.
    pub(crate) fn literal_source_context(
        &self,
        slot: SlotId,
    ) -> Result<Option<&crate::traits::term::TyTerm>, Diagnostic> {
        let value = self
            .slots
            .get(slot.0 as usize)
            .ok_or_else(|| invalid("literal source context slot absent"))?;
        Ok(if value.strict_hint {
            value.source.as_ref()
        } else {
            None
        })
    }
    /// Retain closed primitive arrays as well as primitive and nongeneric
    /// nominal reference targets. Abstract/nested source tables remain separate;
    /// a descriptor cannot invent a missing source identity.
    fn supported_source(source: &crate::traits::term::TyTerm) -> bool {
        use crate::traits::term::TyTerm;
        matches!(source, TyTerm::Primitive(_) | TyTerm::Param(_))
            || matches!(source, TyTerm::Ref(region, _, target) if !matches!(region, crate::traits::term::RegionTerm::Placeholder(_)) && (matches!(target.as_ref(), TyTerm::Primitive(_))
                || matches!(target.as_ref(), TyTerm::Nominal(_, args) if args.is_empty())
                || matches!(target.as_ref(), TyTerm::Array(element, crate::traits::term::ConstTerm::Value(len))
                    if matches!(element.as_ref(), TyTerm::Primitive(_)) && u32::try_from(*len).is_ok())))
            || matches!(source, TyTerm::Array(element, crate::traits::term::ConstTerm::Value(len))
                if matches!(element.as_ref(), TyTerm::Primitive(_)) && u32::try_from(*len).is_ok())
            || matches!(source, TyTerm::Nominal(_, args) if args.is_empty())
    }
    /// Scalar-match admission precedes the owning parameter read. A genuine
    /// source primitive supplies its descriptor; layout never supplies identity.
    pub(crate) fn definition_parameter_primitive(
        &self,
        ordinal: u32,
    ) -> Result<crate::ir::op::PrimitiveType, Diagnostic> {
        let env = self
            .context
            .as_ref()
            .ok_or_else(|| invalid("scalar parameter context absent"))?;
        if env.definition().definition != self.body.body().definition
            || ordinal >= 100_000
            || !self.parameters_initialized
        {
            return Err(invalid("scalar parameter context owner or ordinal differs"));
        }
        let source = self
            .parameter_sources
            .get(ordinal as usize)
            .ok_or_else(|| invalid("scalar parameter source absent"))?;
        let crate::traits::term::TyTerm::Primitive(primitive) = source else {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "scalar parameter context requires an admitted primitive source".into(),
            ));
        };
        Ok(*primitive)
    }
    /// This is a source-contract projection, not descriptor-based dispatch.
    /// GenericEnv and parameter_sources were installed only after accepting the
    /// exact current callable/header/body and its once-delivered arguments.
    pub(crate) fn definition_numeric_context(
        &self,
        context: crate::ir::op::DefinitionNumericContext,
    ) -> Result<crate::ir::op::PrimitiveType, Diagnostic> {
        use crate::{ir::op::DefinitionNumericContext, traits::term::TyTerm};
        let missing = || {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "numeric local requires its admitted definition source contract".into(),
            )
        };
        let env = self.context.as_ref().ok_or_else(missing)?;
        if env.definition().definition != self.body.body().definition {
            return Err(invalid("numeric context belongs to another definition"));
        }
        let fallback = match context {
            DefinitionNumericContext::ResultOrDefault { floating } => Some(if floating {
                crate::ir::op::PrimitiveType::F64
            } else {
                crate::ir::op::PrimitiveType::I32
            }),
            _ => None,
        };
        let normalized = env
            .normalized_result()
            .map(|(primitive, _)| TyTerm::Primitive(primitive));
        let mut source = match context {
            DefinitionNumericContext::Parameter(ordinal) => {
                if ordinal >= 100_000 || !self.parameters_initialized {
                    return Err(missing());
                }
                Some(
                    self.parameter_sources
                        .get(ordinal as usize)
                        .ok_or_else(missing)?,
                )
            }
            DefinitionNumericContext::Result | DefinitionNumericContext::ResultOrDefault { .. } => {
                env.definition()
                    .result_symbolic
                    .as_ref()
                    .or(normalized.as_ref())
            }
        };
        for _ in 0..64 {
            match source {
                Some(TyTerm::Primitive(primitive)) => {
                    if let Some(default) = fallback {
                        return Ok(
                            if Self::primitive_numeric_family(*primitive)
                                == Self::primitive_numeric_family(default)
                            {
                                *primitive
                            } else {
                                default
                            },
                        );
                    }
                    return Self::require_numeric_primitive(*primitive);
                }
                Some(TyTerm::Param(parameter)) => {
                    source = Some(env.symbolic_binding(*parameter).ok_or_else(missing)?);
                }
                None => return Err(missing()),
                Some(_) => {
                    // No compatible result source is a constraint on this bare
                    // literal. Its source-proven family supplies the ordinary
                    // Rust default; this never relabels the function result.
                    return fallback.ok_or_else(missing);
                }
            }
        }
        Err(missing())
    }
    fn primitive_numeric_family(primitive: crate::ir::op::PrimitiveType) -> Option<bool> {
        use crate::ir::op::PrimitiveType;
        match primitive {
            PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::I128
            | PrimitiveType::Isize
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::U128
            | PrimitiveType::Usize => Some(false),
            PrimitiveType::F32 | PrimitiveType::F64 => Some(true),
            _ => None,
        }
    }
    fn require_numeric_primitive(
        primitive: crate::ir::op::PrimitiveType,
    ) -> Result<crate::ir::op::PrimitiveType, Diagnostic> {
        Self::primitive_numeric_family(primitive)
            .map(|_| primitive)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "definition source is not a numeric primitive".into(),
                )
            })
    }
    pub(crate) fn resolved_annotation_source(
        &self,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        if let crate::ir::op::HintRecipe::DefinitionParameter(ordinal) = hint {
            return Ok(Some(crate::traits::term::TyTerm::Primitive(
                self.definition_parameter_primitive(*ordinal)?,
            )));
        }
        if let crate::ir::op::HintRecipe::DefinitionNumeric(context) = hint {
            return Ok(Some(crate::traits::term::TyTerm::Primitive(
                self.definition_numeric_context(*context)?,
            )));
        }
        Ok(Self::annotation_source(hint))
    }
    pub(crate) fn annotation_source(
        hint: &crate::ir::op::HintRecipe,
    ) -> Option<crate::traits::term::TyTerm> {
        use crate::{ir::op::HintRecipe, traits::term::TyTerm};
        match hint {
            // Dynamic pointee hints are resolved from the reached result slot;
            // they never manufacture an annotation without that frame context.
            HintRecipe::BorrowTarget { .. }
            | HintRecipe::DefinitionNumeric(_)
            | HintRecipe::DefinitionParameter(_) => None,
            HintRecipe::ArrayPrimitive { element, len } => Some(TyTerm::Array(
                Box::new(TyTerm::Primitive(*element)),
                crate::traits::term::ConstTerm::Value(u64::from(*len)),
            )),
            HintRecipe::None => None,
            HintRecipe::Parameter(key) => Some(TyTerm::Param(*key)),
            HintRecipe::Primitive(primitive) => Some(TyTerm::Primitive(*primitive)),
            HintRecipe::Reference {
                region,
                mutable,
                target,
            } => Some(TyTerm::Ref(
                region.clone(),
                *mutable,
                Box::new(TyTerm::Primitive(*target)),
            )),
        }
    }
    pub(crate) fn set_source_view(
        &mut self,
        slot: SlotId,
        source: Option<crate::traits::term::TyTerm>,
    ) -> Result<(), Diagnostic> {
        self.preflight_source_view(slot, source.as_ref())?;
        self.slots[slot.0 as usize].source = source;
        Ok(())
    }
    /// Source attachment is admitted while its current call environment still
    /// owns the tree, before cleanup retires that environment or runs effects.
    pub(crate) fn preflight_source_view(
        &self,
        slot: SlotId,
        source: Option<&crate::traits::term::TyTerm>,
    ) -> Result<(), Diagnostic> {
        self.slots
            .get(slot.0 as usize)
            .ok_or_else(|| invalid("source view slot absent"))?;
        if source.is_some_and(|source| !Self::supported_source(source)) {
            return Err(invalid(
                "composite frame source view requires table integration",
            ));
        }
        Ok(())
    }
    pub(crate) fn set_binding_source(
        &mut self,
        binding: NodeId,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<(), Diagnostic> {
        let written = self.resolved_annotation_source(hint)?;
        if let Some(written) = written {
            if !Self::supported_source(&written) {
                return Err(invalid(
                    "binding source view requires admitted frame metadata",
                ));
            }
            *self
                .binding_sources
                .get_mut(binding.0 as usize)
                .ok_or_else(|| invalid("binding source absent"))? = Some(written);
        }
        Ok(())
    }
    pub(crate) fn parameter_hint(
        &self,
        key: crate::traits::term::ParamKey,
    ) -> Result<Option<TypeId>, Diagnostic> {
        if key.definition != self.body.body().definition {
            return Err(invalid("hint parameter belongs to another definition"));
        }
        Ok(self
            .context
            .as_ref()
            .and_then(|context| context.runtime_binding(key)))
    }
    pub fn set_fuel_limit(&mut self, limit: u64) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Running
            || self.fuel.is_shared()
            || self.fuel.snapshot().used != 0
            || self.fuel.snapshot().stop.is_some()
            || self.counters.operations_started != 0
            || self.counters.exits_completed != 0
        {
            return Err(invalid("cannot reset fuel after execution"));
        }
        self.fuel = super::fuel::RunFuel::new(limit)?;
        Ok(())
    }
    pub fn fuel_snapshot(&self) -> super::fuel::FuelSnapshot {
        self.fuel.snapshot()
    }
    pub(crate) fn charge_fuel(&mut self) -> Result<(), Diagnostic> {
        self.fuel.charge()?;
        self.counters.fuel_used = self.fuel.snapshot().used;
        Ok(())
    }
    pub fn pc(&self) -> &PcKey {
        &self.pc
    }
    pub fn status(&self) -> FrameStatus {
        self.status
    }
    pub fn counters(&self) -> &OpcodeCounters {
        &self.counters
    }
    pub fn active_scopes(&self) -> &[ScopeId] {
        &self.active_scopes
    }
    fn slot(&self, id: SlotId) -> Result<&Slot, Diagnostic> {
        if self.body.slot_kind(id)? != SlotKind::Value {
            return Err(invalid("VM expected a value slot"));
        }
        self.slots
            .get(id.0 as usize)
            .ok_or_else(|| invalid("VM slot absent"))
    }
    pub fn cell(&self, id: SlotId) -> Result<CellId, Diagnostic> {
        self.slot(id)?
            .cell
            .ok_or_else(|| invalid("VM slot has no executed storage"))
    }
    fn place_slot(&self, id: SlotId) -> Result<&Slot, Diagnostic> {
        if self.body.slot_kind(id)? != SlotKind::Place {
            return Err(invalid("VM expected a place slot"));
        }
        let slot = self
            .slots
            .get(id.0 as usize)
            .ok_or_else(|| invalid("VM place slot absent"))?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("place scope inactive"));
        }
        Ok(slot)
    }
    pub(crate) fn preflight_make_place(
        &self,
        binding: NodeId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.place_slot(destination)?.place.is_some() {
            return Err(invalid("place slot already initialized"));
        }
        if self
            .binding_sources
            .get(binding.0 as usize)
            .and_then(Option::as_ref)
            .is_some_and(|source| {
                !Self::supported_source(source)
                    || matches!(source, crate::traits::term::TyTerm::Param(_))
            })
        {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "local borrow requires retained symbolic reference source integration".into(),
            ));
        }
        store.location_epoch(self.binding(binding)?)?;
        Ok(())
    }
    pub(crate) fn make_place(
        &mut self,
        binding: NodeId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_make_place(binding, destination, store)?;
        self.slots[destination.0 as usize].place =
            Some(crate::runtime::place::Place::cell(self.binding(binding)?));
        self.set_source_view(
            destination,
            self.binding_sources[binding.0 as usize].clone(),
        )
    }
    pub(crate) fn preflight_value_place(
        &self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.place_slot(destination)?.place.is_some()
            || self.place_slot(destination)?.source.is_some()
        {
            return Err(invalid("temporary place destination already prepared"));
        }
        if self.slot(source)?.source.as_ref().is_some_and(|source| {
            !Self::supported_source(source)
                || matches!(source, crate::traits::term::TyTerm::Param(_))
        }) {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "temporary receiver requires its retained symbolic source binding".into(),
            ));
        }
        // Formation never reads/copies/moves the guest value or allocates a new
        // guest cell. The value's original slot remains its sole cleanup owner.
        store
            .live_place_type_id(&crate::runtime::place::Place::cell(self.cell(source)?))
            .map(|_| ())
            .map_err(|error| match error {
                crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                    invalid("unprojected temporary location reported a bounds panic")
                }
            })
    }
    pub(crate) fn value_place(
        &mut self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_value_place(source, destination, store)?;
        let cell = self.cell(source)?;
        self.set_source_view(destination, self.source_view(source)?)?;
        self.slots[destination.0 as usize].place = Some(crate::runtime::place::Place::cell(cell));
        Ok(())
    }
    pub(crate) fn preflight_place_transfer(
        &self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if source == destination
            || self.place_slot(destination)?.place.is_some()
            || self.place_slot(destination)?.source.is_some()
        {
            return Err(invalid(
                "place transfer destination already prepared or aliases source",
            ));
        }
        let place = self.executed_place(source)?;
        store
            .place_type_id(place)
            .map(|_| ())
            .map_err(|error| match error {
                crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                    invalid("retained place reported a bounds panic during transfer")
                }
            })
    }
    pub(crate) fn transfer_place(
        &mut self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_place_transfer(source, destination, store)?;
        let place = self.slots[source.0 as usize].place.take();
        let source_view = self.slots[source.0 as usize].source.take();
        self.slots[destination.0 as usize].place = place;
        self.slots[destination.0 as usize].source = source_view;
        // Moving the bounded address buffer changes neither its size nor its
        // owning guest cell. Existing leases keep their original root/epochs.
        Ok(())
    }
    fn executed_place(&self, slot: SlotId) -> Result<&crate::runtime::place::Place, Diagnostic> {
        self.place_slot(slot)?
            .place
            .as_ref()
            .ok_or_else(|| invalid("place has not executed"))
    }
    #[cfg(test)]
    pub(crate) fn executed_place_for_test(
        &self,
        slot: SlotId,
    ) -> Result<&crate::runtime::place::Place, Diagnostic> {
        self.executed_place(slot)
    }
    fn clear_place(&mut self, id: SlotId) -> Result<(), Diagnostic> {
        let bytes = self
            .place_slot(id)?
            .place
            .as_ref()
            .map_or(Ok(0), |place| place.projection_bytes())?;
        self.projection_bytes = self
            .projection_bytes
            .checked_sub(bytes)
            .ok_or_else(|| invalid("place projection accounting underflow"))?;
        self.slots[id.0 as usize].place = None;
        self.slots[id.0 as usize].source = None;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn has_source_field_for_test(&self) -> bool {
        self.source_field.is_some()
    }
    fn source_field_bytes(&self) -> Option<usize> {
        self.source_field.as_ref().map_or(Some(0), |facts| {
            facts
                .binding
                .estimated_bytes()?
                .checked_add(std::mem::size_of::<PreparedSourceField>())
        })
    }
    pub(crate) fn nominal_place_source(
        &self,
        base: SlotId,
    ) -> Result<crate::core::digest::Digest, Diagnostic> {
        self.executed_place(base)?;
        match self.place_slot(base)?.source.as_ref() {
            Some(crate::traits::term::TyTerm::Nominal(definition, args)) if args.is_empty() => {
                Ok(*definition)
            }
            Some(crate::traits::term::TyTerm::Ref(_, _, target)) => match target.as_ref() {
                crate::traits::term::TyTerm::Nominal(definition, args) if args.is_empty() => {
                    Ok(*definition)
                }
                _ => Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "named field requires its retained nominal reference binding".into(),
                )),
            },
            _ => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "named field requires its retained nominal source binding".into(),
            )),
        }
    }
    pub(crate) fn prepare_source_field(
        &mut self,
        base: SlotId,
        destination: SlotId,
        binding: crate::frontend::types::BoundNominalField,
    ) -> Result<(), Diagnostic> {
        if self.nominal_place_source(base)? != binding.definition().fingerprint()
            || base == destination
            || self.place_slot(destination)?.place.is_some()
        {
            return Err(invalid("selected member source/place identity differs"));
        }
        Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|bytes| bytes.checked_add(self.constructor_mapping_bytes))
            .and_then(|bytes| bytes.checked_add(self.projection_bytes))
            .and_then(|bytes| bytes.checked_add(self.source_field_bytes()?))
            .and_then(|bytes| bytes.checked_add(binding.estimated_bytes()?))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PreparedSourceField>()))
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        self.source_field = Some(PreparedSourceField {
            pc: self.pc.clone(),
            base,
            destination,
            binding,
        });
        Ok(())
    }
    pub(crate) fn prepare_projection(
        &self,
        op: &crate::ir::op::Op,
        store: &GuestStore,
    ) -> Result<Option<PreparedProjection>, crate::runtime::place::PlaceFailure> {
        use crate::ir::op::Op;
        let (base, destination) = match op {
            Op::ProjectTuple {
                base, destination, ..
            }
            | Op::ProjectIndex {
                base, destination, ..
            }
            | Op::ProjectField { base, destination } => (*base, *destination),
            _ => return Ok(None),
        };
        if base == destination || self.place_slot(destination)?.place.is_some() {
            return Err(invalid("projection destination already live or aliases its base").into());
        }
        let base_place = self.executed_place(base)?;
        let count = base_place
            .projections
            .len()
            .checked_add(1)
            .ok_or_else(crate::runtime::arena::resource)?;
        let added = count
            .checked_mul(std::mem::size_of::<crate::runtime::place::Projection>())
            .ok_or_else(crate::runtime::arena::resource)?;
        // Bound the transient copy as well as all places retained by this frame.
        let metadata = Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .ok_or_else(crate::runtime::arena::resource)?;
        metadata
            .checked_add(self.projection_bytes)
            .and_then(|bytes| bytes.checked_add(self.constructor_mapping_bytes))
            .and_then(|bytes| bytes.checked_add(self.source_field_bytes()?))
            .and_then(|bytes| bytes.checked_add(added))
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        let member = if matches!(op, Op::ProjectField { .. }) {
            let facts = self
                .source_field
                .as_ref()
                .filter(|facts| {
                    facts.pc == self.pc && facts.base == base && facts.destination == destination
                })
                .ok_or_else(|| invalid("named projection lacks exact source dispatcher facts"))?;
            if self.nominal_place_source(base)? != facts.binding.definition().fingerprint() {
                return Err(invalid("named projection source view differs").into());
            }
            Some(&facts.binding)
        } else {
            None
        };
        let source = if let Some(member) = member {
            Some(crate::traits::term::TyTerm::Primitive(member.primitive()))
        } else {
            match (op, self.place_slot(base)?.source.as_ref()) {
                (_, None) => None,
                (
                    Op::ProjectIndex { .. },
                    Some(crate::traits::term::TyTerm::Array(
                        element,
                        crate::traits::term::ConstTerm::Value(len),
                    )),
                ) => {
                    let crate::traits::term::TyTerm::Primitive(primitive) = element.as_ref() else {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "array projection requires symbolic element binding".into(),
                        )
                        .into());
                    };
                    let expected =
                        crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id();
                    let actual = store.place_type_id(base_place)?;
                    if !matches!(store.type_descriptor(actual)?.key.as_ref(), crate::runtime::shape::TypeKey::Array { element, len: actual_len }
                    if *element == expected && u64::from(*actual_len) == *len)
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-OWNERSHIP",
                            "array source view differs from its descriptor".into(),
                        )
                        .into());
                    }
                    Some(crate::traits::term::TyTerm::Primitive(*primitive))
                }
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "projection requires retained composite source binding".into(),
                    )
                    .into());
                }
            }
        };
        let place = match op {
            Op::ProjectField { .. } => {
                let member = member.ok_or_else(|| invalid("prepared source member absent"))?;
                if let Some(crate::traits::term::TyTerm::Ref(_, mutable, _)) =
                    self.place_slot(base)?.source.as_ref()
                {
                    // A reference local is peeked, not copied or moved. Nested
                    // or projected reference receivers need their full source
                    // permission chain before this boundary can admit them.
                    if !matches!(base_place.root, crate::runtime::place::PlaceRoot::Cell(_))
                        || !base_place.projections.is_empty()
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "projected reference receiver requires its source permission chain"
                                .into(),
                        )
                        .into());
                    }
                    let receiver = store.place_type_id(base_place)?;
                    if !matches!(store.type_descriptor(receiver)?.key.as_ref(),
                        crate::runtime::shape::TypeKey::Ref { mutable: actual, .. } if actual == mutable)
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-OWNERSHIP",
                            "field reference source view differs from its descriptor".into(),
                        )
                        .into());
                    }
                    let referent = store.dereference_place(base_place)?;
                    store.project_source_field(&referent, member)?
                } else {
                    store.project_source_field(base_place, member)?
                }
            }
            Op::ProjectTuple { field, .. } => store.project_tuple(base_place, *field)?,
            Op::ProjectIndex { index, .. } => {
                if self.slot(*index)?.source.as_ref().is_some_and(|source| {
                    !matches!(
                        source,
                        crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::Usize)
                    )
                }) {
                    return Err(Diagnostic::new(
                        "LRR-E-INDEX-TYPE",
                        "index source view requires usize".into(),
                    )
                    .into());
                }
                let evaluated = store.preview_scalar(self.cell(*index)?, None)?;
                store.project_index(base_place, evaluated)?
            }
            _ => return Err(invalid("projection preflight discriminator changed").into()),
        };
        let retained_bytes = self
            .projection_bytes
            .checked_sub(base_place.projection_bytes()?)
            .and_then(|bytes| bytes.checked_add(added))
            .ok_or_else(crate::runtime::arena::resource)?;
        Ok(Some(PreparedProjection {
            base,
            destination,
            place,
            source,
            retained_bytes,
        }))
    }
    pub(crate) fn commit_projection(
        &mut self,
        prepared: PreparedProjection,
    ) -> Result<(), Diagnostic> {
        self.set_source_view(prepared.destination, prepared.source)?;
        self.slots[prepared.destination.0 as usize].place = Some(prepared.place);
        self.slots[prepared.base.0 as usize].place = None;
        self.slots[prepared.base.0 as usize].source = None;
        self.projection_bytes = prepared.retained_bytes;
        if self.source_field.as_ref().is_some_and(|facts| {
            facts.pc == self.pc
                && facts.base == prepared.base
                && facts.destination == prepared.destination
        }) {
            self.source_field = None;
        }
        Ok(())
    }
    pub(crate) fn preflight_deref_place(
        &self,
        reference: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        if self.place_slot(destination)?.place.is_some() {
            return Err(invalid("dereference place already executed").into());
        }
        store.dereference_place(self.executed_place(reference)?)?;
        let actual = store.place_type_id(self.executed_place(reference)?)?;
        let crate::runtime::shape::TypeKey::Ref { mutable, target } =
            store.type_descriptor(actual)?.key.as_ref()
        else {
            return Err(invalid("checked reference descriptor absent").into());
        };
        match self.place_slot(reference)?.source.as_ref() {
            Some(crate::traits::term::TyTerm::Ref(_, source_mutable, pointee)) => {
                if mutable != source_mutable {
                    return Err(Diagnostic::new(
                        "LRR-E-OWNERSHIP",
                        "dereference source access differs from its reference descriptor".into(),
                    )
                    .into());
                }
                // Reuse the source-aware borrow predicate: primitive identity
                // or an exact nongeneric nominal definition, never runtime-name
                // lookup or an invented symbolic target from a worker TypeId.
                Self::require_borrow_source_target(Some(pointee), *target, store)?;
            }
            None => Self::require_borrow_source_target(None, *target, store)?,
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "dereference requires retained reference source integration".into(),
                )
                .into());
            }
        }
        // A previously retained destination source is a constraint, not
        // scratch space. Reject mismatch before replacing it or publishing
        // an address; neither runtime shape nor spelling can override it.
        if let Some(expected) = self.place_slot(destination)?.source.as_ref() {
            Self::require_borrow_source_target(Some(expected), *target, store)?;
        }
        Ok(())
    }
    pub(crate) fn deref_place(
        &mut self,
        reference: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        self.preflight_deref_place(reference, destination, store)?;
        let place = store.dereference_place(self.executed_place(reference)?)?;
        let source = match self.place_slot(reference)?.source.as_ref() {
            Some(crate::traits::term::TyTerm::Ref(_, _, pointee)) => Some(pointee.as_ref().clone()),
            _ => None,
        };
        self.set_source_view(destination, source)?;
        self.slots[destination.0 as usize].place = Some(place);
        self.clear_place(reference)?;
        Ok(())
    }
    pub(crate) fn preflight_read_place(
        &self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        let (_, value, _) = store.admit_owning_place_read(self.executed_place(place)?)?;
        self.preflight_write_value(destination, value, store)?;
        Ok(())
    }
    pub(crate) fn read_place(
        &mut self,
        place: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        self.preflight_read_place(place, destination, store)?;
        let source = self.place_slot(place)?.source.clone();
        store.reserve_place_read_result()?;
        self.prepare_write(destination, store)?;
        let ticket = store.read_owning_place(self.executed_place(place)?)?;
        self.write(destination, ticket, store)?;
        self.set_source_view(destination, source)?;
        self.clear_place(place)?;
        Ok(())
    }
    pub(crate) fn preflight_assign_place(
        &self,
        place: SlotId,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        let plan = store
            .preview_intrinsic_place_assignment(self.executed_place(place)?, self.cell(source)?)?;
        if self.slot(destination)?.cell == Some(plan.target()) {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "assignment result aliases its reached target".into(),
            )
            .into());
        }
        self.preflight_write_value(destination, ValueRepr::unit(), store)?;
        Ok(())
    }
    pub(crate) fn assign_place(
        &mut self,
        place: SlotId,
        source: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        self.preflight_assign_place(place, source, destination, store)?;
        let plan = store
            .preview_intrinsic_place_assignment(self.executed_place(place)?, self.cell(source)?)?;
        let materializes = matches!(store.admit_move(self.cell(source)?)?.kind, 11 | 12);
        store.reserve_intrinsic_place_assignment(
            self.slot(destination)?.cell.is_none(),
            plan,
            materializes,
        )?;
        let ty = plan.ty();
        self.prepare_write(destination, store)?;
        let ticket = store.move_from(self.cell(source)?)?;
        let ticket = if matches!(store.peek_ticket(ticket)?.kind, 11 | 12) {
            let concrete = store.materialize_ticket(ticket, ty)?;
            store.discard(ticket)?;
            concrete
        } else {
            ticket
        };
        match plan {
            crate::runtime::cell::IntrinsicAssignment::Initialize { .. } => {
                store.initialize_place(self.executed_place(place)?, ticket)?
            }
            crate::runtime::cell::IntrinsicAssignment::Replace { .. } => {
                let old = store.replace_place(self.executed_place(place)?, ticket)?;
                store.discard(old)?;
            }
        }
        let result = store.ticket(ValueRepr::unit())?;
        self.write(destination, result, store)?;
        self.clear_place(place)?;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn binding_cell_for_test(&self, binding: NodeId) -> Result<CellId, Diagnostic> {
        self.binding(binding)
    }
    #[cfg(test)]
    pub(crate) fn clear_binding_source_for_test(
        &mut self,
        binding: NodeId,
    ) -> Result<(), Diagnostic> {
        self.binding(binding)?;
        *self
            .binding_sources
            .get_mut(binding.0 as usize)
            .ok_or_else(|| invalid("test binding source absent"))? = None;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn inject_test_place(
        &mut self,
        slot: SlotId,
        place: crate::runtime::place::Place,
    ) -> Result<(), Diagnostic> {
        self.place_slot(slot)?;
        self.clear_place(slot)?;
        self.projection_bytes = self
            .projection_bytes
            .checked_add(place.projection_bytes()?)
            .ok_or_else(crate::runtime::arena::resource)?;
        self.slots[slot.0 as usize].place = Some(place);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn projection_bytes_for_test(&self) -> usize {
        self.projection_bytes
    }
    #[cfg(test)]
    pub(crate) fn inject_test_index_scalar(
        &mut self,
        id: SlotId,
        value: ValueRepr,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        value.validate_scalar()?;
        let old = self.cell(id)?;
        store.admit_discard(old)?;
        let fresh = store.new_cell(
            StorageContract::Exact(TypeId(value.type_id)),
            true,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot: id.0,
            },
        )?;
        let ticket = store.ticket(value)?;
        store.initialize(fresh, ticket)?;
        let old_value = store.move_from(old)?;
        store.discard(old_value)?;
        store.retire_cell(old)?;
        self.slots[id.0 as usize].cell = Some(fresh);
        self.slots[id.0 as usize].source = None;
        Ok(())
    }
    fn require_borrow_source_target(
        source: Option<&crate::traits::term::TyTerm>,
        target: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::{runtime::shape::TypeKey, traits::term::TyTerm};
        let matches = match source {
            None => matches!(
                store.type_descriptor(target)?.key.as_ref(),
                TypeKey::Primitive(_)
            ),
            Some(TyTerm::Primitive(primitive)) => {
                crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id() == target
            }
            Some(TyTerm::Nominal(definition, arguments)) if arguments.is_empty() => {
                matches!(store.type_descriptor(target)?.key.as_ref(),
                    TypeKey::Nominal { definition: actual, args, const_args, .. }
                    if actual.fingerprint() == *definition && args.is_empty() && const_args.is_empty())
            }
            Some(TyTerm::Array(element, crate::traits::term::ConstTerm::Value(length))) => {
                let TyTerm::Primitive(primitive) = element.as_ref() else {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "borrowed array requires its retained primitive element binding".into(),
                    ));
                };
                let expected =
                    crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id();
                matches!(store.type_descriptor(target)?.key.as_ref(),
                    TypeKey::Array { element, len } if *element == expected && u64::from(*len) == *length)
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "borrowed composite source requires symbolic reference integration".into(),
                ));
            }
        };
        if !matches {
            return Err(Diagnostic::new(
                if source.is_none() {
                    "LRR-E-UNSUPPORTED"
                } else {
                    "LRR-E-OWNERSHIP"
                },
                if source.is_none() {
                    "borrowed nonprimitive target requires retained symbolic source binding"
                } else {
                    "borrowed place source view differs from its target descriptor"
                }
                .into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn preflight_borrow(
        &self,
        place: SlotId,
        access: crate::ir::op::BorrowAccess,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        use crate::runtime::{reference::Access, shape::TypeKey};
        let source = self
            .place_slot(place)?
            .place
            .as_ref()
            .ok_or_else(|| invalid("borrow place has not executed"))?;
        let access = match access {
            crate::ir::op::BorrowAccess::Shared => Access::Shared,
            crate::ir::op::BorrowAccess::Mutable => Access::Mutable,
        };
        let target = store.admit_place_borrow(source, access)?;
        Self::require_borrow_source_target(self.place_slot(place)?.source.as_ref(), target, store)?;
        let result = self.slot(destination)?;
        if !self.active_scopes.contains(&result.scope) {
            return Err(invalid("borrow result scope inactive").into());
        }
        if let Some(source) = result.source.as_ref() {
            use crate::traits::term::TyTerm;
            let TyTerm::Ref(_, mutable, pointee) = source else {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "borrow source context requires symbolic reference integration".into(),
                )
                .into());
            };
            Self::require_borrow_source_target(Some(pointee), target, store)?;
            if *mutable != (access == Access::Mutable) {
                return Err(Diagnostic::new(
                    "LRR-E-OWNERSHIP",
                    "borrow source annotation differs from its target or access".into(),
                )
                .into());
            }
        }
        let expected = if let Some(cell) = result.cell {
            if !matches!(store.state(cell)?, CellState::Uninit | CellState::Moved) {
                return Err(invalid("borrow result already live").into());
            }
            match store.declared(cell)? {
                StorageContract::Exact(ty) => Some(ty),
                StorageContract::Unbound => None,
                StorageContract::NumericLiteral => {
                    return Err(invalid("reference cannot initialize a numeric contract").into());
                }
            }
        } else if result.strict_hint {
            result.hint
        } else {
            None
        };
        if let Some(expected) = expected
            && !matches!(store.type_descriptor(expected)?.key.as_ref(), TypeKey::Ref { mutable, target: expected_target } if *mutable == (access == Access::Mutable) && *expected_target == target)
        {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "borrow result descriptor differs from its contract".into(),
            )
            .into());
        }
        Ok(())
    }
    pub(crate) fn borrow(
        &mut self,
        place: SlotId,
        access: crate::ir::op::BorrowAccess,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), crate::runtime::place::PlaceFailure> {
        use crate::runtime::reference::Access;
        self.preflight_borrow(place, access, destination, store)?;
        let written_source = match self.source_view(destination)? {
            Some(written) => Some(written),
            None => match self.place_slot(place)?.source.as_ref() {
                Some(
                    source @ (crate::traits::term::TyTerm::Primitive(_)
                    | crate::traits::term::TyTerm::Nominal(_, _)
                    | crate::traits::term::TyTerm::Array(_, _)),
                ) => {
                    // Preflight has already matched this written source target
                    // to the actual descriptor; a worker TypeId never invents it.
                    let range = self
                        .body
                        .nodes()
                        .get(self.pc.node as usize)
                        .ok_or_else(|| invalid("borrow source node absent"))?
                        .range;
                    Some(crate::traits::term::TyTerm::Ref(
                        crate::traits::term::RegionTerm::Local {
                            definition: self.body.body().definition,
                            start: range.start,
                            end: range.end,
                        },
                        access == crate::ir::op::BorrowAccess::Mutable,
                        Box::new(source.clone()),
                    ))
                }
                _ => None,
            },
        };
        let access = match access {
            crate::ir::op::BorrowAccess::Shared => Access::Shared,
            crate::ir::op::BorrowAccess::Mutable => Access::Mutable,
        };
        store.reserve_place_borrow_result(
            self.executed_place(place)?,
            self.slot(destination)?.cell.is_none(),
        )?;
        self.prepare_write(destination, store)?;
        let ticket = store.borrow_ticket(self.executed_place(place)?, access)?;
        self.write(destination, ticket, store)?;
        self.set_source_view(destination, written_source)?;
        self.clear_place(place)?;
        Ok(())
    }
    fn constructor_slot(&self, id: SlotId) -> Result<&Slot, Diagnostic> {
        if self.body.slot_kind(id)? != SlotKind::Guard {
            return Err(invalid("VM expected a construction guard slot"));
        }
        let slot = self
            .slots
            .get(id.0 as usize)
            .ok_or_else(|| invalid("construction guard slot absent"))?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("construction guard scope inactive"));
        }
        Ok(slot)
    }
    fn preflight_constructor_start(&self, id: SlotId) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Running || self.constructor_slot(id)?.construction.is_some()
        {
            return Err(invalid(
                "construction guard is already active or frame is not running",
            ));
        }
        Ok(())
    }
    fn admit_constructor_plan(
        &self,
        plan: &crate::syntax::constructors::ConstructorPlan,
    ) -> Result<SlotId, Diagnostic> {
        if &plan.origin().body_key != self.body.body()
            || plan.profile() != self.body.profile()
            || plan.node().0 != self.pc.node
            || self.pc.phase != 0
            || self.constructor_start.is_some()
        {
            return Err(invalid(
                "constructor preparation requires the exact entered source plan",
            ));
        }
        let guard = self
            .body
            .nodes()
            .get(plan.node().0 as usize)
            .and_then(|node| node.constructor_guard)
            .ok_or_else(|| invalid("prepared constructor guard absent"))?;
        self.preflight_constructor_start(guard)?;
        Ok(guard)
    }
    /// Install dispatcher-bound runtime facts without starting guest allocation.
    pub fn prepare_structural_constructor(
        &mut self,
        plan: &crate::syntax::constructors::ConstructorPlan,
        ty: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::{runtime::shape::TypeKey as T, syntax::constructors::ConstructorKind as C};
        let guard = self.admit_constructor_plan(plan)?;
        let key = store.type_descriptor(ty)?.key.as_ref();
        let matched = match (plan.kind(), key) {
            (C::Tuple(fields), T::Tuple(members)) => fields.len() == members.len(),
            (C::Tuple(fields), T::Primitive(_)) => fields.is_empty() && ty == TypeId(1),
            (C::Array(fields), T::Array { len, .. }) => {
                u32::try_from(fields.len()).ok() == Some(*len)
            }
            // A descriptor alone cannot prove a source repeat length or Copy.
            (C::Repeat { .. }, T::Array { .. }) => false,
            _ => false,
        };
        if !matched {
            return Err(invalid(
                "structural constructor plan/descriptor shape mismatch",
            ));
        }
        let mut hints = Vec::new();
        match (plan.kind(), key) {
            (C::Tuple(fields), T::Tuple(members)) => {
                hints
                    .try_reserve_exact(fields.len())
                    .map_err(|_| crate::runtime::arena::resource())?;
                hints.extend(
                    fields
                        .iter()
                        .zip(members.iter())
                        .map(|(field, ty)| (field.value, *ty)),
                );
            }
            (C::Array(fields), T::Array { element, .. }) => {
                hints
                    .try_reserve_exact(fields.len())
                    .map_err(|_| crate::runtime::arena::resource())?;
                hints.extend(fields.iter().map(|field| (field.value, *element)));
            }
            _ => {}
        }
        self.admit_constructor_hints(&hints, 0)?;
        self.constructor_start = Some(PreparedConstructor {
            pc: self.pc.clone(),
            guard,
            binding: ConstructorBinding::Structural(ty),
            hints,
        });
        Ok(())
    }
    pub(crate) fn prepare_observed_structural_constructor(
        &mut self,
        plan: &crate::syntax::constructors::ConstructorPlan,
    ) -> Result<(), Diagnostic> {
        use crate::{
            runtime::object::StructuralConstructionKind as S,
            syntax::constructors::ConstructorKind as C,
        };
        let guard = self.admit_constructor_plan(plan)?;
        let result = self
            .body
            .constructor_node(guard)?
            .value
            .ok_or_else(|| invalid("observed constructor result absent"))?;
        if self.source_view(result)?.is_some() || self.hint(result)?.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "observed structural constructor cannot replace its source result context".into(),
            ));
        }
        let (kind, fields) = match plan.kind() {
            C::Tuple(fields) => (S::Tuple, fields.len()),
            C::Array(fields) if fields.is_empty() => {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "empty array requires its source element type context".into(),
                ));
            }
            C::Array(fields) => (S::Array, fields.len()),
            _ => {
                return Err(invalid(
                    "observed construction requires tuple or array source",
                ));
            }
        };
        if fields > crate::runtime::object::MAX_CONSTRUCTION_FIELDS {
            return Err(crate::runtime::arena::resource());
        }
        let hints = Vec::new();
        self.admit_constructor_hints(&hints, 0)?;
        self.constructor_start = Some(PreparedConstructor {
            pc: self.pc.clone(),
            guard,
            binding: ConstructorBinding::Observed { kind, fields },
            hints,
        });
        Ok(())
    }
    fn require_nominal_result_source(
        &self,
        guard: SlotId,
        definition: crate::core::digest::Digest,
        infer: bool,
    ) -> Result<(), Diagnostic> {
        let result = self
            .body
            .constructor_node(guard)?
            .value
            .ok_or_else(|| invalid("nominal constructor result absent"))?;
        let admitted = match self.source_view(result)? {
            None => infer,
            Some(crate::traits::term::TyTerm::Nominal(source, args)) => {
                source == definition && args.is_empty()
            }
            _ => false,
        };
        if !admitted {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "nominal constructor result requires its source type binding".into(),
            ));
        }
        Ok(())
    }
    fn publish_nominal_source(
        &mut self,
        guard: SlotId,
        definition: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        let result = self
            .body
            .constructor_node(guard)?
            .value
            .ok_or_else(|| invalid("nominal constructor result absent"))?;
        self.set_source_view(
            result,
            Some(crate::traits::term::TyTerm::Nominal(definition, Vec::new())),
        )
    }
    pub fn prepare_nominal_constructor(
        &mut self,
        plan: &crate::syntax::constructors::ConstructorPlan,
        bound: crate::frontend::types::BoundNominalConstructor,
    ) -> Result<(), Diagnostic> {
        let guard = self.admit_constructor_plan(plan)?;
        if bound.mapping().constructor() != crate::core::digest::StableEncode::fingerprint(plan)
            || !matches!(plan.kind(), crate::syntax::constructors::ConstructorKind::Record { fields, rest: None, .. } if fields.len() == bound.variant().fields().len())
        {
            return Err(invalid("nominal constructor source shape mismatch"));
        }
        self.require_nominal_result_source(
            guard,
            bound.variant().definition().fingerprint(),
            true,
        )?;
        let crate::syntax::constructors::ConstructorKind::Record { fields, .. } = plan.kind()
        else {
            return Err(invalid("nominal constructor written fields absent"));
        };
        let mut hints = Vec::new();
        hints
            .try_reserve_exact(fields.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for (field, ordinal) in fields.iter().zip(bound.mapping().written_to_declaration()) {
            let primitive = *bound
                .variant()
                .fields()
                .get(*ordinal as usize)
                .ok_or_else(|| invalid("constructor field map outside variant"))?;
            hints.push((
                field.operand.value,
                crate::runtime::shape::PrimitiveIdentity::from_recipe(primitive)?.id(),
            ));
        }
        let extra = bound
            .estimated_bytes()
            .and_then(|bytes| {
                bytes.checked_add(
                    bound
                        .mapping()
                        .written_to_declaration()
                        .len()
                        .checked_mul(std::mem::size_of::<u32>())?
                        .checked_add(3 * std::mem::size_of::<usize>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        self.admit_constructor_hints(&hints, extra)?;
        self.constructor_start = Some(PreparedConstructor {
            pc: self.pc.clone(),
            guard,
            binding: ConstructorBinding::Nominal(Box::new(bound)),
            hints,
        });
        Ok(())
    }
    fn admit_constructor_hints(
        &self,
        hints: &Vec<(SlotId, TypeId)>,
        extra: usize,
    ) -> Result<(), Diagnostic> {
        for (slot, _) in hints {
            self.preflight_hint(*slot)?;
        }
        if hints
            .capacity()
            .checked_mul(std::mem::size_of::<(SlotId, TypeId)>())
            .and_then(|n| n.checked_add(extra))
            .and_then(|n| n.checked_add(self.constructor_mapping_bytes))
            .and_then(|n| n.checked_add(self.projection_bytes))
            .and_then(|n| n.checked_add(self.source_field_bytes()?))
            .and_then(|n| {
                n.checked_add(Self::metadata_bytes(
                    &self.body,
                    self.parameter_metadata_count(),
                )?)
            })
            .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        Ok(())
    }
    pub(crate) fn preflight_prepared_constructor(&self, guard: SlotId) -> Result<(), Diagnostic> {
        self.preflight_constructor_start(guard)?;
        if self
            .constructor_start
            .as_ref()
            .is_none_or(|facts| facts.pc != self.pc || facts.guard != guard)
        {
            return Err(invalid(
                "constructor opcode lacks matching dispatcher facts",
            ));
        }
        let facts = self
            .constructor_start
            .as_ref()
            .ok_or_else(|| invalid("constructor facts absent"))?;
        if let ConstructorBinding::Nominal(bound) = &facts.binding {
            self.require_nominal_result_source(
                guard,
                bound.variant().definition().fingerprint(),
                true,
            )?;
        }
        if matches!(facts.binding, ConstructorBinding::Observed { .. }) {
            let result = self
                .body
                .constructor_node(guard)?
                .value
                .ok_or_else(|| invalid("observed constructor result absent"))?;
            if self.source_view(result)?.is_some() || self.hint(result)?.is_some() {
                return Err(invalid(
                    "observed constructor source context changed before entry",
                ));
            }
        }
        for (slot, _) in &facts.hints {
            self.preflight_hint(*slot)?;
        }
        Ok(())
    }
    pub(crate) fn start_prepared_constructor(
        &mut self,
        guard: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_prepared_constructor(guard)?;
        let facts = self
            .constructor_start
            .take()
            .ok_or_else(|| invalid("constructor facts absent"))?;
        match facts.binding {
            ConstructorBinding::Structural(ty) => {
                self.begin_structural_constructor(guard, ty, store)?
            }
            ConstructorBinding::Observed { kind, fields } => {
                self.preflight_constructor_start(guard)?;
                let owned = store.begin_observed_construction(kind, fields)?;
                self.slots[guard.0 as usize].construction = Some(ActiveConstruction {
                    guard: owned,
                    result: ConstructionResult::Observed(kind),
                    cleaning: false,
                    mapping: None,
                });
            }
            ConstructorBinding::Nominal(bound) => {
                self.begin_bound_nominal_constructor(guard, &bound, store)?
            }
        }
        // Every hint destination was checked before fuel/guard allocation.
        for (slot, ty) in facts.hints {
            self.slots[slot.0 as usize].hint = Some(ty);
        }
        Ok(())
    }
    /// Dispatcher-bound structural descriptors stay in the physical frame;
    /// worker TypeIds are never embedded in pure query recipes.
    pub fn begin_structural_constructor(
        &mut self,
        id: SlotId,
        ty: TypeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constructor_start(id)?;
        let key = store.type_descriptor(ty)?.key.clone();
        let guard = match key.as_ref() {
            crate::runtime::shape::TypeKey::Tuple(fields) => store.begin_construction(fields)?,
            crate::runtime::shape::TypeKey::Array { element, len } => {
                let len = usize::try_from(*len).map_err(|_| crate::runtime::arena::resource())?;
                if len > crate::runtime::object::MAX_CONSTRUCTION_FIELDS {
                    return Err(crate::runtime::arena::resource());
                }
                let mut fields = Vec::new();
                fields
                    .try_reserve_exact(len)
                    .map_err(|_| crate::runtime::arena::resource())?;
                fields.resize(len, *element);
                store.begin_construction(&fields)?
            }
            crate::runtime::shape::TypeKey::Primitive(_) if ty == TypeId(1) => {
                store.begin_construction(&[])?
            }
            _ => {
                return Err(invalid(
                    "structural constructor requires tuple, array or unit descriptor",
                ));
            }
        };
        self.slots[id.0 as usize].construction = Some(ActiveConstruction {
            guard,
            result: ConstructionResult::Bound(ty),
            cleaning: false,
            mapping: None,
        });
        Ok(())
    }
    /// A nominal constructor requires the sealed source/schema witness.
    pub fn begin_nominal_constructor(
        &mut self,
        id: SlotId,
        bound: &crate::frontend::types::BoundNominalVariant,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constructor_start(id)?;
        self.require_nominal_result_source(id, bound.definition().fingerprint(), true)?;
        let (ty, guard) = store.begin_nominal_construction(bound)?;
        self.slots[id.0 as usize].construction = Some(ActiveConstruction {
            guard,
            result: ConstructionResult::Bound(ty),
            cleaning: false,
            mapping: None,
        });
        self.publish_nominal_source(id, bound.definition().fingerprint())
    }
    fn begin_bound_nominal_constructor(
        &mut self,
        id: SlotId,
        bound: &crate::frontend::types::BoundNominalConstructor,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constructor_start(id)?;
        self.require_nominal_result_source(id, bound.variant().definition().fingerprint(), true)?;
        let bytes = bound
            .mapping()
            .written_to_declaration()
            .len()
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(3 * std::mem::size_of::<usize>()))
            .ok_or_else(crate::runtime::arena::resource)?;
        let total = self
            .constructor_mapping_bytes
            .checked_add(bytes)
            .ok_or_else(crate::runtime::arena::resource)?;
        if Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|base| base.checked_add(total))
            .and_then(|base| base.checked_add(self.projection_bytes))
            .and_then(|base| base.checked_add(self.source_field_bytes()?))
            .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        let mut map = Vec::new();
        map.try_reserve_exact(bound.mapping().written_to_declaration().len())
            .map_err(|_| crate::runtime::arena::resource())?;
        map.extend_from_slice(bound.mapping().written_to_declaration());
        let mapping: std::sync::Arc<[u32]> = map.into();
        let (ty, guard) = store.begin_nominal_construction(bound.variant())?;
        self.slots[id.0 as usize].construction = Some(ActiveConstruction {
            guard,
            result: ConstructionResult::Bound(ty),
            cleaning: false,
            mapping: Some(mapping),
        });
        self.constructor_mapping_bytes = total;
        self.publish_nominal_source(id, bound.variant().definition().fingerprint())
    }
    pub fn constructor_field(
        &mut self,
        id: SlotId,
        field: u32,
        source: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constructor_field(id, field, source, store)?;
        let active = self
            .constructor_slot(id)?
            .construction
            .clone()
            .filter(|active| !active.cleaning)
            .ok_or_else(|| invalid("building construction guard absent"))?;
        store.construction_move_from(
            active.guard,
            active.storage_field(field)?,
            self.cell(source)?,
        )?;
        self.set_source_view(source, None)
    }
    pub(crate) fn preflight_constructor_field(
        &self,
        id: SlotId,
        field: u32,
        source: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let active = self
            .constructor_slot(id)?
            .construction
            .clone()
            .filter(|active| !active.cleaning)
            .ok_or_else(|| invalid("building construction guard absent"))?;
        if self
            .source_view(source)?
            .is_some_and(|source| matches!(source, crate::traits::term::TyTerm::Param(_)))
        {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "rigid field source requires a matching constructor binding".into(),
            ));
        }
        store.admit_construction_move(
            active.guard,
            active.storage_field(field)?,
            self.cell(source)?,
        )
    }
    pub fn finish_constructor(
        &mut self,
        id: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_constructor_finish(id, destination, store)?;
        let active = self
            .constructor_slot(id)?
            .construction
            .clone()
            .filter(|active| !active.cleaning)
            .ok_or_else(|| invalid("building construction guard absent"))?;
        // Reserve destination storage before consuming the guard. Failure leaves
        // all completed fields owned by the guard for explicit cleanup/retry.
        let remaining_mapping_bytes = self
            .constructor_mapping_bytes
            .checked_sub(
                active
                    .mapping_bytes()
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(|| invalid("constructor mapping charge mismatch"))?;
        let ty = match active.result {
            ConstructionResult::Bound(ty) => ty,
            ConstructionResult::Observed(_) => {
                store.intern_type(store.observed_construction_key(active.guard)?)?
            }
        };
        self.preflight_write_type(destination, ty, store)?;
        let cell = self.prepare_write(destination, store)?;
        store.validate_constructor_destination(cell, ty)?;
        let ticket = store.commit_construction(active.guard, ty)?;
        store.initialize(cell, ticket)?;
        self.slots[id.0 as usize].construction = None;
        self.constructor_mapping_bytes = remaining_mapping_bytes;
        // The admitted source view remains attached to the completed constructor.
        // Descriptor commitment cannot erase its written array element/length.
        Ok(())
    }
    pub(crate) fn preflight_constructor_finish(
        &self,
        id: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let active = self
            .constructor_slot(id)?
            .construction
            .clone()
            .filter(|active| !active.cleaning)
            .ok_or_else(|| invalid("building construction guard absent"))?;
        if self.body.constructor_node(id)?.value != Some(destination) {
            return Err(invalid(
                "construction destination belongs to another expression",
            ));
        }
        match active.result {
            ConstructionResult::Bound(ty) => {
                if let crate::runtime::shape::TypeKey::Nominal { definition, .. } =
                    store.type_descriptor(ty)?.key.as_ref()
                {
                    self.require_nominal_result_source(id, definition.fingerprint(), false)?;
                }
                self.preflight_write_type(destination, ty, store)?;
                if let Some(cell) = self.slot(destination)?.cell {
                    store.validate_constructor_destination(cell, ty)?;
                }
            }
            ConstructionResult::Observed(kind) => {
                use crate::runtime::{
                    object::StructuralConstructionKind as S, shape::TypeKey as T,
                };
                if self.source_view(destination)?.is_some() {
                    return Err(Diagnostic::new(
                        "LRR-E-GENERIC-BINDING",
                        "observed structural result cannot erase its source type view".into(),
                    ));
                }
                let key = store.observed_construction_key(active.guard)?;
                if !matches!(
                    (kind, &key),
                    (S::Tuple, T::Tuple(_)) | (S::Array, T::Array { .. })
                ) && !(kind == S::Tuple && store.find_type(&key) == Some(TypeId(1)))
                {
                    return Err(invalid("observed guard/result structural shape differs"));
                }
                if let Some(ty) = store.find_type(&key) {
                    self.preflight_write_type(destination, ty, store)?;
                    if let Some(cell) = self.slot(destination)?.cell {
                        store.validate_constructor_destination(cell, ty)?;
                    }
                } else {
                    self.preflight_temporary_slot(destination)?;
                    let slot = self.slot(destination)?;
                    if !self.active_scopes.contains(&slot.scope) {
                        return Err(invalid("observed constructor result scope inactive"));
                    }
                    if (slot.strict_hint || Some(destination) == self.body.nodes()[0].value)
                        && slot.hint.is_some()
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-OWNERSHIP",
                            "observed constructor conflicts with destination descriptor".into(),
                        ));
                    }
                    if let Some(cell) = slot.cell {
                        store.validate_observed_constructor_destination(cell)?;
                    }
                }
            }
        }
        Ok(())
    }
    /// Transfer one completed field to the ordinary cleanup continuation.
    /// The caller must retain the returned ticket across a guest Drop suspension.
    /// An empty guard retires only after every completed field was transferred.
    pub fn constructor_cleanup_next(
        &mut self,
        id: SlotId,
        store: &mut GuestStore,
    ) -> Result<Option<TicketId>, Diagnostic> {
        let active = self
            .constructor_slot(id)?
            .construction
            .clone()
            .ok_or_else(|| invalid("construction cleanup guard absent"))?;
        let remaining_mapping_bytes = self
            .constructor_mapping_bytes
            .checked_sub(
                active
                    .mapping_bytes()
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(|| invalid("constructor mapping charge mismatch"))?;
        let ticket = store.construction_cleanup_next(active.guard)?;
        if ticket.is_some() {
            self.slots[id.0 as usize].construction = Some(ActiveConstruction {
                cleaning: true,
                ..active
            });
        } else {
            self.slots[id.0 as usize].construction = None;
            self.constructor_mapping_bytes = remaining_mapping_bytes;
        }
        Ok(ticket)
    }
    pub(crate) fn hint(&self, id: SlotId) -> Result<Option<TypeId>, Diagnostic> {
        Ok(self.slot(id)?.hint)
    }
    pub(crate) fn preflight_hint(&self, id: SlotId) -> Result<(), Diagnostic> {
        if self.slot(id)?.cell.is_some() {
            return Err(invalid("hint applied after child storage was executed"));
        }
        Ok(())
    }
    pub(crate) fn set_hint(&mut self, id: SlotId, hint: Option<TypeId>) -> Result<(), Diagnostic> {
        self.preflight_hint(id)?;
        self.slots[id.0 as usize].hint = hint;
        Ok(())
    }
    pub(crate) fn declared_hint(
        &mut self,
        id: SlotId,
        hint: Option<TypeId>,
    ) -> Result<(), Diagnostic> {
        self.set_hint(id, hint)?;
        self.slots[id.0 as usize].strict_hint = true;
        Ok(())
    }
    fn numeric_hint_value(
        &self,
        parent: SlotId,
        left: Option<SlotId>,
        operator: Option<crate::ir::op::NumericBinary>,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        use crate::ir::op::NumericBinary as B;
        let left_type = if let Some(left) = left {
            let CellState::Live(value) = store.state(self.cell(left)?)? else {
                return Err(invalid("hint source is not live"));
            };
            if matches!(
                operator,
                Some(
                    B::Add
                        | B::Sub
                        | B::Mul
                        | B::Div
                        | B::Rem
                        | B::Shl
                        | B::Shr
                        | B::BitAnd
                        | B::BitOr
                        | B::BitXor
                )
            ) {
                super::numeric::borrowed_operand(store, self.cell(left)?)?
                    .map(|value| TypeId(value.type_id))
                    .or(store.effective_type(value)?)
            } else {
                store.effective_type(value)?
            }
        } else {
            None
        };
        let comparison = matches!(
            operator,
            Some(B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge)
        );
        let shift = matches!(operator, Some(B::Shl | B::Shr));
        let parent_type = self.hint(parent)?.filter(|ty| (2..=15).contains(&ty.0));
        let hint = if shift && left.is_some() {
            // Shift result/left width never constrains the independent RHS.
            // Source lowering selects fallback only for closed numeric syntax.
            // Named operands retain their source width or remain unresolved.
            None
        } else if comparison {
            left_type
        } else {
            left_type.or(parent_type)
        };
        Ok(hint)
    }
    pub(crate) fn preflight_numeric_hint(
        &self,
        parent: SlotId,
        child: SlotId,
        left: Option<SlotId>,
        operator: Option<crate::ir::op::NumericBinary>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_hint(child)?;
        self.numeric_hint_value(parent, left, operator, store)?;
        Ok(())
    }
    pub(crate) fn preflight_branch_hint(
        &self,
        parent: SlotId,
        condition: SlotId,
        yes: Option<SlotId>,
        no: Option<SlotId>,
        boolean: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let value = store.read_scalar(self.cell(condition)?)?;
        if value.kind != 2 || value.type_id != 2 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "branch requires a concrete bool".into(),
            ));
        }
        if let Some(child) = if value.payload == 1 { yes } else { no } {
            self.preflight_hint(child)?;
            if !boolean {
                self.numeric_hint_value(parent, None, None, store)?;
            }
        }
        Ok(())
    }
    pub(crate) fn numeric_hint(
        &mut self,
        parent: SlotId,
        child: SlotId,
        left: Option<SlotId>,
        operator: Option<crate::ir::op::NumericBinary>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let hint = self.numeric_hint_value(parent, left, operator, store)?;
        self.set_hint(child, hint)?;
        // A forwarded primitive operator constrains the borrow's pointee,
        // while the operand itself retains a reference source identity.
        // Primary structural value slots are paired with their node IDs; verify
        // that identity before consulting the already-indexed expression role.
        let primitive_borrow = matches!(hint, Some(TypeId(2 | 4..=15)))
            && self
                .source_view(parent)?
                .as_ref()
                .is_none_or(|source| matches!(source, crate::traits::term::TyTerm::Primitive(_)))
            && self
                .body
                .nodes()
                .get((child.0 / 2) as usize)
                .is_some_and(|node| {
                    node.value == Some(child)
                        && node.role
                            == crate::syntax::body_index::NodeRole::Expression(
                                crate::syntax::body_index::ExpressionKind::Borrow,
                            )
                });
        if operator.is_none() && !primitive_borrow {
            self.set_source_view(child, self.source_view(parent)?)?;
        }
        self.slots[child.0 as usize].strict_hint = false;
        Ok(())
    }
    pub(crate) fn branch_hint(
        &mut self,
        parent: SlotId,
        condition: SlotId,
        yes: Option<SlotId>,
        no: Option<SlotId>,
        boolean: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let value = store.read_scalar(self.cell(condition)?)?;
        if value.kind != 2 || value.type_id != 2 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "branch requires a concrete bool".into(),
            ));
        }
        if let Some(child) = if value.payload == 1 { yes } else { no } {
            if boolean {
                self.declared_hint(child, Some(TypeId(2)))?;
            } else {
                self.numeric_hint(parent, child, None, None, store)?;
            }
        }
        Ok(())
    }
    pub(crate) fn operand(
        &self,
        id: SlotId,
        expected: Option<TypeId>,
        store: &mut GuestStore,
    ) -> Result<ValueRepr, Diagnostic> {
        let cell = self.cell(id)?;
        let CellState::Live(value) = store.state(cell)? else {
            return Err(invalid("numeric operand is not live"));
        };
        if matches!(value.kind, 11 | 12)
            && store.effective_type(value)?.is_none()
            && let Some(expected) = expected
        {
            let alias = store.copy_from(cell)?;
            let result = store.materialize_ticket(alias, expected)?;
            let value = store.peek_ticket(result)?;
            store.discard(result)?;
            store.discard(alias)?;
            return Ok(value);
        }
        store.read_scalar(cell)
    }
    pub(crate) fn preflight_primitive_floating_constant(
        &self,
        constant: &crate::ir::op::PrimitiveFloatingConstant,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::{
            ir::op::PrimitiveType,
            syntax::body_index::{ExpressionKind, NodeRole},
            traits::term::TyTerm,
        };
        let value = constant.scalar();
        value.validate_scalar()?;
        if !matches!(
            (constant.primitive(), value.kind, value.type_id),
            (PrimitiveType::F32, 6, 14) | (PrimitiveType::F64, 7, 15)
        ) {
            return Err(invalid(
                "floating constant scalar differs from its primitive proof",
            ));
        }
        let node = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("floating constant indexed node absent"))?;
        if self.pc.phase != 0
            || node.role != NodeRole::Expression(ExpressionKind::Path)
            || node.value != Some(destination)
            || constant.destination() != destination
            || self.body.span(node.id)? != *constant.source_span()
        {
            return Err(invalid(
                "floating constant entry or destination differs from its source proof",
            ));
        }
        let descriptor = TypeId(value.type_id);
        if self
            .hint(destination)?
            .is_some_and(|expected| expected != descriptor)
        {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "floating constant differs from reached descriptor context".into(),
            ));
        }
        let source = TyTerm::Primitive(constant.primitive());
        if self
            .slot(destination)?
            .source
            .as_ref()
            .is_some_and(|expected| expected != &source)
        {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "floating constant differs from reached symbolic source context".into(),
            ));
        }
        self.preflight_source_view(destination, Some(&source))?;
        // Unlike decoding an unresolved literal, this closed proof applies no
        // new hint. Existing Uninit/Moved storage is admissible; live storage
        // remains refused by the ordinary write preflight.
        self.preflight_write_type(destination, descriptor, store)?;
        store.preflight_owning_read_result(self.slot(destination)?.cell.is_none(), false)
    }
    pub(crate) fn write_primitive_floating_constant(
        &mut self,
        constant: &crate::ir::op::PrimitiveFloatingConstant,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        // Reuse the complete owning-result reservation. No ticket exists when
        // capacity, scope or destination storage admission refuses this write.
        store.reserve_owning_read_result(self.slot(destination)?.cell.is_none(), false)?;
        let value = constant.scalar();
        let cell = self.prepare_write(destination, store)?;
        store.validate_initialization_value(cell, value)?;
        let ticket = store.ticket(value)?;
        // For this prevalidated scalar FrameSlot, initialize has no allocation,
        // parent field chain or callback. The ticket is consumed exactly once.
        self.write(destination, ticket, store)?;
        self.set_source_view(
            destination,
            Some(crate::traits::term::TyTerm::Primitive(constant.primitive())),
        )
    }
    pub(crate) fn preflight_literal(
        &self,
        id: SlotId,
        actual: Option<TypeId>,
        hint: Option<TypeId>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_hint(id)?;
        self.preflight_temporary_slot(id)?;
        let slot = self.slot(id)?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("literal scope inactive"));
        }
        if slot.strict_hint && hint.is_some() && actual != hint {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "literal violates declared destination descriptor".into(),
            ));
        }
        if let Some(ty) = actual {
            self.preflight_write_type(id, ty, store)?;
        }
        Ok(())
    }
    fn preflight_result_type(&self, id: SlotId, actual: Option<TypeId>) -> Result<(), Diagnostic> {
        if Some(id) == self.body.nodes()[0].value
            && let (Some(expected), Some(actual)) = (self.hint(id)?, actual)
            && expected != actual
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "reached body result differs from its declared descriptor".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn preflight_write_type(
        &self,
        id: SlotId,
        ty: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_result_type(id, Some(ty))?;
        self.preflight_temporary_slot(id)?;
        let slot = self.slot(id)?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("numeric result scope inactive"));
        }
        if let Some(cell) = slot.cell {
            if !matches!(store.state(cell)?, CellState::Uninit | CellState::Moved) {
                return Err(invalid("numeric result overwrites executed storage"));
            }
            if let StorageContract::Exact(expected) = store.declared(cell)?
                && expected != ty
            {
                return Err(Diagnostic::new(
                    "LRR-E-OWNERSHIP",
                    "numeric result violates storage descriptor".into(),
                ));
            }
        } else if slot.strict_hint && slot.hint.is_some_and(|expected| expected != ty) {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "numeric result violates declared hint".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn prepare_write(
        &mut self,
        id: SlotId,
        store: &mut GuestStore,
    ) -> Result<CellId, Diagnostic> {
        self.preflight_temporary_slot(id)?;
        let slot = self.slot(id)?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("value written outside its active scope"));
        }
        let cell = if let Some(cell) = slot.cell {
            if !matches!(store.state(cell)?, CellState::Uninit | CellState::Moved) {
                return Err(invalid("VM overwrote a live or retired temporary"));
            }
            cell
        } else {
            let contract = if slot.strict_hint {
                slot.hint
                    .map_or(StorageContract::Unbound, StorageContract::Exact)
            } else {
                StorageContract::Unbound
            };
            let cell = store.new_cell(
                contract,
                true,
                CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot: id.0,
                },
            )?;
            self.slots[id.0 as usize].cell = Some(cell);
            cell
        };
        Ok(cell)
    }
    pub(crate) fn write(
        &mut self,
        id: SlotId,
        ticket: TicketId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.prepare_write(id, store)?;
        store.initialize(cell, ticket)?;
        self.set_source_view(id, None)
    }
    pub(crate) fn preflight_item(&self) -> Result<(), Diagnostic> {
        let node = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("declaration item absent"))?;
        if node.role != crate::syntax::body_index::NodeRole::OpaqueItem
            || self.pc.phase != 0
            || !self.active_scopes.contains(&node.scope)
        {
            return Err(invalid(
                "declaration marker requires active entered opaque item",
            ));
        }
        Ok(())
    }
    pub(crate) fn preflight_declaration(&self, binding: NodeId) -> Result<(), Diagnostic> {
        let node = self
            .body
            .nodes()
            .get(binding.0 as usize)
            .ok_or_else(|| invalid("binding node absent"))?;
        if node.role != crate::syntax::body_index::NodeRole::Pattern
            || !self.active_scopes.contains(&node.scope)
            || self.bindings[binding.0 as usize].is_some()
        {
            return Err(invalid("binding location inactive or already declared"));
        }
        self.body
            .slot_count()
            .checked_add(binding.0)
            .ok_or_else(|| invalid("binding slot overflow"))?;
        Ok(())
    }
    fn preflight_write_value(
        &self,
        id: SlotId,
        value: ValueRepr,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_result_type(id, store.effective_type(value)?)?;
        self.preflight_temporary_slot(id)?;
        let slot = self.slot(id)?;
        if !self.active_scopes.contains(&slot.scope) {
            return Err(invalid("result scope inactive"));
        }
        if let Some(cell) = slot.cell {
            store.validate_initialization_value(cell, value)?;
        } else {
            store.admit_new_contract(if slot.strict_hint { slot.hint } else { None }, value)?;
        }
        Ok(())
    }
    pub(crate) fn preflight_transfer(
        &self,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let value = store.admit_move(self.cell(source)?)?;
        self.preflight_write_value(destination, value, store)
    }
    fn check_rigid_local_source(
        &self,
        expected: Option<&crate::traits::term::TyTerm>,
        source: SlotId,
    ) -> Result<(), Diagnostic> {
        if let (
            Some(crate::traits::term::TyTerm::Param(expected)),
            Some(crate::traits::term::TyTerm::Param(actual)),
        ) = (expected, self.source_view(source)?)
            && expected != &actual
        {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "local contract and initializer have distinct rigid source parameters".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn preflight_binding_source(
        &self,
        source: SlotId,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<(), Diagnostic> {
        let expected = match hint {
            crate::ir::op::HintRecipe::DefinitionParameter(ordinal) => {
                let expected = crate::traits::term::TyTerm::Primitive(
                    self.definition_parameter_primitive(*ordinal)?,
                );
                if self
                    .source_view(source)?
                    .as_ref()
                    .is_some_and(|actual| actual != &expected)
                {
                    return Err(Diagnostic::new(
                        "LRR-E-GENERIC-BINDING",
                        "local initializer differs from primitive parameter source".into(),
                    ));
                }
                Some(expected)
            }
            crate::ir::op::HintRecipe::DefinitionNumeric(context) => {
                let expected = crate::traits::term::TyTerm::Primitive(
                    self.definition_numeric_context(*context)?,
                );
                if self
                    .source_view(source)?
                    .as_ref()
                    .is_some_and(|actual| actual != &expected)
                {
                    return Err(Diagnostic::new(
                        "LRR-E-GENERIC-BINDING",
                        "local initializer source differs from admitted numeric context".into(),
                    ));
                }
                Some(expected)
            }
            crate::ir::op::HintRecipe::Parameter(key) => {
                Some(crate::traits::term::TyTerm::Param(*key))
            }
            _ => None,
        };
        self.check_rigid_local_source(expected.as_ref(), source)
    }
    pub(crate) fn preflight_bind(
        &self,
        binding: NodeId,
        source: SlotId,
        hint: Option<TypeId>,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_declaration(binding)?;
        let value = store.admit_move(self.cell(source)?)?;
        store.admit_new_contract(hint, value)
    }
    /// A reached use may resolve an unresolved literal from its admitted source
    /// context. This retains that source while materializing the shared literal;
    /// a concrete cell or a missing context never invents a primitive identity.
    fn local_read_source(
        &self,
        binding: NodeId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        use crate::traits::term::TyTerm;
        let source = self.binding_sources[binding.0 as usize].as_ref();
        if source.is_some() {
            return Ok(source.cloned());
        }
        let cell = self.binding(binding)?;
        let value = store.admit_move(cell)?;
        if !matches!(value.kind, 11 | 12) {
            return Ok(None);
        }
        let Some(TyTerm::Primitive(primitive)) = self.source_view_ref(destination)? else {
            return Ok(None);
        };
        let descriptor = crate::runtime::shape::PrimitiveIdentity::from_recipe(*primitive)?.id();
        if self.hint(destination)? != Some(descriptor) {
            return Err(invalid("local literal source and reached context differ"));
        }
        // Copies share their unresolved literal record. A prior reached use
        // may already have bound it even when this local has no retained source.
        if store
            .effective_type(value)?
            .is_some_and(|bound| bound != descriptor)
        {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "local literal was already resolved to a different source type".into(),
            ));
        }
        // Preview is pure: validate the literal family and range before fuel,
        // an owning read, or source publication occurs.
        store.preview_scalar(cell, Some(descriptor))?;
        Ok(Some(TyTerm::Primitive(*primitive)))
    }
    pub(crate) fn preflight_read_local(
        &self,
        binding: NodeId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.binding(binding)?;
        store.admit_owning_read(cell)?;
        let source = self.local_read_source(binding, destination, store)?;
        self.preflight_source_view(destination, source.as_ref())?;
        let mut value = store.admit_move(cell)?;
        if matches!(value.kind, 11 | 12)
            && let Some(hint) = store.effective_type(value)?.or(self.hint(destination)?)
        {
            value = store.preview_scalar(cell, Some(hint))?;
        }
        self.preflight_write_value(destination, value, store)
    }
    pub(crate) fn preflight_assign(
        &self,
        binding: NodeId,
        source: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let source_cell = self.cell(source)?;
        let value = store.admit_move(source_cell)?;
        let target = self.binding(binding)?;
        if source_cell == target || self.slot(destination)?.cell == Some(target) {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "assignment transfer aliases its reached destination".into(),
            ));
        }
        let owner = CellOwner::FrameSlot {
            frame: self.anchor,
            slot: self
                .body
                .slot_count()
                .checked_add(binding.0)
                .ok_or_else(|| invalid("replacement binding slot overflow"))?,
        };
        if store.owner(target)? != owner {
            return Err(invalid(
                "replacement local location belongs to another frame slot",
            ));
        }
        match store.state(target)? {
            CellState::Uninit | CellState::Moved => {
                store.validate_initialization_value(target, value)?
            }
            CellState::Live(_) => {
                store.admit_replacement(target, value)?;
                if Self::requires_structural_cleanup(target, store)? {
                    store.admit_cleanup_cell(target)?;
                } else {
                    store.admit_discard(target)?;
                }
            }
            _ => return Err(invalid("assignment target retired")),
        };
        self.check_rigid_local_source(self.binding_sources[binding.0 as usize].as_ref(), source)?;
        self.preflight_write_value(destination, ValueRepr::unit(), store)
    }
    pub(crate) fn preflight_discard(
        &self,
        source: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.cell(source)?;
        if self.requires_iterative_discard(source, store)? {
            self.preflight_temporary_slot(source)?;
            if !self.active_scopes.contains(&self.slot(source)?.scope) {
                return Err(invalid("discard owner scope inactive"));
            }
            store.admit_cleanup_cell(cell)
        } else {
            store.admit_discard(cell)
        }
    }
    /// Closed structural owners use the bounded machine. Nominal policies still
    /// require selected witnesses; their absence is never an intrinsic NoDrop.
    pub(super) fn requires_iterative_discard(
        &self,
        source: SlotId,
        store: &GuestStore,
    ) -> Result<bool, Diagnostic> {
        Self::requires_structural_cleanup(self.cell(source)?, store)
    }
    fn requires_structural_cleanup(cell: CellId, store: &GuestStore) -> Result<bool, Diagnostic> {
        let CellState::Live(value) = store.state(cell)? else {
            return Ok(false);
        };
        Ok(value.kind == 8
            && matches!(
                store.type_descriptor(TypeId(value.type_id))?.key.as_ref(),
                crate::runtime::shape::TypeKey::Tuple(_)
                    | crate::runtime::shape::TypeKey::Array { .. }
            ))
    }
    pub(crate) fn bind(
        &mut self,
        binding: NodeId,
        source: SlotId,
        hint: Option<TypeId>,
        writable: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_bind(binding, source, hint, store)?;
        let source_view = self.source_view(source)?;
        let node = self
            .body
            .nodes()
            .get(binding.0 as usize)
            .ok_or_else(|| invalid("binding node absent"))?;
        if node.role != crate::syntax::body_index::NodeRole::Pattern
            || !self.active_scopes.contains(&node.scope)
            || self.bindings[binding.0 as usize].is_some()
        {
            return Err(invalid(
                "binding location is inactive or already initialized",
            ));
        }
        let source = self.cell(source)?;
        let CellState::Live(value) = store.state(source)? else {
            return Err(invalid("binding initializer is not live"));
        };
        let physical_slot = self
            .body
            .slot_count()
            .checked_add(binding.0)
            .ok_or_else(|| invalid("binding physical slot overflow"))?;
        let destination = store.new_cell(
            hint.map_or(StorageContract::Unbound, StorageContract::Exact),
            writable,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot: physical_slot,
            },
        )?;
        store.validate_initialization_value(destination, value)?;
        let ticket = store.move_from(source)?;
        store.initialize(destination, ticket)?;
        self.bindings[binding.0 as usize] = Some(destination);
        self.binding_sources[binding.0 as usize] = source_view;
        Ok(())
    }
    pub(crate) fn declare_local(
        &mut self,
        binding: NodeId,
        hint: Option<TypeId>,
        writable: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_declaration(binding)?;
        let node = self
            .body
            .nodes()
            .get(binding.0 as usize)
            .ok_or_else(|| invalid("binding absent"))?;
        if node.role != crate::syntax::body_index::NodeRole::Pattern
            || !self.active_scopes.contains(&node.scope)
            || self.bindings[binding.0 as usize].is_some()
        {
            return Err(invalid("declaration location inactive or already declared"));
        }
        let slot = self
            .body
            .slot_count()
            .checked_add(binding.0)
            .ok_or_else(|| invalid("binding slot overflow"))?;
        let cell = store.new_cell(
            hint.map_or(StorageContract::Unbound, StorageContract::Exact),
            writable,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot,
            },
        )?;
        self.bindings[binding.0 as usize] = Some(cell);
        Ok(())
    }
    fn binding(&self, binding: NodeId) -> Result<CellId, Diagnostic> {
        self.bindings
            .get(binding.0 as usize)
            .and_then(|c| *c)
            .ok_or_else(|| invalid("local precedes declaration"))
    }
    fn local_hint_value(
        &self,
        binding: NodeId,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let cell = self.binding(binding)?;
        let hint = match store.declared(cell)? {
            StorageContract::Exact(ty) => Some(ty),
            StorageContract::NumericLiteral => match store.state(cell)? {
                CellState::Live(value) => store.effective_type(value)?,
                _ => None,
            },
            StorageContract::Unbound => None,
        };
        Ok(hint)
    }
    fn definition_assign_kind(
        &self,
        kind: crate::ir::op::AssignKind,
    ) -> Result<(crate::ir::op::PrimitiveType, crate::ir::op::PrimitiveType), Diagnostic> {
        match kind {
            crate::ir::op::AssignKind::Primitive { left, right } => Ok((left, right)),
            crate::ir::op::AssignKind::DefinitionNumeric { context, right } => {
                let left = self.definition_numeric_context(context)?;
                Ok((left, right.unwrap_or(left)))
            }
        }
    }
    pub(crate) fn preflight_compound_hint(
        &self,
        binding: NodeId,
        child: SlotId,
        kind: crate::ir::op::AssignKind,
        _store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let (left, _) = self.definition_assign_kind(kind)?;
        self.preflight_hint(child)?;
        self.binding(binding)?;
        let expected = crate::traits::term::TyTerm::Primitive(left);
        let actual = self.binding_sources[binding.0 as usize].as_ref();
        let conflicts = match kind {
            crate::ir::op::AssignKind::DefinitionNumeric { .. } => actual != Some(&expected),
            crate::ir::op::AssignKind::Primitive { .. } => {
                actual.is_some_and(|source| source != &expected)
            }
        };
        if conflicts {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "primitive compound assignment conflicts with definition-source local view".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn compound_hint(
        &mut self,
        binding: NodeId,
        child: SlotId,
        kind: crate::ir::op::AssignKind,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_compound_hint(binding, child, kind, store)?;
        let (_, right) = self.definition_assign_kind(kind)?;
        self.declared_hint(
            child,
            Some(crate::runtime::shape::PrimitiveIdentity::from_recipe(right)?.id()),
        )?;
        self.set_source_view(child, Some(crate::traits::term::TyTerm::Primitive(right)))
    }
    pub(crate) fn preflight_local_hint(
        &self,
        binding: NodeId,
        child: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_hint(child)?;
        self.local_hint_value(binding, store)?;
        Ok(())
    }
    pub(crate) fn local_hint(
        &mut self,
        binding: NodeId,
        child: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let hint = self.local_hint_value(binding, store)?;
        self.declared_hint(child, hint)?;
        self.set_source_view(child, self.binding_sources[binding.0 as usize].clone())
    }
    pub(crate) fn assign_local(
        &mut self,
        binding: NodeId,
        source: SlotId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_assign(binding, source, destination, store)?;
        let target = self.binding(binding)?;
        if Self::requires_structural_cleanup(target, store)? {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "structural local replacement requires its bounded opcode activation".into(),
            ));
        }
        let source_cell = self.cell(source)?;
        let state = store.state(target)?;
        let source_view = self.source_view(source)?;
        store.reserve_local_assignment_result(
            self.slot(destination)?.cell.is_none(),
            matches!(state, CellState::Live(_)),
        )?;
        self.prepare_write(destination, store)?;
        let ticket = store.move_from(source_cell)?;
        match state {
            CellState::Uninit | CellState::Moved => store.initialize(target, ticket)?,
            CellState::Live(_) => {
                let old = store.replace(target, ticket)?;
                store.discard(old)?;
            }
            _ => return Err(invalid("assignment target retired")),
        }
        if state == CellState::Uninit && self.binding_sources[binding.0 as usize].is_none() {
            self.binding_sources[binding.0 as usize] = source_view;
        }
        let unit = store.ticket(ValueRepr::unit())?;
        self.write(destination, unit, store)
    }
    pub(crate) fn read_local(
        &mut self,
        binding: NodeId,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_read_local(binding, destination, store)?;
        let source = self.local_read_source(binding, destination, store)?;
        let cell = self
            .bindings
            .get(binding.0 as usize)
            .and_then(|cell| *cell)
            .ok_or_else(|| invalid("local read precedes executed binding"))?;
        let value = store.admit_move(cell)?;
        let materializes_literal = matches!(value.kind, 11 | 12)
            && store
                .effective_type(value)?
                .or(self.hint(destination)?)
                .is_some();
        store.reserve_owning_read_result(
            self.slot(destination)?.cell.is_none(),
            materializes_literal,
        )?;
        self.prepare_write(destination, store)?;
        let ticket = store.read_owning(cell)?;
        let value = store.peek_ticket(ticket)?;
        let ticket = if matches!(value.kind, 11 | 12) {
            if let Some(hint) = store.effective_type(value)?.or(self.hint(destination)?) {
                let concrete = store.materialize_ticket(ticket, hint)?;
                store.discard(ticket)?;
                concrete
            } else {
                ticket
            }
        } else {
            ticket
        };
        self.write(destination, ticket, store)?;
        if self.binding_sources[binding.0 as usize].is_none() {
            self.binding_sources[binding.0 as usize] = source.clone();
        }
        self.set_source_view(destination, source)
    }
    pub(crate) fn discard(
        &mut self,
        source: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        // Direct synchronous callers cannot bypass structural activation.
        // Reached Discard opcodes are routed through begin_scope_cleanup.
        let cell = self.cell(source)?;
        store.admit_discard(cell)?;
        if matches!(store.state(cell)?, CellState::Live(_)) {
            let ticket = store.begin_drop(cell)?;
            store.discard(ticket)?;
        }
        store.retire_cell(cell)?;
        self.slots[source.0 as usize].cell = None;
        self.slots[source.0 as usize].source = None;
        self.slots[source.0 as usize].hint = None;
        self.slots[source.0 as usize].strict_hint = true;
        Ok(())
    }
    /// Dispatcher return cleanup for the initial scalar/storage inventory.
    pub fn finish_return(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Returned {
            return Err(invalid("return cleanup requires an actual returned frame"));
        }
        while let Some(scope) = self.active_scopes.last().copied() {
            if let Err(error) = self.leave(scope, store) {
                self.status = FrameStatus::Faulted;
                return Err(error);
            }
            self.counters.return_scopes_cleaned = self
                .counters
                .return_scopes_cleaned
                .checked_add(1)
                .ok_or_else(|| invalid("return cleanup counter overflow"))?;
        }
        Ok(())
    }
    pub fn finish_panic(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        if self.status != FrameStatus::Panicked {
            return Err(invalid("panic cleanup requires an actual guest panic"));
        }
        while let Some(scope) = self.active_scopes.last().copied() {
            if let Err(error) = self
                .drain_scalar_constructor_guards(scope, store)
                .and_then(|()| self.leave(scope, store))
            {
                self.status = FrameStatus::Faulted;
                return Err(error);
            }
            self.counters.panic_scopes_cleaned = self
                .counters
                .panic_scopes_cleaned
                .checked_add(1)
                .ok_or_else(|| invalid("panic cleanup counter overflow"))?;
        }
        self.status = FrameStatus::PanicCleaned;
        Ok(())
    }
    /// The scalar cleanup component never treats a missing nominal Drop witness
    /// as proof of NoDrop. Each unknown field stays guarded for later dispatch.
    fn drain_scalar_constructor_guards(
        &mut self,
        scope: ScopeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        for ordinal in (0..self.slots.len()).rev() {
            if self.slots[ordinal].scope != scope {
                continue;
            }
            let id = SlotId(u32::try_from(ordinal).map_err(|_| invalid("guard slot overflow"))?);
            while let Some(active) = self.slots[ordinal].construction.as_ref() {
                self.fuel.poll()?;
                if !store.admit_scalar_construction_cleanup(active.guard)? {
                    if self.constructor_cleanup_next(id, store)?.is_some() {
                        return Err(invalid("empty constructor cleanup yielded a field"));
                    }
                    break;
                }
                let completed = self
                    .counters
                    .cleanup_elements_completed
                    .checked_add(1)
                    .ok_or_else(|| invalid("constructor cleanup element counter overflow"))?;
                self.charge_fuel()?;
                let ticket = self
                    .constructor_cleanup_next(id, store)?
                    .ok_or_else(|| invalid("admitted constructor cleanup field absent"))?;
                store.discard(ticket)?;
                self.counters.cleanup_elements_completed = completed;
            }
        }
        Ok(())
    }
    pub(crate) fn preflight_return(
        &self,
        value: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let cell = self.cell(value)?;
        let admitted = store.admit_move(cell)?;
        // Checked object ownership/storage is already admitted by GuestStore.
        // A scalar preview cannot validate or copy an aggregate. Numeric
        // literal binding still requires the existing reached scalar context.
        let actual = if admitted.kind == 8 {
            admitted
        } else {
            store.preview_scalar(cell, None)?
        };
        if let Some(expected) = self.hint(SlotId(0))?
            && actual.type_id != expected.0
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "reached return value does not match its declared primitive descriptor".into(),
            ));
        }
        self.certified_return_source(value, store)?;
        Ok(())
    }
    pub(crate) fn preflight_control(
        &self,
        target: crate::syntax::body_index::LabelId,
        value: Option<SlotId>,
        continuing: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let target = self
            .body
            .labels()
            .get(target.0 as usize)
            .ok_or_else(|| invalid("control target absent"))?;
        if !self.active_scopes.contains(&target.scope) || continuing && !target.is_loop {
            return Err(invalid("control target inactive or not a loop"));
        }
        if !continuing {
            let destination = self.body.nodes()[target.node.0 as usize]
                .value
                .ok_or_else(|| invalid("control target result absent"))?;
            let scope = self.slot(destination)?.scope;
            let target_position = self
                .active_scopes
                .iter()
                .position(|s| *s == target.scope)
                .ok_or_else(|| invalid("target scope absent"))?;
            if self.active_scopes[target_position..].contains(&scope) {
                return Err(invalid("break result storage would be retired"));
            }
            let incoming = if let Some(source) = value {
                store.admit_move(self.cell(source)?)?
            } else {
                ValueRepr::unit()
            };
            self.preflight_write_value(destination, incoming, store)?;
        }
        Ok(())
    }
    pub(crate) fn control_transfer(
        &mut self,
        target: crate::syntax::body_index::LabelId,
        value: Option<SlotId>,
        continuing: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_control(target, value, continuing, store)?;
        let target = self
            .body
            .labels()
            .get(target.0 as usize)
            .cloned()
            .ok_or_else(|| invalid("control target absent"))?;
        if !self.active_scopes.contains(&target.scope) || continuing && !target.is_loop {
            return Err(invalid("control target inactive or not a loop"));
        }
        let destination = self.body.nodes()[target.node.0 as usize]
            .value
            .ok_or_else(|| invalid("control target result absent"))?;
        let source_view = value
            .map(|slot| self.source_view(slot))
            .transpose()?
            .flatten();
        let ticket = if continuing {
            None
        } else if let Some(source) = value {
            Some(store.move_from(self.cell(source)?)?)
        } else {
            Some(store.ticket(ValueRepr::unit())?)
        };
        loop {
            let scope = self
                .active_scopes
                .last()
                .copied()
                .ok_or_else(|| invalid("control cleanup lost target"))?;
            self.leave(scope, store)?;
            if scope == target.scope {
                break;
            }
        }
        if let Some(ticket) = ticket {
            self.write(destination, ticket, store)?;
            self.set_source_view(destination, source_view)?;
        }
        Ok(())
    }
    pub(crate) fn preflight_enter(&self, scope: ScopeId) -> Result<(), Diagnostic> {
        let descriptor = self
            .body
            .scopes()
            .get(scope.0 as usize)
            .ok_or_else(|| invalid("VM scope absent"))?;
        if descriptor.parent != self.active_scopes.last().copied()
            || self.active_scopes.contains(&scope)
        {
            return Err(invalid("VM scope entry violates lexical ancestry"));
        }
        Ok(())
    }
    pub(crate) fn preflight_leave(&self, scope: ScopeId) -> Result<(), Diagnostic> {
        // The cleanup dispatcher must drain guards before retiring their scope.
        // Never silently abandon completed field owners during scalar teardown.
        if let Some(position) = self
            .active_scopes
            .iter()
            .position(|active| *active == scope)
            && self.slots.iter().any(|slot| {
                slot.construction.is_some() && self.active_scopes[position..].contains(&slot.scope)
            })
        {
            return Err(invalid(
                "active constructor requires explicit cleanup before scope retirement",
            ));
        }
        self.preflight_cleanup_scope(scope)
    }
    /// Scope ancestry admission for the bounded dispatcher. The legacy scalar
    /// `leave` path retains its separate requirement that guards are drained.
    pub(crate) fn preflight_cleanup_scope(&self, scope: ScopeId) -> Result<(), Diagnostic> {
        if self.active_scopes.last() == Some(&scope) {
            return Ok(());
        }
        // A condition boundary drains only its entered pattern-clause prefix.
        // Other lexical scopes retain the strict innermost-only contract.
        if self
            .body
            .scopes()
            .get(scope.0 as usize)
            .is_some_and(|s| s.kind == crate::syntax::body_index::ScopeKind::Conditional)
            && let Some(position) = self.active_scopes.iter().position(|s| *s == scope)
            && self.active_scopes[position + 1..].iter().all(|s| {
                self.body.scopes()[s.0 as usize].kind
                    == crate::syntax::body_index::ScopeKind::Clause
            })
        {
            return Ok(());
        }
        Err(invalid("VM scope leave is not the active innermost scope"))
    }
    pub(crate) fn enter(&mut self, scope: ScopeId) -> Result<(), Diagnostic> {
        self.preflight_enter(scope)?;
        self.active_scopes.push(scope);
        Ok(())
    }
    fn retire_bindings(
        &mut self,
        scope: ScopeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        for (ordinal, cell) in self.bindings.iter_mut().enumerate().rev() {
            if self.body.nodes()[ordinal].scope == scope
                && let Some(id) = *cell
            {
                store.admit_discard(id)?;
                let completed = self
                    .counters
                    .cleanup_elements_completed
                    .checked_add(1)
                    .ok_or_else(|| invalid("cleanup element counter overflow"))?;
                self.fuel.charge()?;
                self.counters.fuel_used = self.fuel.snapshot().used;
                if matches!(store.state(id)?, CellState::Live(_)) {
                    let ticket = store.begin_drop(id)?;
                    store.discard(ticket)?;
                }
                store.retire_cell(id)?;
                *cell = None;
                self.binding_sources[ordinal] = None;
                self.counters.cleanup_elements_completed = completed;
            }
        }
        Ok(())
    }
    pub(crate) fn leave(
        &mut self,
        scope: ScopeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_leave(scope)?;
        while self.active_scopes.last() != Some(&scope) {
            let clause = self
                .active_scopes
                .last()
                .copied()
                .ok_or_else(|| invalid("condition prefix absent"))?;
            self.leave(clause, store)?;
        }
        let bindings_first = self.body.scopes()[scope.0 as usize].kind
            == crate::syntax::body_index::ScopeKind::Clause;
        if bindings_first {
            self.retire_bindings(scope, store)?;
        }
        self.fuel.poll()?;
        // Only the initial scalar/storage inventory is admitted by this VM task.
        // General user Drop dispatch is assigned to the later cleanup tasks.
        for slot in self.slots.iter_mut().rev().filter(|s| s.scope == scope) {
            if let Some(cell) = slot.cell {
                store.admit_discard(cell)?;
                let completed = self
                    .counters
                    .cleanup_elements_completed
                    .checked_add(1)
                    .ok_or_else(|| invalid("cleanup element counter overflow"))?;
                self.fuel.charge()?;
                self.counters.fuel_used = self.fuel.snapshot().used;
                if matches!(store.state(cell)?, CellState::Live(_)) {
                    let ticket = store.begin_drop(cell)?;
                    store.discard(ticket)?;
                }
                store.retire_cell(cell)?;
                slot.cell = None;
                self.counters.cleanup_elements_completed = completed;
            }
            slot.source = None;
            let bytes = slot
                .place
                .as_ref()
                .map_or(Ok(0), |place| place.projection_bytes())?;
            self.projection_bytes = self
                .projection_bytes
                .checked_sub(bytes)
                .ok_or_else(|| invalid("scope place accounting underflow"))?;
            slot.place = None;
            slot.hint = None;
            slot.strict_hint = true;
        }
        if !bindings_first {
            self.retire_bindings(scope, store)?;
        }
        if scope == ScopeId(0) {
            self.parameter_sources.clear();
            for cell in self.parameters.iter_mut().rev() {
                if let Some(id) = *cell {
                    store.admit_discard(id)?;
                    let completed = self
                        .counters
                        .cleanup_elements_completed
                        .checked_add(1)
                        .ok_or_else(|| invalid("parameter cleanup counter overflow"))?;
                    self.fuel.charge()?;
                    self.counters.fuel_used = self.fuel.snapshot().used;
                    if matches!(store.state(id)?, CellState::Live(_)) {
                        let ticket = store.begin_drop(id)?;
                        store.discard(ticket)?;
                    }
                    store.retire_cell(id)?;
                    *cell = None;
                    self.counters.cleanup_elements_completed = completed;
                }
            }
        }
        self.active_scopes.pop();
        Ok(())
    }
}

#[cfg(test)]
#[path = "intrinsic_literal_source_tests.rs"]
mod intrinsic_literal_source_tests;

#[cfg(test)]
#[path = "primitive_float_constant_tests.rs"]
mod primitive_float_constant_tests;

#[cfg(test)]
#[path = "frame_scope_order_tests.rs"]
mod scope_order_tests;
#[cfg(test)]
#[path = "frame_temporary_borrow_tests.rs"]
mod temporary_borrow_tests;

#[cfg(test)]
#[path = "frame_borrow_context_tests.rs"]
mod borrow_context_tests;

#[cfg(test)]
#[path = "frame_cast_scope_tests.rs"]
mod cast_scope_tests;

#[cfg(test)]
#[path = "frame_ref_pattern_tests.rs"]
mod ref_pattern_tests;

#[cfg(test)]
#[path = "frame_assignment_tests.rs"]
mod assignment_tests;

#[cfg(test)]
#[path = "frame_aggregate_copy_tests.rs"]
mod aggregate_copy_tests;

#[cfg(test)]
#[path = "frame_discard_cleanup_tests.rs"]
mod discard_cleanup_tests;
#[cfg(test)]
#[path = "frame_replacement_cleanup_tests.rs"]
mod replacement_cleanup_tests;
#[cfg(test)]
#[path = "frame_scope_cleanup_tests.rs"]
mod scope_cleanup_tests;

#[cfg(test)]
#[path = "frame_return_cleanup_tests.rs"]
mod return_cleanup_tests;

#[cfg(test)]
#[path = "frame_projected_assignment_tests.rs"]
mod projected_assignment_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::{
            body_index::{ExpressionKind, NodeRole},
            parse::ParsedFile,
        },
    };

    fn original_tuple_field_move_frame() -> (
        Frame,
        GuestStore,
        crate::ir::recipe::VerifiedRecipe,
        CellId,
        CellId,
        SlotId,
        SlotId,
    ) {
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/tuple-field-move-with-drop-borrow.rs"
        );
        assert_eq!(
            crate::syntax::place_fixture_facts::tuple_reference_move_selector(original),
            0
        );
        let (mut store, root, field, sibling, _) =
            crate::runtime::cell::tests::stock_tuple_reference_owner();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"stock-tuple-field-move"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "consume")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|node| {
                node.range
                    == crate::syntax::place_fixture_facts::tuple_reference_move_range(original)
                    && node.role == NodeRole::Expression(ExpressionKind::Other)
            })
            .unwrap()
            .clone();
        let entry = body.pc(node.id, crate::ir::pc::Phase::Commit).unwrap();
        let recipe = parsed.initial_thunk(handle, &body, &entry).unwrap();
        let [
            crate::ir::op::Op::ProjectTuple {
                base,
                field: ordinal,
                destination: place,
            },
            crate::ir::op::Op::ReadPlace {
                place: read_place,
                destination,
            },
        ] = recipe.recipe().operations.as_ref()
        else {
            panic!("exact original tuple projection/read recipe absent")
        };
        assert_eq!(*ordinal, 0);
        assert_eq!(place, read_place);
        let (base, place, destination) = (*base, *place, *destination);
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        let mut scopes = Vec::new();
        let mut scope = Some(node.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = frame.body.scopes()[id.0 as usize].parent;
        }
        for scope in scopes.into_iter().rev() {
            frame.enter(scope).unwrap();
        }
        // Disclosed prepared parameter/place context: the original projection
        // and read execute, while general tuple source binding, argument
        // delivery, returned reference and sibling Drop are separate gates.
        frame
            .inject_test_place(base, crate::runtime::place::Place::cell(root))
            .unwrap();
        (frame, store, recipe, field, sibling, place, destination)
    }
    #[test]
    fn rustc_original_tuple_reference_projection_executes_the_field_move_once() {
        let (mut frame, mut store, recipe, field, sibling, place, destination) =
            original_tuple_field_move_frame();
        let sibling_before = store.state(sibling).unwrap();
        crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap();
        assert_eq!(store.state(field).unwrap(), CellState::Moved);
        assert_eq!(store.state(sibling).unwrap(), sibling_before);
        assert!(frame.slots[place.0 as usize].place.is_none());
        assert_eq!(frame.counters.operations_completed, 2);
        let reference = store
            .reference_from_cell(frame.cell(destination).unwrap())
            .unwrap();
        assert_eq!(store.read_reference(reference).unwrap(), ValueRepr::unit());
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_tuple_reference_move_fuel_refusal_retains_projected_source() {
        let (mut frame, mut store, recipe, field, sibling, place, destination) =
            original_tuple_field_move_frame();
        // The projection may finish; the owning read has its own guarded fuel.
        frame.set_fuel_limit(1).unwrap();
        let before = (
            store.state(field).unwrap(),
            store.state(sibling).unwrap(),
            store.location_epoch(field).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(
            before,
            (
                store.state(field).unwrap(),
                store.state(sibling).unwrap(),
                store.location_epoch(field).unwrap(),
                store.budget().handles()
            )
        );
        assert!(frame.slots[place.0 as usize].place.is_some());
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.operations_completed, 1);
    }
    #[test]
    fn rustc_original_tuple_reference_move_result_mismatch_preserves_the_field_owner() {
        let (mut frame, mut store, recipe, field, sibling, place, destination) =
            original_tuple_field_move_frame();
        frame.declared_hint(destination, Some(TypeId(8))).unwrap();
        let before = (
            store.state(field).unwrap(),
            store.state(sibling).unwrap(),
            store.location_epoch(field).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            before,
            (
                store.state(field).unwrap(),
                store.state(sibling).unwrap(),
                store.location_epoch(field).unwrap(),
                store.budget().handles()
            )
        );
        assert!(frame.slots[place.0 as usize].place.is_some());
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.operations_completed, 1);
    }
    fn original_string_read_frame(
        max_handles: u32,
    ) -> (
        Frame,
        GuestStore,
        crate::ir::recipe::VerifiedRecipe,
        NodeId,
        SlotId,
        String,
    ) {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/use/use-after-move-based-on-type.rs"
        );
        let (text, writable, _) = crate::syntax::place_fixture_facts::string_local_move(source);
        let (pattern_range, read_range) =
            crate::syntax::place_fixture_facts::string_local_move_ranges(source);
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-owning-read"),
                vec![],
                &CfgSet::guest_default(),
            )
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
        let pattern = body
            .nodes()
            .iter()
            .find(|node| node.range == pattern_range && node.role == NodeRole::Pattern)
            .unwrap()
            .id;
        let read = body
            .nodes()
            .iter()
            .find(|node| {
                node.range == read_range && node.role == NodeRole::Expression(ExpressionKind::Path)
            })
            .unwrap()
            .clone();
        let entry = body.pc(read.id, crate::ir::pc::Phase::Entry).unwrap();
        let recipe = parsed.initial_thunk(handle, &body, &entry).unwrap();
        let [
            crate::ir::op::Op::ReadLocal {
                binding,
                destination,
            },
        ] = recipe.recipe().operations.as_ref()
        else {
            panic!("exact original local read absent")
        };
        assert_eq!(*binding, pattern);
        let destination = *destination;
        let mut store = GuestStore::new(1, max_handles, 1_000_000).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        let mut scopes = Vec::new();
        let mut scope = Some(read.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = frame.body.scopes()[id.0 as usize].parent;
        }
        for id in scopes.into_iter().rev() {
            frame.enter(id).unwrap();
        }
        // Supply only the original to_string result and executed x binding.
        // Method/println/full source execution remains a separate pending oracle.
        let ticket = store.string(&text).unwrap();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let cell = store
            .new_cell(
                StorageContract::Exact(ty),
                writable,
                CellOwner::FrameSlot {
                    frame: frame.anchor,
                    slot: frame.body.slot_count().checked_add(pattern.0).unwrap(),
                },
            )
            .unwrap();
        store.initialize(cell, ticket).unwrap();
        frame.bindings[pattern.0 as usize] = Some(cell);
        (frame, store, recipe, pattern, destination, text)
    }
    #[test]
    fn reached_literal_local_retains_source_and_refuses_shared_binding_conflict() {
        use crate::{ir::op::PrimitiveType, traits::term::TyTerm};
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
        );
        for prior in [None, Some(TypeId(11))] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
                ProfileIdentity::current(),
            )
            .unwrap();
            let outline = parsed
                .outline(
                    Digest::of_bytes(b"literal local source guard"),
                    vec![],
                    &CfgSet::guest_default(),
                )
                .unwrap();
            let handle = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "break_value")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let body = parsed.index_body(handle, Default::default()).unwrap();
            let read = body
                .nodes()
                .iter()
                .find(|node| {
                    node.role == NodeRole::Expression(ExpressionKind::Path)
                        && &original[(handle.range.start + node.range.start) as usize
                            ..(handle.range.start + node.range.end) as usize]
                            == "result"
                })
                .unwrap()
                .clone();
            let entry = body.pc(read.id, crate::ir::pc::Phase::Entry).unwrap();
            let recipe = parsed.initial_thunk(handle, &body, &entry).unwrap();
            let [
                crate::ir::op::Op::ReadLocal {
                    binding,
                    destination,
                },
            ] = recipe.recipe().operations.as_ref()
            else {
                panic!("original return-local read recipe absent")
            };
            let (binding, destination) = (*binding, *destination);
            let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
            let mut frame = Frame::new(body, entry, &mut store).unwrap();
            frame.set_fuel_limit(100).unwrap();
            let mut scopes = Vec::new();
            let mut scope = Some(read.scope);
            while let Some(id) = scope {
                scopes.push(id);
                scope = frame.body.scopes()[id.0 as usize].parent;
            }
            for scope in scopes.into_iter().rev() {
                frame.enter(scope).unwrap();
            }
            // Disclosed VM seam: supply the unresolved result of the original
            // labelled block and an admitted destination source. This is an
            // LRR source/ownership guard, not complete original-main execution.
            let literal = store.integer_literal(false, 1, "1".into()).unwrap();
            let cell = store
                .new_cell(
                    StorageContract::Unbound,
                    false,
                    CellOwner::FrameSlot {
                        frame: frame.anchor,
                        slot: frame.body.slot_count().checked_add(binding.0).unwrap(),
                    },
                )
                .unwrap();
            store.initialize(cell, literal).unwrap();
            frame.bindings[binding.0 as usize] = Some(cell);
            if let Some(bound) = prior {
                let copy = store.read_owning(cell).unwrap();
                let concrete = store.materialize_ticket(copy, bound).unwrap();
                store.discard(copy).unwrap();
                store.discard(concrete).unwrap();
            }
            let (primitive, descriptor) = if prior.is_some() {
                (PrimitiveType::U64, TypeId(12))
            } else {
                (PrimitiveType::U32, TypeId(11))
            };
            let source = TyTerm::Primitive(primitive);
            frame.declared_hint(destination, Some(descriptor)).unwrap();
            frame
                .set_source_view(destination, Some(source.clone()))
                .unwrap();
            let before = (
                frame.fuel_snapshot(),
                store.budget().handles(),
                store.state(cell).unwrap(),
            );
            let outcome = crate::vm::step::step(&mut frame, &mut store, &recipe);
            if prior.is_some() {
                assert_eq!(outcome.unwrap_err().code, "LRR-E-TYPE-CONTEXT");
                assert_eq!(
                    (
                        frame.fuel_snapshot(),
                        store.budget().handles(),
                        store.state(cell).unwrap()
                    ),
                    before
                );
                assert!(frame.binding_sources[binding.0 as usize].is_none());
                assert!(frame.slots[destination.0 as usize].cell.is_none());
                assert_eq!(frame.counters.operations_started, 0);
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    crate::vm::step::StepOutcome::Next(_)
                ));
                assert_eq!(
                    frame.binding_sources[binding.0 as usize],
                    Some(source.clone())
                );
                assert_eq!(
                    store
                        .read_scalar(frame.cell(destination).unwrap())
                        .unwrap()
                        .type_id,
                    descriptor.0
                );
                assert_eq!(frame.counters.started_by_opcode[8], 1);
                assert_eq!(frame.counters.operations_completed, 1);
            }
            assert_eq!(frame.source_view(destination).unwrap(), Some(source));
        }
    }
    #[test]
    fn rustc_original_string_local_read_executes_checked_move_once() {
        let (mut frame, mut store, recipe, binding, destination, text) =
            original_string_read_frame(1000);
        let source = frame.binding(binding).unwrap();
        let before = store.state(source).unwrap();
        let epoch = store.location_epoch(source).unwrap();
        assert!(matches!(
            crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap(),
            crate::vm::step::StepOutcome::Next(_)
        ));
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        assert_eq!(store.location_epoch(source).unwrap(), epoch + 1);
        assert_eq!(
            store.state(frame.cell(destination).unwrap()).unwrap(),
            before
        );
        assert_eq!(frame.counters.started_by_opcode[8], 1);
        assert_eq!(frame.counters.operations_completed, 1);
        assert_eq!(
            frame
                .preflight_read_local(binding, destination, &store)
                .unwrap_err()
                .code,
            "LRR-E-USE-AFTER-MOVE"
        );
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        assert_eq!(store.string_contents(ticket).unwrap(), text);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_string_local_read_reserves_destination_before_consuming_source() {
        let (mut frame, mut store, recipe, binding, destination, _) = original_string_read_frame(5);
        let source = frame.binding(binding).unwrap();
        assert_eq!(store.budget().handles(), 4);
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.budget().handles(),
                store.budget().bytes()
            ),
            before
        );
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.operations_completed, 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_string_local_read_fuel_refusal_keeps_one_original_owner() {
        let (mut frame, mut store, recipe, binding, destination, _) =
            original_string_read_frame(1000);
        let source = frame.binding(binding).unwrap();
        frame.set_fuel_limit(0).unwrap();
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.budget().handles(),
                store.budget().bytes()
            ),
            before
        );
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.started_by_opcode[8], 0);
        assert_eq!(frame.fuel_snapshot().used, 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_string_local_read_destination_mismatch_refuses_before_move_fuel() {
        let (mut frame, mut store, recipe, binding, destination, _) =
            original_string_read_frame(1000);
        let source = frame.binding(binding).unwrap();
        frame.slots[destination.0 as usize].hint = Some(TypeId(6));
        frame.slots[destination.0 as usize].strict_hint = true;
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.budget().handles(),
                store.budget().bytes()
            ),
            before
        );
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.started_by_opcode[8], 0);
        assert_eq!(frame.fuel_snapshot().used, 0);
        store.reset(2).unwrap();
    }
    struct PendingMutableParameter {
        frame: Frame,
        store: GuestStore,
        recipe: crate::ir::recipe::VerifiedRecipe,
        packet: super::super::call::ArgumentPacket,
        ty: TypeId,
        target: CellId,
        shared: crate::runtime::cell::RefId,
        destination: SlotId,
    }
    fn original_mutable_parameter_arguments() -> PendingMutableParameter {
        use crate::runtime::{place::Place, reference::Access};
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-move-mut-base-ptr.rs"
        );
        let range = crate::syntax::place_fixture_facts::mutable_parameter_move(source);
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-mutable-parameter-move"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "foo")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|node| {
                node.range == range && node.role == NodeRole::Expression(ExpressionKind::Path)
            })
            .unwrap()
            .clone();
        let entry = body.pc(node.id, crate::ir::pc::Phase::Entry).unwrap();
        let recipe = parsed.initial_thunk(handle, &body, &entry).unwrap();
        let [
            crate::ir::op::Op::ReadParameter {
                ordinal,
                destination,
            },
        ] = recipe.recipe().operations.as_ref()
        else {
            panic!("exact original parameter read absent")
        };
        assert_eq!(*ordinal, 0);
        let destination = *destination;
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        let target = store
            .new_cell(
                StorageContract::Exact(TypeId(8)),
                true,
                CellOwner::FrameSlot {
                    frame: frame.anchor,
                    slot: 100_000,
                },
            )
            .unwrap();
        let value = store
            .ticket(ValueRepr::signed(TypeId(8), 0).unwrap())
            .unwrap();
        store.initialize(target, value).unwrap();
        let mutable = store
            .borrow_ticket(&Place::cell(target), Access::Mutable)
            .unwrap();
        let ty = TypeId(store.peek_ticket(mutable).unwrap().type_id);
        let argument = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot {
                    frame: frame.anchor,
                    slot: 100_001,
                },
            )
            .unwrap();
        store.initialize(argument, mutable).unwrap();
        let packet =
            super::super::call::ArgumentPacket::seal(&mut store, &[argument], &[ty]).unwrap();
        // Disclosed caller setup supplies the written closed primitive-reference
        // signature through the real checked source installer, without generic
        // inference, prior reborrow statement or ordinary method execution.
        frame
            .install_parameter_sources(vec![crate::traits::term::TyTerm::Ref(
                crate::traits::term::RegionTerm::Local {
                    definition: frame.body.body().definition,
                    start: range.start,
                    end: range.end,
                },
                true,
                Box::new(crate::traits::term::TyTerm::Primitive(
                    crate::ir::op::PrimitiveType::Isize,
                )),
            )])
            .unwrap();
        let shared = store.borrow(&Place::cell(target), Access::Shared).unwrap();
        PendingMutableParameter {
            frame,
            store,
            recipe,
            packet,
            ty,
            target,
            shared,
            destination,
        }
    }
    fn original_mutable_parameter_read() -> (
        Frame,
        GuestStore,
        crate::ir::recipe::VerifiedRecipe,
        CellId,
        crate::runtime::cell::RefId,
        SlotId,
    ) {
        let PendingMutableParameter {
            mut frame,
            mut store,
            recipe,
            packet,
            ty,
            target,
            shared,
            destination,
        } = original_mutable_parameter_arguments();
        frame
            .initialize_parameters(&packet, &[(ty, false)], &mut store)
            .unwrap();
        let node = &frame.body.nodes()[recipe.recipe().key.node as usize];
        let mut scopes = Vec::new();
        let mut scope = Some(node.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = frame.body.scopes()[id.0 as usize].parent;
        }
        for id in scopes.into_iter().rev() {
            frame.enter(id).unwrap();
        }
        (frame, store, recipe, target, shared, destination)
    }
    #[test]
    fn rustc_original_reference_parameter_scalar_guard_refuses_before_owning_read() {
        // Component admission setup uses the unchanged upstream reference
        // parameter witness. It does not claim full upstream/main execution or
        // integration of reference argument preparation in RunMachine.
        let PendingMutableParameter {
            mut frame,
            mut store,
            packet,
            ty,
            target,
            ..
        } = original_mutable_parameter_arguments();
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-move-mut-base-ptr.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-mutable-parameter-move"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let owner = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "foo")
            .unwrap();
        let signature = parsed
            .function_signature(
                owner.def_key.fingerprint(),
                &owner.header,
                &crate::syntax::outline::DeclarationContext {
                    scope: owner.scope.clone(),
                    range: owner.declaration_span,
                },
                owner.body.as_ref(),
            )
            .unwrap();
        assert_eq!(signature.definition, frame.body.body().definition);
        let source_result = signature.result_symbolic.clone();
        let result_descriptor = match &source_result {
            Some(crate::traits::term::TyTerm::Primitive(p)) => {
                Some(crate::frontend::call_contracts::primitive_runtime_id(*p).unwrap())
            }
            _ => None,
        };
        // Disclosed component contract construction: the actual parameter
        // batch, written reference source and concrete descriptor come from the
        // existing original-source ownership helper; no callable is fabricated.
        let contracts = crate::frontend::call_contracts::PreparedCallContracts {
            definition: signature.definition,
            source_parameters: frame.parameter_sources.clone(),
            parameter_descriptors: vec![ty],
            source_substitution: Default::default(),
            runtime_substitution: Default::default(),
            source_result,
            result_descriptor,
            normalized_deferred_return: None,
        };
        frame
            .install_context(
                super::super::context::GenericEnv::capture(&signature, &contracts).unwrap(),
            )
            .unwrap();
        frame
            .initialize_parameters(&packet, &[(ty, false)], &mut store)
            .unwrap();
        let parameter = frame.parameters[0].unwrap();
        let before = (
            store.state(parameter).unwrap(),
            store.location_epoch(parameter).unwrap(),
            store.location_epoch(target).unwrap(),
            store.budget().handles(),
            frame.fuel_snapshot().used,
            frame.counters.operations_started,
        );
        let error = frame.definition_parameter_primitive(0).unwrap_err();
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(
            error.message,
            "scalar parameter context requires an admitted primitive source"
        );
        assert_eq!(
            before,
            (
                store.state(parameter).unwrap(),
                store.location_epoch(parameter).unwrap(),
                store.location_epoch(target).unwrap(),
                store.budget().handles(),
                frame.fuel_snapshot().used,
                frame.counters.operations_started,
            )
        );
        assert_eq!(frame.counters.started_by_opcode[21], 0);
        assert!(frame.active_scopes.is_empty());
    }
    #[test]
    fn rustc_original_mutable_parameter_read_moves_lease_without_invalidating_referent() {
        let (mut frame, mut store, recipe, target, shared, destination) =
            original_mutable_parameter_read();
        let source = frame.parameter(0).unwrap();
        let original = store.state(source).unwrap();
        let epoch = store.location_epoch(source).unwrap();
        let target_epoch = store.location_epoch(target).unwrap();
        let written = frame.parameter_sources[0].clone();
        assert!(matches!(
            crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap(),
            crate::vm::step::StepOutcome::Next(_)
        ));
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        assert_eq!(store.location_epoch(source).unwrap(), epoch + 1);
        let result = frame.cell(destination).unwrap();
        assert_eq!(store.state(result).unwrap(), original);
        assert_eq!(frame.source_view(destination).unwrap(), Some(written));
        let lease = store.reference_from_cell(result).unwrap();
        assert_eq!(store.location_epoch(target).unwrap(), target_epoch);
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-move-mut-base-ptr.rs"
        );
        let (_, assigned) = crate::syntax::place_fixture_facts::assignment(source, "foo");
        let value = store
            .ticket(ValueRepr::signed(TypeId(8), assigned).unwrap())
            .unwrap();
        let old = store.replace_reference(lease, value).unwrap();
        store.discard(old).unwrap();
        assert_eq!(
            store.read_reference(shared).unwrap().payload as i64,
            assigned
        );
        assert_eq!(frame.counters.started_by_opcode[21], 1);
        assert_eq!(
            frame
                .preflight_parameter(0, destination, &store)
                .unwrap_err()
                .code,
            "LRR-E-USE-AFTER-MOVE"
        );
        store.reset(2).unwrap();
        // Direct replacement checks the original next assignment value, not
        // full execution of t1 binding, that assignment or p.use_ref.
    }
    #[test]
    fn rustc_original_mutable_parameter_fuel_refusal_preserves_lease_owner() {
        let (mut frame, mut store, recipe, target, shared, destination) =
            original_mutable_parameter_read();
        let source = frame.parameter(0).unwrap();
        frame.set_fuel_limit(0).unwrap();
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.location_epoch(target).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.location_epoch(target).unwrap(),
                store.budget().handles()
            ),
            before
        );
        store.reference_from_cell(source).unwrap();
        assert_eq!(store.read_reference(shared).unwrap().payload, 0);
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.started_by_opcode[21], 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_mutable_parameter_source_installer_accounts_for_owned_target_box() {
        let (frame, mut store, _, _, _, _) = original_mutable_parameter_read();
        let before = Frame::metadata_bytes(&frame.body, 0).unwrap();
        let estimate =
            Frame::metadata_bytes(&frame.body, frame.parameter_metadata_count()).unwrap();
        let owned = frame.parameters.capacity() * std::mem::size_of::<Option<CellId>>()
            + frame.parameter_sources.capacity()
                * std::mem::size_of::<crate::traits::term::TyTerm>()
            + std::mem::size_of::<crate::traits::term::TyTerm>();
        assert!(estimate.checked_sub(before).unwrap() >= owned);
        assert_eq!(frame.counters.operations_started, 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_mutable_parameter_wrong_source_access_refuses_before_read_fuel() {
        let (mut frame, mut store, recipe, target, shared, destination) =
            original_mutable_parameter_read();
        let crate::traits::term::TyTerm::Ref(_, mutable, _) = &mut frame.parameter_sources[0]
        else {
            panic!("written reference source absent")
        };
        *mutable = false;
        let source = frame.parameter(0).unwrap();
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.location_epoch(target).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(
            crate::vm::step::step(&mut frame, &mut store, &recipe)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.location_epoch(target).unwrap(),
                store.budget().handles()
            ),
            before
        );
        store.reference_from_cell(source).unwrap();
        assert_eq!(store.read_reference(shared).unwrap().payload, 0);
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(frame.counters.started_by_opcode[21], 0);
        assert_eq!(frame.fuel_snapshot().used, 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_mutable_parameter_source_mismatch_refuses_before_argument_delivery() {
        enum Mismatch {
            Access,
            Identity,
        }
        for mismatch in [Mismatch::Access, Mismatch::Identity] {
            let mut pending = original_mutable_parameter_arguments();
            let crate::traits::term::TyTerm::Ref(_, mutable, target) =
                &mut pending.frame.parameter_sources[0]
            else {
                panic!("source reference absent")
            };
            match mismatch {
                Mismatch::Access => *mutable = false,
                // Same runtime width is insufficient: isize and i64 differ.
                Mismatch::Identity => {
                    **target =
                        crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::I64)
                }
            }
            let before = (
                pending.store.state(pending.target).unwrap(),
                pending.store.location_epoch(pending.target).unwrap(),
                pending.store.budget().handles(),
                pending.store.budget().bytes(),
            );
            assert_eq!(
                pending
                    .frame
                    .initialize_parameters(
                        &pending.packet,
                        &[(pending.ty, false)],
                        &mut pending.store
                    )
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            pending
                .packet
                .check_types(&pending.store, &[pending.ty])
                .unwrap();
            assert_eq!(
                (
                    pending.store.state(pending.target).unwrap(),
                    pending.store.location_epoch(pending.target).unwrap(),
                    pending.store.budget().handles(),
                    pending.store.budget().bytes()
                ),
                before
            );
            assert!(!pending.frame.parameters_initialized);
            assert!(pending.frame.parameters.is_empty());
            assert!(pending.frame.active_scopes().is_empty());
            assert_eq!(pending.frame.counters.operations_started, 0);
            pending.store.read_reference(pending.shared).unwrap();
            pending.store.reset(2).unwrap();
        }
    }
    #[test]
    fn rustc_bound_literal_parameter_read_uses_effective_signature_identity() {
        use crate::traits::term::TyTerm;
        let source =
            include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/moves/move-arg.rs");
        let expected = crate::syntax::place_fixture_facts::scalar_copy_binding(source);
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-bound-parameter-literal"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "test")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let macro_node = body
            .nodes()
            .iter()
            .find(|node| !node.runtime_scalar_slots.is_empty())
            .unwrap()
            .clone();
        let destination = macro_node.runtime_scalar_slots[0];
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        // Direct parameter-read adaptation uses the original assert_eq operand
        // inventory. Original macro expansion/call/main execution is unclaimed.
        let ticket = store
            .integer_literal(
                expected < 0,
                u128::from(expected.unsigned_abs()),
                expected.to_string(),
            )
            .unwrap();
        let concrete = store.materialize(ticket, TypeId(8)).unwrap();
        assert_eq!(concrete, ValueRepr::signed(TypeId(8), expected).unwrap());
        let representation = store.peek_ticket(ticket).unwrap();
        assert_eq!((representation.kind, representation.type_id), (11, 0));
        let argument = store
            .new_cell(
                StorageContract::Exact(TypeId(8)),
                false,
                CellOwner::FrameSlot {
                    frame: frame.anchor,
                    slot: 100_000,
                },
            )
            .unwrap();
        store.initialize(argument, ticket).unwrap();
        let packet =
            super::super::call::ArgumentPacket::seal(&mut store, &[argument], &[TypeId(8)])
                .unwrap();
        frame
            .install_parameter_sources(vec![TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize)])
            .unwrap();
        frame
            .initialize_parameters(&packet, &[(TypeId(8), false)], &mut store)
            .unwrap();
        let mut scopes = Vec::new();
        let mut scope = Some(macro_node.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = frame.body.scopes()[id.0 as usize].parent;
        }
        for id in scopes.into_iter().rev() {
            frame.enter(id).unwrap();
        }
        let parameter = frame.parameter(0).unwrap();
        let before = (
            store.state(parameter).unwrap(),
            store.location_epoch(parameter).unwrap(),
        );
        frame.preflight_parameter(0, destination, &store).unwrap();
        frame.read_parameter(0, destination, &mut store).unwrap();
        assert_eq!(
            store
                .preview_scalar(frame.cell(destination).unwrap(), None)
                .unwrap(),
            concrete
        );
        assert_eq!(
            (
                store.state(parameter).unwrap(),
                store.location_epoch(parameter).unwrap()
            ),
            before
        );
        assert_eq!(
            frame.source_view(destination).unwrap(),
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize))
        );
        assert_eq!(frame.counters.operations_started, 0);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_universal_region_cannot_enter_slot_parameter_or_binding_views() {
        use crate::{
            ir::op::{HintRecipe, PrimitiveType},
            syntax::outline::DeclarationContext,
            traits::{
                subst::{Substitution, instantiate_binder},
                term::{Binder, GenericArg, ParamKey, RegionTerm, TyTerm},
            },
        };
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-outlives-projection-hrtype.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(original.as_bytes()),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let alias = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "FnType")
            .unwrap();
        let definition = alias.def_key.fingerprint();
        let recipe = parsed
            .type_alias_recipe(
                definition,
                &alias.header,
                &DeclarationContext {
                    scope: alias.scope.clone(),
                    range: alias.declaration_span,
                },
            )
            .unwrap();
        let mut substitution = Substitution::default();
        substitution
            .insert(
                ParamKey {
                    definition,
                    ordinal: recipe.generics[0].ordinal,
                },
                GenericArg::Type(TyTerm::Primitive(PrimitiveType::Unit)),
            )
            .unwrap();
        let TyTerm::Function(function) = substitution
            .apply(recipe.symbolic.as_ref().unwrap())
            .unwrap()
        else {
            panic!("original function pointer absent")
        };
        let reference = instantiate_binder(
            &Binder {
                variables: function.variables.clone(),
                value: function.value.parameters[0].clone(),
            },
            Digest::of_bytes(original.as_bytes()),
            1,
        )
        .unwrap();
        let TyTerm::Ref(RegionTerm::Placeholder(key), mutable, target) = &reference else {
            panic!("original universal region absent")
        };
        assert_eq!(target.as_ref(), &TyTerm::Primitive(PrimitiveType::Unit));
        let key = *key;
        let mutable = *mutable;
        let target = target.clone();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let destination = body.nodes()[0].value.unwrap();
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 1_000, 32 * 1024 * 1024).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        let stable = TyTerm::Ref(RegionTerm::Static, mutable, target.clone());
        frame
            .set_source_view(destination, Some(stable.clone()))
            .unwrap();
        frame
            .set_binding_source(NodeId(0), &HintRecipe::Primitive(PrimitiveType::Unit))
            .unwrap();
        let before = (
            frame.source_view(destination).unwrap(),
            frame.binding_sources[0].clone(),
            frame.counters.operations_started,
            store.budget().handles(),
            frame.active_scopes.clone(),
        );
        assert_eq!(
            frame
                .set_source_view(destination, Some(reference.clone()))
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            frame
                .install_parameter_sources(vec![reference])
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            frame
                .set_binding_source(
                    NodeId(0),
                    &HintRecipe::Reference {
                        region: RegionTerm::Placeholder(key),
                        mutable,
                        target: PrimitiveType::Unit,
                    }
                )
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            before,
            (
                frame.source_view(destination).unwrap(),
                frame.binding_sources[0].clone(),
                frame.counters.operations_started,
                store.budget().handles(),
                frame.active_scopes.clone()
            )
        );
        assert!(frame.parameter_sources.is_empty());
        frame.install_parameter_sources(vec![]).unwrap();
        frame
            .set_source_view(destination, Some(stable.clone()))
            .unwrap();
        assert_eq!(frame.source_view(destination).unwrap(), Some(stable));
        // Source-derived metadata corruption guard only: the original's
        // projection/outlives/check-pass/main execution remains unclaimed.
    }
    #[test]
    fn rustc_array_reference_metadata_accounts_for_both_owned_source_boxes() {
        use crate::traits::term::{ConstTerm, RegionTerm, TyTerm};
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/unconditional_panic_98444.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-array-reference-metadata"),
                vec![],
                &CfgSet::guest_default(),
            )
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
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 100_000, 32 * 1024 * 1024).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        let view = TyTerm::Ref(
            RegionTerm::Static,
            false,
            Box::new(TyTerm::Array(
                Box::new(TyTerm::Primitive(crate::ir::op::PrimitiveType::I32)),
                ConstTerm::Value(
                    u64::try_from(crate::syntax::place_fixture_facts::array_binding(source).len())
                        .unwrap(),
                ),
            )),
        );
        // Supplemental resource saturation uses the original body's real
        // inventory, without executing wildcard statements or a stock oracle.
        for ordinal in 0..frame.slots.len() {
            frame
                .set_source_view(SlotId(u32::try_from(ordinal).unwrap()), Some(view.clone()))
                .unwrap();
        }
        for source in &mut frame.binding_sources {
            *source = Some(view.clone());
        }
        frame.return_source = Some(view);
        fn owned_boxes(source: &TyTerm) -> usize {
            match source {
                TyTerm::Ref(_, _, target) | TyTerm::Array(target, _) => {
                    std::mem::size_of_val(target.as_ref()) + owned_boxes(target)
                }
                TyTerm::Primitive(_) => 0,
                _ => panic!("unexpected resource-control source"),
            }
        }
        let retained = frame.slots.capacity() * std::mem::size_of::<Slot>()
            + frame.bindings.capacity() * std::mem::size_of::<Option<CellId>>()
            + frame.binding_sources.capacity() * std::mem::size_of::<Option<TyTerm>>()
            + frame.active_scopes.capacity() * std::mem::size_of::<ScopeId>()
            + frame
                .slots
                .iter()
                .map(|slot| owned_boxes(slot.source.as_ref().unwrap()))
                .sum::<usize>()
            + frame
                .binding_sources
                .iter()
                .map(|source| owned_boxes(source.as_ref().unwrap()))
                .sum::<usize>()
            + owned_boxes(frame.return_source.as_ref().unwrap());
        assert!(Frame::metadata_bytes(&frame.body, 0).unwrap() >= retained);
        assert_eq!(frame.counters.operations_started, 0);
    }

    fn stock_unary_tuple() -> (
        ParsedFile,
        crate::syntax::outline::BodyHandle,
        BodyIndex,
        crate::syntax::constructors::ConstructorPlan,
    ) {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/one-tuple.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-frame-constructor"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let body = parsed.index_body(&handle, Default::default()).unwrap();
        let plan = body
            .nodes()
            .iter()
            .filter_map(|node| {
                parsed
                    .constructor_plan(&handle, &body, node.id, &CfgSet::guest_default())
                    .unwrap()
            })
            .rfind(|plan| {
                matches!(
                    plan.kind(),
                    crate::syntax::constructors::ConstructorKind::Tuple(_)
                )
            })
            .unwrap();
        (parsed, handle, body, plan)
    }
    fn enter_constructor_scope(frame: &mut Frame, guard: SlotId) {
        let mut scopes = Vec::new();
        let mut next = Some(frame.slots[guard.0 as usize].scope);
        while let Some(scope) = next {
            scopes.push(scope);
            next = frame.body.scopes()[scope.0 as usize].parent;
        }
        for scope in scopes.into_iter().rev() {
            frame.enter(scope).unwrap();
        }
    }
    #[test]
    fn rustc_unhinted_tuple_observes_original_char_only_on_completed_field_and_commits_once() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        let (parsed, handle, body, _) = stock_unary_tuple();
        // Select the original unannotated match scrutinee ('c',), not the later
        // explicitly typed let. Match/pattern/assert execution remains cold.
        let plan = body
            .nodes()
            .iter()
            .filter_map(|node| {
                parsed
                    .constructor_plan(&handle, &body, node.id, &CfgSet::guest_default())
                    .unwrap()
            })
            .find(|plan| matches!(plan.kind(), ConstructorKind::Tuple(_)))
            .unwrap();
        let ConstructorKind::Tuple(fields) = plan.kind() else {
            unreachable!()
        };
        assert_eq!(fields.len(), 1);
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let key = TypeKey::Tuple(Box::new([TypeId(3)]));
        assert!(store.find_type(&key).is_none());
        let mut frame = Frame::new(
            body.clone(),
            body.pc(plan.node(), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .prepare_observed_structural_constructor(&plan)
            .unwrap();
        frame.start_prepared_constructor(guard, &mut store).unwrap();
        let owned = frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .guard;
        assert!(store.observed_construction_key(owned).is_err());
        assert!(store.find_type(&key).is_none());
        let entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let literal = parsed.initial_thunk(&handle, &body, &entry).unwrap();
        frame.pc = entry;
        crate::vm::step::step(&mut frame, &mut store, &literal).unwrap();
        let source = frame.cell(fields[0].value).unwrap();
        let actual = store.read_scalar(source).unwrap();
        assert_eq!(actual.type_id, 3);
        frame
            .constructor_field(guard, 0, fields[0].value, &mut store)
            .unwrap();
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        assert_eq!(store.observed_construction_key(owned).unwrap(), key);
        assert!(store.find_type(&key).is_none());
        // A conflicting destination refuses before a new type, object or owner
        // transfer; the completed field remains owned by the exact guard.
        frame.declared_hint(destination, Some(TypeId(2))).unwrap();
        let handles = store.budget().handles();
        let types = store.type_metadata_bytes();
        assert_eq!(
            frame
                .finish_constructor(guard, destination, &mut store)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(store.type_metadata_bytes(), types);
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        assert_eq!(store.observed_construction_key(owned).unwrap(), key);
        frame.set_hint(destination, None).unwrap();
        frame
            .finish_constructor(guard, destination, &mut store)
            .unwrap();
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        assert!(
            frame
                .finish_constructor(guard, destination, &mut store)
                .is_err()
        );
        let tuple = store.find_type(&key).unwrap();
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        assert_eq!(store.peek_ticket(ticket).unwrap().type_id, tuple.0);
        let field = store.aggregate_field(ticket, 0).unwrap();
        assert_eq!(store.read_scalar(field).unwrap(), actual);
        let crate::runtime::cell::AggregateCleanupTransfer::Field {
            cell,
            ticket: child,
        } = store.aggregate_cleanup_transfer(ticket).unwrap()
        else {
            panic!("observed original field absent")
        };
        assert_eq!(cell, field);
        store.discard(child).unwrap();
        store
            .finish_aggregate_cleanup_field(ticket, cell, child)
            .unwrap();
        assert!(matches!(
            store.aggregate_cleanup_transfer(ticket).unwrap(),
            crate::runtime::cell::AggregateCleanupTransfer::Complete
        ));
        store.discard(ticket).unwrap();
        // Original character thunk ran once; full stock match/main is not claimed.
        assert_eq!(frame.counters.started_by_opcode[0], 1);
    }
    fn stock_observed_array(
        ordinal: usize,
    ) -> (
        ParsedFile,
        crate::syntax::outline::BodyHandle,
        BodyIndex,
        crate::syntax::constructors::ConstructorPlan,
    ) {
        let source = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/fixed-size-arrays-zero-size-types-8898.rs"
        );
        stock_source_array_constructor(source, ordinal)
    }
    fn stock_source_array_constructor(
        source: &str,
        ordinal: usize,
    ) -> (
        ParsedFile,
        crate::syntax::outline::BodyHandle,
        BodyIndex,
        crate::syntax::constructors::ConstructorPlan,
    ) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"rustc-observed-array"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let body = parsed.index_body(&handle, Default::default()).unwrap();
        let plan = body
            .nodes()
            .iter()
            .filter_map(|node| {
                parsed
                    .constructor_plan(&handle, &body, node.id, &CfgSet::guest_default())
                    .unwrap()
            })
            .filter(|plan| {
                matches!(
                    plan.kind(),
                    crate::syntax::constructors::ConstructorKind::Array(_)
                )
            })
            .nth(ordinal)
            .unwrap();
        (parsed, handle, body, plan)
    }
    #[test]
    fn rustc_unhinted_bool_array_observes_original_fields_and_reserves_guard_before_transfer() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        // Original second array initializer is [true, false]. Generic formatting
        // calls and slice operations in the rest of the stock program stay cold.
        let (parsed, handle, body, plan) = stock_observed_array(1);
        let ConstructorKind::Array(fields) = plan.kind() else {
            unreachable!()
        };
        assert_eq!(fields.len(), 2);
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        // The frame anchor consumes the only allowed handle. Guard reservation
        // refuses before any field entry, type publication, fuel or owner change.
        let mut limited = GuestStore::new(1, 1, 1_000_000).unwrap();
        let mut refused = Frame::new(
            body.clone(),
            body.pc(plan.node(), Phase::Entry).unwrap(),
            &mut limited,
        )
        .unwrap();
        enter_constructor_scope(&mut refused, guard);
        refused
            .prepare_observed_structural_constructor(&plan)
            .unwrap();
        let before = (
            limited.budget().handles(),
            limited.type_metadata_bytes(),
            refused.fuel.snapshot().used,
        );
        assert_eq!(
            refused
                .start_prepared_constructor(guard, &mut limited)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(
            (
                limited.budget().handles(),
                limited.type_metadata_bytes(),
                refused.fuel.snapshot().used
            ),
            before
        );
        assert!(refused.slots[guard.0 as usize].construction.is_none());
        assert!(refused.slots[destination.0 as usize].cell.is_none());
        assert_eq!(refused.counters.operations_started, 0);

        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(plan.node(), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .prepare_observed_structural_constructor(&plan)
            .unwrap();
        frame.start_prepared_constructor(guard, &mut store).unwrap();
        let owned = frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .guard;
        let types = store.type_metadata_bytes();
        let mut original = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            assert!(store.observed_construction_key(owned).is_err());
            let entry = body.pc(field.node, Phase::Entry).unwrap();
            let thunk = parsed.initial_thunk(&handle, &body, &entry).unwrap();
            frame.pc = entry;
            crate::vm::step::step(&mut frame, &mut store, &thunk).unwrap();
            let source = frame.cell(field.value).unwrap();
            original.push(store.read_scalar(source).unwrap());
            frame
                .constructor_field(
                    guard,
                    u32::try_from(index).unwrap(),
                    field.value,
                    &mut store,
                )
                .unwrap();
            assert_eq!(store.state(source).unwrap(), CellState::Moved);
            assert_eq!(store.type_metadata_bytes(), types);
        }
        let element = TypeId(original[0].type_id);
        assert!(original.iter().all(|value| value.type_id == element.0));
        let key = TypeKey::Array {
            element,
            len: u32::try_from(fields.len()).unwrap(),
        };
        assert_eq!(store.observed_construction_key(owned).unwrap(), key);
        assert!(store.find_type(&key).is_none());
        frame
            .finish_constructor(guard, destination, &mut store)
            .unwrap();
        let array = store.find_type(&key).unwrap();
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        assert_eq!(store.peek_ticket(ticket).unwrap().type_id, array.0);
        // Capture immutable declaration identities while the aggregate is
        // whole. Once cleanup starts, aggregate_field deliberately refuses
        // fresh traversal through partially cleaned storage.
        let completed_fields: Vec<_> = (0..original.len())
            .map(|index| {
                store
                    .aggregate_field(ticket, u32::try_from(index).unwrap())
                    .unwrap()
            })
            .collect();
        for (field, value) in completed_fields.iter().zip(&original) {
            assert_eq!(store.read_scalar(*field).unwrap(), *value);
        }
        for field in completed_fields {
            let crate::runtime::cell::AggregateCleanupTransfer::Field {
                cell,
                ticket: child,
            } = store.aggregate_cleanup_transfer(ticket).unwrap()
            else {
                panic!("original array field absent")
            };
            assert_eq!(cell, field);
            store.discard(child).unwrap();
            store
                .finish_aggregate_cleanup_field(ticket, cell, child)
                .unwrap();
        }
        assert!(matches!(
            store.aggregate_cleanup_transfer(ticket).unwrap(),
            crate::runtime::cell::AggregateCleanupTransfer::Complete
        ));
        store.discard(ticket).unwrap();
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        assert_eq!(
            frame.counters.started_by_opcode[0],
            u64::try_from(fields.len()).unwrap()
        );
    }
    #[test]
    fn rustc_unhinted_numeric_array_preserves_original_unresolved_literal_without_defaulting() {
        use crate::{ir::pc::Phase, syntax::constructors::ConstructorKind};
        let (parsed, handle, body, plan) = stock_observed_array(0);
        let ConstructorKind::Array(fields) = plan.kind() else {
            unreachable!()
        };
        assert_eq!(fields.len(), 3);
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(plan.node(), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .prepare_observed_structural_constructor(&plan)
            .unwrap();
        frame.start_prepared_constructor(guard, &mut store).unwrap();
        let owned = frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .guard;
        let entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let thunk = parsed.initial_thunk(&handle, &body, &entry).unwrap();
        frame.pc = entry;
        crate::vm::step::step(&mut frame, &mut store, &thunk).unwrap();
        let source = frame.cell(fields[0].value).unwrap();
        let value = store.admit_move(source).unwrap();
        assert_eq!(store.effective_type(value).unwrap(), None);
        let before = (
            store.state(source).unwrap(),
            store.location_epoch(source).unwrap(),
            store.budget().handles(),
            store.type_metadata_bytes(),
        );
        assert_eq!(
            frame
                .constructor_field(guard, 0, fields[0].value, &mut store)
                .unwrap_err()
                .code,
            "LRR-E-TYPE-CONTEXT"
        );
        assert_eq!(
            (
                store.state(source).unwrap(),
                store.location_epoch(source).unwrap(),
                store.budget().handles(),
                store.type_metadata_bytes()
            ),
            before
        );
        assert_eq!(store.effective_type(value).unwrap(), None);
        assert!(store.observed_construction_key(owned).is_err());
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        // No completed field means no field cleanup obligation. The unresolved
        // first literal remains in its original cell; the later operands never run.
        assert!(
            frame
                .constructor_cleanup_next(guard, &mut store)
                .unwrap()
                .is_none()
        );
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        assert_eq!(frame.counters.started_by_opcode[0], 1);
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_original_array_suffix_binds_later_unresolved_field_after_reached_context() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        const SOURCE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/issue-69841.rs"
        );
        let (parsed, handle, body, plan) = stock_source_array_constructor(SOURCE, 0);
        let ConstructorKind::Array(fields) = plan.kind() else {
            unreachable!()
        };
        assert_eq!(fields.len(), 2);
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(plan.node(), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .prepare_observed_structural_constructor(&plan)
            .unwrap();
        frame.start_prepared_constructor(guard, &mut store).unwrap();
        let owned = frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .guard;
        let types = store.type_metadata_bytes();
        let first_entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let thunk = parsed.initial_thunk(&handle, &body, &first_entry).unwrap();
        frame.pc = first_entry;
        crate::vm::step::step(&mut frame, &mut store, &thunk).unwrap();
        let first_cell = frame.cell(fields[0].value).unwrap();
        let first = store.read_scalar(first_cell).unwrap();
        let element = TypeId(first.type_id);
        frame
            .constructor_field(guard, 0, fields[0].value, &mut store)
            .unwrap();
        assert_eq!(store.state(first_cell).unwrap(), CellState::Moved);
        assert!(store.observed_construction_key(owned).is_err());
        let second_entry = body.pc(fields[1].node, Phase::Entry).unwrap();
        let thunk = parsed.initial_thunk(&handle, &body, &second_entry).unwrap();
        frame.pc = second_entry;
        crate::vm::step::step(&mut frame, &mut store, &thunk).unwrap();
        let second_cell = frame.cell(fields[1].value).unwrap();
        let unresolved = store.admit_move(second_cell).unwrap();
        assert_eq!(store.effective_type(unresolved).unwrap(), None);
        let (negative, magnitude) = store.integer_parts(unresolved).unwrap().unwrap();
        assert!(!negative);
        // Only the completed suffixed first field supplies this context.
        // No default or future-use scan chooses the element descriptor.
        frame
            .constructor_field(guard, 1, fields[1].value, &mut store)
            .unwrap();
        assert_eq!(store.state(second_cell).unwrap(), CellState::Moved);
        assert_eq!(store.effective_type(unresolved).unwrap(), Some(element));
        assert_eq!(store.type_metadata_bytes(), types);
        let key = TypeKey::Array {
            element,
            len: u32::try_from(fields.len()).unwrap(),
        };
        assert_eq!(store.observed_construction_key(owned).unwrap(), key);
        assert!(store.find_type(&key).is_none());
        frame
            .finish_constructor(guard, destination, &mut store)
            .unwrap();
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        let first_field = store.aggregate_field(ticket, 0).unwrap();
        let second_field = store.aggregate_field(ticket, 1).unwrap();
        assert_eq!(store.read_scalar(first_field).unwrap(), first);
        let second = store.read_scalar(second_field).unwrap();
        assert_eq!(second.type_id, element.0);
        assert_eq!(second.payload, u64::try_from(magnitude).unwrap());
        for expected in [first_field, second_field] {
            let crate::runtime::cell::AggregateCleanupTransfer::Field {
                cell,
                ticket: child,
            } = store.aggregate_cleanup_transfer(ticket).unwrap()
            else {
                panic!("original suffixed array field absent")
            };
            assert_eq!(cell, expected);
            store.discard(child).unwrap();
            store
                .finish_aggregate_cleanup_field(ticket, cell, child)
                .unwrap();
        }
        assert!(matches!(
            store.aggregate_cleanup_transfer(ticket).unwrap(),
            crate::runtime::cell::AggregateCleanupTransfer::Complete
        ));
        store.discard(ticket).unwrap();
        assert_eq!(frame.counters.started_by_opcode[0], 2);
        // Original loop/range/unsafe/LLVM behavior stays cold; only its exact
        // array operands and reached context are exercised by this component.
    }
    fn constructor_recipe(
        body: &BodyIndex,
        node: NodeId,
        phase: crate::ir::pc::Phase,
        operations: Vec<crate::ir::op::Op>,
        next: PcKey,
    ) -> Result<crate::ir::recipe::VerifiedRecipe, Diagnostic> {
        crate::ir::validate::verify(
            crate::ir::recipe::RecipeDraft {
                key: body.pc(node, phase)?,
                origin: crate::ir::recipe::Origin {
                    span: body.span(node)?,
                    expansion_ancestry: vec![],
                },
                operations,
                exits: vec![crate::ir::recipe::RecipeExit::Next(next)],
                dependencies: vec![],
            },
            body,
        )
    }
    #[test]
    fn rustc_unary_tuple_panic_cleanup_drains_completed_scalar_field_once() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        let (parsed, handle, body, plan) = stock_unary_tuple();
        let ConstructorKind::Tuple(fields) = plan.kind() else {
            panic!("stock tuple absent")
        };
        let guard = body.nodes()[plan.node().0 as usize]
            .constructor_guard
            .unwrap();
        let entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let literal = parsed.initial_thunk(&handle, &body, &entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let tuple = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .begin_structural_constructor(guard, tuple, &mut store)
            .unwrap();
        crate::vm::step::step(&mut frame, &mut store, &literal).unwrap();
        let source = frame.cell(fields[0].value).unwrap();
        let actual = store.read_scalar(source).unwrap();
        assert_eq!(actual.type_id, 3);
        frame
            .constructor_field(guard, 0, fields[0].value, &mut store)
            .unwrap();
        let construction = frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .guard;
        let handles = store.budget().handles();
        // Inject the LRR panic state after an original operand has transferred;
        // this tests cleanup, not full source execution or a guest destructor.
        frame.status = FrameStatus::Panicked;
        frame.finish_panic(&mut store).unwrap();
        assert_eq!(frame.status(), FrameStatus::PanicCleaned);
        assert!(frame.active_scopes().is_empty());
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        assert_eq!(frame.counters.cleanup_elements_completed, 2);
        assert_eq!(frame.fuel.snapshot().used, 4);
        // Append-only arena tombstones keep their handle charge. The scalar
        // cleanup allocates exactly one transfer ticket.
        assert_eq!(store.budget().handles(), handles + 1);
        assert!(store.construction_cleanup_next(construction).is_err());
        assert!(store.read_scalar(source).is_err());
        let handles = store.budget().handles();
        assert_eq!(
            frame.finish_panic(&mut store).unwrap_err().message,
            "panic cleanup requires an actual guest panic"
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(frame.counters.cleanup_elements_completed, 2);
    }
    #[test]
    fn rustc_unary_tuple_panic_cleanup_retires_empty_guard_without_field_drop() {
        use crate::{ir::pc::Phase, runtime::shape::TypeKey};
        let (_, _, body, plan) = stock_unary_tuple();
        let guard = body.nodes()[plan.node().0 as usize]
            .constructor_guard
            .unwrap();
        let entry = body.pc(plan.node(), Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let tuple = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .begin_structural_constructor(guard, tuple, &mut store)
            .unwrap();
        let handles = store.budget().handles();
        frame.status = FrameStatus::Panicked;
        frame.finish_panic(&mut store).unwrap();
        assert_eq!(frame.status(), FrameStatus::PanicCleaned);
        assert_eq!(frame.counters.cleanup_elements_completed, 0);
        assert_eq!(frame.fuel.snapshot().used, 0);
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        // Retiring an empty guard creates no handle and retains its tombstone.
        assert_eq!(store.budget().handles(), handles);
    }
    #[test]
    fn rustc_unary_tuple_constructor_opcodes_execute_bound_guard_and_original_operand_once() {
        use crate::{
            ir::{pc::Phase, recipe::RecipeExit},
            runtime::shape::TypeKey,
            syntax::constructors::ConstructorKind,
            vm::step::{StepOutcome, step},
        };
        let (parsed, handle, body, plan) = stock_unary_tuple();
        let ConstructorKind::Tuple(fields) = plan.kind() else {
            panic!("stock tuple absent")
        };
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let entry = body.pc(plan.node(), Phase::Entry).unwrap();
        let child = body.pc(fields[0].node, Phase::Entry).unwrap();
        let operand = body.pc(plan.node(), Phase::Operand(0)).unwrap();
        let commit = body.pc(plan.node(), Phase::Commit).unwrap();
        let start = parsed
            .constructor_thunk(&handle, &body, &entry, &plan)
            .unwrap();
        let field = parsed
            .constructor_thunk(&handle, &body, &operand, &plan)
            .unwrap();
        let finish = parsed
            .constructor_thunk(&handle, &body, &commit, &plan)
            .unwrap();
        let RecipeExit::Next(after) = &finish.recipe().exit else {
            panic!("source constructor continuation absent")
        };
        let after = after.clone();
        let literal = parsed.initial_thunk(&handle, &body, &child).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let tuple = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        let handles = store.budget().handles();
        frame
            .prepare_structural_constructor(&plan, tuple, &store)
            .unwrap();
        assert_eq!(store.budget().handles(), handles);
        assert!(
            frame
                .prepare_structural_constructor(&plan, tuple, &store)
                .is_err()
        );
        assert!(
            matches!(step(&mut frame, &mut store, &start).unwrap(), StepOutcome::Next(pc) if pc == child)
        );
        assert!(
            matches!(step(&mut frame, &mut store, &literal).unwrap(), StepOutcome::Next(pc) if pc == operand)
        );
        let source = frame.cell(fields[0].value).unwrap();
        let actual = store.read_scalar(source).unwrap();
        assert!(
            matches!(step(&mut frame, &mut store, &field).unwrap(), StepOutcome::Next(pc) if pc == commit)
        );
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        assert!(
            matches!(step(&mut frame, &mut store, &finish).unwrap(), StepOutcome::Next(pc) if pc == after)
        );
        assert_eq!(frame.counters.operations_started, 4);
        assert_eq!(frame.counters.operations_completed, 4);
        for tag in [0, 27, 28, 29] {
            assert_eq!(frame.counters.started_by_opcode[tag], 1);
        }
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        let field = store.aggregate_field(ticket, 0).unwrap();
        assert_eq!(store.read_scalar(field).unwrap(), actual);
        assert_eq!(store.peek_ticket(ticket).unwrap().type_id, tuple.0);
        let cleanup = store.aggregate_cleanup_next(ticket).unwrap().unwrap();
        store.discard(cleanup).unwrap();
        assert!(store.aggregate_cleanup_next(ticket).unwrap().is_none());
        store.discard(ticket).unwrap();
        // This proves bound LRIR execution on the original unary tuple operand.
        // Complete dispatcher type binding and the full rustc body remain pending.
    }
    #[test]
    fn rustc_constructor_opcode_admission_rejects_value_slot_guards_and_missing_runtime_facts() {
        use crate::{
            ir::{op::Op, pc::Phase},
            vm::step::step,
        };
        let (_, _, body, plan) = stock_unary_tuple();
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let entry = body.pc(plan.node(), Phase::Entry).unwrap();
        let next = body.pc(plan.node(), Phase::Commit).unwrap();
        assert_eq!(
            constructor_recipe(
                &body,
                plan.node(),
                Phase::Entry,
                vec![Op::ConstructStart { guard: destination }],
                next.clone()
            )
            .unwrap_err()
            .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            constructor_recipe(
                &body,
                plan.node(),
                Phase::Entry,
                vec![Op::ConstructFinish {
                    guard,
                    destination: body.nodes()[0].value.unwrap()
                }],
                next.clone()
            )
            .unwrap_err()
            .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            constructor_recipe(
                &body,
                plan.node(),
                Phase::Operand(0),
                vec![Op::ConstructField {
                    guard,
                    field: 0,
                    source: body.nodes()[0].value.unwrap()
                }],
                next.clone()
            )
            .unwrap_err()
            .code,
            "LRR-E-INTERNAL-IR"
        );
        let start = constructor_recipe(
            &body,
            plan.node(),
            Phase::Entry,
            vec![Op::ConstructStart { guard }],
            next,
        )
        .unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        let handles = store.budget().handles();
        assert_eq!(
            step(&mut frame, &mut store, &start).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(frame.counters.started_by_opcode[27], 0);
        assert_eq!(frame.counters.fuel_used, 0);
        assert!(frame.slots[guard.0 as usize].construction.is_none());
    }
    #[test]
    fn rustc_unary_tuple_frame_guard_commits_the_once_executed_source_literal() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        let (parsed, handle, body, plan) = stock_unary_tuple();
        let ConstructorKind::Tuple(fields) = plan.kind() else {
            panic!("stock tuple absent")
        };
        assert_eq!(fields.len(), 1);
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        assert_eq!(body.slot_kind(guard).unwrap(), SlotKind::Guard);
        assert_eq!(body.slot_kind(destination).unwrap(), SlotKind::Value);
        let entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let literal = parsed.initial_thunk(&handle, &body, &entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let tuple = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        assert!(frame.cell(guard).is_err());
        let handles = store.budget().handles();
        assert!(
            frame
                .begin_structural_constructor(destination, tuple, &mut store)
                .is_err()
        );
        assert_eq!(store.budget().handles(), handles);
        frame
            .begin_structural_constructor(guard, tuple, &mut store)
            .unwrap();
        assert!(
            frame
                .begin_structural_constructor(guard, tuple, &mut store)
                .is_err()
        );
        crate::vm::step::step(&mut frame, &mut store, &literal).unwrap();
        let source = frame.cell(fields[0].value).unwrap();
        let actual = store.read_scalar(source).unwrap();
        assert_eq!(actual.type_id, 3);
        assert_eq!(frame.counters.started_by_opcode[0], 1);
        frame
            .constructor_field(guard, 0, fields[0].value, &mut store)
            .unwrap();
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        let handles = store.budget().handles();
        assert!(
            frame
                .finish_constructor(guard, body.nodes()[0].value.unwrap(), &mut store)
                .is_err()
        );
        assert_eq!(store.budget().handles(), handles);
        frame
            .finish_constructor(guard, destination, &mut store)
            .unwrap();
        assert!(frame.slots[guard.0 as usize].construction.is_none());
        let ticket = store.move_from(frame.cell(destination).unwrap()).unwrap();
        assert_eq!(store.peek_ticket(ticket).unwrap().type_id, tuple.0);
        assert_eq!(store.aggregate_variant(ticket).unwrap(), None);
        let field = store.aggregate_field(ticket, 0).unwrap();
        assert_eq!(store.read_scalar(field).unwrap(), actual);
        assert!(store.aggregate_field(ticket, 1).is_err());
        let owned = store.aggregate_cleanup_next(ticket).unwrap().unwrap();
        assert_eq!(store.peek_ticket(owned).unwrap(), actual);
        store.discard(owned).unwrap();
        assert!(store.aggregate_cleanup_next(ticket).unwrap().is_none());
        store.discard(ticket).unwrap();
        // The original char thunk ran once. Match/tuple-pattern execution and
        // the complete stock assertions are not exercised by this kernel test.
    }
    #[test]
    fn rustc_unary_tuple_frame_rejection_and_cleanup_preserve_unique_field_ownership() {
        use crate::{
            ir::pc::Phase, runtime::shape::TypeKey, syntax::constructors::ConstructorKind,
        };
        let (parsed, handle, body, plan) = stock_unary_tuple();
        let ConstructorKind::Tuple(fields) = plan.kind() else {
            panic!("stock tuple absent")
        };
        let node = &body.nodes()[plan.node().0 as usize];
        let guard = node.constructor_guard.unwrap();
        let destination = node.value.unwrap();
        let entry = body.pc(fields[0].node, Phase::Entry).unwrap();
        let literal = parsed.initial_thunk(&handle, &body, &entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let wrong = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(7)])))
            .unwrap();
        let right = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        enter_constructor_scope(&mut frame, guard);
        frame
            .begin_structural_constructor(guard, wrong, &mut store)
            .unwrap();
        crate::vm::step::step(&mut frame, &mut store, &literal).unwrap();
        let source = frame.cell(fields[0].value).unwrap();
        let actual = store.read_scalar(source).unwrap();
        let handles = store.budget().handles();
        assert_eq!(
            frame
                .constructor_field(guard, 0, fields[0].value, &mut store)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(store.state(source).unwrap(), CellState::Live(actual));
        assert_eq!(store.budget().handles(), handles);
        assert!(
            frame
                .constructor_cleanup_next(guard, &mut store)
                .unwrap()
                .is_none()
        );
        frame
            .begin_structural_constructor(guard, right, &mut store)
            .unwrap();
        frame
            .constructor_field(guard, 0, fields[0].value, &mut store)
            .unwrap();
        frame.declared_hint(destination, Some(TypeId(7))).unwrap();
        assert_eq!(
            frame
                .finish_constructor(guard, destination, &mut store)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert!(frame.slots[destination.0 as usize].cell.is_none());
        let scope = frame.slots[guard.0 as usize].scope;
        assert!(frame.preflight_leave(scope).is_err());
        let owned = frame
            .constructor_cleanup_next(guard, &mut store)
            .unwrap()
            .unwrap();
        assert_eq!(store.peek_ticket(owned).unwrap(), actual);
        assert!(
            frame
                .constructor_field(guard, 0, fields[0].value, &mut store)
                .is_err()
        );
        assert!(
            frame
                .finish_constructor(guard, destination, &mut store)
                .is_err()
        );
        store.discard(owned).unwrap();
        assert!(
            frame
                .constructor_cleanup_next(guard, &mut store)
                .unwrap()
                .is_none()
        );
        assert!(frame.constructor_cleanup_next(guard, &mut store).is_err());
        frame.leave(scope, &mut store).unwrap();
        assert_eq!(frame.counters.started_by_opcode[0], 1);
        // Explicit guard cleanup exercises one owned char ticket, not guest
        // panic/Drop dispatch or the full rustc run-pass body.
    }

    #[test]
    fn rigid_local_source_rejection_preserves_values_views_and_fuel() {
        use crate::{
            ir::op::HintRecipe,
            traits::term::{ParamKey, TyTerm},
        };
        let blob = SourceBlob::new(b"fn live<T,U>(){let mut z:T=(1i64,2i64);}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([78; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let binding = body
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Pattern)
            .unwrap()
            .id;
        let values = body
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
            .map(|node| node.value.unwrap())
            .collect::<Vec<_>>();
        let destination = body.nodes()[0].value.unwrap();
        let definition = body.body().definition;
        let t = ParamKey {
            definition,
            ordinal: 0,
        };
        let u = ParamKey {
            definition,
            ordinal: 1,
        };
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
        let statement = body.nodes().iter().find_map(|n| n.statement_scope).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        frame.enter(ScopeId(0)).unwrap();
        frame.enter(statement).unwrap();
        for (slot, value) in values.iter().zip([1, 2]) {
            let ticket = store
                .ticket(ValueRepr::signed(TypeId(7), value).unwrap())
                .unwrap();
            frame.write(*slot, ticket, &mut store).unwrap();
        }
        frame
            .set_source_view(values[0], Some(TyTerm::Param(u)))
            .unwrap();
        let initializer = frame.cell(values[0]).unwrap();
        let before = store.state(initializer).unwrap();
        assert_eq!(
            frame
                .preflight_binding_source(values[0], &HintRecipe::Parameter(t))
                .unwrap_err()
                .code,
            "LRR-E-GENERIC-BINDING"
        );
        assert_eq!(store.state(initializer).unwrap(), before);
        assert!(frame.bindings[binding.0 as usize].is_none());
        frame
            .set_source_view(values[0], Some(TyTerm::Param(t)))
            .unwrap();
        frame
            .bind(binding, values[0], Some(TypeId(7)), true, &mut store)
            .unwrap();
        frame
            .set_binding_source(binding, &HintRecipe::Parameter(t))
            .unwrap();
        frame
            .set_source_view(values[1], Some(TyTerm::Param(u)))
            .unwrap();
        let source = frame.cell(values[1]).unwrap();
        let target = frame.binding(binding).unwrap();
        let source_before = store.state(source).unwrap();
        let target_before = store.state(target).unwrap();
        assert_eq!(
            frame
                .assign_local(binding, values[1], destination, &mut store)
                .unwrap_err()
                .code,
            "LRR-E-GENERIC-BINDING"
        );
        assert_eq!(store.state(source).unwrap(), source_before);
        assert_eq!(store.state(target).unwrap(), target_before);
        assert_eq!(
            frame.source_view(values[1]).unwrap(),
            Some(TyTerm::Param(u))
        );
        assert_eq!(
            frame.binding_sources[binding.0 as usize],
            Some(TyTerm::Param(t))
        );
        assert_eq!(frame.fuel_snapshot().used, 0);
        assert!(frame.slots[destination.0 as usize].cell.is_none());
    }

    #[test]
    fn parameter_cleanup_is_reverse_order_and_preserves_unprocessed_owned_values() {
        let blob = SourceBlob::new(b"fn live(a:String,b:String){}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([83; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame.set_fuel_limit(1).unwrap();
        let mut arguments = Vec::new();
        let mut types = Vec::new();
        for ordinal in 0..2 {
            let ticket = store.string("parameter-owned value").unwrap();
            let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
            let cell = store
                .new_cell(
                    StorageContract::Exact(ty),
                    false,
                    CellOwner::FrameSlot {
                        frame: frame.anchor,
                        slot: 100 + ordinal,
                    },
                )
                .unwrap();
            store.initialize(cell, ticket).unwrap();
            arguments.push(cell);
            types.push(ty);
        }
        let packet =
            super::super::call::ArgumentPacket::seal(&mut store, &arguments, &types).unwrap();
        frame
            .initialize_parameters(
                &packet,
                &types.iter().map(|ty| (*ty, false)).collect::<Vec<_>>(),
                &mut store,
            )
            .unwrap();
        let first = frame.parameters[0].unwrap();
        let last = frame.parameters[1].unwrap();
        frame.enter(ScopeId(0)).unwrap();
        assert_eq!(
            frame.leave(ScopeId(0), &mut store).unwrap_err().code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert!(matches!(store.state(first).unwrap(), CellState::Live(_)));
        assert_eq!(
            store.location_epoch(last).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(frame.parameters, vec![Some(first), None]);
        assert_eq!(frame.counters.cleanup_elements_completed, 1);
        assert_eq!(frame.active_scopes, vec![ScopeId(0)]);
    }
    #[test]
    fn callee_cleanup_consumes_shared_fuel_and_interrupted_entry_allocates_nothing() {
        let blob = SourceBlob::new(b"fn live(){1;}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([82; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let value = body
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
            .unwrap()
            .value
            .unwrap();
        let root = body.scopes()[0].id;
        let statement = body.nodes().iter().find_map(|n| n.statement_scope).unwrap();
        let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
        let mut caller = Frame::new(body.clone(), entry.clone(), &mut store).unwrap();
        caller.set_fuel_limit(2).unwrap();
        caller.charge_fuel().unwrap();
        let mut callee =
            Frame::new_in_run(body.clone(), entry.clone(), &mut store, &mut caller).unwrap();
        callee.enter(root).unwrap();
        callee.enter(statement).unwrap();
        let ticket = store.string("callee-owned value").unwrap();
        callee.write(value, ticket, &mut store).unwrap();
        callee.leave(statement, &mut store).unwrap();
        callee.leave(root, &mut store).unwrap();
        assert_eq!(callee.counters.cleanup_elements_completed, 1);
        assert_eq!(caller.fuel_snapshot().used, 2);
        assert_eq!(
            caller.charge_fuel().unwrap_err().code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(caller.fuel_snapshot(), callee.fuel_snapshot());
        let handles = store.budget().handles();
        assert!(Frame::new_in_run(body.clone(), entry.clone(), &mut store, &mut caller).is_err());
        assert_eq!(store.budget().handles(), handles);
        for deadline in [false, true] {
            let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
            let mut caller = Frame::new(body.clone(), entry.clone(), &mut store).unwrap();
            if deadline {
                caller.fuel.set_deadline(std::time::Instant::now());
            } else {
                caller.fuel.cancellation().cancel();
            }
            let handles = store.budget().handles();
            assert!(
                Frame::new_in_run(body.clone(), entry.clone(), &mut store, &mut caller).is_err()
            );
            assert_eq!(store.budget().handles(), handles);
            assert_eq!(caller.fuel_snapshot().used, 0);
        }
    }
    #[test]
    fn scope_cleanup_charges_before_each_cell_and_preserves_unprocessed_ownership() {
        let blob = SourceBlob::new(b"fn live(){let _=(1,2);}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([81; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let root = body.nodes().iter().find_map(|n| n.statement_scope).unwrap();
        let values: Vec<_> = body
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
            .map(|node| node.value.unwrap())
            .collect();
        assert_eq!(values.len(), 2);
        for cancelled in [false, true] {
            let mut store = GuestStore::new(1, 1000, 1024 * 1024).unwrap();
            let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
            let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
            frame.set_fuel_limit(1).unwrap();
            frame.enter(ScopeId(0)).unwrap();
            frame.enter(root).unwrap();
            for value in &values {
                let ticket = store.string("owned cleanup value").unwrap();
                frame.write(*value, ticket, &mut store).unwrap();
            }
            let first = frame.cell(values[0]).unwrap();
            let last = frame.cell(values[1]).unwrap();
            let CellState::Live(first_value) = store.state(first).unwrap() else {
                panic!("expected live owned string")
            };
            if cancelled {
                frame.fuel.cancellation().cancel();
            }
            assert_eq!(
                frame.leave(root, &mut store).unwrap_err().code,
                "LRR-E-RUNTIME-RESOURCE"
            );
            assert_eq!(frame.active_scopes, vec![ScopeId(0), root]);
            assert!(
                matches!(store.state(first).unwrap(), CellState::Live(value) if value == first_value)
            );
            assert_eq!(frame.cell(values[0]).unwrap(), first);
            if cancelled {
                assert!(matches!(store.state(last).unwrap(), CellState::Live(_)));
                assert_eq!(frame.fuel_snapshot().used, 0);
                assert_eq!(frame.counters.cleanup_elements_completed, 0);
            } else {
                assert_eq!(
                    store.location_epoch(last).unwrap_err().code,
                    "LRR-E-STALE-HANDLE"
                );
                assert!(frame.slots[values[1].0 as usize].cell.is_none());
                assert_eq!(frame.fuel_snapshot().used, 1);
                assert_eq!(frame.counters.cleanup_elements_completed, 1);
            }
        }
    }

    #[test]
    fn repeated_scope_entry_retires_binding_locations_and_owned_objects_without_reuse() {
        let blob = SourceBlob::new(b"fn live(){ {let x=\"owned\";} }".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([79; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let root = body.scopes()[0].id;
        let tail = body.nodes()[0].tail_scope.unwrap();
        let binding = body
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Pattern)
            .unwrap()
            .id;
        let inner = body.nodes()[binding.0 as usize].scope;
        let statement = body.nodes().iter().find_map(|n| n.statement_scope).unwrap();
        let source = body
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
            .unwrap()
            .value
            .unwrap();
        let entry = body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 100_000, 2 * 1024 * 1024).unwrap();
        let mut frame = Frame::new(body, entry, &mut store).unwrap();
        frame.enter(root).unwrap();
        frame.enter(tail).unwrap();
        let text = "x".repeat(4096);
        let mut previous = None;
        for _ in 0..1000 {
            frame.enter(inner).unwrap();
            frame.enter(statement).unwrap();
            let ticket = store.string(&text).unwrap();
            let object = store.peek_ticket(ticket).unwrap();
            frame.write(source, ticket, &mut store).unwrap();
            frame
                .bind(binding, source, None, false, &mut store)
                .unwrap();
            let cell = frame.binding(binding).unwrap();
            assert_ne!(Some(cell), previous);
            frame.leave(statement, &mut store).unwrap();
            frame.leave(inner, &mut store).unwrap();
            assert!(frame.bindings[binding.0 as usize].is_none());
            assert!(frame.slots[source.0 as usize].cell.is_none());
            assert_eq!(
                store.location_epoch(cell).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
            assert_eq!(store.ticket(object).unwrap_err().code, "LRR-E-STALE-HANDLE");
            // Total string payload across iterations exceeds this store's
            // budget; success requires releasing each object's owned bytes.
            previous = Some(cell);
        }
        frame.leave(tail, &mut store).unwrap();
        frame.leave(root, &mut store).unwrap();
        assert!(frame.active_scopes.is_empty());
    }
    #[test]
    fn condition_prefix_cleanup_retires_last_binding_before_temporaries_and_preserves_fault_state()
    {
        let blob =
            SourceBlob::new(b"fn live(){if let x=1_i64 && let y=2_i64 {}}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([82; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let conditional = body.nodes().iter().find_map(|n| n.condition_scope).unwrap();
        let clauses = body
            .nodes()
            .iter()
            .filter(|n| n.clause_scope.is_some())
            .collect::<Vec<_>>();
        assert_eq!(clauses.len(), 2);
        for cancelled in [false, true] {
            let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
            let mut frame = Frame::new(
                body.clone(),
                body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
                &mut store,
            )
            .unwrap();
            frame.set_fuel_limit(1).unwrap();
            frame.enter(ScopeId(0)).unwrap();
            frame.enter(body.nodes()[0].tail_scope.unwrap()).unwrap();
            frame.enter(conditional).unwrap();
            let mut cells = Vec::new();
            for clause in &clauses {
                frame.enter(clause.clause_scope.unwrap()).unwrap();
                let binding = body
                    .nodes()
                    .iter()
                    .find(|n| n.parent == Some(clause.id) && n.role == NodeRole::Pattern)
                    .unwrap()
                    .id;
                let source = body.nodes()[clause.operands[0].0 as usize].value.unwrap();
                let ticket = store.string("owned clause binding").unwrap();
                frame.write(source, ticket, &mut store).unwrap();
                let original = frame.cell(source).unwrap();
                frame
                    .bind(binding, source, None, false, &mut store)
                    .unwrap();
                cells.push((frame.binding(binding).unwrap(), original));
            }
            if cancelled {
                frame.fuel.cancellation().cancel();
            }
            assert_eq!(
                frame.leave(conditional, &mut store).unwrap_err().code,
                "LRR-E-RUNTIME-RESOURCE"
            );
            assert!(matches!(
                store.state(cells[0].0).unwrap(),
                CellState::Live(_)
            ));
            assert_eq!(store.state(cells[1].1).unwrap(), CellState::Moved);
            assert_eq!(
                frame.active_scopes().last(),
                Some(&clauses[1].clause_scope.unwrap())
            );
            if cancelled {
                assert!(matches!(
                    store.state(cells[1].0).unwrap(),
                    CellState::Live(_)
                ));
                assert_eq!(frame.counters.cleanup_elements_completed, 0);
            } else {
                assert_eq!(store.state(cells[1].0).unwrap(), CellState::Dropped);
                assert_eq!(frame.counters.cleanup_elements_completed, 1);
            }
        }
    }
    #[test]
    fn conditional_leave_refuses_to_cross_a_live_consequent_block() {
        let blob = SourceBlob::new(b"fn live(){if let x=1_i64 {}}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([83; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let owner = body
            .nodes()
            .iter()
            .find(|n| n.condition_scope.is_some())
            .unwrap();
        let conditional = owner.condition_scope.unwrap();
        let clause = body.nodes().iter().find_map(|n| n.clause_scope).unwrap();
        let consequent = body.nodes()[owner.operands[1].0 as usize].scope;
        let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame.enter(ScopeId(0)).unwrap();
        frame.enter(body.nodes()[0].tail_scope.unwrap()).unwrap();
        frame.enter(conditional).unwrap();
        frame.enter(clause).unwrap();
        frame.enter(consequent).unwrap();
        let before = frame.active_scopes().to_vec();
        assert_eq!(
            frame.leave(conditional, &mut store).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(frame.active_scopes(), before);
        assert_eq!(frame.counters.cleanup_elements_completed, 0);
    }
    #[test]
    fn unconditional_let_extension_retains_owned_storage_past_inner_tail_and_block_cleanup() {
        let blob = SourceBlob::new(b"fn live(){let r={&make()};}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([84; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let temporary = body
            .nodes()
            .iter()
            .find(|n| {
                n.role == NodeRole::Expression(ExpressionKind::Call) && n.extended_to.is_some()
            })
            .unwrap();
        assert_eq!(temporary.extended_to.as_ref().unwrap().scope, ScopeId(0));
        assert!(
            temporary
                .extended_to
                .as_ref()
                .unwrap()
                .constructors
                .is_empty()
        );
        let value = temporary.value.unwrap();
        let mut scopes = Vec::new();
        let mut next = Some(temporary.scope);
        while let Some(scope) = next {
            scopes.push(scope);
            next = body.scopes()[scope.0 as usize].parent;
        }
        scopes.reverse();
        let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        for scope in &scopes {
            frame.enter(*scope).unwrap();
        }
        // Explicit owned-storage setup; source borrows/calls are not executed.
        let ticket = store.string("extended owned storage").unwrap();
        let object = store.peek_ticket(ticket).unwrap();
        frame.write(value, ticket, &mut store).unwrap();
        let cell = frame.cell(value).unwrap();
        for scope in scopes.iter().rev().filter(|s| **s != ScopeId(0)) {
            frame.leave(*scope, &mut store).unwrap();
        }
        assert_eq!(store.admit_move(cell).unwrap(), object);
        assert_eq!(frame.cell(value).unwrap(), cell);
        frame.leave(ScopeId(0), &mut store).unwrap();
        assert_eq!(store.state(cell).unwrap(), CellState::Dropped);
        assert_eq!(store.ticket(object).unwrap_err().code, "LRR-E-STALE-HANDLE");
    }
    #[test]
    fn unproven_constructor_extension_refuses_before_default_tail_storage_publication() {
        let blob = SourceBlob::new(b"fn live(){let r=ordinary({&make()});}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([85; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let temporary = body
            .nodes()
            .iter()
            .find(|n| {
                n.role == NodeRole::Expression(ExpressionKind::Call)
                    && n.extended_to
                        .as_ref()
                        .is_some_and(|e| !e.constructors.is_empty())
            })
            .unwrap();
        let value = temporary.value.unwrap();
        let tail = temporary.scope;
        assert_eq!(
            body.scopes()[tail.0 as usize].kind,
            crate::syntax::body_index::ScopeKind::Tail
        );
        let mut scopes = Vec::new();
        let mut next = Some(tail);
        while let Some(scope) = next {
            scopes.push(scope);
            next = body.scopes()[scope.0 as usize].parent;
        }
        scopes.reverse();
        let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        for scope in &scopes {
            frame.enter(*scope).unwrap();
        }
        // The old component assumed a default lifetime without selected identity.
        // Section13.5 requires refusal rather than guessing this reached scope.
        let ticket = store.string("unextended owned storage").unwrap();
        let object = store.peek_ticket(ticket).unwrap();
        let handles = store.budget().handles();
        let fuel = frame.fuel_snapshot().used;
        assert_eq!(frame.slots[value.0 as usize].scope, tail);
        assert_eq!(
            frame.write(value, ticket, &mut store).unwrap_err().code,
            "LRR-E-TEMPORARY-SCOPE"
        );
        assert!(frame.cell(value).is_err());
        assert_eq!(store.peek_ticket(ticket).unwrap(), object);
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(frame.fuel_snapshot().used, fuel);
        assert_eq!(frame.counters.operations_started, 0);
        frame.leave(tail, &mut store).unwrap();
        // Refused publication leaves ownership with the original producer.
        assert_eq!(store.peek_ticket(ticket).unwrap(), object);
        store.discard(ticket).unwrap();
        assert_eq!(store.ticket(object).unwrap_err().code, "LRR-E-STALE-HANDLE");
    }
    #[test]
    fn statement_boundary_retires_ordinary_owned_storage_but_retains_explicit_extension() {
        let blob = SourceBlob::new(b"fn live(){make();let r=&extended();}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([86; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let calls = body
            .nodes()
            .iter()
            .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        let statements = body
            .nodes()
            .iter()
            .filter_map(|n| n.statement_scope)
            .collect::<Vec<_>>();
        assert_eq!(statements.len(), 2);
        let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame.enter(ScopeId(0)).unwrap();
        frame.enter(statements[0]).unwrap();
        let ticket = store.string("ordinary statement temporary").unwrap();
        let ordinary = store.peek_ticket(ticket).unwrap();
        frame
            .write(calls[0].value.unwrap(), ticket, &mut store)
            .unwrap();
        let cell = frame.cell(calls[0].value.unwrap()).unwrap();
        frame.leave(statements[0], &mut store).unwrap();
        assert_eq!(store.state(cell).unwrap(), CellState::Dropped);
        assert_eq!(
            store.ticket(ordinary).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        frame.enter(statements[1]).unwrap();
        let ticket = store.string("explicitly extended temporary").unwrap();
        let extended = store.peek_ticket(ticket).unwrap();
        frame
            .write(calls[1].value.unwrap(), ticket, &mut store)
            .unwrap();
        let cell = frame.cell(calls[1].value.unwrap()).unwrap();
        frame.leave(statements[1], &mut store).unwrap();
        assert_eq!(store.admit_move(cell).unwrap(), extended);
        frame.leave(ScopeId(0), &mut store).unwrap();
        assert_eq!(
            store.ticket(extended).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }
    #[test]
    fn ordinary_lazy_operand_owned_storage_retires_before_the_next_sibling_scope() {
        let blob = SourceBlob::new(b"fn live()->bool{(make()>0_i64)&&true}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([87; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let temporary = body
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
            .unwrap();
        let left = temporary.scope;
        let right = body
            .nodes()
            .iter()
            .filter_map(|n| n.boolean_scope)
            .find(|s| *s != left)
            .unwrap();
        assert_eq!(
            body.scopes()[left.0 as usize].parent,
            body.scopes()[right.0 as usize].parent
        );
        let mut scopes = Vec::new();
        let mut next = Some(left);
        while let Some(scope) = next {
            scopes.push(scope);
            next = body.scopes()[scope.0 as usize].parent;
        }
        scopes.reverse();
        let mut store = GuestStore::new(1, 1000, 1_048_576).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), crate::ir::pc::Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        for scope in scopes {
            frame.enter(scope).unwrap();
        }
        let ticket = store.string("lazy operand temporary").unwrap();
        let object = store.peek_ticket(ticket).unwrap();
        frame
            .write(temporary.value.unwrap(), ticket, &mut store)
            .unwrap();
        let cell = frame.cell(temporary.value.unwrap()).unwrap();
        frame.leave(left, &mut store).unwrap();
        assert_eq!(store.state(cell).unwrap(), CellState::Dropped);
        assert_eq!(store.ticket(object).unwrap_err().code, "LRR-E-STALE-HANDLE");
        frame.enter(right).unwrap();
        assert_eq!(frame.active_scopes().last(), Some(&right));
    }
}
