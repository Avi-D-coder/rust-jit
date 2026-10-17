//! Generated header facts for the existing namespace index, with pinned hygiene.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::generated_items::GeneratedItems,
    frontend::names::NameNamespaces,
    source::{manifest::ItemKind, snapshot::CfgSet},
    syntax::{outline::ScopeKey, parse::ParsedFile, token_wire::Key},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[path = "declared_item_names.rs"]
pub(crate) mod declared_names;
use declared_names::DeclaredItemName;
pub struct GeneratedHeaderPlan {
    scope: ScopeKey,
    routes: Vec<GeneratedRoute>,
    blocked: BTreeSet<Digest>,
    impl_frontier: Option<Digest>,
    impl_ranges: Vec<crate::syntax::spans::SourceRange>,
    cfg: Digest,
    epoch: u64,
    attributes: BTreeSet<Digest>,
    guard: Digest,
    imports: Vec<crate::syntax::imports::SourceImport>,
    edition: u16,
    module_sources: Vec<GeneratedModuleSource>,
}
/// Frozen declaration demand. Only a later explicit source capture may discharge it.
#[derive(Debug, Clone)]
pub struct GeneratedModuleSource {
    producer: Digest,
    header: crate::expand::identity::GeneratedItemKey,
    scope: ScopeKey,
    request: crate::syntax::modules::ModuleRequest,
    site: Key,
}
impl GeneratedModuleSource {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.request
            .module_path
            .iter()
            .chain(&self.request.directory)
            .try_fold(0usize, |n, s| {
                n.checked_add(s.capacity()).and_then(|n| n.checked_add(64))
            })
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .and_then(|n| n.checked_add(self.request.name.capacity()))
            .and_then(|n| {
                n.checked_add(
                    self.request
                        .path_override
                        .as_ref()
                        .map_or(0, String::capacity),
                )
            })
            .and_then(|n| n.checked_add(self.header.structural_path.capacity().checked_mul(4)?))
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "module source metadata overflow"))
    }
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn header(&self) -> &crate::expand::identity::GeneratedItemKey {
        &self.header
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn request(&self) -> &crate::syntax::modules::ModuleRequest {
        &self.request
    }
    pub fn site(&self) -> Key {
        self.site
    }
}
struct GeneratedRoute {
    declaration: Arc<DeclaredItemName>,
    module: bool,
}
fn contained_scope(scope: &ScopeKey, owner: &ScopeKey) -> bool {
    scope.owner.is_none()
        && scope.crate_key == owner.crate_key
        && scope.module_path.starts_with(&owner.module_path)
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
impl GeneratedHeaderPlan {
    pub fn prepare(
        items: &GeneratedItems,
        scope: ScopeKey,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_owned(items, scope, owner_context, owner_context, cfg, epoch)
    }
    /// The caller scope comes from its captured parent fragment, never from a hygiene mark.
    pub(crate) fn prepare_owned(
        items: &GeneratedItems,
        scope: ScopeKey,
        call_context: Key,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_using(
            items,
            scope,
            call_context,
            owner_context,
            cfg,
            epoch,
            None,
            &BTreeSet::new(),
        )
    }
    pub(crate) fn prepare_with_captured_root(
        items: &crate::expand::generated_items::GeneratedItems,
        scope: ScopeKey,
        call_context: Key,
        root: &crate::syntax::token_wire::ContextRecord,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_using(
            items,
            scope,
            call_context,
            root.key(),
            cfg,
            epoch,
            Some(root),
            &BTreeSet::new(),
        )
    }
    pub(crate) fn prepare_catalog(
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<Self, Diagnostic> {
        let (items, scope, root) = catalog.captured_owner();
        Self::prepare_using(
            items,
            scope.clone(),
            Key(items.record().key().parent_context.0),
            root.key(),
            catalog.cfg(),
            items.epoch(),
            Some(root),
            catalog.settled_derive_items(),
        )
    }
    pub(crate) fn settled_originals(
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<Self, Diagnostic> {
        let mut plan = Self::prepare_catalog(catalog)?;
        plan.routes.retain(|r| {
            catalog
                .settled_derive_items()
                .contains(&r.declaration.definition().identity)
        });
        plan.blocked.clear();
        plan.impl_frontier = None;
        plan.impl_ranges.clear();
        plan.attributes.clear();
        plan.imports.clear();
        plan.module_sources.clear();
        Ok(plan)
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_using(
        items: &GeneratedItems,
        scope: ScopeKey,
        call_context: Key,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
        captured_root: Option<&crate::syntax::token_wire::ContextRecord>,
        settled_derives: &BTreeSet<Digest>,
    ) -> Result<Self, Diagnostic> {
        if epoch != items.epoch() {
            return Err(fault(
                "LRR-E-EXPANSION-EPOCH",
                "generated header publication epoch differs",
            ));
        }
        if scope.owner.is_some() || items.record().key().parent_context != Digest(call_context.0) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated header parent scope/context differs",
            ));
        }
        let contexts = items.tokens().contexts();
        let owner = captured_root
            .or_else(|| contexts.iter().find(|c| c.key() == owner_context))
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated header owner context absent",
                )
            })?;
        if owner.parent.is_some() || owner.defining_unit.0 != scope.crate_key.0 {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated header owner is not caller source root",
            ));
        }
        let mut cursor = call_context;
        let mut steps = 0usize;
        while captured_root.is_none() && cursor != owner_context {
            steps = steps
                .checked_add(1)
                .filter(|n| *n <= contexts.len())
                .ok_or_else(|| {
                    fault(
                        "LRR-E-EXPANSION-INPUT",
                        "generated call context ancestry cycles",
                    )
                })?;
            cursor = contexts
                .iter()
                .find(|c| c.key() == cursor)
                .and_then(|c| c.parent)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-EXPANSION-INPUT",
                        "generated call context does not reach captured caller root",
                    )
                })?;
        }
        let edition = items.record().key().call_edition;
        let parsed = ParsedFile::from_generated_items(items)?;
        let source_parse = parsed.key();
        let outline = parsed.outline(scope.crate_key, scope.module_path.clone(), cfg)?;
        let owner_class = items.item_class(if captured_root.is_some() {
            call_context
        } else {
            owner_context
        })?;
        let declaration_hygiene = items.hygiene_facts();
        let spans = items
            .tokens()
            .spans()
            .iter()
            .map(|s| (s.key().0, s))
            .collect::<BTreeMap<_, _>>();
        let headers = items
            .items()
            .iter()
            .map(|h| ((h.start, h.end), h))
            .collect::<BTreeMap<_, _>>();
        let settled = |start: u32, end: u32| {
            headers
                .get(&(start, end))
                .is_some_and(|header| settled_derives.contains(&header.key.fingerprint()))
        };
        let retired = outline
            .items
            .iter()
            .filter(|item| settled(item.declaration_span.start, item.declaration_span.end))
            .filter_map(crate::syntax::outline::potential_attribute_producer)
            .map(|call| items.frontier_identity(call.frontier))
            .collect::<BTreeSet<_>>();
        let definitions = outline
            .items
            .iter()
            .map(|i| (i.def_key.fingerprint(), i))
            .collect::<BTreeMap<_, _>>();
        let attributes = outline
            .items
            .iter()
            .filter_map(crate::syntax::outline::potential_attribute_producer)
            .map(|call| items.frontier_identity(call.frontier))
            .filter(|id| !retired.contains(id))
            .collect::<BTreeSet<_>>();
        let mut blocked = BTreeSet::new();
        blocked.extend(attributes.iter().copied());
        // Impl inventory completion cannot discharge unsupported imports or
        // other owner scopes which also keep this expansion incomplete.
        let impl_scope_bytes = outline
            .items
            .len()
            .checked_mul(128)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "impl scope index limit"))?;
        let impl_scopes = outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == ItemKind::Impl)
            .map(|item| (item.scope.module_path.as_slice(), item.header.fingerprint()))
            .collect::<BTreeSet<_>>();
        let impl_frontier = (!impl_scopes.is_empty()).then(|| {
            let mut e = crate::core::digest::StableEncoder::new("lrr/v1/generated-impl-frontier");
            e.digest(items.record().descriptor().fingerprint());
            e.record(&scope);
            e.digest(cfg.fingerprint());
            e.u64(epoch);
            e.finish()
        });
        let mut impl_ranges = Vec::new();
        impl_ranges
            .try_reserve_exact(
                outline
                    .items
                    .iter()
                    .filter(|item| item.def_key.item_kind == ItemKind::Impl)
                    .count(),
            )
            .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "impl range allocation"))?;
        impl_ranges.extend(
            outline
                .items
                .iter()
                .filter(|item| item.def_key.item_kind == ItemKind::Impl)
                .map(|item| item.declaration_span),
        );
        impl_ranges.sort_by_key(|range| (range.start, range.end));
        let impl_owned_scope = |candidate: &ScopeKey| {
            candidate.crate_key == scope.crate_key
                && candidate.owner.is_some_and(|owner| {
                    impl_scopes.contains(&(candidate.module_path.as_slice(), owner))
                })
        };
        let mut routes = Vec::new();
        let mut bytes = impl_scope_bytes;
        for members in outline.scopes.values() {
            for b in &members.blockers {
                let id = items.frontier_identity(b.syntax);
                if !retired.contains(&id) {
                    blocked.insert(id);
                }
            }
        }
        let mut module_items = BTreeMap::new();
        for item in outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == ItemKind::Module)
        {
            bytes = bytes
                .checked_add(item.scope.heap_bytes())
                .and_then(|n| n.checked_add(item.def_key.declared_name.capacity()))
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated module index metadata limit",
                    )
                })?;
            module_items
                .entry((
                    item.scope.module_path.clone(),
                    item.def_key.declared_name.clone(),
                ))
                .and_modify(|candidate| *candidate = None)
                .or_insert(Some(item));
        }
        let mut module_sources = Vec::new();
        // Use the pinned owned syntax tree, never a text round-trip or filesystem read.
        for request in
            crate::syntax::modules::external_modules_from_tree(&parsed.parsed.tree(), cfg)?
        {
            let mut child = scope.clone();
            child
                .module_path
                .extend(request.module_path.iter().cloned());
            if child.module_path.len() > 256 {
                return Err(fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated module depth limit",
                ));
            }
            let parent_path = &child.module_path[..child.module_path.len() - 1];
            let item = module_items
                .get(&(parent_path.to_vec(), request.name.clone()))
                .and_then(|candidate| *candidate)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-CATALOG-INCOMPLETE",
                        "generated module declaration is absent or ambiguous",
                    )
                })?;
            // A replacing attribute owns the declaration until its output is adopted.
            // Its original external module must not request source before replacement.
            if crate::syntax::outline::potential_attribute_producer(item).is_some()
                && !settled(item.declaration_span.start, item.declaration_span.end)
            {
                continue;
            }
            let header = headers
                .get(&(item.declaration_span.start, item.declaration_span.end))
                .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated module header absent"))?;
            let origin = items
                .origins()
                .iter()
                .find(|origin| origin.end > header.header_start && origin.start < header.header_end)
                .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated module origin absent"))?;
            let site = Key(origin.span.0);
            if !spans.contains_key(&site.0) {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated module span absent",
                ));
            }
            let mut producer =
                crate::core::digest::StableEncoder::new("lrr/v2/generated-module-source");
            producer.record(&header.key);
            producer.record(&child);
            let producer = producer.finish();
            let request_bytes = request
                .module_path
                .iter()
                .chain(&request.directory)
                .try_fold(0usize, |bytes, name| {
                    bytes
                        .checked_add(name.capacity())
                        .and_then(|n| n.checked_add(64))
                })
                .and_then(|n| n.checked_add(request.name.capacity()))
                .and_then(|n| {
                    n.checked_add(request.path_override.as_ref().map_or(0, String::capacity))
                })
                .and_then(|n| n.checked_add(child.heap_bytes()))
                .and_then(|n| n.checked_add(512))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated module demand metadata overflow",
                    )
                })?;
            bytes = bytes
                .checked_add(request_bytes)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated module demand metadata limit",
                    )
                })?;
            module_sources.try_reserve(1).map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated module demand allocation",
                )
            })?;
            module_sources.push(GeneratedModuleSource {
                producer,
                header: header.key.clone(),
                scope: child,
                request,
                site,
            });
            blocked.insert(producer);
        }
        module_sources.sort_by_key(GeneratedModuleSource::producer);
        let mut imports = Vec::new();
        for import in &outline.imports {
            if !contained_scope(&import.scope, &scope)
                || import.path.iter().any(
                    |p| matches!(p,crate::syntax::imports::ImportSegment::Name(n) if n=="$crate"),
                )
            {
                blocked.insert(items.record().key().invocation);
                continue;
            }
            let first = items
                .origins()
                .partition_point(|o| o.end <= import.range.start);
            let last = items
                .origins()
                .partition_point(|o| o.start < import.range.end);
            let mut comparable = true;
            for origin in &items.origins()[first..last] {
                let span = spans.get(&origin.span.0).ok_or_else(|| {
                    fault("LRR-E-EXPANSION-INPUT", "generated import span absent")
                })?;
                if items.item_class(span.context)? != owner_class
                    || declaration_hygiene.item(span.context)? != owner_context
                {
                    comparable = false;
                    break;
                }
            }
            if !comparable {
                return Err(fault(
                    "LRR-E-NAME-CONTEXT",
                    "generated import requires actual binding and target name-context occurrences",
                ));
            }
            let index = items
                .items()
                .partition_point(|h| h.start <= import.range.start)
                .checked_sub(1)
                .ok_or_else(|| {
                    fault("LRR-E-CATALOG-INCOMPLETE", "generated import header absent")
                })?;
            let header = &items.items()[index];
            if header.end < import.range.end {
                return Err(fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "generated import escapes header",
                ));
            }
            let mut import = import.clone();
            import.identity = header.key.fingerprint();
            blocked.insert(NameNamespaces::import_identity(&import));
            if imports.len() >= 100000 {
                return Err(fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated import count limit",
                ));
            }
            imports
                .try_reserve(1)
                .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated import allocation"))?;
            imports.push(import);
        }
        for item in &outline.items {
            if item.def_key.item_kind == ItemKind::Impl || impl_owned_scope(&item.scope) {
                blocked.insert(
                    impl_frontier.ok_or_else(|| {
                        fault("LRR-E-EXPANSION-INPUT", "impl frontier owner absent")
                    })?,
                );
                if !contained_scope(&item.scope, &scope) && !impl_owned_scope(&item.scope) {
                    blocked.insert(items.record().key().invocation);
                }
            } else if !contained_scope(&item.scope, &scope) {
                blocked.insert(items.record().key().invocation);
            } else if item.def_key.item_kind == ItemKind::Macro {
                let header = headers
                    .get(&(item.declaration_span.start, item.declaration_span.end))
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "generated macro structural header absent",
                        )
                    })?;
                blocked.insert(header.key.fingerprint());
            }
        }
        if !outline.scopes.contains_key(&scope) {
            return Err(fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "generated root scope absent",
            ));
        }
        for (route_scope, members) in &outline.scopes {
            if !contained_scope(route_scope, &scope) {
                if impl_owned_scope(route_scope) {
                    blocked.insert(impl_frontier.ok_or_else(|| {
                        fault("LRR-E-EXPANSION-INPUT", "impl scope frontier absent")
                    })?);
                } else {
                    blocked.insert(items.record().key().invocation);
                }
                continue;
            }
            for ((namespace, _name), ids) in &members.candidates {
                for id in ids {
                    let item = definitions.get(id).ok_or_else(|| {
                        fault(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "generated candidate header absent",
                        )
                    })?;
                    if (crate::syntax::outline::potential_attribute_producer(item).is_some()
                        && !settled(item.declaration_span.start, item.declaration_span.end))
                        || matches!(item.def_key.item_kind, ItemKind::Macro | ItemKind::Impl)
                    {
                        continue;
                    }
                    let header = headers
                        .get(&(item.declaration_span.start, item.declaration_span.end))
                        .ok_or_else(|| {
                            fault(
                                "LRR-E-CATALOG-INCOMPLETE",
                                "generated structural header mapping absent",
                            )
                        })?;
                    let origin = header.name.as_ref().ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "generated named header origin absent",
                        )
                    })?;
                    let span = spans.get(&origin.span.0).copied().ok_or_else(|| {
                        fault("LRR-E-EXPANSION-INPUT", "generated name span absent")
                    })?;
                    let visibility = NameNamespaces::item_visibility(item, *namespace, edition)?;
                    let declaration = Arc::new(DeclaredItemName::capture(
                        items,
                        header,
                        &source_parse,
                        span,
                        route_scope,
                        *namespace,
                        visibility,
                        owner,
                    )?);
                    bytes = bytes
                        .checked_add(declaration.metadata_bytes()?)
                        .and_then(|n| n.checked_add(std::mem::size_of::<GeneratedRoute>() + 96))
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| {
                            fault("LRR-E-DISCOVERY-LIMIT", "generated route metadata limit")
                        })?;
                    if routes.len() >= 1_000_000 {
                        return Err(fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "generated route count limit",
                        ));
                    }
                    routes.try_reserve(1).map_err(|_| {
                        fault("LRR-E-DISCOVERY-LIMIT", "generated route allocation")
                    })?;
                    routes.push(GeneratedRoute {
                        declaration,
                        module: item.def_key.item_kind == ItemKind::Module
                            && header.kind
                                != crate::expand::generated_items::GeneratedHeaderKind::ExternCrate,
                    });
                }
            }
        }
        Ok(Self {
            scope,
            routes,
            blocked,
            impl_frontier,
            impl_ranges,
            cfg: cfg.fingerprint(),
            epoch,
            attributes,
            guard: items.record().descriptor().fingerprint(),
            imports,
            edition,
            module_sources,
        })
    }
    pub(crate) fn attribute_frontiers(&self) -> &BTreeSet<Digest> {
        &self.attributes
    }
    pub fn module_sources(&self) -> &[GeneratedModuleSource] {
        &self.module_sources
    }
    pub(crate) fn discovery_imports(&self) -> &[crate::syntax::imports::SourceImport] {
        &self.imports
    }
    pub fn blocked(&self) -> impl Iterator<Item = &Digest> {
        self.blocked.iter()
    }
    pub(crate) fn impl_frontier(&self) -> Option<Digest> {
        self.impl_frontier
    }
    pub(crate) fn active_impl_ranges(
        &self,
        items: &GeneratedItems,
        cfg: &CfgSet,
        scope: &ScopeKey,
        epoch: u64,
    ) -> Result<&[crate::syntax::spans::SourceRange], Diagnostic> {
        if self.guard != items.record().descriptor().fingerprint()
            || self.cfg != cfg.fingerprint()
            || &self.scope != scope
            || self.epoch != epoch
            || items.epoch() != epoch
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "active impl projection owner differs",
            ));
        }
        Ok(&self.impl_ranges)
    }
    pub fn publish(self, graph: &mut NameNamespaces) -> Result<(), Diagnostic> {
        // Import frontiers are installed at their actual lexical module by
        // admit_generated_import, then retired by that module's import resolver.
        // Reserving them again at the fragment root would leave an orphan frontier.
        self.imports
            .len()
            .checked_mul(96)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated import frontier metadata limit",
                )
            })?;
        let imported = self
            .imports
            .iter()
            .map(NameNamespaces::import_identity)
            .collect::<BTreeSet<_>>();
        // Reserve before mutation: a failed or partial integration cannot certify absence.
        graph.reserve_generated_frontier(self.scope.clone(), self.guard)?;
        for id in &self.blocked {
            if !imported.contains(id) {
                graph.reserve_generated_frontier(self.scope.clone(), *id)?;
            }
        }
        for route in self.routes {
            graph.define_declared_item(route.declaration, route.module)?;
        }
        for import in self.imports {
            graph.admit_generated_import(import, self.edition)?;
        }
        graph.release_generated_frontier(&self.scope, self.guard);
        Ok(())
    }
}

#[cfg(test)]
#[path = "generated_impl_frontier_tests.rs"]
mod frontier_tests;
