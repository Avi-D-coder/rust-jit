//! Pinned frontend adapter over shared owned records.
mod model;
pub(crate) mod pinned;
use crate::AdapterError;
pub use model::*;
use model::{error, limit};
use std::collections::BTreeMap;
/// Lex captured source through the pinned lexer and remap all spans to owned source identities.
/// Identifier spellings become NFC; source spans and literal spellings stay intact.
/// Whitespace/non-doc comments are not Rust tokens; doc comments follow upstream desugaring.
pub fn parse_source(
    text: &str,
    source: SourceIdentity,
    context: ContextRecord,
    origin: Key,
) -> Result<TokenForest, AdapterError> {
    pinned::parse(text, source, context, origin, &[], 0)
}
/// Lex included source using the exact retained caller context and ancestry table.
pub fn parse_source_in_context(
    text: &str,
    source: SourceIdentity,
    caller: &TokenForest,
    site: Key,
    origin: Key,
) -> Result<TokenForest, AdapterError> {
    let span = caller
        .spans()
        .iter()
        .find(|s| s.key() == site)
        .ok_or_else(|| AdapterError {
            code: "LRR-E-INCLUDE-PARSE",
            message: "include caller site is absent".into(),
        })?;
    let context = caller
        .contexts()
        .iter()
        .find(|c| c.key() == span.context)
        .ok_or_else(|| AdapterError {
            code: "LRR-E-INCLUDE-PARSE",
            message: "include caller context is absent".into(),
        })?;
    pinned::parse(text, source, context.clone(), origin, caller.contexts(), 0)
}
/// Validate transport through pinned token storage with stable span bindings.
/// Identifier spellings become NFC, including externally supplied token records.
/// This does not execute a macro or construct upstream expansion hygiene marks.
pub fn transport_round_trip(forest: &TokenForest) -> Result<TokenForest, AdapterError> {
    pinned::transport(forest, 0)
}
