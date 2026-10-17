//! Retained original physical module declarations. Source identity and actual
//! adopted parent records, rather than caller module paths, own every demand.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    },
    frontend::source_module_declarations::SourceModuleDeclaration,
    source::{include_paths::IncludeAuthorities, macro_export_module::MacroExportModuleAdoption},
};
use std::{path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OrdinaryModuleOwner {
    Original(usize),
    Staged(Digest),
}
pub(super) struct OriginalModuleDeclaration {
    pub(super) ordinal: usize,
    pub(super) declaration: SourceModuleDeclaration,
}
pub(super) fn capture_original_modules(
    sources: &[PreparedSourceView],
    retained: &mut usize,
) -> Result<BTreeMap<Digest, OriginalModuleDeclaration>, Diagnostic> {
    let mut rows = BTreeMap::new();
    for (ordinal, source) in sources.iter().enumerate() {
        for declaration in SourceModuleDeclaration::from_captured(source)? {
            *retained = retained
                .checked_add(declaration.retained_metadata_bytes()?)
                .and_then(|bytes| {
                    bytes.checked_add(160 + std::mem::size_of::<OriginalModuleDeclaration>())
                })
                .ok_or_else(|| fault("original module declaration metadata limit"))?;
            if rows
                .insert(
                    declaration.producer(),
                    OriginalModuleDeclaration {
                        ordinal,
                        declaration,
                    },
                )
                .is_some()
            {
                return Err(fault("original module producer identity/count conflict"));
            }
        }
    }
    Ok(rows)
}
impl SourceDiscovery {
    pub fn original_module_declaration(
        &self,
        producer: Digest,
    ) -> Result<&SourceModuleDeclaration, Diagnostic> {
        let row = self
            .original_modules
            .get(&producer)
            .ok_or_else(|| fault("original module declaration absent"))?;
        let source = self
            .sources
            .get(row.ordinal)
            .ok_or_else(|| fault("original module source owner absent"))?;
        if row.declaration.source().selection() != source.selection()
            || !row.declaration.source().same_publication(source)
        {
            return Err(fault("original module declaration source owner differs"));
        }
        Ok(&row.declaration)
    }
    /// Actual preceding adopted chain, if this original role source is itself
    /// a child. A nonroot role source without that lease is not a path authority.
    pub fn original_module_parent_adoption(
        &self,
        producer: Digest,
    ) -> Result<Option<&Arc<MacroExportModuleAdoption>>, Diagnostic> {
        let row = self
            .original_modules
            .get(&producer)
            .ok_or_else(|| fault("original module declaration absent"))?;
        let declaration = self.original_module_declaration(producer)?;
        if self.macro_export_sources.contains_key(&row.ordinal)
            && !declaration
                .source()
                .outline()
                .root_scope
                .module_path
                .is_empty()
        {
            return self
                .macro_export_modules
                .get(&row.ordinal)
                .map(Some)
                .ok_or_else(|| fault("original macro-export parent requires adopted ancestry"));
        }
        Ok(None)
    }
    pub(super) fn original_module_directory(&self, producer: Digest) -> Result<&Path, Diagnostic> {
        let declaration = self.original_module_declaration(producer)?;
        if let Some(parent) = self.original_module_parent_adoption(producer)? {
            return Ok(parent.capture().record().directory());
        }
        if !declaration
            .source()
            .outline()
            .root_scope
            .module_path
            .is_empty()
        {
            return Err(fault(
                "nonroot original module requires an adopted physical parent layout",
            ));
        }
        Path::new(declaration.source().selection().path.as_str())
            .parent()
            .ok_or_else(|| fault("original root module source has no physical directory"))
    }
    pub(super) fn require_original_module_source(
        &self,
        db: &mut Database,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        let row = self
            .original_modules
            .get(&producer)
            .ok_or_else(|| fault("original module declaration absent"))?;
        let source = self.original_module_declaration(producer)?.source();
        if let Some(expected) = self.macro_export_sources.get(&row.ordinal) {
            let current = db
                .macro_export_source_observation(source.selection(), self.key.unit_graph)?
                .ok_or_else(|| fault("original macro-export module parent role is unpublished"))?;
            current.require_source(source, self.key.unit_graph)?;
            if current.dependencies() != expected.dependencies() {
                return Err(fault("original macro-export module parent role changed"));
            }
            if let Some(parent) = self.original_module_parent_adoption(producer)? {
                parent.revalidate(db)?;
                parent.require_child(source, expected)?;
            }
        } else {
            let current = db
                .input(&InputKey::CapturedSource(
                    source.selection().location_identity(),
                ))
                .ok_or_else(|| fault("original module parent source publication absent"))?;
            if !matches!(current.as_ref(), InputValue::CapturedSource(selected) if selected == source.selection())
            {
                return Err(fault("original module parent source publication changed"));
            }
        }
        Ok(())
    }
    /// Pre-route preparation. This produces only the existing capture demand;
    /// no bytes are read, child source attached, ordinary inputs granted or
    /// missing import declared absent. The coordinator performs actual adoption.
    pub fn prepare_original_module_source(
        &self,
        db: &mut Database,
        producer: Digest,
        authorities: &IncludeAuthorities,
    ) -> QueryPoll<InputRequest> {
        let prepared = (|| {
            if self.failed.is_some() || self.seal.is_some() || !self.producers.contains(&producer) {
                return Err(fault(
                    "original module capture requires a current pending producer",
                ));
            }
            self.require_original_module_source(db, producer)?;
            let declaration = self.original_module_declaration(producer)?;
            Ok((declaration, self.original_module_directory(producer)?))
        })();
        let (declaration, directory) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let environment =
            match SyntaxProvider::compile_environment(db, declaration.scope().crate_key) {
                QueryPoll::Ready(Ok(environment)) => environment,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        QueryPoll::Ready((|| {
            if declaration.scope().crate_key == self.scope.crate_key
                && self
                    .declarative
                    .as_ref()
                    .is_some_and(|(expected, _, _)| *expected != environment.fingerprint())
            {
                return Err(fault("original module owning compile environment changed"));
            }
            if authorities.unit() != declaration.scope().crate_key {
                return Err(fault("original module authority belongs to another unit"));
            }
            let input =
                declaration.capture_request(authorities, directory, environment.fingerprint())?;
            let row = self
                .original_modules
                .get(&producer)
                .ok_or_else(|| fault("original module declaration disappeared"))?;
            if self.macro_export_sources.contains_key(&row.ordinal) {
                let current = db
                    .input(&InputKey::MacroExportSource(
                        declaration.source().selection().location_identity(),
                    ))
                    .ok_or_else(|| {
                        fault("original macro-export parent source receipt disappeared")
                    })?;
                let InputValue::MacroExportSource(receipt) = current.as_ref() else {
                    return Err(fault("original macro-export parent source receipt differs"));
                };
                let InputRequest::CaptureModule { request, .. } = &input else {
                    return Err(fault("original module request kind differs"));
                };
                if request
                    .candidates()
                    .iter()
                    .any(|candidate| candidate.generation() != receipt.preparation())
                {
                    return Err(fault(
                        "original macro-export module authority preparation differs",
                    ));
                }
            }
            Ok(input)
        })())
    }
}
