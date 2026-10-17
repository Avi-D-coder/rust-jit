//! Section17 closed synthetic module metadata, separate from macro authority.
//! No real std source/native symbols or executable API are loaded here.
use super::*;
use crate::{
    frontend::names::{NameDefinition, NameNamespaces},
    source::manifest::Namespace,
};
/// Immutable compiler/source facts for candidate import graph construction.
/// Fields stay private; only this catalog can mint the fixed injected import.
pub(crate) struct CompilerInjectedImport {
    consumer: Digest,
    alias: MacroName,
    provider: Digest,
    identity: Digest,
}
impl CompilerInjectedImport {
    pub(crate) fn parts(&self) -> (Digest, &MacroName, Digest) {
        (self.consumer, &self.alias, self.provider)
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
}
impl CompileInputCatalog {
    pub(crate) fn injected_import(
        &self,
        source: &crate::source::changes::PreparedSourceView,
    ) -> Result<Option<CompilerInjectedImport>, Diagnostic> {
        if self.facades.is_empty() || !source.selection().outline.module_path.is_empty() {
            return Ok(None);
        }
        if self.facades.len() != 2
            || self.facades.get("core") != Some(&self.unit)
            || self
                .facades
                .get("std")
                .is_none_or(|unit| *unit == self.unit)
            || self
                .facades
                .keys()
                .any(|name| !matches!(name.as_str(), "core" | "std"))
        {
            return Err(fault(
                "compiler-injected imports require exact private facade ownership",
            ));
        }
        let policy = source.macro_prelude_policy()?;
        if policy.disabled.iter().any(Vec::is_empty) {
            return Ok(None);
        }
        let name = if policy.standard { "std" } else { "core" };
        let provider = *self
            .facades
            .get(name)
            .ok_or_else(|| fault("selected compiler-injected facade absent"))?;
        let mut e = StableEncoder::new("lrr/v1/source-compiler-injected-import");
        e.record(&source.selection().outline);
        e.digest(source.selection().location_identity());
        e.digest(self.unit);
        e.u16(self.edition);
        e.string(name);
        e.digest(provider);
        Ok(Some(CompilerInjectedImport {
            consumer: source.selection().outline.crate_key,
            alias: MacroName::parse(name, source.outline().profile.edition)?,
            provider,
            identity: e.finish(),
        }))
    }
    pub(crate) fn install_injected_import(
        &self,
        graph: &mut crate::frontend::macro_names::MacroNamespaces,
        source: &crate::source::changes::PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        if let Some(import) = self.injected_import(source)? {
            let (consumer, alias, provider) = import.parts();
            graph.compiler_injected_import(consumer, alias.clone(), provider)?;
        }
        Ok(())
    }

    /// Only the actual immutable standard-facade catalog can disclose these
    /// explicit module entries. Ordinary custom compiler catalogs grant none.
    pub(crate) fn install_ordinary_module_facets(
        &self,
        graph: &mut NameNamespaces,
    ) -> Result<(), Diagnostic> {
        if self.facades.is_empty() {
            return Ok(());
        }
        if self.facades.len() != 2
            || self.facades.get("core") != Some(&self.unit)
            || self
                .facades
                .get("std")
                .is_none_or(|unit| *unit == self.unit)
            || self
                .facades
                .keys()
                .any(|name| !matches!(name.as_str(), "core" | "std"))
            || !crate::core::profile::is_supported_source_edition(self.edition)
        {
            return Err(fault(
                "ordinary module facets require exact private standard facade ownership",
            ));
        }
        for unit in self.facades.values() {
            // The LRR-provided facade sources declare `panic` themselves; the
            // compiler-owned entry only stands in for an absent source module.
            if graph
                .declared_module_access(
                    &crate::syntax::outline::ScopeKey {
                        crate_key: *unit,
                        module_path: vec!["panic".into()],
                        owner: None,
                    },
                    self.edition,
                )
                .is_ok()
            {
                continue;
            }
            let mut e = StableEncoder::new("lrr/v1/compiler-facade-panic-module");
            e.digest(*unit);
            e.u16(self.edition);
            e.string("panic");
            let definition = NameDefinition {
                namespace: Namespace::Type,
                identity: e.finish(),
                defining_unit: *unit,
                definition_edition: self.edition,
            };
            graph.declare_module(*unit, &[], self.edition, VisibilityScope::Public)?;
            // Existing admitted-module namespace operation retains Type identity,
            // module status, path, edition, access and alias routing. This is a
            // compiler-owned facade entry, not generated/live/native authority.
            graph.define_generated_module(
                NameNamespaces::path(*unit, &["panic"], self.edition)?,
                definition,
                VisibilityScope::Public,
            )?;
        }
        Ok(())
    }
}
