//! Primitive string emission through the pinned builtin quote implementation.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-COMPILE-INPUT",
        message: message.into(),
    }
}
pub fn emit(
    input: &TokenForest,
    site: Key,
    value: &str,
    max_wire_bytes: usize,
) -> Result<TokenForest, AdapterError> {
    if max_wire_bytes == 0 {
        return Err(fault("string output wire budget is invalid"));
    }
    let escaped = value
        .chars()
        .try_fold(0usize, |n, c| n.checked_add(c.escape_default().count()))
        .ok_or_else(|| fault("string emission escaped text limit"))?;
    let mut store = SpanStore::default();
    let _ = store.import(input)?;
    let span = store.span(site)?;
    let empty = "";
    let skeleton = store.export(&hir_expand::quote! {span=>#empty})?;
    skeleton.spans().iter().try_fold(
        escaped
            .checked_add(2)
            .ok_or_else(|| fault("output text size overflow"))?,
        |total, record| {
            total
                .checked_add(record.source.path.len())
                .ok_or_else(|| fault("output aggregate text limit"))
        },
    )?;
    let required = skeleton
        .encoded_len()
        .map_err(|e| fault(&e.to_string()))?
        .checked_add(escaped)
        .ok_or_else(|| fault("string emission wire size overflow"))?;
    let output = store.export(&hir_expand::quote! {span=>#value})?;
    if output.encoded_len().map_err(|e| fault(&e.to_string()))? != required {
        return Err(fault("string emission wire preflight mismatch"));
    }
    Ok(output)
}
