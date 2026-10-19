//! Reached default binding modes for the single pattern matcher. The open
//! transaction stack lives in `matcher`; Frame owns guest commit/cleanup.
use crate::core::{
    diagnostic::Diagnostic,
    digest::{StableEncode, StableEncoder},
};
#[path = "pattern_match.rs"]
pub(crate) mod matcher;

/// Preparation requested by one actually reached non-consuming pattern goal.
/// It carries source/type facts, never a pattern evaluator or guest value.
#[derive(Debug)]
pub enum PatternDemand {
    Initializer(crate::core::digest::Digest),
    Nominal {
        slot: u32,
        ty: crate::runtime::shape::TypeId,
        source: crate::traits::term::TyTerm,
    },
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
    /// A subpattern written with another token edition (macro output keeps
    /// its owning edition) applies that edition's binding-mode rules.
    pub(crate) fn with_edition(self, edition: u16) -> Result<Self, Diagnostic> {
        Ok(Self {
            mode: self.mode,
            ..Self::new(edition)?
        })
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
    /// Consume the written reference layer. Static edition restrictions on
    /// where this is permitted belong to rustc/cargo check (E-R5-006).
    pub fn explicit_reference(self) -> Result<Self, Diagnostic> {
        Ok(Self {
            mode: DefaultBindingMode::Move,
            ..self
        })
    }
    pub(crate) fn modifier(by_reference: bool, mutable: bool) -> BindingModifier {
        match (by_reference, mutable) {
            (false, false) => BindingModifier::Plain,
            (false, true) => BindingModifier::Mut,
            (true, false) => BindingModifier::Ref,
            (true, true) => BindingModifier::RefMut,
        }
    }
    /// Binding-mode selection is non-consuming. The later pattern engine must
    /// complete its non-consuming test before committing moves or leases.
    pub fn binding_mode(self, modifier: BindingModifier) -> Result<DefaultBindingMode, Diagnostic> {
        Ok(match modifier {
            BindingModifier::Plain => self.mode,
            BindingModifier::Mut => DefaultBindingMode::Move,
            BindingModifier::Ref => DefaultBindingMode::Ref,
            BindingModifier::RefMut => DefaultBindingMode::RefMut,
        })
    }
}
