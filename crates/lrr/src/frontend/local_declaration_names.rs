//! One ordinary declaration-table/import/prefix kernel, independent of runtime bindings.
//! Wrappers must prove their real source occurrence and current lexical owner.
use super::*;
use crate::syntax::entered_macros::DeclarationFacts;
use crate::syntax::token_wire::NameContextKey;

/// Immutable declaration half of the ordinary local tables. Data alone grants
/// no source lookup authority; construction is private to the shared preparation.
pub struct DeclarationNameTables {
    pub(super) anonymous_imports: BTreeMap<u32, Vec<usize>>,
    pub(super) item_candidates: BTreeMap<
        (u32, crate::source::manifest::Namespace, MacroName),
        Vec<crate::frontend::names::NameDefinition>,
    >,
    pub(super) item_frontiers: BTreeMap<u32, BTreeSet<Digest>>,
    pub(super) named_imports: BTreeMap<(u32, MacroName), Vec<usize>>,
    pub(super) glob_imports: BTreeMap<u32, Vec<usize>>,
}
impl DeclarationNameTables {
    pub(super) fn prepare(
        namespace: &DeclarationFacts,
        edition: u16,
        declaration_names: Option<&owned_plan::LocalTypeNames>,
        retired: impl Fn(Digest) -> bool,
    ) -> Result<Self, Diagnostic> {
        let mut item_candidates: BTreeMap<_, Vec<crate::frontend::names::NameDefinition>> =
            BTreeMap::new();
        let mut item_frontiers: BTreeMap<_, BTreeSet<Digest>> = BTreeMap::new();
        let mut item_bytes = 0usize;
        for row in &namespace.items {
            for namespace in &row.namespaces {
                if *namespace == crate::source::manifest::Namespace::Macro {
                    continue;
                }
                let (name, definition_edition) = match declaration_names
                    .and_then(|names| names.item_spelling(row.item.def_key.fingerprint()))
                {
                    Some((name, actual_edition)) => (name.clone(), actual_edition),
                    None => (
                        MacroName::parse(&row.item.def_key.declared_name, edition)?,
                        edition,
                    ),
                };
                item_bytes = item_bytes
                    .checked_add(name.as_str().len())
                    .and_then(|n| n.checked_add(160))
                    .ok_or_else(|| error("body item namespace metadata limit"))?;
                item_candidates
                    .entry((row.scope, *namespace, name))
                    .or_default()
                    .push(crate::frontend::names::NameDefinition {
                        namespace: *namespace,
                        identity: row.item.def_key.fingerprint(),
                        defining_unit: row.item.scope.crate_key,
                        definition_edition,
                    });
            }
            if let Some(producer) = row.attribute_input {
                item_bytes = item_bytes
                    .checked_add(128)
                    .ok_or_else(|| error("body attribute frontier metadata limit"))?;
                item_frontiers
                    .entry(row.scope)
                    .or_default()
                    .insert(producer);
            }
        }
        for event in &namespace.events {
            if let BodyMacroEventKind::Producer { identity, .. } = event.kind {
                if retired(identity) {
                    continue;
                }
                item_bytes = item_bytes
                    .checked_add(128)
                    .ok_or_else(|| error("body item frontier metadata limit"))?;
                item_frontiers
                    .entry(event.scope)
                    .or_default()
                    .insert(identity);
            }
        }
        let mut named_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        let mut glob_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        let mut anonymous_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        for (index, import) in namespace.imports.iter().enumerate() {
            item_bytes = item_bytes
                .checked_add(128)
                .ok_or_else(|| error("body import frontier metadata limit"))?;
            if let crate::syntax::imports::ImportKind::Named { alias } = &import.source.kind {
                let name = alias.as_deref().or_else(|| {
                    import.source.path.iter().rev().find_map(|part| {
                        if let crate::syntax::imports::ImportSegment::Name(name) = part {
                            Some(name.as_str())
                        } else {
                            None
                        }
                    })
                });
                if let Some(name) = name {
                    item_bytes = item_bytes
                        .checked_add(name.len())
                        .ok_or_else(|| error("body named import metadata limit"))?;
                    named_imports
                        .entry((import.scope, MacroName::parse(name, edition)?))
                        .or_default()
                        .push(index);
                } else {
                    item_frontiers.entry(import.scope).or_default().insert(
                        crate::frontend::names::NameNamespaces::import_identity(&import.source),
                    );
                }
            } else if import.source.kind == crate::syntax::imports::ImportKind::Glob {
                glob_imports.entry(import.scope).or_default().push(index);
            } else if import.source.kind == crate::syntax::imports::ImportKind::Anonymous {
                anonymous_imports
                    .entry(import.scope)
                    .or_default()
                    .push(index);
            }
            if let Some(producer) = import.attribute_input {
                item_frontiers
                    .entry(import.scope)
                    .or_default()
                    .insert(producer);
            }
        }
        Ok(Self {
            anonymous_imports,
            glob_imports,
            named_imports,
            item_candidates,
            item_frontiers,
        })
    }
}
/// Borrowed view over the actual immutable declaration tables and facts. It
/// cannot grant entry, manufacture a body owner or select a module by spelling.
pub(super) struct DeclarationNameIndex<'a> {
    tables: &'a DeclarationNameTables,
    facts: &'a DeclarationFacts,
    module_scope: &'a crate::syntax::outline::ScopeKey,
    edition: u16,
}
impl Deref for DeclarationNameIndex<'_> {
    type Target = DeclarationNameTables;
    fn deref(&self) -> &Self::Target {
        self.tables
    }
}
impl<'a> DeclarationNameIndex<'a> {
    pub(super) fn new(
        tables: &'a DeclarationNameTables,
        facts: &'a DeclarationFacts,
        module_scope: &'a crate::syntax::outline::ScopeKey,
        edition: u16,
    ) -> Self {
        Self {
            tables,
            facts,
            module_scope,
            edition,
        }
    }
    pub(super) fn anonymous_import(
        &self,
        scope: u32,
        index: usize,
        names: &impl crate::frontend::names::reads::QualifiedNames,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<crate::frontend::names::AnonymousImportOutcome, Diagnostic> {
        use crate::frontend::names::AnonymousImportCandidates;
        use crate::syntax::imports::ImportSegment;
        let import = &self.facts.imports[index].source;
        let mut candidates = AnonymousImportCandidates::default();
        let edition = self.edition;
        for namespace in [
            crate::source::manifest::Namespace::Value,
            crate::source::manifest::Namespace::Type,
            crate::source::manifest::Namespace::Macro,
        ] {
            if !import.absolute
                && let [ImportSegment::Name(name)] = import.path.as_slice()
            {
                candidates.observe(self.import_lookup(
                    scope,
                    &MacroName::parse(name, edition)?,
                    namespace,
                    names,
                    work,
                    memo,
                )?)?;
            } else {
                for route in self.import_routes(scope, import, names, work)? {
                    candidates.observe(names.lookup_import_target(&route, namespace, edition)?)?;
                }
            }
        }
        Ok(candidates.finish(import))
    }
    pub(super) fn import_routes(
        &self,
        scope: u32,
        source: &crate::syntax::imports::SourceImport,
        names: &impl crate::frontend::names::reads::QualifiedNames,
        work: &mut usize,
    ) -> Result<Vec<crate::syntax::imports::SourceImport>, Diagnostic> {
        use crate::syntax::imports::{ImportSegment, SourceImport};
        fn charge_route(bytes: &mut usize, source: &SourceImport) -> Result<(), Diagnostic> {
            import_metadata(bytes, 1, 0)?;
            for part in &source.path {
                import_metadata(
                    bytes,
                    1,
                    match part {
                        ImportSegment::Name(n) => n.len(),
                        _ => 0,
                    },
                )?;
            }
            for name in &source.scope.module_path {
                import_metadata(bytes, 1, name.len())?;
            }
            if let crate::syntax::imports::ImportKind::Named { alias: Some(alias) } = &source.kind {
                import_metadata(bytes, 1, alias.len())?;
            }
            if let crate::syntax::outline::Visibility::Restricted(tokens) = &source.visibility {
                for token in tokens {
                    import_metadata(bytes, 1, token.text.len())?;
                }
            }
            Ok(())
        }
        // Explicit continuations resolve glob bases before testing the requested
        // module prefix. The active glob set cuts cycles without native recursion.
        enum Task {
            Expand {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
            },
            Search {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
                ancestry: BTreeSet<u32>,
            },
            Weak {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
                ancestry: BTreeSet<u32>,
                children: Vec<usize>,
            },
            Union {
                slot: usize,
                children: Vec<usize>,
            },
        }
        let edition = self.edition;
        let mut bytes = 0usize;
        charge_route(&mut bytes, source)?;
        let mut results: Vec<Vec<SourceImport>> = vec![Vec::new()];
        let mut tasks = vec![Task::Expand {
            slot: 0,
            scope,
            source: source.clone(),
            active: BTreeSet::new(),
        }];
        while let Some(task) = tasks.pop() {
            *work = work
                .checked_add(1)
                .ok_or_else(|| error("body glob route work limit"))?;
            import_metadata(&mut bytes, 1, 0)?;
            match task {
                Task::Expand {
                    slot,
                    scope,
                    source,
                    active,
                } => {
                    let routes = self.named_import_routes(scope, &source, work)?;
                    let mut children = Vec::new();
                    let mut pending = Vec::new();
                    for (scope, source) in routes {
                        charge_route(&mut bytes, &source)?;
                        let child = results.len();
                        import_metadata(&mut bytes, active.len() + 2, 0)?;
                        results.push(Vec::new());
                        children.push(child);
                        pending.push(Task::Search {
                            slot: child,
                            scope,
                            source,
                            active: active.clone(),
                            ancestry: BTreeSet::new(),
                        });
                    }
                    tasks.push(Task::Union { slot, children });
                    tasks.extend(pending);
                }
                Task::Union { slot, children } => {
                    for child in children {
                        let rows = std::mem::take(&mut results[child]);
                        import_metadata(&mut bytes, rows.len(), 0)?;
                        results[slot].extend(rows);
                    }
                }
                Task::Search {
                    slot,
                    scope,
                    source,
                    active,
                    mut ancestry,
                } => {
                    if !ancestry.insert(scope) {
                        return Err(error("body glob route ancestry limit"));
                    }
                    let first = if source.absolute {
                        None
                    } else {
                        source.path.first()
                    };
                    let Some(ImportSegment::Name(first)) = first else {
                        results[slot].push(source);
                        continue;
                    };
                    let name = MacroName::parse(first, edition)?;
                    if self.item_candidates.contains_key(&(
                        scope,
                        crate::source::manifest::Namespace::Type,
                        name,
                    )) {
                        results[slot].push(source);
                        continue;
                    }
                    let mut children = Vec::new();
                    let mut pending = Vec::new();
                    for index in self.glob_imports.get(&scope).into_iter().flatten() {
                        if active.contains(index) {
                            continue;
                        }
                        charge_route(&mut bytes, &self.facts.imports[*index].source)?;
                        let mut active = active.clone();
                        active.insert(*index);
                        import_metadata(&mut bytes, active.len() + 2, 0)?;
                        let child = results.len();
                        results.push(Vec::new());
                        children.push(child);
                        pending.push(Task::Expand {
                            slot: child,
                            scope,
                            source: self.facts.imports[*index].source.clone(),
                            active,
                        });
                    }
                    import_metadata(&mut bytes, ancestry.len() + active.len(), 0)?;
                    tasks.push(Task::Weak {
                        slot,
                        scope,
                        source,
                        active,
                        ancestry,
                        children,
                    });
                    tasks.extend(pending);
                }
                Task::Weak {
                    slot,
                    scope,
                    source,
                    active,
                    ancestry,
                    children,
                } => {
                    let Some(ImportSegment::Name(first)) = source.path.first() else {
                        return Err(error("body glob route continuation lost prefix"));
                    };
                    for child in children {
                        for mut candidate in std::mem::take(&mut results[child]) {
                            candidate.path.push(ImportSegment::Name(first.clone()));
                            if let crate::frontend::macro_names::ModuleLookup::Defined { .. } =
                                names.lookup_import_module(&candidate, edition)?
                            {
                                charge_route(&mut bytes, &source)?;
                                let mut next = source.clone();
                                next.path = candidate.path;
                                next.path.extend_from_slice(&source.path[1..]);
                                next.scope = candidate.scope;
                                next.absolute = candidate.absolute;
                                let text = next
                                    .path
                                    .iter()
                                    .try_fold(0usize, |n, part| {
                                        n.checked_add(match part {
                                            ImportSegment::Name(n) => n.len(),
                                            _ => 0,
                                        })
                                    })
                                    .ok_or_else(|| error("body glob route path size overflow"))?;
                                import_metadata(&mut bytes, next.path.len(), text)?;
                                results[slot].push(next);
                            }
                        }
                    }
                    if results[slot].is_empty() {
                        if let Some(parent) = self
                            .facts
                            .scopes
                            .iter()
                            .find(|s| s.ordinal == scope)
                            .ok_or_else(|| error("body glob route scope absent"))?
                            .parent
                        {
                            tasks.push(Task::Search {
                                slot,
                                scope: parent,
                                source,
                                active,
                                ancestry,
                            });
                        } else {
                            results[slot].push(source);
                        }
                    }
                }
            }
        }
        let mut seen = BTreeSet::new();
        Ok(std::mem::take(&mut results[0])
            .into_iter()
            .filter(|route| seen.insert(route.fingerprint()))
            .collect())
    }
    pub(super) fn named_import_routes(
        &self,
        scope: u32,
        source: &crate::syntax::imports::SourceImport,
        work: &mut usize,
    ) -> Result<Vec<(u32, crate::syntax::imports::SourceImport)>, Diagnostic> {
        use crate::syntax::imports::ImportSegment;
        let mut stack = vec![(scope, source.clone())];
        let mut seen = BTreeSet::new();
        let mut routes = Vec::new();
        let mut bytes = 0usize;
        while let Some((scope, source)) = stack.pop() {
            *work = work
                .checked_add(1)
                .ok_or_else(|| error("body module alias work limit"))?;
            let text = source
                .path
                .iter()
                .try_fold(0usize, |n, part| {
                    n.checked_add(match part {
                        ImportSegment::Name(n) => n.len(),
                        _ => 0,
                    })
                })
                .ok_or_else(|| error("body module alias path size overflow"))?;
            import_metadata(&mut bytes, source.path.len(), text)?;
            if !seen.insert((scope, source.fingerprint())) {
                continue;
            }
            let first = if source.absolute {
                None
            } else {
                source.path.first().and_then(|p| {
                    if let ImportSegment::Name(n) = p {
                        Some(n)
                    } else {
                        None
                    }
                })
            };
            let mut binding = None;
            if let Some(first) = first {
                let name = MacroName::parse(first, self.edition)?;
                let mut current = Some(scope);
                let mut ancestry = BTreeSet::new();
                while let Some(scope) = current {
                    if !ancestry.insert(scope) {
                        return Err(error("body module alias ancestry limit"));
                    }
                    if let Some(imports) = self.named_imports.get(&(scope, name.clone())) {
                        binding = Some((scope, imports));
                        break;
                    }
                    if self.item_candidates.contains_key(&(
                        scope,
                        crate::source::manifest::Namespace::Type,
                        name.clone(),
                    )) {
                        break;
                    }
                    current = self
                        .facts
                        .scopes
                        .iter()
                        .find(|s| s.ordinal == scope)
                        .ok_or_else(|| error("body module alias scope absent"))?
                        .parent;
                }
            }
            if let Some((scope, indices)) = binding {
                for index in indices {
                    let original = &self.facts.imports[*index].source;
                    let mut next = source.clone();
                    next.path = original.path.clone();
                    next.path.extend_from_slice(&source.path[1..]);
                    next.absolute = original.absolute;
                    next.scope = original.scope.clone();
                    let text = next
                        .path
                        .iter()
                        .try_fold(0usize, |n, part| {
                            n.checked_add(match part {
                                ImportSegment::Name(n) => n.len(),
                                _ => 0,
                            })
                        })
                        .ok_or_else(|| error("body module alias path size overflow"))?;
                    import_metadata(&mut bytes, next.path.len(), text)?;
                    stack.push((scope, next));
                }
            } else {
                routes.push((scope, source));
            }
        }
        Ok(routes)
    }
    pub(super) fn glob_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &impl crate::frontend::names::reads::QualifiedNames,
        work: &mut usize,
    ) -> Result<crate::frontend::names::NameLookup, Diagnostic> {
        use crate::frontend::names::NameLookup;
        let mut definitions = BTreeSet::new();
        let mut metadata = 0usize;
        for index in self.glob_imports.get(&scope).into_iter().flatten() {
            *work = work
                .checked_add(1)
                .ok_or_else(|| error("body glob work limit"))?;
            let import = &self.facts.imports[*index].source;
            for route in self.import_routes(scope, import, names, work)? {
                match names.lookup_glob_member(&route, name.as_str(), namespace, self.edition)? {
                    NameLookup::Defined(d) => {
                        import_metadata(&mut metadata, 1, 0)?;
                        definitions.insert(d);
                    }
                    NameLookup::Ambiguous(ds) => {
                        import_metadata(&mut metadata, ds.len(), 0)?;
                        definitions.extend(ds);
                    }
                    NameLookup::NeedInput(id) => return Ok(NameLookup::NeedInput(id)),
                    NameLookup::Absent => {}
                    NameLookup::Inaccessible(paths) => return Ok(NameLookup::Inaccessible(paths)),
                    NameLookup::AmbiguousModules(paths) => {
                        return Ok(NameLookup::AmbiguousModules(paths));
                    }
                    _ => return Err(error("unsupported body glob member outcome")),
                }
            }
        }
        Ok(if definitions.len() == 1 {
            NameLookup::Defined(definitions.pop_first().expect("one candidate"))
        } else if definitions.len() > 1 {
            NameLookup::Ambiguous(definitions.into_iter().collect())
        } else {
            NameLookup::Absent
        })
    }
    pub(super) fn import_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &impl crate::frontend::names::reads::QualifiedNames,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<crate::frontend::names::NameLookup, Diagnostic> {
        use crate::frontend::names::NameLookup;
        use crate::syntax::imports::ImportSegment;
        let mut linear = true;
        let mut scratch_bytes = 0usize;
        import_metadata(&mut scratch_bytes, 1, name.as_str().len())?;
        let mut stack = vec![(scope, name.clone())];
        let mut seen = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut demands = BTreeSet::new();
        let mut denied = Vec::new();
        let mut roots = BTreeSet::new();
        while let Some((mut scope, name)) = stack.pop() {
            if !seen.insert((scope, name.clone())) {
                linear = false;
                continue;
            }
            if let Some(found) = memo.entries.get(&(scope, namespace, name.clone())) {
                match found {
                    NameLookup::Defined(d) => {
                        definitions.insert(d.clone());
                    }
                    NameLookup::ModuleRoot(unit) => {
                        roots.insert(*unit);
                    }
                    NameLookup::Absent => {}
                    _ => return Err(error("invalid body import memo outcome")),
                }
                continue;
            }
            loop {
                import_metadata(&mut scratch_bytes, 1, name.as_str().len())?;
                *work = work
                    .checked_add(1)
                    .ok_or_else(|| error("body named import work limit"))?;
                if let Some(id) = self.item_frontiers.get(&scope).and_then(|ids| ids.first()) {
                    demands.insert(*id);
                    break;
                }
                let items = self.item_candidates.get(&(scope, namespace, name.clone()));
                let imports = self.named_imports.get(&(scope, name.clone()));
                if items.map_or(0, Vec::len) + imports.map_or(0, Vec::len) > 1 {
                    linear = false;
                }
                if let Some(items) = items {
                    definitions.extend(items.iter().cloned());
                }
                if let Some(imports) = imports {
                    for index in imports {
                        *work = work
                            .checked_add(1)
                            .ok_or_else(|| error("body named import work limit"))?;
                        let row = self
                            .facts
                            .imports
                            .get(*index)
                            .ok_or_else(|| error("body named import input absent"))?;
                        if !row.source.absolute
                            && row.source.path.len() > 1
                            && let Some(ImportSegment::Name(first)) = row.source.path.first()
                        {
                            let first = MacroName::parse(first, self.edition)?;
                            if !self.named_imports.contains_key(&(scope, first.clone()))
                                && !self.item_candidates.contains_key(&(
                                    scope,
                                    crate::source::manifest::Namespace::Type,
                                    first.clone(),
                                ))
                            {
                                match self.glob_lookup(
                                    scope,
                                    &first,
                                    crate::source::manifest::Namespace::Type,
                                    names,
                                    work,
                                )? {
                                    NameLookup::Ambiguous(ds) => {
                                        return Ok(NameLookup::Ambiguous(ds));
                                    }
                                    NameLookup::NeedInput(id) => {
                                        return Ok(NameLookup::NeedInput(id));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if !row.source.absolute
                            && let [ImportSegment::Name(target)] = row.source.path.as_slice()
                        {
                            import_metadata(&mut scratch_bytes, 1, target.len())?;
                            stack.push((scope, MacroName::parse(target, self.edition)?));
                            continue;
                        }
                        let routes = self.import_routes(scope, &row.source, names, work)?;
                        if routes.len() > 1 {
                            linear = false;
                        }
                        for route in routes {
                            match names.lookup_import_target(&route, namespace, self.edition)? {
                                NameLookup::Defined(d) => {
                                    definitions.insert(d);
                                }
                                NameLookup::Ambiguous(ds) => {
                                    import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                                    definitions.extend(ds);
                                }
                                NameLookup::NeedInput(id) => {
                                    demands.insert(id);
                                }
                                NameLookup::Inaccessible(paths) => {
                                    let text = paths
                                        .iter()
                                        .flat_map(|p| p.components())
                                        .try_fold(0usize, |n, name| {
                                            n.checked_add(name.as_str().len())
                                        })
                                        .ok_or_else(|| {
                                            error("body import denied path size overflow")
                                        })?;
                                    import_metadata(&mut scratch_bytes, paths.len(), text)?;
                                    denied.extend(paths);
                                }
                                NameLookup::ModuleRoot(unit) => {
                                    roots.insert(unit);
                                }
                                NameLookup::Absent => {}
                                NameLookup::AmbiguousModules(paths) => {
                                    return Ok(NameLookup::AmbiguousModules(paths));
                                }
                                NameLookup::LocalBinding(_) => {
                                    return Err(error(
                                        "body import target cannot be a local binding",
                                    ));
                                }
                            }
                        }
                    }
                }
                if items.is_some() || imports.is_some() {
                    break;
                }
                match self.glob_lookup(scope, &name, namespace, names, work)? {
                    NameLookup::Defined(d) => {
                        definitions.insert(d);
                        break;
                    }
                    NameLookup::Ambiguous(ds) => {
                        import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                        definitions.extend(ds);
                        break;
                    }
                    NameLookup::NeedInput(id) => {
                        demands.insert(id);
                        break;
                    }
                    NameLookup::Absent => {}
                    found => return Ok(found),
                }
                if let Some(parent) = self
                    .facts
                    .scopes
                    .iter()
                    .find(|s| s.ordinal == scope)
                    .ok_or_else(|| error("body import scope absent"))?
                    .parent
                {
                    scope = parent;
                } else {
                    match names.lookup_unqualified(
                        namespace,
                        name.as_str(),
                        self.module_scope,
                        self.edition,
                        None,
                    )? {
                        NameLookup::Defined(d) => {
                            definitions.insert(d);
                        }
                        NameLookup::NeedInput(id) => {
                            demands.insert(id);
                        }
                        NameLookup::Ambiguous(ds) => {
                            import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                            definitions.extend(ds);
                        }
                        NameLookup::Inaccessible(paths) => {
                            let text = paths
                                .iter()
                                .flat_map(|p| p.components())
                                .try_fold(0usize, |n, name| n.checked_add(name.as_str().len()))
                                .ok_or_else(|| error("body import denied path size overflow"))?;
                            import_metadata(&mut scratch_bytes, paths.len(), text)?;
                            denied.extend(paths);
                        }
                        NameLookup::ModuleRoot(unit) => {
                            roots.insert(unit);
                        }
                        NameLookup::Absent => {}
                        _ => return Err(error("unsupported body import target outcome")),
                    }
                    break;
                }
            }
        }
        let found = if let Some(id) = demands.first() {
            NameLookup::NeedInput(*id)
        } else if definitions.len() == 1 {
            NameLookup::Defined(definitions.pop_first().expect("one candidate"))
        } else if definitions.len() > 1 {
            NameLookup::Ambiguous(definitions.into_iter().collect())
        } else if let Some(unit) = roots.first() {
            NameLookup::ModuleRoot(*unit)
        } else if !denied.is_empty() {
            denied.sort();
            denied.dedup();
            NameLookup::Inaccessible(denied)
        } else {
            NameLookup::Absent
        };
        // A single-path traversal gives every visited alias the same outcome.
        // Branches and cycles cannot share this certificate.
        if linear
            && matches!(
                found,
                NameLookup::Defined(_) | NameLookup::Absent | NameLookup::ModuleRoot(_)
            )
        {
            for (scope, name) in seen {
                let key = (scope, namespace, name);
                if !memo.entries.contains_key(&key) {
                    import_metadata(&mut memo.bytes, 2, key.2.as_str().len())?;
                    memo.entries.insert(key, found.clone());
                }
            }
        }
        Ok(found)
    }
    pub(super) fn lookup_prefix<N: crate::frontend::names::reads::QualifiedNames>(
        &self,
        scope: u32,
        namespace: crate::source::manifest::Namespace,
        name: &MacroName,
        item_context: impl Fn() -> Result<NameContextKey, Diagnostic>,
        names: Option<&N>,
        evidence: Option<&owned_plan::LocalTypeNames>,
        value_binding: impl Fn(u32) -> Option<crate::frontend::names::NameLookup>,
    ) -> Result<PrefixResolution, Diagnostic> {
        use crate::frontend::names::NameLookup;
        let mut current = Some(scope);
        let mut seen = BTreeSet::new();
        let mut import_work = 0usize;
        let mut memo = ImportMemo::default();
        while let Some(ordinal) = current {
            if !seen.insert(ordinal) {
                return Err(error("ordinary body item ancestry limit"));
            }
            if namespace == crate::source::manifest::Namespace::Value {
                if let Some(found) = value_binding(ordinal) {
                    return Ok(PrefixResolution::Name(found));
                }
            }
            if let Some(producer) = self
                .item_frontiers
                .get(&ordinal)
                .and_then(|ids| ids.first())
            {
                return Ok(PrefixResolution::Name(NameLookup::NeedInput(*producer)));
            }
            for index in self.anonymous_imports.get(&ordinal).into_iter().flatten() {
                let Some(names) = names else {
                    return Ok(PrefixResolution::NeedImport(
                        self.facts.imports[*index].source.identity,
                    ));
                };
                use crate::frontend::names::AnonymousImportOutcome;
                match self.anonymous_import(ordinal, *index, names, &mut import_work, &mut memo)? {
                    AnonymousImportOutcome::Ready(_) => {}
                    AnonymousImportOutcome::Pending(id) => {
                        return Ok(PrefixResolution::Name(NameLookup::NeedInput(id)));
                    }
                    _ => {
                        return Ok(PrefixResolution::Name(NameLookup::NeedInput(
                            crate::frontend::names::NameNamespaces::import_identity(
                                &self.facts.imports[*index].source,
                            ),
                        )));
                    }
                }
            }
            for ((scope, alias), indices) in &self.named_imports {
                if *scope != ordinal {
                    continue;
                }
                let Some(names) = names else {
                    // Without the module graph a named import cannot be
                    // resolved, but it binds only its own alias: a lookup of
                    // any other name is unaffected by it.
                    if alias != name {
                        continue;
                    }
                    return Ok(PrefixResolution::NeedImport(
                        self.facts.imports[indices[0]].source.identity,
                    ));
                };
                let mut ready = false;
                for namespace in [
                    crate::source::manifest::Namespace::Value,
                    crate::source::manifest::Namespace::Type,
                    crate::source::manifest::Namespace::Macro,
                ] {
                    match self.import_lookup(
                        ordinal,
                        alias,
                        namespace,
                        names,
                        &mut import_work,
                        &mut memo,
                    )? {
                        NameLookup::Defined(_) | NameLookup::ModuleRoot(_) => ready = true,
                        NameLookup::NeedInput(id) => {
                            return Ok(PrefixResolution::Name(NameLookup::NeedInput(id)));
                        }
                        NameLookup::Ambiguous(ds) if alias == name && namespace == namespace => {
                            return Ok(PrefixResolution::Name(NameLookup::Ambiguous(ds)));
                        }
                        NameLookup::AmbiguousModules(paths)
                            if alias == name && namespace == namespace =>
                        {
                            return Ok(PrefixResolution::Name(NameLookup::AmbiguousModules(paths)));
                        }
                        _ => {}
                    }
                }
                if !ready {
                    let source = &self.facts.imports[indices[0]].source;
                    return Ok(PrefixResolution::Name(NameLookup::NeedInput(
                        crate::frontend::names::NameNamespaces::import_identity(source),
                    )));
                }
            }
            if self.named_imports.contains_key(&(ordinal, name.clone())) {
                let Some(names) = names else {
                    return Ok(PrefixResolution::NeedImport(
                        self.facts.imports[self.named_imports[&(ordinal, name.clone())][0]]
                            .source
                            .identity,
                    ));
                };
                let found = self.import_lookup(
                    ordinal,
                    name,
                    namespace,
                    names,
                    &mut import_work,
                    &mut memo,
                )?;
                if found != NameLookup::Absent {
                    return Ok(PrefixResolution::Name(found));
                }
            }
            if let Some(items) = self
                .item_candidates
                .get(&(ordinal, namespace, name.clone()))
            {
                if let Some(evidence) = evidence {
                    match evidence.items(items, item_context()?)? {
                        owned_plan::ItemNames::NeedDeclaration(identity) => {
                            return Ok(PrefixResolution::NeedDeclaration(identity));
                        }
                        owned_plan::ItemNames::Selected(Some(found)) => {
                            return Ok(PrefixResolution::Name(found));
                        }
                        owned_plan::ItemNames::Selected(None) => {}
                    }
                } else {
                    return Ok(PrefixResolution::Name(if items.len() == 1 {
                        NameLookup::Defined(items[0].clone())
                    } else {
                        NameLookup::Ambiguous(items.clone())
                    }));
                }
            }
            if let Some(names) = names {
                let glob = self.glob_lookup(ordinal, name, namespace, names, &mut import_work)?;
                if glob != NameLookup::Absent {
                    return Ok(PrefixResolution::Name(glob));
                }
            } else if let Some(index) = self
                .glob_imports
                .get(&ordinal)
                .and_then(|rows| rows.first())
            {
                return Ok(PrefixResolution::NeedImport(
                    self.facts.imports[*index].source.identity,
                ));
            }
            current = self
                .facts
                .scopes
                .iter()
                .find(|s| s.ordinal == ordinal)
                .ok_or_else(|| error("ordinary body item scope absent"))?
                .parent;
        }
        Ok(PrefixResolution::ModuleTail)
    }
}
