//! General entered Type source goals, shared across language consumers.
//! No runtime descriptors, body entry or spelling-selected primitive authority.
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
        source_locals::PreparedSourceLocalPlan,
        source_types::{NamedTypePreparation, SourceType},
    },
    ir::{
        op::PrimitiveType,
        pc::{NodeId, PcKey},
    },
    syntax::{
        body_index::{BodyIndex, NodeRole},
        spans::SourceRange,
    },
};
use std::sync::Arc;
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
#[derive(Clone)]
pub struct EnteredTypeGoal {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    range: SourceRange,
    structure: Digest,
    source: SourceType,
}
impl std::fmt::Debug for EnteredTypeGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnteredTypeGoal")
            .field("key", &self.key)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}
impl EnteredTypeGoal {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .checked_add(self.source.heap_bytes().unwrap_or(usize::MAX))
            .unwrap_or(usize::MAX)
    }
    pub(crate) fn source_type(&self) -> &SourceType {
        &self.source
    }
    /// Use an already selected concrete Type at its exact indexed occurrence.
    /// The owning PreparedTypePlan guard supplies currentness separately.
    pub(crate) fn source_for(
        &self,
        index: &BodyIndex,
        site: &PcKey,
        range: SourceRange,
    ) -> Result<&SourceType, Diagnostic> {
        let source = self.plan.context().source();
        if self.key.phase != 0
            || &self.key != site
            || index.body() != &site.body
            || source.body().key() != site.body
            || index.semantic_identity() != self.structure
            || source.index().semantic_identity() != self.structure
            || self.range != range
            || !source.body().range.contains(range)
        {
            return Err(fault("prepared Type use source/body/range differs"));
        }
        let node = index
            .nodes()
            .get(site.node as usize)
            .filter(|node| node.id == NodeId(site.node) && node.role == NodeRole::Syntax)
            .ok_or_else(|| fault("prepared Type use indexed occurrence differs"))?;
        let start = source.body().range.start;
        if start.checked_add(node.range.start) != Some(range.start)
            || start.checked_add(node.range.end) != Some(range.end)
            || range.start.checked_sub(start) != Some(node.range.start)
            || range.end.checked_sub(start) != Some(node.range.end)
        {
            return Err(fault("prepared Type use indexed range differs"));
        }
        Ok(&self.source)
    }
    pub(crate) fn primitive_for(
        &self,
        index: &BodyIndex,
        body: &PcKey,
        range: SourceRange,
    ) -> Result<PrimitiveType, Diagnostic> {
        if self.key.body != body.body {
            return Err(fault("prepared Type use source/body/range differs"));
        }
        self.source_for(index, &self.key, range)?
            .primitive()
            .ok_or_else(|| fault("selected cast target requires nonprimitive cast integration"))
    }
}
impl StableEncode for EnteredTypeGoal {
    const DOMAIN: &'static str = "lrr/v1/entered-source-type";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        e.digest(self.structure);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.record(&self.source);
    }
}
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<EnteredTypeGoal> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("entered Type requires entry key")));
    }
    let owner = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(key.body.clone())));
    let QueryValue::SourceLocalPlan(plan) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("entered Type owning plan variant differs")));
    };
    ready!(plan.require_current(db));
    let node = crate::ir::pc::NodeId(key.node);
    let occurrence = ready!(plan.context().body_type_occurrence(db, node));
    let Some(occurrence) = occurrence else {
        return QueryPoll::Ready(Err(fault("entered Type actual indexed occurrence absent")));
    };
    let range = occurrence.path_range();
    let source = match ready!(plan.prepare_named_body_type(db, node)) {
        NamedTypePreparation::Ready(source) => source,
        NamedTypePreparation::Parameter(_) => {
            return QueryPoll::Ready(Err(fault(
                "entered Type parameter needs reached source substitution",
            )));
        }
        NamedTypePreparation::Alias(_) => {
            return QueryPoll::Ready(Err(fault(
                "entered Type alias needs contextual normalization",
            )));
        }
        NamedTypePreparation::Name(_) => {
            return QueryPoll::Ready(Err(fault(
                "entered Type requires complete named source resolution",
            )));
        }
    };
    let goal = EnteredTypeGoal {
        key: key.clone(),
        plan: Arc::clone(plan),
        range,
        structure: plan.context().source().index().semantic_identity(),
        source,
    };
    if goal.metadata_bytes() > 32 * 1024 * 1024 {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    QueryPoll::Ready(Ok(goal))
}
/// One actual Type goal and input closure, retained across all use phases.
#[derive(Clone, Debug)]
pub struct PreparedTypePlan {
    goal: EnteredTypeGoal,
    guard: Arc<PreparedInputGuard>,
    guard_fingerprint: Digest,
}
impl PreparedTypePlan {
    pub(crate) fn key(&self) -> &PcKey {
        self.goal.key()
    }
    pub(crate) fn goal(&self) -> &EnteredTypeGoal {
        &self.goal
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        self.goal.retained_plan()
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.goal
            .metadata_bytes()
            .checked_add(self.guard.metadata_bytes())
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>())
            })
            .unwrap_or(usize::MAX)
    }
}
impl StableEncode for PreparedTypePlan {
    const DOMAIN: &'static str = "lrr/v1/prepared-source-type-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.goal);
        e.digest(self.guard_fingerprint);
    }
}
pub(crate) fn prepare_plan(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedTypePlan> {
    let query = QueryKey::EnteredType(key.clone());
    let value = ready!(db.fetch(&SyntaxProvider, query.clone()));
    let QueryValue::EnteredType(goal) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("prepared Type goal variant differs")));
    };
    if !value.cacheable {
        return QueryPoll::Ready(Err(fault("entered Type has unretained dependencies")));
    }
    let guard = Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &query,
        value.fingerprint
    )));
    let result = PreparedTypePlan {
        goal: goal.as_ref().clone(),
        guard_fingerprint: guard.fingerprint(),
        guard,
    };
    if result.metadata_bytes() > 64 * 1024 * 1024 {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    QueryPoll::Ready(Ok(result))
}
