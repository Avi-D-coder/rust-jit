//! One source-parameter transport to the shared solver's parent-first order.
//! This is representation transport, not inference or a fulfillment witness.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    syntax::signatures::{DefinitionParameter, FunctionSignature, GenericKind, GenericSourceLayout},
    traits::term::{BoundKind, ParamKey},
};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct SourceParameter { source: ParamKey, kind: BoundKind, index: u32 }
impl SourceParameter {
    pub(crate) fn source(self) -> ParamKey { self.source }
    pub(crate) fn kind(self) -> BoundKind { self.kind }
}
impl Hash for SourceParameter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.definition.hash(state); self.source.ordinal.hash(state);
        self.kind.tag().hash(state); self.index.hash(state);
    }
}
impl rustc_type_ir::inherent::ParamLike for SourceParameter { fn index(self) -> u32 { self.index } }

/// Borrowed actual source layout. Written ordinals are preserved even when
/// inherited parameters and the actual trait's implicit Self precede them.
/// Only source-query/signature/impl constructors can create this view.
pub(super) struct DeclarationParameters<'a> {
    definition: Digest,
    parameters: &'a [DefinitionParameter],
    layout: GenericSourceLayout,
    trait_self: bool,
    count: u32,
    parent_count: u32,
}
pub(super) enum DeclaredParameterPreparation<'a> {
    Ready(DeclarationParameters<'a>), Pending(ParameterPending),
}
impl<'a> DeclarationParameters<'a> {
    pub(super) fn prepare(declaration: &'a crate::frontend::declaration_generics::DeclarationGenerics)
        -> Result<DeclaredParameterPreparation<'a>, Diagnostic> {
        // Written generic syntax is not a complete upstream parameter list
        // when lowering must introduce argument-position impl Trait identities.
        // Keep ordinary source ordinals/layout unchanged and do not claim even
        // a zero total count until the actual anonymous identity producer exists.
        if !declaration.argument_impl_traits().is_empty() {
            return Ok(DeclaredParameterPreparation::Pending(ParameterPending::AnonymousFunctionParameters));
        }
        if !declaration.parameter_type_macros().is_empty() {
            return Ok(DeclaredParameterPreparation::Pending(ParameterPending::FunctionParameterMacros));
        }
        let Some(layout) = declaration.layout() else {
            return Ok(DeclaredParameterPreparation::Pending(if declaration.is_function() {
                ParameterPending::FunctionRegionBinders
            } else { ParameterPending::SourceLayout }));
        };
        Self::new(declaration.definition(), declaration.parameters(), layout, declaration.has_trait_self())
            .map(DeclaredParameterPreparation::Ready)
    }
    fn new(definition: Digest, parameters: &'a [DefinitionParameter], layout: GenericSourceLayout,
        trait_self: bool) -> Result<Self, Diagnostic> {
        let written = layout.count().ok_or_else(|| fault("source generic layout index overflow"))?;
        if usize::try_from(written).ok() != Some(parameters.len())
            || parameters.iter().enumerate().any(|(i, parameter)| usize::try_from(parameter.ordinal).ok() != Some(i)) {
            return Err(fault("source generic layout differs from its declared parameters"));
        }
        if layout.implicit_self && parameters.last().is_none_or(|p| p.kind != GenericKind::Type || p.name != "Self") {
            return Err(fault("source associated trait layout lacks its actual implicit Self"));
        }
        if trait_self && (layout.implicit_self || layout.inherited != 0) {
            return Err(fault("actual trait Self conflicts with inherited declaration layout"));
        }
        let count = written.checked_add(u32::from(trait_self)).ok_or_else(|| fault("source generic count overflow"))?;
        let parent_count = layout.inherited.checked_add(u32::from(layout.implicit_self))
            .ok_or_else(|| fault("source parent generic index overflow"))?;
        Ok(Self { definition, parameters, layout, trait_self, count, parent_count })
    }
    pub(super) fn count(&self) -> u32 { self.count }
    pub(super) fn parent_count(&self) -> u32 { self.parent_count }
    pub(super) fn name(&self, ordinal: u32) -> Option<&'a str> {
        if self.trait_self && ordinal == self.count - 1 { return Some("Self"); }
        self.parameters.get(ordinal as usize).map(|parameter| parameter.name.as_str())
    }
    pub(super) fn from_source(&self, key: ParamKey, kind: BoundKind) -> Result<SourceParameter, Diagnostic> {
        if key.definition != self.definition || key.ordinal >= self.count {
            return Err(fault("upstream parameter has a foreign source binder"));
        }
        let parameter = self.parameter(key.ordinal)?;
        if parameter.kind != kind { return Err(fault("upstream parameter has a different source namespace")); }
        Ok(parameter)
    }
    pub(super) fn from_upstream(&self, index: u32) -> Result<SourceParameter, Diagnostic> {
        if index >= self.count { return Err(fault("upstream index is outside the actual source binder")); }
        let ordinal = if self.trait_self {
            if index == 0 { self.count - 1 } else { index - 1 }
        } else if index >= self.parent_count { index - self.parent_count }
        else if self.layout.implicit_self && index == 0 { self.count - 1 }
        else { self.layout.own + index - u32::from(self.layout.implicit_self) };
        self.parameter(ordinal)
    }
    fn parameter(&self, ordinal: u32) -> Result<SourceParameter, Diagnostic> {
        let implicit_trait_self = self.trait_self && ordinal == self.count - 1;
        let kind = if implicit_trait_self { BoundKind::Type } else {
            let parameter = self.parameters.get(ordinal as usize)
                .ok_or_else(|| fault("source ordinal is outside its actual declared binder"))?;
            match parameter.kind { GenericKind::Type => BoundKind::Type,
                GenericKind::Lifetime => BoundKind::Region, GenericKind::Const => BoundKind::Const }
        };
        let index = if self.trait_self { if implicit_trait_self { 0 } else { ordinal + 1 } }
            else if ordinal < self.layout.own { self.parent_count + ordinal }
            else if self.layout.implicit_self && ordinal == self.count - 1 { 0 }
            else { ordinal - self.layout.own + u32::from(self.layout.implicit_self) };
        Ok(SourceParameter { source: ParamKey { definition: self.definition, ordinal }, kind, index })
    }
}

/// View of an immutable ordinary FunctionSignature query, retaining its source
/// proof/pending policy while sharing all ordering with declaration/impl views.
pub(crate) struct SignatureParameters<'a> { parameters: DeclarationParameters<'a> }
pub(crate) enum ParameterPreparation<'a> { Ready(SignatureParameters<'a>), Pending(ParameterPending) }
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParameterPending {
    SourceLayout,
    /// Pinned collect/generics_of.rs:270 and is_late_bound. Written function
    /// lifetimes cannot all be silently treated as early parameters.
    FunctionRegionBinders,
    /// Actual parameter Type contains impl Trait; compiler-created anonymous
    /// generic declaration identities have not been supplied by preparation.
    AnonymousFunctionParameters,
    /// A parameter Type macro can introduce anonymous parameters. Its ordinary
    /// admitted expansion must join source metadata before count completeness.
    FunctionParameterMacros,
}
impl<'a> SignatureParameters<'a> {
    pub(crate) fn prepare(signature: &'a FunctionSignature) -> Result<ParameterPreparation<'a>, Diagnostic> {
        let Some(layout) = signature.generic_source_layout else {
            return Ok(ParameterPreparation::Pending(ParameterPending::SourceLayout));
        };
        let parameters = DeclarationParameters::new(signature.definition, &signature.generics, layout, false)?;
        if signature.generics[..layout.own as usize].iter().any(|p| p.kind == GenericKind::Lifetime) {
            return Ok(ParameterPreparation::Pending(ParameterPending::FunctionRegionBinders));
        }
        Ok(ParameterPreparation::Ready(Self { parameters }))
    }
    pub(crate) fn count(&self) -> u32 { self.parameters.count() }
    pub(crate) fn parent_count(&self) -> u32 { self.parameters.parent_count() }
    pub(crate) fn from_source(&self, key: ParamKey, kind: BoundKind) -> Result<SourceParameter, Diagnostic> {
        self.parameters.from_source(key, kind)
    }
    pub(crate) fn from_upstream(&self, index: u32) -> Result<SourceParameter, Diagnostic> { self.parameters.from_upstream(index) }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-PARAMETER", message.into()) }
