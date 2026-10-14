//! Exact original SourceFile prefix producers, with parser facts confined to
//! the syntax adapter. No manual readiness or macro execution.
pub(super) use crate::syntax::original_inert::capture;
use crate::{
    core::diagnostic::Diagnostic, frontend::builtin_attributes::BuiltinAttributeCatalog,
    source::changes::PreparedSourceView,
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
/// A negative Attribute-facet witness must come from an actual matched compiler
/// world. Bang-only declarations do not become Attribute facets by spelling.
pub(super) fn matched_attribute_facet<'a>(
    source: &PreparedSourceView,
    provider: &'a super::super::source_declarative::SourceDeclarative,
) -> Result<Option<&'a BuiltinAttributeCatalog>, Diagnostic> {
    if !source.same_publication(provider.module_source())
        || source.selection() != provider.module_source().selection()
    {
        return Err(fault(
            "original inert prefix provider belongs to another source publication",
        ));
    }
    let Some(compiler) = provider.compiler.as_ref() else {
        return Ok(None);
    };
    let Some(attributes) = compiler.attribute_catalog() else {
        return Ok(None);
    };
    if compiler.runtime_prelude_unit(false)
        != crate::facade::unit(&source.selection().outline.parse.profile, "core")
        || compiler.runtime_catalog()?.is_none()
    {
        return Err(fault(
            "original inert prefix compiler Attribute facet/profile differs",
        ));
    }
    Ok(Some(attributes))
}
pub(super) fn revalidate(
    source: &PreparedSourceView,
    provider: &super::super::source_declarative::SourceDeclarative,
    proof: &super::super::nested_inert::NestedInertProof,
    namespaces: &crate::frontend::macro_names::MacroNamespaces,
) -> Result<(), Diagnostic> {
    if !capture(source, proof.scan(), 32 * 1024 * 1024)?.contains(&proof.producer()) {
        return Err(fault(
            "original inert proof is not attached to this exact root meta",
        ));
    }
    let attributes = matched_attribute_facet(source, provider)?.ok_or_else(|| {
        fault("original inert prefix compiler Attribute facet absent before sealing")
    })?;
    let expected = proof
        .attribute_facet()
        .ok_or_else(|| fault("original inert prefix has no selected Attribute-facet proof"))?;
    proof.revalidate(source.captured_macro_blob()?, source.captured_macro_cfg()?)?;
    let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
        source,
        proof.scan(),
        proof.producer(),
        namespaces,
    )?;
    let actual = super::super::builtin_attributes::certify_inert_binding(
        proof.name()?,
        &lookup,
        provider.procedural.as_deref(),
        Some(attributes),
    )?
    .ok_or_else(|| fault("original inert prefix Attribute selection remains incomplete"))?;
    if actual != expected {
        return Err(fault(
            "original inert prefix Attribute facet changed before sealing",
        ));
    }
    Ok(())
}
