//! Borrow actual definition bodies from a separate sealed provider arena.
//! Caller scopes and source-parent ordinals never change arenas here.
use super::*;
use crate::expand::source_discovery::SealedMacroExportProviders;
use crate::frontend::generated_macros::GeneratedMacroCatalog;

fn arena_sources(exports: &SealedMacroExportProviders) -> impl Iterator<Item = &SourceDeclarative> {
    exports
        .providers()
        .iter()
        .chain(
            exports
                .modules()
                .values()
                .map(|module| module.provider().as_ref()),
        )
        .chain(
            exports
                .generated()
                .values()
                .filter_map(|frame| frame.source_prefix().map(AsRef::as_ref)),
        )
}
fn arena_catalogs(
    exports: &SealedMacroExportProviders,
) -> impl Iterator<Item = &GeneratedMacroCatalog> {
    // These are identity joins, never textual lookup. Child order is the actual
    // retained structural map; no guest ordinal or invented parent is involved.
    arena_sources(exports)
        .flat_map(|source| {
            source
                .catalogs
                .values()
                .chain(source.generated.values())
                .map(AsRef::as_ref)
        })
        .chain(
            exports
                .modules()
                .values()
                .flat_map(|module| module.catalogs().values().map(AsRef::as_ref)),
        )
        .chain(exports.generated().values().flat_map(|frame| {
            std::iter::once(frame.catalog().as_ref()).chain(frame.child_order().values().map(
                move |key| {
                    frame
                        .children()
                        .get(key)
                        .expect("sealed export child ordering preflight")
                        .catalog()
                        .as_ref()
                },
            ))
        }))
}
// Each parent is a genuine retained owner with its own ordinal arena. The
// private transfer/builder validates the ownership-chain bound; this borrowed
// iterator also caps work and cannot manufacture a missing owner or body.
fn arenas(
    exports: &SealedMacroExportProviders,
) -> impl Iterator<Item = &SealedMacroExportProviders> {
    std::iter::successors(Some(exports), |owner| owner.imports().map(AsRef::as_ref))
}
fn sources(exports: &SealedMacroExportProviders) -> impl Iterator<Item = &SourceDeclarative> {
    arenas(exports).flat_map(arena_sources)
}
fn catalogs(exports: &SealedMacroExportProviders) -> impl Iterator<Item = &GeneratedMacroCatalog> {
    arenas(exports).flat_map(arena_catalogs)
}
impl<'a> CapturedMacroContext<'a> {
    pub(super) fn require_provider(self, provider: &SourceDeclarative) -> Result<(), Diagnostic> {
        if !std::ptr::eq(self.provider, provider) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "request caller provider/context differs",
            ));
        }
        Ok(())
    }
    pub(super) fn exported_definition_owner(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
        remaining: &mut usize,
    ) -> Result<Option<&'a SourceDeclarative>, Diagnostic> {
        let Some(exports) = self.exports else {
            return Ok(None);
        };
        for owner in sources(exports) {
            if let Some(owner) = owner.retained_definition_owner(definition, 0, remaining)? {
                return Ok(Some(owner));
            }
        }
        for catalog in catalogs(exports) {
            if let Some(owner) = catalog.captured_definition_owner(definition)
                && let Some(owner) = owner.retained_definition_owner(definition, 0, remaining)?
            {
                return Ok(Some(owner));
            }
        }
        Ok(None)
    }
    pub(super) fn exported_definition_body(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
    ) -> Option<std::sync::Arc<super::super::generated_items::GeneratedItems>> {
        self.exports.and_then(|exports| {
            catalogs(exports).find_map(|catalog| catalog.module_definition_body(definition))
        })
    }
    pub(super) fn exported_definition_origin(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
        ancestry: &mut crate::frontend::macro_origins::ExpansionAncestry,
    ) -> Result<Option<Option<crate::syntax::token_wire::Key>>, Diagnostic> {
        let Some(exports) = self.exports else {
            return Ok(None);
        };
        let mut origin = None;
        let mut join = |found| -> Result<(), Diagnostic> {
            if origin.is_some_and(|old| old != found) {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "sealed imported definition origins differ",
                ));
            }
            origin = Some(found);
            Ok(())
        };
        let mut remaining = usize::MAX;
        if let Some(owner) = self.exported_definition_owner(definition, &mut remaining)?
            && let Some(found) = owner.index.definition_origin(definition)?
        {
            join(found)?;
            ancestry.extend(owner.index.expansion_ancestry())?;
        }
        for catalog in catalogs(exports) {
            if let Some(found) = catalog.definition_origin(definition)? {
                join(found)?;
                ancestry.extend(catalog.ancestry())?;
            }
        }
        Ok(origin)
    }
}
