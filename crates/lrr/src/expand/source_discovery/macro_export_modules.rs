//! Genuine adopted parent/file choices for actual configured role sources.
use super::*;
use crate::{
    db::{
        dependency::{DependencyStamp, InputKey},
        memo::Database,
    },
    source::macro_export_module::MacroExportModuleAdoption,
};
use std::sync::Arc;
impl SourceDiscovery {
    /// Bind the complete actual nonroot role-source adoption set before route
    /// construction. Every ancestor is an actual source in this discovery.
    pub fn with_macro_export_module_adoptions(
        mut self,
        db: &mut Database,
        modules: &[Arc<MacroExportModuleAdoption>],
    ) -> Result<Self, Diagnostic> {
        if self.started
            || self.failed.is_some()
            || self.seal.is_some()
            || self.declarative.is_some()
        {
            return Err(fault(
                "macro-export module adoption configuration is frozen or oversized",
            ));
        }
        let mut source_rows = BTreeMap::new();
        for ordinal in self.macro_export_sources.keys() {
            let source = self
                .sources
                .get(*ordinal)
                .ok_or_else(|| fault("macro-export role source owner is absent"))?;
            if source_rows
                .insert(source.selection().location_identity(), *ordinal)
                .is_some()
            {
                return Err(fault("macro-export module source is ambiguous"));
            }
        }
        let mut staged = BTreeMap::new();
        let mut additions: BTreeMap<InputKey, Digest> = BTreeMap::new();
        let mut bytes = 0usize;
        for module in modules {
            module.revalidate(db)?;
            let request = module.request();
            let ordinal = *source_rows
                .get(&request.child().source().selection().location_identity())
                .ok_or_else(|| fault("macro-export adopted child is not an actual role source"))?;
            let source = &self.sources[ordinal];
            if source.outline().root_scope.module_path.is_empty() {
                return Err(fault(
                    "macro-export configured root cannot acquire a module parent",
                ));
            }
            module.require_child(source, &self.macro_export_sources[&ordinal])?;
            if staged.insert(ordinal, Arc::clone(module)).is_some() {
                return Err(fault("macro-export adopted child is duplicated"));
            }
            let parent = *source_rows
                .get(&request.parent().source().selection().location_identity())
                .ok_or_else(|| fault("macro-export adopted parent is not an actual role source"))?;
            let actual = &self.sources[parent];
            self.macro_export_sources[&parent].require_facet(request.parent())?;
            if actual.selection() != request.parent().source().selection()
                || !actual.same_publication(request.parent().source())
            {
                return Err(fault("macro-export adopted parent prepared owner differs"));
            }
            bytes = bytes
                .checked_add(module.retained_admission_bytes())
                .and_then(|bytes| {
                    bytes.checked_add(128 + std::mem::size_of::<Arc<MacroExportModuleAdoption>>())
                })
                .ok_or_else(|| fault("macro-export adopted module graph size overflow"))?;
            for dependency in request
                .role_dependencies()
                .chain(std::iter::once(module.module_dependency()))
            {
                let DependencyStamp::Input { key, fingerprint } = dependency else {
                    return Err(fault("macro-export adopted module dependency kind differs"));
                };
                if self
                    .compiler_inputs
                    .get(key)
                    .is_some_and(|old| old != fingerprint)
                    || additions.get(key).is_some_and(|old| old != fingerprint)
                {
                    return Err(fault("macro-export adopted module preparation changed"));
                }
                if !self.compiler_inputs.contains_key(key) && !additions.contains_key(key) {
                    additions.insert(key.clone(), *fingerprint);
                }
            }
        }
        for (ordinal, role) in &self.macro_export_sources {
            let source = &self.sources[*ordinal];
            if !role.source().outline.module_path.is_empty() && !staged.contains_key(ordinal) {
                return Err(fault(
                    "macro-export child requires adopted source-parent selection",
                ));
            }
            if let Some(module) = staged.get(ordinal) {
                let parent = *source_rows
                    .get(
                        &module
                            .request()
                            .parent()
                            .source()
                            .selection()
                            .location_identity(),
                    )
                    .ok_or_else(|| fault("macro-export adopted ancestor source is absent"))?;
                if staged.get(&parent).map(|module| module.identity())
                    != module
                        .request()
                        .parent_adoption()
                        .map(|module| module.identity())
                {
                    return Err(fault(
                        "macro-export adopted ancestor chain differs from actual sources",
                    ));
                }
                if module.request().declaration().scope() != &source.outline().root_scope {
                    return Err(fault("macro-export adopted declaration scope differs"));
                }
            }
        }
        if !self.macro_export_modules.is_empty() {
            if self.macro_export_modules.len() != staged.len()
                || self.macro_export_modules.iter().any(|(ordinal, old)| {
                    staged
                        .get(ordinal)
                        .is_none_or(|new| new.identity() != old.identity())
                })
            {
                return Err(fault("macro-export adopted source set is already frozen"));
            }
            return Ok(self);
        }
        let rows = additions
            .len()
            .checked_mul(std::mem::size_of::<(InputKey, Digest)>() + 192)
            .and_then(|n| n.checked_add(source_rows.len().checked_mul(160)?))
            .ok_or_else(|| fault("macro-export adopted module metadata overflow"))?;
        self.retained_record_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(rows))
            .ok_or_else(|| fault("macro-export adopted module retained/transient limit"))?;
        self.retained_record_bytes = self
            .retained_record_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(rows))
            .ok_or_else(|| fault("macro-export adopted module metadata overflow"))?;
        self.compiler_inputs.extend(additions);
        self.macro_export_modules = staged;
        self.macro_export_module_bytes = bytes;
        Ok(self)
    }
    /// Conservative charge for the retained adoption graphs/map rows. Input
    /// stamp rows remain within this discovery's ordinary retained budget.
    pub fn macro_export_module_admission_bytes(&self) -> usize {
        self.macro_export_module_bytes
    }
    /// Actual owned lease allocations for graph accounting and publication.
    pub fn macro_export_module_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<MacroExportModuleAdoption>> {
        self.macro_export_modules.values()
    }
}
