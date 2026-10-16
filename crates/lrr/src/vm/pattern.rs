//! Reached binding modes and bounded pending pattern transactions. Transactions
//! retain checked frame identities only; Frame owns guest commit/cleanup.
use crate::core::{
    diagnostic::Diagnostic,
    digest::{StableEncode, StableEncoder},
};

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
}
/// One stack per frame, shared by all ordinary pattern owners. Capacity is
/// reserved/accounted with that frame; reached transitions allocate no vectors.
pub(crate) struct Transactions {
    active: Vec<Transaction>,
    pending: Vec<PendingBinding>,
    limit: usize,
}
fn transaction_error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TRANSACTION", message.into())
}
impl Transactions {
    pub(crate) fn metadata_bytes(limit: usize) -> Option<usize> {
        limit
            .checked_mul(
                std::mem::size_of::<Transaction>()
                    .checked_add(std::mem::size_of::<PendingBinding>())?,
            )?
            .checked_add(std::mem::size_of::<Self>())
    }
    pub(crate) fn new(limit: usize) -> Result<Self, Diagnostic> {
        if limit > 100_000 || Self::metadata_bytes(limit).is_none() {
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
        Ok(Self {
            active,
            pending,
            limit,
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
        if self.pending(owner)?.iter().any(|existing| {
            existing.binding == binding.binding || existing.source == binding.source
        }) {
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
    pub(crate) fn finish(&mut self, owner: crate::ir::pc::NodeId) -> Result<(), Diagnostic> {
        let start = self.current(owner)?.pending_start;
        self.pending.truncate(start);
        self.active.pop();
        Ok(())
    }
    pub(crate) fn retire_scope(&mut self, scope: crate::syntax::body_index::ScopeId) {
        if let Some(index) = self.active.iter().position(|active| active.scope == scope) {
            self.pending.truncate(self.active[index].pending_start);
            self.active.truncate(index);
        }
    }
}
