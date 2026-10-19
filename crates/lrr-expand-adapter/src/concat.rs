//! Owned syntax-only pinned concat calculation; no HIR call or source provider.
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
pub fn literal_value(
    input: &TokenForest,
    call_site: Key,
    max_input_bytes: usize,
    max_output_bytes: usize,
) -> Result<String, AdapterError> {
    if max_input_bytes == 0 || max_output_bytes == 0 {
        return Err(fault(
            "concat limits must be positive reductions of fixed bounds",
        ));
    }
    if input.allocated_bytes().map_err(|e| fault(&e.to_string()))? > max_input_bytes {
        return Err(fault("concat input metadata limit"));
    }
    let mut store = SpanStore::default();
    let tokens = store.import(input)?;
    let call_site = store.span(call_site)?;
    let value =
        hir_expand::builtin::concat_value_tokens(&tokens, call_site).map_err(|e| AdapterError {
            code: "LRR-E-COMPILE-INPUT",
            message: match e.kind() {
                hir_expand::ExpandErrorKind::Other(message) => message.to_string(),
                _ => "unsupported pinned concat diagnostic kind".into(),
            },
        })?;
    if value.len() > max_output_bytes {
        return Err(fault("concat output string limit"));
    }
    Ok(value)
}

/// The pinned compiler hook produces owned byte-string tokens, not a guest value.
pub fn byte_output(input: &TokenForest, call_site: Key) -> Result<TokenForest, AdapterError> {
    let mut store = SpanStore::default();
    let tokens = store.import(input)?;
    let call_site = store.span(call_site)?;
    let expanded = hir_expand::builtin::concat_bytes_tokens(&tokens, call_site);
    if let Some(error) = expanded.err {
        return Err(AdapterError {
            code: "LRR-E-COMPILE-INPUT",
            message: match error.kind() {
                hir_expand::ExpandErrorKind::Other(message) => message.to_string(),
                _ => "unsupported pinned byte concat diagnostic kind".into(),
            },
        });
    }
    store.export(&expanded.value)
}
