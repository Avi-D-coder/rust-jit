//! Wire conversion of immutable owned producer facts. No filesystem or tool effects.
use crate::{core::diagnostic::Diagnostic, frontend::procedural_exports::PreparedDefinitionSite};
use lrr_expand_adapter::{
    proc_client::{CapturedSpanSource, SourceDigestEncoding},
    token_wire::{self, Key},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
pub(crate) fn source(site: &PreparedDefinitionSite) -> Result<CapturedSpanSource, Diagnostic> {
    CapturedSpanSource::new(
        token_wire::SourceIdentity {
            unit: Key(site.source.unit.0),
            content: Key(site.source.content.0),
            path: site.source.path.clone(),
        },
        site.text.to_string(),
        SourceDigestEncoding::LrrSourceBytes,
        None,
    )
    .map_err(|e| fault(&e.message))
}
pub(crate) fn merge(
    site: &PreparedDefinitionSite,
    input: &token_wire::TokenForest,
) -> Result<token_wire::TokenForest, Diagnostic> {
    let owned = crate::syntax::token_wire::TokenForest::decode(
        &input.encode().map_err(|e| fault(&e.message))?,
    )
    .map_err(|e| fault(&e.message))?;
    let merged = site.merge_into(&owned)?;
    token_wire::TokenForest::decode(&merged.encode().map_err(|e| fault(&e.message))?)
        .map_err(|e| fault(&e.message))
}
