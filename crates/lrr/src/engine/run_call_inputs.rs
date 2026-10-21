//! Immutable reached-call input transport. Capture never binds a literal,
//! relates Types, selects a trait candidate or constructs a caller ParamEnv.
use super::{ArgumentType, RunMachine, SourceCallable, call_fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::{Database, PreparedInputGuard},
    },
    ir::pc::{NodeId, PcKey, SlotId},
    runtime::{
        cell::{CellId, CellState},
        shape::TypeId,
        value::ValueRepr,
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        signatures::GenericKind,
        spans::BodySpan,
    },
    traits::{
        term::{InferVarKey, ParamKey, TyTerm},
        upstream::root_input::{
            CallerSourceContext, SourceCallRelation, SourceCallRelationKind, SourceCallSite,
            SourceRootCallInput, SourceVariableFamily,
        },
    },
    vm::{call::CallIdentity, context::GenericEnv},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ReachedCallInputPending {
    CallerSource,
    ArgumentSource(u32),
    ArgumentOccurrence(u32),
    ParameterSource(u32),
    ResultSource,
}
struct ArgumentSnapshot {
    slot: SlotId,
    cell: CellId,
    state: CellState,
    value: ValueRepr,
    effective_type: Option<TypeId>,
}
/// Source occurrence and runtime numeric identity are separate. A receiver's
/// source expression owns its adjustment boundary; its bare Type is not the
/// adjusted argument Type. No inference equality is certified by either row.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum NumericSourceKind {
    WrittenArgument,
    MethodReceiver,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct NumericSourceOccurrence {
    pub(super) argument: u32,
    pub(super) slot: SlotId,
    pub(super) node: NodeId,
    pub(super) kind: NumericSourceKind,
}
/// Identity of one actual pending selection, not its reusable source call site.
/// The pending map owns the original; capture retains it across NeedInput.
pub(super) struct ReachedCallAttempt;
/// Owned by the active call preparation, not Clone and not an entry token.
/// A future consumer must retain this under an actual call-attempt/activation
/// identity across NeedInput. Checks bind it to that exact pending selection and
/// current result expectation; root admission, result-slot/phase ownership and
/// once-consumed commit remain consumer obligations. No guest ownership or
/// engine value crosses source().
pub(super) struct ReachedCallInputs {
    pc: PcKey,
    identity: CallIdentity,
    attempt: Arc<ReachedCallAttempt>,
    body: Arc<BodyIndex>,
    callee: Arc<SourceCallable>,
    signature: Digest,
    typed: Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
    owner_application: Option<Arc<PreparedInputGuard>>,
    caller_environment: Option<Arc<GenericEnv>>,
    caller_callable: Option<Arc<SourceCallable>>,
    caller_source: Option<Arc<InputValue>>,
    caller_body: Option<Arc<InputValue>>,
    span: BodySpan,
    source_site: Option<SourceCallSite>,
    arguments: Vec<ArgumentSnapshot>,
    result_hint: Option<ArgumentType>,
    parameters: BTreeMap<ParamKey, TyTerm>,
    variables: BTreeMap<InferVarKey, SourceVariableFamily>,
    numeric_sources: BTreeMap<InferVarKey, Vec<NumericSourceOccurrence>>,
    relations: Vec<SourceCallRelation>,
    caller: CallerSourceContext,
    pending: Option<ReachedCallInputPending>,
}
impl ReachedCallInputs {
    pub(super) fn caller_environment(&self) -> Option<&Arc<GenericEnv>> {
        self.caller_environment.as_ref()
    }
    pub(super) fn caller_callable(&self) -> Option<&Arc<SourceCallable>> {
        self.caller_callable.as_ref()
    }
    pub(super) fn callee_callable(&self) -> &Arc<SourceCallable> {
        &self.callee
    }
    pub(super) fn pending(&self) -> Option<ReachedCallInputPending> {
        self.pending
    }
    pub(super) fn source(&self) -> Result<SourceRootCallInput<'_>, ReachedCallInputPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        Ok(SourceRootCallInput {
            callee: self.callee.identity(),
            signature: self.signature,
            parameters: &self.parameters,
            variables: &self.variables,
            relations: &self.relations,
            caller: &self.caller,
            component: None,
        })
    }
    pub(super) fn site(&self) -> &BodySpan {
        &self.span
    }
    pub(super) fn source_site(&self) -> Option<SourceCallSite> {
        self.source_site
    }
    /// Borrow retained source occurrences after the same packet's ordinary
    /// currentness guard. An empty map does not manufacture a constraint
    /// closure; method rows still require their actual adjustment boundary.
    pub(super) fn numeric_sources(
        &self,
    ) -> Result<&BTreeMap<InferVarKey, Vec<NumericSourceOccurrence>>, ReachedCallInputPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        Ok(&self.numeric_sources)
    }
    fn numeric_bindings(
        &self,
        values: &BTreeMap<InferVarKey, TyTerm>,
        unresolved: &BTreeMap<InferVarKey, SourceVariableFamily>,
    ) -> Result<Vec<(CellId, TypeId)>, Diagnostic> {
        let mut bindings = Vec::new();
        for (key, occurrences) in &self.numeric_sources {
            let value = values.get(key).ok_or_else(|| {
                call_fault("upstream response omitted an original numeric variable")
            })?;
            let primitive = match value {
                TyTerm::Primitive(primitive) => *primitive,
                TyTerm::Infer(representative)
                    if unresolved.get(representative) == self.variables.get(key) =>
                {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "numeric fallback requires complete ordinary source constraint closure"
                            .into(),
                    ));
                }
                _ => {
                    return Err(call_fault(
                        "upstream numeric answer is neither a primitive nor an original unresolved family",
                    ));
                }
            };
            let descriptor = crate::frontend::call_contracts::primitive_runtime_id(primitive)?;
            for occurrence in occurrences {
                if occurrence.kind != NumericSourceKind::WrittenArgument {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "numeric receiver inference requires its retained source adjustment".into(),
                    ));
                }
                let argument = self
                    .arguments
                    .get(occurrence.argument as usize)
                    .filter(|argument| argument.slot == occurrence.slot)
                    .ok_or_else(|| {
                        call_fault("numeric answer occurrence lost its actual argument cell")
                    })?;
                if !matches!(argument.value.kind, 11 | 12) || argument.effective_type.is_some() {
                    return Err(call_fault(
                        "numeric answer target differs from original unbound literal",
                    ));
                }
                bindings.push((argument.cell, descriptor));
            }
        }
        Ok(bindings)
    }
    /// Checks read the actual active caller/store and retained source inputs.
    /// PC/call/cell identities can recur; the attempt must also be the actual
    /// retained pending selection. The result hint is freshly supplied by the
    /// ordinary seal's current result contract, never read from this packet.
    /// This still grants neither root permission nor once-consumed VM commit.
    pub(super) fn require_current(
        &self,
        machine: &RunMachine,
        db: &mut Database,
        result_hint: Option<&ArgumentType>,
    ) -> Result<(), Diagnostic> {
        self.require_current_with_bindings(machine, db, result_hint, &[])
    }
    fn require_current_with_bindings(
        &self,
        machine: &RunMachine,
        db: &mut Database,
        result_hint: Option<&ArgumentType>,
        bindings: &[(CellId, TypeId)],
    ) -> Result<(), Diagnostic> {
        if machine.frame.pc() != &self.pc
            || machine.frame.call_identity(self.identity.0) != self.identity
            || !Arc::ptr_eq(&machine.frame.body, &self.body)
        {
            return Err(call_fault("reached call input owner changed"));
        }
        let selected = machine
            .pending
            .get(&self.identity)
            .ok_or_else(|| call_fault("reached call selection is no longer active"))?;
        if !selected.source_input_attempt.as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.attempt))
            || !Arc::ptr_eq(&selected.callee, &self.callee)
            || selected.callee.signature().fingerprint() != self.signature
            || selected.argument_slots.len() != self.arguments.len()
            || selected
                .argument_slots
                .iter()
                .zip(&self.arguments)
                .any(|(slot, row)| *slot != row.slot)
        {
            return Err(call_fault(
                "reached call selection differs from captured inputs",
            ));
        }
        if match (&self.result_hint, result_hint) {
            (None, None) => false,
            (Some(previous), Some(current)) => {
                previous.descriptor != current.descriptor || previous.source != current.source
            }
            _ => true,
        } {
            return Err(call_fault(
                "reached call result expectation changed before input commit",
            ));
        }
        self.callee.revalidate(db)?;
        if let Some(caller) = &self.caller_callable {
            caller.revalidate(db)?;
            if let Some(environment) = &self.caller_environment {
                environment.require_current(db, caller)?;
            }
        }
        if let Some(typed) = &self.typed {
            typed.require_current(db, &machine.frame.body, machine.frame.pc())?;
        }
        if let Some(guard) = &self.owner_application {
            guard.require_current(db)?;
        }
        let current_callable = machine
            .calls
            .last()
            .map(|call| &call.call.callee)
            .or(machine.root_source.as_ref());
        if match (self.caller_callable.as_ref(), current_callable) {
            (None, None) => false,
            (Some(old), Some(current)) => !Arc::ptr_eq(old, current),
            _ => true,
        } {
            return Err(call_fault("reached caller callable changed"));
        }
        let environment = machine.frame.shared_generic_context();
        if match (&self.caller_environment, &environment) {
            (None, None) => false,
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            _ => true,
        } {
            return Err(call_fault("reached caller generic environment changed"));
        }
        for (key, old) in [
            (
                InputKey::DefinitionSource(self.pc.body.definition),
                &self.caller_source,
            ),
            (
                InputKey::DefinitionBody(self.pc.body.definition),
                &self.caller_body,
            ),
        ] {
            let current = db.input(&key);
            if match (old, &current) {
                (None, None) => false,
                (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                _ => true,
            } {
                return Err(call_fault("reached caller source attachment changed"));
            }
        }
        for argument in &self.arguments {
            if machine.frame.cell(argument.slot)? != argument.cell
                || machine.store.state(argument.cell)? != argument.state
                || machine.store.admit_move(argument.cell)? != argument.value
                || machine.store.effective_type(argument.value)?
                    != argument.effective_type.or_else(|| {
                        bindings
                            .iter()
                            .find(|(cell, _)| *cell == argument.cell)
                            .map(|(_, ty)| *ty)
                    })
            {
                return Err(call_fault(
                    "reached call argument changed before input commit",
                ));
            }
        }
        Ok(())
    }
}

impl RunMachine {
    /// Fulfill the selected call's actual numeric source constraints before
    /// ordinary literal fallback. Retain the packet across real NeedInput;
    /// argument operations and effects are never repeated here.
    pub(super) fn solve_reached_call_inputs(
        &mut self,
        db: &mut Database,
        site: NodeId,
        arguments: &[SlotId],
        result_hint: Option<&ArgumentType>,
    ) -> crate::db::poll::QueryPoll<()> {
        use crate::db::poll::QueryPoll;
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let identity = self.frame.call_identity(site.0);
        let selected = match self.pending.get(&identity) {
            Some(selected) => selected,
            None => {
                return QueryPoll::Ready(Err(call_fault("numeric inference has no selected call")));
            }
        };
        if let Some(bindings) = &selected.source_input_bindings {
            let packet = Arc::clone(
                selected
                    .source_inputs
                    .as_ref()
                    .expect("applied inputs retained"),
            );
            let bindings = Arc::clone(bindings);
            let response = Arc::clone(
                selected
                    .source_response
                    .as_ref()
                    .expect("applied response retained"),
            );
            if let Err(error) = response.require_current(db) {
                return QueryPoll::Ready(Err(error));
            }
            return QueryPoll::Ready(packet.require_current_with_bindings(
                self,
                db,
                result_hint,
                &bindings,
            ));
        }
        if selected.source_inputs.is_none() {
            // A concrete or nongeneric call has no open numeric source goal.
            // This is a demand predicate, not an inference/candidate algorithm.
            if !selected
                .callee
                .signature()
                .generics
                .iter()
                .any(|g| g.kind == GenericKind::Type)
            {
                return QueryPoll::Ready(Ok(()));
            }
            let needed = arguments.iter().try_fold(false, |found, slot| {
                let value = self.store.admit_move(self.frame.cell(*slot)?)?;
                Ok::<_, Diagnostic>(
                    found
                        || (matches!(value.kind, 11 | 12)
                            && self.store.effective_type(value)?.is_none()),
                )
            });
            match needed {
                Ok(false) => return QueryPoll::Ready(Ok(())),
                Err(error) => return QueryPoll::Ready(Err(error)),
                Ok(true) => {}
            }
            let packet = match self.capture_reached_call_inputs(db, site, arguments, result_hint) {
                Ok(packet) => Arc::new(packet),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            self.pending
                .get_mut(&identity)
                .expect("retained selected call")
                .source_inputs = Some(packet);
        }
        let packet = Arc::clone(
            self.pending[&identity]
                .source_inputs
                .as_ref()
                .expect("captured call inputs"),
        );
        if let Err(error) = packet.require_current(self, db, result_hint) {
            return QueryPoll::Ready(Err(error));
        }
        let source = match packet.source() {
            Ok(source) => source,
            Err(why) => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    format!("reached source inference input is incomplete: {why:?}"),
                )));
            }
        };
        let Some(inventory) = self.impl_catalog.clone() else {
            return QueryPoll::Ready(Err(Diagnostic::new("LRR-E-CATALOG-INCOMPLETE",
                "reached generic numeric call requires its coordinator-owned source catalog locator".into())));
        };
        let Some(caller) = packet.caller_callable() else {
            return QueryPoll::Ready(Err(call_fault("reached caller source callable is absent")));
        };
        let outcome = ready!(crate::traits::upstream::poll_reached_call(
            db,
            &inventory,
            &source,
            packet.caller_environment(),
            caller,
            packet.callee_callable(),
        ));
        match outcome {
            crate::traits::upstream::ReachedCallOutcome::Solved(response) => {
                // The upstream entry has exported original variables only after
                // its actual source/root/constraint guards. No DB/source mutation
                // intervenes before this final active-attempt/store preflight.
                if let Err(error) = packet.require_current(self, db, result_hint) {
                    return QueryPoll::Ready(Err(error));
                }
                if let Err(error) = response.require_current(db) {
                    return QueryPoll::Ready(Err(error));
                }
                let bindings = match packet.numeric_bindings(&response.values, &response.unresolved)
                {
                    Ok(bindings) => bindings,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                // Allocate the retained committed snapshot before the first store
                // write. Retry guards use these exact post-commit descriptors.
                let bindings: Arc<[(CellId, TypeId)]> = Arc::from(bindings);
                let response = Arc::new(response);
                if let Err(error) = self.store.bind_numeric_literals(&bindings) {
                    return QueryPoll::Ready(Err(error));
                }
                let selected = self
                    .pending
                    .get_mut(&identity)
                    .expect("same selected numeric call");
                selected.source_response = Some(response);
                selected.source_input_bindings = Some(bindings);
                QueryPoll::Ready(Ok(()))
            }
            other => QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-TRAIT-INFERENCE",
                format!("reached upstream inference did not settle: {other:?}"),
            ))),
        }
    }
    /// Explicit pre-fallback integration point. Only an actual upstream input
    /// consumer requests capture; legacy CallSeal never allocates this packet.
    /// `result_hint` must be the existing call contract's actual expectation.
    pub(super) fn capture_reached_call_inputs(
        &mut self,
        db: &mut Database,
        site: NodeId,
        arguments: &[SlotId],
        result_hint: Option<&ArgumentType>,
    ) -> Result<ReachedCallInputs, Diagnostic> {
        let pc = self.frame.pc().clone();
        if pc.node != site.0 {
            return Err(call_fault("reached input site differs from active call"));
        }
        let identity = self.frame.call_identity(site.0);
        let selected = self
            .pending
            .get_mut(&identity)
            .ok_or_else(|| call_fault("reached inputs require an actual selected call"))?;
        if selected.argument_slots != arguments
            || selected.callee.signature().parameters.len() != arguments.len()
        {
            return Err(call_fault(
                "reached argument slots differ from selected call",
            ));
        }
        selected.callee.revalidate(db)?;
        if let Some(typed) = &selected.typed {
            typed.require_current(db, &self.frame.body, &pc)?;
        }
        if let Some((_, guard)) = &selected.owner_application {
            guard.require_current(db)?;
        }
        let callee = Arc::clone(&selected.callee);
        let typed = selected.typed.clone();
        let owner_application = selected
            .owner_application
            .as_ref()
            .map(|(_, guard)| Arc::clone(guard));
        let explicit = selected.explicit.clone();
        let attempt = Arc::clone(
            selected
                .source_input_attempt
                .get_or_insert_with(|| Arc::new(ReachedCallAttempt)),
        );
        let signature = callee.signature().fingerprint();
        let span = if let Some(call) = &self.frame.runtime_call {
            call.require(
                self.frame.body.body(),
                &pc,
                crate::syntax::runtime_macro_scalars::RuntimeMacroCallPhase::Seal,
            )?;
            if call.arguments() != arguments {
                return Err(call_fault("reached source call argument identity differs"));
            }
            call.span().clone()
        } else {
            self.frame.body.span(site)?
        };
        if span.body_key != pc.body {
            return Err(call_fault(
                "reached input source span belongs to another body",
            ));
        }
        let (caller, caller_environment, caller_callable) = self.reached_caller(&pc);
        // Use actual body/source attachments to turn retained BodySpan into a
        // real source site. Missing joins remain pending, never fabricated spans.
        let caller_source = db.input(&InputKey::DefinitionSource(pc.body.definition));
        let caller_body = db.input(&InputKey::DefinitionBody(pc.body.definition));
        let source_site = self.reached_source_site(&span, caller_source.as_deref(), caller_body.as_deref())?;
        let mut scope = StableEncoder::new("lrr/v1/reached-call-input-variables");
        scope.record(&pc);
        scope.option(&identity.1, |e, ordinal| e.u32(*ordinal));
        scope.digest(callee.identity());
        scope.digest(signature);
        let transaction = scope.finish();
        let mut next = 0u32;
        let mut allocate = || -> Result<InferVarKey, Diagnostic> {
            let key = InferVarKey {
                transaction,
                ordinal: next,
            };
            next = next
                .checked_add(1)
                .ok_or_else(|| call_fault("reached input variable identity overflow"))?;
            Ok(key)
        };
        let mut parameters = BTreeMap::new();
        let mut variables = BTreeMap::new();
        for parameter in &callee.signature().generics {
            if parameter.kind != GenericKind::Type {
                continue;
            }
            let key = allocate()?;
            parameters.insert(
                ParamKey {
                    definition: callee.signature().definition,
                    ordinal: parameter.ordinal,
                },
                TyTerm::Infer(key),
            );
            variables.insert(key, SourceVariableFamily::Type);
        }
        let mut relations = Vec::new();
        for (ordinal, actual) in explicit {
            let parameter = ParamKey {
                definition: callee.signature().definition,
                ordinal,
            };
            if !parameters.contains_key(&parameter) {
                return Err(call_fault(
                    "explicit reached input has no source Type parameter",
                ));
            }
            // Keep equality evidence, not an unifier/last-write binding. The
            // same parameter's repeated constraints share its original key.
            if let Some(site) = source_site {
                relations.push(SourceCallRelation {
                    formal: TyTerm::Param(parameter),
                    actual: actual.source.clone(),
                    kind: SourceCallRelationKind::Equal,
                    site,
                });
            }
            // The ordinary explicit argument preparation already authenticates
            // uniqueness. This is its actual source mapping, not inference.
            if let Some(TyTerm::Infer(unused)) = parameters.insert(parameter, actual.source) {
                variables.remove(&unused);
            }
        }
        let mut pending = source_site
            .is_none()
            .then_some(ReachedCallInputPending::CallerSource);
        let mut snapshots = Vec::new();
        let mut literals = BTreeMap::<(u32, u64), InferVarKey>::new();
        let mut numeric_sources = BTreeMap::<InferVarKey, Vec<NumericSourceOccurrence>>::new();
        let source_operands: BTreeSet<NodeId> = self.frame.body.nodes().get(site.0 as usize)
            .into_iter().flat_map(|call| call.operands.iter().copied()).collect();
        for (ordinal, slot) in arguments.iter().enumerate() {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| call_fault("reached argument ordinal overflow"))?;
            let cell = self.frame.cell(*slot)?;
            let state = self.store.state(cell)?;
            let value = self.store.admit_move(cell)?;
            let effective_type = self.store.effective_type(value)?;
            // selection_argument uses the same actual source/bound view and
            // literal-family observation as ordinary trait call selection.
            let observed = self.selection_argument(*slot)?;
            let actual = match observed {
                super::trait_calls::SelectionArgument::Type(source) => Some(source),
                super::trait_calls::SelectionArgument::Literal {
                    integer,
                    behind_reference: false,
                } => {
                    if effective_type.is_some()
                        || !matches!((value.kind, integer), (11, true) | (12, false))
                    {
                        return Err(call_fault(
                            "reached numeric input differs from validated literal record",
                        ));
                    }
                    // admit_move/validate_value above checked kind+payload as
                    // an actual live GuestStore LiteralRecord handle. Payload
                    // is neither the number's bits nor its source spelling.
                    let key = if let Some(key) = literals.get(&(value.kind, value.payload)) {
                        *key
                    } else {
                        let key = allocate()?;
                        literals.insert((value.kind, value.payload), key);
                        variables.insert(
                            key,
                            if integer {
                                SourceVariableFamily::Integer
                            } else {
                                SourceVariableFamily::Float
                            },
                        );
                        key
                    };
                    match indexed_argument_occurrence(
                        &self.frame.body,
                        site,
                        &source_operands,
                        ordinal,
                        *slot,
                    ) {
                        Some(source) => numeric_sources.entry(key).or_default().push(source),
                        None => {
                            pending.get_or_insert(ReachedCallInputPending::ArgumentOccurrence(
                                ordinal,
                            ));
                        }
                    }
                    Some(TyTerm::Infer(key))
                }
                _ => {
                    pending.get_or_insert(ReachedCallInputPending::ArgumentSource(ordinal));
                    None
                }
            };
            let formal = callee.signature().parameters[ordinal as usize]
                .symbolic
                .as_ref();
            match (formal, actual, source_site) {
                (Some(formal), Some(actual), Some(site)) => relations.push(SourceCallRelation {
                    formal: formal.clone(),
                    actual,
                    kind: SourceCallRelationKind::Coerce,
                    site,
                }),
                (None, _, _) => {
                    pending.get_or_insert(ReachedCallInputPending::ParameterSource(ordinal));
                }
                _ => {}
            }
            snapshots.push(ArgumentSnapshot {
                slot: *slot,
                cell,
                state,
                value,
                effective_type,
            });
        }
        if let Some(expected) = result_hint {
            if let (Some(formal), Some(site)) = (&callee.signature().result_symbolic, source_site) {
                relations.push(SourceCallRelation {
                    formal: formal.clone(),
                    actual: expected.source.clone(),
                    kind: SourceCallRelationKind::CoerceResult,
                    site,
                });
            } else {
                pending.get_or_insert(ReachedCallInputPending::ResultSource);
            }
        }
        let packet = ReachedCallInputs {
            pc,
            identity,
            attempt,
            body: Arc::clone(&self.frame.body),
            callee,
            signature,
            typed,
            owner_application,
            caller_environment,
            caller_callable,
            caller_source,
            caller_body,
            span,
            source_site,
            arguments: snapshots,
            result_hint: result_hint.cloned(),
            parameters,
            variables,
            numeric_sources,
            relations,
            caller,
            pending,
        };
        packet.require_current(self, db, result_hint)?;
        Ok(packet)
    }
}

impl RunMachine {
    /// The caller side of a reached source root: the executing callable, its
    /// generic environment and the symbolic substitutions of that environment.
    /// Shared by every consumer of the pinned root (calls and constructors).
    pub(super) fn reached_caller(
        &self,
        pc: &PcKey,
    ) -> (
        CallerSourceContext,
        Option<Arc<GenericEnv>>,
        Option<Arc<SourceCallable>>,
    ) {
        let caller_environment = self.frame.shared_generic_context();
        let caller_callable = self
            .calls
            .last()
            .map(|call| Arc::clone(&call.call.callee))
            .or_else(|| self.root_source.as_ref().map(Arc::clone));
        let mut caller = CallerSourceContext {
            body_definition: pc.body.definition,
            generic_definition: None,
            signature: None,
            application: None,
            substitutions: BTreeMap::new(),
        };
        if let Some(environment) = &caller_environment {
            caller.generic_definition = Some(environment.definition().definition);
            caller.signature = Some(environment.signature_key());
            caller.application = environment.application_identity();
            for parameter in &environment.definition().generics {
                if parameter.kind != GenericKind::Type {
                    continue;
                }
                let key = ParamKey {
                    definition: environment.definition().definition,
                    ordinal: parameter.ordinal,
                };
                if let Some(source) = environment.symbolic_binding(key) {
                    caller.substitutions.insert(key, source.clone());
                }
            }
        }
        (caller, caller_environment, caller_callable)
    }
    /// The real source site of a retained body span, from the actual source and
    /// body attachments; absent attachments stay pending, never fabricated.
    pub(super) fn reached_source_site(
        &self,
        span: &BodySpan,
        caller_source: Option<&InputValue>,
        caller_body: Option<&InputValue>,
    ) -> Result<Option<SourceCallSite>, Diagnostic> {
        let pc = self.frame.pc();
        Ok(        match (caller_source, caller_body) {
            (Some(InputValue::DefinitionSource(parse)), Some(InputValue::DefinitionBody(body)))
                if body.key() == pc.body
                    && span.relative_start <= span.relative_end
                    && span.relative_end <= body.range.len()
                    && self
                        .frame
                        .runtime_call
                        .as_ref()
                        .is_none_or(|call| call.source() == parse) =>
            {
                Some(SourceCallSite {
                    origin: parse.content,
                    start: body
                        .range
                        .start
                        .checked_add(span.relative_start)
                        .ok_or_else(|| call_fault("reached input source offset overflow"))?,
                    end: body
                        .range
                        .start
                        .checked_add(span.relative_end)
                        .ok_or_else(|| call_fault("reached input source offset overflow"))?,
                })
            }
            _ => None,
        })
    }
}

/// The active CallSeal's selected slot list supplies argument order. Structural
/// call operands include the callee, so their ordinal is not an argument index.
/// Primary slots use the retained direct index, never slot/2 or source text.
fn indexed_argument_occurrence(
    body: &BodyIndex,
    site: NodeId,
    operands: &BTreeSet<NodeId>,
    argument: u32,
    slot: SlotId,
) -> Option<NumericSourceOccurrence> {
    let call = body.nodes().get(site.0 as usize).filter(|node| node.id == site)?;
    if !matches!(call.role, NodeRole::Expression(ExpressionKind::Call | ExpressionKind::MethodCall | ExpressionKind::LangCall)) {
        return None;
    }
    if call.role == NodeRole::Expression(ExpressionKind::MethodCall)
        && argument == 0 && call.runtime_temporaries[0] == Some(slot) {
        let receiver = *call.operands.first()?;
        body.nodes()
            .get(receiver.0 as usize)
            .filter(|node| node.id == receiver)?;
        return Some(NumericSourceOccurrence {
            argument,
            slot,
            node: receiver,
            kind: NumericSourceKind::MethodReceiver,
        });
    }
    let source = body.value_node(slot)?;
    if !operands.contains(&source.id) {
        return None;
    }
    Some(NumericSourceOccurrence {
        argument,
        slot,
        node: source.id,
        kind: NumericSourceKind::WrittenArgument,
    })
}
