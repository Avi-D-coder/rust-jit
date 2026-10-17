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
            prepare_argument_hint, prepare_contracts, prepare_contracts_with_references,
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
#[path = "run_constructors.rs"]
mod constructors;
#[cfg(test)]
#[path = "run_discarded_numeric_tests.rs"]
mod discarded_numeric_tests;
#[path = "run_fields.rs"]
mod fields;
#[cfg(test)]
#[path = "run_floating_constant_tests.rs"]
mod floating_constant_tests;
#[path = "run_floating_constants.rs"]
mod floating_constants;
#[cfg(test)]
#[path = "run_nested_match_context_tests.rs"]
mod nested_match_context_tests;
#[cfg(test)]
#[path = "run_observed_constructors_tests.rs"]
mod observed_constructor_tests;
#[cfg(test)]
#[path = "run_panic_tests.rs"]
mod panic_tests;
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
#[path = "run_panic.rs"]
mod unwinding;
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
}
#[derive(Debug, Serialize)]
pub struct PanicReport {
    pub live_edition: u16,
    pub semantics_profile: &'static str,
    pub schema: &'static str,
    pub status: &'static str,
    pub panic: crate::vm::step::GuestPanic,
    pub opcode_counters: OpcodeCounters,
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
    context: Option<crate::vm::context::GenericEnv>,
    callee: Box<SourceCallable>,
    contracts: PreparedCallContracts,
    body: Arc<BodyIndex>,
    body_lease: Option<crate::db::memo::BodyExecutionLease>,
    retained_bytes: usize,
}
#[derive(Clone)]
struct SelectedArgumentCall {
    callee: Box<SourceCallable>,
    explicit: Vec<(u32, ArgumentType)>,
    typed: Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
    hints: Vec<ArgumentHint>,
    argument_slots: Vec<SlotId>,
    result_hint: Option<ArgumentType>,
    bytes: usize,
}
struct SuspendedCall {
    action: crate::vm::action::ActionId,
    origin: crate::ir::recipe::Origin,
    caller: Frame,
    body_lease: crate::db::memo::BodyExecutionLease,
    resume: PcKey,
    result: SlotId,
    call: PreparedSourceCall,
    pending: BTreeMap<crate::vm::call::CallIdentity, SelectedArgumentCall>,
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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CallLimits {
    pub max_frames: u32,
    pub max_metadata_bytes: usize,
}
impl Default for CallLimits {
    fn default() -> Self {
        Self {
            max_frames: 4096,
            max_metadata_bytes: 32 * 1024 * 1024,
        }
    }
}
impl CallLimits {
    pub fn validate(self) -> Result<Self, Diagnostic> {
        if self.max_frames == 0
            || self.max_frames > 100_000
            || self.max_metadata_bytes == 0
            || self.max_metadata_bytes > 256 * 1024 * 1024
        {
            return Err(Diagnostic::new(
                "LRR-E-RUNTIME-RESOURCE",
                "call limits outside supported bounds".into(),
            ));
        }
        Ok(self)
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
    static_panic_plan_bytes: usize,
    runtime_no_item_proofs:
        BTreeMap<(BodyKey, u32, u32), crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    runtime_no_item_bytes: usize,
    runtime_calls:
        BTreeMap<PcKey, std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>,
    runtime_call_bytes: usize,
    floating_plans: BTreeMap<PcKey, crate::frontend::primitive_constants::FloatingConstantRecipe>,
    floating_plan_bytes: usize,
    primitive_methods: BTreeMap<
        PcKey,
        (
            crate::frontend::runtime_macro_plan::PrimitiveFloatAdmission,
            crate::core::digest::Digest,
        ),
    >,
    primitive_method_bytes: usize,
    constant_plans: BTreeMap<PcKey, source_constants::SourceConstantPlan>,
    constant_plan_bytes: usize,
    declarative_runtime: super::declarative_runtime::DeclarativeRuntimePlans,
    prepared_call: Option<PreparedSourceCall>,
    returning: Option<returning::ReturnContinuation>,
    unwinding: Option<unwinding::PanicContinuation>,
    pending: BTreeMap<crate::vm::call::CallIdentity, SelectedArgumentCall>,
    pending_bytes: usize,
    calls: Vec<SuspendedCall>,
    collected: OpcodeCounters,
    call_bytes: usize,
    call_limits: CallLimits,
    peak_guest_frames: u32,
}
impl RunMachine {
    pub fn declarative_expressions(
        &self,
    ) -> impl Iterator<Item = &crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression>
    {
        self.declarative_runtime.plans()
    }

    pub fn start(db: &mut Database, body: BodyKey, fuel_limit: u64) -> QueryPoll<Self> {
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
            let mut store = GuestStore::new(1, 2_000_000, 256 * 1024 * 1024)?;
            let mut frame = Frame::new(
                Arc::clone(index),
                index.pc(NodeId(0), Phase::Entry)?,
                &mut store,
            )?;
            frame.set_fuel_limit(fuel_limit)?;
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
                query_trace_start,
                terminal_facts: None,
                runtime_capture: None,
                sources: vec![],
                graph: None,
                static_panic_body_bases: BTreeMap::new(),
                static_panic_plans: BTreeMap::new(),
                static_panic_plan_bytes: 0,
                runtime_no_item_proofs: BTreeMap::new(),
                runtime_no_item_bytes: 0,
                runtime_calls: BTreeMap::new(),
                runtime_call_bytes: 0,
                floating_plans: BTreeMap::new(),
                floating_plan_bytes: 0,
                primitive_methods: BTreeMap::new(),
                primitive_method_bytes: 0,
                constant_plans: BTreeMap::new(),
                constant_plan_bytes: 0,
                declarative_runtime: Default::default(),
                prepared_call: None,
                returning: None,
                unwinding: None,
                pending: BTreeMap::new(),
                pending_bytes: 0,
                calls: vec![],
                collected: OpcodeCounters::default(),
                call_bytes: 0,
                call_limits: CallLimits::default(),
                peak_guest_frames: 1,
            })
        })();
        QueryPoll::Ready(initialize)
    }
    pub fn start_with_sources(
        db: &mut Database,
        body: BodyKey,
        fuel_limit: u64,
        sources: Vec<PreparedSourceView>,
    ) -> QueryPoll<Self> {
        if sources.len() > 100_000 {
            return QueryPoll::Ready(Err(call_fault("call source catalogue limit exceeded")));
        }
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
        fuel_limit: u64,
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
        fuel_limit: u64,
        discovery: crate::expand::source_discovery::SourceDiscovery,
    ) -> QueryPoll<Self> {
        if let Err(error) = discovery.publish(db) {
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
    fn select_argument_call(
        &self,
        db: &mut Database,
        name: &str,
        syntax: &[String],
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(r) => r,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let callee = match resolver.source_function(db, self.frame.pc().body.definition, name) {
            QueryPoll::Ready(Ok(CallSelection::Source(callee))) => callee,
            QueryPoll::Ready(Ok(_)) => {
                return QueryPoll::Ready(Err(call_fault(
                    "callee classification requires later call context integration",
                )));
            }
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        self.prepare_selected_argument_call(db, callee, syntax, argument_slots)
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
            let descriptor = match row.source().materialize(&mut self.store) {
                Ok(descriptor) => descriptor,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            explicit.push((
                row.ordinal(),
                ArgumentType {
                    source: row.source().source().clone(),
                    descriptor,
                },
            ));
        }
        self.finish_selected_argument_call(db, callee, explicit, Some(typed), arguments)
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
                    return self.prepare_selected_argument_call(db, callee, syntax, arguments);
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        // A lowered lexical source path must never fall back to a same-named
        // module callable when its entered namespace context was not supplied.
        if !name.contains("::") {
            use crate::db::dependency::{InputKey, InputValue};
            let definition = self.frame.pc().body.definition;
            let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
                Some(InputValue::DefinitionSource(source)) => source.clone(),
                _ => return QueryPoll::Ready(Err(call_fault("reached caller source absent"))),
            };
            let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
                Some(InputValue::DefinitionBody(handle))
                    if handle.key() == self.frame.pc().body =>
                {
                    handle.clone()
                }
                _ => {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached caller body identity differs",
                    )));
                }
            };
            let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
                return QueryPoll::Ready(Err(call_fault("reached caller parse kind differs")));
            };
            match parsed.entered_call_requires_lexical_context(
                &handle,
                &self.frame.body,
                self.frame.pc(),
                name,
            ) {
                Ok(true) => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "lexical callee requires exact entered source context".into(),
                    )));
                }
                Ok(false) => {}
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        }
        self.select_argument_call(db, name, syntax, arguments)
    }
    fn prepare_selected_argument_call(
        &self,
        db: &mut Database,
        callee: Box<SourceCallable>,
        syntax: &[String],
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
            for (text, parameter) in syntax.iter().zip(parameters) {
                let term = match names.unqualified(text, None) {
                    Ok(SignatureName::Primitive(p)) => crate::traits::term::TyTerm::Primitive(p),
                    Ok(SignatureName::Parameter(key)) => crate::traits::term::TyTerm::Param(key),
                    Ok(SignatureName::Source(found)) => {
                        match normalizer.normalize(db, found.identity) {
                            QueryPoll::Ready(Ok(
                                crate::frontend::type_aliases::AliasTarget::Type(term),
                            )) => term,
                            QueryPoll::Ready(Ok(_)) => {
                                return QueryPoll::Ready(Err(call_fault(
                                    "explicit alias needs further type integration",
                                )));
                            }
                            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                        }
                    }
                    Ok(_) => {
                        return QueryPoll::Ready(Err(call_fault(
                            "explicit type needs further source-view integration",
                        )));
                    }
                    Err(e) => return QueryPoll::Ready(Err(e)),
                };
                let descriptor = match &term {
                    crate::traits::term::TyTerm::Primitive(p) => {
                        match crate::frontend::call_contracts::primitive_runtime_id(*p) {
                            Ok(ty) => ty,
                            Err(e) => return QueryPoll::Ready(Err(e)),
                        }
                    }
                    crate::traits::term::TyTerm::Param(key) => {
                        match self.frame.parameter_hint(*key) {
                            Ok(Some(ty)) => ty,
                            Ok(None) => {
                                return QueryPoll::Ready(Err(Diagnostic::new(
                                    "LRR-E-TYPE-CONTEXT",
                                    "explicit caller parameter has no reached runtime binding"
                                        .into(),
                                )));
                            }
                            Err(e) => return QueryPoll::Ready(Err(e)),
                        }
                    }
                    _ => {
                        return QueryPoll::Ready(Err(call_fault(
                            "explicit structural type requires layout integration",
                        )));
                    }
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
        self.finish_selected_argument_call(db, callee, explicit, None, argument_slots)
    }
    /// Shared ordinary call context after source Types have been selected.
    /// Entered calls never pass explicit spellings through the legacy adapter.
    fn finish_selected_argument_call(
        &self,
        db: &mut Database,
        callee: Box<SourceCallable>,
        explicit: Vec<(u32, ArgumentType)>,
        typed: Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut hints = if argument_slots.is_empty() {
            Vec::new()
        } else {
            match prepare_argument_hint(db, &callee, graph, &sources, 0) {
                QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(h))) => h,
                QueryPoll::Ready(Ok(_)) => {
                    return QueryPoll::Ready(Err(call_fault(
                        "argument source hints require further type integration",
                    )));
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
        if self
            .pending_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(self.call_bytes))
            .is_none_or(|n| n > self.call_limits.max_metadata_bytes)
        {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-RUNTIME-RESOURCE",
                "call stack metadata budget exceeded".into(),
            )));
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
        if self
            .pending_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(self.call_bytes))
            .is_none_or(|n| n > self.call_limits.max_metadata_bytes)
        {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        QueryPoll::Ready(Ok(SelectedArgumentCall {
            callee,
            explicit,
            typed,
            hints,
            argument_slots: slots,
            result_hint,
            bytes,
        }))
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
        // primitive conversion. Other descriptors supply no source authority.
        if matches!(descriptor.0, 1..=15) {
            primitive_argument_type(descriptor).map(Some)
        } else {
            Ok(None)
        }
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
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("call source catalogue absent")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        if let Some(typed) = &selected.typed {
            if let Err(error) = typed.require_current(db, &self.frame.body, self.frame.pc()) {
                return QueryPoll::Ready(Err(error));
            }
        }
        let callee = selected.callee;
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
                &selected.explicit,
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
        } else {
            None
        };
        let mut references = Vec::new();
        let reference_arguments = arguments
            .iter()
            .any(|argument| matches!(argument.source, crate::traits::term::TyTerm::Ref(..)));
        if reference_arguments {
            let Some(available) = self
                .call_limits
                .max_metadata_bytes
                .checked_sub(self.call_bytes)
                .and_then(|bytes| bytes.checked_sub(self.pending_bytes))
            else {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            };
            let Some(mut reference_bytes) = arguments
                .len()
                .checked_mul(std::mem::size_of::<Option<ClosedReferenceArgument>>())
                .filter(|bytes| *bytes <= available)
            else {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            };
            if references.try_reserve_exact(arguments.len()).is_err() {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            for (ordinal, argument) in arguments.iter().enumerate() {
                let evidence = if matches!(argument.source, crate::traits::term::TyTerm::Ref(..)) {
                    let Some(slot) = selected.argument_slots.get(ordinal) else {
                        return QueryPoll::Ready(Err(call_fault(
                            "reference source argument slot absent",
                        )));
                    };
                    match ClosedReferenceArgument::capture_budgeted(
                        &self.frame,
                        *slot,
                        &self.store,
                        available - reference_bytes,
                    ) {
                        Ok(evidence) => {
                            let bytes = match evidence.allocated_bytes() {
                                Ok(bytes) => bytes,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            };
                            reference_bytes = match reference_bytes
                                .checked_add(bytes)
                                .filter(|bytes| *bytes <= available)
                            {
                                Some(bytes) => bytes,
                                None => {
                                    return QueryPoll::Ready(
                                        Err(crate::runtime::arena::resource()),
                                    );
                                }
                            };
                            Some(evidence)
                        }
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                } else {
                    None
                };
                references.push(evidence);
            }
        }
        let prepared = if reference_arguments {
            prepare_contracts_with_references(
                db,
                &callee,
                graph,
                &sources,
                &selected.explicit,
                result_hint,
                ReferenceCallArguments {
                    store: &self.store,
                    arguments,
                    references: &references,
                },
            )
        } else {
            prepare_contracts(
                db,
                &callee,
                graph,
                &sources,
                &selected.explicit,
                result_hint,
                arguments,
            )
        };
        let contracts = match prepared {
            QueryPoll::Ready(Ok(ContractPreparation::Ready(c))) => c,
            QueryPoll::Ready(Ok(_)) => {
                return QueryPoll::Ready(Err(call_fault(
                    "call contracts require later source type integration",
                )));
            }
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        // Until symbolic body-hint integration, reject named source return types
        // rather than interpreting a primitive-looking alias as its spelling.
        if callee.signature().result_symbolic.is_none() && contracts.source_result.is_none() {
            let names = match SignatureNames::from_database(db, graph, callee.signature()) {
                Ok(names) => names,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            if callee.signature().result.len() != 1 {
                return QueryPoll::Ready(Err(call_fault(
                    "symbolic return syntax awaits body-hint integration",
                )));
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
                    return QueryPoll::Ready(Err(call_fault(
                        "symbolic return identity awaits body-hint integration",
                    )));
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
                .and_then(|n| n.checked_add(context_bytes))
                .and_then(|n| n.checked_add(4096));
            if self
                .calls
                .len()
                .checked_add(2)
                .is_none_or(|frames| frames > self.call_limits.max_frames as usize)
            {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-RUNTIME-RESOURCE",
                    "guest frame limit exceeded".into(),
                )));
            }
            let Some(retained_bytes) = bytes.filter(|n| {
                self.call_bytes
                    .checked_add(*n)
                    .and_then(|total| total.checked_add(self.pending_bytes))
                    .is_some_and(|total| total <= self.call_limits.max_metadata_bytes)
            }) else {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-RUNTIME-RESOURCE",
                    "call stack metadata budget exceeded".into(),
                )));
            };
            let context =
                match crate::vm::context::GenericEnv::capture_current(db, &callee, &contracts) {
                    Ok(context) => context,
                    Err(e) => return QueryPoll::Ready(Err(e)),
                };
            let body_lease = match db.pin_guest_body(body.body()) {
                QueryPoll::Ready(Ok(lease)) => lease,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            QueryPoll::Ready(Ok(PreparedSourceCall {
                context: Some(context),
                callee,
                contracts,
                body,
                body_lease: Some(body_lease),
                retained_bytes,
            }))
        }
    }
    fn check_source_return(
        &self,
        db: &mut Database,
        call: &PreparedSourceCall,
        actual: crate::runtime::shape::TypeId,
    ) -> Result<(), Diagnostic> {
        call.callee.revalidate(db)?;
        if let Some(proof) = call
            .contracts
            .closed_references
            .as_ref()
            .filter(|proof| proof.has_result())
        {
            proof.validate_packet(call.callee.signature(), &call.contracts)?;
            return call.contracts.check_return_descriptor(actual);
        }
        if call.contracts.result_descriptor.is_some() {
            call.contracts.check_return_descriptor(actual)?;
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
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| call_fault("return source catalogue absent"))?;
        let signature = call.callee.signature();
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
        if self.static_panic_plans.contains_key(&key) || self.static_panic_plans.len() >= 4096 {
            return Err(call_fault("duplicate or excessive static panic plans"));
        }
        let charge = plan
            .allocated_bytes()?
            .checked_add(8192)
            .ok_or_else(|| call_fault("static panic plan size overflow"))?;
        let bytes = self
            .static_panic_plan_bytes
            .checked_add(charge)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| call_fault("static panic plan metadata limit"))?;
        self.static_panic_body_bases
            .insert(identity, handle.range.start);
        self.static_panic_plans.insert(key, (source, plan));
        self.static_panic_plan_bytes = bytes;
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
        max_path_bytes: usize,
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
                if selection.path.as_str().len() > max_path_bytes {
                    return Err(Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "guest frame path retention limit".into(),
                    ));
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
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "generated fault definition retention limit".into(),
            )
        };
        let mut found: Option<DefinitionOrigin> = None;
        for source in &self.sources {
            let Some(item) = source.outline().items.iter().find(|item| {
                item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                    && item.def_key.fingerprint() == *definition
            }) else {
                continue;
            };
            let selected = source.selection();
            if selected
                .path
                .as_str()
                .len()
                .checked_add(item.def_key.declared_name.len())
                .is_none_or(|n| n > 65_536)
            {
                return Err(limit());
            }
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
                .map_err(|_| limit())?;
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
    pub fn cancellation_token(&self) -> crate::vm::fuel::CancellationToken {
        self.frame.fuel.cancellation()
    }
    pub fn set_deadline(&mut self, deadline: std::time::Instant) {
        self.frame.fuel.set_deadline(deadline);
    }
    pub fn set_call_limits(&mut self, limits: CallLimits) -> Result<(), Diagnostic> {
        let limits = limits.validate()?;
        if self.terminal
            || self.frame.fuel_snapshot().used != 0
            || !self.calls.is_empty()
            || !self.pending.is_empty()
        {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "call limits require an unstarted run".into(),
            ));
        }
        self.call_limits = limits;
        Ok(())
    }
    pub fn peak_guest_frames(&self) -> u32 {
        self.peak_guest_frames
    }
    pub fn call_limits(&self) -> CallLimits {
        self.call_limits
    }
    pub fn action_snapshot(&self) -> crate::vm::action::ActionSnapshot {
        self.frame.actions.snapshot()
    }
    pub fn fuel_snapshot(&self) -> crate::vm::fuel::FuelSnapshot {
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
                RunPoll::Panicked(report) => {
                    let message = report.panic.description();
                    // This additional retained execution record has a fixed 64KiB
                    // owned budget, separate from recipes and public reports.
                    if message.len() > 65_536 {
                        self.terminal_facts = Some(TerminalFacts {
                            outcome: TerminalExecution::Fault {
                                code: "LRR-E-METADATA-LIMIT",
                            },
                            cleanup_ran: None,
                        });
                        return RunPoll::Fault(Diagnostic::new(
                            "LRR-E-METADATA-LIMIT",
                            "terminal panic observation exceeds owned budget".into(),
                        ));
                    }
                    Some(TerminalExecution::GuestPanic {
                        message: message.to_owned(),
                    })
                }
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
        if contexts.len() > 1024
            || contexts.iter().enumerate().any(|(i, context)| {
                let source = context.source();
                !self
                    .sources
                    .iter()
                    .any(|s| s.same_publication(source) && s.selection() == source.selection())
                    || contexts[..i]
                        .iter()
                        .any(|prior| prior.source().selection() == source.selection())
            })
        {
            return RunPoll::Fault(call_fault(
                "runtime macro contexts are foreign, duplicate or excessive",
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
        loop {
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
            // Rollback is bounded physical work and gets priority over a
            // depleted/cancelled run; it can never publish a source value.
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
            if let Err(error) = self.frame.fuel.require_credit() {
                if self.frame.has_source_constant_activation() {
                    return self.fail_source_constant(error);
                }
                self.terminal = true;
                return RunPoll::Fault(error);
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
            let prepared_constant = if !self.installed.contains_pc(&pc) {
                match self.prepare_source_constant_recipe(db) {
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
            let field = match self.installed.get_current(db, id) {
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
            self.frame.runtime_call = self.runtime_calls.get(self.frame.pc()).cloned();
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
                                    hint: crate::ir::op::HintRecipe::Primitive(p),
                                } if Some(*child) == result => Some(*p),
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
            for op in call_ops {
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
                        let call_hints = (|| -> Result<Vec<_>, Diagnostic> {
                            let mut result = Vec::new();
                            result
                                .try_reserve_exact(arguments.len())
                                .map_err(|_| crate::runtime::arena::resource())?;
                            for (ordinal, slot) in arguments.iter().enumerate() {
                                let (descriptor, source) = match selected.hints.get(ordinal) {
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
                                result.push((*slot, descriptor, source));
                            }
                            Ok(result)
                        })();
                        let call_hints = match call_hints {
                            Ok(hints) => hints,
                            Err(error) => {
                                self.terminal = true;
                                return RunPoll::Fault(error);
                            }
                        };
                        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
                            ordinary,
                            site: site.0,
                            hints: call_hints,
                        });
                    }
                    Op::CallAdvance { site, argument } if call_next.is_some() => {
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
                        let mut types = Vec::new();
                        let mut cells = Vec::new();
                        for slot in arguments {
                            let argument = (|| {
                                let cell = self.frame.cell(slot)?;
                                let value = self.store.admit_move(cell)?;
                                let descriptor =
                                    self.store.effective_type(value)?.ok_or_else(|| {
                                        Diagnostic::new(
                                            "LRR-E-TYPE-CONTEXT",
                                            "call argument requires a reached concrete type".into(),
                                        )
                                    })?;
                                let argument = reached_argument_type(
                                    descriptor,
                                    self.frame.source_view(slot)?,
                                )?;
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
                                    match call_result_hint {
                                        Some(primitive) => {
                                            let hinted = crate::frontend::call_contracts::primitive_runtime_id(primitive)
                                            .and_then(primitive_argument_type);
                                            match hinted {
                                                Ok(hint) => Some(hint),
                                                Err(error) => {
                                                    self.terminal = true;
                                                    return RunPoll::Fault(error);
                                                }
                                            }
                                        }
                                        None => None,
                                    }
                                }
                                Err(e) => {
                                    self.terminal = true;
                                    return RunPoll::Fault(e);
                                }
                            }
                        };
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
                            }
                            QueryPoll::Ready(Err(e)) => {
                                self.terminal = true;
                                return RunPoll::Fault(e);
                            }
                            QueryPoll::NeedInput(i) => return RunPoll::NeedInput(i),
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
                        let mut call = self.prepared_call.take().ok_or_else(|| {
                            call_fault("Call exit has no selected source context")
                        })?;
                        call.callee.revalidate(db)?;
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
                        let mut callee = Frame::new_in_run(
                            call.body.clone(),
                            call.body.pc(NodeId(0), Phase::Entry)?,
                            &mut self.store,
                            &mut self.frame,
                        )?;
                        let args = self
                            .frame
                            .call_arguments
                            .take()
                            .ok_or_else(|| call_fault("Call exit has no argument packet"))?;
                        let contracts = call
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
                            .collect::<Vec<_>>();
                        callee.install_context(call.context.take().ok_or_else(|| {
                            call_fault("accepted definition environment already consumed")
                        })?)?;
                        callee
                            .install_parameter_sources(call.contracts.source_parameters.clone())?;
                        callee.initialize_parameters(&args, &contracts, &mut self.store)?;
                        if let Some(selected) =
                            self.pending.remove(&self.frame.call_identity(site.0))
                        {
                            self.pending_bytes =
                                self.pending_bytes.checked_sub(selected.bytes).ok_or_else(
                                    || call_fault("pending context retirement underflow"),
                                )?;
                        }
                        let call_bytes = self
                            .call_bytes
                            .checked_add(call.retained_bytes)
                            .filter(|bytes| *bytes <= self.call_limits.max_metadata_bytes)
                            .ok_or_else(|| {
                                Diagnostic::new(
                                    "LRR-E-RUNTIME-RESOURCE",
                                    "call stack metadata budget exceeded".into(),
                                )
                            })?;
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
                            action: consumed.id,
                            origin: consumed.origin,
                            caller,
                            body_lease,
                            resume: consumed.resume,
                            result: consumed.result,
                            call,
                            pending,
                        });
                        Ok(())
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
