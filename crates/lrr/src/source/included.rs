//! Immutable coordinator-captured include data; no filesystem authority.
use super::{
    include_paths::{IncludeKind, IncludeLocation},
    snapshot::SourceBlob,
};
use crate::core::digest::Digest;
use std::path::PathBuf;
#[derive(Debug, Clone)]
pub struct CapturedInclude {
    pub(crate) request: Digest,
    pub(crate) identity: Digest,
    pub(crate) location: IncludeLocation,
    pub(crate) virtual_location: IncludeLocation,
    pub(crate) original: PathBuf,
    pub(crate) kind: IncludeKind,
    pub(crate) blob: SourceBlob,
}
impl CapturedInclude {
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.location.path.as_str().len())?
            .checked_add(self.virtual_location.path.as_str().len())?
            .checked_add(self.original.capacity())
    }
    pub fn request(&self) -> Digest {
        self.request
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn location(&self) -> &IncludeLocation {
        &self.location
    }
    /// Containing-file spelling used for lexical nested include resolution.
    pub fn virtual_location(&self) -> &IncludeLocation {
        &self.virtual_location
    }
    pub fn original_path(&self) -> &std::path::Path {
        &self.original
    }
    pub fn kind(&self) -> IncludeKind {
        self.kind
    }
    pub fn blob(&self) -> &SourceBlob {
        &self.blob
    }
}

/// The consuming lexical context, rather than a new module for the included file.
pub struct IncludeItemsContext {
    pub unit: Digest,
    pub crate_key: Digest,
    pub module_path: Vec<String>,
    pub cfg: super::snapshot::CfgSet,
    pub profile: crate::core::profile::ProfileIdentity,
}
pub struct IncludedItems {
    pub parsed: std::sync::Arc<crate::syntax::parse::ParsedFile>,
    pub outline: std::sync::Arc<crate::syntax::outline::Outline>,
    /// Retains the captured source publication and its origin/base-path facts.
    pub input: std::sync::Arc<crate::db::dependency::InputValue>,
}

/// An atomically captured and revalidated include universe for explicit preparation.
pub struct CapturedIncludeSet {
    pub(crate) identity: Digest,
    pub(crate) records: std::collections::BTreeMap<Digest, CapturedInclude>,
}
impl CapturedIncludeSet {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn records(&self) -> &std::collections::BTreeMap<Digest, CapturedInclude> {
        &self.records
    }
}
