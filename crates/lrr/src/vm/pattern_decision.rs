//! Bounded control cursor over a prepared ordinary pattern decision graph.
//! Leaf tests, projections, publication and cleanup remain Frame/ordinary ops.
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{NodeId, Phase},
    syntax::pattern_product::{DecisionContinuation, DecisionKind, PatternProductPlan},
};
use std::sync::Arc;

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TRANSACTION", message.into())
}

#[derive(Debug, Clone, Copy)]
struct Choice {
    node: NodeId,
    selected: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DecisionTarget {
    Node(NodeId, Phase),
    Matched,
    Unmatched,
    /// Exactly one exhausted choice was removed. The next charged cleanup
    /// continuation may inspect the next choice; no unbounded backtrack loop.
    Cleanup,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Position {
    Node(NodeId, Phase),
    Matched,
    Cleaning,
    Unmatched,
}

#[derive(Debug)]
pub(crate) struct DecisionState {
    plan: Arc<PatternProductPlan>,
    choices: Vec<Choice>,
    cursor: usize,
    position: Position,
}
impl DecisionState {
    pub(crate) fn required_bytes(plan: &PatternProductPlan) -> Option<usize> {
        plan.choice_bound()
            .checked_mul(std::mem::size_of::<Choice>())?
            .checked_add(std::mem::size_of::<Self>())?
            .checked_add(plan.metadata_bytes())?
            .checked_add(3 * std::mem::size_of::<usize>())
    }
    pub(crate) fn prepare(
        plan: Arc<PatternProductPlan>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        let count = plan.choice_bound();
        let bytes = Self::required_bytes(&plan)
            .filter(|bytes| *bytes <= byte_limit)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut choices = Vec::new();
        choices
            .try_reserve_exact(count)
            .map_err(|_| crate::runtime::arena::resource())?;
        let position = Position::Node(plan.root(), Phase::Entry);
        let state = Self {
            plan,
            choices,
            cursor: 0,
            position,
        };
        if state
            .metadata_bytes()
            .is_none_or(|actual| actual > byte_limit || actual < bytes)
        {
            return Err(crate::runtime::arena::resource());
        }
        Ok(state)
    }
    pub(crate) fn plan(&self) -> &Arc<PatternProductPlan> {
        &self.plan
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.choices
                    .capacity()
                    .checked_mul(std::mem::size_of::<Choice>())?,
            )?
            .checked_add(self.plan.metadata_bytes())?
            .checked_add(3 * std::mem::size_of::<usize>())
    }
    pub(crate) fn cleaning(&self) -> bool {
        self.position == Position::Cleaning
    }
    pub(crate) fn matched(&self) -> bool {
        self.position == Position::Matched
    }
    pub(crate) fn unmatched(&self) -> bool {
        self.position == Position::Unmatched
    }
    pub(crate) fn require_node(&self, node: NodeId, phase: Phase) -> Result<(), Diagnostic> {
        if self.position != Position::Node(node, phase) {
            return Err(invalid(
                "decision operation differs from retained continuation",
            ));
        }
        Ok(())
    }
    pub(crate) fn preflight_enter(&self, node: NodeId) -> Result<(), Diagnostic> {
        self.require_node(node, Phase::Entry)?;
        let row = self
            .plan
            .row(node)
            .ok_or_else(|| invalid("decision row absent"))?;
        match &row.kind {
            DecisionKind::Unsupported(error) => Err(error.clone()),
            DecisionKind::Or(alternatives) => {
                if alternatives.is_empty() || self.cursor > self.choices.len() {
                    return Err(invalid("decision choice cursor is invalid"));
                }
                if let Some(choice) = self.choices.get(self.cursor) {
                    if choice.node != node || choice.selected >= alternatives.len() {
                        return Err(invalid("decision source choice differs"));
                    }
                } else if self.choices.len() >= self.plan.choice_bound()
                    || self.choices.len() == self.choices.capacity()
                {
                    return Err(crate::runtime::arena::resource());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn preflight_success(&self, node: NodeId) -> Result<(), Diagnostic> {
        self.require_node(node, Phase::Commit)?;
        let row = self
            .plan
            .row(node)
            .ok_or_else(|| invalid("decision row absent"))?;
        if row.success.is_none() && node != self.plan.root() {
            return Err(invalid("non-root decision has no success continuation"));
        }
        Ok(())
    }
    pub(crate) fn preflight_failure(&self, failed: Option<NodeId>) -> Result<(), Diagnostic> {
        let admitted = match (failed, self.position) {
            (Some(node), Position::Node(expected, Phase::Commit)) => node == expected,
            (None, Position::Matched) => true,
            _ => false,
        };
        if !admitted || self.cursor > self.choices.len() {
            return Err(invalid(
                "decision failure differs from retained node/guard continuation",
            ));
        }
        Ok(())
    }
    fn choose(&mut self, node: NodeId) -> Result<NodeId, Diagnostic> {
        let row = self
            .plan
            .row(node)
            .ok_or_else(|| invalid("decision choice node absent"))?;
        let DecisionKind::Or(alternatives) = &row.kind else {
            return Err(invalid("decision choice is not an or-pattern"));
        };
        if alternatives.is_empty() || self.cursor > self.choices.len() {
            return Err(invalid("decision choice cursor is invalid"));
        }
        if self.cursor == self.choices.len() {
            if self.choices.len() >= self.plan.choice_bound()
                || self.choices.len() == self.choices.capacity()
            {
                return Err(crate::runtime::arena::resource());
            }
            self.choices.push(Choice { node, selected: 0 });
        }
        let choice = self.choices[self.cursor];
        if choice.node != node {
            return Err(invalid("decision retry entered a different source choice"));
        }
        let selected = *alternatives
            .get(choice.selected)
            .ok_or_else(|| invalid("decision alternative ordinal is invalid"))?;
        self.cursor = self
            .cursor
            .checked_add(1)
            .ok_or_else(crate::runtime::arena::resource)?;
        Ok(selected)
    }
    /// Called after the ordinary entered-node operations/checks succeeded.
    pub(crate) fn enter(&mut self, node: NodeId) -> Result<DecisionTarget, Diagnostic> {
        self.preflight_enter(node)?;
        let row = self
            .plan
            .row(node)
            .ok_or_else(|| invalid("entered decision node absent"))?;
        let next = match &row.kind {
            DecisionKind::Or(_) => {
                let child = self.choose(node)?;
                self.position = Position::Node(child, Phase::Entry);
                return Ok(DecisionTarget::Node(child, Phase::Entry));
            }
            DecisionKind::Tuple(children) => children.first().copied(),
            DecisionKind::Group(child) => Some(*child),
            DecisionKind::Binding(intent) => intent.subpattern,
            DecisionKind::Wildcard | DecisionKind::Scalar(_) => None,
            DecisionKind::Unsupported(error) => return Err(error.clone()),
        };
        let (next, phase) = next.map_or((node, Phase::Commit), |child| (child, Phase::Entry));
        self.position = Position::Node(next, phase);
        Ok(DecisionTarget::Node(next, phase))
    }
    pub(crate) fn success(&mut self, node: NodeId) -> Result<DecisionTarget, Diagnostic> {
        self.preflight_success(node)?;
        let row = self
            .plan
            .row(node)
            .ok_or_else(|| invalid("completed decision node absent"))?;
        let target = match row.success {
            Some(DecisionContinuation::Enter(node)) => DecisionTarget::Node(node, Phase::Entry),
            Some(DecisionContinuation::Commit(node)) => DecisionTarget::Node(node, Phase::Commit),
            None if node == self.plan.root() => DecisionTarget::Matched,
            None => return Err(invalid("non-root decision has no success continuation")),
        };
        self.position = match target {
            DecisionTarget::Node(node, phase) => Position::Node(node, phase),
            DecisionTarget::Matched => Position::Matched,
            _ => return Err(invalid("invalid success decision target")),
        };
        Ok(target)
    }
    pub(crate) fn begin_failure(&mut self, failed: Option<NodeId>) -> Result<(), Diagnostic> {
        self.preflight_failure(failed)?;
        // Unvisited suffix choices from an earlier longer attempt cannot
        // participate after a newly failed prefix. Keep actual reached choices.
        self.choices.truncate(self.cursor);
        self.position = Position::Cleaning;
        Ok(())
    }
    /// Invoke only after Frame retired one pending source owner per charged
    /// cleanup step. Each call then advances or removes at most one choice.
    pub(crate) fn backtrack_step(&mut self) -> Result<DecisionTarget, Diagnostic> {
        if self.position != Position::Cleaning {
            return Err(invalid(
                "decision backtrack requires completed attempt cleanup",
            ));
        }
        let Some(choice) = self.choices.last_mut() else {
            self.position = Position::Unmatched;
            return Ok(DecisionTarget::Unmatched);
        };
        let row = self
            .plan
            .row(choice.node)
            .ok_or_else(|| invalid("retained decision choice absent"))?;
        let DecisionKind::Or(alternatives) = &row.kind else {
            return Err(invalid("retained decision choice kind changed"));
        };
        let next = choice
            .selected
            .checked_add(1)
            .ok_or_else(crate::runtime::arena::resource)?;
        if next < alternatives.len() {
            choice.selected = next;
            self.cursor = 0;
            self.position = Position::Node(self.plan.root(), Phase::Entry);
            Ok(DecisionTarget::Node(self.plan.root(), Phase::Entry))
        } else {
            self.choices.pop();
            Ok(DecisionTarget::Cleanup)
        }
    }
}
