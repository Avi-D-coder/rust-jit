//! Map real source binder identities to the shared solver's parent-first order.
//! This is representation transport, not inference or a fulfillment witness.
use crate::{
    core::diagnostic::Diagnostic,
    syntax::signatures::{FunctionSignature, GenericKind, GenericSourceLayout},
    traits::term::{BoundKind, ParamKey},
};
use std::hash::{Hash, Hasher};

/// A source parameter and its position in this prepared upstream binder.
/// The source key stays distinct from the solver's local positional index.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct SourceParameter {
    source: ParamKey,
    kind: BoundKind,
    index: u32,
}
impl SourceParameter {
    pub(crate) fn source(self) -> ParamKey {
        self.source
    }
    pub(crate) fn kind(self) -> BoundKind {
        self.kind
    }
}
impl Hash for SourceParameter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.definition.hash(state);
        self.source.ordinal.hash(state);
        self.kind.tag().hash(state);
        self.index.hash(state);
    }
}
impl rustc_type_ir::inherent::ParamLike for SourceParameter {
    fn index(self) -> u32 {
        self.index
    }
}

/// Borrowed view of an immutable ordinary FunctionSignature query result.
/// Construct once when preparing an upstream context. Its owner must retain
/// the real query dependencies/currentness; this view grants no source or call
/// authority and deliberately cannot turn a synthetic signature into one.
pub(crate) struct SignatureParameters<'a> {
    signature: &'a FunctionSignature,
    layout: GenericSourceLayout,
    count: u32,
    parent_count: u32,
}
/// Missing representation facts are not a negative trait answer or a guest
/// rejection. Preparation must supply them before entering the shared solver.
pub(crate) enum ParameterPreparation<'a> {
    Ready(SignatureParameters<'a>),
    Pending(ParameterPending),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParameterPending {
    SourceLayout,
    /// rustc excludes late-bound function lifetimes from `Generics::own_params`.
    /// See the pinned collect/generics_of.rs:270 and is_late_bound provider.
    /// Do not silently treat every written lifetime as an early parameter.
    FunctionRegionBinders,
}
impl<'a> SignatureParameters<'a> {
    pub(crate) fn prepare(
        signature: &'a FunctionSignature,
    ) -> Result<ParameterPreparation<'a>, Diagnostic> {
        let Some(layout) = signature.generic_source_layout else {
            return Ok(ParameterPreparation::Pending(
                ParameterPending::SourceLayout,
            ));
        };
        let count = layout
            .count()
            .ok_or_else(|| fault("source generic layout index overflow"))?;
        if usize::try_from(count).ok() != Some(signature.generics.len())
            || signature
                .generics
                .iter()
                .enumerate()
                .any(|(i, parameter)| usize::try_from(parameter.ordinal).ok() != Some(i))
        {
            return Err(fault(
                "source generic layout differs from its declared parameters",
            ));
        }
        let parent_count = layout
            .inherited
            .checked_add(u32::from(layout.implicit_self))
            .ok_or_else(|| fault("source parent generic index overflow"))?;
        if layout.implicit_self
            && signature
                .generics
                .last()
                .is_none_or(|p| p.kind != GenericKind::Type || p.name != "Self")
        {
            return Err(fault(
                "source trait layout lacks its declared implicit Self",
            ));
        }
        if signature.generics[..layout.own as usize]
            .iter()
            .any(|p| p.kind == GenericKind::Lifetime)
        {
            return Ok(ParameterPreparation::Pending(
                ParameterPending::FunctionRegionBinders,
            ));
        }
        Ok(ParameterPreparation::Ready(Self {
            signature,
            layout,
            count,
            parent_count,
        }))
    }
    pub(crate) fn count(&self) -> u32 {
        self.count
    }
    pub(crate) fn parent_count(&self) -> u32 {
        self.parent_count
    }
    /// Rust puts parent parameters first. A trait's implicit Self is the
    /// parent's first slot; LRR retains it after the inherited written list.
    pub(crate) fn from_source(
        &self,
        key: ParamKey,
        kind: BoundKind,
    ) -> Result<SourceParameter, Diagnostic> {
        if key.definition != self.signature.definition || key.ordinal >= self.count {
            return Err(fault("upstream parameter has a foreign source binder"));
        }
        let actual = self.parameter(key.ordinal)?;
        if actual.kind != kind {
            return Err(fault("upstream parameter has a different source namespace"));
        }
        Ok(actual)
    }
    pub(crate) fn from_upstream(&self, index: u32) -> Result<SourceParameter, Diagnostic> {
        if index >= self.count {
            return Err(fault(
                "upstream parameter index is outside its source binder",
            ));
        }
        let ordinal = if index >= self.parent_count {
            index - self.parent_count
        } else if self.layout.implicit_self && index == 0 {
            self.count - 1
        } else {
            self.layout.own + index - u32::from(self.layout.implicit_self)
        };
        self.parameter(ordinal)
    }
    fn parameter(&self, ordinal: u32) -> Result<SourceParameter, Diagnostic> {
        let parameter = usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| self.signature.generics.get(ordinal))
            .ok_or_else(|| fault("source parameter index is outside its declared binder"))?;
        let index = if ordinal < self.layout.own {
            self.parent_count + ordinal
        } else if self.layout.implicit_self && ordinal == self.count - 1 {
            0
        } else {
            ordinal - self.layout.own + u32::from(self.layout.implicit_self)
        };
        Ok(SourceParameter {
            source: ParamKey {
                definition: self.signature.definition,
                ordinal,
            },
            kind: match parameter.kind {
                GenericKind::Lifetime => BoundKind::Region,
                GenericKind::Type => BoundKind::Type,
                GenericKind::Const => BoundKind::Const,
            },
            index,
        })
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
