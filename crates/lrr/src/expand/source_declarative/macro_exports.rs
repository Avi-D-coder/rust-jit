//! Source-only MacroExport facets from explicit, configured preparation records.
//! These are candidate declaration routes, never a completed namespace seal or
//! permission to interpret the related native unit's runtime bodies.
use super::SourceDeclarative;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    frontend::source_macro_routes::SourceMacroRoutes,
    project::{cargo_units::UnitRole, tool_manifest::ToolManifest},
    source::changes::PreparedSourceView,
};
use std::sync::Arc;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-EXPORT-PREPARATION", message.into())
}

/// Opaque, source-backed declaration facet. Construction checks the actual
/// configured unit and recorded source; a unit ID or a macro spelling alone
/// cannot construct a facet. No executable runtime body is prepared here.
pub struct SourceMacroExportFacet {
    identity: Digest,
    preparation: Digest,
    unit: Digest,
    environment: Digest,
    source: PreparedSourceView,
    provider: Arc<SourceDeclarative>,
    routes: Arc<SourceMacroRoutes>,
    metadata_bytes: usize,
    exclusive_bytes: usize,
    provider_bytes: usize,
    route_bytes: usize,
    source_admission_bytes: usize,
    recorded_path: String,
    compiler_builtins: Vec<Digest>,
}
impl std::fmt::Debug for SourceMacroExportFacet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceMacroExportFacet")
            .field("identity", &self.identity)
            .field("unit", &self.unit)
            .field("selection", self.source.selection())
            .finish_non_exhaustive()
    }
}
impl SourceMacroExportFacet {
    /// Coordinator-only capture join; this function performs no filesystem IO,
    /// producer execution, runtime indexing or native-symbol inspection.
    pub(crate) fn capture(
        manifest: &ToolManifest,
        unit: Digest,
        recorded_path: &str,
        source: PreparedSourceView,
    ) -> Result<Self, Diagnostic> {
        manifest.validate()?;
        let configured = manifest
            .graph
            .units
            .get(&unit)
            .filter(|configured| configured.key.role == UnitRole::MacroExport)
            .ok_or_else(|| fault("configured MacroExport unit is absent"))?;
        let environment = manifest
            .compile_env
            .get(&unit)
            .ok_or_else(|| fault("configured MacroExport compile environment is absent"))?;
        let selection = source.selection();
        if configured.key.fingerprint() != unit
            || selection.outline.crate_key != unit
            || selection.outline.parse.profile.edition != configured.key.edition
            || selection.outline.parse.profile.semantics != manifest.semantics_profile
            || selection.outline.cfg != environment.cfg.fingerprint()
            || source.captured_macro_cfg()? != &environment.cfg
            || manifest.inputs.source_files.get(recorded_path)
                != Some(&Some(selection.outline.parse.content))
            || source.captured_macro_blob()?.content() != selection.outline.parse.content
        {
            return Err(fault(
                "macro-export source/unit/edition/configuration/record differs",
            ));
        }
        let package = environment
            .cargo
            .get("CARGO_MANIFEST_DIR")
            .ok_or_else(|| fault("macro-export package source root is absent"))?;
        if !std::path::Path::new(package).is_absolute()
            || std::path::Path::new(package).components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(fault(
                "macro-export configured package root is not normalized absolute",
            ));
        }
        let relative = std::path::Path::new(recorded_path)
            .strip_prefix(package)
            .map_err(|_| fault("macro-export source is outside its configured package"))?;
        if crate::source::snapshot::SourcePath::new(relative)? != selection.path {
            return Err(fault(
                "macro-export logical and recorded source paths differ",
            ));
        }
        source.require_original_hygiene()?;
        source.require_declaration_syntax()?;
        // This cold upper bound rejects generated/module-origin publications.
        // Nothing is registered in the live source/query registry for accounting.
        let source_admission_bytes = source.original_retained_admission_bytes(64 * 1024 * 1024)?;
        // Preserve exact configured dependency aliases. Missing imported roots
        // remain a preparation prerequisite, rather than implicit std/core names.
        let dependencies = source
            .source_dependencies()?
            .aliases_for(unit, configured.key.edition)?;
        if dependencies.len() != configured.dependencies.len()
            || configured.dependencies.iter().any(|edge| {
                crate::expand::scopes::MacroName::parse(&edge.alias, configured.key.edition)
                    .ok()
                    .and_then(|name| dependencies.get(&name).copied())
                    != Some(edge.target)
            })
        {
            return Err(fault(
                "macro-export configured dependency source roots differ",
            ));
        }
        let is_builtin = |item: &&crate::syntax::outline::ItemOutline| {
            item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                && item
                    .header
                    .attributes
                    .iter()
                    .any(|attribute| attribute.name == "rustc_builtin_macro")
        };
        let builtin_count = source.outline().items.iter().filter(is_builtin).count();
        builtin_count
            .checked_mul(std::mem::size_of::<Digest>())
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("macro-export compiler marker metadata limit"))?;
        let mut compiler_builtins = Vec::new();
        compiler_builtins
            .try_reserve_exact(builtin_count)
            .map_err(|_| fault("macro-export compiler marker allocation"))?;
        compiler_builtins.extend(
            source
                .outline()
                .items
                .iter()
                .filter(is_builtin)
                .map(|item| item.def_key.fingerprint()),
        );
        compiler_builtins.sort_unstable();
        let provider = Arc::new(SourceDeclarative::new(source.share())?);
        let routes = Arc::new(SourceMacroRoutes::from_completed_index(
            source.outline(),
            provider.macro_index(),
            &dependencies,
            &[],
        )?);
        let mut captured_path = String::new();
        captured_path
            .try_reserve_exact(recorded_path.len())
            .map_err(|_| fault("macro-export captured path allocation"))?;
        captured_path.push_str(recorded_path);
        // Pending producers/imports are retained by the ordinary route object;
        // this receipt does not claim that export discovery has completed.
        let exclusive_bytes = std::mem::size_of::<Self>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(captured_path.capacity()))
            .and_then(|n| {
                n.checked_add(
                    compiler_builtins
                        .capacity()
                        .checked_mul(std::mem::size_of::<Digest>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(2usize.checked_mul(source.selection().metadata_bytes().ok()?)?)
            })
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("macro-export retained metadata exceeds budget"))?;
        let provider_bytes = std::mem::size_of::<SourceDeclarative>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(provider.module_snapshot_bytes().ok()?))
            .and_then(|n| {
                n.checked_add(2usize.checked_mul(source.selection().metadata_bytes().ok()?)?)
            })
            .ok_or_else(|| fault("macro-export provider metadata overflow"))?;
        let route_bytes = routes
            .metadata_bytes()?
            .checked_add(2 * std::mem::size_of::<usize>())
            .ok_or_else(|| fault("macro-export route metadata overflow"))?;
        let metadata_bytes = exclusive_bytes
            .checked_add(provider_bytes)
            .and_then(|n| n.checked_add(route_bytes))
            .and_then(|n| n.checked_add(source_admission_bytes))
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("macro-export source/provider/routes admission limit"))?;
        let preparation = manifest.fingerprint();
        let mut identity = StableEncoder::new("lrr/v1/captured-source-macro-export-facet");
        identity.digest(preparation);
        identity.digest(unit);
        identity.digest(environment.fingerprint());
        identity.string(recorded_path);
        identity.string(selection.path.as_str());
        identity.record(&selection.outline);
        Ok(Self {
            identity: identity.finish(),
            preparation,
            unit,
            environment: environment.fingerprint(),
            source,
            provider,
            routes,
            metadata_bytes,
            exclusive_bytes,
            provider_bytes,
            route_bytes,
            source_admission_bytes,
            recorded_path: captured_path,
            compiler_builtins,
        })
    }
    /// The same ordinary hygienic source lookup and declarative request path.
    /// This is unsealed declaration preparation only; callers must adopt the
    /// returned expansion through normal discovery/assembly authority.
    pub fn declaration_demand(
        &self,
        consumer: &SourceDeclarative,
        invocation: Digest,
        compile_environment: Digest,
        engine: Digest,
    ) -> Result<super::SourceDeclarativeDemand, Diagnostic> {
        let owners = std::slice::from_ref(self.provider.as_ref());
        let mut routes = consumer.routes_in(owners)?;
        routes
            .try_reserve(1)
            .map_err(|_| fault("macro-export route selection allocation"))?;
        routes.push(Arc::clone(&self.routes));
        let namespace = SourceMacroRoutes::discovery_graph(&routes)?;
        let demand =
            consumer.demand_in(invocation, compile_environment, engine, &namespace, owners)?;
        if let super::SourceDeclarativeDemand::Ready(request) = &demand
            && let crate::expand::identity::MacroIdentity::Declaration { definition } =
                &request.key().macro_identity
            && self.compiler_builtins.binary_search(definition).is_ok()
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-EXPORT-BUILTIN",
                "selected source macro requires its pinned compiler-builtin adapter hook".into(),
            ));
        }
        Ok(demand)
    }
    /// Compiler markers are source facts, not permission to run their empty
    /// placeholder through the declarative matcher.
    pub fn requires_compiler_hook(&self, definition: Digest) -> bool {
        self.compiler_builtins.binary_search(&definition).is_ok()
    }
    pub fn recorded_path(&self) -> &str {
        &self.recorded_path
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn unit(&self) -> Digest {
        self.unit
    }
    pub fn environment(&self) -> Digest {
        self.environment
    }
    pub fn preparation(&self) -> Digest {
        self.preparation
    }
    pub fn source(&self) -> &PreparedSourceView {
        &self.source
    }
    pub fn provider(&self) -> &Arc<SourceDeclarative> {
        &self.provider
    }
    /// Candidate routes include ordinary pending producer/import obligations.
    /// They cannot certify absence, declaration completion or runtime access.
    pub fn routes(&self) -> &Arc<SourceMacroRoutes> {
        &self.routes
    }
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub fn provider_metadata_bytes(&self) -> usize {
        self.provider_bytes
    }
    pub fn routes_metadata_bytes(&self) -> usize {
        self.route_bytes
    }
    pub fn source_retained_admission_bytes(&self) -> usize {
        self.source_admission_bytes
    }
    /// Conservative admission accounting includes nested shared roots. A future
    /// publication pool may deduplicate the actual handles; it must not omit them.
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
}

#[cfg(test)]
#[path = "macro_exports_tests.rs"]
mod tests;
