//! Reached binding modes and bounded pending pattern transactions. Transactions
//! retain checked frame identities only; Frame owns guest commit/cleanup.
use crate::core::{
    diagnostic::Diagnostic,
    digest::{StableEncode, StableEncoder},
};
#[path = "pattern_decision.rs"]
pub(crate) mod decision;
#[cfg(test)]
#[path = "pattern_decision_tests.rs"]
mod decision_tests;
use decision::{DecisionState, DecisionTarget};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DecisionSource {
    pub(crate) node: crate::ir::pc::NodeId,
    pub(crate) depth: usize,
    pub(crate) place: Option<crate::ir::pc::SlotId>,
    pub(crate) ready: bool,
}

#[derive(Debug)]
pub(crate) struct DecisionTransaction {
    pub(crate) state: DecisionState,
    pub(crate) matched: crate::ir::pc::PcKey,
    pub(crate) unmatched: crate::ir::pc::PcKey,
    pub(crate) unmatched_inactive: crate::ir::pc::PcKey,
    pub(crate) next: DecisionTarget,
    pub(crate) producer: Option<crate::ir::pc::PcKey>,
    pub(crate) source_cursor: Option<DecisionSource>,
    pub(crate) source_cell: crate::runtime::cell::CellId,
    pub(crate) source_epoch: u32,
}
impl DecisionTransaction {
    fn extra_bytes() -> usize {
        std::mem::size_of::<Self>() - std::mem::size_of::<DecisionState>()
    }
    pub(crate) fn required_bytes(
        plan: &crate::syntax::pattern_product::PatternProductPlan,
    ) -> Option<usize> {
        DecisionState::required_bytes(plan)?.checked_add(Self::extra_bytes())
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        self.state
            .metadata_bytes()?
            .checked_add(Self::extra_bytes())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DefaultBindingMode {
    Move,
    Ref,
    RefMut,
}
impl StableEncode for DefaultBindingMode {
    const DOMAIN: &'static str = "lrr/v2/default-binding-mode";
    fn encode_fields(&self, encoder: &mut StableEncoder) {
        encoder.u32(match self {
            Self::Move => 0,
            Self::Ref => 1,
            Self::RefMut => 2,
        });
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReferenceKind {
    Shared,
    Mutable,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingModifier {
    Plain,
    Mut,
    Ref,
    RefMut,
}
/// Copy for each selected child; one child's explicit modifier cannot change a
/// sibling's inherited mode. Edition is the reached pattern's source edition.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PatternContext {
    edition: u16,
    mode: DefaultBindingMode,
}
impl StableEncode for PatternContext {
    const DOMAIN: &'static str = "lrr/v2/pattern-context";
    fn encode_fields(&self, encoder: &mut StableEncoder) {
        encoder.u16(self.edition);
        encoder.record(&self.mode);
    }
}
fn edition_pattern(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EDITION-PATTERN", message.into())
}
impl PatternContext {
    pub fn new(edition: u16) -> Result<Self, Diagnostic> {
        if !matches!(edition, 2015 | 2018 | 2021 | 2024) {
            return Err(Diagnostic::new(
                "LRR-E-TOOLCHAIN-CAPABILITY",
                "unknown pattern source edition".into(),
            ));
        }
        Ok(Self {
            edition,
            mode: DefaultBindingMode::Move,
        })
    }
    pub fn mode(self) -> DefaultBindingMode {
        self.mode
    }
    /// Called only when matching implicitly peels an actual reference layer.
    pub fn peel_implicit(self, reference: ReferenceKind) -> Self {
        use DefaultBindingMode::{Move, Ref, RefMut};
        Self {
            mode: match (reference, self.mode) {
                (ReferenceKind::Shared, _) | (_, Ref) => Ref,
                (ReferenceKind::Mutable, Move | RefMut) => RefMut,
            },
            ..self
        }
    }
    /// Check the reached explicit reference pattern before consuming its layer.
    pub fn explicit_reference(self) -> Result<Self, Diagnostic> {
        if self.edition == 2024 && self.mode != DefaultBindingMode::Move {
            return Err(edition_pattern(
                "explicit reference pattern requires move binding mode in edition2024",
            ));
        }
        Ok(Self {
            mode: DefaultBindingMode::Move,
            ..self
        })
    }
    /// Binding-mode selection is non-consuming. The later pattern engine must
    /// complete its non-consuming test before committing moves or leases.
    pub fn binding_mode(self, modifier: BindingModifier) -> Result<DefaultBindingMode, Diagnostic> {
        if self.edition == 2024
            && self.mode != DefaultBindingMode::Move
            && modifier != BindingModifier::Plain
        {
            return Err(edition_pattern(
                "explicit binding modifier requires move binding mode in edition2024",
            ));
        }
        Ok(match modifier {
            BindingModifier::Plain => self.mode,
            BindingModifier::Mut => DefaultBindingMode::Move,
            BindingModifier::Ref => DefaultBindingMode::Ref,
            BindingModifier::RefMut => DefaultBindingMode::RefMut,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PendingBinding {
    pub(crate) binding: crate::ir::pc::NodeId,
    pub(crate) source: crate::ir::pc::SlotId,
    pub(crate) writable: bool,
}
#[derive(Debug)]
pub(crate) struct Transaction {
    pub(crate) owner: crate::ir::pc::NodeId,
    pub(crate) alternative: crate::ir::pc::NodeId,
    pub(crate) source: crate::ir::pc::SlotId,
    pub(crate) retry: crate::ir::pc::PcKey,
    scope: crate::syntax::body_index::ScopeId,
    pending_start: usize,
    pub(crate) previewed: bool,
    pub(crate) decision: Option<Box<DecisionTransaction>>,
    dynamic_before: usize,
}
/// One stack per frame, shared by all ordinary pattern owners. Capacity is
/// reserved/accounted with that frame; reached transitions allocate no vectors.
pub(crate) struct Transactions {
    active: Vec<Transaction>,
    pending: Vec<PendingBinding>,
    limit: usize,
    dynamic_bytes: usize,
    binding_rows: Vec<usize>,
    source_rows: Vec<usize>,
}
fn transaction_error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TRANSACTION", message.into())
}
impl Transactions {
    pub(crate) fn metadata_bytes(limit: usize, slots: usize) -> Option<usize> {
        limit
            .checked_mul(
                std::mem::size_of::<Transaction>()
                    .checked_add(std::mem::size_of::<PendingBinding>())?,
            )?
            .checked_add(std::mem::size_of::<Self>())?
            .checked_add(
                limit
                    .checked_add(slots)?
                    .checked_mul(std::mem::size_of::<usize>())?,
            )
    }
    pub(crate) fn new(limit: usize, slots: usize) -> Result<Self, Diagnostic> {
        if limit > 100_000
            || Self::metadata_bytes(limit, slots).is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(crate::runtime::arena::resource());
        }
        let mut active = Vec::new();
        let mut pending = Vec::new();
        active
            .try_reserve_exact(limit)
            .map_err(|_| crate::runtime::arena::resource())?;
        pending
            .try_reserve_exact(limit)
            .map_err(|_| crate::runtime::arena::resource())?;
        let mut binding_rows = Vec::new();
        let mut source_rows = Vec::new();
        binding_rows
            .try_reserve_exact(limit)
            .map_err(|_| crate::runtime::arena::resource())?;
        source_rows
            .try_reserve_exact(slots)
            .map_err(|_| crate::runtime::arena::resource())?;
        if active.capacity() != limit
            || pending.capacity() != limit
            || binding_rows.capacity() != limit
            || source_rows.capacity() != slots
        {
            return Err(crate::runtime::arena::resource());
        }
        binding_rows.resize(limit, usize::MAX);
        source_rows.resize(slots, usize::MAX);
        Ok(Self {
            active,
            pending,
            limit,
            dynamic_bytes: 0,
            binding_rows,
            source_rows,
        })
    }
    pub(crate) fn preflight_begin(&self, owner: crate::ir::pc::NodeId) -> Result<(), Diagnostic> {
        if self.active.len() >= self.limit || self.active.iter().any(|active| active.owner == owner)
        {
            return Err(transaction_error(
                "pattern owner is already active or exceeds frame bound",
            ));
        }
        Ok(())
    }
    pub(crate) fn begin(
        &mut self,
        owner: crate::ir::pc::NodeId,
        alternative: crate::ir::pc::NodeId,
        source: crate::ir::pc::SlotId,
        retry: crate::ir::pc::PcKey,
        scope: crate::syntax::body_index::ScopeId,
    ) -> Result<(), Diagnostic> {
        self.preflight_begin(owner)?;
        self.active.push(Transaction {
            owner,
            alternative,
            source,
            retry,
            scope,
            pending_start: self.pending.len(),
            previewed: false,
            decision: None,
            dynamic_before: self.dynamic_bytes,
        });
        Ok(())
    }
    pub(crate) fn current(&self, owner: crate::ir::pc::NodeId) -> Result<&Transaction, Diagnostic> {
        self.active
            .last()
            .filter(|active| active.owner == owner)
            .ok_or_else(|| {
                transaction_error("pattern continuation does not own the active transaction")
            })
    }
    pub(crate) fn top_decision(&self) -> Option<(&Transaction, &DecisionTransaction)> {
        let active = self.active.last()?;
        Some((active, active.decision.as_deref()?))
    }
    pub(crate) fn dynamic_metadata_bytes(&self) -> usize {
        self.dynamic_bytes
    }
    pub(crate) fn begin_decision(
        &mut self,
        owner: crate::ir::pc::NodeId,
        source: crate::ir::pc::SlotId,
        scope: crate::syntax::body_index::ScopeId,
        decision: DecisionTransaction,
    ) -> Result<(), Diagnostic> {
        self.preflight_begin(owner)?;
        let added = decision
            .metadata_bytes()
            .ok_or_else(crate::runtime::arena::resource)?;
        let bytes = self
            .dynamic_bytes
            .checked_add(added)
            .ok_or_else(crate::runtime::arena::resource)?;
        let alternative = decision.state.plan().root();
        let retry = decision.unmatched.clone();
        let retained = Box::new(decision);
        self.begin(owner, alternative, source, retry, scope)?;
        self.active
            .last_mut()
            .expect("checked transaction")
            .decision = Some(retained);
        self.dynamic_bytes = bytes;
        Ok(())
    }
    pub(crate) fn decision(
        &self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<&DecisionTransaction, Diagnostic> {
        self.current(owner)?
            .decision
            .as_deref()
            .ok_or_else(|| transaction_error("pattern transaction has no prepared decision"))
    }
    pub(crate) fn decision_mut(
        &mut self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<&mut DecisionTransaction, Diagnostic> {
        self.current(owner)?;
        self.active
            .last_mut()
            .and_then(|active| active.decision.as_deref_mut())
            .ok_or_else(|| transaction_error("pattern transaction has no prepared decision"))
    }
    pub(crate) fn pop_pending(
        &mut self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<Option<PendingBinding>, Diagnostic> {
        let start = self.current(owner)?.pending_start;
        if self.pending.len() > start {
            Ok(self.pending.pop())
        } else {
            Ok(None)
        }
    }
    pub(crate) fn pending(
        &self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<&[PendingBinding], Diagnostic> {
        let current = self.current(owner)?;
        Ok(&self.pending[current.pending_start..])
    }
    pub(crate) fn preflight_stage(
        &self,
        owner: crate::ir::pc::NodeId,
        binding: PendingBinding,
    ) -> Result<(), Diagnostic> {
        let current = self.current(owner)?;
        if current.previewed || self.pending.len() >= self.limit {
            return Err(transaction_error(
                "pattern binding cannot be staged after preview or beyond frame bound",
            ));
        }
        let binding_row = *self
            .binding_rows
            .get(binding.binding.0 as usize)
            .ok_or_else(|| transaction_error("binding identity exceeds frame pattern bound"))?;
        let source_row = *self
            .source_rows
            .get(binding.source.0 as usize)
            .ok_or_else(|| transaction_error("source identity exceeds frame slot bound"))?;
        // A stale row after truncate/pop has no authority. Reused vector
        // positions must still contain this exact identity. Thus cleanup needs
        // no O(binding-count) marker-clearing pass or generation reset.
        if self
            .pending
            .get(binding_row)
            .is_some_and(|existing| existing.binding == binding.binding)
            || self
                .pending
                .get(source_row)
                .is_some_and(|existing| existing.source == binding.source)
        {
            return Err(transaction_error(
                "conflicting pattern binding or source move",
            ));
        }
        Ok(())
    }
    pub(crate) fn stage(
        &mut self,
        owner: crate::ir::pc::NodeId,
        binding: PendingBinding,
    ) -> Result<(), Diagnostic> {
        self.preflight_stage(owner, binding)?;
        self.binding_rows[binding.binding.0 as usize] = self.pending.len();
        self.source_rows[binding.source.0 as usize] = self.pending.len();
        self.pending.push(binding);
        Ok(())
    }
    pub(crate) fn mark_previewed(
        &mut self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<(), Diagnostic> {
        if self.current(owner)?.previewed {
            return Err(transaction_error("pattern bindings were already previewed"));
        }
        self.active
            .last_mut()
            .expect("checked active transaction")
            .previewed = true;
        Ok(())
    }
    pub(crate) fn clear_previewed(
        &mut self,
        owner: crate::ir::pc::NodeId,
    ) -> Result<(), Diagnostic> {
        self.current(owner)?;
        self.active
            .last_mut()
            .expect("checked transaction")
            .previewed = false;
        Ok(())
    }
    pub(crate) fn finish(&mut self, owner: crate::ir::pc::NodeId) -> Result<(), Diagnostic> {
        let active = self.current(owner)?;
        let start = active.pending_start;
        let bytes = active.dynamic_before;
        self.pending.truncate(start);
        self.active.pop();
        self.dynamic_bytes = bytes;
        Ok(())
    }
    pub(crate) fn retire_scope(&mut self, scope: crate::syntax::body_index::ScopeId) {
        if let Some(index) = self.active.iter().position(|active| active.scope == scope) {
            self.dynamic_bytes = self.active[index].dynamic_before;
            self.pending.truncate(self.active[index].pending_start);
            self.active.truncate(index);
        }
    }
}
