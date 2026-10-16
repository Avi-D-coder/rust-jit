//! Borrowed compiler proof replay facts, not publication or name authority.
//! The coordinator supplies the actual retained source and independently owned
//! scan from its preparation plan after checking currentness and full closure.
use super::{
    changes::{PreparedSourceView, SourceSelection},
    snapshot::{CfgSet, SourceBlob},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    syntax::{declaration_scan::DeclarationScan, parse::ParsedFile},
};
use std::sync::Arc;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILER-SOURCE-REPLAY", message.into())
}
/// Immutable syntax/config data borrowed from actual preparation owners. This
/// type cannot certify currentness, namespace completeness or runtime access.
pub(crate) struct CompilerReplaySource<'a> {
    source: &'a PreparedSourceView,
    parser: &'a Arc<ParsedFile>,
    scan: &'a Arc<DeclarationScan>,
    blob: &'a SourceBlob,
    cfg: &'a CfgSet,
    scan_identity: Digest,
}
impl<'a> CompilerReplaySource<'a> {
    /// Called with the separately retained scan of the actual source plan,
    /// never the proof's stored scan as its own independent replay witness.
    pub(crate) fn prepare(
        source: &'a PreparedSourceView,
        scan: &'a Arc<DeclarationScan>,
    ) -> Result<Self, Diagnostic> {
        let selection = source.selection();
        let parser = source.retained_declaration_parser()?;
        let blob = source.captured_macro_blob()?;
        let cfg = source.captured_macro_cfg()?;
        let outline = source.outline();
        if selection.canonical()? != *selection
            || blob.content() != selection.outline.parse.content
            || parser.content != selection.outline.parse.content
            || parser.profile != selection.outline.parse.profile
            || parser.edition != selection.outline.parse.profile.edition
            || outline.profile != selection.outline.parse.profile
            || outline.content != selection.outline.parse.content
            || outline.cfg != selection.outline.cfg
            || cfg.fingerprint() != selection.outline.cfg
            || outline.root_scope.crate_key != selection.outline.crate_key
            || outline.root_scope.module_path != selection.outline.module_path
            || scan.parse() != &selection.outline.parse
            || scan.path() != &selection.path
            || scan.cfg() != selection.outline.cfg
            || scan.scope() != &outline.root_scope
            || scan.generated_origin() != parser.generated_origin
        {
            return Err(fault("compiler replay source/parser/scan identity differs"));
        }
        Ok(Self {
            source,
            parser,
            scan,
            blob,
            cfg,
            scan_identity: scan.fingerprint(),
        })
    }
    pub(crate) fn selection(&self) -> &SourceSelection {
        self.source.selection()
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    pub(crate) fn parser(&self) -> &Arc<ParsedFile> {
        self.parser
    }
    pub(crate) fn scan(&self) -> &Arc<DeclarationScan> {
        self.scan
    }
    pub(crate) fn scan_identity(&self) -> Digest {
        self.scan_identity
    }
    pub(crate) fn blob(&self) -> &SourceBlob {
        self.blob
    }
    pub(crate) fn cfg(&self) -> &CfgSet {
        self.cfg
    }
    /// Only fixed borrowed view storage; actual parser/source/scan allocations
    /// remain charged to their preparation owners.
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
#[path = "compiler_replay_tests.rs"]
mod tests;
