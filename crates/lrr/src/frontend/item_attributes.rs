//! Custom attribute classification after compiler-owned builtin identity handling.
//! No execution, guessed inert attributes, or unresolved negative publication.
use super::{macro_names::PathLookup, procedural_exports::PreparedProcCatalog};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::{
        derive::{HelperLookup, ItemHelperScope},
        identity::{ExpansionKey, MacroIdentity, ProceduralKind},
        scopes::{MacroDefinition, MacroName},
    },
};
pub enum CustomAttributeDecision {
    Helpers(Vec<ExpansionKey>),
    Procedural(MacroIdentity),
    NeedHelpers(Vec<ExpansionKey>),
    NeedNamespace(Digest),
    NeedDefinition(MacroDefinition),
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
/// Compiler-owned builtins are handled by identity before entering this stage.
/// A ready unqualified helper precedes ordinary namespace candidates; qualified
/// paths cannot borrow item-local helper visibility.
pub fn classify_custom_attribute(
    helpers: &ItemHelperScope,
    owner: Digest,
    cfg: Digest,
    active: &[ExpansionKey],
    path: &[MacroName],
    namespace: PathLookup,
    catalog: &PreparedProcCatalog,
) -> Result<CustomAttributeDecision, Diagnostic> {
    if path.is_empty() || path.len() > 256 {
        return Err(fault("empty or oversized attribute path"));
    }
    classify_custom_candidate(
        helpers,
        owner,
        cfg,
        active,
        (path.len() == 1).then(|| path[0].as_str()),
        namespace,
        catalog,
    )
}
pub(crate) fn classify_custom_candidate(
    helpers: &ItemHelperScope,
    owner: Digest,
    cfg: Digest,
    active: &[ExpansionKey],
    unqualified: Option<&str>,
    namespace: PathLookup,
    catalog: &PreparedProcCatalog,
) -> Result<CustomAttributeDecision, Diagnostic> {
    if let Some(name) = unqualified {
        match helpers.lookup_helpers(owner, cfg, active, &format!("r#{name}"))? {
            HelperLookup::NeedDeclarations(keys) => {
                return Ok(CustomAttributeDecision::NeedHelpers(keys));
            }
            HelperLookup::Ready(owners) if !owners.is_empty() => {
                return Ok(CustomAttributeDecision::Helpers(owners));
            }
            HelperLookup::Ready(_) => {}
        }
    }
    match namespace {
        PathLookup::NeedInput(input) => Ok(CustomAttributeDecision::NeedNamespace(input)),
        PathLookup::Defined(definition) => {
            match catalog.select(&definition, ProceduralKind::Attribute)? {
                Some(export) => Ok(CustomAttributeDecision::Procedural(
                    export.identity().clone(),
                )),
                None => Ok(CustomAttributeDecision::NeedDefinition(definition)),
            }
        }
        PathLookup::Absent => Err(fault(
            "custom attribute is neither an active helper nor a resolved macro",
        )),
        PathLookup::Ambiguous(_) => Err(fault("custom attribute namespace is ambiguous")),
        PathLookup::Inaccessible(_) => Err(fault("custom attribute namespace is inaccessible")),
    }
}
