//! Bound source-facet helper metadata. These records grant no macro execution.
pub use crate::expand::derive_receipt::PreparedDeriveDeclaration;
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    syntax::spans::SourceRange,
};
pub enum DeriveDeclarationPoll {
    Ready(PreparedDeriveDeclaration),
    NeedExpansion {
        producer: Digest,
        ranges: Vec<SourceRange>,
    },
    NeedSource {
        producer: Digest,
        export: String,
    },
}
/// Read only the immutable preparation's captured macro-host root, with a checked
/// allocation bound. This helper is called by the coordinator, never a provider.
pub(crate) fn captured_root(
    manifest: &crate::project::tool_manifest::ToolManifest,
    producer: Digest,
) -> Result<crate::source::snapshot::SourceBlob, Diagnostic> {
    if manifest
        .graph
        .units
        .get(&producer)
        .is_none_or(|u| u.key.role != crate::project::cargo_units::UnitRole::ProcMacroHost)
    {
        return Err(Diagnostic::new(
            "LRR-E-TOOL-PREPARE-STALE",
            "macro-host producer is absent".into(),
        ));
    }
    captured_source_root(manifest, producer)
}
pub(crate) fn captured_source_root(
    manifest: &crate::project::tool_manifest::ToolManifest,
    producer: Digest,
) -> Result<crate::source::snapshot::SourceBlob, Diagnostic> {
    use std::io::Read;
    let fail = || {
        Diagnostic::new(
            "LRR-E-TOOL-PREPARE-STALE",
            "macro-host source differs from captured preparation".into(),
        )
    };
    let unit = manifest
        .graph
        .units
        .get(&producer)
        .filter(|u| {
            matches!(
                u.key.role,
                crate::project::cargo_units::UnitRole::ProcMacroHost
                    | crate::project::cargo_units::UnitRole::Live
                    | crate::project::cargo_units::UnitRole::MacroExport
            )
        })
        .ok_or_else(fail)?;
    let expected = manifest
        .inputs
        .source_files
        .get(&unit.source_path)
        .and_then(|d| *d)
        .ok_or_else(fail)?;
    let mut file = std::fs::File::open(&unit.source_path).map_err(|_| fail())?;
    let size = usize::try_from(file.metadata().map_err(|_| fail())?.len()).map_err(|_| fail())?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|_| fail())?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|_| fail())?;
        if n == 0 {
            break;
        }
        bytes.len().checked_add(n).ok_or_else(fail)?;
        bytes.try_reserve(n).map_err(|_| fail())?;
        bytes.extend_from_slice(&buffer[..n]);
    }
    let blob = crate::source::snapshot::SourceBlob::new(bytes)?;
    if blob.content() != expected {
        return Err(fail());
    }
    Ok(blob)
}
