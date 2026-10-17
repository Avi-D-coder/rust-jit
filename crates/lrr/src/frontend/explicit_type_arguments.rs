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
    ir::pc::{NodeId, PcKey, Phase},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        signatures::GenericKind,
        spans::SourceRange,
    },
};
use std::sync::Arc;
const LIMIT: usize = 32 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 100_000;
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
        .filter(|n| *n <= LIMIT)
        .ok_or_else(crate::runtime::arena::resource)
}
fn array_bytes<T>(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .filter(|n| *n <= LIMIT)
        .ok_or_else(crate::runtime::arena::resource)
}
#[derive(Debug)]
pub(crate) struct ExplicitTypeArgument {
    ordinal: u32,
    site: PcKey,
    range: SourceRange,
    source: SourceType,
    guard_fingerprint: Digest,
    observation: Digest,
}
impl ExplicitTypeArgument {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn source(&self) -> &SourceType {
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
    bytes: usize,
    fingerprint: Digest,
}
impl PreparedExplicitTypeArguments {
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
        e.sequence(&self.rows, |e, row| e.record(row));
    }
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
    if target.arguments().len() > MAX_ARGUMENTS {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
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
        let value = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedTypePlan(site.key().clone())
        ));
        let QueryValue::PreparedTypePlan(ty) = value.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("explicit Type plan query variant differs")));
        };
        if !value.cacheable || !plan.same_environment(ty.retained_plan()) {
            return QueryPoll::Ready(Err(fault("explicit Type argument owning source differs")));
        }
        checked!(ty.guard().require_current(db));
        let source = checked!(ty.goal().source_for(index, site.key(), site.range()));
        let reserved = checked!(source.heap_bytes());
        bytes = checked!(add(bytes, reserved));
        checked!(add(checked!(add(bytes, guard_bytes)), buffers));
        let owned = source.clone();
        bytes = checked!(add(
            checked!(
                bytes
                    .checked_sub(reserved)
                    .ok_or_else(crate::runtime::arena::resource)
            ),
            checked!(owned.heap_bytes())
        ));
        guard_bytes = checked!(add(
            guard_bytes,
            checked!(add(ty.guard().metadata_bytes(), ARC_HEADER))
        ));
        checked!(add(checked!(add(bytes, guard_bytes)), buffers));
        guards.push(Arc::clone(ty.guard()));
        rows.push(ExplicitTypeArgument {
            ordinal: parameter.ordinal,
            site: site.key().clone(),
            range: site.range(),
            source: owned,
            guard_fingerprint: ty.guard().fingerprint(),
            observation: value.fingerprint,
        });
    }
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
        rows: Arc::from(rows.into_boxed_slice()),
        bytes,
        fingerprint: Digest([0; 32]),
    };
    result.fingerprint = StableEncode::fingerprint(&result);
    checked!(result.require_call(db, index, key, goal));
    QueryPoll::Ready(Ok(result))
}
