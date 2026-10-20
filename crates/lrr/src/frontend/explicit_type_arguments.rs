//! Explicit call Type arguments from actual indexed caller occurrences.
//! Pure declaration/Type preparation only; descriptor interning and ordinary
//! call/GenericEnv entry remain reached-coordinator responsibilities.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        entered_call_goals::{
            EnteredCallGoal, EnteredCallTarget, EnteredGenericArgumentKind, EnteredTypeArgument,
        },
        source_locals::PreparedSourceLocalPlan,
        source_types::SourceType,
    },
    ir::{
        pc::{NodeId, PcKey, Phase},
        written_type::WrittenType,
    },
    runtime::{cell::GuestStore, shape::TypeId},
    traits::term::TyTerm,
    syntax::{
        thunks_let_annotations::AnnotationShape,
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        signatures::GenericKind,
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;
#[path = "explicit_path_arguments.rs"]
mod paths;
pub(crate) use paths::{PathArgumentPosition, select_path_arguments};
const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
macro_rules! checked {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn add(bytes: usize, more: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(more)
        .ok_or_else(crate::runtime::arena::resource)
}
fn array_bytes<T>(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(crate::runtime::arena::resource)
}
/// One explicit Type argument: a named Type the entered Type service selected
/// at its own indexed occurrence, or the written structure around such
/// selected leaves (`()`, `&str`, `[u8; 2]`), selected exactly as a `let`
/// annotation's written Type is.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ExplicitType {
    Selected(SourceType),
    Written {
        term: TyTerm,
        written: Arc<WrittenType>,
    },
    /// The caller's own generic Type parameter (`TyTerm::Param`): its runtime
    /// instantiation is the reached frame's binding, read at the call.
    Parameter(TyTerm),
    /// A composite application of this actual caller's parameters. The full
    /// source term is retained separately from the runtime descriptor recipe.
    FrameTemplate {
        definition: Digest,
        term: TyTerm,
        written: Arc<WrittenType>,
    },
}
impl ExplicitType {
    pub(crate) fn source(&self) -> &TyTerm {
        match self {
            Self::Selected(source) => source.source(),
            Self::Written { term, .. } | Self::FrameTemplate { term, .. } | Self::Parameter(term) => term,
        }
    }
    /// The run-local descriptor under the calling frame: a caller generic
    /// parameter takes the frame's reached binding.
    pub(crate) fn descriptor(
        &self,
        frame: &crate::vm::frame::Frame,
        store: &mut GuestStore,
    ) -> Result<TypeId, Diagnostic> {
        match self {
            Self::Parameter(TyTerm::Param(key)) => frame.parameter_hint(*key)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "explicit caller parameter has no reached runtime binding".into(),
                )
            }),
            Self::FrameTemplate { definition, written, .. } => {
                let context = frame.type_template_context(*definition)?;
                written.materialize_with(store, &|ordinal| context.runtime_binding(
                    crate::traits::term::ParamKey { definition: *definition, ordinal }
                ))?.ok_or_else(|| fault("Type application requires its actual reached parameter bindings"))
            }
            _ => self.materialize(store),
        }
    }
    /// The run-local descriptor; a structure naming no single runtime Type
    /// (a trait object or slice pointee) has none.
    pub(crate) fn materialize(&self, store: &mut GuestStore) -> Result<TypeId, Diagnostic> {
        match self {
            Self::Parameter(_) | Self::FrameTemplate { .. } => Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "explicit caller parameter needs its calling frame's binding".into(),
            )),
            Self::Selected(source) => source.materialize(store),
            // A type argument is one concrete Type even where the same written
            // structure at a coercion site is not (`&[u8]`, `Option<fn()>`).
            Self::Written { written, .. } => written.materialize_in(store, &[])?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "explicit Type argument names no single runtime Type".into(),
                )
            }),
        }
    }
    pub(super) fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::Selected(source) => source.heap_bytes(),
            // E006: no new retained-admission resource accounting.
            Self::FrameTemplate { .. } => Ok(0),
            Self::Written { term, .. } => term
                .heap_bytes()
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<WrittenType>()))
                .ok_or_else(crate::runtime::arena::resource),
            Self::Parameter(term) => term.heap_bytes().ok_or_else(crate::runtime::arena::resource),
        }
    }
}
impl StableEncode for ExplicitType {
    const DOMAIN: &'static str = "lrr/v2/explicit-type-argument-type";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Selected(source) => {
                e.u8(0);
                e.record(source);
            }
            Self::Written { term, written } => {
                e.u8(1);
                e.record(term);
                e.record(written.as_ref());
            }
            Self::Parameter(term) => {
                e.u8(2);
                e.record(term);
            }
            Self::FrameTemplate { definition, term, written } => {
                e.u8(3);
                e.digest(*definition);
                e.record(term);
                e.record(written.as_ref());
            }
        }
    }
}
/// Common source application finalizer for aliases, associated qualifiers and
/// ordinary explicit arguments. No source or schema planning occurs at use.
pub(crate) fn prepare_application(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    term: TyTerm,
) -> QueryPoll<Option<ExplicitType>> {
    let definition = plan.context().source().index().body().definition;
    if let TyTerm::Param(key) = &term {
        return QueryPoll::Ready(Ok((key.definition == definition)
            .then_some(ExplicitType::Parameter(term))));
    }
    let Some(environment) = plan.environment() else {
        return QueryPoll::Ready(Err(fault("Type application source environment absent")));
    };
    let completed = environment.macros().source_environment();
    let sources = completed.sources().iter().collect::<Vec<_>>();
    if let Some(written) = ready!(crate::frontend::written_terms::closed_written_type(db,&sources,&term)) {
        return QueryPoll::Ready(Ok(Some(ExplicitType::Written {term,written:Arc::new(written)})));
    }
    let Some(written) = ready!(crate::frontend::written_terms::written_template(db,&sources,&term,definition)) else {
        return QueryPoll::Ready(Ok(None));
    };
    // The template converter may leave unsupported parts for inference. An
    // explicit application cannot turn those holes or foreign binders into a
    // caller's parameter: its exact symbolic source must round-trip.
    if !written.has_parameters() || written.source_with(&|ordinal| Some(TyTerm::Param(
        crate::traits::term::ParamKey {definition,ordinal}
    ))).as_ref() != Some(&term) {
        return QueryPoll::Ready(Ok(None));
    }
    QueryPoll::Ready(Ok(Some(ExplicitType::FrameTemplate {definition,term,written:Arc::new(written)})))
}
#[derive(Debug)]
pub(crate) struct ExplicitTypeArgument {
    ordinal: u32,
    site: PcKey,
    range: SourceRange,
    source: ExplicitType,
    guard_fingerprint: Digest,
    observation: Digest,
}
impl ExplicitTypeArgument {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn source(&self) -> &ExplicitType {
        &self.source
    }
}
impl StableEncode for ExplicitTypeArgument {
    const DOMAIN: &'static str = "lrr/v1/indexed-explicit-type-argument";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.ordinal);
        e.record(&self.site);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.record(&self.source);
        e.digest(self.observation);
        e.digest(self.guard_fingerprint);
    }
}
/// Clone shares immutable rows and one joined certified input closure, and
/// explicitly retains one parent-plan handle for existing source-root accounting.
#[derive(Debug, Clone)]
pub struct PreparedExplicitTypeArguments {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    target: Arc<EnteredCallTarget>,
    definition: Digest,
    signature: Digest,
    call: Digest,
    call_observation: Digest,
    guard: Arc<PreparedInputGuard>,
    rows: Arc<[ExplicitTypeArgument]>,
    /// The call result's written Type when its open type arguments are
    /// decided by later code (the common source constraints); its inputs are
    /// part of `guard`.
    inferred: Option<Arc<WrittenType>>,
    /// The callee's own generic parameters later code decided, stated as the
    /// written turbofish would (also inputs of `guard`).
    inferred_arguments: Option<Arc<crate::ir::recipe::InferredArguments>>,
    bytes: usize,
    fingerprint: Digest,
}
impl PreparedExplicitTypeArguments {
    /// The call result's Type decided from the value's later uses, stated
    /// first by the call's recipe as a parent's `SetHint` would.
    pub(crate) fn inferred_result(&self) -> Option<&Arc<WrittenType>> {
        self.inferred.as_ref()
    }
    pub(crate) fn inferred_arguments(
        &self,
    ) -> Option<&Arc<crate::ir::recipe::InferredArguments>> {
        self.inferred_arguments.as_ref()
    }
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn rows(&self) -> &[ExplicitTypeArgument] {
        &self.rows
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Cached once during cold preparation, never a per-phase row walk.
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    /// Cold proof renewal after an authorized executable-body publication.
    /// Guard/observation identities may change; source argument semantics and
    /// the selected declaration's complete signature must remain identical.
    pub(crate) fn same_call_contract(&self, other: &Self) -> bool {
        self.key == other.key
            && self.definition == other.definition
            && self.signature == other.signature
            && self.plan.source_identity() == other.plan.source_identity()
            && self.inferred == other.inferred
            && self.inferred_arguments == other.inferred_arguments
            && self.rows.len() == other.rows.len()
            && self.rows.iter().zip(other.rows.iter()).all(|(a, b)| {
                a.ordinal == b.ordinal
                    && a.site == b.site
                    && a.range == b.range
                    && a.source == b.source
            })
    }
    pub(crate) fn require_written(&self, name: &str, syntax: &[String]) -> Result<(), Diagnostic> {
        self.target.require_written(name, syntax)
    }
    /// Reached coordinator projection from retained admission, without Q44/Q47
    /// fetch. Caller site/index checks remain required at the runtime boundary.
    pub(crate) fn callable(
        &self,
        db: &mut Database,
    ) -> Result<Box<crate::frontend::callables::SourceCallable>, Diagnostic> {
        self.guard.require_current(db)?;
        self.target.callable(db)
    }
    /// Initial selected-call association. The cached goal identity commits its
    /// actual signature; no signature token walk or caller-provided digest.
    pub(crate) fn require_call(
        &self,
        db: &Database,
        index: &BodyIndex,
        pc: &PcKey,
        call: &EnteredCallGoal,
    ) -> Result<(), Diagnostic> {
        self.require_current(db, index, pc)?;
        if call.key() != &self.key
            || call.fingerprint() != self.call
            || call
                .signature()
                .is_none_or(|signature| signature.definition != self.definition)
        {
            return Err(fault("explicit Type plan selected callee differs"));
        }
        call.require_current(db)?;
        if call.indexed_source_name(index, pc)?.is_none() {
            return Err(fault("explicit Type plan indexed call differs"));
        }
        Ok(())
    }
    /// Retained boundary after argument effects/suspension. No Q44 refetch,
    /// source preparation, signature hashing or per-row guard reconstruction.
    pub(crate) fn require_current(
        &self,
        db: &Database,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<(), Diagnostic> {
        self.require_site(index, pc)?;
        self.guard.require_current(db)
    }
    /// Structural association only; this does not grant current execution
    /// authority. Cold publication renewal separately checks the actual guard.
    pub(crate) fn require_site(&self, index: &BodyIndex, pc: &PcKey) -> Result<(), Diagnostic> {
        let source = self.plan.context().source();
        let phase = Phase::from_code(pc.phase);
        if self.key.phase != 0
            || pc.body != self.key.body
            || pc.node != self.key.node
            || index.body() != &self.key.body
            || source.body().key() != self.key.body
            || index.semantic_identity() != source.index().semantic_identity()
            || !matches!(phase, Phase::Entry | Phase::Operand(_))
            || index.pc(NodeId(pc.node), phase)? != *pc
            || !index.nodes().get(pc.node as usize).is_some_and(|node| {
                node.id == NodeId(pc.node)
                    && node.role == NodeRole::Expression(ExpressionKind::Call)
            })
        {
            return Err(fault("explicit Type plan caller use differs"));
        }
        Ok(())
    }
}
impl StableEncode for PreparedExplicitTypeArguments {
    const DOMAIN: &'static str = "lrr/v1/prepared-indexed-explicit-type-arguments";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        e.digest(self.definition);
        e.digest(self.signature);
        e.digest(self.call);
        e.digest(self.call_observation);
        e.record(self.target.as_ref());
        e.digest(self.guard.fingerprint());
        e.option(&self.inferred, |e, written| e.record(written.as_ref()));
        e.option(&self.inferred_arguments, |e, inferred| {
            e.digest(inferred.definition);
            e.sequence(&inferred.arguments, |e, (ordinal, written)| {
                e.u32(*ordinal);
                e.record(written.as_ref());
            });
        });
        e.sequence(&self.rows, |e, row| e.record(row));
    }
}
/// One written Type argument selected at its indexed occurrence.
pub(crate) struct SelectedTypeArgument {
    pub(crate) source: ExplicitType,
    /// The certified inputs the selection depends on.
    pub(crate) guards: Vec<Arc<PreparedInputGuard>>,
    /// The Type plan's observation, for a plain named leaf.
    pub(crate) observation: Option<Digest>,
}
/// A written Type argument at the indexed Type occurrence `site` (`range`) of
/// the body `plan` owns, selected as a `let` annotation's written Type is:
/// each named leaf at its own indexed occurrence, with the written structure
/// (`()`, `&str`, `[u8; 2]`) around it. `key` is the owning call's entry PC.
/// `None` for the inference hole `_`. Shared by a call's own Type arguments
/// and by the arguments written on a path's owner segment
/// (`Vec::<T>::new()`), which bind the owner impl's parameters.
pub(crate) fn select_argument(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    key: &PcKey,
    site: &PcKey,
    range: SourceRange,
) -> QueryPoll<Option<SelectedTypeArgument>> {
    let owner = plan.context().source();
    let index = owner.index();
    let Some(written) = checked!(owner.parsed().syntax_at(range, |node| {
        ast::Type::can_cast(node.kind())
    }))
    .and_then(ast::Type::cast) else {
        return QueryPoll::Ready(Err(fault("explicit Type argument syntax absent")));
    };
    match checked!(owner.parsed().annotation_shape(
        owner.body(),
        index,
        key.body.definition,
        &written
    )) {
        Some(AnnotationShape::Infer) => return QueryPoll::Ready(Ok(None)),
        Some(shape)
            if !matches!(&shape, AnnotationShape::Leaf(node) if node.0 == site.node) =>
        {
            let mut selected = Vec::new();
            let written = ready!(crate::db::let_annotation_thunk::select(
                db,
                key,
                &shape,
                &mut selected
            ));
            let definition = key.body.definition;
            let Some(term) = written.source_with(&|ordinal| Some(TyTerm::Param(
                crate::traits::term::ParamKey { definition, ordinal }
            ))) else {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "explicit Type argument with an inference hole requires inference".into(),
                )));
            };
            return QueryPoll::Ready(Ok(Some(SelectedTypeArgument {
                source: if written.has_parameters() {
                    ExplicitType::FrameTemplate { definition, term, written: Arc::new(written) }
                } else {
                    ExplicitType::Written { term, written: Arc::new(written) }
                },
                guards: selected,
                observation: None,
            })));
        }
        _ => {}
    }
    // The common application query owns every alias/parameter/schema read.
    // Retain its complete guard, not only the caller target's dependencies.
    let application_key=QueryKey::PreparedBodyTypeApplication(site.clone());
    let selected=ready!(db.fetch(&SyntaxProvider,application_key.clone()));
    let QueryValue::PreparedBodyTypeApplication(application)=selected.value.as_ref()else{
        return QueryPoll::Ready(Err(fault("explicit Type application query variant differs")));
    };
    if !selected.cacheable || application.key()!=site || !plan.same_environment(application.retained_plan()) {
        return QueryPoll::Ready(Err(fault("explicit Type application actual source differs")));
    }
    let Some(source)=checked!(application.source_for(index,site,range)) else {
        return QueryPoll::Ready(Err(fault("explicit Type application requires its actual source preparation")));
    };
    let guard=Arc::new(ready!(db.prepare_input_guard(&SyntaxProvider,&application_key,selected.fingerprint)));
    checked!(guard.require_current(db));
    QueryPoll::Ready(Ok(Some(SelectedTypeArgument{source:source.clone(),guards:vec![guard],observation:Some(selected.fingerprint)})))
}

pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedExplicitTypeArguments> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("explicit Types require call entry")));
    }
    let call_key = QueryKey::EnteredCall(key.clone());
    let call = ready!(db.fetch(&SyntaxProvider, call_key.clone()));
    let QueryValue::EnteredCall(goal) = call.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("explicit Type call goal variant differs")));
    };
    if !call.cacheable {
        return QueryPoll::Ready(Err(fault("explicit Type call is not retained")));
    }
    checked!(goal.require_current(db));
    let plan = goal.retained_plan();
    let index = plan.context().source().index();
    let Some(signature) = goal.signature() else {
        return QueryPoll::Ready(Err(fault(
            "explicit Type call has no admitted source signature",
        )));
    };
    let Some(target) = checked!(goal.indexed_source_name(index, key)) else {
        return QueryPoll::Ready(Err(fault(
            "explicit Type call has no indexed source target",
        )));
    };
    // Distinct identities: this cached opaque goal is not its wrapped Q44 value.
    let call_identity = goal.fingerprint();
    let signature_identity = signature.fingerprint();
    let retained_target = checked!(goal.retained_target());
    // Full Q44 closure includes actual signature preparation, unlike Q39 alone.
    let call_guard = Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &call_key,
        call.fingerprint
    )));
    let count = checked!(
        target
            .arguments()
            .len()
            .checked_add(1)
            .ok_or_else(crate::runtime::arena::resource)
    );
    let mut guards = Vec::new();
    let mut rows = Vec::new();
    let mut bytes = checked!(add(
        std::mem::size_of::<PreparedExplicitTypeArguments>(),
        ARC_HEADER * 2
    ));
    bytes = checked!(add(bytes, retained_target.metadata_bytes()));
    bytes = checked!(add(
        bytes,
        checked!(array_bytes::<ExplicitTypeArgument>(
            target.arguments().len()
        ))
    ));
    let mut guard_bytes = checked!(add(call_guard.metadata_bytes(), ARC_HEADER));
    checked!(add(
        bytes,
        checked!(add(
            guard_bytes,
            checked!(array_bytes::<Arc<PreparedInputGuard>>(count))
        ))
    ));
    if guards.try_reserve_exact(count).is_err()
        || rows.try_reserve_exact(target.arguments().len()).is_err()
    {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    let row_spare = checked!(
        rows.capacity()
            .checked_sub(target.arguments().len())
            .ok_or_else(crate::runtime::arena::resource)
    );
    let buffers = checked!(add(
        checked!(array_bytes::<ExplicitTypeArgument>(row_spare)),
        checked!(array_bytes::<Arc<PreparedInputGuard>>(guards.capacity()))
    ));
    checked!(add(checked!(add(bytes, guard_bytes)), buffers));
    guards.push(call_guard);
    let mut parameters = signature.generics.iter().peekable();
    for argument in target.arguments() {
        if argument.kind() == EnteredGenericArgumentKind::Lifetime {
            return QueryPoll::Ready(Err(fault(
                "explicit lifetime argument requires source lifetime goals",
            )));
        }
        while parameters
            .peek()
            .is_some_and(|p| p.kind == GenericKind::Lifetime)
        {
            parameters.next();
        }
        let Some(parameter) = parameters.next() else {
            return QueryPoll::Ready(Err(fault("too many explicit generic arguments")));
        };
        if parameter.kind != GenericKind::Type
            || argument.kind() != EnteredGenericArgumentKind::Type
        {
            return QueryPoll::Ready(Err(fault(
                "explicit const or associated argument requires its typed source goal",
            )));
        }
        let Some(EnteredTypeArgument::Indexed(site)) = argument.type_argument() else {
            return QueryPoll::Ready(Err(fault(
                "explicit Type argument lacks its indexed Type occurrence",
            )));
        };
        // The argument's written structure, selected exactly as a `let`
        // annotation's: each named leaf at its own indexed Type occurrence.
        let Some(selected) = ready!(select_argument(db, plan, key, site.key(), site.range()))
        else {
            // `_` is an inference hole: the parameter is inferred as if the
            // argument were not written.
            continue;
        };
        let SelectedTypeArgument {
            source,
            guards: selected,
            observation,
        } = selected;
        let mut fingerprint = StableEncoder::new("lrr/v1/explicit-written-type-guards");
        for guard in &selected {
            checked!(guard.require_current(db));
            fingerprint.digest(guard.fingerprint());
            guard_bytes = checked!(add(
                guard_bytes,
                checked!(add(guard.metadata_bytes(), ARC_HEADER))
            ));
        }
        bytes = checked!(add(bytes, checked!(source.heap_bytes())));
        let guard_fingerprint = match (&source, selected.first()) {
            (ExplicitType::Selected(_), Some(guard)) => guard.fingerprint(),
            _ => fingerprint.finish(),
        };
        if guards.try_reserve(selected.len()).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        guards.extend(selected);
        rows.push(ExplicitTypeArgument {
            ordinal: parameter.ordinal,
            site: site.key().clone(),
            range: site.range(),
            guard_fingerprint,
            observation: observation.unwrap_or_else(|| StableEncode::fingerprint(&source)),
            source,
        });
    }
    // The Type later code decides for the call's open type arguments. A
    // plan that cannot state it leaves the explicit route unchanged.
    let (inferred, inferred_arguments) =
        match db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(key.clone())) {
            QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                QueryValue::PreparedBindingPlan(binding) if value.cacheable => {
                    match (binding.constraints().state(), binding.guard()) {
                        (
                            crate::frontend::binding_constraints::BindingConstraintState::ResolvedType,
                            Some(guard),
                        ) => {
                            guards.push(Arc::clone(guard));
                            (
                                binding
                                    .constraints()
                                    .written()
                                    .map(|written| Arc::new(written.clone())),
                                binding.constraints().inferred_arguments().cloned(),
                            )
                        }
                        _ => (None, None),
                    }
                }
                _ => (None, None),
            },
            QueryPoll::Ready(Err(_)) => (None, None),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
    let mut references = Vec::new();
    checked!(add(
        bytes,
        checked!(add(
            guard_bytes,
            checked!(add(
                checked!(array_bytes::<Arc<PreparedInputGuard>>(guards.capacity())),
                checked!(array_bytes::<&PreparedInputGuard>(guards.len()))
            ))
        ))
    ));
    if references.try_reserve_exact(guards.len()).is_err() {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    references.extend(guards.iter().map(Arc::as_ref));
    let temporary = checked!(add(
        checked!(add(guard_bytes, buffers)),
        checked!(array_bytes::<&PreparedInputGuard>(references.capacity()))
    ));
    checked!(add(bytes, temporary));
    let joined = checked!(PreparedInputGuard::join(&references));
    bytes = checked!(add(bytes, joined.metadata_bytes()));
    checked!(add(bytes, temporary));
    drop(references);
    drop(guards);
    let capacity_bytes = checked!(array_bytes::<ExplicitTypeArgument>(rows.capacity()));
    // Both the old Vec allocation and final exact slice can exist during conversion.
    checked!(add(bytes, capacity_bytes));
    let mut result = PreparedExplicitTypeArguments {
        key: key.clone(),
        plan: Arc::clone(plan),
        target: retained_target,
        definition: signature.definition,
        signature: signature_identity,
        call: call_identity,
        call_observation: call.fingerprint,
        guard: Arc::new(joined),
        inferred,
        inferred_arguments,
        rows: Arc::from(rows.into_boxed_slice()),
        bytes,
        fingerprint: Digest([0; 32]),
    };
    result.fingerprint = StableEncode::fingerprint(&result);
    checked!(result.require_call(db, index, key, goal));
    QueryPoll::Ready(Ok(result))
}
