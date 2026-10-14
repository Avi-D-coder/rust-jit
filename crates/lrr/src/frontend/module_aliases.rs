//! Deterministic source module-alias worklist. No queries, expansion or effects.
use super::{
    macro_names::{MacroNamespaces, MacroPath},
    source_macro_routes::{DeferredImportReason, SourceMacroRoutes, module_names},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::MacroName,
    syntax::imports::{ImportKind, ImportSegment, SourceImport},
};
use std::collections::{BTreeMap, BTreeSet};
type ModuleKey = (Digest, Vec<MacroName>);
type WeakModules =
    BTreeMap<ModuleKey, BTreeMap<ModuleKey, BTreeSet<crate::syntax::visibility::VisibilityScope>>>;
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
#[derive(Default)]
struct Budget {
    bytes: usize,
}
impl Budget {
    fn key(&mut self, key: &ModuleKey) -> Result<(), Diagnostic> {
        self.path(&key.1)
    }
    fn path(&mut self, names: &[MacroName]) -> Result<(), Diagnostic> {
        if names.len() > 256 {
            return Err(error("module alias path depth exceeded"));
        }
        self.bytes = self
            .bytes
            .checked_add(std::mem::size_of::<ModuleKey>())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("module alias metadata budget exceeded"))?;
        for name in names {
            self.bytes = self
                .bytes
                .checked_add(name.as_str().len() + std::mem::size_of::<MacroName>())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("module alias metadata budget exceeded"))?;
        }
        Ok(())
    }
}
struct Job<'a> {
    source: &'a SourceImport,
    target: ModuleKey,
    destination: ModuleKey,
}
struct Modules {
    known: BTreeSet<ModuleKey>,
    aliases: BTreeMap<ModuleKey, ModuleKey>,
    externs: BTreeMap<ModuleKey, (Digest, ModuleKey)>,
    injections: BTreeMap<ModuleKey, (Digest, ModuleKey)>,
    weak: WeakModules,
    budget: Budget,
}
enum MacroTarget {
    Known(MacroPath),
    Wait(ModuleKey),
}
enum Resolution {
    Known(ModuleKey),
    Wait(ModuleKey),
}
impl Modules {
    fn resolve(&self, unit: Digest, names: &[MacroName]) -> Result<Resolution, Diagnostic> {
        let mut current = (unit, Vec::new());
        if !self.known.contains(&current) {
            return Ok(Resolution::Wait(current));
        }
        for name in names {
            if current.1.len() >= 256 {
                return Err(error("resolved module alias path depth exceeded"));
            }
            current
                .1
                .try_reserve(1)
                .map_err(|_| error("module alias path allocation failed"))?;
            current.1.push(name.clone());
            if let Some(target) = self.aliases.get(&current) {
                current = target.clone();
            } else if !self.known.contains(&current) {
                match self.weak.get(&current) {
                    Some(members) if members.len() == 1 => {
                        current = members.first_key_value().expect("one member").0.clone()
                    }
                    _ => return Ok(Resolution::Wait(current)),
                }
            }
        }
        Ok(Resolution::Known(current))
    }
    fn macro_target(&self, reference: &MacroPath) -> Result<MacroTarget, Diagnostic> {
        let (last, modules) = reference
            .components()
            .split_last()
            .ok_or_else(|| error("empty macro target"))?;
        let (unit, mut names) = match self.resolve(reference.unit, modules)? {
            Resolution::Known(target) => target,
            Resolution::Wait(key) => return Ok(MacroTarget::Wait(key)),
        };
        if names.len() >= 256 {
            return Err(error("resolved macro alias depth exceeded"));
        }
        names
            .try_reserve(1)
            .map_err(|_| error("macro alias target allocation failed"))?;
        names.push(last.clone());
        MacroPath::new(unit, names).map(MacroTarget::Known)
    }
}
fn module_demand(key: &ModuleKey) -> Digest {
    let mut encoded = StableEncoder::new("lrr/v2/pending-source-module-membership");
    encoded.digest(key.0);
    encoded.sequence(&key.1, |encoded, name| encoded.string(name.as_str()));
    encoded.finish()
}
fn reference(
    source: &SourceImport,
    catalog: &SourceMacroRoutes,
    known: &BTreeSet<ModuleKey>,
) -> Result<ModuleKey, Diagnostic> {
    let edition = catalog.edition;
    let mut unit = source.scope.crate_key;
    let mut names = module_names(&source.scope.module_path, edition)?;
    let mut segments = source.path.iter().peekable();
    if source.absolute && edition == 2015 {
        names.clear();
    } else if source.absolute {
        let Some(ImportSegment::Name(alias)) = segments.next() else {
            return Err(error("absolute alias requires a dependency"));
        };
        if !catalog.extern_prelude_enabled(&source.scope) {
            return Err(error("absolute dependency is outside the implicit prelude"));
        }
        unit = *catalog
            .dependencies
            .get(&MacroName::parse(alias, edition)?)
            .ok_or_else(|| error("unknown absolute dependency alias"))?;
        names.clear();
    } else {
        if edition == 2015 && matches!(segments.peek(), Some(ImportSegment::Name(_))) {
            names.clear();
        }
        match segments.peek() {
            Some(ImportSegment::Crate) => {
                names.clear();
                segments.next();
            }
            Some(ImportSegment::SelfModule) => {
                segments.next();
            }
            Some(ImportSegment::Name(first)) => {
                let first = MacroName::parse(first, edition)?;
                let mut local = names.clone();
                local.push(first.clone());
                if !known.contains(&(unit, local))
                    && catalog.extern_prelude_enabled(&source.scope)
                    && let Some(provider) = catalog.dependencies.get(&first)
                {
                    unit = *provider;
                    names.clear();
                    segments.next();
                }
            }
            _ => {}
        }
    }
    let mut named = false;
    for segment in segments {
        match segment {
            ImportSegment::Super if !named => {
                names
                    .pop()
                    .ok_or_else(|| error("module alias escapes crate root"))?;
            }
            ImportSegment::SelfModule if named => {}
            ImportSegment::Name(name) => {
                named = true;
                if names.len() >= 256 {
                    return Err(error("module alias depth exceeded"));
                }
                names
                    .try_reserve(1)
                    .map_err(|_| error("module alias allocation failed"))?;
                names.push(MacroName::parse(name, edition)?);
            }
            _ => return Err(error("invalid module alias qualifier")),
        }
    }
    Ok((unit, names))
}
fn destination(source: &SourceImport, edition: u16) -> Result<ModuleKey, Diagnostic> {
    let alias = match &source.kind {
        ImportKind::Named { alias: Some(alias) } => MacroName::parse(alias, edition)?,
        ImportKind::Named { alias: None } => source
            .path
            .iter()
            .rev()
            .find_map(|segment| {
                if let ImportSegment::Name(name) = segment {
                    Some(MacroName::parse(name, edition))
                } else {
                    None
                }
            })
            .ok_or_else(|| error("module import needs an explicit alias"))??,
        _ => return Err(error("unnamed module import")),
    };
    let mut names = module_names(&source.scope.module_path, edition)?;
    if names.len() >= 256 {
        return Err(error("module binding depth exceeded"));
    }
    names
        .try_reserve(1)
        .map_err(|_| error("module binding allocation failed"))?;
    names.push(alias);
    Ok((source.scope.crate_key, names))
}
pub(super) fn candidate_graph(
    catalogs: &[SourceMacroRoutes],
) -> Result<MacroNamespaces, Diagnostic> {
    if catalogs.len() > 100_000 {
        return Err(error("source catalog count exceeded"));
    }
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve_exact(catalogs.len())
        .map_err(|_| error("source catalogue borrow allocation failed"))?;
    borrowed.extend(catalogs.iter());
    graph_with_producers(&borrowed, false)
}
pub(super) fn graph_with_producers(
    catalogs: &[&SourceMacroRoutes],
    pending: bool,
) -> Result<MacroNamespaces, Diagnostic> {
    graph_with_compiler_imports(catalogs, pending, &[])
}
pub(super) fn graph_with_compiler_imports(
    catalogs: &[&SourceMacroRoutes],
    pending: bool,
    imports: &[super::compile_input_catalog::CompilerInjectedImport],
) -> Result<MacroNamespaces, Diagnostic> {
    if imports.len() > 100_000 {
        return Err(error("compiler import count limit"));
    }
    if catalogs.len() > 100_000 {
        return Err(error("source catalog count exceeded"));
    }
    if catalogs.iter().flat_map(|c| c.deferred()).any(|d| {
        !matches!(
            d.reason,
            DeferredImportReason::ModuleAlias
                | DeferredImportReason::ModuleLookup
                | DeferredImportReason::Glob
                | DeferredImportReason::DependencyModuleCollision
        ) && !(pending && matches!(d.reason, DeferredImportReason::TextualProducer(_)))
    }) {
        return Err(error("source routes have unresolved import demands"));
    }
    let mut modules = Modules {
        known: BTreeSet::new(),
        aliases: BTreeMap::new(),
        externs: BTreeMap::new(),
        injections: BTreeMap::new(),
        weak: BTreeMap::new(),
        budget: Budget::default(),
    };
    for catalog in catalogs {
        for key in &catalog.modules {
            if !modules.known.contains(key) {
                modules.budget.key(key)?;
                modules.known.insert(key.clone());
            }
        }
    }
    for import in imports {
        let (consumer, alias, provider) = import.parts();
        // A captured dependency with this spelling must have the exact facade
        // owner, and a disclosed source module retains lexical precedence.
        if !catalogs.iter().any(|catalog| {
            catalog.modules.contains(&(consumer, vec![]))
                && catalog.dependencies.get(alias) == Some(&provider)
        }) {
            continue;
        }
        let destination = (consumer, vec![alias.clone()]);
        if modules.known.contains(&destination) {
            continue;
        }
        let target = (provider, vec![]);
        if let Some((_, previous)) = modules.injections.get(&destination) {
            if previous != &target {
                return Err(error("conflicting compiler import targets"));
            }
            continue;
        }
        modules.budget.key(&destination)?;
        modules.budget.key(&target)?;
        modules
            .injections
            .insert(destination, (import.identity(), target));
    }
    for catalog in catalogs {
        for row in &catalog.extern_aliases {
            let Some(name) = &row.name else {
                continue;
            };
            let mut names = module_names(&row.scope.module_path, catalog.edition)?;
            if names.len() >= 256 {
                return Err(error("extern alias path depth exceeded"));
            }
            names
                .try_reserve(1)
                .map_err(|_| error("extern alias path allocation failed"))?;
            names.push(name.clone());
            let destination = (row.scope.crate_key, names);
            if modules.known.contains(&destination) {
                return Err(error("extern crate alias conflicts with a source module"));
            }
            let target = (row.provider, vec![]);
            modules.budget.key(&destination)?;
            modules.budget.key(&target)?;
            let declaration = (row.declaration, target);
            if let Some(old) = modules.externs.insert(destination, declaration.clone())
                && old != declaration
            {
                return Err(error(
                    "duplicate explicit extern crate aliases across captured fragments",
                ));
            }
        }
    }
    let mut jobs = Vec::new();
    for catalog in catalogs {
        for source in &catalog.source_imports {
            if !matches!(source.kind, ImportKind::Named { .. }) {
                continue;
            }
            if jobs.len() >= 100_000 {
                return Err(error("module import edge limit exceeded"));
            }
            let target = reference(source, catalog, &modules.known)?;
            let destination = destination(source, catalog.edition)?;
            modules.budget.key(&target)?;
            modules.budget.key(&destination)?;
            jobs.try_reserve(1)
                .map_err(|_| error("module import job allocation failed"))?;
            jobs.push(Job {
                source,
                target,
                destination,
            });
        }
    }
    let mut steps = 0usize;
    let mut seen = BTreeSet::new();
    let (completed, mut graph) = loop {
        modules.aliases.clear();
        for (destination, (_, target)) in &modules.injections {
            modules.budget.key(destination)?;
            modules.budget.key(target)?;
            modules.aliases.insert(destination.clone(), target.clone());
        }
        for (destination, (_, target)) in &modules.externs {
            modules.budget.key(destination)?;
            modules.budget.key(target)?;
            modules.aliases.insert(destination.clone(), target.clone());
        }
        let completed = named_modules(&jobs, &mut modules, &mut steps)?;
        let graph = module_registry(catalogs, &completed, &modules)?;
        let weak = glob_modules(catalogs, &modules, &graph, &mut steps)?;
        if weak == modules.weak {
            break (completed, graph);
        }
        let digest = module_state(&modules.aliases, &weak);
        if !seen.insert(digest) {
            return Err(error(
                "cyclic strong/glob module selection has no stable result",
            ));
        }
        if seen.len() > 100_000 {
            return Err(error("module glob fixed point state limit exceeded"));
        }
        modules.weak = weak;
    };
    for catalog in catalogs {
        for definition in catalog.definitions() {
            modules.budget.path(definition.path.components())?;
            let scope = crate::syntax::outline::ScopeKey {
                crate_key: definition.path.unit,
                module_path: definition.path.components()[..definition.path.components().len() - 1]
                    .iter()
                    .map(|name| name.as_str().into())
                    .collect(),
                owner: None,
            };
            graph.define_visible_origin(
                definition.path.clone(),
                definition.definition.clone(),
                crate::syntax::visibility::VisibilityScope::for_declaration(
                    &definition.visibility,
                    &scope,
                    catalog.edition,
                )?,
                catalog.binding_origin,
            )?;
        }
        for route in catalog.imports() {
            let (unit, names) = reference(&route.source, catalog, &modules.known)?;
            let original_target = MacroPath::new(unit, names)?;
            let target = match modules.macro_target(&original_target)? {
                MacroTarget::Known(target) => target,
                MacroTarget::Wait(key) if pending => {
                    modules.budget.key(&key)?;
                    modules.budget.path(route.path.components())?;
                    graph.pending(route.path.clone(), module_demand(&key))?;
                    continue;
                }
                MacroTarget::Wait(_) => {
                    return Err(error("source module membership remains unresolved"));
                }
            };
            modules.budget.path(route.path.components())?;
            modules.budget.path(target.components())?;
            graph.reexport_visible_origin(
                route.path.clone(),
                original_target,
                crate::syntax::visibility::VisibilityScope::for_declaration(
                    &route.source.visibility,
                    &route.source.scope,
                    catalog.edition,
                )?,
                catalog.binding_origin,
            )?;
        }
        for deferred in catalog.deferred() {
            if let DeferredImportReason::TextualProducer(producer) = deferred.reason {
                let dest = destination(&deferred.source, catalog.edition)?;
                modules.budget.path(&dest.1)?;
                graph.pending(MacroPath::new(dest.0, dest.1)?, producer)?;
                continue;
            }
            if deferred.reason == DeferredImportReason::Glob {
                continue;
            }
            let source = &deferred.source;
            let target = reference(source, catalog, &modules.known)?;
            if deferred.reason == DeferredImportReason::ModuleAlias {
                if !completed.contains(&(source.identity, source.leaf_ordinal)) {
                    return Err(error("module alias cycle or unresolved target"));
                }
                // A named module and macro may share a spelling. Retain the macro edge too
                // whenever the target has a nonempty path; macro absence stays separate.
                if target.1.is_empty()
                    || matches!(
                        source.path.last(),
                        Some(
                            ImportSegment::SelfModule | ImportSegment::Crate | ImportSegment::Super
                        )
                    )
                {
                    continue;
                }
            }
            let original_target = MacroPath::new(target.0, target.1)?;
            let target = match modules.macro_target(&original_target)? {
                MacroTarget::Known(target) => target,
                MacroTarget::Wait(key) if pending => {
                    let dest = destination(source, catalog.edition)?;
                    modules.budget.key(&key)?;
                    modules.budget.path(&dest.1)?;
                    graph.pending(MacroPath::new(dest.0, dest.1)?, module_demand(&key))?;
                    continue;
                }
                MacroTarget::Wait(_) => {
                    return Err(error("source module membership remains unresolved"));
                }
            };
            let dest = destination(source, catalog.edition)?;
            let dest = MacroPath::new(dest.0, dest.1)?;
            modules.budget.path(dest.components())?;
            modules.budget.path(target.components())?;
            graph.reexport_visible_origin(
                dest,
                original_target,
                crate::syntax::visibility::VisibilityScope::for_declaration(
                    &source.visibility,
                    &source.scope,
                    catalog.edition,
                )?,
                catalog.binding_origin,
            )?;
        }
    }
    let names = graph.member_names();
    let mut glob_count = jobs.len();
    for catalog in catalogs {
        for deferred in catalog
            .deferred()
            .iter()
            .filter(|d| d.reason == DeferredImportReason::Glob)
        {
            glob_count = glob_count
                .checked_add(1)
                .filter(|n| *n <= 100_000)
                .ok_or_else(|| error("source import edge limit exceeded"))?;
            let source = &deferred.source;
            let (unit, parts) = reference(source, catalog, &modules.known)?;
            if !matches!(modules.resolve(unit, &parts)?, Resolution::Known(_)) {
                return Err(error("glob module scope remains unresolved"));
            }
            if !matches!(
                graph.module_paths.resolve_module(
                    &super::module_paths::ModulePath {
                        unit,
                        names: parts.clone()
                    },
                    Some(&source.scope),
                )?,
                super::module_paths::ModuleResolution::Known { .. }
            ) {
                return Err(error("glob source module is inaccessible"));
            }
            let visibility = crate::syntax::visibility::VisibilityScope::for_declaration(
                &source.visibility,
                &source.scope,
                catalog.edition,
            )?;
            for name in &names {
                let mut target = parts.clone();
                target.push(name.clone());
                let mut destination = module_names(&source.scope.module_path, catalog.edition)?;
                destination.push(name.clone());
                modules.budget.path(&target)?;
                modules.budget.path(&destination)?;
                graph.glob_member_origin(
                    MacroPath::new(source.scope.crate_key, destination)?,
                    MacroPath::new(unit, target)?,
                    visibility.clone(),
                    catalog.binding_origin,
                )?;
            }
        }
    }
    for catalog in catalogs {
        for unit in catalog
            .modules
            .iter()
            .map(|(unit, _)| *unit)
            .collect::<BTreeSet<_>>()
        {
            graph.pending_exports(unit, &catalog.export_pending)?;
        }
        for row in &catalog.extern_aliases {
            graph.macro_use_import(row, catalog.edition, catalog.binding_origin.is_some())?;
        }
    }
    graph.validate_macro_use_selectors()?;
    Ok(graph)
}

fn named_modules(
    jobs: &[Job<'_>],
    modules: &mut Modules,
    steps: &mut usize,
) -> Result<BTreeSet<(Digest, u32)>, Diagnostic> {
    let mut ready: BTreeSet<_> = (0..jobs.len()).collect();
    let mut waiting: BTreeMap<ModuleKey, BTreeSet<usize>> = BTreeMap::new();
    let mut completed = BTreeSet::new();

    while let Some(index) = ready.pop_first() {
        *steps = steps
            .checked_add(1)
            .filter(|n| *n <= 100_000 * 257)
            .ok_or_else(|| error("module alias work limit exceeded"))?;
        let job = &jobs[index];
        match modules.resolve(job.target.0, &job.target.1)? {
            Resolution::Wait(key) => {
                if !waiting.contains_key(&key) {
                    modules.budget.key(&key)?;
                }
                waiting.entry(key).or_default().insert(index);
            }
            Resolution::Known(target) => {
                if modules.externs.contains_key(&job.destination) {
                    return Err(error(
                        "named module import conflicts with an explicit extern crate declaration",
                    ));
                }
                if modules.known.contains(&job.destination) && target != job.destination {
                    return Err(error("module alias conflicts with a source module"));
                }
                if let Some(existing) = modules.aliases.get(&job.destination) {
                    if existing != &target {
                        return Err(error("module alias has ambiguous targets"));
                    }
                } else {
                    modules.budget.key(&job.destination)?;
                    modules.budget.key(&target)?;
                    modules.aliases.insert(job.destination.clone(), target);
                    if let Some(wake) = waiting.remove(&job.destination) {
                        ready.extend(wake);
                    }
                }
                completed.insert((job.source.identity, job.source.leaf_ordinal));
            }
        }
    }
    Ok(completed)
}
fn module_registry(
    catalogs: &[&SourceMacroRoutes],
    completed: &BTreeSet<(Digest, u32)>,
    modules: &Modules,
) -> Result<MacroNamespaces, Diagnostic> {
    let mut graph = MacroNamespaces::default();
    for catalog in catalogs {
        for (definition, producer) in &catalog.body_owner_pending {
            graph.defer_body_owner(*definition, *producer)?;
        }
        for scope in &catalog.implicit_disabled {
            graph.disable_implicit(scope.clone())?;
        }
    }

    for catalog in catalogs {
        for ((unit, names), visibility) in &catalog.module_access {
            graph.module_paths.declare(
                super::module_paths::ModulePath {
                    unit: *unit,
                    names: names.clone(),
                },
                visibility.clone(),
            )?;
        }
    }
    for catalog in catalogs {
        for row in &catalog.extern_aliases {
            let Some(name) = &row.name else {
                continue;
            };
            let (scope, alias, provider, visibility) =
                (&row.scope, name, &row.provider, &row.visibility);
            let mut names = module_names(&scope.module_path, catalog.edition)?;
            names.push(alias.clone());
            graph.module_paths.alias(
                super::module_paths::ModulePath {
                    unit: scope.crate_key,
                    names,
                },
                super::module_paths::ModulePath {
                    unit: *provider,
                    names: vec![],
                },
                crate::syntax::visibility::VisibilityScope::for_declaration(
                    visibility,
                    scope,
                    catalog.edition,
                )?,
            )?;
        }
    }
    for ((unit, names), (_, (provider, target_names))) in &modules.injections {
        let path = super::module_paths::ModulePath {
            unit: *unit,
            names: names.clone(),
        };
        if !graph.module_paths.contains(&path) {
            graph.module_paths.alias(
                path,
                super::module_paths::ModulePath {
                    unit: *provider,
                    names: target_names.clone(),
                },
                crate::syntax::visibility::VisibilityScope::Within {
                    unit: *unit,
                    module_path: vec![],
                },
            )?;
        }
    }
    for catalog in catalogs {
        for source in &catalog.source_imports {
            if !completed.contains(&(source.identity, source.leaf_ordinal)) {
                continue;
            }
            let (unit, names) = destination(source, catalog.edition)?;
            let (target_unit, target_names) = reference(source, catalog, &modules.known)?;
            graph.module_paths.alias(
                super::module_paths::ModulePath { unit, names },
                super::module_paths::ModulePath {
                    unit: target_unit,
                    names: target_names,
                },
                crate::syntax::visibility::VisibilityScope::for_declaration(
                    &source.visibility,
                    &source.scope,
                    catalog.edition,
                )?,
            )?;
        }
    }
    for catalog in catalogs {
        if let Some((unit, _)) = catalog.modules.iter().find(|(_, names)| names.is_empty()) {
            for (alias, provider) in &catalog.dependencies {
                graph.dependency(*unit, alias.clone(), *provider)?;
                let path = super::module_paths::ModulePath {
                    unit: *unit,
                    names: vec![alias.clone()],
                };
                if catalog.extern_prelude_enabled(&crate::syntax::outline::ScopeKey {
                    crate_key: *unit,
                    module_path: vec![],
                    owner: None,
                }) && !graph.module_paths.contains(&path)
                {
                    graph.module_paths.alias(
                        path,
                        super::module_paths::ModulePath {
                            unit: *provider,
                            names: vec![],
                        },
                        crate::syntax::visibility::VisibilityScope::Within {
                            unit: *unit,
                            module_path: vec![],
                        },
                    )?;
                }
            }
        }
    }

    for ((unit, names), members) in &modules.weak {
        for ((target_unit, target_names), ranges) in members {
            for range in ranges {
                graph.module_paths.glob(
                    super::module_paths::ModulePath {
                        unit: *unit,
                        names: names.clone(),
                    },
                    super::module_paths::ModulePath {
                        unit: *target_unit,
                        names: target_names.clone(),
                    },
                    range.clone(),
                )?;
            }
        }
    }
    Ok(graph)
}

fn module_state(aliases: &BTreeMap<ModuleKey, ModuleKey>, weak: &WeakModules) -> Digest {
    use crate::core::digest::StableEncode;
    let mut e = StableEncoder::new("lrr/v2/module-glob-state");
    fn key(e: &mut StableEncoder, key: &ModuleKey) {
        e.digest(key.0);
        e.sequence(&key.1, |e, n| e.string(n.as_str()));
    }
    e.u64(u64::try_from(aliases.len()).expect("bounded module aliases"));
    for (path, target) in aliases {
        key(&mut e, path);
        key(&mut e, target);
    }
    e.u64(u64::try_from(weak.len()).expect("bounded module members"));
    for (path, targets) in weak {
        key(&mut e, path);
        e.u64(u64::try_from(targets.len()).expect("bounded module targets"));
        for (target, ranges) in targets {
            key(&mut e, target);
            e.u64(u64::try_from(ranges.len()).expect("bounded visibility ranges"));
            for range in ranges {
                range.encode_fields(&mut e);
            }
        }
    }
    e.finish()
}
fn glob_modules(
    catalogs: &[&SourceMacroRoutes],
    modules: &Modules,
    graph: &MacroNamespaces,
    steps: &mut usize,
) -> Result<WeakModules, Diagnostic> {
    use super::module_paths::{ModulePath, ModuleResolution};
    use crate::syntax::visibility::VisibilityScope;
    modules
        .known
        .len()
        .checked_add(modules.aliases.len())
        .and_then(|n| n.checked_add(modules.weak.len()))
        .and_then(|n| n.checked_mul(std::mem::size_of::<&ModuleKey>()))
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(|| error("module member key index budget exceeded"))?;
    let keys: BTreeSet<_> = modules
        .known
        .iter()
        .chain(modules.aliases.keys())
        .chain(modules.weak.keys())
        .collect();
    let mut result = WeakModules::new();
    let mut budget = Budget::default();
    let mut edges = 0usize;
    for catalog in catalogs {
        for source in catalog
            .source_imports
            .iter()
            .filter(|s| s.kind == ImportKind::Glob)
        {
            *steps = steps
                .checked_add(1)
                .filter(|n| *n <= 100_000 * 257)
                .ok_or_else(|| error("module glob work limit exceeded"))?;
            let (unit, names) = reference(source, catalog, &modules.known)?;
            let ModuleResolution::Known { path: selected, .. } = graph
                .module_paths
                .resolve_module(&ModulePath { unit, names }, Some(&source.scope))?
            else {
                continue;
            };
            let visibility = VisibilityScope::for_declaration(
                &source.visibility,
                &source.scope,
                catalog.edition,
            )?;
            for member in &keys {
                *steps = steps
                    .checked_add(1)
                    .filter(|n| *n <= 100_000 * 257)
                    .ok_or_else(|| error("module glob work limit exceeded"))?;
                if member.0 != selected.unit
                    || member.1.len() != selected.names.len() + 1
                    || !member.1.starts_with(&selected.names)
                {
                    continue;
                }
                *steps = steps
                    .checked_add(1)
                    .filter(|n| *n <= 100_000 * 257)
                    .ok_or_else(|| error("module glob work limit exceeded"))?;
                let path = ModulePath {
                    unit: member.0,
                    names: member.1.clone(),
                };
                for (target, range) in graph.module_paths.member_candidates(&path, &source.scope)? {
                    let Some(range) = super::glob_lookup::intersection(&visibility, &range) else {
                        continue;
                    };
                    let mut names = module_names(&source.scope.module_path, catalog.edition)?;
                    names.push(member.1.last().expect("module child").clone());
                    let dest = (source.scope.crate_key, names);
                    // Explicit declarations and named module bindings suppress weak origins.
                    if modules.known.contains(&dest) || modules.aliases.contains_key(&dest) {
                        continue;
                    }
                    let target = (target.unit, target.names);
                    if result
                        .get(&dest)
                        .and_then(|m| m.get(&target))
                        .is_some_and(|r| r.contains(&range))
                    {
                        continue;
                    }
                    edges = edges
                        .checked_add(1)
                        .filter(|n| *n <= 100_000)
                        .ok_or_else(|| error("module glob member limit exceeded"))?;
                    budget.key(&dest)?;
                    budget.key(&target)?;
                    if let VisibilityScope::Within { module_path, .. } = &range {
                        budget.path(&module_names(module_path, catalog.edition)?)?;
                    }
                    result
                        .entry(dest)
                        .or_default()
                        .entry(target)
                        .or_default()
                        .insert(range);
                }
            }
        }
    }
    Ok(result)
}
