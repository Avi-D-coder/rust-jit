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
#[path = "generated_impl_projection.rs"]
pub(crate) mod impl_projection;
pub(crate) use impl_projection::ActiveImplProjection;
pub struct GeneratedHeaderPlan {
    scope: ScopeKey,
    routes: Vec<GeneratedRoute>,
    blocked: BTreeSet<Digest>,
    impl_frontier: Option<Digest>,
    trait_frontier: Option<Digest>,
    trait_ranges: Vec<crate::syntax::spans::SourceRange>,
    cfg: Digest,
    epoch: u64,
    impl_projection: Arc<ActiveImplProjection>,
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
    enumeration: bool,
}
fn contained_scope(scope: &ScopeKey, owner: &ScopeKey) -> bool {
    (scope.owner == owner.owner
        || scope.owner.is_none() && scope.module_path.len() > owner.module_path.len())
        && scope.crate_key == owner.crate_key
        && scope.module_path.starts_with(&owner.module_path)
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
enum HeaderOwnership<'a> {
    Module,
    Declarations(&'a crate::expand::generated_declarations::GeneratedDeclarations),
    Catalog(&'a super::generated_macros::GeneratedMacroCatalog),
    PendingParent(&'a super::generated_macros::GeneratedMacroCatalog),
}
impl GeneratedHeaderPlan {
    /// Used only by a completed declaration fragment's projection. Surviving
    /// headers have had their annotation decisions settled by that fragment;
    /// replacement masks remain applied by the owning projection.
    pub(crate) fn prepare_completed_declarations(
        items: &GeneratedItems,
        completed: &crate::expand::declaration_fragment::CompletedDeclarationNameSource<'_>,
        root: &crate::syntax::token_wire::ContextRecord,
        cfg: &CfgSet,
    ) -> Result<Self, Diagnostic> {
        let declarations = completed.output();
        let settled = items.items().iter().map(|header| header.key.fingerprint()).collect();
        Self::prepare_using(items, declarations.scan().scope().clone(),
            Key(items.record().key().parent_context.0), root.key(), cfg, items.epoch(),
            Some(root), &settled, HeaderOwnership::Declarations(declarations))
    }
    pub(crate) fn declared_names(&self) -> impl Iterator<Item = &Arc<DeclaredItemName>> {
        self.routes.iter().map(|route| &route.declaration)
    }
    pub fn prepare(
        items: &GeneratedItems,
        scope: ScopeKey,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_owned(items, scope, owner_context, owner_context, cfg, epoch)
    }
    /// Header projection of actual admitted item output. The declaration
    /// receipt supplies its insertion scope; arbitrary owner keys are not a
    /// substitute for an admitted source/producer attachment.
    pub(crate) fn prepare_admitted(
        items: &GeneratedItems,
        declarations: &crate::expand::generated_declarations::GeneratedDeclarations,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        let context = Key(items.record().key().parent_context.0);
        Self::prepare_using(
            items,
            declarations.scan().scope().clone(),
            context,
            context,
            cfg,
            epoch,
            None,
            &BTreeSet::new(),
            HeaderOwnership::Declarations(declarations),
        )
    }
    pub(crate) fn prepare_admitted_with_root(
        items: &GeneratedItems,
        declarations: &crate::expand::generated_declarations::GeneratedDeclarations,
        call_context: Key,
        root: &crate::syntax::token_wire::ContextRecord,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_using(items, declarations.scan().scope().clone(), call_context,
            root.key(), cfg, epoch, Some(root), &BTreeSet::new(),
            HeaderOwnership::Declarations(declarations))
    }
    pub(crate) fn prepare_pending_child(
        items: &GeneratedItems,
        scope: ScopeKey,
        call_context: Key,
        root: &crate::syntax::token_wire::ContextRecord,
        cfg: &CfgSet,
        epoch: u64,
        parent: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_using(items, scope, call_context, root.key(), cfg, epoch,
            Some(root), &BTreeSet::new(), HeaderOwnership::PendingParent(parent))
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
            HeaderOwnership::Module,
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
            HeaderOwnership::Module,
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
            HeaderOwnership::Catalog(catalog),
        )
    }
    pub(crate) fn settled_originals(
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<Self, Diagnostic> {
        let mut plan = Self::prepare_catalog(catalog)?;
        plan.routes.retain(|r| {
            catalog
                .settled_derive_items()
                .contains(&r.declaration.source().fingerprint())
        });
        plan.blocked.clear();
        plan.impl_frontier = None;
        plan.trait_frontier = None;
        plan.trait_ranges.clear();
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
        ownership: HeaderOwnership<'_>,
    ) -> Result<Self, Diagnostic> {
        if epoch != items.epoch() {
            return Err(fault(
                "LRR-E-EXPANSION-EPOCH",
                "generated header publication epoch differs",
            ));
        }
        let verified_local = match ownership {
            HeaderOwnership::Module => false,
            HeaderOwnership::Declarations(declarations) => {
                if declarations.record().descriptor().fingerprint() != items.record().descriptor().fingerprint()
                    || declarations.epoch() != epoch || declarations.scan().cfg() != cfg.fingerprint()
                    || declarations.scan().scope() != &scope {
                    return Err(fault("LRR-E-EXPANSION-INPUT", "generated header admitted insertion differs"));
                }
                true
            }
            HeaderOwnership::Catalog(catalog) => {
                catalog.require_captured_owner(items, &scope, cfg, epoch)?;
                true
            }
            HeaderOwnership::PendingParent(parent) => {
                parent.require_pending_owner(items, &scope, cfg, epoch)?;
                true
            }
        };
        if scope.owner.is_some() && !verified_local
            || items.record().key().parent_context != Digest(call_context.0) {
            return Err(fault("LRR-E-EXPANSION-INPUT", "generated header parent scope/context differs"));
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
        let outline = if verified_local {
            parsed.outline_item_fragment(&scope, cfg)?
        } else {
            parsed.outline(scope.crate_key, scope.module_path.clone(), cfg)?
        };
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
            .filter_map(|item| outline.item_attribute_producer(item))
            .map(|call| items.frontier_identity(call.frontier))
            .collect::<BTreeSet<_>>();
        let definitions = outline
            .items
            .iter()
            .enumerate()
            .map(|(index, i)| (i.def_key.fingerprint(), (index, i)))
            .collect::<BTreeMap<_, _>>();
        let attributes = outline
            .items
            .iter()
            .filter_map(|item| outline.item_attribute_producer(item))
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
        let trait_scopes = outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == ItemKind::Trait)
            .map(|item| {
                (
                    item.scope.module_path.as_slice(),
                    item.def_key.fingerprint(),
                )
            })
            .collect::<BTreeSet<_>>();
        let trait_frontier = (!trait_scopes.is_empty()).then(|| {
            let mut e = crate::core::digest::StableEncoder::new(
                "lrr/v1/generated-trait-declaration-frontier",
            );
            e.digest(items.record().descriptor().fingerprint());
            e.record(&scope);
            e.digest(cfg.fingerprint());
            e.u64(epoch);
            e.finish()
        });
        let mut trait_ranges = Vec::new();
        trait_ranges
            .try_reserve_exact(trait_scopes.len())
            .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "trait range allocation"))?;
        trait_ranges.extend(
            outline
                .items
                .iter()
                .filter(|item| item.def_key.item_kind == ItemKind::Trait)
                .map(|item| item.declaration_span),
        );
        trait_ranges.sort_by_key(|range| (range.start, range.end));
        if let Some(frontier) = trait_frontier {
            blocked.insert(frontier);
        }
        let trait_owned_scope = |candidate: &ScopeKey| {
            candidate.crate_key == scope.crate_key
                && candidate.owner.is_some_and(|owner| {
                    trait_scopes.contains(&(candidate.module_path.as_slice(), owner))
                })
        };
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
        let impl_projection = Arc::new(ActiveImplProjection::prepare(
            items,
            cfg,
            &scope,
            epoch,
            impl_ranges,
        )?);
        let impl_owned_scope = |candidate: &ScopeKey| {
            candidate.crate_key == scope.crate_key
                && candidate.owner.is_some_and(|owner| {
                    impl_scopes.contains(&(candidate.module_path.as_slice(), owner))
                })
        };
        let mut routes = Vec::new();
        let mut bytes = impl_scope_bytes
            .checked_add(
                trait_ranges
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::syntax::spans::SourceRange>())
                    .ok_or_else(|| {
                        fault("LRR-E-DISCOVERY-LIMIT", "trait range storage overflow")
                    })?,
            )
            .and_then(|bytes| bytes.checked_add(impl_projection.metadata_bytes()))
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "impl projection metadata limit"))?;
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
            if outline.item_attribute_producer(item).is_some()
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
            bytes = bytes.checked_add(request_bytes).ok_or_else(|| {
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
            } else if trait_owned_scope(&item.scope) {
                blocked.insert(trait_frontier.ok_or_else(|| {
                    fault("LRR-E-EXPANSION-INPUT", "trait frontier owner absent")
                })?);
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
                } else if trait_owned_scope(route_scope) {
                    blocked.insert(trait_frontier.ok_or_else(|| {
                        fault("LRR-E-EXPANSION-INPUT", "trait scope frontier absent")
                    })?);
                } else {
                    blocked.insert(items.record().key().invocation);
                }
                continue;
            }
            for ((namespace, _name), ids) in &members.candidates {
                for id in ids {
                    let (item_index, item) = *definitions.get(id).ok_or_else(|| {
                        fault(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "generated candidate header absent",
                        )
                    })?;
                    if (outline.item_attribute_producer(item).is_some()
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
                        .ok_or_else(|| {
                            fault("LRR-E-DISCOVERY-LIMIT", "generated route metadata limit")
                        })?;
                    routes.try_reserve(1).map_err(|_| {
                        fault("LRR-E-DISCOVERY-LIMIT", "generated route allocation")
                    })?;
                    routes.push(GeneratedRoute {
                        declaration: Arc::clone(&declaration),
                        enumeration: item.def_key.item_kind == ItemKind::Enum,
                        module: item.def_key.item_kind == ItemKind::Module
                            && header.kind
                                != crate::expand::generated_items::GeneratedHeaderKind::ExternCrate,
                    });
                    if item.def_key.item_kind == ItemKind::Enum {
                        let item_index = u32::try_from(item_index).map_err(|_| {
                            fault("LRR-E-DISCOVERY-LIMIT", "enum owner index overflow")
                        })?;
                        for (index, variant) in item.variants.iter().enumerate() {
                            let index = u32::try_from(index).map_err(|_| {
                                fault("LRR-E-DISCOVERY-LIMIT", "enum variant index overflow")
                            })?;
                            let origin = items
                                .origins()
                                .partition_point(|o| o.end <= variant.name_range.start);
                            let origin = items.origins().get(origin).ok_or_else(|| {
                                fault("LRR-E-EXPANSION-INPUT", "generated variant origin absent")
                            })?;
                            let span = spans.get(&origin.span.0).copied().ok_or_else(|| {
                                fault("LRR-E-EXPANSION-INPUT", "generated variant span absent")
                            })?;
                            let namespaces: &[crate::source::manifest::Namespace] =
                                if variant.has_value_constructor() {
                                    &[
                                        crate::source::manifest::Namespace::Type,
                                        crate::source::manifest::Namespace::Value,
                                    ]
                                } else {
                                    &[crate::source::manifest::Namespace::Type]
                                };
                            for namespace in namespaces {
                                let member = Arc::new(DeclaredItemName::capture_variant(
                                    items,
                                    &declaration,
                                    item_index,
                                    index,
                                    variant,
                                    span,
                                    *namespace,
                                    owner,
                                )?);
                                bytes = bytes
                                    .checked_add(member.metadata_bytes()?)
                                    .and_then(|n| {
                                        n.checked_add(std::mem::size_of::<GeneratedRoute>() + 96)
                                    })
                                    .ok_or_else(|| {
                                        fault(
                                            "LRR-E-DISCOVERY-LIMIT",
                                            "enum member metadata overflow",
                                        )
                                    })?;
                                routes.try_reserve(1).map_err(|_| {
                                    fault("LRR-E-DISCOVERY-LIMIT", "enum member allocation")
                                })?;
                                routes.push(GeneratedRoute {
                                    declaration: member,
                                    module: false,
                                    enumeration: false,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(Self {
            scope,
            routes,
            blocked,
            impl_frontier,
            trait_frontier,
            trait_ranges,
            cfg: cfg.fingerprint(),
            epoch,
            impl_projection,
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
    pub(crate) fn trait_frontier(&self) -> Option<Digest> {
        self.trait_frontier
    }
    pub(crate) fn active_trait_ranges(
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
                "active trait projection owner differs",
            ));
        }
        Ok(&self.trait_ranges)
    }
    pub(crate) fn active_impl_ranges(
        &self,
        items: &GeneratedItems,
        cfg: &CfgSet,
        scope: &ScopeKey,
        epoch: u64,
    ) -> Result<&[crate::syntax::spans::SourceRange], Diagnostic> {
        self.impl_projection
            .active_impl_ranges(items, cfg, scope, epoch)
    }
    pub(crate) fn impl_projection(&self) -> &ActiveImplProjection {
        &self.impl_projection
    }
    pub(crate) fn retained_impl_projection(&self) -> Arc<ActiveImplProjection> {
        self.impl_projection.clone()
    }
    /// Retain the same checked range projection for a child trait frontier.
    /// Declaration kind is checked by the common item attachment operation.
    pub(crate) fn retained_trait_projection(
        &self,
        items: &GeneratedItems,
        cfg: &CfgSet,
    ) -> Result<Arc<ActiveImplProjection>, Diagnostic> {
        let ranges = self.active_trait_ranges(items, cfg, &self.scope, self.epoch)?;
        Ok(Arc::new(ActiveImplProjection::prepare(
            items,
            cfg,
            &self.scope,
            self.epoch,
            ranges.to_vec(),
        )?))
    }
    pub fn publish(self, graph: &mut NameNamespaces) -> Result<(), Diagnostic> {
        // Import frontiers are installed at their actual lexical module by
        // admit_generated_import, then retired by that module's import resolver.
        // Reserving them again at the fragment root would leave an orphan frontier.
        self.imports.len().checked_mul(96).ok_or_else(|| {
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
            if route.enumeration {
                graph.declare_enum_scope(
                    route.declaration.scope(),
                    route.declaration.name(),
                    route.declaration.definition().definition_edition,
                    route.declaration.visibility().clone(),
                    route.declaration.class(),
                )?;
            }
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
