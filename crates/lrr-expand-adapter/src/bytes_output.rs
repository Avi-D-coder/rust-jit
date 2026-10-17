//! Owned byte-string emission through pinned TT construction and span export.
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
    value: &[u8],
    max_wire_bytes: usize,
) -> Result<TokenForest, AdapterError> {
    if max_wire_bytes == 0 {
        return Err(fault("byte output wire budget is invalid"));
    }
    let escaped = value
        .iter()
        .try_fold(0usize, |n, b| {
            n.checked_add(std::ascii::escape_default(*b).count())
        })
        .ok_or_else(|| fault("byte output escaped text limit"))?;
    let mut store = SpanStore::default();
    let _ = store.import(input)?;
    let span = store.span(site)?;
    let tree = |text: &str| {
        tt::TopSubtree::invisible_from_leaves(
            span,
            [tt::Leaf::Literal(tt::Literal::new_no_suffix(
                text,
                span,
                tt::LitKind::ByteStr,
            ))],
        )
    };
    let skeleton = store.export(&tree(""))?;
    skeleton.spans().iter().try_fold(
        escaped
            .checked_add(3)
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
        .ok_or_else(|| fault("byte output wire size overflow"))?;
    let mut text = String::with_capacity(escaped);
    for byte in value {
        text.extend(std::ascii::escape_default(*byte).map(char::from));
    }
    let output = store.export(&tree(&text))?;
    if output.encoded_len().map_err(|e| fault(&e.to_string()))? != required {
        return Err(fault("byte output wire preflight mismatch"));
    }
    Ok(output)
}
