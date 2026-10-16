//! Allocation-only traversal of actual retained owning/local roots. No pointer
//! enters a semantic key or name decision. Parser/index/Outline and retained
//! InputValue roots stay in their existing generic weak-owner accounting.
use super::{Database, account_add};
use crate::{
    core::diagnostic::Diagnostic,
    db::{dependency::InputValue, value::SourceArtifactRef},
    frontend::{
        macro_boundary::RetainedMacroScope,
        source_body_context::{AdmittedBodyContext, PreparedBodyContext},
        source_locals::PreparedSourceLocalPlan,
        source_macros::{PublishedMacroIndex, SourceMacroIndex},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const ARC: usize = 2 * std::mem::size_of::<usize>();
const MAX_ROOTS: usize = 100_000;
#[derive(Default)]
struct Roots {
    seen: BTreeSet<usize>,
    bytes: usize,
}
impl Roots {
    fn add<T>(&mut self, root: &Arc<T>, bytes: usize) -> Result<bool, Diagnostic> {
        let id = Arc::as_ptr(root) as usize;
        if self.seen.contains(&id) {
            return Ok(false);
        }
        if self.seen.len() >= MAX_ROOTS {
            return Err(crate::runtime::arena::resource());
        }
        self.bytes = account_add(self.bytes, account_add(bytes, ARC)?)?;
        self.seen.insert(id);
        Ok(true)
    }
    fn map<K: Ord, V>(rows: &BTreeMap<K, V>) -> Result<usize, Diagnostic> {
        rows.len()
            .checked_mul(
                std::mem::size_of::<(K, V)>()
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .and_then(|n| n.checked_add(std::mem::size_of_val(rows)))
            .ok_or_else(crate::runtime::arena::resource)
    }
    fn scope(&mut self, root: &RetainedMacroScope) -> Result<(), Diagnostic> {
        self.add(root.scopes(), root.scopes().metadata_bytes()?)?;
        self.add(
            root.declarations(),
            account_add(
                root.declarations().metadata_bytes()?,
                std::mem::size_of_val(root.declarations().as_ref()),
            )?,
        )?;
        self.add(
            root.facts(),
            account_add(
                root.facts().metadata_bytes()?,
                std::mem::size_of_val(root.facts().as_ref()),
            )?,
        )?;
        self.add(
            root.ancestry(),
            account_add(
                root.ancestry().metadata_bytes()?,
                std::mem::size_of_val(root.ancestry().as_ref()),
            )?,
        )?;
        Ok(())
    }
    fn macro_index(&mut self, index: &Arc<SourceMacroIndex>) -> Result<(), Diagnostic> {
        if !self.add(index, index.exclusive_metadata_bytes()?)? {
            return Ok(());
        }
        let (scopes, declarations, facts, ancestry) = index.immutable_roots();
        self.add(scopes, scopes.metadata_bytes()?)?;
        self.add(
            declarations,
            account_add(
                declarations.metadata_bytes()?,
                std::mem::size_of_val(declarations.as_ref()),
            )?,
        )?;
        self.add(
            facts,
            account_add(
                facts.metadata_bytes()?,
                std::mem::size_of_val(facts.as_ref()),
            )?,
        )?;
        self.add(
            ancestry,
            account_add(
                ancestry.metadata_bytes()?,
                std::mem::size_of_val(ancestry.as_ref()),
            )?,
        )?;
        Ok(())
    }
    fn published(&mut self, index: &Arc<PublishedMacroIndex>) -> Result<(), Diagnostic> {
        if !self.add(index, index.exclusive_metadata_bytes()?)? {
            return Ok(());
        }
        self.macro_index(index.source_index_root())?;
        let outers = index.runtime_outer_root();
        if self.add(
            outers,
            std::mem::size_of_val(outers.as_ref())
                .checked_add(
                    outers
                        .capacity()
                        .checked_mul(std::mem::size_of::<
                            crate::frontend::source_macros::OwnedRuntimeOuter,
                        >())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?,
        )? {
            for outer in outers.iter() {
                self.add(outer.scopes(), outer.scopes().metadata_bytes()?)?;
                self.add(
                    outer.declarations(),
                    account_add(
                        outer.declarations().metadata_bytes()?,
                        std::mem::size_of_val(outer.declarations().as_ref()),
                    )?,
                )?;
            }
        }
        Ok(())
    }
    fn items(
        &mut self,
        items: &Arc<crate::expand::generated_items::GeneratedItems>,
    ) -> Result<(), Diagnostic> {
        let facts = items.hygiene_facts();
        let record = items.record_root();
        // The legacy nested estimate includes these shared allocations. Split
        // them here, so a boundary and a parser/input never charge them twice.
        let exclusive = items
            .metadata_bytes()?
            .checked_sub(facts.metadata_bytes()?)
            .and_then(|n| n.checked_sub(record.estimated_bytes().ok()?))
            .ok_or_else(crate::runtime::arena::resource)?;
        if self.add(items, exclusive)? {
            self.add(
                &facts,
                account_add(
                    facts.metadata_bytes()?,
                    std::mem::size_of_val(facts.as_ref()),
                )?,
            )?;
            self.add(record, record.estimated_bytes()?)?;
        }
        Ok(())
    }
    fn admitted(&mut self, source: &Arc<AdmittedBodyContext>) -> Result<(), Diagnostic> {
        if !self.add(source, source.exclusive_metadata_bytes())? {
            return Ok(());
        }
        // These exact handles were fetched by capture. Existing QueryPin and
        // input_pins retain/charge them even after their outer wrappers retire.
        if let Some(boundary) = source.generated_boundary() {
            if self.add(boundary, boundary.exclusive_metadata_bytes())? {
                if let Some(root) = boundary.selected_root() {
                    self.scope(root)?;
                }
                for root in boundary.scopes() {
                    self.scope(root)?;
                }
                self.items(boundary.output())?;
                for root in boundary.generated_roots() {
                    self.items(root)?;
                }
                let modules = boundary.module_roots();
                let bytes = modules.keys().try_fold(Self::map(modules)?, |n, key| {
                    account_add(n, key.heap_bytes())
                })?;
                self.add(modules, bytes)?;
            }
        }
        Ok(())
    }
    fn context(&mut self, context: &Arc<PreparedBodyContext>) -> Result<(), Diagnostic> {
        if !self.add(
            context,
            account_add(context.exclusive_metadata_bytes(), ARC)?,
        )? {
            return Ok(());
        }
        // exclusive_metadata_bytes includes its one owned facts allocation.
        self.admitted(context.source())?;
        for outer in context.macro_outers() {
            self.scope(outer)?;
        }
        if let Some(facts) = context.macro_hygiene() {
            self.add(
                facts,
                account_add(
                    facts.metadata_bytes()?,
                    std::mem::size_of_val(facts.as_ref()),
                )?,
            )?;
        }
        if let Some(ancestry) = context.macro_ancestry() {
            self.add(
                ancestry,
                account_add(
                    ancestry.metadata_bytes()?,
                    std::mem::size_of_val(ancestry.as_ref()),
                )?,
            )?;
        }
        Ok(())
    }
    fn environment(
        &mut self,
        environment: &Arc<crate::db::BodyEnvironmentInput>,
    ) -> Result<(), Diagnostic> {
        if !self.add(environment, environment.exclusive_metadata_bytes())? {
            return Ok(());
        }
        self.admitted(environment.source())?;
        let owner = environment.macros();
        // Selected receipt exclusive bytes already include its Arc control.
        if self.add(
            owner,
            owner
                .exclusive_metadata_bytes()
                .checked_sub(ARC)
                .ok_or_else(crate::runtime::arena::resource)?,
        )? {
            self.published(owner.index_root())?;
        }
        Ok(())
    }
    fn plan(&mut self, plan: &Arc<PreparedSourceLocalPlan>) -> Result<(), Diagnostic> {
        // SourceArtifactRef accounts the plan allocation itself.
        let allocation = Arc::as_ptr(plan) as usize;
        if self.seen.contains(&allocation) {
            return Ok(());
        }
        if self.seen.len() >= MAX_ROOTS {
            return Err(crate::runtime::arena::resource());
        }
        self.seen.insert(allocation);
        self.context(plan.context())?;
        if let Some(environment) = plan.environment() {
            self.environment(environment)?;
        }
        let roots = plan.allocation_roots();
        self.macro_index(roots.source_index())?;
        let outers = roots.runtime_outers();
        if self.add(
            outers,
            std::mem::size_of_val(outers.as_ref())
                .checked_add(
                    outers
                        .capacity()
                        .checked_mul(std::mem::size_of::<
                            crate::frontend::source_macros::OwnedRuntimeOuter,
                        >())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?,
        )? {
            for root in outers.iter() {
                self.add(root.scopes(), root.scopes().metadata_bytes()?)?;
                self.add(
                    root.declarations(),
                    account_add(
                        root.declarations().metadata_bytes()?,
                        std::mem::size_of_val(root.declarations().as_ref()),
                    )?,
                )?;
            }
        }
        self.add(roots.scopes(), roots.scopes().metadata_bytes()?)?;
        self.add(
            roots.declarations(),
            account_add(
                roots.declarations().metadata_bytes()?,
                std::mem::size_of_val(roots.declarations().as_ref()),
            )?,
        )?;
        self.add(
            roots.hygiene(),
            account_add(
                roots.hygiene().metadata_bytes()?,
                std::mem::size_of_val(roots.hygiene().as_ref()),
            )?,
        )?;
        self.add(
            roots.ancestry(),
            account_add(
                roots.ancestry().metadata_bytes()?,
                std::mem::size_of_val(roots.ancestry().as_ref()),
            )?,
        )?;
        self.add(roots.scope_keys(), Self::map(roots.scope_keys())?)?;
        let retired = roots.retired_producers();
        self.add(
            retired,
            retired
                .len()
                .checked_mul(16 * std::mem::size_of::<crate::core::digest::Digest>() + 256)
                .and_then(|n| n.checked_add(std::mem::size_of_val(retired.as_ref())))
                .ok_or_else(crate::runtime::arena::resource)?,
        )?;
        for producers in roots.producers() {
            self.add(producers, Self::map(producers)?)?;
        }
        if let Some(namespace) = roots.namespace() {
            self.add(namespace, namespace.metadata_bytes()?)?;
        }
        for scope in roots.retained_scopes() {
            self.scope(scope)?;
        }
        let tables = plan.tables_root();
        self.add(tables, tables.metadata_bytes()?)?;
        self.add(
            tables.hygiene_root(),
            account_add(
                tables.hygiene_root().metadata_bytes()?,
                std::mem::size_of_val(tables.hygiene_root().as_ref()),
            )?,
        )?;
        Ok(())
    }
}
impl Database {
    /// Same allocation traversal for budget preflight and live/retired usage.
    pub(super) fn body_environment_root_bytes(
        &self,
        extra: Option<&Arc<crate::db::BodyEnvironmentInput>>,
    ) -> Result<usize, Diagnostic> {
        self.collect_body_environment_roots(extra, None, false)
    }
    pub(super) fn pinned_body_environment_root_bytes(&self) -> Result<usize, Diagnostic> {
        self.collect_body_environment_roots(None, None, true)
    }
    pub(crate) fn preflight_source_local_plan(
        &self,
        plan: &Arc<PreparedSourceLocalPlan>,
    ) -> Result<(), Diagnostic> {
        let extra = account_add(
            self.additional_local_plan_bytes(plan)?,
            self.source_plan_pins
                .extra_bytes(SourceArtifactRef::LocalPlan(plan))?,
        )?;
        let collector = self.collectors.last().ok_or_else(|| {
            super::fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "local-plan preflight outside pure query",
            )
        })?;
        let dependencies = collector
            .dependencies
            .capacity()
            .checked_mul(std::mem::size_of::<crate::db::dependency::DependencyStamp>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let dependencies = collector
            .dependencies
            .iter()
            .try_fold(dependencies, |bytes, stamp| {
                account_add(bytes, stamp.heap_bytes())
            })?;
        let memo = account_add(
            std::mem::size_of::<super::MemoEntry>(),
            std::mem::size_of::<crate::db::key::QueryKey>() + 64,
        )?;
        let value = std::mem::size_of::<crate::db::value::QueryValue>();
        let query_pin = 64 + std::mem::size_of::<usize>() + std::mem::size_of::<super::QueryPin>();
        let extra = account_add(
            extra,
            account_add(
                dependencies,
                account_add(memo, account_add(value, query_pin)?)?,
            )?,
        )?;
        let usage = self.cache_usage()?;
        if account_add(usage.memo_bytes, extra)? > self.run_budgets.unwrap_or_default().query_bytes
        {
            return Err(super::fault(
                "LRR-E-METADATA-LIMIT",
                "owning local plan exceeds query metadata cap",
            ));
        }
        Ok(())
    }
    pub(super) fn additional_local_plan_bytes(
        &self,
        plan: &Arc<PreparedSourceLocalPlan>,
    ) -> Result<usize, Diagnostic> {
        self.collect_body_environment_roots(None, Some(plan), false)?
            .checked_sub(self.body_environment_root_bytes(None)?)
            .ok_or_else(crate::runtime::arena::resource)
    }
    fn collect_body_environment_roots(
        &self,
        extra: Option<&Arc<crate::db::BodyEnvironmentInput>>,
        extra_plan: Option<&Arc<PreparedSourceLocalPlan>>,
        pinned_only: bool,
    ) -> Result<usize, Diagnostic> {
        let mut roots = Roots::default();
        // ParsedFile already charges its retained pinned facts; generated source
        // inputs already charge their items. Seed actual handles, not digests.
        for pin in self.query_pins.values() {
            if let Some(parsed) = pin.parsed.upgrade() {
                if let Some(facts) = parsed.hygiene_facts() {
                    roots.seen.insert(Arc::as_ptr(facts) as usize);
                }
            }
        }
        for value in self
            .inputs
            .values()
            .map(|row| Arc::clone(&row.value))
            .chain(
                self.input_pins
                    .values()
                    .filter_map(std::sync::Weak::upgrade),
            )
        {
            if let InputValue::GeneratedSyntax(unit) = value.as_ref() {
                roots.seen.insert(Arc::as_ptr(&unit.items) as usize);
                roots
                    .seen
                    .insert(Arc::as_ptr(unit.items.record_root()) as usize);
                roots
                    .seen
                    .insert(Arc::as_ptr(&unit.items.hygiene_facts()) as usize);
            } else if let InputValue::RetainedSyntax(unit) = value.as_ref() {
                for output in unit.prepared().outputs() {
                    roots
                        .seen
                        .insert(Arc::as_ptr(&output.record_handle()) as usize);
                    if let Ok(facts) = output.hygiene_facts() {
                        roots.seen.insert(facts as *const _ as usize);
                    }
                }
            }
        }
        // A current or externally retained input owns these roots even if its
        // between-run retirement row has already been pruned.
        for value in self.inputs.values().map(|row| &row.value) {
            if let InputValue::BodyEnvironment(environment) = value.as_ref() {
                roots.environment(environment)?;
            }
        }
        for value in self
            .input_pins
            .values()
            .filter_map(std::sync::Weak::upgrade)
        {
            if let InputValue::BodyEnvironment(environment) = value.as_ref() {
                roots.environment(environment)?;
            }
        }
        for root in self.source_plan_pins.roots() {
            match root {
                SourceArtifactRef::LocalPlan(plan) => roots.plan(plan)?,
                SourceArtifactRef::BodyEnvironment(environment) => {
                    roots.environment(environment)?
                }
                _ => {}
            }
        }
        let (outer, inner) = self.memo_owners();
        for memo in self.memos.values() {
            if let crate::db::value::QueryValue::SourceLocalPlan(plan) = memo.value.as_ref() {
                // Cached-only descendants are reclaimable with their memo.
                // External inner/outer leases and retirement rows remain pinned.
                if !pinned_only || super::QueryPin::new(&memo.value).external(&outer, &inner) {
                    roots.plan(plan)?;
                }
            }
        }
        if let Some(extra) = extra {
            roots.environment(extra)?;
        }
        if let Some(plan) = extra_plan {
            roots.plan(plan)?;
        }
        Ok(roots.bytes)
    }
}
