//! Immutable, schema-checked expansion transport; no expansion executes here.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::identity::{ExpansionKey, MacroIdentity},
    syntax::token_wire::TokenForest,
};
#[derive(Debug, Clone)]
pub struct ExpansionRecord {
    key: ExpansionKey,
    output: Digest,
    wire: Vec<u8>,
    token_count: usize,
}
#[derive(Debug, Clone)]
pub struct ExpansionDescriptor {
    pub key: ExpansionKey,
    pub output: Digest,
    pub wire_bytes: usize,
}
#[derive(Clone, Copy)]
pub(crate) struct ExpansionUsage {
    pub fingerprint: Digest,
    pub tokens: usize,
    pub wire_bytes: usize,
    pub procedural_epoch: Option<u64>,
}
impl ExpansionRecord {
    pub fn from_wire(key: ExpansionKey, wire: Vec<u8>) -> Result<Self, Diagnostic> {
        key.validate()?;
        let forest = TokenForest::decode(&wire).map_err(wire_error)?;
        let token_count = count_tokens(forest.roots(), 0)?;
        let output = Digest::of_bytes(&wire);
        Ok(Self {
            key,
            output,
            wire,
            token_count,
        })
    }
    pub fn from_tokens(key: ExpansionKey, tokens: &TokenForest) -> Result<Self, Diagnostic> {
        Self::from_wire(key, tokens.encode().map_err(wire_error)?)
    }
    pub(crate) fn try_copy(&self) -> Result<Self, Diagnostic> {
        let mut wire = Vec::new();
        wire.try_reserve_exact(self.wire.len()).map_err(|_| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "expansion publication wire allocation".into(),
            )
        })?;
        wire.extend_from_slice(&self.wire);
        Ok(Self {
            key: self.key.clone(),
            output: self.output,
            wire,
            token_count: self.token_count,
        })
    }
    pub fn key(&self) -> &ExpansionKey {
        &self.key
    }
    pub fn output(&self) -> Digest {
        self.output
    }
    pub fn tokens(&self) -> Result<TokenForest, Diagnostic> {
        TokenForest::decode(&self.wire).map_err(wire_error)
    }
    /// Count all owned token-tree nodes, including groups, from validated wire.
    pub fn token_count(&self) -> usize {
        self.token_count
    }
    pub(crate) fn usage(&self) -> ExpansionUsage {
        ExpansionUsage {
            fingerprint: self.descriptor().fingerprint(),
            tokens: self.token_count,
            wire_bytes: self.wire.len(),
            procedural_epoch: self.key.procedural_epoch,
        }
    }
    pub(crate) fn wire(&self) -> &[u8] {
        &self.wire
    }
    pub fn descriptor(&self) -> ExpansionDescriptor {
        ExpansionDescriptor {
            key: self.key.clone(),
            output: self.output,
            wire_bytes: self.wire.len(),
        }
    }
    pub(crate) fn same_record(&self, other: &Self) -> bool {
        self.key == other.key && self.output == other.output && self.wire == other.wire
    }
    pub(crate) fn estimated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.wire.capacity())
            .and_then(|n| n.checked_add(key_heap_bytes(&self.key)))
            .ok_or(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "expansion record size overflow".into(),
            ))
    }
}
fn count_tokens(
    nodes: &[crate::syntax::token_wire::TokenTree],
    depth: usize,
) -> Result<usize, Diagnostic> {
    let limit = || {
        Diagnostic::new(
            "LRR-E-EXPANSION-WIRE",
            "expansion token count or depth limit".into(),
        )
    };
    let mut count = 0usize;
    for node in nodes {
        count = count
            .checked_add(1)
            .ok_or_else(limit)?;
        if let crate::syntax::token_wire::TokenTree::Group { children, .. } = node {
            count = count
                .checked_add(count_tokens(children, depth + 1)?)
                .ok_or_else(limit)?;
        }
    }
    Ok(count)
}
pub(crate) fn key_heap_bytes(key: &ExpansionKey) -> usize {
    match &key.macro_identity {
        MacroIdentity::Procedural { export, .. } => export.capacity(),
        _ => 0,
    }
}
impl ExpansionDescriptor {
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + key_heap_bytes(&self.key)
    }
}
impl StableEncode for ExpansionDescriptor {
    const DOMAIN: &'static str = "lrr/v2/expansion-descriptor";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.output);
        e.u64(u64::try_from(self.wire_bytes).expect("bounded expansion wire size"))
    }
}

fn wire_error(error: crate::syntax::token_wire::WireError) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-WIRE", error.to_string())
}
