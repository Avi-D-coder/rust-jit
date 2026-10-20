//! Memo-owned actual source/cfg settings for a scoped shared-IR context.
use super::{model::{FeatureSet, Symbol}, prepared::{self, PreparedProvider, Settings}};
use crate::{
    core::{diagnostic::Diagnostic, digest::{StableEncode, StableEncoder}},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::Database,
        poll::QueryPoll, provider::SyntaxProvider, seals::DiscoveryInput, value::QueryValue},
    source::changes::SourceSelection,
};
use std::sync::Arc;
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SolverConfigurationKey {
    pub(crate) source: crate::db::key::OutlineKey,
    pub(crate) path: crate::source::snapshot::SourcePath,
}
impl SolverConfigurationKey {
    pub(crate) fn from_source(source: &SourceSelection) -> Self {
        Self { source: source.outline.clone(), path: source.path.clone() }
    }
}
impl StableEncode for SolverConfigurationKey {
    const DOMAIN: &'static str = "lrr/v1/source-solver-configuration-key";
    fn encode_fields(&self, e: &mut StableEncoder) { e.string(self.path.as_str()); e.record(&self.source); }
}
#[derive(Debug)]
pub struct PreparedSolverConfiguration {
    source: SourceSelection,
    parsed: Arc<QueryValue>,
    cfg: Arc<InputValue>,
    settings: crate::syntax::solver_configuration::SolverCrateConfiguration,
}
/// The supported standalone semantic profile has no compiler-option override
/// channel. This receipt cannot be supplied by a caller as a guessed bool.
#[derive(Debug, Clone, Copy)]
pub(super) struct NoCompilerDefaultOverrides(());
impl NoCompilerDefaultOverrides {
    /// Exact pinned rustc_session/src/options.rs experimental_default_bounds=false.
    /// Available only after authoritative standalone profile admission.
    pub(super) fn experimental_default_bounds(self) -> bool { false }
}
impl PreparedSolverConfiguration {
    pub fn source(&self) -> &SourceSelection { &self.source }
    pub(crate) fn matches_key(&self, key: &SolverConfigurationKey) -> bool {
        self.source.outline == key.source && self.source.path == key.path
    }
    pub(crate) fn features(&self) -> &std::collections::BTreeSet<String> { &self.settings.features }
    pub(crate) fn recursion_limit(&self) -> usize { self.settings.recursion_limit }
    pub(super) fn rustc_no_implicit_bounds(&self) -> bool { self.settings.rustc_no_implicit_bounds }
    pub(super) fn default_bound_options(&self) -> Option<NoCompilerDefaultOverrides> {
        // Any future Cargo/compiler override profile needs its own producer.
        self.source.outline.parse.profile.validate_source().ok()
            .map(|()| NoCompilerDefaultOverrides(()))
    }
    pub(crate) fn prepare(db: &mut Database, key: &SolverConfigurationKey) -> QueryPoll<Self> {
        let source = SourceSelection { outline: key.source.clone(), path: key.path.clone() };
        if !source.outline.module_path.is_empty() {
            return QueryPoll::Ready(Err(fault("solver configuration needs the actual crate root")));
        }
        if !matches!(db.input(&InputKey::CapturedSource(source.location_identity())).as_deref(),
            Some(InputValue::CapturedSource(current)) if current == &source) {
            return QueryPoll::Ready(Err(fault("solver configuration source is not current")));
        }
        let Some(cfg) = db.input(&InputKey::CrateCfg(source.outline.crate_key)) else {
            return QueryPoll::Ready(Err(fault("solver configuration cfg absent")));
        };
        let InputValue::CrateCfg(configuration) = cfg.as_ref() else {
            return QueryPoll::Ready(Err(fault("solver configuration cfg variant differs")));
        };
        if configuration.fingerprint() != source.outline.cfg {
            return QueryPoll::Ready(Err(fault("solver configuration cfg changed")));
        }
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.outline.parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(tree) = parsed.as_ref() else {
            return QueryPoll::Ready(Err(fault("solver configuration parse variant differs")));
        };
        let settings = match tree.solver_crate_configuration(configuration) {
            Ok(settings) => settings, Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(Ok(Self { source: source.clone(), parsed, cfg, settings }))
    }
    pub(super) fn pointer_bytes(&self) -> Option<u8> {
        let InputValue::CrateCfg(cfg) = self.cfg.as_ref() else { return None; };
        let widths = cfg.values.get("target_pointer_width")?;
        if widths.len() != 1 { return None; }
        let bits: u16 = widths.first()?.parse().ok()?;
        (bits % 8 == 0).then(|| u8::try_from(bits / 8).ok()).flatten()
    }
    /// Representation construction is available; this is not ready source facts
    /// or an inference delegate. The catalog binds the actual selected root.
    pub(super) fn with_context<R>(&self, db: &mut Database, catalog_input: Arc<InputValue>,
        run: impl for<'cx> FnOnce(super::prepared::Cx<'cx>, PreparedProvider<'cx>) -> R,
    ) -> Result<R, Diagnostic> {
        self.with_source_context(db, catalog_input, None, run)
    }
    pub(super) fn with_source_context<R>(&self, db: &mut Database, catalog_input: Arc<InputValue>,
        generics: Option<&super::source_generics::SourceGenericLayouts>,
        run: impl for<'cx> FnOnce(super::prepared::Cx<'cx>, PreparedProvider<'cx>) -> R,
    ) -> Result<R, Diagnostic> {
        if generics.is_some_and(|generics| !generics.matches_catalog(&catalog_input)) {
            return Err(fault("constructor generic metadata belongs to another catalog"));
        }
        let InputValue::DiscoverySeal(catalog) = catalog_input.as_ref() else {
            return Err(fault("context catalog input variant differs"));
        };
        prepared::catalog_world(catalog)?;
        if !matches!(db.input(&InputKey::CapturedSource(self.source.location_identity())).as_deref(),
            Some(InputValue::CapturedSource(current)) if current == &self.source)
            || !matches!(db.input(&InputKey::CrateCfg(self.source.outline.crate_key)).as_deref(),
                Some(current) if current.fingerprint() == self.cfg.fingerprint())
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),
                Some(InputValue::ExpansionEpoch(epoch)) if *epoch == catalog.seal.key().epoch)
            || !db.input(&InputKey::DiscoverySeal(catalog.seal.key())).is_some_and(|current|
                Arc::ptr_eq(&current, &catalog_input)) {
            return Err(fault("source configuration/catalog is no longer current"));
        }

        if catalog.impl_catalog.as_ref().is_none_or(|proof| proof.root() != &self.source) {
            return Err(fault("solver configuration belongs to another catalog root"));
        }
        let enabled = self.settings.features.iter().map(|name| Symbol(name.as_str())).collect::<Vec<_>>();
        let settings = Settings {
            catalog, construction_generics: generics, recursion_limit: self.settings.recursion_limit,
            features: FeatureSet {
                generic_const_exprs: self.settings.features.contains("generic_const_exprs"),
                gca_const_items: self.settings.features.contains("gca_const_items"),
                coroutine_clone: self.settings.features.contains("coroutine_clone"), enabled: &enabled,
            },
            // Pinned rustc_session/options.rs:2401,2832; current standalone
            // catalogs have no -Z flag input. Cargo/override profiles remain work.
            assumptions_on_binders: false, renormalize_rigid_aliases: false,
            // Pinned infer/mod.rs:611 and rustc_session/options.rs:2473.
            // Current standalone profile admits no -Z override input.
            enable_next_solver_overflow_fcw: true, disable_trait_solver_fast_paths: false,
        };
        prepared::with_provider(&settings, self, run)
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
impl StableEncode for PreparedSolverConfiguration {
    const DOMAIN: &'static str = "lrr/v2/prepared-source-solver-configuration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.source.path.as_str()); e.record(&self.source.outline);
        e.u64(self.settings.recursion_limit as u64); e.bool(self.settings.rustc_no_implicit_bounds);
        e.sequence(&self.settings.features.iter().collect::<Vec<_>>(), |e, name| e.string(name));
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else { unreachable!("checked source parse") };
        e.record(&parsed.key()); e.digest(self.cfg.fingerprint());
    }
}
