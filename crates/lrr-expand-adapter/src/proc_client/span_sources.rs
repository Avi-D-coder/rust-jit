//! Immutable captured source data for external span callbacks; never reads host files.
use crate::{
    AdapterError,
    token_wire::{Key, SourceIdentity},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceDigestEncoding {
    RawBlake3,
    LrrSourceBytes,
}
#[derive(Clone)]
pub struct CapturedSpanSource {
    source: SourceIdentity,
    text: Arc<str>,
    encoding: SourceDigestEncoding,
    local_path: Option<String>,
}
impl CapturedSpanSource {
    pub fn new(
        source: SourceIdentity,
        text: String,
        encoding: SourceDigestEncoding,
        local_path: Option<String>,
    ) -> Result<Self, AdapterError> {
        if text.len() > 32 * 1024 * 1024
            || source.path.is_empty()
            || source.path.len() > 64 * 1024
            || source.path.contains('\0')
            || local_path.as_ref().is_some_and(|p| {
                p.len() > 64 * 1024 || p.contains('\0') || !std::path::Path::new(p).is_absolute()
            })
        {
            return Err(error("invalid or excessive captured span source"));
        }
        let hash = match encoding {
            SourceDigestEncoding::RawBlake3 => *blake3::hash(text.as_bytes()).as_bytes(),
            SourceDigestEncoding::LrrSourceBytes => {
                let domain = b"lrr/v2/source-bytes";
                let mut hash = blake3::Hasher::new();
                hash.update(&(domain.len() as u64).to_le_bytes());
                hash.update(domain);
                hash.update(&(text.len() as u64).to_le_bytes());
                hash.update(text.as_bytes());
                *hash.finalize().as_bytes()
            }
        };
        if source.content != Key(hash) {
            return Err(error("captured span source content identity mismatch"));
        }
        Ok(Self {
            source,
            text: text.into(),
            encoding,
            local_path,
        })
    }
    pub fn source(&self) -> &SourceIdentity {
        &self.source
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn local_path(&self) -> Option<&str> {
        self.local_path.as_deref()
    }
    pub fn encoding(&self) -> SourceDigestEncoding {
        self.encoding
    }
    /// Retained heap storage, including shared text allocation and owned path capacities.
    pub fn heap_bytes(&self) -> Result<usize, AdapterError> {
        self.text
            .len()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(self.source.path.capacity()))
            .and_then(|n| n.checked_add(self.local_path.as_ref().map_or(0, String::capacity)))
            .ok_or_else(|| error("captured source allocation overflow"))
    }
}
fn error(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_source_digest_encoding_and_local_path_are_checked() {
        let text = "hé";
        let source = SourceIdentity {
            unit: Key([1; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "logical.rs".into(),
        };
        assert!(
            CapturedSpanSource::new(
                source.clone(),
                text.into(),
                SourceDigestEncoding::RawBlake3,
                None
            )
            .is_ok()
        );
        for (bytes, encoding, path) in [
            ("changed", SourceDigestEncoding::RawBlake3, None),
            (text, SourceDigestEncoding::LrrSourceBytes, None),
            (
                text,
                SourceDigestEncoding::RawBlake3,
                Some("relative.rs".into()),
            ),
        ] {
            assert!(CapturedSpanSource::new(source.clone(), bytes.into(), encoding, path).is_err());
        }
        let mut hash = blake3::Hasher::new();
        let domain = b"lrr/v2/source-bytes";
        hash.update(&(domain.len() as u64).to_le_bytes());
        hash.update(domain);
        hash.update(&(text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
        let source = SourceIdentity {
            content: Key(*hash.finalize().as_bytes()),
            ..source
        };
        assert!(
            CapturedSpanSource::new(
                source,
                text.into(),
                SourceDigestEncoding::LrrSourceBytes,
                None
            )
            .is_ok()
        );
    }
}
