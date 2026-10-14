//! Synthetic compiler macro facets for original standalone source execution.
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts, macro_names::MacroNamespaces,
        runtime_macro_catalog::RuntimeMacroCatalog, source_macro_routes::SourceMacroRoutes,
        source_macros::PublishedMacroIndex,
    },
    source::changes::PreparedSourceView,
};
use std::collections::BTreeMap;
fn source_root(
    unit: crate::core::digest::Digest,
    edition: u16,
) -> Result<(crate::syntax::token_wire::Key, HygieneFacts), Diagnostic> {
    let source = crate::expand::source_hygiene::source_root(unit, edition)?;
    Ok((
        source.context,
        HygieneFacts::new(source.contexts, source.rows)?.with_name_contexts(source.names)?,
    ))
}
pub struct StandaloneRuntimeMacros {
    index: PublishedMacroIndex,
    namespaces: MacroNamespaces,
    catalog: RuntimeMacroCatalog,
    exports: Vec<SourceMacroRoutes>,
}
impl StandaloneRuntimeMacros {
    pub fn new(source: &PreparedSourceView) -> Result<Self, Diagnostic> {
        source.require_declaration_syntax()?;
        let profile = &source.selection().outline.parse.profile;
        if !source.selection().outline.module_path.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "standalone macro facets require their crate root source".into(),
            ));
        }
        let core = crate::facade::unit(profile, "core");
        let standard = crate::facade::unit(profile, "std");
        let policy = source.macro_prelude_policy()?;
        let root_disabled = policy.disabled.iter().any(Vec::is_empty);
        let mut dependencies = BTreeMap::new();
        if !root_disabled {
            dependencies.insert(MacroName::parse("core", profile.edition)?, core);
            if policy.standard {
                dependencies.insert(MacroName::parse("std", profile.edition)?, standard);
            }
        }
        let available = BTreeMap::from([
            (MacroName::parse("core", profile.edition)?, core),
            (MacroName::parse("std", profile.edition)?, standard),
        ]);
        let disabled = policy
            .disabled
            .iter()
            .map(|path| crate::syntax::outline::ScopeKey {
                crate_key: source.selection().outline.crate_key,
                module_path: path.clone(),
                owner: None,
            })
            .collect::<Vec<_>>();
        let (context, facts) = source_root(source.selection().outline.crate_key, profile.edition)?;
        let routes = SourceMacroRoutes::from_outline_scoped_available(
            source.outline(),
            context,
            facts,
            &dependencies,
            &available,
            &disabled,
        )?;
        let catalog = RuntimeMacroCatalog::standard(profile)?;
        let compiler =
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(profile)?;
        let exports = vec![
            compiler.source_routes()?,
            compiler.source_exports(standard)?,
        ];
        let all = vec![
            routes,
            compiler.source_routes()?,
            compiler.source_exports(standard)?,
        ];
        let mut namespaces = SourceMacroRoutes::candidate_graph(&all)?;
        for (name, definition) in compiler.compiler_globals()? {
            namespaces.compiler_global(source.selection().outline.crate_key, name, definition)?;
        }
        for scope in source
            .outline()
            .scopes
            .keys()
            .filter(|scope| scope.owner.is_none())
        {
            namespaces.set_prelude(scope.clone(), if policy.standard { standard } else { core })?;
        }
        for path in policy.disabled {
            namespaces.disable_implicit(crate::syntax::outline::ScopeKey {
                crate_key: source.selection().outline.crate_key,
                module_path: path,
                owner: None,
            })?;
        }
        let (index_context, facts) =
            source_root(source.selection().outline.crate_key, profile.edition)?;
        if index_context != context {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "pinned source root context changed".into(),
            ));
        }
        let index = PublishedMacroIndex::from_view(source.share(), context, facts)?;
        Ok(Self {
            index,
            namespaces,
            catalog,
            exports,
        })
    }
    pub fn context<'a>(
        &'a self,
        source: &'a PreparedSourceView,
    ) -> Result<super::runtime_macros::RuntimeMacroContext<'a>, Diagnostic> {
        Ok(super::runtime_macros::RuntimeMacroContext::new(
            source,
            &self.index,
            &self.namespaces,
            &self.catalog,
        )?
        .with_compiler_exports(&self.exports))
    }
}

/// Owned sealed source indexes; borrowed dispatch contexts convey no macro authority.
pub struct CompletedStandaloneRuntimeMacros {
    sources: Vec<(PreparedSourceView, PublishedMacroIndex)>,
    namespaces: MacroNamespaces,
    catalog: RuntimeMacroCatalog,
    exports: Vec<SourceMacroRoutes>,
}
impl CompletedStandaloneRuntimeMacros {
    pub fn from_discovery(
        root: &PreparedSourceView,
        discovery: &crate::expand::source_discovery::SourceDiscovery,
    ) -> Result<Self, Diagnostic> {
        root.require_declaration_syntax()?;
        if !root.selection().outline.module_path.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "completed runtime macro contexts require their crate root".into(),
            ));
        }
        let sources = discovery.completed_runtime_macro_indexes()?;
        if !discovery.contains_original_source(root.selection()) {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "completed runtime macro root is absent from sealed sources".into(),
            ));
        }
        // Discovery already captured ordinary extern-crate aliases and their
        // exact source worlds. Rebuilding standalone-only routes would erase
        // that closure. Only the standard compiler facets are reconstructed.
        let profile = &root.selection().outline.parse.profile;
        let compiler =
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(profile)?;
        let standard = crate::facade::unit(profile, "std");
        Ok(Self {
            sources,
            namespaces: discovery.completed_runtime_macro_namespace()?,
            catalog: RuntimeMacroCatalog::standard(profile)?,
            exports: vec![
                compiler.source_routes()?,
                compiler.source_exports(standard)?,
            ],
        })
    }
    pub fn contexts(
        &self,
    ) -> Result<Vec<super::runtime_macros::RuntimeMacroContext<'_>>, Diagnostic> {
        let mut result = Vec::new();
        result.try_reserve_exact(self.sources.len()).map_err(|_| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "runtime macro dispatch context allocation".into(),
            )
        })?;
        for (source, index) in &self.sources {
            result.push(
                super::runtime_macros::RuntimeMacroContext::new(
                    source,
                    index,
                    &self.namespaces,
                    &self.catalog,
                )?
                .with_compiler_exports(&self.exports),
            );
        }
        Ok(result)
    }
}
