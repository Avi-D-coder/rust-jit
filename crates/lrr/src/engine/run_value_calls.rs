//! Calls through a callee value (`f(x)` with `f` a local, a parameter or any
//! non-path expression). The callee operand executed as a place; its reached
//! `TypeKey::Callable` type selects the callee exactly once per call
//! identity, and the call then proceeds as an ordinary selected source call:
//! a function item's identity selects its source function through the same
//! `CallResolver::select` a direct path call uses (with the item's own unit
//! and edition, `CallResolver::select_member_function`).
//!
//! Any other callee Type is rustc's overloaded call: `Fn::call` on an
//! autoref'd callee, else `FnMut::call_mut` on a `&mut` autoref, else
//! `FnOnce::call_once` on the callee by value (`FnCtxt::
//! try_overloaded_call_traits`), selected through the run's impl member index
//! like an operator's lang-item method. The written arguments spread into the
//! method's argument tuple (the rust-call ABI) and the impl method enters as
//! any selected source call at the call's own Call exit.
use super::*;
use crate::runtime::shape::TypeKey;

pub(super) use crate::ir::op::VALUE_CALL_NAME;

impl RunMachine {
    /// Select a value call once per call identity; `true` selects a closure
    /// call (`start_closure_call`/`seal_closure_call`), `false` an ordinary
    /// source call recorded like any selected call.
    pub(super) fn select_value_call(
        &mut self,
        db: &mut Database,
        site: u32,
        arguments: &[SlotId],
    ) -> QueryPoll<bool> {
        let identity = self.frame.call_identity(site);
        if self.closures.selected.contains_key(&identity) {
            return QueryPoll::Ready(Ok(true));
        }
        if let Some(entry) = self.closures.overloaded.get(&identity) {
            // A selection made for another callee Type (a generic body's
            // call site reached again while its arguments evaluate) is not
            // this call's.
            match self.frame.value_callee(site, &self.store) {
                Ok((_, ty)) if ty == entry.callee_type => return QueryPoll::Ready(Ok(true)),
                _ => {
                    self.closures.overloaded.remove(&identity);
                }
            }
        }
        if self.pending.contains_key(&identity) {
            return QueryPoll::Ready(Ok(false));
        }
        let callee = (|| {
            // CalleeDeref has already adjusted the once-evaluated place,
            // including ordinary source `Deref` calls, before Commit.
            let (place, ty) = self.frame.value_callee(site, &self.store)?;
            match self.store.type_descriptor(ty)?.key.as_ref() {
                TypeKey::Callable { identity, .. } => Ok(ValueCallee::Identity(*identity)),
                TypeKey::Function { .. } => {
                    let value = self
                        .store
                        .read_place(&place)
                        .map_err(|failure| match failure {
                            crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                            crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                                call_fault("callee read panicked")
                            }
                        })?;
                    let address = self
                        .store
                        .pointer_address_peek(value)?
                        .ok_or_else(|| call_fault("fn pointer value holds no code address"))?;
                    self.store
                        .function_at(address)
                        .map(ValueCallee::Identity)
                        .ok_or_else(|| {
                            Diagnostic::new(
                                "LRR-E-POINTER-DANGLING",
                                "call through a fn pointer that designates no function".into(),
                            )
                        })
                }
                // Any other Type is called through its own `Fn*` impl.
                _ => Ok(ValueCallee::Overloaded(ty)),
            }
        })();
        let callee = match callee {
            Ok(ValueCallee::Identity(callee)) => callee,
            Ok(ValueCallee::Overloaded(ty)) => {
                return self.select_overloaded_call(db, site, identity, ty, arguments.len());
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Some(target) = self.closure_target(&callee) {
            return QueryPoll::Ready(
                self.select_closure_call(site, target, arguments)
                    .map(|()| true),
            );
        }
        // A body-local function item value: the entered admission its value
        // path recorded, consumed exactly as a direct call consumes it.
        if let Some(proof) = self.local_function_values.get(&callee).cloned() {
            let callee = match proof
                .revalidate(db)
                .and_then(|()| db.publish_ordinary_local_function(&proof))
                .and_then(|()| proof.callable_shared(db))
            {
                Ok(callee) => callee,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            return match self.prepare_selected_argument_call(db, Arc::from(callee), &[], arguments)
            {
                QueryPoll::Ready(Ok(call)) => {
                    self.pending_bytes = self.pending_bytes.saturating_add(call.bytes);
                    self.pending.insert(identity, call);
                    QueryPoll::Ready(Ok(false))
                }
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "value calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // The function item's own unit and edition (one shared helper).
        let callee = match resolver.select_member_function(db, callee) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        match self.prepare_selected_argument_call(db, Arc::from(callee), &[], arguments) {
            QueryPoll::Ready(Ok(call)) => {
                self.pending_bytes = self.pending_bytes.saturating_add(call.bytes);
                self.pending.insert(identity, call);
                QueryPoll::Ready(Ok(false))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}

/// A closure expression's executable entry, registered when its value is
/// created: the body index its body belongs to, its node, captures and
/// parameters. A closure value's `TypeKey::Callable` identity names it.
pub(super) struct ClosureTarget {
    identity: crate::core::digest::Digest,
    context: Option<Arc<crate::vm::context::GenericEnv>>,
    body: Arc<BodyIndex>,
    node: NodeId,
    result: SlotId,
    captures: Box<[crate::ir::op::ClosureCapture]>,
    parameters: Box<[crate::ir::op::ClosureParameter]>,
}
/// A selected closure call: its target and the closure value's place.
struct SelectedClosure {
    target: Arc<ClosureTarget>,
    closure: crate::runtime::place::Place,
    signature: Option<crate::runtime::shape::TypeId>,
}
/// The closure frame entry a sealed closure call carries to `enter_callee`.
pub(super) struct ClosureEntry {
    pub(super) entry: PcKey,
    pub(super) result: SlotId,
    pub(super) context: Option<Arc<crate::vm::context::GenericEnv>>,
    pub(super) captures: Box<[crate::ir::op::ClosureCapture]>,
    pub(super) closure: crate::runtime::place::Place,
    pub(super) mutable: Box<[bool]>,
}
#[derive(Default)]
pub(super) struct ClosureFacts {
    pub(super) accesses: BTreeMap<crate::frontend::pattern_access::PatternAccessKey,
        (Arc<QueryValue>,Arc<crate::db::memo::PreparedInputGuard>)>,
    targets: BTreeMap<crate::core::digest::Digest, Arc<ClosureTarget>>,
    selected: BTreeMap<crate::vm::call::CallIdentity, SelectedClosure>,
    /// Calls through a callee Type's own `Fn*` impl, selected for the call
    /// being prepared (the same call-identity protocol as a closure call).
    overloaded: BTreeMap<crate::vm::call::CallIdentity, SelectedOverload>,
    /// Each (call position, callee Type)'s selected `Fn*` impls, fixed once.
    selections: BTreeMap<(PcKey, crate::runtime::shape::TypeId, bool), Arc<OverloadSelection>>,
    /// The dispatcher-owned receiver and argument-tuple locations of a sealed
    /// overloaded call; their values moved into its packet, they retire as
    /// the callee frame is entered.
    locations: Vec<crate::runtime::cell::CellId>,
    /// Each closure body's actual creating source declaration, retained once.
    owners: BTreeMap<crate::source::manifest::BodyKey, Arc<SourceCallable>>,
}
impl ClosureFacts {
    pub(super) fn selected(&self, identity: &crate::vm::call::CallIdentity) -> bool {
        self.selected.contains_key(identity) || self.overloaded.contains_key(identity)
    }
}

impl RunMachine {
    /// Register the closure expression the current recipe creates, so a
    /// later call of its value (in any frame) finds its body.
    pub(super) fn register_closures(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> Result<(), Diagnostic> {
        // Only a closure expression's own entry creates its value.
        let pc = self.frame.pc();
        if pc.phase != crate::ir::pc::Phase::Entry.code()?
            || !self
                .frame
                .body
                .nodes()
                .get(pc.node as usize)
                .is_some_and(|node| {
                    node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Closure,
                        )
                })
        {
            return Ok(());
        }
        let recipe = self.installed.get_current(db, id)?;
        for op in recipe
            .recipe()
            .operations
            .iter()
            .skip(self.frame.resume_operation_index())
        {
            let Op::MakeClosure {
                body: closure_body,
                captures,
                parameters,
                ..
            } = op
            else {
                continue;
            };
            // The entered call already owns the actual source declaration,
            // including const/static initializers and nested closure frames.
            // Retain that proof instead of later guessing a function owner.
            if let Some(entered) = self.calls.last()
                && entered.call.body.body() == self.frame.body.body()
            {
                self.closures.owners.entry(self.frame.body.body().clone())
                    .or_insert_with(|| Arc::clone(&entered.call.callee));
            }
            let node = NodeId(self.frame.pc().node);
            let identity = self.frame.closure_identity(node);
            let context = self.frame.shared_generic_context();
            let result = self.frame.body.nodes().get(closure_body.0 as usize)
                .and_then(|node| node.value)
                .ok_or_else(|| call_fault("verified closure body result absent"))?;
            self.closures.targets.entry(identity).or_insert_with(|| {
                Arc::new(ClosureTarget {
                    identity,
                    context,
                    body: Arc::clone(&self.frame.body),
                    node,
                    result,
                    captures: captures.clone(),
                    parameters: parameters.clone(),
                })
            });
        }
        Ok(())
    }
    /// The closure a callee type names, when it is a registered closure.
    fn closure_target(&self, identity: &crate::core::digest::Digest) -> Option<Arc<ClosureTarget>> {
        self.closures.targets.get(identity).cloned()
    }
    /// Select a closure call: the callee place (through references) holds
    /// the closure value the frame's captured bindings will be bound from.
    fn select_closure_call(
        &mut self,
        site: u32,
        target: Arc<ClosureTarget>,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        if target.parameters.len() != arguments.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "closure called with a different number of arguments than its parameters".into(),
            ));
        }
        let (closure, ty) = self.frame.value_callee(site, &self.store)?;
        let signature = match self.store.type_descriptor(ty)?.key.as_ref() {
            TypeKey::Callable { .. } => None,
            // A reified non-capturing closure has no aggregate captures to
            // bind; its registered code address selected this same target.
            // The erased fn-pointer descriptor (`fn()`: no parameters, unit
            // result) records no signature, as a fn-pointer parameter or
            // location names it without the signature's own descriptors
            // (E-R5-008). The closure's own parameters and body then give the
            // call its argument and result types.
            TypeKey::Function { params, result }
                if target.captures.is_empty() && params.is_empty() && result.0 == 1 =>
            {
                None
            }
            TypeKey::Function { .. } if target.captures.is_empty() => Some(ty),
            _ => return Err(call_fault("closure callee type differs")),
        };
        let identity = self.frame.call_identity(site);
        self.closures
            .selected
            .insert(identity, SelectedClosure { target, closure, signature });
        Ok(())
    }
    /// A selected closure call's arguments take its written primitive
    /// parameter annotations as hints, like declared parameter types.
    pub(super) fn start_closure_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        let identity = self.frame.call_identity(site);
        if self.closures.overloaded.contains_key(&identity) {
            return self.start_overloaded_call(site, arguments);
        }
        let selected = self
            .closures
            .selected
            .get(&identity)
            .ok_or_else(|| call_fault("closure call start lacks its selection"))?;
        let mut hints = Vec::with_capacity(arguments.len());
        let pointer_parameters = selected.signature.map(|ty| {
            match self.store.type_descriptor(ty)?.key.as_ref() {
                TypeKey::Function { params, .. } => Ok(params.as_ref()),
                _ => Err(call_fault("selected closure pointer signature differs")),
            }
        }).transpose()?;
        if pointer_parameters.is_some_and(|params| params.len() != arguments.len()) {
            return Err(Diagnostic::new("LRR-E-CALL-ARITY",
                "function pointer called with a different number of arguments".into()));
        }
        for (ordinal, (slot, parameter)) in arguments.iter()
            .zip(selected.target.parameters.iter()).enumerate() {
            if let Some(params) = pointer_parameters {
                let ty = params[ordinal];
                hints.push((*slot, Some(ty), self.store.descriptor_source_term(ty)
                    .filter(crate::vm::frame::Frame::admissible_parameter_source)));
                continue;
            }
            match parameter.hint {
                Some(primitive) => hints.push((
                    *slot,
                    Some(crate::frontend::call_contracts::primitive_runtime_id(
                        primitive,
                    )?),
                    Some(crate::traits::term::TyTerm::Primitive(primitive)),
                )),
                None => hints.push((*slot, None, None)),
            }
        }
        let callee = selected.target.identity;
        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
            ordinary: crate::frontend::callables::OrdinaryCallWitness::facade(
                self.frame.body.body().clone(),
                NodeId(site),
                callee,
            ),
            site,
            hints,
            partial: Vec::new(),
        });
        Ok(())
    }
    /// Seal a closure call: its evaluated arguments become the closure
    /// frame's parameters; its owning function's source stays current for
    /// the call, and the closure frame enters at the closure's Commit phase.
    pub(super) fn seal_closure_call(
        &mut self,
        db: &mut Database,
        site: u32,
        arguments: &[SlotId],
    ) -> QueryPoll<()> {
        let identity = self.frame.call_identity(site);
        if self.closures.overloaded.contains_key(&identity) {
            return self.seal_overloaded_call(db, site, arguments);
        }
        let Some(selected) = self.closures.selected.remove(&identity) else {
            return QueryPoll::Ready(Err(call_fault("closure call seal lacks its selection")));
        };
        let target = Arc::clone(&selected.target);
        let result_descriptor = match selected.signature {
            Some(ty) => match self.store.type_descriptor(ty) {
                Ok(descriptor) => match descriptor.key.as_ref() {
                    TypeKey::Function { result, .. } => Some(*result),
                    _ => return QueryPoll::Ready(Err(call_fault("closure signature differs"))),
                },
                Err(error) => return QueryPoll::Ready(Err(error)),
            },
            None => None,
        };
        let prepared = (|| {
            let mut cells = Vec::with_capacity(arguments.len());
            let mut types = Vec::with_capacity(arguments.len());
            for slot in arguments {
                self.frame
                    .materialize_fallback_argument(*slot, &mut self.store)?;
                let cell = self.frame.cell(*slot)?;
                let value = self.store.admit_move(cell)?;
                let ty = self.store.effective_type(value)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "closure argument requires a reached concrete type".into(),
                    )
                })?;
                cells.push(cell);
                types.push(ty);
            }
            Ok::<_, Diagnostic>((cells, types))
        })();
        let (cells, types) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // The closure body is part of its creating function's source.
        if let Some(owner) = self.closures.owners.get(target.body.body()) {
            let owner = owner.clone();
            return self.finish_closure_seal(
                db,
                site,
                target,
                selected.closure,
                owner,
                cells,
                types,
                result_descriptor,
            );
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "closure calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let owner = match resolver.select_member_function(db, target.body.body().definition) {
            QueryPoll::Ready(Ok(owner)) => owner,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let owner: Arc<SourceCallable> = Arc::from(owner);
        self.closures
            .owners
            .insert(target.body.body().clone(), owner.clone());
        self.finish_closure_seal(db, site, target, selected.closure, owner, cells, types, result_descriptor)
    }
    #[allow(clippy::too_many_arguments)]
    fn finish_closure_seal(
        &mut self,
        db: &mut Database,
        site: u32,
        target: Arc<ClosureTarget>,
        closure: crate::runtime::place::Place,
        owner: Arc<SourceCallable>,
        cells: Vec<crate::runtime::cell::CellId>,
        types: Vec<crate::runtime::shape::TypeId>,
        result_descriptor: Option<crate::runtime::shape::TypeId>,
    ) -> QueryPoll<()> {
        if let Err(error) = owner.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        if let Some(context) = &target.context
            && let Err(error) = context.require_current(db, &owner)
        {
            return QueryPoll::Ready(Err(error));
        }
        let lease = match db.pin_guest_body(target.body.body()) {
            QueryPoll::Ready(Ok(lease)) => lease,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let entry = match target.body.pc(target.node, crate::ir::pc::Phase::Commit) {
            Ok(entry) => entry,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // The closure's parameters take the reached closed Types of the
        // call's arguments as their source views (every parameter or none),
        // so a forwarded reference keeps its source identity in the body.
        let source_parameters = types
            .iter()
            .map(|ty| {
                self.store
                    .descriptor_source_term(*ty)
                    .filter(crate::vm::frame::Frame::admissible_parameter_source)
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        let contracts = crate::frontend::call_contracts::PreparedCallContracts {
            closed_references: None,
            inferred_equations: None,
            definition: owner.identity(),
            source_parameters,
            parameter_descriptors: types.clone(),
            source_substitution: BTreeMap::new(),
            runtime_substitution: BTreeMap::new(),
            source_result: result_descriptor.and_then(|ty| self.store.descriptor_source_term(ty)),
            result_descriptor,
            normalized_deferred_return: None,
            structural_references: false,
        };
        self.prepared_call = Some(PreparedSourceCall {
            source_response: None,
            context: None,
            callee: owner,
            contracts,
            body: Arc::clone(&target.body),
            body_lease: Some(lease),
            retained_bytes: 0,
            closure: Some(Box::new(ClosureEntry {
                entry,
                result: target.result,
                context: target.context.as_ref().map(Arc::clone),
                captures: target.captures.clone(),
                closure,
                mutable: target.parameters.iter().map(|p| p.mutable).collect(),
            })),
        });
        self.frame.call_types = types;
        self.frame.call_cells = cells;
        self.frame.call_ready = true;
        QueryPoll::Ready(self.frame.retire_value_callee(site))
    }
}

/// What a value call's reached callee Type selects.
enum ValueCallee {
    /// A closure's or function item's identity (or a fn pointer's target).
    Identity(crate::core::digest::Digest),
    /// Any other Type: rustc's overloaded call, through the Type's own
    /// `Fn`/`FnMut`/`FnOnce` impl.
    Overloaded(crate::runtime::shape::TypeId),
}

/// The lang-item trait an overloaded call goes through. rustc tries them in
/// this order (`FnCtxt::try_overloaded_call_traits`): `Fn::call` on an
/// autoref'd callee, `FnMut::call_mut` on a `&mut` autoref, `FnOnce::call_once`
/// on the callee by value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OverloadedCall {
    Fn,
    FnMut,
    FnOnce,
}
impl OverloadedCall {
    const ORDER: [Self; 3] = [Self::Fn, Self::FnMut, Self::FnOnce];
    /// The trait (in `core::ops`) and its method.
    fn lang_item(self) -> (&'static str, &'static str) {
        match self {
            Self::Fn => ("Fn", "call"),
            Self::FnMut => ("FnMut", "call_mut"),
            Self::FnOnce => ("FnOnce", "call_once"),
        }
    }
    /// How the method takes the callee: borrowed, or by value (`None`).
    fn access(self) -> Option<crate::runtime::reference::Access> {
        match self {
            Self::Fn => Some(crate::runtime::reference::Access::Shared),
            Self::FnMut => Some(crate::runtime::reference::Access::Mutable),
            Self::FnOnce => None,
        }
    }
}

/// The impls an overloaded call of one callee Type at one call position
/// selects from: those of the first of `Fn`, `FnMut`, `FnOnce` the Type
/// implements for the call's argument count, and the written arguments' hints
/// when one impl remains.
pub(super) struct OverloadSelection {
    kind: OverloadedCall,
    candidates: Vec<(crate::core::digest::Digest, Arc<SourceCallable>)>,
    /// Each candidate's parameter hints (the receiver, then the argument tuple).
    parameters: Vec<Vec<Option<ArgumentHint>>>,
    /// Each candidate's declared argument tuple (`None` when not stated).
    tuples: Vec<Option<crate::traits::term::TyTerm>>,
    hints: Vec<(
        Option<crate::runtime::shape::TypeId>,
        Option<crate::traits::term::TyTerm>,
    )>,
}
/// An overloaded call being prepared: its selection and, once its arguments
/// are evaluated, the formed method arguments.
pub(super) struct SelectedOverload {
    callee_type: crate::runtime::shape::TypeId,
    selection: Arc<OverloadSelection>,
    /// The candidate among several the written arguments' Types selected.
    chosen: Option<usize>,
    formed: Option<(
        Vec<crate::vm::frame::operator_calls_frame::OperatorArgument>,
        Vec<crate::runtime::cell::CellId>,
    )>,
}

impl RunMachine {
    /// Whether the callee Type takes any `Fn*` impl: its autoderef walk ends
    /// there rather than stepping through `Deref`.
    pub(super) fn overloaded_call_kind(
        &mut self,
        db: &mut Database,
        ty: crate::runtime::shape::TypeId,
    ) -> QueryPoll<Option<OverloadedCall>> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Ok(self_type) = self.reached_self_type(ty) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("overloaded call requires the impl index")));
        };
        for kind in OverloadedCall::ORDER {
            let (trait_name, method) = kind.lang_item();
            let Some(trait_key) = poll!(self.call_trait(db, trait_name)) else {
                continue;
            };
            if !members.trait_method(self_type, trait_key, method).is_empty() {
                return QueryPoll::Ready(Ok(Some(kind)));
            }
        }
        QueryPoll::Ready(Ok(None))
    }
    /// A call trait (`core::ops::Fn`, `FnMut`, `FnOnce`) of the run's library;
    /// `None` when the library declares none (nothing implements it then).
    fn call_trait(
        &mut self,
        db: &mut Database,
        name: &'static str,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        match self.operator_trait(db, "ops", name) {
            QueryPoll::Ready(Ok(key)) => QueryPoll::Ready(Ok(Some(key))),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                QueryPoll::Ready(Ok(None))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// The `Fn*` impls an overloaded call of `ty` with `arity` written
    /// arguments selects from (rustc's lookup with an argument tuple of that
    /// arity), memoized per call position and callee Type.
    fn overload_selection(
        &mut self,
        db: &mut Database,
        ty: crate::runtime::shape::TypeId,
        arity: usize,
        writable: bool,
    ) -> QueryPoll<Option<Arc<OverloadSelection>>> {
        use crate::{ir::op::PrimitiveType, traits::term::TyTerm};
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        macro_rules! ready {
            ($e:expr) => {
                match $e {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let key = (self.frame.pc().clone(), ty, writable);
        if let Some(selection) = self.closures.selections.get(&key).cloned() {
            if selection
                .candidates
                .iter()
                .all(|(_, callee)| callee.revalidate(db).is_ok())
            {
                return QueryPoll::Ready(Ok(Some(selection)));
            }
            self.closures.selections.remove(&key);
        }
        let Ok(self_type) = self.reached_self_type(ty) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("overloaded call requires the impl index")));
        };
        for kind in OverloadedCall::ORDER {
            // A callee that cannot be borrowed uniquely (an immutable binding
            // a generic body calls under an `FnOnce` bound) goes to the next
            // trait: the body of a valid program calls what its bounds state.
            if kind == OverloadedCall::FnMut && !writable {
                continue;
            }
            let (trait_name, method) = kind.lang_item();
            let Some(trait_key) = poll!(self.call_trait(db, trait_name)) else {
                continue;
            };
            let mut functions = Vec::new();
            for (_, function) in members.trait_method(self_type, trait_key, method) {
                if !functions.contains(&function) {
                    functions.push(function);
                }
            }
            let mut candidates = Vec::new();
            let mut parameters = Vec::new();
            let mut tuples = Vec::new();
            for function in functions {
                let callee = poll!(self.member_callee(db, function));
                let hints = poll!(self.operator_parameter_hints(db, &callee));
                // The impl's argument tuple is the method's parameter after
                // its receiver; one that is generic in the tuple fits any.
                let tuple = hints
                    .get(1)
                    .cloned()
                    .flatten()
                    .map(|hint| hint.source);
                let fits = match &tuple {
                    Some(TyTerm::Tuple(elements)) => elements.len() == arity,
                    Some(TyTerm::Primitive(PrimitiveType::Unit)) => arity == 0,
                    _ => true,
                };
                if fits {
                    candidates.push((function, callee));
                    parameters.push(hints);
                    tuples.push(tuple);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            // One impl remaining types the written arguments, like a
            // declared signature does.
            let mut hints = vec![(None, None); arity];
            if let [Some(TyTerm::Tuple(elements))] = tuples.as_slice() {
                for (hint, element) in hints.iter_mut().zip(elements) {
                    let descriptor = ready!(self.store.intern_structural_term(element));
                    let source = Some(element.clone()).filter(|element| {
                        crate::frontend::call_contracts::closed_term(element)
                            && crate::vm::frame::Frame::admissible_parameter_source(element)
                    });
                    *hint = (descriptor, source);
                }
            }
            let selection = Arc::new(OverloadSelection {
                kind,
                candidates,
                parameters,
                tuples,
                hints,
            });
            self.closures.selections.insert(key, Arc::clone(&selection));
            return QueryPoll::Ready(Ok(Some(selection)));
        }
        QueryPoll::Ready(Ok(None))
    }
    /// Select a call of a callee value of Type `ty` through the Type's own
    /// `Fn*` impl; `true` selects the closure-shaped call protocol
    /// (`start_closure_call`/`seal_closure_call`).
    fn select_overloaded_call(
        &mut self,
        db: &mut Database,
        site: u32,
        identity: crate::vm::call::CallIdentity,
        ty: crate::runtime::shape::TypeId,
        arity: usize,
    ) -> QueryPoll<bool> {
        let writable = match self
            .frame
            .value_callee(site, &self.store)
            .and_then(|(place, _)| {
                self.store.place_writable(&place).map_err(|failure| match failure {
                    crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                    crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                        call_fault("callee place inspection panicked")
                    }
                })
            }) {
            Ok(writable) => writable,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let selection = match self.overload_selection(db, ty, arity, writable) {
            QueryPoll::Ready(Ok(Some(selection))) => selection,
            QueryPoll::Ready(Ok(None)) => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "called value is neither a function item or closure nor of a Type \
                     implementing `Fn`, `FnMut` or `FnOnce`"
                        .into(),
                )));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.closures.overloaded.insert(
            identity,
            SelectedOverload {
                callee_type: ty,
                selection,
                chosen: None,
                formed: None,
            },
        );
        QueryPoll::Ready(Ok(true))
    }
    /// An overloaded call's written arguments take the selected impl's
    /// argument Types as hints, like declared parameter types.
    fn start_overloaded_call(&mut self, site: u32, arguments: &[SlotId]) -> Result<(), Diagnostic> {
        let identity = self.frame.call_identity(site);
        let selected = self
            .closures
            .overloaded
            .get(&identity)
            .ok_or_else(|| call_fault("overloaded call start lacks its selection"))?;
        let hints = arguments
            .iter()
            .enumerate()
            .map(|(ordinal, slot)| {
                let (descriptor, source) = selected
                    .selection
                    .hints
                    .get(ordinal)
                    .cloned()
                    .unwrap_or((None, None));
                (*slot, descriptor, source)
            })
            .collect();
        let callee = selected
            .selection
            .candidates
            .first()
            .map(|(function, _)| *function)
            .ok_or_else(|| call_fault("overloaded call selection has no impl"))?;
        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
            ordinary: crate::frontend::callables::OrdinaryCallWitness::facade(
                self.frame.body.body().clone(),
                NodeId(site),
                callee,
            ),
            site,
            hints,
            partial: Vec::new(),
        });
        Ok(())
    }
    /// Seal an overloaded call: `Fn::call(&callee, (args..))` (likewise
    /// `FnMut::call_mut`, `FnOnce::call_once`) is an ordinary source call of
    /// the selected impl method whose receiver and argument tuple the
    /// dispatcher forms from the evaluated written arguments (the rust-call
    /// ABI); it enters at the call's Call exit like any selected call.
    fn seal_overloaded_call(
        &mut self,
        db: &mut Database,
        site: u32,
        arguments: &[SlotId],
    ) -> QueryPoll<()> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        macro_rules! ready {
            ($e:expr) => {
                match $e {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let identity = self.frame.call_identity(site);
        let Some(selected) = self.closures.overloaded.get(&identity) else {
            return QueryPoll::Ready(Err(call_fault("overloaded call seal lacks its selection")));
        };
        let selection = Arc::clone(&selected.selection);
        let formed = selected.formed.clone();
        // Several impls of the trait for the Type (`Fn<(i32,)>` beside
        // `Fn<(&str,)>`) are told apart by the argument tuple's Type, read
        // before the arguments move into the tuple (a re-polled preparation
        // reuses the choice).
        let chosen = match (selected.chosen, selection.candidates.as_slice()) {
            (Some(chosen), _) => chosen,
            (None, [_]) => 0,
            (None, several) => {
                let functions: Vec<_> = several.iter().map(|(function, _)| *function).collect();
                let chosen = match poll!(self.select_overloaded_function(
                    db,
                    &functions,
                    &selection.tuples,
                    arguments
                )) {
                    Some(function) => several
                        .iter()
                        .position(|(candidate, _)| *candidate == function)
                        .expect("selected among the candidates"),
                    None => {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-NAME-AMBIGUOUS",
                            "several impls of the callee Type's call trait accept the call's \
                             arguments"
                                .into(),
                        )));
                    }
                };
                if let Some(selected) = self.closures.overloaded.get_mut(&identity) {
                    selected.chosen = Some(chosen);
                }
                chosen
            }
        };
        let callee = Arc::clone(&selection.candidates[chosen].1);
        let hints = &selection.parameters[chosen];
        // The receiver and argument tuple, formed once (a re-polled
        // preparation reuses them).
        let (formed, cells) = match formed {
            Some(formed) => formed,
            None => {
                // The selected impl's argument Types type a literal argument
                // its Type was not already fixed by.
                let mut stated = Vec::new();
                if let Some(crate::traits::term::TyTerm::Tuple(elements)) =
                    hints.get(1).and_then(Option::as_ref).map(|hint| &hint.source)
                {
                    for element in elements {
                        stated.push(ready!(self.store.intern_structural_term(element)));
                    }
                }
                let formed = ready!(self.frame.form_overloaded_call_arguments(
                    site,
                    selection.kind.access(),
                    arguments,
                    &stated,
                    &mut self.store,
                ));
                if let Some(selected) = self.closures.overloaded.get_mut(&identity) {
                    selected.formed = Some(formed.clone());
                }
                formed
            }
        };
        if formed.len() != hints.len() {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "overloaded call method parameters differ from the receiver and argument tuple"
                    .into(),
            )));
        }
        let (types, references) = ready!(self.formed_call_arguments(hints, &formed));
        let result_hint = match self.frame.body.nodes().get(site as usize).and_then(|n| n.value) {
            Some(result) => ready!(self.call_result_source_hint(result)),
            None => None,
        }
        .map(|mut hint| {
            hint.source = self.frame.bound_view(&hint.source, &self.store);
            hint
        });
        let call = poll!(self.prepare_selected_source_call(
            db,
            callee,
            &[],
            result_hint.as_ref(),
            &types,
            &references,
        ));
        QueryPoll::Ready((|| {
            if let Some(proof) = &call.contracts.closed_references {
                proof.require_argument_cells(&self.store, &cells, &types)?;
            }
            call.callee.revalidate(db)?;
            self.frame.call_types = call.contracts.parameter_descriptors.clone();
            self.frame.call_cells = cells.clone();
            self.frame.call_ready = true;
            self.prepared_call = Some(call);
            self.closures.locations.extend(cells);
            self.closures.overloaded.remove(&identity);
            self.frame.retire_value_callee(site)
        })())
    }
    /// The one impl method among several that accepts the written
    /// arguments as its argument tuple: by the reached Types, an unresolved
    /// numeric literal fitting a member of its kind (rustc's integer or float
    /// variable) and, of several that do, the one taking rustc's fallback.
    fn select_overloaded_function(
        &mut self,
        db: &mut Database,
        functions: &[crate::core::digest::Digest],
        tuples: &[Option<crate::traits::term::TyTerm>],
        arguments: &[SlotId],
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        use super::trait_calls::SelectionArgument;
        use crate::{ir::op::PrimitiveType as P, traits::term::TyTerm};
        enum Written {
            Type(TyTerm),
            Literal { integer: bool },
            Unknown,
        }
        let written = (|| {
            let mut written = Vec::with_capacity(arguments.len());
            for slot in arguments {
                let cell = self.frame.cell(*slot)?;
                let value = self.store.admit_move(cell)?;
                let Some(ty) = self.store.effective_type(value)? else {
                    written.push(if matches!(value.kind, 11 | 12) {
                        Written::Literal {
                            integer: value.kind == 11,
                        }
                    } else {
                        Written::Unknown
                    });
                    continue;
                };
                written.push(match self.frame.argument_source_view(*slot, ty, &self.store)? {
                    Some(view) => Written::Type(self.frame.bound_view(&view, &self.store)),
                    None => Written::Unknown,
                });
            }
            Ok::<_, Diagnostic>(written)
        })();
        let written = match written {
            Ok(written) => written,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Every written argument typed: the shared selection by the argument
        // tuple's Type.
        if written.iter().all(|argument| matches!(argument, Written::Type(_))) {
            let views = written
                .into_iter()
                .filter_map(|argument| match argument {
                    Written::Type(view) => Some(view),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let tuple = if views.is_empty() {
                TyTerm::Primitive(P::Unit)
            } else {
                TyTerm::Tuple(views)
            };
            return self.select_by_arguments(
                db,
                functions,
                &[SelectionArgument::Unknown, SelectionArgument::Type(tuple)],
                None,
            );
        }
        // Otherwise member by member against each impl's declared tuple.
        let mut accepted = Vec::new();
        for (function, tuple) in functions.iter().zip(tuples) {
            let elements = match tuple {
                Some(TyTerm::Tuple(elements)) => elements.as_slice(),
                Some(TyTerm::Primitive(P::Unit)) => &[],
                _ => continue,
            };
            let mut fallback = true;
            let fits = written.iter().zip(elements).all(|(argument, element)| match argument {
                Written::Unknown => true,
                Written::Type(view) => {
                    !crate::frontend::call_contracts::closed_term(element)
                        || crate::frontend::signature_subst::erase_regions(element)
                            == crate::frontend::signature_subst::erase_regions(view)
                }
                Written::Literal { integer } => match element {
                    TyTerm::Param(_) | TyTerm::Infer(_) => true,
                    TyTerm::Primitive(P::F32 | P::F64) if !integer => {
                        fallback &= *element == TyTerm::Primitive(P::F64);
                        true
                    }
                    TyTerm::Primitive(
                        P::I8 | P::I16 | P::I32 | P::I64 | P::I128 | P::Isize
                        | P::U8 | P::U16 | P::U32 | P::U64 | P::U128 | P::Usize,
                    ) if *integer => {
                        fallback &= *element == TyTerm::Primitive(P::I32);
                        true
                    }
                    _ => false,
                },
            });
            if fits && elements.len() == written.len() {
                accepted.push((*function, fallback));
            }
        }
        QueryPoll::Ready(Ok(match accepted.as_slice() {
            [(function, _)] => Some(*function),
            several => {
                let mut by_fallback = several.iter().filter(|(_, fallback)| *fallback);
                match (by_fallback.next(), by_fallback.next()) {
                    (Some((function, _)), None) => Some(*function),
                    _ => None,
                }
            }
        }))
    }
    /// The receiver and argument-tuple locations of a sealed overloaded call
    /// retire once its packet took their values.
    pub(super) fn retire_call_locations(&mut self) -> Result<(), Diagnostic> {
        for cell in std::mem::take(&mut self.closures.locations) {
            self.store.retire_cell(cell)?;
        }
        Ok(())
    }
}
