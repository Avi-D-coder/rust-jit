//! Owned module prefix resolution. Alias targets retain original import paths,
//! rather than flattening away privacy checks. Traversal is iterative.
#[path = "module_paths/retention.rs"]
mod retention;
use super::macro_names::MacroPath;
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::MacroName,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ModulePath {
    pub unit: Digest,
    pub names: Vec<MacroName>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_derived_globs_restores_quota_and_preserves_strong_aliases() {
        let unit = Digest([86; 32]);
        let root = ModulePath {
            unit,
            names: vec![],
        };
        let named = ModulePath {
            unit,
            names: vec![MacroName::parse("named", 2024).unwrap()],
        };
        let weak = ModulePath {
            unit,
            names: vec![MacroName::parse("weak", 2024).unwrap()],
        };
        let mut registry = ModulePaths::default();
        registry
            .declare(root.clone(), VisibilityScope::Public)
            .unwrap();
        registry
            .alias(named.clone(), root.clone(), VisibilityScope::Public)
            .unwrap();
        let base = registry.bytes;
        for _ in 0..4 {
            registry
                .glob(weak.clone(), root.clone(), VisibilityScope::Public)
                .unwrap();
            let charged = registry.bytes;
            assert!(charged > base);
            registry
                .glob(weak.clone(), root.clone(), VisibilityScope::Public)
                .unwrap();
            assert_eq!(registry.bytes, charged);
            registry.clear_globs().unwrap();
            assert_eq!(registry.bytes, base);
            assert_eq!(registry.weak_bytes, 0);
            assert!(!registry.contains(&weak));
            assert!(registry.has_strong(&named));
            assert!(
                matches!(registry.resolve_module(&named, None).unwrap(), ModuleResolution::Known { path, .. } if path==root)
            );
        }
    }

    #[test]
    fn module_lookup_accepts_the_full_depth_without_a_synthetic_member() {
        let unit = Digest([85; 32]);
        let mut registry = ModulePaths::default();
        let mut path = ModulePath {
            unit,
            names: vec![],
        };
        registry
            .declare(path.clone(), VisibilityScope::Public)
            .unwrap();
        for _ in 0..256 {
            path.names.push(MacroName::parse("m", 2024).unwrap());
            registry
                .declare(path.clone(), VisibilityScope::Public)
                .unwrap();
        }
        let caller = ScopeKey {
            crate_key: unit,
            module_path: vec![],
            owner: None,
        };
        match registry.resolve_module(&path, Some(&caller)).unwrap() {
            ModuleResolution::Known {
                path: selected,
                visibility,
            } => {
                assert_eq!(selected, path);
                assert_eq!(visibility, VisibilityScope::Public);
            }
            _ => panic!("the disclosed 256-component module was not selected"),
        }
    }
}
impl ModulePath {
    pub(crate) fn validate_components(names: &[MacroName]) -> Result<(), Diagnostic> {
        names
            .iter()
            .try_fold(std::mem::size_of::<ModulePath>(), |bytes, name| {
                bytes
                    .checked_add(std::mem::size_of::<MacroName>())
                    .and_then(|n| n.checked_add(name.as_str().len()))
            })
            .ok_or_else(|| error("module path metadata budget exceeded"))?;
        Ok(())
    }
}
pub(crate) enum ModuleResolution {
    Known {
        path: ModulePath,
        visibility: VisibilityScope,
    },
    Absent,
    Denied(ModulePath),
    Ambiguous(Vec<ModulePath>),
}
#[derive(Clone)]
struct Alias {
    target: ModulePath,
    visibility: VisibilityScope,
    origin: ScopeKey,
}
#[derive(Clone)]
enum Binding {
    Declared(VisibilityScope),
    Alias(Alias),
    // Only the ordinary source-import lane can collect competing routes.
    ImportedAliases(Vec<Alias>),
}
#[derive(Clone, Default)]
pub(crate) struct ModulePaths {
    bindings: BTreeMap<ModulePath, Binding>,
    weak: BTreeMap<ModulePath, BTreeMap<ModulePath, BTreeSet<VisibilityScope>>>,
    bytes: usize,
    weak_bytes: usize,
    imported_aliases: usize,
}
pub(crate) enum Resolution {
    Known(MacroPath),
    Absent,
    Denied(MacroPath),
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn origin(path: &ModulePath) -> ScopeKey {
    ScopeKey {
        crate_key: path.unit,
        module_path: path.names[..path.names.len().saturating_sub(1)]
            .iter()
            .map(|n| n.as_str().into())
            .collect(),
        owner: None,
    }
}
impl ModulePaths {
    fn charge(&mut self, path: &ModulePath) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(std::mem::size_of::<ModulePath>())
            .ok_or_else(|| error("module path metadata budget exceeded"))?;
        for name in &path.names {
            self.bytes = self
                .bytes
                .checked_add(name.as_str().len() + std::mem::size_of::<MacroName>())
                .ok_or_else(|| error("module path metadata budget exceeded"))?;
        }
        Ok(())
    }
    fn charge_visibility(&mut self, visibility: &VisibilityScope) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(std::mem::size_of::<Binding>())
            .ok_or_else(|| error("module path metadata budget exceeded"))?;
        if let VisibilityScope::Within { module_path, .. } = visibility {
            for name in module_path {
                self.bytes = self
                    .bytes
                    .checked_add(name.len() + std::mem::size_of::<String>())
                    .ok_or_else(|| error("module visibility metadata budget exceeded"))?;
            }
        }
        Ok(())
    }
    pub(crate) fn has_strong(&self, path: &ModulePath) -> bool {
        self.bindings.contains_key(path)
    }
    pub fn contains(&self, path: &ModulePath) -> bool {
        self.bindings.contains_key(path) || self.weak.contains_key(path)
    }
    pub fn declare(
        &mut self,
        path: ModulePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if !path.names.is_empty() {
            visibility.validate_for_declaration(&origin(&path))?;
        }
        if let Some(old) = self.bindings.get(&path) {
            return match old {
                Binding::Declared(old) if old == &visibility => Ok(()),
                _ => Err(error("conflicting module declaration provenance")),
            };
        }
        self.charge(&path)?;
        self.charge_visibility(&visibility)?;
        self.bindings.insert(path, Binding::Declared(visibility));
        Ok(())
    }
    /// An explicit source declaration can set the visibility of its matched extern-prelude edge.
    pub(crate) fn explicit_extern_alias(
        &mut self,
        path: ModulePath,
        target: ModulePath,
        visibility: VisibilityScope,
        prelude: bool,
    ) -> Result<(), Diagnostic> {
        visibility.validate_for_declaration(&origin(&path))?;
        let replace = prelude
            && path.names.len() == 1
            && matches!(self.bindings.get(&path), Some(Binding::Alias(old)) if old.target == target && old.origin.module_path.is_empty() && old.visibility == VisibilityScope::Within { unit: path.unit, module_path: vec![] } && old.visibility != visibility);
        if replace {
            self.charge_visibility(&visibility)?;
            let Some(Binding::Alias(old)) = self.bindings.get_mut(&path) else {
                return Err(error("extern prelude alias changed during publication"));
            };
            old.visibility = visibility;
            return Ok(());
        }
        self.alias(path, target, visibility)
    }
    pub fn alias(
        &mut self,
        path: ModulePath,
        target: ModulePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if path.names.is_empty() {
            return Err(error("module alias has no binding name"));
        }
        let origin = origin(&path);
        visibility.validate_for_declaration(&origin)?;
        if let Some(old) = self.bindings.get(&path) {
            return match old {
                Binding::Alias(old) if old.target == target && old.visibility == visibility => {
                    Ok(())
                }
                _ => Err(error("conflicting module alias provenance")),
            };
        }
        self.charge(&path)?;
        self.charge(&target)?;
        self.charge_visibility(&visibility)?;
        self.bytes = self
            .bytes
            .checked_add(std::mem::size_of::<ScopeKey>())
            .ok_or_else(|| error("module origin metadata budget exceeded"))?;
        for name in &origin.module_path {
            self.bytes = self
                .bytes
                .checked_add(name.len() + std::mem::size_of::<String>())
                .ok_or_else(|| error("module origin metadata budget exceeded"))?;
        }
        self.bindings.insert(
            path,
            Binding::Alias(Alias {
                target,
                visibility,
                origin,
            }),
        );
        Ok(())
    }
    /// Retain actual ordinary import routes without relaxing compiler aliases.
    /// The caller has already selected each source module and checked reexport
    /// width. The original route/origin remains necessary for subsequent walks.
    pub(crate) fn imported_alias(
        &mut self,
        path: ModulePath,
        target: ModulePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        ModulePath::validate_components(&path.names)?;
        ModulePath::validate_components(&target.names)?;
        if path.names.is_empty() {
            return Err(error("imported module alias has no binding name"));
        }
        let owner = origin(&path);
        visibility.validate_for_declaration(&owner)?;
        match self.bindings.get(&path) {
            Some(Binding::Declared(_)) | Some(Binding::Alias(_)) => {
                // Extern/compiler/source declaration corruption remains strict.
                return self.alias(path, target, visibility);
            }
            Some(Binding::ImportedAliases(aliases)) => {
                if aliases.iter().any(|old| {
                    old.target == target && old.visibility == visibility && old.origin == owner
                }) {
                    return Ok(());
                }
                if aliases.iter().any(|old| old.target == target) {
                    return Err(error("conflicting imported module alias provenance"));
                }
            }
            None => {}
        }
        let count = self
            .imported_aliases
            .checked_add(1)
            .ok_or_else(|| error("imported module alias count exceeded"))?;
        let mut budget = Self::default();
        budget.charge(&path)?;
        budget.charge(&target)?;
        budget.charge_visibility(&visibility)?;
        let mut added = budget
            .bytes
            .checked_add(std::mem::size_of::<Alias>() + std::mem::size_of::<ScopeKey>() + 128)
            .ok_or_else(|| error("imported module alias metadata overflow"))?;
        for name in &owner.module_path {
            added = added
                .checked_add(name.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
                .ok_or_else(|| error("imported module origin metadata overflow"))?;
        }
        let bytes = self
            .bytes
            .checked_add(added)
            .ok_or_else(|| error("imported module alias metadata budget exceeded"))?;
        let alias = Alias {
            target,
            visibility,
            origin: owner,
        };
        match self.bindings.get_mut(&path) {
            Some(Binding::ImportedAliases(aliases)) => {
                aliases
                    .try_reserve_exact(1)
                    .map_err(|_| error("imported module alias allocation failed"))?;
                aliases.push(alias);
                aliases.sort_by(|a, b| a.target.cmp(&b.target));
            }
            None => {
                let mut aliases = Vec::new();
                aliases
                    .try_reserve_exact(1)
                    .map_err(|_| error("imported module alias allocation failed"))?;
                aliases.push(alias);
                self.bindings
                    .insert(path, Binding::ImportedAliases(aliases));
            }
            _ => return Err(error("imported module alias changed during admission")),
        }
        self.bytes = bytes;
        self.imported_aliases = count;
        Ok(())
    }
    pub(crate) fn member_candidates(
        &self,
        path: &ModulePath,
        caller: &ScopeKey,
    ) -> Result<Vec<(ModulePath, VisibilityScope)>, Diagnostic> {
        if !self.bindings.contains_key(path)
            && let Some(members) = self.weak.get(path)
        {
            let mut result = Vec::new();
            let mut budget = ModulePaths::default();
            for (target, ranges) in members {
                for range in ranges {
                    if range.allows(caller)? {
                        budget.charge(target)?;
                        budget.charge_visibility(range)?;
                        result
                            .try_reserve(1)
                            .map_err(|_| error("module member candidate allocation failed"))?;
                        result.push((target.clone(), range.clone()));
                    }
                }
            }
            return Ok(result);
        }
        Ok(match self.resolve_module(path, Some(caller))? {
            ModuleResolution::Known { path, visibility } => vec![(path, visibility)],
            ModuleResolution::Denied(_) | ModuleResolution::Absent => vec![],
            ModuleResolution::Ambiguous(_) => {
                return Err(error("ambiguous explicit module member"));
            }
        })
    }
    pub(crate) fn glob(
        &mut self,
        path: ModulePath,
        target: ModulePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if path.names.is_empty() {
            return Err(error("glob module member has no name"));
        }
        visibility.validate_for_declaration(&origin(&path))?;
        if self.bindings.contains_key(&path) {
            return Ok(());
        }
        if self
            .weak
            .get(&path)
            .and_then(|members| members.get(&target))
            .is_some_and(|ranges| ranges.contains(&visibility))
        {
            return Ok(());
        }
        let before = self.bytes;
        self.charge(&path)?;
        self.charge(&target)?;
        self.charge_visibility(&visibility)?;
        self.weak_bytes = self
            .weak_bytes
            .checked_add(self.bytes - before)
            .ok_or_else(|| error("glob module metadata overflow"))?;
        self.weak
            .entry(path)
            .or_default()
            .entry(target)
            .or_default()
            .insert(visibility);
        Ok(())
    }
    pub(crate) fn clear_globs(&mut self) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_sub(self.weak_bytes)
            .ok_or_else(|| error("glob module quota accounting mismatch"))?;
        self.weak_bytes = 0;
        self.weak.clear();
        Ok(())
    }
    pub fn resolve(
        &self,
        path: &MacroPath,
        caller: Option<&ScopeKey>,
    ) -> Result<Resolution, Diagnostic> {
        self.resolve_observed(
            path,
            caller,
            &mut crate::frontend::names::reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn resolve_observed(
        &self,
        path: &MacroPath,
        caller: Option<&ScopeKey>,
        reads: &mut crate::frontend::names::reads::NamespaceReads,
    ) -> Result<Resolution, Diagnostic> {
        reads.scope(
            crate::source::manifest::Namespace::Type,
            &ScopeKey {
                crate_key: path.unit,
                module_path: vec![],
                owner: None,
            },
        )?;
        // Standalone macro-only indexes retain their pre-source-registry semantics.
        if self.bindings.is_empty() {
            return Ok(Resolution::Known(path.clone()));
        }
        let (last, names) = path
            .components()
            .split_last()
            .ok_or_else(|| error("empty qualified macro path"))?;
        let module = ModulePath {
            unit: path.unit,
            names: names.to_vec(),
        };
        match self.resolve_module_observed(&module, caller, reads)? {
            ModuleResolution::Known { path: module, .. } => {
                let mut names = module.names;
                names
                    .try_reserve(1)
                    .map_err(|_| error("qualified alias result allocation failed"))?;
                names.push(last.clone());
                Ok(Resolution::Known(MacroPath::new(module.unit, names)?))
            }
            ModuleResolution::Ambiguous(_) => Err(error("ambiguous module prefix")),
            ModuleResolution::Absent => Ok(Resolution::Absent),
            ModuleResolution::Denied(module) => {
                Ok(Resolution::Denied(if module.names.is_empty() {
                    path.clone()
                } else {
                    MacroPath::new(module.unit, module.names)?
                }))
            }
        }
    }
    pub(crate) fn resolve_module<'a>(
        &'a self,
        path: &'a ModulePath,
        caller: Option<&'a ScopeKey>,
    ) -> Result<ModuleResolution, Diagnostic> {
        self.resolve_module_observed(
            path,
            caller,
            &mut crate::frontend::names::reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn resolve_module_observed<'a>(
        &'a self,
        path: &'a ModulePath,
        caller: Option<&'a ScopeKey>,
        reads: &mut crate::frontend::names::reads::NamespaceReads,
    ) -> Result<ModuleResolution, Diagnostic> {
        ModulePath::validate_components(&path.names)?;
        let names = &path.names;
        static PUBLIC: VisibilityScope = VisibilityScope::Public;
        struct Frame<'a> {
            names: &'a [MacroName],
            position: usize,
            current: ModulePath,
            caller: Option<&'a ScopeKey>,
            required: Option<&'a VisibilityScope>,
            final_visibility: &'a VisibilityScope,
            active: Option<&'a ModulePath>,
        }
        let root = ModulePath {
            unit: path.unit,
            names: vec![],
        };
        reads.module(&root)?;
        if !self.bindings.contains_key(&root) {
            return Ok(ModuleResolution::Absent);
        }
        let mut frames = vec![Frame {
            names,
            position: 0,
            current: root,
            caller,
            required: None,
            final_visibility: &PUBLIC,
            active: None,
        }];
        let mut active = BTreeSet::new();
        let mut steps = 0usize;
        loop {
            steps = steps
                .checked_add(1)
                .ok_or_else(|| error("module path walk limit exceeded"))?;
            let frame = frames
                .last_mut()
                .ok_or_else(|| error("module path continuation missing"))?;
            if frame.position == frame.names.len() {
                if let Some(required) = frame.required
                    && !frame.final_visibility.permits_reexport_as(required)
                {
                    return Ok(ModuleResolution::Denied(path.clone()));
                }
                let finished = frames.pop().expect("checked frame");
                if let Some(key) = finished.active {
                    active.remove(key);
                }
                if let Some(parent) = frames.last_mut() {
                    parent.current = finished.current;
                    continue;
                }
                return Ok(ModuleResolution::Known {
                    path: finished.current,
                    visibility: finished.final_visibility.clone(),
                });
            }
            frame
                .current
                .names
                .try_reserve(1)
                .map_err(|_| error("module prefix allocation failed"))?;
            frame
                .current
                .names
                .push(frame.names[frame.position].clone());
            frame.position += 1;
            reads.module(&frame.current)?;
            let Some((key, binding)) = self.bindings.get_key_value(&frame.current) else {
                let Some(members) = self.weak.get(&frame.current) else {
                    return Ok(ModuleResolution::Absent);
                };
                let mut selected: BTreeMap<&ModulePath, &VisibilityScope> = BTreeMap::new();
                for (target, ranges) in members {
                    reads.module(target)?;
                    for range in ranges {
                        if let Some(caller) = frame.caller
                            && !range.allows(caller)?
                        {
                            continue;
                        }
                        let old = selected.entry(target).or_insert(range);
                        if range.permits_reexport_as(old) {
                            *old = range;
                        }
                    }
                }
                if selected.is_empty() {
                    return Ok(ModuleResolution::Denied(frame.current.clone()));
                }
                if selected.len() > 1 {
                    return Ok(ModuleResolution::Ambiguous(
                        selected.keys().map(|p| (*p).clone()).collect(),
                    ));
                }
                let (target, visibility) = selected.pop_first().expect("one selected member");
                frame.current = target.clone();
                frame.final_visibility = visibility;
                continue;
            };
            let alias = match binding {
                Binding::Declared(visibility) => {
                    if let Some(caller) = frame.caller
                        && !visibility.allows(caller)?
                    {
                        return Ok(ModuleResolution::Denied(key.clone()));
                    }
                    frame.final_visibility = visibility;
                    continue;
                }
                Binding::Alias(alias) => alias,
                Binding::ImportedAliases(aliases) => {
                    let mut selected = None;
                    let mut visible = Vec::new();
                    let mut budget = Self::default();
                    for alias in aliases {
                        reads.scope(crate::source::manifest::Namespace::Type, &alias.origin)?;
                        reads.module(&alias.target)?;
                        if let Some(caller) = frame.caller
                            && !alias.visibility.allows(caller)?
                        {
                            continue;
                        }
                        budget.charge(&alias.target)?;
                        visible.try_reserve_exact(1).map_err(|_| {
                            error("ambiguous imported module path allocation failed")
                        })?;
                        visible.push(alias.target.clone());
                        selected = Some(alias);
                    }
                    if visible.len() > 1 {
                        return Ok(ModuleResolution::Ambiguous(visible));
                    }
                    let Some(alias) = selected else {
                        return Ok(ModuleResolution::Denied(key.clone()));
                    };
                    alias
                }
            };
            reads.scope(crate::source::manifest::Namespace::Type, &alias.origin)?;
            reads.module(&alias.target)?;
            if let Some(caller) = frame.caller
                && !alias.visibility.allows(caller)?
            {
                return Ok(ModuleResolution::Denied(key.clone()));
            }
            if !active.insert(key) {
                return Err(error("module path alias cycle"));
            }
            reads.module(&ModulePath {
                unit: alias.target.unit,
                names: vec![],
            })?;
            if !self.bindings.contains_key(&ModulePath {
                unit: alias.target.unit,
                names: vec![],
            }) {
                return Ok(ModuleResolution::Absent);
            }
            frame.final_visibility = &alias.visibility;
            let checked = frame.caller.is_some();
            frames
                .try_reserve(1)
                .map_err(|_| error("module path frame allocation failed"))?;
            frames.push(Frame {
                names: &alias.target.names,
                position: 0,
                current: ModulePath {
                    unit: alias.target.unit,
                    names: vec![],
                },
                caller: checked.then_some(&alias.origin),
                required: checked.then_some(&alias.visibility),
                final_visibility: &PUBLIC,
                active: Some(key),
            });
        }
    }
}

#[cfg(test)]
#[path = "module_paths_import_tests.rs"]
mod module_paths_import_tests;
