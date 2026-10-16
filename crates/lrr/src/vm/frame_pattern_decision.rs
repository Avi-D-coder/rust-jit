//! Runtime cursor/projection hooks for the retained ordinary pattern graph.
//! No source attachment, parser, namespace lookup or query provider runs here.
use super::*;
use crate::{
    ir::{
        op::{Op, PatternDecision},
        pc::Phase,
    },
    syntax::pattern_product::{DecisionKind, PatternProductPlan},
    vm::pattern::{
        DecisionSource, DecisionTransaction,
        decision::{DecisionState, DecisionTarget},
    },
};

fn projection_fault(error: crate::runtime::place::PlaceFailure) -> Diagnostic {
    match error {
        crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
        // Tuple ordinals below are checked against the actual retained source
        // before the ordinary place service. They never perform array indexing.
        crate::runtime::place::PlaceFailure::GuestPanic(_) => {
            invalid("source-certified tuple projection reported an index panic")
        }
    }
}
impl Frame {
    pub(super) fn require_decision_source(
        &self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let active = self.patterns.current(owner)?;
        let decision = self.patterns.decision(owner)?;
        let actual = self.cell(active.source)?;
        if actual != decision.source_cell || store.location_epoch(actual)? != decision.source_epoch
        {
            return Err(invalid(
                "pattern decision source storage changed during continuation",
            ));
        }
        store.admit_move(actual).map(|_| ())
    }
    fn decision_places(&self, owner: NodeId, node: NodeId) -> Result<(SlotId, SlotId), Diagnostic> {
        let scratch = |node: NodeId| -> Result<SlotId, Diagnostic> {
            let value = self
                .body
                .nodes()
                .get(node.0 as usize)
                .and_then(|node| node.runtime_temporaries[0])
                .ok_or_else(|| invalid("pattern source scratch absent"))?;
            Ok(SlotId(value.0.checked_add(1).ok_or_else(|| {
                invalid("pattern source scratch overflow")
            })?))
        };
        let a = scratch(node)?;
        let b = scratch(owner)?;
        if a == b {
            return Err(invalid("pattern source places alias"));
        }
        self.place_slot(a)?;
        self.place_slot(b)?;
        Ok((a, b))
    }
    pub(super) fn preflight_pattern_decision(
        &self,
        owner: NodeId,
        plan: &Arc<PatternProductPlan>,
        transition: &PatternDecision,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        plan.require_runtime(&self.pc.body, self.body.profile())?;
        plan.require_structure(self.body.semantic_identity())?;
        let node = NodeId(self.pc.node);
        if let PatternDecision::Begin {
            source,
            matched,
            unmatched,
            unmatched_inactive,
        } = transition
        {
            self.patterns.preflight_begin(owner)?;
            if !self.active_scopes.contains(&self.slot(*source)?.scope) {
                return Err(invalid("pattern source scope is inactive"));
            }
            for target in [matched, unmatched, unmatched_inactive] {
                if target.body != self.pc.body {
                    return Err(invalid("pattern continuation body differs"));
                }
                self.body
                    .pc(NodeId(target.node), Phase::from_code(target.phase))?;
            }
            store.admit_move(self.cell(*source)?)?;
            if store.owner(self.cell(*source)?)?
                != (CellOwner::FrameSlot {
                    frame: self.anchor,
                    slot: source.0,
                })
            {
                return Err(invalid("pattern source is not its unique frame-slot owner"));
            }
            return self.source_storage_limit(
                self.source_extra_bytes,
                DecisionTransaction::required_bytes(plan)
                    .ok_or_else(crate::runtime::arena::resource)?,
            );
        }
        let active = self.patterns.current(owner)?;
        let decision = self.patterns.decision(owner)?;
        if !Arc::ptr_eq(decision.state.plan(), plan)
            && decision.state.plan().identity() != plan.identity()
        {
            return Err(invalid("pattern decision plan differs from active source"));
        }
        self.require_decision_source(owner, store)?;
        match transition {
            PatternDecision::EnterAttempt { scope } => {
                decision.state.require_node(node, Phase::Entry)?;
                if self.active_scopes.contains(scope) {
                    if self.active_scopes.last() != Some(scope) {
                        return Err(invalid("pattern attempt retained a nested scope"));
                    }
                } else {
                    self.preflight_enter(*scope)?;
                }
                Ok(())
            }
            PatternDecision::Observe => {
                let phase = match Phase::from_code(self.pc.phase) {
                    Phase::Entry | Phase::Operand(1) => Phase::Entry,
                    Phase::Operand(2) | Phase::Commit | Phase::Exit => Phase::Commit,
                    _ => return Err(invalid("pattern observation phase differs")),
                };
                decision.state.require_node(node, phase)?;
                if Phase::from_code(self.pc.phase) == Phase::Operand(1) {
                    let cursor = decision
                        .source_cursor
                        .as_ref()
                        .filter(|cursor| cursor.node == node && cursor.ready)
                        .ok_or_else(|| {
                            invalid("pattern observation has no completed source cursor")
                        })?;
                    self.executed_place(
                        cursor
                            .place
                            .ok_or_else(|| invalid("pattern source place absent"))?,
                    )?;
                }
                Ok(())
            }
            PatternDecision::StartSource => {
                decision.state.require_node(node, Phase::Entry)?;
                if decision.source_cursor.is_some() {
                    return Err(invalid("pattern source traversal is already active"));
                }
                self.decision_places(owner, node)?;
                let row = plan
                    .row(node)
                    .ok_or_else(|| invalid("pattern source row absent"))?;
                if !matches!(
                    row.kind,
                    DecisionKind::Scalar(_) | DecisionKind::Binding(_) | DecisionKind::Tuple(_)
                ) {
                    return Err(invalid("pattern node does not require a source place"));
                }
                Ok(())
            }
            PatternDecision::SourceStep => {
                decision.state.require_node(node, Phase::Entry)?;
                let cursor = decision
                    .source_cursor
                    .as_ref()
                    .filter(|cursor| cursor.node == node && !cursor.ready)
                    .ok_or_else(|| invalid("pattern source cursor is not current"))?;
                let row = plan
                    .row(node)
                    .ok_or_else(|| invalid("pattern source row absent"))?;
                if cursor.depth > row.source_path.len() {
                    return Err(invalid("pattern source cursor exceeds retained path"));
                }
                if let Some(place) = cursor.place {
                    // Every projection starts from an actually admitted source.
                    // The common projection service performs correspondence.
                    if self.place_slot(place)?.source.is_none() {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "pattern projection requires retained source identity".into(),
                        ));
                    }
                    self.executed_place(place)?;
                    Ok(())
                } else {
                    if self.source_view_ref(active.source)?.is_none() {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "pattern source requires retained source identity".into(),
                        ));
                    }
                    self.preflight_value_place(
                        active.source,
                        self.decision_places(owner, node)?.0,
                        store,
                    )
                }
            }
            PatternDecision::CheckTuple { place, arity } => {
                decision.state.require_node(node, Phase::Entry)?;
                let cursor = decision
                    .source_cursor
                    .as_ref()
                    .filter(|cursor| {
                        cursor.node == node && cursor.ready && cursor.place == Some(*place)
                    })
                    .ok_or_else(|| invalid("tuple check differs from reached source cursor"))?;
                let _ = cursor;
                let source = self.place_slot(*place)?.source.as_ref();
                let actual_arity = match source {
                    Some(crate::traits::term::TyTerm::Tuple(members)) => members.len(),
                    Some(crate::traits::term::TyTerm::Primitive(
                        crate::ir::op::PrimitiveType::Unit,
                    )) => 0,
                    _ => {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "tuple pattern requires admitted tuple source".into(),
                        ));
                    }
                };
                if actual_arity != *arity as usize {
                    return Err(Diagnostic::new(
                        "LRR-E-PATTERN-TYPE",
                        "tuple pattern arity differs from admitted source".into(),
                    ));
                }
                Self::require_structural_source_target(
                    source.expect("checked tuple/unit source"),
                    store
                        .place_type_id(self.executed_place(*place)?)
                        .map_err(projection_fault)?,
                    store,
                )
            }
            PatternDecision::Enter => {
                decision.state.preflight_enter(node)?;
                if plan.row(node).is_some_and(|row| {
                    matches!(
                        row.kind,
                        DecisionKind::Scalar(_) | DecisionKind::Binding(_) | DecisionKind::Tuple(_)
                    )
                }) && decision.source_cursor.is_none()
                {
                    return Err(invalid(
                        "pattern source entry has no admitted source traversal",
                    ));
                }
                if let Some(cursor) = &decision.source_cursor {
                    if cursor.node != node
                        || !cursor.ready
                        || cursor
                            .place
                            .is_some_and(|place| self.slots[place.0 as usize].place.is_some())
                    {
                        return Err(invalid(
                            "pattern source place was not consumed before entry",
                        ));
                    }
                }
                Ok(())
            }
            PatternDecision::Success => decision.state.preflight_success(node),
            PatternDecision::Test { result, first, .. } => {
                if *first {
                    decision.state.preflight_enter(node)?;
                    let cursor = decision
                        .source_cursor
                        .as_ref()
                        .filter(|cursor| cursor.node == node && cursor.ready)
                        .ok_or_else(|| invalid("pattern test has no completed source cursor"))?;
                    if cursor
                        .place
                        .is_some_and(|place| self.slots[place.0 as usize].place.is_some())
                    {
                        return Err(invalid("pattern test did not retire its source place"));
                    }
                } else {
                    decision.state.require_node(node, Phase::Commit)?;
                }
                self.pattern_bool(*result, store).map(|_| ())
            }
            PatternDecision::Fail { node } => {
                decision.state.preflight_failure(*node)?;
                if node.is_none() {
                    let arm = plan
                        .arm()
                        .and_then(|arm| self.body.nodes().get(arm.0 as usize))
                        .ok_or_else(|| invalid("guard retry has no owning arm"))?;
                    if self.active_scopes.contains(&arm.scope)
                        || arm
                            .guard_scope
                            .is_some_and(|scope| self.active_scopes.contains(&scope))
                    {
                        return Err(invalid(
                            "guard retry requires completed ordinary guard/arm cleanup",
                        ));
                    }
                }
                Ok(())
            }
            PatternDecision::Cleanup => {
                if !decision.state.cleaning() || decision.source_cursor.is_some() {
                    return Err(invalid(
                        "pattern cleanup has no failed completed source attempt",
                    ));
                }
                if let Some(pending) = self.patterns.pending(owner)?.last() {
                    if self.bindings[pending.binding.0 as usize].is_some() {
                        return Err(invalid("provisional binding outlived its guard scope"));
                    }
                    // Current pending reads are source-certified intrinsic
                    // scalar copies. General owning projections remain held.
                    if self.slot(pending.source)?.cell.is_some() {
                        store.admit_copy(self.cell(pending.source)?)?;
                        self.preflight_discard(pending.source, store)?;
                    }
                }
                Ok(())
            }
            PatternDecision::Begin { .. } => unreachable!(),
        }
    }
    pub(super) fn preflight_decision_read_place(
        &self,
        place: SlotId,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let Some((active, decision)) = self.patterns.top_decision() else {
            return Ok(());
        };
        let node = NodeId(self.pc.node);
        let Some(row) = decision.state.plan().row(node) else {
            return Ok(());
        };
        self.require_decision_source(active.owner, store)?;
        decision.state.require_node(node, Phase::Entry)?;
        let cursor = decision
            .source_cursor
            .as_ref()
            .filter(|cursor| {
                cursor.node == node
                    && cursor.ready
                    && cursor.depth == row.source_path.len()
                    && cursor.place == Some(place)
            })
            .ok_or_else(|| invalid("pattern owning read differs from retained source cursor"))?;
        let _ = cursor;
        if Phase::from_code(self.pc.phase) != Phase::Operand(1)
            || self.body.nodes()[node.0 as usize].runtime_temporaries[0] != Some(destination)
        {
            return Err(invalid(
                "pattern owning read differs from exact phase/scratch",
            ));
        }
        Ok(())
    }
    pub(super) fn execute_pattern_decision(
        &mut self,
        owner: NodeId,
        plan: &Arc<PatternProductPlan>,
        transition: &PatternDecision,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let node = NodeId(self.pc.node);
        if let PatternDecision::Begin {
            source,
            matched,
            unmatched,
            unmatched_inactive,
        } = transition
        {
            let state = DecisionState::prepare(Arc::clone(plan), 32 * 1024 * 1024)?;
            let decision = DecisionTransaction {
                state,
                matched: matched.clone(),
                unmatched: unmatched.clone(),
                unmatched_inactive: unmatched_inactive.clone(),
                next: DecisionTarget::Node(plan.root(), Phase::Entry),
                producer: Some(self.pc.clone()),
                source_cursor: None,
                source_cell: self.cell(*source)?,
                source_epoch: store.location_epoch(self.cell(*source)?)?,
            };
            self.source_storage_limit(
                self.source_extra_bytes,
                decision
                    .metadata_bytes()
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?;
            return self.patterns.begin_decision(
                owner,
                *source,
                self.slot(*source)?.scope,
                decision,
            );
        }
        let target = match transition {
            PatternDecision::Observe => return Ok(()),
            PatternDecision::EnterAttempt { scope } => {
                if !self.active_scopes.contains(scope) {
                    self.enter(*scope)?;
                }
                return Ok(());
            }
            PatternDecision::StartSource => {
                self.patterns.decision_mut(owner)?.source_cursor = Some(DecisionSource {
                    node,
                    depth: 0,
                    place: None,
                    ready: false,
                });
                DecisionTarget::Node(node, Phase::Operand(0))
            }
            PatternDecision::SourceStep => {
                let active = self.patterns.current(owner)?;
                let source = active.source;
                let mut cursor = self
                    .patterns
                    .decision(owner)?
                    .source_cursor
                    .ok_or_else(|| invalid("pattern source cursor disappeared"))?;
                let row = plan
                    .row(node)
                    .ok_or_else(|| invalid("pattern source row disappeared"))?;
                let (a, b) = self.decision_places(owner, node)?;
                if let Some(base) = cursor.place {
                    if let Some(field) = row.source_path.get(cursor.depth) {
                        let destination = if base == a { b } else { a };
                        let prepared = self
                            .prepare_projection(
                                &Op::ProjectTuple {
                                    base,
                                    field: *field,
                                    destination,
                                },
                                store,
                            )
                            .map_err(projection_fault)?
                            .ok_or_else(|| {
                                invalid("ordinary tuple projection preparation absent")
                            })?;
                        self.commit_projection(prepared)?;
                        cursor.place = Some(destination);
                        cursor.depth += 1;
                    } else {
                        cursor.ready = true;
                    }
                } else {
                    self.value_place(source, a, store)?;
                    cursor.place = Some(a);
                }
                self.patterns.decision_mut(owner)?.source_cursor = Some(cursor);
                DecisionTarget::Node(
                    node,
                    if cursor.ready {
                        Phase::Operand(1)
                    } else {
                        Phase::Operand(0)
                    },
                )
            }
            PatternDecision::CheckTuple { place, .. } => {
                self.clear_place(*place)?;
                return Ok(());
            }
            PatternDecision::Enter => {
                let decision = self.patterns.decision_mut(owner)?;
                decision.source_cursor = None;
                decision.state.enter(node)?
            }
            PatternDecision::Success => self.patterns.decision_mut(owner)?.state.success(node)?,
            PatternDecision::Test {
                result,
                first,
                upper_pending,
            } => {
                let matched = self.pattern_bool(*result, store)?;
                if *first {
                    let decision = self.patterns.decision_mut(owner)?;
                    decision.source_cursor = None;
                    decision.state.enter(node)?;
                }
                DecisionTarget::Node(
                    node,
                    if matched {
                        if *upper_pending {
                            Phase::Operand(2)
                        } else {
                            Phase::Commit
                        }
                    } else {
                        Phase::Exit
                    },
                )
            }
            PatternDecision::Fail { node } => {
                self.patterns
                    .decision_mut(owner)?
                    .state
                    .begin_failure(*node)?;
                DecisionTarget::Cleanup
            }
            PatternDecision::Cleanup => {
                if let Some(pending) = self.patterns.pending(owner)?.last().copied() {
                    if self.slot(pending.source)?.cell.is_some() {
                        self.discard(pending.source, store)?;
                    }
                    self.patterns.pop_pending(owner)?;
                    DecisionTarget::Cleanup
                } else {
                    self.patterns.clear_previewed(owner)?;
                    self.patterns.decision_mut(owner)?.state.backtrack_step()?
                }
            }
            PatternDecision::Begin { .. } => unreachable!(),
        };
        let producer = self.pc.clone();
        let decision = self.patterns.decision_mut(owner)?;
        decision.next = target;
        decision.producer = Some(producer);
        Ok(())
    }
    fn decision_target(&self, owner: NodeId) -> Result<PcKey, Diagnostic> {
        let decision = self.patterns.decision(owner)?;
        if decision.producer.as_ref() != Some(&self.pc) {
            return Err(invalid(
                "pattern exit has no current completed decision transition",
            ));
        }
        match decision.next {
            DecisionTarget::Node(node, phase) => self.body.pc(node, phase),
            DecisionTarget::Matched => Ok(decision.matched.clone()),
            DecisionTarget::Unmatched => {
                let root_scope = self.body.nodes()[decision.state.plan().root().0 as usize].scope;
                Ok(if self.active_scopes.contains(&root_scope) {
                    &decision.unmatched
                } else {
                    &decision.unmatched_inactive
                }
                .clone())
            }
            DecisionTarget::Cleanup => self
                .body
                .pc(decision.state.plan().root(), Phase::PatternCleanup),
        }
    }
    pub(crate) fn preflight_pattern_next(
        &self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.require_store(store)?;
        self.patterns
            .decision(owner)?
            .state
            .plan()
            .require_runtime(&self.pc.body, self.body.profile())?;
        self.patterns
            .decision(owner)?
            .state
            .plan()
            .require_structure(self.body.semantic_identity())?;
        self.decision_target(owner)?;
        self.require_decision_source(owner, store)?;
        Ok(())
    }
    pub(crate) fn pattern_next(
        &mut self,
        owner: NodeId,
        store: &GuestStore,
    ) -> Result<PcKey, Diagnostic> {
        self.preflight_pattern_next(owner, store)?;
        let target = self.decision_target(owner)?;
        self.patterns.decision_mut(owner)?.producer = None;
        Ok(target)
    }
}
