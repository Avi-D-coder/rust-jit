//! Demand source constraints, never a Rust expression or runtime evaluator.
//! Names and declared types come exclusively from the common source services.
use super::source_parameter_goals::ParameterTypeGoal;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    ir::{
        op::PrimitiveType,
        pc::{NodeId, PcKey},
    },
    syntax::source_constraints::{FactKind, SourceConstraintPlan},
    traits::term::TyTerm,
};
use std::collections::BTreeSet;

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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConstraintDemand {
    Binding,
    ValueUse,
    ExpressionRelation,
    SelectedDeclaration,
    ModuleEnvironment,
    ParameterType,
    NumericFamily,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BindingConstraintState {
    NoNumericDemand,
    Unconstrained,
    Resolved(PrimitiveType),
    Pending {
        node: NodeId,
        demand: ConstraintDemand,
    },
}
/// A private-minted source result, not a binding commit or execution grant.
#[derive(Debug, Clone)]
pub struct PreparedBindingConstraints {
    key: PcKey,
    structure: Digest,
    binding: Option<Digest>,
    state: BindingConstraintState,
}
impl PreparedBindingConstraints {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn state(&self) -> BindingConstraintState {
        self.state
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
    /// Phase selection from the retained source goal. The provider must attach
    /// its actual prepared-input guard before installing the resulting recipe.
    pub(crate) fn hint_for(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
    ) -> Result<Option<crate::ir::op::HintRecipe>, Diagnostic> {
        if self.key.body != key.body
            || self.key.node != key.node
            || self.key.phase != 0
            || index.body() != &key.body
            || index.semantic_identity() != self.structure
        {
            return Err(fault("binding constraint source/body/node differs"));
        }
        match self.state {
            BindingConstraintState::NoNumericDemand | BindingConstraintState::Unconstrained => {
                Ok(None)
            }
            BindingConstraintState::Resolved(primitive) => {
                Ok(Some(crate::ir::op::HintRecipe::Primitive(primitive)))
            }
            BindingConstraintState::Pending { .. } => Err(fault(
                "numeric source constraint requires a missing common source goal",
            )),
        }
    }
    fn pending(mut self, node: NodeId, demand: ConstraintDemand) -> QueryPoll<Self> {
        self.state = BindingConstraintState::Pending { node, demand };
        QueryPoll::Ready(Ok(self))
    }
}
fn entry(key: &PcKey, node: NodeId) -> PcKey {
    PcKey {
        body: key.body.clone(),
        node: node.0,
        phase: 0,
    }
}
fn family(primitive: PrimitiveType) -> Option<bool> {
    match primitive {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::Isize
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::Usize => Some(false),
        PrimitiveType::F32 | PrimitiveType::F64 => Some(true),
        _ => None,
    }
}
fn numeric_initializer(
    plan: &SourceConstraintPlan,
    mut node: NodeId,
) -> Result<Option<bool>, Diagnostic> {
    for _ in 0..4096 {
        let fact = plan
            .fact(node)
            .ok_or_else(|| fault("numeric constraint initializer node absent"))?;
        match fact.kind() {
            FactKind::Integer => return Ok(Some(false)),
            FactKind::Float => return Ok(Some(true)),
            FactKind::Parenthesis => {
                let Some(Some(child)) = fact.operands().first() else {
                    return Ok(None);
                };
                node = *child;
            }
            FactKind::Prefix if fact.numeric_prefix() => {
                let Some(Some(child)) = fact.operands().first() else {
                    return Ok(None);
                };
                node = *child;
            }
            _ => return Ok(None),
        }
    }
    Err(crate::runtime::arena::resource())
}
/// Pure provider. Syntax discovery/prelude publication must already be genuine;
/// missing authority remains pending and never becomes a guessed primitive.
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedBindingConstraints> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("binding constraint goal requires entry phase")));
    }
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(key.body.clone())
    ));
    let QueryValue::SourceConstraints(plan) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint source plan variant differs")));
    };
    let mut result = PreparedBindingConstraints {
        key: key.clone(),
        structure: plan.structure(),
        binding: None,
        state: BindingConstraintState::NoNumericDemand,
    };
    let Some(fact) = plan.fact(NodeId(key.node)) else {
        return QueryPoll::Ready(Err(fault("binding constraint node absent")));
    };
    if fact.kind() != FactKind::Let || fact.annotation().is_some() {
        return QueryPoll::Ready(Ok(result));
    }
    let [Some(pattern), Some(initializer)] = fact.operands() else {
        return QueryPoll::Ready(Ok(result));
    };
    let floating = match numeric_initializer(plan, *initializer) {
        Ok(Some(floating)) => floating,
        Ok(None) => return QueryPoll::Ready(Ok(result)),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let selected = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::EnteredValue(entry(key, *pattern))
    ));
    let QueryValue::EnteredValue(target) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding target query variant differs")));
    };
    let Some(binding) = target.local_binding() else {
        return result.pending(*pattern, ConstraintDemand::Binding);
    };
    let Some(target) = target.binding_target() else {
        return result.pending(*pattern, ConstraintDemand::Binding);
    };
    if binding != target.identity() {
        return QueryPoll::Ready(Err(fault("binding target occurrence identity differs")));
    }
    result.binding = Some(binding);
    result.state = BindingConstraintState::Unconstrained;
    let mut concrete = None;
    let mut work = 100_000usize;
    // Spelling is only a bounded candidate filter. Actual use identity below
    // excludes another same-spelling binding, including each shadowing let.
    for use_node in plan.paths_named(target.name()) {
        let observed = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(key, *use_node))
        ));
        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("binding use query variant differs")));
        };
        if observed.pending().is_some() {
            return result.pending(*use_node, ConstraintDemand::ValueUse);
        }
        if observed.local_binding() != Some(binding) {
            continue;
        }
        let mut pending = vec![*use_node];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            work = match work.checked_sub(1) {
                Some(work) => work,
                None => return QueryPoll::Ready(Err(crate::runtime::arena::resource())),
            };
            if !visited.insert(node) {
                continue;
            }
            for use_edge in plan.uses(node) {
                let Some(parent) = plan.fact(use_edge.parent) else {
                    return QueryPoll::Ready(Err(fault("binding use parent absent")));
                };
                if parent.kind() == FactKind::Parenthesis {
                    if pending.try_reserve(1).is_err() {
                        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
                    }
                    pending.push(use_edge.parent);
                    continue;
                }
                if parent.kind() != FactKind::Call || use_edge.ordinal == 0 {
                    return result.pending(use_edge.parent, ConstraintDemand::ExpressionRelation);
                }
                let Some(Some(callee)) = parent.operands().first() else {
                    return result.pending(use_edge.parent, ConstraintDemand::SelectedDeclaration);
                };
                let selected =
                    ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(entry(key, *callee))));
                let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("constraint callee query variant differs")));
                };
                let Some(declaration) = selected.declaration() else {
                    return result.pending(*callee, ConstraintDemand::SelectedDeclaration);
                };
                if selected.pending().is_some() {
                    return result.pending(*callee, ConstraintDemand::SelectedDeclaration);
                }
                if declaration.owner().is_none() {
                    return result.pending(*callee, ConstraintDemand::ModuleEnvironment);
                }
                let goal = ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::DeclaredParameterGoal {
                        definition: declaration.identity(),
                        ordinal: use_edge.ordinal - 1
                    }
                ));
                let QueryValue::DeclaredParameterGoal(goal) = goal.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault(
                        "constraint parameter query variant differs",
                    )));
                };
                let ParameterTypeGoal::Ready(goal) = goal.goal() else {
                    return result.pending(use_edge.parent, ConstraintDemand::ParameterType);
                };
                let TyTerm::Primitive(primitive) = goal.source() else {
                    return result.pending(use_edge.parent, ConstraintDemand::ParameterType);
                };
                if family(*primitive) != Some(floating) {
                    return result.pending(use_edge.parent, ConstraintDemand::NumericFamily);
                }
                if concrete.is_some_and(|old| old != *primitive) {
                    return QueryPoll::Ready(Err(fault("source numeric constraints disagree")));
                }
                concrete = Some(*primitive);
            }
        }
    }
    if let Some(primitive) = concrete {
        result.state = BindingConstraintState::Resolved(primitive);
    }
    QueryPoll::Ready(Ok(result))
}
impl StableEncode for PreparedBindingConstraints {
    const DOMAIN: &'static str = "lrr/v1/prepared-binding-constraints";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.structure);
        e.option(&self.binding, |e, id| e.digest(*id));
        match self.state {
            BindingConstraintState::NoNumericDemand => e.u8(0),
            BindingConstraintState::Unconstrained => e.u8(1),
            BindingConstraintState::Resolved(primitive) => {
                e.u8(2);
                e.u32(primitive.tag());
            }
            BindingConstraintState::Pending { node, demand } => {
                e.u8(3);
                e.u32(node.0);
                e.u8(match demand {
                    ConstraintDemand::Binding => 0,
                    ConstraintDemand::ValueUse => 1,
                    ConstraintDemand::ExpressionRelation => 2,
                    ConstraintDemand::SelectedDeclaration => 3,
                    ConstraintDemand::ModuleEnvironment => 4,
                    ConstraintDemand::ParameterType => 5,
                    ConstraintDemand::NumericFamily => 6,
                });
            }
        }
    }
}
