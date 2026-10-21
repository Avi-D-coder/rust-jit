//! Dispatcher-owned scalar execution. Queries finish before VM mutation.
#[path = "run_capture.rs"]
mod capture;
use super::install::InstalledRecipes;
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        call_contracts::{
            ArgumentHint, ArgumentHintPreparation, ArgumentType, ClosedReferenceArgument,
            ContractPreparation, PreparedCallContracts, ReferenceCallArguments,
            prepare_argument_hint, prepare_contracts_with_references,
        },
        callables::{CallResolver, CallSelection, SourceCallable},
        names::NameNamespaces,
        signature_names::{SignatureName, SignatureNames},
    },
    ir::{
        op::Op,
        pc::{NodeId, PcKey, Phase, SlotId},
    },
    runtime::{cell::GuestStore, value::ValueRepr},
    source::{changes::PreparedSourceView, manifest::BodyKey},
    syntax::body_index::BodyIndex,
    vm::{
        frame::{Frame, OpcodeCounters},
        step::{self, StepOutcome},
    },
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};
#[path = "run_argument_hints.rs"]
mod argument_hints;
#[cfg(test)]
#[path = "run_body_lease_tests.rs"]
mod body_lease_tests;
#[cfg(test)]
#[path = "run_body_sharing_tests.rs"]
mod body_sharing_tests;
#[cfg(test)]
#[path = "run_borrows.rs"]
mod borrows;
#[path = "run_call_inputs.rs"]
mod call_inputs;
#[cfg(test)]
pub(crate) use borrows::tests::start_discovered_body;
#[path = "run_constructors.rs"]
mod constructors;
#[path = "run_constructor_inputs.rs"]
mod constructor_inputs;
#[cfg(test)]
#[path = "run_discarded_numeric_tests.rs"]
mod discarded_numeric_tests;
#[path = "run_drop_inventory.rs"]
mod drop_inventory;
#[path = "run_drop_witness.rs"]
mod drop_witness;
#[cfg(test)]
#[path = "run_drop_witness_tests.rs"]
mod drop_witness_tests;
#[path = "run_enum_discriminants.rs"]
mod enum_discriminants;
#[path = "run_facade.rs"]
mod facade;
#[path = "run_fields.rs"]
mod fields;
#[cfg(test)]
#[path = "run_floating_constant_tests.rs"]
mod floating_constant_tests;
#[path = "run_floating_constants.rs"]
mod floating_constants;
#[path = "run_format_calls.rs"]
mod format_calls;
#[path = "run_initializers.rs"]
mod initializers;
#[path = "run_layout.rs"]
mod layout_facts;
#[path = "run_methods.rs"]
mod methods;
#[path = "run_module_source_preparation.rs"]
pub(crate) mod module_source_preparation;
#[cfg(test)]
#[path = "run_nested_match_context_tests.rs"]
mod nested_match_context_tests;
#[path = "run_nominal.rs"]
mod nominal;
#[cfg(test)]
#[path = "run_observed_constructors_tests.rs"]
mod observed_constructor_tests;
#[path = "run_operator_calls.rs"]
mod operator_calls;
#[path = "run_owner_arguments.rs"]
mod owner_arguments;
#[cfg(test)]
#[path = "run_panic_tests.rs"]
mod panic_tests;
#[path = "run_pattern_access.rs"]
mod pattern_access;
#[path = "run_pattern_names.rs"]
mod pattern_names;
#[cfg(test)]
#[path = "run_pattern_tests.rs"]
mod pattern_tests;
#[cfg(test)]
#[path = "run_places.rs"]
mod places;
#[path = "run_primitive_float_methods.rs"]
mod primitive_float_methods;
#[cfg(test)]
#[path = "run_replacement_tests.rs"]
mod replacement_tests;
#[cfg(test)]
#[path = "run_return_tests.rs"]
mod return_tests;
#[path = "run_return.rs"]
mod returning;
#[path = "run_runtime_calls.rs"]
mod runtime_calls;
#[cfg(test)]
#[path = "run_shift_fallback_tests.rs"]
mod shift_fallback_tests;
#[cfg(test)]
#[path = "run_source_constant_tests.rs"]
mod source_constant_tests;
#[path = "run_source_constants.rs"]
mod source_constants;
#[path = "run_source_preparation.rs"]
pub(crate) mod source_preparation;
#[cfg(test)]
#[path = "run_temporaries.rs"]
mod temporaries;
#[path = "run_trait_calls.rs"]
mod trait_calls;
#[path = "run_panic.rs"]
mod unwinding;
#[path = "run_value_calls.rs"]
mod value_calls;
#[path = "run_value_items.rs"]
mod value_items;
#[cfg(test)]
#[path = "run_wildcard_tests.rs"]
mod wildcard_tests;
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ValueSummary {
    Unit,
    Bool(bool),
    Signed(String),
    Unsigned(String),
    Char(char),
    F32(String),
    F64(String),
}
fn summary(value: ValueRepr) -> Result<ValueSummary, Diagnostic> {
    value.validate_scalar()?;
    match value.kind {
        1 => Ok(ValueSummary::Unit),
        2 => Ok(ValueSummary::Bool(value.payload == 1)),
        3 => Ok(ValueSummary::Char(
            char::from_u32(value.payload as u32).expect("checked scalar"),
        )),
        4 => Ok(ValueSummary::Signed((value.payload as i64).to_string())),
        5 => Ok(ValueSummary::Unsigned(value.payload.to_string())),
        6 => Ok(ValueSummary::F32(
            f32::from_bits(value.payload as u32).to_string(),
        )),
        7 => Ok(ValueSummary::F64(f64::from_bits(value.payload).to_string())),
        _ => Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "result summary awaits its ordered value family".into(),
        )),
    }
}
#[derive(Debug, Serialize)]
pub struct RunReport {
    pub live_edition: u16,
    pub semantics_profile: &'static str,
    pub schema: &'static str,
    pub status: &'static str,
    pub value: ValueSummary,
    pub opcode_counters: OpcodeCounters,
    pub installed_recipes: usize,
    pub installed_lrir_bytes: usize,
    /// Guest `std::io::_print`/`_eprint` output, in order.
    #[serde(skip_serializing_if = "GuestOutput::is_empty")]
    pub output: GuestOutput,
}
#[derive(Debug, Serialize)]
pub struct PanicReport {
    pub live_edition: u16,
    pub semantics_profile: &'static str,
    pub schema: &'static str,
    pub status: &'static str,
    pub panic: crate::vm::step::GuestPanic,
    pub opcode_counters: OpcodeCounters,
    #[serde(skip_serializing_if = "GuestOutput::is_empty")]
    pub output: GuestOutput,
}
/// Guest standard output/error written through the closed library facade.
/// The development report carries it; the process streams stay the report's.
#[derive(Debug, Default, Clone, Serialize)]
pub struct GuestOutput {
    pub stdout: String,
    pub stderr: String,
}
impl GuestOutput {
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }
}
pub enum RunPoll {
    Completed(RunReport),
    NeedInput(InputRequest),
    Fault(Diagnostic),
    Panicked(PanicReport),
}
/// Retained only by the dispatcher after its first actual terminal result.
/// Public reports are detached values and cannot replace these execution facts.
#[derive(Debug)]
pub(crate) enum TerminalExecution {
    Completed { unit: bool },
    GuestPanic { message: String },
    Fault { code: &'static str },
}
#[derive(Debug)]
pub(crate) struct TerminalFacts {
    pub outcome: TerminalExecution,
    pub cleanup_ran: Option<bool>,
}
struct PreparedSourceCall {
    source_response: Option<Arc<crate::traits::upstream::ReachedCallResponse>>,
    context: Option<crate::vm::context::GenericEnv>,
    /// Shared with a selection cache (operator methods); never deep-copied.
    callee: Arc<SourceCallable>,
    contracts: PreparedCallContracts,
    body: Arc<BodyIndex>,
    body_lease: Option<crate::db::memo::BodyExecutionLease>,
    retained_bytes: usize,
    /// A closure call enters its creating function's body at the closure
    /// (`callee` is that function); see `run_value_calls.rs`.
    closure: Option<Box<value_calls::ClosureEntry>>,
}
#[derive(Clone)]
struct SelectedArgumentCall {
    /// Shared: a pending selection is cloned at its seal.
    callee: Arc<SourceCallable>,
    explicit: Vec<(u32, ArgumentType)>,
    typed: Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
    /// Actual qualifier Type plan and dependency guard retained across argument
    /// effects/suspension; source application is never reconstructed at entry.
    owner_application: Option<(Arc<QueryValue>, Arc<crate::db::memo::PreparedInputGuard>)>,
    hints: Vec<ArgumentHint>,
    argument_slots: Vec<SlotId>,
    result_hint: Option<ArgumentType>,
    /// Allocated lazily only when source inference captures this actual
    /// pending selection. Clones/suspension retain its same attempt identity.
    source_input_attempt: Option<Arc<call_inputs::ReachedCallAttempt>>,
    source_inputs: Option<Arc<call_inputs::ReachedCallInputs>>,
    source_response: Option<Arc<crate::traits::upstream::ReachedCallResponse>>,
    source_input_bindings:
        Option<Arc<[(crate::runtime::cell::CellId, crate::runtime::shape::TypeId)]>>,
    /// Callee type parameters the reached result context fixes through the
    /// declared result: the expectation its arguments evaluate under.
    expected: BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>,
    bytes: usize,
}
struct SuspendedCall {
    continuation: CallContinuation,
    caller: Frame,
    body_lease: crate::db::memo::BodyExecutionLease,
    call: PreparedSourceCall,
    pending: BTreeMap<crate::vm::call::CallIdentity, SelectedArgumentCall>,
}
/// What a returning (or unwinding) callee resumes in its suspended caller.
enum CallContinuation {
    /// An ordinary recipe call site: its consumed action, result slot and PC.
    Site {
        action: crate::vm::action::ActionId,
        origin: crate::ir::recipe::Origin,
        resume: PcKey,
        result: SlotId,
    },
    /// Drop glue's `Drop::drop` witness: discard the unit result and resume
    /// the caller's suspended cleanup activation (and its own continuation).
    DropWitness(drop_witness::DropWitnessResume),
    /// An overloaded operator's trait method: its result completes the
    /// caller's suspended operator operation.
    Operator,
    /// A `format_args!` placeholder's trait `fmt`: its output buffer becomes
    /// the placeholder's text in the caller's suspended format operation.
    Format { sink: usize },
    /// A `const`/`static` initializer demanded by the caller's `ValuePath`.
    Initializer {
        identity: crate::core::digest::Digest,
        is_static: bool,
        writable: bool,
    },
}
impl SuspendedCall {
    fn action(&self) -> Option<crate::vm::action::ActionId> {
        match &self.continuation {
            CallContinuation::Site { action, .. } => Some(*action),
            CallContinuation::DropWitness(_)
            | CallContinuation::Operator
            | CallContinuation::Format { .. }
            | CallContinuation::Initializer { .. } => None,
        }
    }
    #[cfg(test)]
    fn result(&self) -> SlotId {
        let CallContinuation::Site { result, .. } = &self.continuation else {
            panic!("not a call-site continuation")
        };
        *result
    }
    #[cfg(test)]
    fn resume(&self) -> PcKey {
        let CallContinuation::Site { resume, .. } = &self.continuation else {
            panic!("not a call-site continuation")
        };
        resume.clone()
    }
}
fn call_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn primitive_argument_type(
    descriptor: crate::runtime::shape::TypeId,
) -> Result<ArgumentType, Diagnostic> {
    use crate::ir::op::PrimitiveType::*;
    let source = crate::traits::term::TyTerm::Primitive(match descriptor.0 {
        1 => Unit,
        2 => Bool,
        3 => Char,
        4 => I8,
        5 => I16,
        6 => I32,
        7 => I64,
        8 => Isize,
        9 => U8,
        10 => U16,
        11 => U32,
        12 => U64,
        13 => Usize,
        14 => F32,
        15 => F64,
        17 => I128,
        18 => U128,
        _ => {
            return Err(call_fault(
                "source view requires further value-family integration",
            ));
        }
    });
    Ok(ArgumentType { source, descriptor })
}
/// Keep an evaluated source identity; descriptors cannot reconstruct references
/// or nominal declarations. Source contracts validate the pair at the boundary.
fn reached_argument_type(
    descriptor: crate::runtime::shape::TypeId,
    source: Option<crate::traits::term::TyTerm>,
) -> Result<ArgumentType, Diagnostic> {
    match source {
        Some(source) => Ok(ArgumentType { source, descriptor }),
        None => primitive_argument_type(descriptor),
    }
}
/// Discovery-complete graphs are shared immutable source data. The owned
/// legacy entry still performs its existing context contribution separately;
/// it must not turn a completed graph into a copy-on-write runtime namespace.
enum RunNames {
    Legacy(NameNamespaces),
    Completed(Arc<crate::expand::source_discovery::CompletedSourceEnvironment>),
}
impl std::ops::Deref for RunNames {
    type Target = NameNamespaces;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Legacy(graph) => graph,
            Self::Completed(environment) => environment.names(),
        }
    }
}
pub struct RunMachine {
    frame: Frame,
    body_lease: crate::db::memo::BodyExecutionLease,
    store: GuestStore,
    installed: InstalledRecipes,
    terminal: bool,
    root_body: BodyKey,
    root_source: Option<Arc<SourceCallable>>,
    impl_catalog: Option<crate::db::seals::SealKey>,
    query_trace_start: usize,
    terminal_facts: Option<TerminalFacts>,
    runtime_capture: Option<capture::RunCapture>,
    sources: Vec<PreparedSourceView>,
    graph: Option<RunNames>,
    static_panic_body_bases: BTreeMap<crate::core::digest::Digest, u32>,
    static_panic_plans: BTreeMap<
        (crate::core::digest::Digest, u32, u32),
        (
            PreparedSourceView,
            Box<crate::frontend::runtime_macro_plan::RuntimeIntrinsicPlan>,
        ),
    >,
    runtime_no_item_proofs:
        BTreeMap<(BodyKey, u32, u32), crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    runtime_calls:
        BTreeMap<PcKey, std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>,
    floating_plans: BTreeMap<PcKey, crate::frontend::primitive_constants::FloatingConstantRecipe>,
    primitive_methods: BTreeMap<
        PcKey,
        (
            crate::frontend::runtime_macro_plan::PrimitiveFloatAdmission,
            crate::core::digest::Digest,
        ),
    >,
    constant_plans: BTreeMap<PcKey, source_constants::SourceConstantPlan>,
    pattern_names: BTreeMap<
        PcKey,
        (
            crate::core::digest::Digest,
            Arc<[crate::vm::pattern::matcher::PatternTarget]>,
            Vec<Arc<crate::db::memo::PreparedInputGuard>>,
        ),
    >,
    /// The entered lexical selection of each single-name `ValuePath` PC a
    /// body-local item may bind (`None`: the module route), selected once.
    /// The selected static cell of each by-value static read PC.
    static_reads: BTreeMap<
        PcKey,
        (
            crate::runtime::cell::CellId,
            Option<crate::traits::term::TyTerm>,
        ),
    >,
    /// Body-local function items used as values: their entered admission
    /// (the same proof a direct call of the path consumes), by identity.
    local_function_values: BTreeMap<
        crate::core::digest::Digest,
        Arc<crate::frontend::callables::ordinary_local::OrdinaryLocalAdmission>,
    >,
    lexical_values: BTreeMap<PcKey, Option<value_items::LexicalSelection>>,
    /// Run-global static storage: one cell per static, initialized once from
    /// its const-evaluated initializer, owned by a never-retired anchor.
    static_cells: BTreeMap<crate::core::digest::Digest, crate::runtime::cell::CellId>,
    static_anchor: Option<crate::runtime::cell::FrameId>,
    declarative_runtime: super::declarative_runtime::DeclarativeRuntimePlans,
    prepared_call: Option<PreparedSourceCall>,
    returning: Option<returning::ReturnContinuation>,
    unwinding: Option<unwinding::PanicContinuation>,
    pending: BTreeMap<crate::vm::call::CallIdentity, SelectedArgumentCall>,
    pending_bytes: usize,
    /// Type arguments later code decided for the call at this PC (the
    /// recipe's `inferred_arguments`), as explicit arguments of the callee
    /// declaration they belong to.
    inferred_call: Option<(PcKey, crate::core::digest::Digest, Vec<(u32, ArgumentType)>)>,
    calls: Vec<SuspendedCall>,
    /// Impl members of the run's nominal types (built on first poll).
    members: Option<Arc<crate::frontend::member_index::MemberIndex>>,
    /// `<T as Drop>::drop` selected once per nominal declaration; each
    /// witness entry revalidates it and prepares its own call contracts.
    drop_witnesses: BTreeMap<crate::core::digest::Digest, Box<SourceCallable>>,
    /// Operator methods selected per (site, operator, self type, right
    /// operand type): the shared callee and its per-parameter hints; each
    /// use revalidates the callee and prepares its own call contracts.
    operator_methods: BTreeMap<
        (
            PcKey,
            u32,
            crate::runtime::shape::TypeId,
            Option<crate::runtime::shape::TypeId>,
        ),
        Arc<operator_calls::OperatorMethod>,
    >,
    /// Lang-item trait definitions of operators, by trait name, for the run.
    operator_traits: BTreeMap<&'static str, crate::core::digest::Digest>,
    /// Facade formatting entries, by name, for the run.
    format_entries: BTreeMap<&'static str, Arc<SourceCallable>>,
    /// Output buffers of in-progress trait-method formatting calls.
    format_sinks: format_calls::FormatSinks,
    /// A `ValuePath`'s demanded `const`/`static` initializer (E-R5-007).
    initializer_demand: Option<initializers::InitializerDemand>,
    /// Evaluated constants copied on each use, with their returned views.
    const_cells: BTreeMap<crate::core::digest::Digest, initializers::StoredConstant>,
    /// A constant's value evaluated for the one use at this PC.
    initialized_values: BTreeMap<
        (PcKey, crate::core::digest::Digest),
        (
            crate::runtime::cell::TicketId,
            Option<crate::traits::term::TyTerm>,
        ),
    >,
    /// Slots allocated under the run's static anchor.
    static_slots: u32,
    /// Source views of statics whose initializer ran in the interpreter.
    static_views: BTreeMap<crate::core::digest::Digest, crate::traits::term::TyTerm>,
    /// Call sites (PC and call node) whose written qualified path is not a
    /// trait path.
    ordinary_call_paths: std::collections::BTreeSet<(PcKey, u32)>,
    /// Type arguments written on a call path's owner segment, selected once
    /// per call site (`None`: the site writes none).
    owner_arguments: BTreeMap<
        (PcKey, owner_arguments::ArgumentPosition),
        Option<Arc<owner_arguments::OwnerArguments>>,
    >,
    /// The impl function selected among impls of one trait that differ by
    /// the trait's arguments, per (call site, reached argument Types).
    argument_selections:
        BTreeMap<(PcKey, Vec<Option<crate::runtime::shape::TypeId>>), crate::core::digest::Digest>,
    /// Source callables of member functions the impl index selected
    /// (methods, trait-path and operator impls), by definition identity.
    member_callees: BTreeMap<crate::core::digest::Digest, Arc<SourceCallable>>,
    /// Selected unit/tuple struct and enum variant constructors.
    nominal: nominal::NominalRunFacts,
    /// Registered closure expressions and selected closure calls.
    closures: value_calls::ClosureFacts,
    output: GuestOutput,
    collected: OpcodeCounters,
    call_bytes: usize,
    peak_guest_frames: u32,
}
impl RunMachine {
    pub fn declarative_expressions(
        &self,
    ) -> impl Iterator<Item = &crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression>
    {
        self.declarative_runtime.plans()
    }

    // E-R5-006: ignored, remove at callers (`_fuel_limit`; no guest step limit).
    pub fn start(db: &mut Database, body: BodyKey, _fuel_limit: u64) -> QueryPoll<Self> {
        let query_trace_start = db.trace.events.len();
        let outcome = match db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body)) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "entered body structure result mismatch".into(),
            )));
        };
        let body_lease = match db.pin_guest_body(index.body()) {
            QueryPoll::Ready(Ok(lease)) => lease,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let initialize = (|| {
            let mut store = GuestStore::for_epoch(1)?;
            let frame = Frame::new(
                Arc::clone(index),
                index.pc(NodeId(0), Phase::Entry)?,
                &mut store,
            )?;
            let installed = InstalledRecipes::new(
                db.source_revision(),
                super::telemetry::INSTALLED_LRIR_BUDGET,
            )?;
            Ok(Self {
                frame,
                body_lease,
                store,
                installed,
                terminal: false,
                root_body: index.body().clone(),
                root_source: None,
                impl_catalog: None,
                query_trace_start,
                terminal_facts: None,
                runtime_capture: None,
                sources: vec![],
                graph: None,
                static_panic_body_bases: BTreeMap::new(),
                static_panic_plans: BTreeMap::new(),
                runtime_no_item_proofs: BTreeMap::new(),
                runtime_calls: BTreeMap::new(),
                floating_plans: BTreeMap::new(),
                primitive_methods: BTreeMap::new(),
                constant_plans: BTreeMap::new(),
                pattern_names: BTreeMap::new(),
                lexical_values: BTreeMap::new(),
                local_function_values: BTreeMap::new(),
                static_reads: BTreeMap::new(),
                static_cells: BTreeMap::new(),
                static_anchor: None,
                declarative_runtime: Default::default(),
                prepared_call: None,
                returning: None,
                unwinding: None,
                pending: BTreeMap::new(),
                pending_bytes: 0,
                inferred_call: None,
                output: GuestOutput::default(),
                members: None,
                drop_witnesses: BTreeMap::new(),
                operator_methods: BTreeMap::new(),
                operator_traits: BTreeMap::new(),
                format_entries: BTreeMap::new(),
                format_sinks: Default::default(),
                initializer_demand: None,
                const_cells: BTreeMap::new(),
                initialized_values: BTreeMap::new(),
                static_slots: 0,
                static_views: BTreeMap::new(),
                ordinary_call_paths: std::collections::BTreeSet::new(),
                owner_arguments: BTreeMap::new(),
                argument_selections: BTreeMap::new(),
                member_callees: BTreeMap::new(),
                nominal: Default::default(),
                closures: Default::default(),
                calls: vec![],
                collected: OpcodeCounters::default(),
                call_bytes: 0,
                peak_guest_frames: 1,
            })
        })();
        QueryPoll::Ready(initialize)
    }
    pub fn start_with_sources(
        db: &mut Database,
        body: BodyKey,
        fuel_limit: u64, // E-R5-006: ignored, remove at callers
        sources: Vec<PreparedSourceView>,
    ) -> QueryPoll<Self> {
        // This bounded metadata allocation precedes both the body query and
        // guest frame/store creation. Resource refusal must remain typed.
        let mut outlines = Vec::new();
        if outlines.try_reserve_exact(sources.len()).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        outlines.extend(sources.iter().map(PreparedSourceView::outline));
        let mut graph = match NameNamespaces::from_outlines(&outlines) {
            Ok(graph) => graph,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Err(error) = graph.resolve_imports() {
            return QueryPoll::Ready(Err(error));
        }
        match Self::start(db, body, fuel_limit) {
            QueryPoll::Ready(Ok(mut run)) => {
                run.sources = sources;
                run.graph = Some(RunNames::Legacy(graph));
                QueryPoll::Ready(Ok(run))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(crate) fn start_with_test_catalog(
        db: &mut Database,
        body: BodyKey,
        fuel_limit: u64, // E-R5-006: ignored, remove at callers
        sources: Vec<PreparedSourceView>,
        catalog: &super::test_catalog::TestCatalog,
    ) -> QueryPoll<Self> {
        if let Err(error) = catalog.require_current(db) {
            return QueryPoll::Ready(Err(error));
        }
        match Self::start(db, body, fuel_limit) {
            QueryPoll::Ready(Ok(mut run)) => {
                run.sources = sources;
                run.graph = catalog
                    .runtime_environment()
                    .map(|environment| RunNames::Completed(Arc::clone(environment)));
                QueryPoll::Ready(Ok(run))
            }
            other => other,
        }
    }
    pub fn start_with_discovery(
        db: &mut Database,
        body: BodyKey,
        fuel_limit: u64, // E-R5-006: ignored, remove at callers
        mut discovery: crate::expand::source_discovery::SourceDiscovery,
    ) -> QueryPoll<Self> {
        if let Err(error) = discovery.republish(db) {
            return QueryPoll::Ready(Err(error));
        }
        let source_environment = match discovery.completed_source_environment(db) {
            QueryPoll::Ready(Ok(environment)) => environment,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let seal_fingerprint = match db.fetch(
            &SyntaxProvider,
            QueryKey::DiscoverySeal(discovery.key().clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => {
                let QueryValue::DiscoverySeal(seal) = value.value.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "runtime discovery certificate mismatch",
                    )));
                };
                if seal.key() != *discovery.key() {
                    return QueryPoll::Ready(Err(call_fault(
                        "runtime discovery certificate mismatch",
                    )));
                }
                seal.fingerprint()
            }
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let seal_key = discovery.key().clone();
        let declarative_records = match discovery.sealed_declaration_expansions() {
            Ok(records) => records,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (sources, _graph) = match discovery.into_runtime_parts() {
            Ok(parts) => parts,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let declarative_runtime =
            match super::declarative_runtime::DeclarativeRuntimePlans::from_sealed(
                &declarative_records,
                &sources,
            ) {
                Ok(plans) => plans,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        match Self::start(db, body, fuel_limit) {
            QueryPoll::Ready(Ok(mut run)) => {
                run.sources = sources;
                run.graph = Some(RunNames::Completed(source_environment));
                run.declarative_runtime = declarative_runtime;
                if let Err(error) = run.bootstrap_no_item_proofs(db, &seal_key, seal_fingerprint) {
                    return QueryPoll::Ready(Err(error));
                }
                QueryPoll::Ready(Ok(run))
            }
            QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
        }
    }
    /// Retain actual coordinator-owned locators and the selected root callable.
    /// The locator is not an impl catalog completion or solver-entry proof.
    pub(crate) fn install_source_inference(
        &mut self,
        root: Arc<SourceCallable>,
        inventory: crate::db::seals::SealKey,
    ) -> Result<(), Diagnostic> {
        if root.identity() != self.root_body.definition || self.root_source.is_some() {
            return Err(call_fault(
                "source inference root differs from selected entry",
            ));
        }
        inventory.validate()?;
        self.root_source = Some(root);
        self.impl_catalog = Some(inventory);
        Ok(())
    }
    fn select_argument_call(
        &mut self,
        db: &mut Database,
        name: &str,
        syntax: &[String],
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let callee = match self.select_named_callee(db, name) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        self.prepare_selected_argument_call(db, callee, syntax, argument_slots)
    }
    /// The source function a written call path names, through the ordinary
    /// call resolver.
    fn select_named_callee(&self, db: &mut Database, name: &str) -> QueryPoll<Arc<SourceCallable>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(r) => r.with_members(self.members.as_deref()),
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        match resolver.source_function(db, self.frame.pc().body.definition, name) {
            QueryPoll::Ready(Ok(CallSelection::Source(callee))) => {
                QueryPoll::Ready(Ok(Arc::from(callee)))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(call_fault(
                "callee classification requires later call context integration",
            ))),
            QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
        }
    }
    /// Consume the same immutable source selection used by ordinary lowering.
    /// Publication is a reached coordinator action; the pure goal never enters
    /// or prepares a callee body. Qualified/value paths remain explicitly held.
    fn select_common_argument_call(
        &mut self,
        db: &mut Database,
        name: &str,
        syntax: &[String],
        arguments: &[SlotId],
    ) -> QueryPoll<Option<SelectedArgumentCall>> {
        let pc = self.frame.pc().clone();
        let typed = match self.installed.source_call(db, &pc) {
            Ok(Some(plan)) => plan.clone(),
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Err(error) = typed
            .require_current(db, &self.frame.body, &pc)
            .and_then(|()| typed.require_written(name, syntax))
        {
            return QueryPoll::Ready(Err(error));
        }
        let callee = match typed.callable(db) {
            Ok(callee) => callee,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut explicit = Vec::new();
        if explicit.try_reserve_exact(typed.rows().len()).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        for row in typed.rows() {
            let descriptor = match row.source().descriptor(&self.frame, &mut self.store) {
                Ok(descriptor) => descriptor,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            explicit.push((
                row.ordinal(),
                ArgumentType {
                    source: self.frame.bound_view(row.source().source(), &self.store),
                    descriptor,
                },
            ));
        }
        if let Some(qualifier) = typed.associated_source() {
            match self.associated_owner_bindings(db, &callee, qualifier) {
                QueryPoll::Ready(Ok(bindings)) => explicit.extend(bindings),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        self.finish_selected_argument_call(db, Arc::from(callee), explicit, Some(typed), arguments)
            .map(Some)
    }
    fn select_reached_argument_call(
        &mut self,
        db: &mut Database,
        contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
        site: crate::ir::recipe::CallSiteId,
        name: &str,
        syntax: &[String],
        arguments: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        if self.frame.pc().node != site.0 {
            return QueryPoll::Ready(Err(call_fault("reached lexical call site differs")));
        }
        if self.frame.runtime_call.is_some() {
            match self.require_runtime_call_start(db) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        match self.select_common_argument_call(db, name, syntax, arguments) {
            QueryPoll::Ready(Ok(Some(selected))) => return QueryPoll::Ready(Ok(selected)),
            QueryPoll::Ready(Ok(None)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        for context in contexts {
            if !matches!(db.input(&crate::db::dependency::InputKey::DefinitionSource(self.frame.pc().body.definition)).as_deref(),
                Some(crate::db::dependency::InputValue::DefinitionSource(source))
                if source==&context.source().selection().outline.parse)
            {
                continue;
            }
            match context.bind_lexical_numeric_function(
                db,
                graph,
                &self.frame.body,
                self.frame.pc(),
                name,
                self.runtime_no_item_proofs.values(),
            ) {
                QueryPoll::Ready(Ok(Some(callee))) => {
                    return self.prepare_selected_argument_call(
                        db,
                        Arc::from(callee),
                        syntax,
                        arguments,
                    );
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        // A lowered lexical source path (a single name, or `Root::member`
        // whose root a block item or import bound) must never fall back to a
        // same-named module callable when its entered namespace context was
        // not supplied.
        if !name.starts_with('.') {
            match self.requires_lexical_callee_context(db, name) {
                QueryPoll::Ready(Ok(true)) => {
                    // `Root::function` where a block item or import binds
                    // `Root` to a type: its associated function.
                    match self.select_lexical_associated_call(db, name, syntax, arguments) {
                        QueryPoll::Ready(Ok(Some(selected))) => {
                            return QueryPoll::Ready(Ok(selected));
                        }
                        QueryPoll::Ready(Ok(None)) => {}
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "lexical callee requires exact entered source context".into(),
                    )));
                }
                QueryPoll::Ready(Ok(false)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        match self.owner_arguments(db) {
            QueryPoll::Ready(Ok(Some(owner))) => {
                self.select_owner_argument_call(db, name, syntax, &owner, arguments)
            }
            QueryPoll::Ready(Ok(None)) => self.select_argument_call(db, name, syntax, arguments),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn prepare_selected_argument_call(
        &mut self,
        db: &mut Database,
        callee: Arc<SourceCallable>,
        syntax: &[String],
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        self.prepare_selected_argument_call_with(db, callee, syntax, Vec::new(), argument_slots)
    }
    /// `prepare_selected_argument_call` with Type arguments already selected
    /// by other means (`extra`: the owner segment's arguments bound to the
    /// callee's impl parameters).
    fn prepare_selected_argument_call_with(
        &mut self,
        db: &mut Database,
        callee: Arc<SourceCallable>,
        syntax: &[String],
        extra: Vec<(u32, ArgumentType)>,
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut explicit = Vec::new();
        if !syntax.is_empty() {
            let parameters: Vec<_> = callee
                .signature()
                .generics
                .iter()
                .filter(|p| p.kind == crate::syntax::signatures::GenericKind::Type)
                .collect();
            if syntax.len() > parameters.len() {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "too many explicit type arguments".into(),
                )));
            }
            let caller_definition = self.frame.pc().body.definition;
            let caller = match db.fetch(
                &SyntaxProvider,
                QueryKey::FunctionSignature(caller_definition),
            ) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            };
            let QueryValue::FunctionSignature(caller_signature) = caller.value.as_ref() else {
                return QueryPoll::Ready(Err(call_fault(
                    "caller signature query variant mismatch",
                )));
            };
            let names = match SignatureNames::from_database(db, graph, caller_signature) {
                Ok(names) => names,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            let normalizer =
                match crate::frontend::type_aliases::AliasNormalizer::new(graph, &sources) {
                    Ok(n) => n,
                    Err(e) => return QueryPoll::Ready(Err(e)),
                };
            // A written compound or nominal Type argument (`Option<u8>`,
            // `[u8; 4]`, `Foo`) is selected in the caller's declaration
            // context by the same written-Type service a `let` annotation
            // uses, then interned.
            macro_rules! written_argument {
                ($text:expr) => {{
                    let tokens = [crate::syntax::ast_adapter::TokenRecipe {
                        kind: 0,
                        text: $text.clone(),
                    }];
                    let term = match crate::frontend::source_types::resolve_written_type(
                        db,
                        graph,
                        &sources,
                        caller_signature,
                        &tokens,
                    ) {
                        QueryPoll::Ready(Ok(Some(term))) => term,
                        QueryPoll::Ready(Ok(None)) => {
                            return QueryPoll::Ready(Err(call_fault(
                                "explicit type needs further source-view integration",
                            )));
                        }
                        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                    };
                    let written = match crate::frontend::written_terms::closed_written_type(
                        db, &sources, &term,
                    ) {
                        QueryPoll::Ready(Ok(Some(written))) => written,
                        QueryPoll::Ready(Ok(None)) => {
                            return QueryPoll::Ready(Err(call_fault(
                                "explicit structural type requires layout integration",
                            )));
                        }
                        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                    };
                    let descriptor = match written.materialize_in(&mut self.store, &[]) {
                        Ok(Some(descriptor)) => descriptor,
                        Ok(None) => {
                            return QueryPoll::Ready(Err(call_fault(
                                "explicit Type argument names no single runtime Type",
                            )));
                        }
                        Err(e) => return QueryPoll::Ready(Err(e)),
                    };
                    (term, descriptor)
                }};
            }
            for (text, parameter) in syntax.iter().zip(parameters) {
                let (term, descriptor) = match names.unqualified(text, None) {
                    Ok(SignatureName::Primitive(p)) => {
                        let term = crate::traits::term::TyTerm::Primitive(p);
                        match crate::frontend::call_contracts::primitive_runtime_id(p) {
                            Ok(ty) => (term, ty),
                            Err(e) => return QueryPoll::Ready(Err(e)),
                        }
                    }
                    Ok(SignatureName::Parameter(key)) => match self.frame.parameter_hint(key) {
                        Ok(Some(ty)) => (crate::traits::term::TyTerm::Param(key), ty),
                        Ok(None) => {
                            return QueryPoll::Ready(Err(Diagnostic::new(
                                "LRR-E-TYPE-CONTEXT",
                                "explicit caller parameter has no reached runtime binding".into(),
                            )));
                        }
                        Err(e) => return QueryPoll::Ready(Err(e)),
                    },
                    Ok(SignatureName::Source(found)) => {
                        match normalizer.normalize(db, found.identity) {
                            QueryPoll::Ready(Ok(
                                crate::frontend::type_aliases::AliasTarget::Type(
                                    term @ crate::traits::term::TyTerm::Primitive(_),
                                ),
                            )) => {
                                let crate::traits::term::TyTerm::Primitive(p) = &term else {
                                    unreachable!("matched a primitive alias target");
                                };
                                match crate::frontend::call_contracts::primitive_runtime_id(*p) {
                                    Ok(ty) => (term, ty),
                                    Err(e) => return QueryPoll::Ready(Err(e)),
                                }
                            }
                            // A nominal, or an alias of a structure.
                            QueryPoll::Ready(Ok(_)) => written_argument!(text),
                            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                        }
                    }
                    // Text that is not a single name: the written-Type service.
                    Ok(_) | Err(_) => written_argument!(text),
                };
                explicit.push((
                    parameter.ordinal,
                    ArgumentType {
                        source: term,
                        descriptor,
                    },
                ));
            }
        }
        explicit.extend(extra);
        self.finish_selected_argument_call(db, callee, explicit, None, argument_slots)
    }
    /// Shared ordinary call context after source Types have been selected.
    /// Entered calls never pass explicit spellings through the legacy adapter.
    /// The decided generic arguments of a call as explicit arguments: each
    /// closed written Type's run-local descriptor and source view.
    fn inferred_explicit(
        &mut self,
        inferred: &crate::ir::recipe::InferredArguments,
    ) -> Result<Vec<(u32, ArgumentType)>, Diagnostic> {
        let mut arguments = Vec::with_capacity(inferred.arguments.len());
        for (ordinal, written) in &inferred.arguments {
            let (Some(source), Some(descriptor)) =
                (written.source(), written.materialize(&mut self.store)?)
            else {
                continue;
            };
            arguments.push((*ordinal, ArgumentType { source, descriptor }));
        }
        Ok(arguments)
    }
    fn finish_selected_argument_call(
        &mut self,
        db: &mut Database,
        callee: Arc<SourceCallable>,
        mut explicit: Vec<(u32, ArgumentType)>,
        typed: Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut owner_application = None;
        // A provided trait body's implicit `Self` is the actual Type qualifier
        // selected at this call's source occurrence, even with no value args.
        // Method receivers and already-bound applications retain their normal
        // argument/contract inference; only an actual Path callee uses this.
        if let Some(parameter) = callee
            .signature()
            .generics
            .iter()
            .find(|parameter| {
                parameter.kind == crate::syntax::signatures::GenericKind::Type
                    && parameter.name == "Self"
            })
            .filter(|parameter| {
                !explicit
                    .iter()
                    .any(|(ordinal, _)| *ordinal == parameter.ordinal)
            })
        {
            let path = self
                .frame
                .body
                .nodes()
                .get(self.frame.pc().node as usize)
                .filter(|node| {
                    node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Call,
                        )
                })
                .and_then(|node| node.operands.first())
                .copied()
                .filter(|path| {
                    self.frame
                        .body
                        .nodes()
                        .get(path.0 as usize)
                        .is_some_and(|node| {
                            node.role
                                == crate::syntax::body_index::NodeRole::Expression(
                                    crate::syntax::body_index::ExpressionKind::Path,
                                )
                        })
                });
            if let Some(path) = path {
                let qualifier_key = QueryKey::AssociatedItemQualifier(PcKey {
                    body: self.frame.pc().body.clone(),
                    node: path.0,
                    phase: 0,
                });
                let selected = match db.fetch(&SyntaxProvider, qualifier_key.clone()) {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let QueryValue::AssociatedItemQualifier(qualifier) = selected.value.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "associated qualifier query variant differs",
                    )));
                };
                if !selected.cacheable {
                    return QueryPoll::Ready(Err(call_fault(
                        "associated qualifier Type is not retained",
                    )));
                }
                if let Some(source) = qualifier.source() {
                    let descriptor = match source.descriptor(&self.frame, &mut self.store) {
                        Ok(descriptor) => descriptor,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    };
                    explicit.push((
                        parameter.ordinal,
                        ArgumentType {
                            source: self.frame.bound_view(source.source(), &self.store),
                            descriptor,
                        },
                    ));
                } else if qualifier.pending().is_some() && !qualifier.permits_trait_self_inference()
                {
                    return QueryPoll::Ready(Err(call_fault(
                        "associated call qualifier requires its actual source Type application",
                    )));
                }
                // Both a selected Type and a positively selected trait use
                // this retained query authority through argument effects.
                // Only the latter may defer Self to ordinary call inference.
                if qualifier.source().is_some() || qualifier.permits_trait_self_inference() {
                    let guard = match db.prepare_input_guard(
                        &SyntaxProvider,
                        &qualifier_key,
                        selected.fingerprint,
                    ) {
                        QueryPoll::Ready(Ok(guard)) => Arc::new(guard),
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    };
                    if let Err(error) = guard.require_current(db) { return QueryPoll::Ready(Err(error)); }
                    owner_application = Some((Arc::clone(&selected.value), guard));
                }
            }
        }
        // Generic arguments later code decided for this call, when the
        // selected callee is the declaration they were decided for: the same
        // explicit arguments a written turbofish would supply.
        if let Some((pc, definition, inferred)) = &self.inferred_call
            && pc == self.frame.pc()
            && *definition == callee.signature().definition
        {
            for (ordinal, argument) in inferred {
                if !explicit.iter().any(|(written, _)| written == ordinal) {
                    explicit.push((*ordinal, argument.clone()));
                }
            }
        }
        let mut hints = if argument_slots.is_empty() {
            Vec::new()
        } else {
            match prepare_argument_hint(db, &callee, graph, &sources, 0) {
                QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(h))) => h,
                QueryPoll::Ready(Ok(other)) => {
                    return QueryPoll::Ready(Err(call_fault(&format!(
                        "argument source hints require further type integration: {}",
                        other.describe()
                    ))));
                }
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            }
        };
        let result_slot = self
            .frame
            .runtime_call
            .as_ref()
            .map(|call| call.result())
            .or(self.frame.body.nodes()[self.frame.pc().node as usize].value);
        let result_hint = match result_slot
            .map(|slot| self.call_result_source_hint(slot))
            .transpose()
        {
            Ok(hint) => hint.flatten(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        for hint in &mut hints {
            argument_hints::apply_context(hint, &callee, &explicit, result_hint.as_ref());
        }
        let hint_capacity = callee
            .signature()
            .parameters
            .len()
            .min(argument_slots.len());
        let reserved_bytes = hint_capacity
            .checked_mul(std::mem::size_of::<ArgumentHint>())
            .and_then(|n| {
                argument_slots
                    .len()
                    .checked_mul(std::mem::size_of::<SlotId>())
                    .and_then(|slots| n.checked_add(slots))
            });
        let Some(mut bytes) = explicit
            .capacity()
            .checked_mul(std::mem::size_of::<(u32, ArgumentType)>())
            .and_then(|n| n.checked_add(callee.signature().estimated_bytes()))
            .and_then(|n| n.checked_add(callee.retained_lexical_metadata_bytes()))
            .and_then(|n| {
                n.checked_add(if result_hint.is_some() {
                    std::mem::size_of::<ArgumentType>()
                } else {
                    0
                })
            })
            .and_then(|n| reserved_bytes.and_then(|reserved| n.checked_add(reserved)))
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<SlotId>>()))
        else {
            return QueryPoll::Ready(Err(call_fault("explicit call context size overflow")));
        };
        if let Some((_, guard)) = &owner_application {
            bytes = match bytes.checked_add(guard.metadata_bytes()).and_then(|bytes|
                bytes.checked_add(4 * std::mem::size_of::<usize>())) {
                Some(bytes) => bytes,
                None => return QueryPoll::Ready(Err(crate::runtime::arena::resource())),
            };
        }
        bytes = match explicit
            .iter()
            .try_fold(bytes, |bytes, (_, argument)| {
                bytes.checked_add(argument.source.heap_bytes()?)
            })
            .and_then(|bytes| {
                bytes.checked_add(typed.as_ref().map_or(0, |plan| plan.metadata_bytes()))
            }) {
            Some(bytes) => bytes,
            None => return QueryPoll::Ready(Err(crate::runtime::arena::resource())),
        };
        for hint in &hints {
            bytes = match argument_hints::retained_hint_bytes(hint).and_then(|heap| {
                bytes
                    .checked_add(heap)
                    .ok_or_else(crate::runtime::arena::resource)
            }) {
                Ok(bytes) => bytes,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        }
        if hints
            .try_reserve_exact(hint_capacity.saturating_sub(hints.len()))
            .is_err()
        {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        let mut slots = Vec::new();
        if slots.try_reserve_exact(argument_slots.len()).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        slots.extend_from_slice(argument_slots);
        // Account for actual allocator-reported capacities before publication.
        let actual_reserved = hints
            .capacity()
            .checked_mul(std::mem::size_of::<ArgumentHint>())
            .and_then(|n| {
                slots
                    .capacity()
                    .checked_mul(std::mem::size_of::<SlotId>())
                    .and_then(|slots| n.checked_add(slots))
            });
        bytes = match actual_reserved.and_then(|actual| {
            reserved_bytes
                .and_then(|planned| actual.checked_sub(planned))
                .and_then(|extra| bytes.checked_add(extra))
        }) {
            Some(n) => n,
            None => return QueryPoll::Ready(Err(crate::runtime::arena::resource())),
        };
        let expected = match self.expected_result_parameters(db, &callee, result_hint.as_ref()) {
            QueryPoll::Ready(Ok(expected)) => expected,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(SelectedArgumentCall {
            callee,
            explicit,
            typed,
            owner_application,
            hints,
            argument_slots: slots,
            result_hint,
            source_input_attempt: None,
            source_inputs: None,
            source_response: None,
            source_input_bindings: None,
            expected,
            bytes,
        }))
    }
    /// The callee type parameters the reached result context fixes through
    /// the declared result (`Box::new(1234)` where `Box<isize>` is expected
    /// fixes `T = isize`): the expectation each argument is evaluated under,
    /// like rustc's expected inputs for an expected output. Advisory only;
    /// the evaluated contract binds and checks the actual arguments.
    fn expected_result_parameters(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
        result: Option<&ArgumentType>,
    ) -> QueryPoll<BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>> {
        let mut expected = BTreeMap::new();
        let signature = callee.signature();
        let Some(result) = result else {
            return QueryPoll::Ready(Ok(expected));
        };
        if !signature
            .generics
            .iter()
            .any(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
        {
            return QueryPoll::Ready(Ok(expected));
        }
        let template = match &signature.result_symbolic {
            Some(term) => term.clone(),
            None => {
                let Some(graph) = &self.graph else {
                    return QueryPoll::Ready(Ok(expected));
                };
                let sources: Vec<_> = self.sources.iter().collect();
                match crate::frontend::source_types::resolve_written_type(
                    db,
                    graph,
                    &sources,
                    signature,
                    &signature.result,
                ) {
                    QueryPoll::Ready(Ok(Some(term))) => term,
                    QueryPoll::Ready(_) => return QueryPoll::Ready(Ok(expected)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
        };
        if crate::frontend::call_contracts::bind_runtime_parameters(
            &template,
            result.descriptor,
            &self.store,
            signature.definition,
            &mut expected,
        )
        .is_err()
        {
            expected.clear();
        }
        QueryPoll::Ready(Ok(expected))
    }
    /// A physical expectation may precede the genuine returned source. Keep
    /// that descriptor in the slot without inventing a region or nominal type.
    fn call_result_source_hint(&self, slot: SlotId) -> Result<Option<ArgumentType>, Diagnostic> {
        let Some(descriptor) = self.frame.hint(slot)? else {
            return Ok(None);
        };
        self.store.type_descriptor(descriptor)?;
        if let Some(source) = self.frame.source_view(slot)? {
            return Ok(Some(ArgumentType { source, descriptor }));
        }
        // These are the fixed builtin source IDs handled by the same ordinary
        // primitive conversion.
        if matches!(descriptor.0, 1..=15 | 17 | 18) {
            return primitive_argument_type(descriptor).map(Some);
        }
        // A written nominal/tuple/array context (`let v: Option<u16> = ..`)
        // materialized its closed descriptor: its structure is the source.
        // References carry no region here and supply no source authority.
        // A reference that is a type argument of the context's own nominal
        // (`Vec<&str>`, `Option<&T>`) is part of that Type's structure: the
        // descriptor records its pointee exactly and its region is erased
        // like every runtime reference's (`descriptor_source_term`), so it
        // supplies the argument's identity but no region authority. A
        // top-level reference context still supplies none.
        fn reference_free(term: &crate::traits::term::TyTerm, nested: bool) -> bool {
            use crate::traits::term::{GenericArg, TyTerm};
            match term {
                TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Dyn(_) => true,
                TyTerm::Nominal(_, args) => args.iter().all(|arg| match arg {
                    GenericArg::Type(ty) => reference_free(ty, true),
                    _ => true,
                }),
                TyTerm::Tuple(items) => items.iter().all(|item| reference_free(item, nested)),
                TyTerm::Array(item, _) | TyTerm::Slice(item) => reference_free(item, nested),
                TyTerm::Ref(_, _, target) => nested && reference_free(target, true),
                _ => false,
            }
        }
        Ok(self
            .store
            .descriptor_source_term(descriptor)
            .filter(|source| {
                matches!(
                    source,
                    crate::traits::term::TyTerm::Nominal(..)
                        | crate::traits::term::TyTerm::Tuple(_)
                        | crate::traits::term::TyTerm::Array(..)
                ) && reference_free(source, false)
            })
            .map(|source| ArgumentType { source, descriptor }))
    }
    fn prepare_source_call(
        &mut self,
        db: &mut Database,
        name: &str,
        selected: Option<SelectedArgumentCall>,
        arguments: &[ArgumentType],
        result_hint: Option<&ArgumentType>,
        syntax: &[String],
    ) -> QueryPoll<PreparedSourceCall> {
        let selected = if let Some(selected) = selected {
            selected
        } else {
            match self.select_common_argument_call(db, name, syntax, &[]) {
                QueryPoll::Ready(Ok(Some(selected))) => selected,
                QueryPoll::Ready(Ok(None)) => {
                    match self.select_argument_call(db, name, syntax, &[]) {
                        QueryPoll::Ready(Ok(call)) => call,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
        if let Some(typed) = &selected.typed {
            if let Err(error) = typed.require_current(db, &self.frame.body, self.frame.pc()) {
                return QueryPoll::Ready(Err(error));
            }
        }
        if let Some((_, guard)) = &selected.owner_application {
            if let Err(error) = guard.require_current(db) { return QueryPoll::Ready(Err(error)); }
        }
        let mut references = Vec::new();
        if arguments
            .iter()
            .any(|argument| matches!(argument.source, crate::traits::term::TyTerm::Ref(..)))
        {
            if references.try_reserve_exact(arguments.len()).is_err() {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            for (ordinal, argument) in arguments.iter().enumerate() {
                // Only closed reference parameters consume owning-frame
                // evidence; generic reference parameters bind structurally.
                let closed_parameter = selected
                    .hints
                    .get(ordinal)
                    .is_none_or(|hint| hint.closed_reference.is_some());
                let evidence = if closed_parameter
                    && matches!(argument.source, crate::traits::term::TyTerm::Ref(..))
                {
                    let Some(slot) = selected.argument_slots.get(ordinal) else {
                        return QueryPoll::Ready(Err(call_fault(
                            "reference source argument slot absent",
                        )));
                    };
                    match ClosedReferenceArgument::capture(&self.frame, *slot, &self.store) {
                        Ok(evidence) => Some(evidence),
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                } else {
                    None
                };
                references.push(evidence);
            }
        }
        let SelectedArgumentCall {
            callee, explicit, source_response, ..
        } = selected;
        match self.prepare_selected_source_call(
            db, callee, &explicit, result_hint, arguments, &references,
        ) {
            QueryPoll::Ready(Ok(mut call)) => {
                if let Some(response) = &source_response {
                    if let Err(error) = response.require_current(db) {
                        return QueryPoll::Ready(Err(error));
                    }
                }
                call.source_response = source_response;
                QueryPoll::Ready(Ok(call))
            }
            other => other,
        }
    }
    /// Shared entry preparation of a selected source callee for already
    /// evaluated arguments: owner environment, contracts (closed reference
    /// parameters consume their actual evidence), body and definition context.
    fn prepare_selected_source_call(
        &mut self,
        db: &mut Database,
        callee: Arc<SourceCallable>,
        explicit: &[(u32, ArgumentType)],
        result_hint: Option<&ArgumentType>,
        arguments: &[ArgumentType],
        references: &[Option<ClosedReferenceArgument>],
    ) -> QueryPoll<PreparedSourceCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("call source catalogue absent")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        // An admitted local owner needs its derived environment before revision-
        // bound contracts. Other source calls retain contract-before-body order.
        // This is reached call preparation, never a signature/type-goal demand.
        let prepared_body = if db
            .input(&crate::db::dependency::InputKey::LocalDeclarationOwner(
                callee.identity(),
            ))
            .is_some()
        {
            if let Err(error) = crate::frontend::call_contracts::preflight_evaluated_call(
                &callee,
                explicit,
                result_hint,
                arguments,
            ) {
                return QueryPoll::Ready(Err(error));
            }
            let body = match Self::selected_source_body(db, &callee) {
                QueryPoll::Ready(Ok(body)) => body,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match self.prepare_reached_local_source(db, &callee, &body) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            Some(body)
        } else if callee.body().is_some_and(|handle| {
            db.input(&crate::db::dependency::InputKey::BodyEnvironment(
                handle.key(),
            ))
            .is_none()
        }) && matches!(self.graph, Some(RunNames::Completed(_)))
        {
            // A module/impl callee selected by ordinary resolution owns the same
            // derived body environment as an entered-goal callee. Publication
            // advances the revision, so it precedes revision-bound contracts.
            if let Err(error) = crate::frontend::call_contracts::preflight_evaluated_call(
                &callee,
                explicit,
                result_hint,
                arguments,
            ) {
                return QueryPoll::Ready(Err(error));
            }
            let body = match Self::selected_source_body(db, &callee) {
                QueryPoll::Ready(Ok(body)) => body,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match self.prepare_selected_module_source(db, &callee, &body) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            Some(body)
        } else {
            None
        };
        let reference_arguments = arguments
            .iter()
            .any(|argument| matches!(argument.source, crate::traits::term::TyTerm::Ref(..)));
        let prepared = if reference_arguments {
            prepare_contracts_with_references(
                db,
                &callee,
                graph,
                &sources,
                explicit,
                result_hint,
                ReferenceCallArguments {
                    store: &self.store,
                    arguments,
                    references,
                },
            )
        } else {
            crate::frontend::call_contracts::prepare_contracts_in_store(
                db,
                &callee,
                graph,
                &sources,
                explicit,
                result_hint,
                arguments,
                &self.store,
            )
        };
        let mut contracts = match prepared {
            QueryPoll::Ready(Ok(ContractPreparation::Ready(c))) => c,
            QueryPoll::Ready(Ok(other)) => {
                let detail = match other {
                    ContractPreparation::NeedType { syntax, .. } => format!(
                        "type `{}`",
                        syntax.iter().map(|t| t.text.as_str()).collect::<String>()
                    ),
                    ContractPreparation::NeedAlias(alias) => format!("alias {alias:?}"),
                    ContractPreparation::NeedName(name) => format!("name {name:?}"),
                    ContractPreparation::Ready(_) => unreachable!("matched above"),
                };
                return QueryPoll::Ready(Err(call_fault(&format!(
                    "call contracts require later source type integration: {detail}"
                ))));
            }
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        match contracts.require_inference_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        // A generic result mentioning an associated type of its bound
        // parameters (`-> Result<F, F::Err>`) is normalized with them.
        if let Some(result) = contracts.source_result.as_ref()
            && crate::frontend::associated_types::contains_projection(result)
            && !crate::frontend::call_contracts::closed_term(result)
            && let Some(members) = self.members.clone()
        {
            let bindings = contracts
                .runtime_substitution
                .iter()
                .filter_map(|(key, ty)| {
                    self.store
                        .descriptor_source_term(*ty)
                        .map(|term| (*key, term))
                })
                .collect();
            let bound = crate::frontend::associated_types::substitute(result, &bindings);
            if let QueryPoll::Ready(Ok(Some(normalized))) =
                crate::frontend::associated_types::normalize(db, graph, &sources, &members, &bound)
                && crate::frontend::call_contracts::closed_term(&normalized)
            {
                contracts.source_result = Some(normalized);
            }
        }
        // An associated-type result (`-> T::V`) is normalized with the bound
        // generics; when it cannot be yet, the returned value supplies it.
        let mut projection_result = false;
        if callee.signature().result_symbolic.is_none()
            && contracts.source_result.is_none()
            && callee.signature().result.len() > 1
            && let QueryPoll::Ready(Ok(Some(written))) =
                crate::frontend::source_types::resolve_written_type(
                    db,
                    graph,
                    &sources,
                    callee.signature(),
                    &callee.signature().result,
                )
            && crate::frontend::associated_types::contains_projection(&written)
        {
            projection_result = true;
            if let Some(members) = self.members.clone() {
                let bindings = contracts
                    .runtime_substitution
                    .iter()
                    .filter_map(|(key, ty)| {
                        self.store
                            .descriptor_source_term(*ty)
                            .map(|term| (*key, term))
                    })
                    .collect();
                let bound = crate::frontend::associated_types::substitute(&written, &bindings);
                if let QueryPoll::Ready(Ok(Some(normalized))) =
                    crate::frontend::associated_types::normalize(
                        db, graph, &sources, &members, &bound,
                    )
                    && crate::frontend::call_contracts::closed_term(&normalized)
                {
                    contracts.source_result = Some(normalized);
                }
            }
        }
        // Until symbolic body-hint integration, reject named source return types
        // rather than interpreting a primitive-looking alias as its spelling.
        if callee.signature().result_symbolic.is_none()
            && contracts.source_result.is_none()
            && !projection_result
        {
            let names = match SignatureNames::from_database(db, graph, callee.signature()) {
                Ok(names) => names,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            if callee.signature().result.len() != 1 {
                return QueryPoll::Ready(Err(call_fault(&format!(
                    "symbolic return syntax awaits body-hint integration: `{}`",
                    callee
                        .signature()
                        .result
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect::<String>()
                ))));
            }
            match names.unqualified(&callee.signature().result[0].text, None) {
                Ok(SignatureName::Primitive(primitive)) => {
                    if let Some(hint) = result_hint {
                        let expected = match crate::frontend::call_contracts::primitive_runtime_id(
                            primitive,
                        ) {
                            Ok(descriptor) => descriptor,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                        if hint.descriptor != expected {
                            return QueryPoll::Ready(Err(Diagnostic::new(
                                "LRR-E-CALL-RETURN-TYPE",
                                "resolved primitive return differs from reached result context"
                                    .into(),
                            )));
                        }
                        if hint.source != crate::traits::term::TyTerm::Primitive(primitive) {
                            return QueryPoll::Ready(Err(Diagnostic::new(
                                "LRR-E-GENERIC-BINDING",
                                "resolved primitive return differs from retained result source identity"
                                    .into(),
                            )));
                        }
                    }
                }
                Ok(_) => {
                    return QueryPoll::Ready(Err(call_fault(&format!(
                        "symbolic return identity awaits body-hint integration: `{}`",
                        callee.signature().result[0].text
                    ))));
                }
                Err(e) => return QueryPoll::Ready(Err(e)),
            }
        }
        let body = match prepared_body {
            Some(body) => body,
            None => match Self::selected_source_body(db, &callee) {
                QueryPoll::Ready(Ok(body)) => body,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
        };
        {
            let context_bytes =
                match crate::vm::context::GenericEnv::checked_bytes(callee.signature(), &contracts)
                {
                    Ok(bytes) => bytes,
                    Err(e) => return QueryPoll::Ready(Err(e)),
                };
            // The query cache owns and charges the shared immutable
            // body. This call retains two handles (plan and frame),
            // plus independently allocated mutable execution metadata.
            let bytes = std::mem::size_of::<Arc<BodyIndex>>()
                .checked_mul(2)
                .and_then(|n| {
                    Frame::metadata_bytes(&body, contracts.source_parameters.len())
                        .and_then(|size| n.checked_add(size))
                })
                .and_then(|n| n.checked_add(callee.signature().estimated_bytes()))
                .and_then(|n| {
                    contracts
                        .runtime_substitution
                        .len()
                        .checked_mul(128)
                        .and_then(|size| n.checked_add(size))
                })
                .and_then(|n| {
                    contracts
                        .source_substitution
                        .len()
                        .checked_mul(128)
                        .and_then(|size| n.checked_add(size))
                })
                .and_then(|n| {
                    contracts
                        .source_parameters
                        .len()
                        .checked_mul(std::mem::size_of::<crate::traits::term::TyTerm>())
                        .and_then(|size| n.checked_add(size))
                })
                .and_then(|n| {
                    n.checked_add(
                        contracts
                            .inferred_equations
                            .as_ref()
                            .map_or(0, |guard| guard.metadata_bytes()),
                    )
                })
                .and_then(|n| n.checked_add(context_bytes))
                .and_then(|n| n.checked_add(4096));
            let Some(retained_bytes) = bytes else {
                return QueryPoll::Ready(Err(call_fault("call metadata size overflow")));
            };
            let context =
                match crate::vm::context::GenericEnv::capture_current(db, &callee, &contracts) {
                    Ok(context) => context,
                    Err(e) => return QueryPoll::Ready(Err(e)),
                };
            // The declared result's written structure is instantiated from
            // this call's actual generic descriptors. Closing its source term
            // first can erase a binding's runtime representation (for example
            // a closure capture tuple nested under a nominal result).
            let context = match contracts
                .source_result
                .as_ref()
                .filter(|term| crate::frontend::call_contracts::closed_term(term))
            {
                Some(term) => {
                    let mut written = match crate::frontend::written_terms::closed_written_type(
                        db, &sources, term,
                    ) {
                        QueryPoll::Ready(Ok(written)) => written,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    };
                    if written.is_none() {
                        let declared = match crate::frontend::source_return_goals::prepare(
                            db,
                            graph,
                            &sources,
                            callee.signature().definition,
                        ) {
                            QueryPoll::Ready(Ok(declared)) => declared,
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        };
                        if let crate::frontend::source_return_goals::ReturnTypeGoal::Ready(proof) =
                            declared
                        {
                            if let Err(error) = proof.require(
                                callee.signature().definition,
                                QueryValue::FunctionSignature(Box::new(callee.signature().clone()))
                                    .fingerprint(),
                            ) {
                                return QueryPoll::Ready(Err(error));
                            }
                            written = match crate::frontend::written_terms::written_template(
                                db,
                                &sources,
                                proof.source(),
                                callee.signature().definition,
                            ) {
                                QueryPoll::Ready(Ok(written)) => written,
                                QueryPoll::Ready(Err(error)) => {
                                    return QueryPoll::Ready(Err(error));
                                }
                                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                            };
                        }
                    }
                    let written = written.map(Arc::new);
                    let context = match written.as_ref() {
                        Some(written) => {
                            match context.with_written_result_template(Arc::clone(written)) {
                                Ok(context) => context,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            }
                        }
                        None => context,
                    };
                    let descriptor = match written {
                        Some(written) => {
                            match written.materialize_with(&mut self.store, &|ordinal| {
                                contracts
                                    .runtime_substitution
                                    .get(&crate::traits::term::ParamKey {
                                        definition: callee.signature().definition,
                                        ordinal,
                                    })
                                    .copied()
                            }) {
                                Ok(descriptor) => descriptor,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            }
                        }
                        None => None,
                    };
                    match descriptor {
                        Some(descriptor) => context.with_written_result(term.clone(), descriptor),
                        None => context,
                    }
                }
                None => context,
            };
            let retained_bytes = match context
                .retained_bytes()
                .checked_sub(context_bytes)
                .and_then(|additional| retained_bytes.checked_add(additional))
            {
                Some(bytes) => bytes,
                None => {
                    return QueryPoll::Ready(Err(call_fault(
                        "declared result retention size overflow",
                    )));
                }
            };
            let body_lease = match db.pin_guest_body(body.body()) {
                QueryPoll::Ready(Ok(lease)) => lease,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            QueryPoll::Ready(Ok(PreparedSourceCall {
                source_response: None,
                context: Some(context),
                callee,
                contracts,
                body,
                body_lease: Some(body_lease),
                retained_bytes,
                closure: None,
            }))
        }
    }
    /// Enter an accepted callee frame with its sealed argument packet and
    /// suspend the current frame under `continuation`. Shared by recipe call
    /// sites and drop glue's `Drop::drop` witness.
    fn enter_callee(
        &mut self,
        mut call: PreparedSourceCall,
        mut callee: Frame,
        args: &crate::vm::call::ArgumentPacket,
        continuation: CallContinuation,
    ) -> Result<(), Diagnostic> {
        self.calls
            .try_reserve(1)
            .map_err(|_| call_fault("call continuation allocation failed"))?;
        let callee_lease = call
            .body_lease
            .take()
            .ok_or_else(|| call_fault("sealed callee source lease absent"))?;
        if callee_lease.body() != call.body.body() {
            return Err(call_fault("sealed callee source lease differs"));
        }
        let contracts = match &call.closure {
            Some(closure) => call
                .contracts
                .parameter_descriptors
                .iter()
                .copied()
                .zip(closure.mutable.iter().copied())
                .collect::<Vec<_>>(),
            None => call
                .contracts
                .parameter_descriptors
                .iter()
                .zip(&call.callee.signature().parameters)
                .map(|(ty, param)| {
                    (
                        *ty,
                        param
                            .pattern
                            .first()
                            .is_some_and(|token| token.text == "mut"),
                    )
                })
                .collect::<Vec<_>>(),
        };
        // A closure frame has no definition environment of its own.
        if call.closure.is_none() {
            callee.install_context(
                call.context.take().ok_or_else(|| {
                    call_fault("accepted definition environment already consumed")
                })?,
            )?;
        }
        callee.install_parameter_sources(call.contracts.source_parameters.clone())?;
        callee.initialize_parameters(args, &contracts, &mut self.store)?;
        if let Some(closure) = &call.closure {
            if let Some(context) = &closure.context {
                callee.install_closure_context(Arc::clone(context))?;
            }
            if let Some(result) = call.contracts.result_descriptor {
                callee.declared_hint(closure.result, Some(result))?;
                callee.set_source_view(closure.result, call.contracts.source_result.clone())?;
            }
            callee.bind_closure_captures(
                &closure.captures,
                closure.mutable.len(),
                &closure.closure,
                &self.store,
            )?;
        }
        let call_bytes = self
            .call_bytes
            .checked_add(call.retained_bytes)
            .ok_or_else(|| call_fault("call metadata size overflow"))?;
        let depth = self
            .calls
            .len()
            .checked_add(2)
            .and_then(|depth| u32::try_from(depth).ok())
            .ok_or_else(|| call_fault("guest frame depth overflow"))?;
        self.collected.accumulate(self.frame.counters())?;
        let pending = std::mem::take(&mut self.pending);
        self.frame.counters = OpcodeCounters::default();
        let caller = std::mem::replace(&mut self.frame, callee);
        let body_lease = std::mem::replace(&mut self.body_lease, callee_lease);
        self.call_bytes = call_bytes;
        self.peak_guest_frames = self.peak_guest_frames.max(depth);
        self.calls.push(SuspendedCall {
            continuation,
            caller,
            body_lease,
            call,
            pending,
        });
        Ok(())
    }
    fn check_source_return(
        &self,
        db: &mut Database,
        call: &PreparedSourceCall,
        actual: crate::runtime::shape::TypeId,
    ) -> Result<(), Diagnostic> {
        call.callee.revalidate(db)?;
        if let Some(response) = &call.source_response { response.require_current(db)?; }
        match call.contracts.require_inference_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => {
                return Err(call_fault(
                    "inferred call dependencies require source preparation",
                ));
            }
        }
        // A closure's result is its body's reached value; no declared
        // result constrains it here.
        if call.closure.is_some() {
            return Ok(());
        }
        if let Some(proof) = call
            .contracts
            .closed_references
            .as_ref()
            .filter(|proof| proof.has_result())
        {
            proof.validate_packet(call.callee.signature(), &call.contracts)?;
            return call
                .contracts
                .check_return_descriptor_in(actual, Some(&self.store));
        }
        // An opaque/trait-object result accepts the returned concrete value.
        if call
            .contracts
            .source_result
            .as_ref()
            .is_some_and(crate::frontend::call_contracts::contains_object)
        {
            return Ok(());
        }
        // A closed nominal/tuple result: the returned value's exact runtime
        // type must be that declared source type.
        if let Some(expected) = call.contracts.source_result.as_ref().filter(|term| {
            !matches!(term, crate::traits::term::TyTerm::Primitive(_))
                && crate::frontend::call_contracts::closed_term(term)
        }) {
            let found = match self.frame.return_source.as_ref() {
                Some(found) => Some(found.clone()),
                None => self.store.descriptor_source_term(actual),
            };
            // Regions are not runtime data: a returned `&'x [T]` view meets
            // its declared `&'a [T]` result up to region names.
            // A function item returned at a declared fn-pointer result
            // (a `static f: fn() = foo;` initializer) is the pointer's value.
            let function_item = matches!(expected, crate::traits::term::TyTerm::Function(_))
                && self.store.type_descriptor(actual).is_ok_and(|descriptor| {
                    matches!(
                        descriptor.key.as_ref(),
                        crate::runtime::shape::TypeKey::Callable { captures, .. }
                            if captures.is_empty()
                    )
                });
            if found
                .as_ref()
                .map(crate::frontend::signature_subst::erase_regions)
                != Some(crate::frontend::signature_subst::erase_regions(expected))
                && !found.as_ref().is_some_and(|found| {
                    crate::frontend::signature_subst::erase_regions(expected)
                        .admits_dyn(&crate::frontend::signature_subst::erase_regions(found))
                })
                && !function_item
                && !found.as_ref().is_some_and(|found| {
                    crate::traits::pointer_terms::same_pointer_family(found, expected)
                })
            {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-RETURN-TYPE",
                    "returned value differs from the closed declared result type".into(),
                ));
            }
            return Ok(());
        }
        if call.contracts.result_descriptor.is_some() {
            call.contracts
                .check_return_descriptor_in(actual, Some(&self.store))?;
            if let (Some(expected @ crate::traits::term::TyTerm::Primitive(_)), Some(found)) = (
                call.contracts.source_result.as_ref(),
                self.frame.return_source.as_ref(),
            ) && expected != found
            {
                return Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "returned source identity differs from admitted primitive result".into(),
                ));
            }
            return Ok(());
        }
        // A declared result still open at the call (`Empty<T>`, `Result<F,
        // <F as FromStr>::Err>`): the returned value is compared with its
        // template structurally and supplies the open positions.
        if call.contracts.has_open_result() {
            let found = match self.frame.return_source.as_ref() {
                Some(found) => Some(found.clone()),
                None => self.store.descriptor_source_term(actual),
            };
            return call.contracts.check_open_return_source(found.as_ref());
        }
        // A normalized associated-type primitive result.
        if let Some(crate::traits::term::TyTerm::Primitive(primitive)) =
            call.contracts.source_result.as_ref()
        {
            if actual != crate::frontend::call_contracts::primitive_runtime_id(*primitive)? {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-RETURN-TYPE",
                    "returned value differs from the normalized declared result type".into(),
                ));
            }
            return Ok(());
        }
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| call_fault("return source catalogue absent"))?;
        let signature = call.callee.signature();
        if signature.result.len() > 1
            && call.contracts.source_result.is_none()
            && let QueryPoll::Ready(Ok(Some(written))) =
                crate::frontend::source_types::resolve_written_type(
                    db,
                    graph,
                    &self.sources.iter().collect::<Vec<_>>(),
                    signature,
                    &signature.result,
                )
            && crate::frontend::associated_types::contains_projection(&written)
        {
            // Not yet normalizable: the well-typed returned value supplies it.
            return Ok(());
        }
        let names = SignatureNames::from_database(db, graph, signature)?;
        if signature.result.len() != 1 {
            return Err(call_fault(
                "reached return requires further source type resolution",
            ));
        }
        let SignatureName::Primitive(p) = names.unqualified(&signature.result[0].text, None)?
        else {
            return Err(call_fault(
                "reached return requires alias or generic source resolution",
            ));
        };
        let expected = crate::frontend::call_contracts::primitive_runtime_id(p)?;
        if actual != expected {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "returned value differs from selected source signature".into(),
            ));
        }
        if self
            .frame
            .return_source
            .as_ref()
            .is_some_and(|source| source != &crate::traits::term::TyTerm::Primitive(p))
        {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "returned source identity differs from resolved primitive signature".into(),
            ));
        }
        Ok(())
    }
    fn retire_pending_frame(&mut self) -> Result<(), Diagnostic> {
        let bytes = self.pending.values().try_fold(0usize, |total, call| {
            total
                .checked_add(call.bytes)
                .ok_or_else(|| call_fault("pending context size overflow"))
        })?;
        self.pending_bytes = self
            .pending_bytes
            .checked_sub(bytes)
            .ok_or_else(|| call_fault("pending context retirement underflow"))?;
        self.pending.clear();
        Ok(())
    }
    fn all_counters(&self) -> Result<OpcodeCounters, Diagnostic> {
        let mut counters = self.collected.clone();
        counters.accumulate(self.frame.counters())?;
        Ok(counters)
    }
    /// Queue resolved syntax facts; LRIR is emitted only if the owned macro PC is reached.
    pub fn queue_static_panic_plan(
        &mut self,
        db: &Database,
        source: PreparedSourceView,
        plan: Box<crate::frontend::runtime_macro_plan::RuntimeIntrinsicPlan>,
    ) -> Result<(), Diagnostic> {
        if self.terminal
            || self.frame.fuel_snapshot().used != 0
            || db.source_revision() != self.installed.source_revision()
        {
            return Err(call_fault(
                "static panic plans require an unexecuted current run",
            ));
        }
        if !plan.arguments().kind().is_static_panic() {
            return Err(call_fault(
                "static panic queue requires its resolved panic-family definition",
            ));
        }
        if !self
            .sources
            .iter()
            .any(|s| s.same_publication(&source) && s.selection() == source.selection())
        {
            return Err(call_fault("static panic source is outside run catalogue"));
        }
        plan.arguments()
            .require_source(&source.selection().outline.parse)?;
        let r = plan.invocation().range();
        let handles = source
            .outline()
            .items
            .iter()
            .filter_map(|i| i.body.as_ref())
            .filter(|h| h.range.contains(r))
            .collect::<Vec<_>>();
        if handles.len() != 1 {
            return Err(call_fault(
                "static panic invocation body is absent or ambiguous",
            ));
        }
        let handle = handles[0];
        handle.require_entry()?;
        let identity = handle.key().fingerprint();
        let start = r
            .start
            .checked_sub(handle.range.start)
            .ok_or_else(|| call_fault("static panic invocation precedes body"))?;
        let end = r
            .end
            .checked_sub(handle.range.start)
            .ok_or_else(|| call_fault("static panic invocation precedes body"))?;
        let key = (identity, start, end);
        if self
            .static_panic_body_bases
            .get(&identity)
            .is_some_and(|base| *base != handle.range.start)
        {
            return Err(call_fault("static panic source body attachment differs"));
        }
        if self.static_panic_plans.contains_key(&key) {
            return Err(call_fault("duplicate static panic plan"));
        }
        self.static_panic_body_bases
            .insert(identity, handle.range.start);
        self.static_panic_plans.insert(key, (source, plan));
        Ok(())
    }
    pub(crate) fn query_trace_start(&self) -> usize {
        self.query_trace_start
    }
    pub(crate) fn root_body(&self) -> &BodyKey {
        &self.root_body
    }
    pub(crate) fn enable_runtime_capture(&mut self) -> Result<(), Diagnostic> {
        if self.terminal
            || self.frame.fuel_snapshot().used != 0
            || !self.calls.is_empty()
            || self.runtime_capture.is_some()
        {
            return Err(Diagnostic::new(
                "LRR-E-RUNTIME-CAPTURE",
                "capture requires a fresh unstarted interpreter".into(),
            ));
        }
        self.runtime_capture = Some(Default::default());
        Ok(())
    }
    pub(crate) fn runtime_capture_facts(&self) -> Option<&capture::CaptureFacts> {
        self.runtime_capture.as_ref().map(|capture| capture.facts())
    }
    pub(crate) fn terminal_facts(&self) -> Option<&TerminalFacts> {
        self.terminal_facts.as_ref()
    }
    /// Borrowed failure metadata only; this neither queries nor mutates a frame.
    pub(crate) fn diagnostic_frames(&self) -> impl Iterator<Item = (&PcKey, &OpcodeCounters)> {
        std::iter::once((&self.frame.pc, &self.frame.counters)).chain(
            self.calls
                .iter()
                .rev()
                .map(|call| (&call.caller.pc, &call.caller.counters)),
        )
    }
    pub(crate) fn diagnostic_frame_spans(
        &self,
    ) -> impl Iterator<
        Item = Result<(crate::core::digest::Digest, crate::syntax::spans::BodySpan), Diagnostic>,
    > {
        std::iter::once(&self.frame)
            .chain(self.calls.iter().rev().map(|call| &call.caller))
            .map(|frame| {
                frame
                    .body
                    .span(NodeId(frame.pc.node))
                    .map(|span| (frame.pc.fingerprint(), span))
            })
    }
    pub(crate) fn diagnostic_source_origin(
        &self,
        span: &crate::syntax::spans::BodySpan,
    ) -> Result<Option<crate::syntax::origin::SourceOrigin>, Diagnostic> {
        use crate::syntax::{
            origin::SourceOrigin,
            spans::SourceRange,
            token_wire::{Key, SourceIdentity},
        };
        let invalid = || {
            Diagnostic::new(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "guest frame source owner differs".into(),
            )
        };
        let mut found: Option<SourceOrigin> = None;
        for source in &self.sources {
            for handle in source
                .outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .filter(|handle| handle.key() == span.body_key)
            {
                if handle.entry_blocked
                    || span.relative_start > span.relative_end
                    || span.relative_end > handle.range.len()
                {
                    return Err(invalid());
                }
                let range = SourceRange::checked(
                    handle
                        .range
                        .start
                        .checked_add(span.relative_start)
                        .ok_or_else(invalid)?,
                    handle
                        .range
                        .start
                        .checked_add(span.relative_end)
                        .ok_or_else(invalid)?,
                )?;
                let selection = source.selection();
                if let Some(old) = &found {
                    if old.source.unit.0 != selection.outline.crate_key.0
                        || old.source.content.0 != selection.outline.parse.content.0
                        || old.source.path != selection.path.as_str()
                        || old.range != range
                    {
                        return Err(invalid());
                    }
                    continue;
                }
                let mut path = String::new();
                path.try_reserve_exact(selection.path.as_str().len())
                    .map_err(|_| invalid())?;
                path.push_str(selection.path.as_str());
                found = Some(SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(selection.outline.crate_key.0),
                        content: Key(selection.outline.parse.content.0),
                        path,
                    },
                    range,
                });
            }
        }
        Ok(found)
    }
    pub(crate) fn diagnostic_current_span(
        &self,
    ) -> Result<crate::syntax::spans::BodySpan, Diagnostic> {
        self.frame.body.span(NodeId(self.frame.pc.node))
    }
    pub(crate) fn diagnostic_invocation_origin(
        &self,
        identity: crate::core::digest::Digest,
    ) -> Result<Option<crate::syntax::origin::SourceOrigin>, Diagnostic> {
        use crate::syntax::{
            origin::SourceOrigin,
            token_wire::{Key, SourceIdentity},
        };
        let invalid = || {
            Diagnostic::new(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "generated fault invocation has conflicting owners".into(),
            )
        };
        let mut found: Option<SourceOrigin> = None;
        for source in &self.sources {
            for call in source
                .outline()
                .item_invocations
                .iter()
                .filter(|call| call.item_input == identity && call.attribute_input.is_none())
            {
                let invocation = source.macro_call_at(call.range)?;
                let selected = source.selection();
                if let Some(old) = &found {
                    if old.source.unit.0 != selected.outline.crate_key.0
                        || old.source.content.0 != selected.outline.parse.content.0
                        || old.source.path != selected.path.as_str()
                        || old.range != invocation.range()
                    {
                        return Err(invalid());
                    }
                    continue;
                }
                let mut path = String::new();
                path.try_reserve_exact(selected.path.as_str().len())
                    .map_err(|_| invalid())?;
                path.push_str(selected.path.as_str());
                found = Some(SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(selected.outline.crate_key.0),
                        content: Key(selected.outline.parse.content.0),
                        path,
                    },
                    range: invocation.range(),
                });
            }
        }
        Ok(found)
    }
    pub(crate) fn diagnostic_definition_origin(
        &self,
        identity: &crate::expand::identity::MacroIdentity,
    ) -> Result<Option<crate::syntax::origin::DefinitionOrigin>, Diagnostic> {
        use crate::syntax::{
            origin::{DefinitionOrigin, SourceOrigin},
            token_wire::{Key, SourceIdentity},
        };
        let crate::expand::identity::MacroIdentity::Declaration { definition } = identity else {
            return Ok(None);
        };
        let invalid = || {
            Diagnostic::new(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "generated fault definition has conflicting owners".into(),
            )
        };
        let mut found: Option<DefinitionOrigin> = None;
        for source in &self.sources {
            if !source.outline().items.iter().any(|item| {
                item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                    && item.def_key.fingerprint() == *definition
            }) {
                continue;
            }
            let selected = source.selection();
            // Reads the retained parsed syntax. No parse or provider is invoked.
            let (range, name) = source.macro_definition_header(*definition)?;
            if let Some(old) = &found {
                if old.name != name
                    || old.location.range != range
                    || old.location.source.unit.0 != selected.outline.crate_key.0
                    || old.location.source.content.0 != selected.outline.parse.content.0
                    || old.location.source.path != selected.path.as_str()
                {
                    return Err(invalid());
                }
                continue;
            }
            let mut path = String::new();
            path.try_reserve_exact(selected.path.as_str().len())
                .map_err(|_| invalid())?;
            path.push_str(selected.path.as_str());
            found = Some(DefinitionOrigin {
                identity: *definition,
                name,
                location: SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(selected.outline.crate_key.0),
                        content: Key(selected.outline.parse.content.0),
                        path,
                    },
                    range,
                },
            });
        }
        Ok(found)
    }
    pub(crate) fn diagnostic_source_contexts(
        &self,
    ) -> impl Iterator<
        Item = (
            &crate::core::digest::Digest,
            &crate::core::digest::Digest,
            u16,
        ),
    > {
        self.sources.iter().map(|source| {
            let selection = source.selection();
            (
                &selection.outline.crate_key,
                &selection.outline.parse.content,
                selection.outline.parse.profile.edition,
            )
        })
    }
    pub fn installed_recipes(&self) -> &InstalledRecipes {
        &self.installed
    }
    pub fn opcode_counters(&self) -> Result<OpcodeCounters, Diagnostic> {
        self.all_counters()
    }
    pub fn peak_guest_frames(&self) -> u32 {
        self.peak_guest_frames
    }
    pub fn action_snapshot(&self) -> crate::vm::action::ActionSnapshot {
        self.frame.actions.snapshot()
    }
    /// Unlimited run-wide step observation (E-R5-006: name kept for callers).
    pub fn fuel_snapshot(&self) -> crate::vm::steps::StepSnapshot {
        self.frame.fuel_snapshot()
    }
    pub fn poll(&mut self, db: &mut Database) -> RunPoll {
        self.poll_with_runtime_macros(db, &[])
    }
    /// Resolve compiler runtime macros at reached PCs using explicit source facts.
    pub fn poll_with_runtime_macros(
        &mut self,
        db: &mut Database,
        contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
    ) -> RunPoll {
        let was_terminal = self.terminal;
        let mut result = self.poll_runtime_macros_inner(db, contexts);
        if !was_terminal && self.terminal && self.terminal_facts.is_none() {
            let outcome = match &result {
                RunPoll::Completed(report) => Some(TerminalExecution::Completed {
                    unit: matches!(report.value, ValueSummary::Unit),
                }),
                RunPoll::Panicked(report) => Some(TerminalExecution::GuestPanic {
                    message: report.panic.description().to_owned(),
                }),
                RunPoll::Fault(error) => Some(TerminalExecution::Fault { code: error.code }),
                RunPoll::NeedInput(_) => None,
            };
            if let Some(outcome) = outcome {
                let cleanup_ran = self
                    .all_counters()
                    .ok()
                    .map(|c| c.return_scopes_cleaned != 0 || c.panic_scopes_cleaned != 0);
                self.terminal_facts = Some(TerminalFacts {
                    outcome,
                    cleanup_ran,
                });
            }
        }
        if let RunPoll::Fault(error) = &mut result
            && error.generated_origin.is_none()
            && db.source_revision() == self.installed.source_revision()
        {
            match super::failure::generated_origin(self, db) {
                Ok(Some(origin)) => error.generated_origin = Some(Box::new(origin)),
                Ok(None) => {}
                Err(attachment) => error.origin_attachment_failure = Some(attachment.code),
            }
        }
        if let RunPoll::Fault(error) = &mut result
            && error.guest_stack.is_none()
            && db.source_revision() == self.installed.source_revision()
        {
            match super::failure::guest_stack(self, db) {
                Ok(stack) => error.guest_stack = Some(Box::new(stack)),
                Err(attachment) => error.origin_attachment_failure = Some(attachment.code),
            }
        }
        if let RunPoll::Fault(error) = &mut result
            && error.is_internal()
            && error.failure_context.is_none()
        {
            error.failure_context = Some(Box::new(match super::failure::ExecutionRecord::capture(
                self,
            ) {
                Ok(record) => super::failure::FailureContext::Available(record),
                Err(_) => super::failure::FailureContext::ResourceUnavailable,
            }));
        }
        if let Some(capture) = &mut self.runtime_capture
            && let Err(error) = capture.observe_poll(&result, self.terminal_facts.as_ref())
        {
            return RunPoll::Fault(error);
        }
        result
    }
    fn poll_runtime_macros_inner(
        &mut self,
        db: &mut Database,
        contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
    ) -> RunPoll {
        if let Err(error) = self.ensure_drop_inventory(db) {
            self.terminal = true;
            return RunPoll::Fault(error);
        }
        if self.body_lease.body() != self.frame.body.body() {
            self.terminal = true;
            return RunPoll::Fault(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "active frame and body source lease differ".into(),
            ));
        }
        if db.source_revision() != self.installed.source_revision() {
            if self.frame.has_source_constant_activation() {
                return self.fail_source_constant(Diagnostic::new(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "runtime macro dispatch source publication is stale".into(),
                ));
            }
            self.terminal = true;
            return RunPoll::Fault(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "runtime macro dispatch source publication is stale".into(),
            ));
        }
        let current = (|| {
            for lease in std::iter::once(&self.body_lease)
                .chain(self.calls.iter().map(|call| &call.body_lease))
                .chain(
                    self.prepared_call
                        .iter()
                        .filter_map(|call| call.body_lease.as_ref()),
                )
            {
                match lease.require_current(db) {
                    QueryPoll::Ready(Ok(())) => {}
                    other => return other,
                }
            }
            QueryPoll::Ready(Ok(()))
        })();
        match current {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
            QueryPoll::Ready(Err(error)) => {
                if self.frame.has_source_constant_activation() {
                    return self.fail_source_constant(error);
                }
                self.terminal = true;
                return RunPoll::Fault(error);
            }
        }
        if contexts.iter().enumerate().any(|(i, context)| {
            let source = context.source();
            !self
                .sources
                .iter()
                .any(|s| s.same_publication(source) && s.selection() == source.selection())
                || contexts[..i]
                    .iter()
                    .any(|prior| prior.source().selection() == source.selection())
        }) {
            return RunPoll::Fault(call_fault(
                "runtime macro contexts are foreign or duplicate",
            ));
        }

        if self.terminal {
            return RunPoll::Fault(Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "terminal run cannot execute again".into(),
            ));
        }
        if let Some(RunNames::Legacy(graph)) = &mut self.graph {
            for context in contexts {
                if let Err(error) = context.contribute_names(graph) {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            }
            if !contexts.is_empty()
                && let Err(error) = graph.resolve_imports()
            {
                self.terminal = true;
                return RunPoll::Fault(error);
            }
        }
        'dispatch: loop {
            // A reached cleanup owner demands its value's `Drop::drop` before
            // any further cleanup or execution of this frame.
            if self.frame.drop_witness_demand().is_some() {
                match self.enter_drop_witness(db) {
                    QueryPoll::Ready(Ok(())) => continue,
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                }
            }
            // A reached operator on non-primitive operands demands its trait
            // method before this frame continues.
            if self.frame.operator_demanded() {
                match self.enter_operator_call(db) {
                    QueryPoll::Ready(Ok(())) => continue,
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                }
            }
            // A `format_args!` placeholder outside the native family demands
            // its trait's `fmt` before this frame continues.
            if self.frame.format_demanded() {
                match self.enter_format_call(db) {
                    QueryPoll::Ready(Ok(())) => continue,
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                }
            }
            if self.unwinding.is_some() {
                match self.advance_panic_cleanup() {
                    Ok(Some(report)) => {
                        self.terminal = true;
                        return RunPoll::Panicked(report);
                    }
                    Ok(None) => continue,
                    Err(error) => {
                        self.frame.status = crate::vm::frame::FrameStatus::Faulted;
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
            }
            if self.returning.is_some() {
                match self.advance_return_cleanup() {
                    Ok(Some(report)) => {
                        self.terminal = true;
                        return RunPoll::Completed(report);
                    }
                    Ok(None) => continue,
                    Err(error) => {
                        self.frame.status = crate::vm::frame::FrameStatus::Faulted;
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
            }
            // Rollback is bounded physical work and gets priority over new
            // execution; it can never publish a source value.
            if self.frame.source_constant_cleaning() {
                match self.frame.drain_source_constant_op(&mut self.store) {
                    Ok(false) => continue,
                    Ok(true) => {
                        self.terminal = true;
                        return RunPoll::Fault(call_fault(
                            "constant rollback completed without its retained failure",
                        ));
                    }
                    Err(error) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
            }
            let pc = self.frame.pc().clone();
            let prepared_key = if pc.phase == 0 {
                self.frame
                    .body
                    .nodes()
                    .get(pc.node as usize)
                    .filter(|node| {
                        node.role
                            == crate::syntax::body_index::NodeRole::Expression(
                                crate::syntax::body_index::ExpressionKind::Macro,
                            )
                    })
                    .map(|node| (pc.body.fingerprint(), node.range.start, node.range.end))
            } else {
                None
            };
            let reached_macro = self
                .frame
                .body
                .nodes()
                .get(pc.node as usize)
                .is_some_and(|node| {
                    node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Macro,
                        )
                });
            let mut prepared_declarative = None;
            let reached_recipe = if reached_macro && !self.installed.contains_pc(&pc) {
                let mut recipe =
                    match self
                        .declarative_runtime
                        .prepare(db, &self.sources, &self.frame.body, &pc)
                    {
                        QueryPoll::Ready(Ok(plan)) => {
                            let recipe = plan.as_ref().map(|plan| Box::new(plan.recipe().clone()));
                            prepared_declarative = plan;
                            recipe
                        }
                        QueryPoll::Ready(Err(error)) => {
                            self.terminal = true;
                            return RunPoll::Fault(error);
                        }
                        QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
                    };
                for context in contexts {
                    if recipe.is_some() {
                        break;
                    }
                    match context.resolve_at(
                        &self.frame.body,
                        &pc,
                        self.runtime_no_item_proofs.values(),
                        Some((&self.frame, &self.store)),
                    ) {
                        Ok(Some(found)) => {
                            if let Some(method) = found.primitive_method
                                && let Err(error) = self.retain_primitive_method(
                                    &pc,
                                    method,
                                    found.recipe.recipe().fingerprint(),
                                )
                            {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            if let Some(call) = found.call
                                && let Err(error) = self.retain_runtime_call(&pc, call)
                            {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            if let Some(proof) = found.no_item
                                && let Err(error) = self.retain_no_item_proof(proof)
                            {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            recipe = Some(found.recipe);
                            break;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.terminal = true;
                            return RunPoll::Fault(error);
                        }
                    }
                }
                recipe
            } else {
                None
            };
            // A constant path is an ordinary `ValuePath`: the dispatcher
            // selects its item and the one pure constant route evaluates it
            // (`prepare_item_value_path`), wherever the path was written.
            let prepared_constant: Option<crate::ir::recipe::VerifiedRecipe> = None;
            let prepared_floating =
                if prepared_constant.is_none() && !self.installed.contains_pc(&pc) {
                    match self.prepare_floating_recipe(db, contexts) {
                        QueryPoll::Ready(Ok(recipe)) => recipe,
                        QueryPoll::Ready(Err(error)) => {
                            self.terminal = true;
                            return RunPoll::Fault(error);
                        }
                        QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                    }
                } else {
                    None
                };
            let resolution = if let Some(recipe) = prepared_constant {
                self.installed.prepare_install(db, pc, &recipe)
            } else if let Some(recipe) = prepared_floating {
                self.installed.prepare_install(db, pc, &recipe)
            } else if let Some(recipe) = reached_recipe {
                self.installed.prepare_install(db, pc, &recipe)
            } else if let Some((source, plan)) = prepared_key
                .as_ref()
                .and_then(|key| self.static_panic_plans.get(key))
            {
                use crate::frontend::runtime_macro_plan::StaticPanicRecipeLowering;
                match self.frame.body.pc(NodeId(0), Phase::Exit).and_then(|next| {
                    plan.lower_static_panic_recipe(source, &self.frame.body, &pc, next)
                }) {
                    Ok(StaticPanicRecipeLowering::Recipe(recipe)) => {
                        self.installed.prepare_install(db, pc, &recipe)
                    }
                    Ok(StaticPanicRecipeLowering::NeedsFormatting) => {
                        QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "reached panic formatting requires pending formatter integration"
                                .into(),
                        )))
                    }
                    Err(error) => QueryPoll::Ready(Err(error)),
                }
            } else {
                self.installed.resolve(db, pc)
            };
            let id = match resolution {
                QueryPoll::Ready(Ok(id)) => id,
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            };
            if let Some(plan) = prepared_declarative
                && let Err(error) = self.declarative_runtime.retain(plan)
            {
                self.terminal = true;
                return RunPoll::Fault(error);
            }
            let constructor = match self.installed.get_current(db, id) {
                Ok(recipe) => recipe
                    .recipe()
                    .operations
                    .iter()
                    .skip(self.frame.resume_operation_index())
                    .any(|op| matches!(op, Op::ConstructStart { .. })),
                Err(error) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            };
            if constructor {
                match self.prepare_installed_constructor(db, id, contexts) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
                }
            }
            // A record whose explicit Type argument holds a `_` settles through
            // the pinned inference owner once its first field operand has run.
            match self.settle_open_constructor(db, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            }
            let image_finish = match self.installed.get_current(db, id) {
                Ok(recipe) => recipe
                    .recipe()
                    .operations
                    .iter()
                    .skip(self.frame.resume_operation_index())
                    .find_map(|op| match op {
                        Op::ConstructFinish { guard, .. } => Some(*guard),
                        _ => None,
                    }),
                Err(error) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            };
            if let Some(guard) = image_finish {
                match self.prepare_constructor_image_layout(db, guard) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                }
            }
            // A place operation whose base needs an overloaded `Deref` step
            // first demands it before the operation is prepared.
            match self.prepare_place_deref(db, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            }
            let field = match self.installed.get_current(db, id) {
                Ok(_)
                    if self.frame.has_place_deref_demand() || self.frame.place_deref_returned() =>
                {
                    None::<(SlotId, SlotId)>
                }
                Ok(recipe) => recipe
                    .recipe()
                    .operations
                    .iter()
                    .skip(self.frame.resume_operation_index())
                    .find_map(|op| match op {
                        Op::ProjectField { base, destination } => Some((*base, *destination)),
                        _ => None,
                    }),
                Err(error) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            };
            if let Some((base, destination)) = field {
                match self.prepare_field(db, base, destination) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
                }
            }
            match if self.frame.has_place_deref_demand() || self.frame.place_deref_returned() {
                QueryPoll::Ready(Ok(()))
            } else {
                self.prepare_nominal_operations(db, id)
            } {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            }
            // An `as` cast of an enum value whose discriminant is an
            // unevaluated expression demands that anonymous constant first
            // (E-R5-013).
            match self.prepare_enum_discriminants(db, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            }
            // A `ValuePath` naming a constant or static whose value is not
            // yet evaluated enters its initializer first.
            if self.initializer_demand.is_some() {
                match self.enter_initializer(db) {
                    QueryPoll::Ready(Ok(())) => continue,
                    QueryPoll::Ready(Err(error)) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                    QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                }
            }
            match self.prepare_pattern_names(db, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(request) => return RunPoll::NeedInput(request),
            }
            match self.prepare_closure_pattern_access(db, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
            }
            if let Err(error) = self.register_closures(db, id) {
                self.terminal = true;
                return RunPoll::Fault(error);
            }
            self.frame.runtime_call = self.runtime_calls.get(self.frame.pc()).cloned();
            let call_inferred = match self.installed.get_current(db, id) {
                Ok(recipe) => recipe.inferred_arguments().cloned(),
                Err(error) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            };
            self.inferred_call = None;
            if let Some(inferred) = call_inferred {
                match self.inferred_explicit(&inferred) {
                    Ok(arguments) => {
                        self.inferred_call =
                            Some((self.frame.pc().clone(), inferred.definition, arguments));
                    }
                    Err(error) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
            }
            let (call_ops, call_result, call_result_hint, call_next) =
                match self.installed.get_current(db, id) {
                    Ok(recipe) => {
                        let result = match &recipe.recipe().exit {
                            crate::ir::recipe::RecipeExit::Call { result, .. } => Some(*result),
                            _ => None,
                        };
                        let hint = recipe
                            .recipe()
                            .operations
                            .iter()
                            .skip(self.frame.resume_operation_index())
                            .find_map(|op| match op {
                                Op::SetHint {
                                    child,
                                    hint:
                                        hint @ (crate::ir::op::HintRecipe::Primitive(_)
                                        | crate::ir::op::HintRecipe::Written(_)),
                                } if Some(*child) == result => Some(hint.clone()),
                                _ => None,
                            });
                        (
                            recipe
                                .recipe()
                                .operations
                                .iter()
                                .skip(self.frame.resume_operation_index())
                                .filter(|op| {
                                    matches!(
                                        op,
                                        Op::CallPrepare { .. }
                                            | Op::CallSeal { .. }
                                            | Op::CallAdvance { .. }
                                            | Op::MethodReceiver { .. }
                                    )
                                })
                                .cloned()
                                .collect::<Vec<_>>(),
                            result,
                            hint,
                            match &recipe.recipe().exit {
                                crate::ir::recipe::RecipeExit::Next(next) => Some(next.clone()),
                                _ => None,
                            },
                        )
                    }
                    Err(error) => {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                };
            let method_places: BTreeMap<u32, SlotId> = call_ops
                .iter()
                .filter_map(|op| match op {
                    Op::MethodReceiver { site, place, .. } => Some((site.0, *place)),
                    _ => None,
                })
                .collect();
            for op in call_ops {
                if matches!(op, Op::MethodReceiver { .. }) {
                    continue;
                }
                if let Err(error) = self.require_runtime_call_operation(&op) {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                match op {
                    Op::CallPrepare {
                        site,
                        name,
                        explicit,
                        arguments,
                    } => {
                        // A `Root::member` callee whose root an import binds to a
                        // module is the module route's written path.
                        let name = match self.lexical_module_name(db, site.0, name) {
                            QueryPoll::Ready(Ok(name)) => name,
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        };
                        if name == value_calls::VALUE_CALL_NAME {
                            match self.select_value_call(db, site.0, &arguments) {
                                QueryPoll::Ready(Ok(false)) => {}
                                QueryPoll::Ready(Ok(true)) => {
                                    if let Err(error) = self.start_closure_call(site.0, &arguments)
                                    {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    continue;
                                }
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                        match self.select_facade_call(db, site.0, &name) {
                            QueryPoll::Ready(Ok(true)) => {
                                if let Err(error) = self.start_facade_call(site.0, &arguments) {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                continue;
                            }
                            QueryPoll::Ready(Ok(false)) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        match self.select_constructor_call(db, site.0, &name) {
                            QueryPoll::Ready(Ok(true)) => {
                                if let Err(error) = self.start_constructor_call(site.0, &arguments)
                                {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                continue;
                            }
                            QueryPoll::Ready(Ok(false)) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        if name.starts_with('.')
                            && !self.pending.contains_key(&self.frame.call_identity(site.0))
                        {
                            let Some(place) = method_places.get(&site.0).copied() else {
                                self.terminal = true;
                                return RunPoll::Fault(call_fault(
                                    "method call has no published receiver place",
                                ));
                            };
                            match self
                                .select_method_call(db, site.0, &name, &explicit, &arguments, place)
                            {
                                QueryPoll::Ready(Ok(methods::MethodCall::Selected(call))) => {
                                    self.pending_bytes =
                                        self.pending_bytes.saturating_add(call.bytes);
                                    self.pending.insert(self.frame.call_identity(site.0), call);
                                }
                                QueryPoll::Ready(Ok(methods::MethodCall::Deferred {
                                    declaration,
                                    call,
                                })) => {
                                    self.start_deferred_trait_call(
                                        site.0,
                                        declaration,
                                        call,
                                        &arguments,
                                    );
                                    continue;
                                }
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                        // A trait-path call selects its impl from its self
                        // type; one whose self argument decides it starts
                        // against the trait's declaration.
                        if !self.pending.contains_key(&self.frame.call_identity(site.0))
                            && explicit.is_empty()
                        {
                            match self.trait_path_call(db, site.0, &name, &arguments) {
                                QueryPoll::Ready(Ok(Some(
                                    trait_calls::TraitPathCall::Deferred { declaration, call },
                                ))) => {
                                    self.start_deferred_trait_call(
                                        site.0,
                                        declaration,
                                        call,
                                        &arguments,
                                    );
                                    continue;
                                }
                                QueryPoll::Ready(Ok(Some(
                                    trait_calls::TraitPathCall::Selected(callee),
                                ))) => match self.prepare_selected_argument_call(
                                    db, callee, &explicit, &arguments,
                                ) {
                                    QueryPoll::Ready(Ok(call)) => {
                                        self.pending_bytes =
                                            self.pending_bytes.saturating_add(call.bytes);
                                        self.pending.insert(self.frame.call_identity(site.0), call);
                                    }
                                    QueryPoll::Ready(Err(error)) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    QueryPoll::NeedInput(input) => {
                                        return RunPoll::NeedInput(input);
                                    }
                                },
                                QueryPoll::Ready(Ok(None)) => {}
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                        if !self.pending.contains_key(&self.frame.call_identity(site.0)) {
                            match self.select_reached_argument_call(
                                db, contexts, site, &name, &explicit, &arguments,
                            ) {
                                QueryPoll::Ready(Ok(call)) => {
                                    self.pending_bytes =
                                        match self.pending_bytes.checked_add(call.bytes) {
                                            Some(n) => n,
                                            None => {
                                                self.terminal = true;
                                                return RunPoll::Fault(call_fault(
                                                    "pending call size overflow",
                                                ));
                                            }
                                        };
                                    self.pending.insert(self.frame.call_identity(site.0), call);
                                }
                                QueryPoll::Ready(Err(e)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(e);
                                }
                                QueryPoll::NeedInput(i) => return RunPoll::NeedInput(i),
                            }
                        }
                        let selected = self
                            .pending
                            .get(&self.frame.call_identity(site.0))
                            .expect("inserted checked selected context");
                        if let Some(typed) = &selected.typed {
                            if let Err(error) =
                                typed.require_current(db, &self.frame.body, self.frame.pc())
                            {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        }
                        let ordinary = match selected.callee.ordinary_call_witness(
                            db,
                            self.frame.body.body().clone(),
                            NodeId(site.0),
                        ) {
                            Ok(witness) => witness,
                            Err(error) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        };
                        let call_hints = (|| -> Result<(Vec<_>, Vec<_>), Diagnostic> {
                            let mut result = Vec::new();
                            let mut partial = Vec::new();
                            result
                                .try_reserve_exact(arguments.len())
                                .map_err(|_| crate::runtime::arena::resource())?;
                            let definition = selected.callee.signature().definition;
                            // Only explicit type arguments are fixed before
                            // the first argument evaluates.
                            let bound = argument_hints::bound_call_parameters(
                                &self.frame,
                                &self.store,
                                definition,
                                &selected.explicit,
                                &[],
                                &[],
                                &selected.expected,
                            )?;
                            for (ordinal, slot) in arguments.iter().enumerate() {
                                let (mut descriptor, source) = match selected.hints.get(ordinal) {
                                    Some(hint) => argument_hints::runtime_hint(
                                        db,
                                        &self.frame,
                                        &mut self.store,
                                        &selected.callee,
                                        hint,
                                        ordinal,
                                        *slot,
                                    )?,
                                    None => (None, None),
                                };
                                if descriptor.is_none()
                                    && let Some(hint) = selected.hints.get(ordinal)
                                {
                                    match argument_hints::written_hint(
                                        &mut self.store,
                                        hint,
                                        definition,
                                        &bound,
                                    )? {
                                        argument_hints::WrittenArgumentHint::Exact(ty) => {
                                            descriptor = Some(ty)
                                        }
                                        argument_hints::WrittenArgumentHint::Partial(
                                            written,
                                            fixed,
                                        ) => partial.push((*slot, written, fixed)),
                                        argument_hints::WrittenArgumentHint::None => {}
                                    }
                                }
                                result.push((*slot, descriptor, source));
                            }
                            Ok((result, partial))
                        })();
                        let (call_hints, partial) = match call_hints {
                            Ok(hints) => hints,
                            Err(error) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        };
                        // A reference parameter beside an open pointee (`&mut H`)
                        // has no descriptor hint, but its argument is still a
                        // reference coercion site (implicit reborrow).
                        for (ordinal, slot) in arguments.iter().enumerate() {
                            let expectation =
                                selected
                                    .hints
                                    .get(ordinal)
                                    .and_then(|hint| match &hint.source {
                                        crate::traits::term::TyTerm::Ref(_, mutable, _) => {
                                            Some(*mutable)
                                        }
                                        _ => None,
                                    });
                            self.frame.expect_reference(*slot, expectation);
                        }
                        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
                            ordinary,
                            site: site.0,
                            hints: call_hints,
                            partial,
                        });
                    }
                    // A deferred trait-path call selects its impl once its
                    // self argument completes; earlier arguments take no hint.
                    Op::CallAdvance { site, argument }
                        if self
                            .frame
                            .trait_calls
                            .contains_key(&self.frame.call_identity(site.0)) =>
                    {
                        let self_ordinal =
                            self.frame.trait_calls[&self.frame.call_identity(site.0)].self_ordinal;
                        if argument < self_ordinal {
                            continue;
                        }
                        match self.select_deferred_trait_call(db, site.0, argument as usize + 1) {
                            QueryPoll::Ready(Ok(())) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        // Impls differing by the trait's arguments wait for
                        // the last argument: no selected call to hint from.
                        if !self.pending.contains_key(&self.frame.call_identity(site.0)) {
                            continue;
                        }
                        if let Some(next) = call_next.as_ref() {
                            match self.prepare_later_argument_hint(db, site.0, argument, next) {
                                QueryPoll::Ready(Ok(())) => {}
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                    }
                    Op::CallAdvance { site, argument }
                        if call_next.is_some()
                            && !self
                                .frame
                                .facade_calls
                                .contains_key(&self.frame.call_identity(site.0))
                            && !self.closures.selected(&self.frame.call_identity(site.0))
                            && self.frame.selected_constructor_call(site.0).is_none() =>
                    {
                        match self.prepare_later_argument_hint(
                            db,
                            site.0,
                            argument,
                            call_next.as_ref().expect("checked Next"),
                        ) {
                            QueryPoll::Ready(Ok(())) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                    }
                    Op::CallSeal {
                        site,
                        name,
                        explicit,
                        arguments,
                    } if self.prepared_call.is_none() => {
                        // A `Root::member` callee whose root an import binds to a
                        // module is the module route's written path.
                        let name = match self.lexical_module_name(db, site.0, name) {
                            QueryPoll::Ready(Ok(name)) => name,
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        };
                        if name == value_calls::VALUE_CALL_NAME {
                            match self.select_value_call(db, site.0, &arguments) {
                                QueryPoll::Ready(Ok(false)) => {}
                                QueryPoll::Ready(Ok(true)) => {
                                    match self.seal_closure_call(db, site.0, &arguments) {
                                        QueryPoll::Ready(Ok(())) => {}
                                        QueryPoll::Ready(Err(error)) => {
                                            self.terminal = true;
                                            return RunPoll::Fault(error);
                                        }
                                        QueryPoll::NeedInput(input) => {
                                            return RunPoll::NeedInput(input);
                                        }
                                    }
                                    continue;
                                }
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                        match self.select_facade_call(db, site.0, &name) {
                            QueryPoll::Ready(Ok(true)) => {
                                // The declarations a layout intrinsic's type
                                // needs (E-R5-009), before its arguments seal.
                                match self.prepare_facade_layout(db, site.0, &arguments) {
                                    QueryPoll::Ready(Ok(())) => {}
                                    QueryPoll::Ready(Err(error)) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    QueryPoll::NeedInput(input) => {
                                        return RunPoll::NeedInput(input);
                                    }
                                }
                                match self.prepare_facade_discriminants(db, site.0, &arguments) {
                                    QueryPoll::Ready(Ok(())) => {}
                                    QueryPoll::Ready(Err(error)) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    QueryPoll::NeedInput(input) => {
                                        return RunPoll::NeedInput(input);
                                    }
                                }
                                if self.initializer_demand.is_some() {
                                    continue 'dispatch;
                                }
                                if let Err(error) = self.seal_facade_call(site.0, &arguments) {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                continue;
                            }
                            QueryPoll::Ready(Ok(false)) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        match self.select_constructor_call(db, site.0, &name) {
                            QueryPoll::Ready(Ok(true)) => {
                                if let Err(error) = self.seal_constructor_call(site.0, &arguments) {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                continue;
                            }
                            QueryPoll::Ready(Ok(false)) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        // A deferred trait-path call selects its impl from its
                        // completed self argument; a zero-argument trait path
                        // (`Default::default()`) from its expected result.
                        if !self.pending.contains_key(&self.frame.call_identity(site.0)) {
                            if self
                                .frame
                                .trait_calls
                                .contains_key(&self.frame.call_identity(site.0))
                            {
                                match self.select_deferred_trait_call(db, site.0, arguments.len()) {
                                    QueryPoll::Ready(Ok(())) => {}
                                    QueryPoll::Ready(Err(error)) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    QueryPoll::NeedInput(input) => {
                                        return RunPoll::NeedInput(input);
                                    }
                                }
                            } else if explicit.is_empty() {
                                match self.trait_path_call(db, site.0, &name, &arguments) {
                                    QueryPoll::Ready(Ok(Some(
                                        trait_calls::TraitPathCall::Selected(callee),
                                    ))) => match self.prepare_selected_argument_call(
                                        db, callee, &explicit, &arguments,
                                    ) {
                                        QueryPoll::Ready(Ok(call)) => {
                                            self.pending_bytes =
                                                self.pending_bytes.saturating_add(call.bytes);
                                            self.pending
                                                .insert(self.frame.call_identity(site.0), call);
                                        }
                                        QueryPoll::Ready(Err(error)) => {
                                            self.terminal = true;
                                            return RunPoll::Fault(error);
                                        }
                                        QueryPoll::NeedInput(input) => {
                                            return RunPoll::NeedInput(input);
                                        }
                                    },
                                    QueryPoll::Ready(Ok(Some(
                                        trait_calls::TraitPathCall::Deferred { .. },
                                    ))) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(call_fault(
                                            "trait call self argument was not reached before its seal",
                                        ));
                                    }
                                    QueryPoll::Ready(Ok(None)) => {}
                                    QueryPoll::Ready(Err(error)) => {
                                        self.terminal = true;
                                        return RunPoll::Fault(error);
                                    }
                                    QueryPoll::NeedInput(input) => {
                                        return RunPoll::NeedInput(input);
                                    }
                                }
                            }
                        }
                        // Zero-argument calls have no CallPrepare opcode. They
                        // must still select lexical identity at this reached
                        // entry, never fall back to a same-named module item.
                        if !self.pending.contains_key(&self.frame.call_identity(site.0)) {
                            match self.select_reached_argument_call(
                                db, contexts, site, &name, &explicit, &arguments,
                            ) {
                                QueryPoll::Ready(Ok(call)) => {
                                    self.pending_bytes =
                                        match self.pending_bytes.checked_add(call.bytes) {
                                            Some(bytes) => bytes,
                                            None => {
                                                self.terminal = true;
                                                return RunPoll::Fault(call_fault(
                                                    "pending call size overflow",
                                                ));
                                            }
                                        };
                                    self.pending.insert(self.frame.call_identity(site.0), call);
                                }
                                QueryPoll::Ready(Err(error)) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                            }
                        }
                        if let Some(selected) = self.pending.get(&self.frame.call_identity(site.0))
                            && selected.callee.signature().parameters.len() != arguments.len()
                        {
                            self.terminal = true;
                            return RunPoll::Fault(Diagnostic::new(
                                "LRR-E-CALL-ARITY",
                                "evaluated written argument count differs from selected signature"
                                    .into(),
                            ));
                        }
                        if let Some(typed) = self
                            .pending
                            .get(&self.frame.call_identity(site.0))
                            .and_then(|selected| selected.typed.as_ref())
                        {
                            if let Err(error) =
                                typed.require_current(db, &self.frame.body, self.frame.pc())
                            {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        }
                        // A selected packet may belong to a surrounding macro,
                        // whose expression value is not the predicate call result.
                        // Retain its source hint only for that exact result slot;
                        // otherwise resolve the verified call slot/recipe context.
                        let result_hint = if let Some(selected) = self
                            .pending
                            .get(&self.frame.call_identity(site.0))
                            .filter(|selected| {
                                selected.result_hint.is_some()
                                    && self
                                        .frame
                                        .runtime_call
                                        .as_ref()
                                        .map(|call| call.result())
                                        .or(self.frame.body.nodes()[self.frame.pc().node as usize]
                                            .value)
                                        == call_result
                            }) {
                            selected.result_hint.clone()
                        } else {
                            let Some(result_slot) = call_result else {
                                self.terminal = true;
                                return RunPoll::Fault(call_fault(
                                    "sealed call result slot absent",
                                ));
                            };
                            match self.call_result_source_hint(result_slot) {
                                Ok(Some(hint)) => Some(hint),
                                Ok(None) => {
                                    // The Type the call recipe itself states for
                                    // its result ahead of the call (a primitive,
                                    // or a written Type decided by the common
                                    // source constraints from the value's uses).
                                    let hinted = match &call_result_hint {
                                        Some(crate::ir::op::HintRecipe::Primitive(primitive)) => {
                                            crate::frontend::call_contracts::primitive_runtime_id(
                                                *primitive,
                                            )
                                            .and_then(primitive_argument_type)
                                            .map(Some)
                                        }
                                        Some(crate::ir::op::HintRecipe::Written(written)) => {
                                            match (
                                                written.source(),
                                                written.materialize(&mut self.store),
                                            ) {
                                                (Some(source), Ok(Some(descriptor))) => {
                                                    Ok(Some(ArgumentType { source, descriptor }))
                                                }
                                                (_, Err(error)) => Err(error),
                                                _ => Ok(None),
                                            }
                                        }
                                        _ => Ok(None),
                                    };
                                    match hinted {
                                        Ok(hint) => hint,
                                        Err(error) => {
                                            self.terminal = true;
                                            return RunPoll::Fault(error);
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(e);
                                }
                            }
                        };
                        let result_hint = result_hint.map(|mut hint| {
                            hint.source = self.frame.bound_view(&hint.source, &self.store);
                            hint
                        });
                        match self.solve_reached_call_inputs(
                            db,
                            NodeId(site.0),
                            &arguments,
                            result_hint.as_ref(),
                        ) {
                            QueryPoll::Ready(Ok(())) => {}
                            QueryPoll::Ready(Err(error)) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                        let mut types = Vec::new();
                        let mut cells = Vec::new();
                        for slot in arguments {
                            let argument = (|| {
                                let cell = self.frame.cell(slot)?;
                                // A literal argument inference left unconstrained
                                // (e.g. to a generic parameter) takes rustc's
                                // integer/float fallback.
                                self.store.bind_literal_fallback(cell)?;
                                let value = self.store.admit_move(cell)?;
                                let descriptor =
                                    self.store.effective_type(value)?.ok_or_else(|| {
                                        Diagnostic::new(
                                            "LRR-E-TYPE-CONTEXT",
                                            "call argument requires a reached concrete type".into(),
                                        )
                                    })?;
                                let view = self.frame.argument_source_view(
                                    slot,
                                    descriptor,
                                    &self.store,
                                )?;
                                // The caller's own generic parameters are
                                // its runtime instantiation at the boundary:
                                // a callee contract never sees them rigid.
                                let view =
                                    view.map(|view| self.frame.bound_view(&view, &self.store));
                                let argument = reached_argument_type(descriptor, view)?;
                                Ok((cell, argument))
                            })();
                            match argument {
                                Ok((cell, ty)) => {
                                    cells.push(cell);
                                    types.push(ty);
                                }
                                Err(e) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(e);
                                }
                            }
                        }
                        let selected = self.pending.get(&self.frame.call_identity(site.0)).cloned();
                        match self.prepare_source_call(
                            db,
                            &name,
                            selected,
                            &types,
                            result_hint.as_ref(),
                            &explicit,
                        ) {
                            QueryPoll::Ready(Ok(call)) => {
                                // A descriptor-only expectation is still a
                                // constraint. Check a known declaration result
                                // before publishing the call or moving inputs.
                                if let (Some(slot), Some(actual)) =
                                    (call_result, call.contracts.result_descriptor)
                                {
                                    match self.frame.hint(slot) {
                                        Ok(Some(expected)) if expected != actual => {
                                            self.terminal = true;
                                            return RunPoll::Fault(Diagnostic::new(
                                                "LRR-E-CALL-RETURN-TYPE",
                                                "concrete call return descriptor differs from the reached result context".into(),
                                            ));
                                        }
                                        Ok(_) => {}
                                        Err(error) => {
                                            self.terminal = true;
                                            return RunPoll::Fault(error);
                                        }
                                    }
                                }
                                if let Some(proof) = &call.contracts.closed_references
                                    && let Err(error) =
                                        proof.require_argument_cells(&self.store, &cells, &types)
                                {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                                self.frame.call_types =
                                    call.contracts.parameter_descriptors.clone();
                                self.frame.call_cells = cells;
                                self.prepared_call = Some(call);
                                self.frame.call_ready = true;
                                if name == value_calls::VALUE_CALL_NAME
                                    && let Err(error) = self.frame.retire_value_callee(site.0)
                                {
                                    self.terminal = true;
                                    return RunPoll::Fault(error);
                                }
                            }
                            QueryPoll::Ready(Err(e)) => {
                                self.terminal = true;
                                return RunPoll::Fault(e);
                            }
                            QueryPoll::NeedInput(i) => return RunPoll::NeedInput(i),
                        }
                    }
                    Op::CallSeal { .. } => {
                        if let Some(response) = self.prepared_call.as_ref().and_then(|call| call.source_response.as_ref()) {
                            if let Err(error) = response.require_current(db) {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Err(error) = self.revalidate_primitive_method(db, contexts, id) {
                self.terminal = true;
                return RunPoll::Fault(error);
            }
            match self.revalidate_floating_recipe(db, contexts, id) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
                QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
            }
            let outcome = match self.installed.get_current(db, id).and_then(|recipe| {
                if let Some(capture) = &mut self.runtime_capture {
                    capture.before_vm_dispatch(recipe)?;
                }
                let outcome = if recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::SourceConstant { .. }))
                {
                    let plan = self.constant_plans.get(self.frame.pc()).ok_or_else(|| {
                        call_fault("source constant installed without its source plan")
                    })?;
                    let construction = plan.construction(db, &self.sources)?;
                    let views = self.sources.iter().collect::<Vec<_>>();
                    let proof = plan.proof(
                        &construction,
                        &self.frame,
                        &mut self.store,
                        &self.sources,
                        &views,
                    )?;
                    step::step_with_constant(
                        &mut self.frame,
                        &mut self.store,
                        recipe,
                        Some(&proof),
                    )?
                } else {
                    step::step(&mut self.frame, &mut self.store, recipe)?
                };
                if let Some(capture) = &mut self.runtime_capture {
                    capture.after_vm_dispatch(&outcome)?;
                }
                Ok(outcome)
            }) {
                Ok(outcome) => outcome,
                Err(error) => {
                    if self.frame.has_source_constant_activation() {
                        return self.fail_source_constant(error);
                    }
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            };
            match outcome {
                StepOutcome::Dispatch => {}
                StepOutcome::PatternPrepare {
                    owner,
                    tree,
                    demand,
                } => match demand {
                    crate::vm::pattern::PatternDemand::Initializer(identity) => {
                        self.demand_initializer(identity, false);
                    }
                    crate::vm::pattern::PatternDemand::Nominal { slot, ty, source } => {
                        match self
                            .prepare_reached_pattern_nominal(db, owner, &tree, slot, ty, &source)
                        {
                            QueryPoll::Ready(Ok(())) => {}
                            QueryPoll::Ready(Err(error)) => {
                                if self.frame.has_source_constant_activation() {
                                    return self.fail_source_constant(error);
                                }
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                            QueryPoll::NeedInput(input) => return RunPoll::NeedInput(input),
                        }
                    }
                },
                StepOutcome::Call {
                    action,
                    site,
                    resume,
                    result,
                } if self.frame.facade_sealed.is_some() => {
                    if let Err(error) = self.complete_facade_call(action, site, resume, result) {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
                StepOutcome::Call {
                    action,
                    site,
                    resume,
                    result,
                } if self.frame.constructor_call_sealed() => {
                    if let Err(error) = self.complete_constructor_call(action, site, resume, result)
                    {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
                StepOutcome::Call {
                    action,
                    site,
                    resume,
                    result,
                } => {
                    let entered = (|| {
                        if site.0 != self.frame.pc().node {
                            return Err(call_fault("Call site changed after sealing"));
                        }
                        let consumed = action.consume(
                            &self.frame.actions,
                            self.frame.pc(),
                            site,
                            &resume,
                            result,
                        )?;
                        let call = self.prepared_call.take().ok_or_else(|| {
                            call_fault("Call exit has no selected source context")
                        })?;
                        call.callee.revalidate(db)?;
                        if let Some(response) = &call.source_response { response.require_current(db)?; }
                        match call.contracts.require_inference_current(db) {
                            QueryPoll::Ready(Ok(())) => {}
                            QueryPoll::Ready(Err(error)) => return Err(error),
                            QueryPoll::NeedInput(_) => {
                                return Err(call_fault(
                                    "inferred call dependencies require source preparation",
                                ));
                            }
                        }
                        let entry = match &call.closure {
                            Some(closure) => closure.entry.clone(),
                            None => call.body.pc(NodeId(0), Phase::Entry)?,
                        };
                        let callee = Frame::new_in_run(
                            call.body.clone(),
                            entry,
                            &mut self.store,
                            &mut self.frame,
                        )?;
                        let args = self
                            .frame
                            .call_arguments
                            .take()
                            .ok_or_else(|| call_fault("Call exit has no argument packet"))?;
                        self.retire_call_locations()?;
                        if let Some(selected) =
                            self.pending.remove(&self.frame.call_identity(site.0))
                        {
                            self.pending_bytes =
                                self.pending_bytes.checked_sub(selected.bytes).ok_or_else(
                                    || call_fault("pending context retirement underflow"),
                                )?;
                        }
                        self.enter_callee(
                            call,
                            callee,
                            &args,
                            CallContinuation::Site {
                                action: consumed.id,
                                origin: consumed.origin,
                                resume: consumed.resume,
                                result: consumed.result,
                            },
                        )
                    })();
                    if let Err(error) = entered {
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
                StepOutcome::Backedge(_) => {
                    self.frame.counters.backedge_polls =
                        match self.frame.counters.backedge_polls.checked_add(1) {
                            Some(count) => count,
                            None => {
                                self.terminal = true;
                                return RunPoll::Fault(Diagnostic::new(
                                    "LRR-E-RUNTIME-RESOURCE",
                                    "backedge poll counter exhausted".into(),
                                ));
                            }
                        };
                }
                StepOutcome::Next(_) => {}
                StepOutcome::GuestPanic(panic) => {
                    if let Err(error) = self.begin_panic_cleanup(panic) {
                        self.frame.status = crate::vm::frame::FrameStatus::Faulted;
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
                StepOutcome::Return(ticket) => {
                    if let Err(error) = self.begin_return_cleanup(db, ticket) {
                        self.frame.status = crate::vm::frame::FrameStatus::Faulted;
                        self.terminal = true;
                        return RunPoll::Fault(error);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "lexical_numeric_callees_tests.rs"]
mod lexical_numeric_callees_tests;

#[cfg(test)]
#[path = "lexical_numeric_admission_tests.rs"]
mod lexical_numeric_admission_tests;

#[cfg(test)]
#[path = "run_assertion_call_tests.rs"]
mod assertion_call_tests;

#[cfg(test)]
#[path = "run_namespace_tests.rs"]
mod namespace_tests;
