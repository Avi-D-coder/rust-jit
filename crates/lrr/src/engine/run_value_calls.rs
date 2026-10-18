//! Calls through a callee value (`f(x)` with `f` a local, a parameter or any
//! non-path expression). The callee operand executed as a place; its reached
//! `TypeKey::Callable` type selects the callee exactly once per call
//! identity, and the call then proceeds as an ordinary selected source call:
//! a function item's identity selects its source function through the same
//! `CallResolver::select` a direct path call uses (with the item's own unit
//! and edition, `CallResolver::select_member_function`).
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
        if self.closures.selected(&identity) {
            return QueryPoll::Ready(Ok(true));
        }
        if self.pending.contains_key(&identity) {
            return QueryPoll::Ready(Ok(false));
        }
        let callee = (|| {
            let (mut place, mut ty) = self.frame.value_callee(site, &self.store)?;
            let place_fault = |failure: crate::runtime::place::PlaceFailure| match failure {
                crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                    call_fault("callee dereference panicked")
                }
            };
            // `(&f)(x)`: a reference to a callable calls its referent.
            loop {
                let key = self.store.type_descriptor(ty)?.key.clone();
                match key.as_ref() {
                    TypeKey::Ref { target, .. } => {
                        place = self.store.dereference_place(&place).map_err(place_fault)?;
                        ty = *target;
                    }
                    TypeKey::Callable { identity, .. } => return Ok(*identity),
                    // A fn pointer (E-R5-008): its code address names the
                    // function item it was made from; the call proceeds as a
                    // call of that item.
                    TypeKey::Function { .. } => {
                        let value = self.store.read_place(&place).map_err(place_fault)?;
                        let address = self.store.pointer_address_peek(value)?.ok_or_else(|| {
                            call_fault("fn pointer value holds no code address")
                        })?;
                        return self.store.function_at(address).ok_or_else(|| {
                            Diagnostic::new(
                                "LRR-E-POINTER-DANGLING",
                                "call through a fn pointer that designates no function".into(),
                            )
                        });
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            "LRR-E-OP-TYPE",
                            "called value is not a function item or closure".into(),
                        ));
                    }
                }
            }
        })();
        let callee = match callee {
            Ok(callee) => callee,
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
            return match self.prepare_selected_argument_call(db, Arc::from(callee), &[], arguments) {
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
    body: Arc<BodyIndex>,
    node: NodeId,
    captures: Box<[crate::ir::op::ClosureCapture]>,
    parameters: Box<[crate::ir::op::ClosureParameter]>,
}
/// A selected closure call: its target and the closure value's place.
struct SelectedClosure {
    target: Arc<ClosureTarget>,
    closure: crate::runtime::place::Place,
}
/// The closure frame entry a sealed closure call carries to `enter_callee`.
pub(super) struct ClosureEntry {
    pub(super) entry: PcKey,
    pub(super) captures: Box<[crate::ir::op::ClosureCapture]>,
    pub(super) closure: crate::runtime::place::Place,
    pub(super) mutable: Box<[bool]>,
}
#[derive(Default)]
pub(super) struct ClosureFacts {
    targets: BTreeMap<crate::core::digest::Digest, Arc<ClosureTarget>>,
    selected: BTreeMap<crate::vm::call::CallIdentity, SelectedClosure>,
    /// Each closure body's owning function, selected once.
    owners: BTreeMap<crate::source::manifest::BodyKey, Box<SourceCallable>>,
}
impl ClosureFacts {
    pub(super) fn selected(&self, identity: &crate::vm::call::CallIdentity) -> bool {
        self.selected.contains_key(identity)
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
            || !self.frame.body.nodes().get(pc.node as usize).is_some_and(|node| {
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
                captures,
                parameters,
                ..
            } = op
            else {
                continue;
            };
            let node = NodeId(self.frame.pc().node);
            let identity =
                crate::vm::frame::closures_frame::closure_identity(self.frame.body.body(), node);
            self.closures.targets.entry(identity).or_insert_with(|| {
                Arc::new(ClosureTarget {
                    body: Arc::clone(&self.frame.body),
                    node,
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
        let (mut closure, mut ty) = self.frame.value_callee(site, &self.store)?;
        loop {
            let key = self.store.type_descriptor(ty)?.key.clone();
            match key.as_ref() {
                TypeKey::Ref { target, .. } => {
                    closure = self
                        .store
                        .dereference_place(&closure)
                        .map_err(|failure| match failure {
                            crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                            crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                                call_fault("closure callee dereference panicked")
                            }
                        })?;
                    ty = *target;
                }
                TypeKey::Callable { .. } => break,
                _ => return Err(call_fault("closure callee type differs")),
            }
        }
        let identity = self.frame.call_identity(site);
        self.closures
            .selected
            .insert(identity, SelectedClosure { target, closure });
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
        let selected = self
            .closures
            .selected
            .get(&identity)
            .ok_or_else(|| call_fault("closure call start lacks its selection"))?;
        let mut hints = Vec::with_capacity(arguments.len());
        for (slot, parameter) in arguments.iter().zip(selected.target.parameters.iter()) {
            match parameter.hint {
                Some(primitive) => hints.push((
                    *slot,
                    Some(crate::frontend::call_contracts::primitive_runtime_id(primitive)?),
                    Some(crate::traits::term::TyTerm::Primitive(primitive)),
                )),
                None => hints.push((*slot, None, None)),
            }
        }
        let callee = crate::vm::frame::closures_frame::closure_identity(
            selected.target.body.body(),
            selected.target.node,
        );
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
        let Some(selected) = self.closures.selected.remove(&identity) else {
            return QueryPoll::Ready(Err(call_fault("closure call seal lacks its selection")));
        };
        let target = Arc::clone(&selected.target);
        let prepared = (|| {
            let mut cells = Vec::with_capacity(arguments.len());
            let mut types = Vec::with_capacity(arguments.len());
            for slot in arguments {
                self.frame.materialize_fallback_argument(*slot, &mut self.store)?;
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
            return self.finish_closure_seal(db, site, target, selected.closure, owner, cells, types);
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
        self.closures
            .owners
            .insert(target.body.body().clone(), owner.clone());
        self.finish_closure_seal(db, site, target, selected.closure, owner, cells, types)
    }
    #[allow(clippy::too_many_arguments)]
    fn finish_closure_seal(
        &mut self,
        db: &mut Database,
        site: u32,
        target: Arc<ClosureTarget>,
        closure: crate::runtime::place::Place,
        owner: Box<SourceCallable>,
        cells: Vec<crate::runtime::cell::CellId>,
        types: Vec<crate::runtime::shape::TypeId>,
    ) -> QueryPoll<()> {
        if let Err(error) = owner.revalidate(db) {
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
            definition: owner.identity(),
            source_parameters,
            parameter_descriptors: types.clone(),
            source_substitution: BTreeMap::new(),
            runtime_substitution: BTreeMap::new(),
            source_result: None,
            result_descriptor: None,
            normalized_deferred_return: None,
            structural_references: false,
        };
        self.prepared_call = Some(PreparedSourceCall {
            context: None,
            callee: Arc::from(owner),
            contracts,
            body: Arc::clone(&target.body),
            body_lease: Some(lease),
            retained_bytes: 0,
            closure: Some(Box::new(ClosureEntry {
                entry,
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
