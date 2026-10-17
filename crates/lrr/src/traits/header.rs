//! Borrowed source impl requirements. This is not an applicable-impl or Drop proof.
use crate::{
    core::digest::Digest,
    syntax::declaration_scan::{DeclarationScan, ScannedImpl},
};

/// An active inventory header joined to its actual independently retained scan.
/// Construction is private to the checked scope inventory preparation route.
#[derive(Clone, Copy)]
pub struct SourceImplHeader<'a> {
    source: &'a DeclarationScan,
    header: &'a ScannedImpl,
}
impl<'a> SourceImplHeader<'a> {
    pub(super) fn joined(source: &'a DeclarationScan, header: &'a ScannedImpl) -> Self {
        Self { source, header }
    }
    /// Source path, parse/profile/edition, cfg, origin and lexical scope facts.
    pub fn source(&self) -> &'a DeclarationScan {
        self.source
    }
    /// Raw impl/generic/where and associated-item headers, never method bodies.
    /// Further source-name and predicate preparation must consume these facts.
    pub fn requirements(&self) -> &'a ScannedImpl {
        self.header
    }
    pub fn identity(&self) -> Digest {
        self.header.identity
    }
    pub fn header_identity(&self) -> Digest {
        self.header.header_identity
    }
}
impl std::fmt::Debug for SourceImplHeader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceImplHeader")
            .field("identity", &self.identity())
            .field("header", &self.header_identity())
            .field("path", &self.source.path())
            .finish_non_exhaustive()
    }
}

pub(super) fn same_requirements(left: &ScannedImpl, right: &ScannedImpl) -> bool {
    left.identity == right.identity
        && left.header_identity == right.header_identity
        && left.range == right.range
        && left.header == right.header
        && left.scope == right.scope
        && left.containers == right.containers
        && left.associated.len() == right.associated.len()
        && left
            .associated
            .iter()
            .zip(&right.associated)
            .all(|(left, right)| left.range == right.range && left.header == right.header)
}
