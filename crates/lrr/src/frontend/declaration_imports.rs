//! Borrowed declaration-only import lookup. No body entry, parsing or effects.
use super::{
    body_imports::Candidates,
    macro_names::{
        DeclarationModuleBinding, DeclarationModuleOrigin, MacroNamespaces, MacroPath,
        ModuleLookup, PathLookup,
    },
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::{MacroName, ScopeKey as LexicalScope},
    syntax::{
        declaration_scan::{
            DeclarationLexicalScope, DeclarationProducer, DeclarationScan, DeclarationScopeKind,
            ScannedImport,
        },
        imports::{ImportKind, ImportSegment},
        outline::ScopeKey,
    },
};
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn charge(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .ok_or_else(|| fault("declaration import storage/work limit"))?;
    Ok(())
}
enum DeclarationModuleLookup {
    Module(ModuleLookup),
    NeedInput(Digest),
}
/// Pure ephemeral candidates: prefix ambiguity is not macro definition identity.
#[derive(Default)]
struct DeclarationCandidates {
    macros: Candidates,
    qualified_prefix_ambiguous: bool,
}
impl DeclarationCandidates {
    fn add(&mut self, result: PathLookup) -> Result<(), Diagnostic> {
        self.macros.add(result)
    }
    fn finish(self, strong: PathLookup) -> Result<PathLookup, Diagnostic> {
        let found = self.macros.finish(strong)?;
        // Retain pending and inaccessible outcomes before reporting the prefix.
        if self.qualified_prefix_ambiguous
            && matches!(
                found,
                PathLookup::Defined(_) | PathLookup::Ambiguous(_) | PathLookup::Absent
            )
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-AMBIGUOUS",
                "ambiguous declaration module prefix".into(),
            ));
        }
        Ok(found)
    }
}
pub struct DeclarationImportIndex<'a> {
    scan: &'a DeclarationScan,
    scopes: BTreeMap<LexicalScope, &'a DeclarationLexicalScope>,
    producers: BTreeMap<Digest, &'a DeclarationProducer>,
    named: BTreeMap<(LexicalScope, MacroName), Vec<usize>>,
    globs: BTreeMap<LexicalScope, Vec<usize>>,
    bytes: usize,
}
impl<'a> DeclarationImportIndex<'a> {
    pub fn new(scan: &'a DeclarationScan) -> Result<Self, Diagnostic> {
        let mut out = Self {
            scan,
            scopes: BTreeMap::new(),
            producers: BTreeMap::new(),
            named: BTreeMap::new(),
            globs: BTreeMap::new(),
            bytes: std::mem::size_of::<Self>(),
        };
        for scope in scan.lexical_scopes() {
            charge(&mut out.bytes, 128)?;
            if out.scopes.insert(scope.identity, scope).is_some() {
                return Err(fault("duplicate declaration lexical scope"));
            }
        }
        for scope in out.scopes.values() {
            if scope.parent.is_some_and(|p| !out.scopes.contains_key(&p)) {
                return Err(fault("declaration import parent scope absent"));
            }
        }
        for row in scan.producers() {
            charge(&mut out.bytes, 128)?;
            if !out.scopes.contains_key(&row.lexical_scope)
                || out.producers.insert(row.identity, row).is_some()
            {
                return Err(fault("declaration import producer scope/identity conflict"));
            }
        }
        let edition = scan.parse().profile.edition;
        for (index, row) in scan.imports().iter().enumerate() {
            if !out.scopes.contains_key(&row.lexical_scope) {
                return Err(fault("declaration import lexical attachment absent"));
            }
            let slots = match &row.source.kind {
                ImportKind::Anonymous => continue,
                ImportKind::Glob => out.globs.entry(row.lexical_scope).or_default(),
                ImportKind::Named { alias } => {
                    let spelling = alias
                        .as_deref()
                        .or_else(|| {
                            row.source.path.iter().rev().find_map(|s| {
                                if let ImportSegment::Name(n) = s {
                                    Some(n.as_str())
                                } else {
                                    None
                                }
                            })
                        })
                        .ok_or_else(|| fault("declaration import named leaf absent"))?;
                    charge(
                        &mut out.bytes,
                        spelling
                            .len()
                            .checked_add(160)
                            .ok_or_else(|| fault("declaration import spelling storage overflow"))?,
                    )?;
                    out.named
                        .entry((row.lexical_scope, MacroName::parse(spelling, edition)?))
                        .or_default()
                }
            };
            charge(&mut out.bytes, 32)?;
            slots
                .try_reserve_exact(1)
                .map_err(|_| fault("declaration import index allocation"))?;
            slots.push(index);
        }
        Ok(out)
    }
    pub fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn lookup(
        &self,
        producer: Digest,
        segments: &[ImportSegment],
        absolute: bool,
        names: &MacroNamespaces,
    ) -> Result<PathLookup, Diagnostic> {
        self.lookup_validated(producer, segments, absolute, names, |_| Ok(true))
    }
    /// The expansion boundary validates generated binding contexts using pinned facts.
    pub(crate) fn lookup_validated(
        &self,
        producer: Digest,
        segments: &[ImportSegment],
        absolute: bool,
        names: &MacroNamespaces,
        validate: impl Fn(&ScannedImport) -> Result<bool, Diagnostic>,
    ) -> Result<PathLookup, Diagnostic> {
        let row = self
            .producers
            .get(&producer)
            .ok_or_else(|| fault("declaration import caller producer absent"))?;
        if segments.is_empty() {
            return Err(fault("declaration import caller path limit"));
        }
        let initial_bytes = segments
            .iter()
            .try_fold(0usize, |n, part| {
                n.checked_add(std::mem::size_of::<ImportSegment>())
                    .and_then(|n| {
                        n.checked_add(if let ImportSegment::Name(name) = part {
                            name.len()
                        } else {
                            0
                        })
                    })
            })
            .ok_or_else(|| fault("declaration import supplied path storage overflow"))?;
        let mut bytes = 0;
        charge(&mut bytes, initial_bytes)?;
        let mut caller = row.scope.clone();
        caller.owner = None;
        let edition = self.scan.parse().profile.edition;
        if absolute || !matches!(segments.first(), Some(ImportSegment::Name(_))) {
            return names.lookup_invocation_segments(segments, absolute, &caller, edition);
        }
        let mut current = row.lexical_scope;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(segments.len())
            .map_err(|_| fault("declaration import supplied path allocation"))?;
        parts.extend_from_slice(segments);
        let mut reference = caller.clone();
        let mut imported = None;
        let mut reference_owner = row.scope.owner;
        let mut seen = BTreeSet::new();
        // An import selected twice along one alias chain is a cycle (rustc
        // leaves such imports indeterminate); this replaces a depth cut-off.
        let mut chain = BTreeSet::new();
        let mut weak = DeclarationCandidates::default();
        for _ in 0usize.. {
            let amount = parts
                .iter()
                .try_fold(96usize, |n, s| {
                    n.checked_add(std::mem::size_of::<ImportSegment>())
                        .and_then(|n| {
                            n.checked_add(if let ImportSegment::Name(name) = s {
                                name.len()
                            } else {
                                0
                            })
                        })
                })
                .ok_or_else(|| fault("declaration import path storage overflow"))?;
            charge(&mut bytes, amount)?;
            let mut state_parts = Vec::new();
            state_parts
                .try_reserve_exact(parts.len())
                .map_err(|_| fault("declaration import cycle state allocation"))?;
            state_parts.extend_from_slice(&parts);
            if !seen.insert((current, state_parts, imported)) {
                return Ok(PathLookup::NeedInput(imported.unwrap_or(producer)));
            }
            let Some(ImportSegment::Name(first)) = parts.first() else {
                return self.finish(&parts, false, (&reference, &caller), imported, names, weak);
            };
            charge(
                &mut bytes,
                first
                    .len()
                    .checked_add(96)
                    .ok_or_else(|| fault("declaration import name work overflow"))?,
            )?;
            let name = MacroName::parse(first, edition)?;
            let known_direct = parts.len() > 1
                && self.known_direct_prefix(&parts, &reference, names, &mut bytes)?;
            let mut weak_prefixes = BTreeSet::new();
            let mut unproven_direct_competition = false;
            let mut scope = Some(current);
            let mut selected = None;
            let mut selected_module: Option<&DeclarationModuleBinding> = None;
            for _ in 0usize.. {
                let Some(at) = scope else {
                    break;
                };
                charge(
                    &mut bytes,
                    name.as_str()
                        .len()
                        .checked_add(128)
                        .ok_or_else(|| fault("declaration import scope work overflow"))?,
                )?;
                if parts.len() > 1 {
                    let kind = self
                        .scopes
                        .get(&at)
                        .ok_or_else(|| fault("declaration import lexical parent missing"))?
                        .kind;
                    let owner = if kind == DeclarationScopeKind::Module {
                        None
                    } else {
                        reference_owner
                    };
                    let bindings =
                        names.declaration_module_bindings(at, &name, &reference, owner, |n| {
                            charge(&mut bytes, n)
                        })?;
                    for binding in &bindings {
                        if binding.pending.is_some()
                            || binding.scope_kind != kind
                            || matches!(
                                kind,
                                DeclarationScopeKind::Unattached
                                    | DeclarationScopeKind::AssociatedItems
                            )
                            || self.scan.generated_origin().is_some()
                            || matches!(binding.origin, DeclarationModuleOrigin::Unproven { .. })
                        {
                            return weak.finish(PathLookup::NeedInput(binding.demand()));
                        }
                    }
                    if let Some(binding) = bindings.first().copied() {
                        if bindings.len() > 1 {
                            weak.qualified_prefix_ambiguous = true;
                        }
                        if let Some(previous) = selected_module {
                            weak.qualified_prefix_ambiguous |=
                                previous.identity != binding.identity;
                        } else {
                            for (unit, components) in &weak_prefixes {
                                weak.qualified_prefix_ambiguous |= *unit != binding.route_unit
                                    || components != &binding.route_components;
                            }
                            selected_module = Some(binding);
                        }
                    }
                }
                if let Some(indices) = self.named.get(&(at, name.clone())) {
                    let mut matching = None;
                    for index in indices {
                        charge(&mut bytes, 128)?;
                        let candidate = &self.scan.imports()[*index];
                        // A use target does not resolve through the binding it introduces.
                        // A self leaf introduces only a module/type binding, not a macro.
                        if imported == Some(candidate.source.identity)
                            || (parts.len() == 1
                                && candidate.source.path.last() == Some(&ImportSegment::SelfModule))
                        {
                            continue;
                        }
                        if candidate.source.scope.module_path == reference.module_path
                            && matching.replace(candidate).is_some()
                        {
                            return Err(Diagnostic::new(
                                "LRR-E-MACRO-AMBIGUOUS",
                                "duplicate declaration named import binding".into(),
                            ));
                        }
                    }
                    if let Some(candidate) = matching {
                        if let Some(binding) = selected_module {
                            // Without a retained module identity for this named
                            // import, do not rank it against a generated module.
                            return weak.finish(PathLookup::NeedInput(
                                if binding.lexical_scope == at {
                                    binding.demand()
                                } else {
                                    candidate.source.identity
                                },
                            ));
                        }
                        selected = matching;
                        break;
                    }
                }
                if let Some(indices) = self.globs.get(&at) {
                    let mut first_prefix = None;
                    'globs: for index in indices {
                        charge(&mut bytes, 128)?;
                        let glob = &self.scan.imports()[*index];
                        if glob.source.scope.module_path != reference.module_path {
                            continue;
                        }
                        if !validate(glob)? {
                            weak.add(PathLookup::NeedInput(glob.source.identity))?;
                            continue;
                        }
                        if let Some(input) = glob.attribute_inputs.first() {
                            weak.add(PathLookup::NeedInput(*input))?;
                            continue;
                        }
                        let module = match self.glob_module(glob, names, &validate, &mut bytes)? {
                            DeclarationModuleLookup::NeedInput(input) => {
                                weak.add(PathLookup::NeedInput(input))?;
                                continue;
                            }
                            DeclarationModuleLookup::Module(module) => module,
                        };
                        match module {
                            ModuleLookup::Defined {
                                unit,
                                mut components,
                                ..
                            } => {
                                if selected_module.is_some_and(|binding| {
                                    binding.lexical_scope == at
                                        && binding.origin == DeclarationModuleOrigin::Original
                                }) {
                                    continue;
                                }
                                let mut valid = true;
                                for (position, part) in parts.iter().enumerate() {
                                    let ImportSegment::Name(part) = part else {
                                        valid = false;
                                        break;
                                    };
                                    charge(
                                        &mut bytes,
                                        part.len().checked_add(64).ok_or_else(|| {
                                            fault("declaration glob spelling storage overflow")
                                        })?,
                                    )?;
                                    components.push(MacroName::parse(part, edition)?);
                                    if parts.len() > 1 && position == 0 {
                                        charge(&mut bytes, Self::module_path_work(&components)?)?;
                                        match names.lookup_module_from(
                                            unit,
                                            &components,
                                            &caller,
                                        )? {
                                            ModuleLookup::Defined {
                                                unit, components, ..
                                            } => {
                                                let prefix = (unit, components);
                                                charge(
                                                    &mut bytes,
                                                    Self::module_path_work(&prefix.1)?,
                                                )?;
                                                weak_prefixes.insert(prefix.clone());
                                                if let Some(binding) = selected_module {
                                                    weak.qualified_prefix_ambiguous |=
                                                        binding.route_unit != prefix.0
                                                            || binding.route_components != prefix.1;
                                                } else if known_direct {
                                                    // Registry membership without lexical/origin
                                                    // proof cannot erase a competing glob prefix.
                                                    unproven_direct_competition = true;
                                                }
                                                if let Some(first) = &first_prefix {
                                                    weak.qualified_prefix_ambiguous |=
                                                        first != &prefix;
                                                } else {
                                                    charge(
                                                        &mut bytes,
                                                        Self::module_path_work(&prefix.1)?,
                                                    )?;
                                                    first_prefix = Some(prefix);
                                                }
                                            }
                                            ModuleLookup::Ambiguous(_) => {
                                                weak.qualified_prefix_ambiguous = true;
                                                continue 'globs;
                                            }
                                            // A private member cannot contribute through a glob.
                                            ModuleLookup::Inaccessible { .. } => continue 'globs,
                                            ModuleLookup::Absent => {}
                                        }
                                    }
                                }
                                if !valid {
                                    weak.add(PathLookup::NeedInput(glob.source.identity))?;
                                    continue;
                                }
                                let found = names
                                    .lookup_from(&MacroPath::new(unit, components)?, &caller)?;
                                if matches!(
                                    found,
                                    PathLookup::Defined(_) | PathLookup::Ambiguous(_)
                                ) && self.module_original_strong_binding(
                                    glob, &parts, &reference, names, &mut bytes,
                                )? {
                                    continue;
                                }
                                if !matches!(found, PathLookup::Inaccessible(_)) {
                                    weak.add(found)?;
                                }
                            }
                            ModuleLookup::Absent | ModuleLookup::Ambiguous(_) => {
                                weak.add(PathLookup::NeedInput(glob.source.identity))?
                            }
                            ModuleLookup::Inaccessible { unit, components } => {
                                let path = MacroPath::new(unit, components)?;
                                return Ok(PathLookup::Inaccessible(vec![path]));
                            }
                        }
                    }
                }
                if selected_module
                    .is_some_and(|binding| binding.origin == DeclarationModuleOrigin::Original)
                {
                    break;
                }
                scope = self
                    .scopes
                    .get(&at)
                    .ok_or_else(|| fault("declaration import lexical parent missing"))?
                    .parent;
            }
            if let Some(binding) = selected_module {
                return self.finish_module(
                    binding,
                    &parts[1..],
                    (&caller, names),
                    weak,
                    &mut bytes,
                );
            }
            if unproven_direct_competition {
                weak.add(PathLookup::NeedInput(producer))?;
            }
            let Some(selected) = selected else {
                return self.finish(&parts, false, (&reference, &caller), imported, names, weak);
            };
            if !chain.insert(selected.source.identity) {
                return Ok(PathLookup::NeedInput(imported.unwrap_or(producer)));
            }
            if !validate(selected)? {
                return Ok(PathLookup::NeedInput(selected.source.identity));
            }
            if let Some(input) = selected.attribute_inputs.first() {
                return Ok(PathLookup::NeedInput(*input));
            }
            let mut target = selected.source.path.as_slice();
            if target.len() > 1 && target.last() == Some(&ImportSegment::SelfModule) {
                target = &target[..target.len() - 1];
            }
            let len = target
                .len()
                .checked_add(parts.len() - 1)                .ok_or_else(|| fault("declaration alias route depth limit"))?;
            let copied_bytes = target
                .iter()
                .chain(parts[1..].iter())
                .try_fold(0usize, |n, s| {
                    n.checked_add(std::mem::size_of::<ImportSegment>())
                        .and_then(|n| {
                            n.checked_add(if let ImportSegment::Name(name) = s {
                                name.len()
                            } else {
                                0
                            })
                        })
                })
                .ok_or_else(|| fault("declaration alias storage overflow"))?;
            charge(&mut bytes, copied_bytes)?;
            let mut next = Vec::new();
            next.try_reserve_exact(len)
                .map_err(|_| fault("declaration alias route allocation"))?;
            next.extend_from_slice(target);
            next.extend_from_slice(&parts[1..]);
            parts = next;
            reference = selected.source.scope.clone();
            reference.owner = None;
            current = selected.lexical_scope;
            imported = Some(selected.source.identity);
            reference_owner = selected.source.scope.owner;
            if selected.source.absolute || !matches!(parts.first(), Some(ImportSegment::Name(_))) {
                return self.finish(
                    &parts,
                    selected.source.absolute,
                    (&reference, &caller),
                    imported,
                    names,
                    weak,
                );
            }
        }
        Ok(PathLookup::NeedInput(imported.unwrap_or(producer)))
    }
    /// Pure borrowed-fact routing for a glob target. It never enters bodies or expands.
    fn glob_module(
        &self,
        glob: &ScannedImport,
        names: &MacroNamespaces,
        validate: &impl Fn(&ScannedImport) -> Result<bool, Diagnostic>,
        bytes: &mut usize,
    ) -> Result<DeclarationModuleLookup, Diagnostic> {
        let edition = self.scan.parse().profile.edition;
        let mut reference = glob.source.scope.clone();
        reference.owner = None;
        let caller = reference.clone();
        let mut current = glob.lexical_scope;
        let mut absolute = glob.source.absolute;
        let mut parts = glob.source.path.as_slice();
        let mut owned;
        let mut imported = glob.source.identity;
        let mut seen = BTreeSet::new();
        // See `lookup`: a repeated selected import is an alias cycle.
        let mut chain = BTreeSet::new();
        for _ in 0usize.. {
            let path_bytes = parts
                .iter()
                .try_fold(96usize, |n, part| {
                    n.checked_add(std::mem::size_of::<ImportSegment>())
                        .and_then(|n| {
                            n.checked_add(if let ImportSegment::Name(name) = part {
                                name.len()
                            } else {
                                0
                            })
                        })
                })
                .ok_or_else(|| fault("declaration glob alias storage overflow"))?;
            charge(bytes, path_bytes)?;
            let mut state_parts = Vec::new();
            state_parts
                .try_reserve_exact(parts.len())
                .map_err(|_| fault("declaration glob alias state allocation"))?;
            state_parts.extend_from_slice(parts);
            if !seen.insert((current, state_parts, imported)) {
                return Ok(DeclarationModuleLookup::NeedInput(imported));
            }
            let mut selected = None;
            if !absolute && let Some(ImportSegment::Name(first)) = parts.first() {
                charge(
                    bytes,
                    first
                        .len()
                        .checked_add(96)
                        .ok_or_else(|| fault("declaration glob alias name work overflow"))?,
                )?;
                let name = MacroName::parse(first, edition)?;
                let mut scope = Some(current);
                for _ in 0usize.. {
                    let Some(at) = scope else {
                        break;
                    };
                    charge(
                        bytes,
                        name.as_str()
                            .len()
                            .checked_add(128)
                            .ok_or_else(|| fault("declaration glob alias scope work overflow"))?,
                    )?;
                    if let Some(indices) = self.named.get(&(at, name.clone())) {
                        for index in indices {
                            charge(bytes, 128)?;
                            let candidate = &self.scan.imports()[*index];
                            if candidate.source.identity == imported
                                || candidate.source.scope.module_path != reference.module_path
                            {
                                continue;
                            }
                            if selected.replace(candidate).is_some() {
                                return Err(Diagnostic::new(
                                    "LRR-E-MACRO-AMBIGUOUS",
                                    "duplicate declaration named import binding".into(),
                                ));
                            }
                        }
                        if selected.is_some() {
                            break;
                        }
                    }
                    scope = self
                        .scopes
                        .get(&at)
                        .ok_or_else(|| fault("declaration import lexical parent missing"))?
                        .parent;
                }
            }
            let Some(selected) = selected else {
                let Some(route) =
                    names.import_module_route(parts, absolute, &reference, edition)?
                else {
                    return Ok(DeclarationModuleLookup::NeedInput(imported));
                };
                let module = names.lookup_module_from(route.unit, &route.names, &reference)?;
                let module = match module {
                    ModuleLookup::Defined {
                        unit,
                        ref components,
                        ..
                    } => names.lookup_module_from(unit, components, &caller)?,
                    other => other,
                };
                return Ok(DeclarationModuleLookup::Module(module));
            };
            if !chain.insert(selected.source.identity) {
                return Ok(DeclarationModuleLookup::NeedInput(imported));
            }
            if !validate(selected)? {
                return Ok(DeclarationModuleLookup::NeedInput(selected.source.identity));
            }
            if let Some(input) = selected.attribute_inputs.first() {
                return Ok(DeclarationModuleLookup::NeedInput(*input));
            }
            let mut target = selected.source.path.as_slice();
            if target.len() > 1 && target.last() == Some(&ImportSegment::SelfModule) {
                target = &target[..target.len() - 1];
            }
            let len = target
                .len()
                .checked_add(parts.len() - 1)                .ok_or_else(|| fault("declaration glob alias route depth limit"))?;
            let copied_bytes = target
                .iter()
                .chain(parts[1..].iter())
                .try_fold(0usize, |n, s| {
                    n.checked_add(std::mem::size_of::<ImportSegment>())
                        .and_then(|n| {
                            n.checked_add(if let ImportSegment::Name(name) = s {
                                name.len()
                            } else {
                                0
                            })
                        })
                })
                .ok_or_else(|| fault("declaration glob alias storage overflow"))?;
            charge(bytes, copied_bytes)?;
            let mut next = Vec::new();
            next.try_reserve_exact(len)
                .map_err(|_| fault("declaration glob alias allocation"))?;
            next.extend_from_slice(target);
            next.extend_from_slice(&parts[1..]);
            owned = next;
            parts = &owned;
            absolute = selected.source.absolute;
            reference = selected.source.scope.clone();
            reference.owner = None;
            current = selected.lexical_scope;
            imported = selected.source.identity;
        }
        Ok(DeclarationModuleLookup::NeedInput(imported))
    }
    /// Pure member routing through the one authoritative module registry.
    fn finish_module(
        &self,
        binding: &DeclarationModuleBinding,
        tail: &[ImportSegment],
        context: (&ScopeKey, &MacroNamespaces),
        weak: DeclarationCandidates,
        bytes: &mut usize,
    ) -> Result<PathLookup, Diagnostic> {
        let (caller, names) = context;
        if weak.qualified_prefix_ambiguous {
            return weak.finish(PathLookup::Absent);
        }
        if !binding.canonical_attachment
            || binding.scope_kind != DeclarationScopeKind::Module
            || binding.scope.owner.is_some()
        {
            // The global authoritative registry cannot prove a block-owned
            // member route. Retain the exact source/producer demand.
            return weak.finish(PathLookup::NeedInput(binding.demand()));
        }
        if !names.declaration_module_route_unique(binding, |n| charge(bytes, n))? {
            return weak.finish(PathLookup::NeedInput(binding.demand()));
        }
        charge(bytes, Self::module_path_work(&binding.route_components)?)?;
        let route = names.lookup_module_from(
            binding.route_unit,
            &binding.route_components,
            &binding.scope,
        )?;
        let ModuleLookup::Defined {
            unit,
            mut components,
            ..
        } = route
        else {
            return match route {
                ModuleLookup::Inaccessible { unit, components } => weak.finish(
                    PathLookup::Inaccessible(vec![MacroPath::new(unit, components)?]),
                ),
                _ => weak.finish(PathLookup::NeedInput(binding.demand())),
            };
        };
        charge(bytes, Self::module_path_work(&components)?)?;
        match names.lookup_module_from(unit, &components, caller)? {
            ModuleLookup::Defined { .. } => {}
            ModuleLookup::Inaccessible { unit, components } => {
                return weak.finish(PathLookup::Inaccessible(vec![MacroPath::new(
                    unit, components,
                )?]));
            }
            _ => return weak.finish(PathLookup::NeedInput(binding.demand())),
        }
        for part in tail {
            let ImportSegment::Name(name) = part else {
                return weak.finish(PathLookup::NeedInput(binding.demand()));
            };
            charge(
                bytes,
                name.len()
                    .checked_add(64)
                    .ok_or_else(|| fault("declaration qualified module member work overflow"))?,
            )?;
            components
                .try_reserve_exact(1)
                .map_err(|_| fault("declaration qualified module member allocation"))?;
            components.push(MacroName::parse(name, self.scan.parse().profile.edition)?);
        }
        weak.finish(names.lookup_from(&MacroPath::new(unit, components)?, caller)?)
    }
    /// Pure charged route preparation; no graph mutation or query/effect.
    fn module_path_work(parts: &[MacroName]) -> Result<usize, Diagnostic> {
        parts
            .iter()
            .try_fold(128usize, |n, part| {
                n.checked_add(part.as_str().len())
                    .and_then(|n| n.checked_add(64))
            })
            .ok_or_else(|| fault("declaration module prefix work overflow"))
    }
    /// Pure disclosed membership check. Positive membership is not origin proof.
    fn known_direct_prefix(
        &self,
        parts: &[ImportSegment],
        reference: &ScopeKey,
        names: &MacroNamespaces,
        bytes: &mut usize,
    ) -> Result<bool, Diagnostic> {
        let work = reference
            .module_path
            .iter()
            .try_fold(128usize, |n, part| {
                n.checked_add(part.len()).and_then(|n| n.checked_add(64))
            })
            .ok_or_else(|| fault("declaration direct prefix work overflow"))?;
        let first = match &parts[0] {
            ImportSegment::Name(name) => name.len(),
            _ => 0,
        };
        let work = work
            .checked_add(first)
            .and_then(|n| n.checked_add(64))
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| fault("declaration direct prefix work overflow"))?;
        charge(bytes, work)?;
        let Some(route) = names.source_module_route(
            &parts[..1],
            false,
            reference,
            self.scan.parse().profile.edition,
        )?
        else {
            return Ok(false);
        };
        charge(bytes, Self::module_path_work(&route.names)?)?;
        Ok(!matches!(
            names.lookup_module_from(route.unit, &route.names, reference)?,
            ModuleLookup::Absent
        ))
    }
    /// Pure module priority proof from retained scope kind and original namespace facts.
    /// Unattached generated roots and lexical blocks cannot grant that priority.
    fn module_original_strong_binding(
        &self,
        glob: &ScannedImport,
        parts: &[ImportSegment],
        reference: &ScopeKey,
        names: &MacroNamespaces,
        bytes: &mut usize,
    ) -> Result<bool, Diagnostic> {
        if parts.len() != 1
            || self
                .scopes
                .get(&glob.lexical_scope)
                .ok_or_else(|| fault("declaration import lexical parent missing"))?
                .kind
                != DeclarationScopeKind::Module
        {
            return Ok(false);
        }
        let amount = reference
            .module_path
            .iter()
            .try_fold(128usize, |n, name| {
                n.checked_add(name.len()).and_then(|n| n.checked_add(64))
            })
            .ok_or_else(|| fault("declaration root binding storage overflow"))?;
        charge(bytes, amount)?;
        let Some(path) = names.source_segments_path(
            parts,
            false,
            reference,
            self.scan.parse().profile.edition,
        )?
        else {
            return Ok(false);
        };
        names.has_original_strong_binding(&path, reference, |amount| charge(bytes, amount))
    }
    fn finish(
        &self,
        parts: &[ImportSegment],
        absolute: bool,
        scopes: (&ScopeKey, &ScopeKey),
        imported: Option<Digest>,
        names: &MacroNamespaces,
        weak: DeclarationCandidates,
    ) -> Result<PathLookup, Diagnostic> {
        let (reference, caller) = scopes;
        let found = if let Some(input) = imported {
            match names.import_segments_path(
                parts,
                absolute,
                reference,
                self.scan.parse().profile.edition,
            )? {
                Some(path) => match names.lookup_from(&path, reference)? {
                    PathLookup::Absent => PathLookup::NeedInput(input),
                    denied @ PathLookup::Inaccessible(_) => denied,
                    _ => names.lookup_from(&path, caller)?,
                },
                None => PathLookup::NeedInput(input),
            }
        } else {
            names.lookup_invocation_segments(
                parts,
                absolute,
                caller,
                self.scan.parse().profile.edition,
            )?
        };
        weak.finish(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        expand::scopes::MacroDefinition,
        frontend::module_paths::ModulePath,
        source::snapshot::{CfgSet, SourceBlob, SourcePath},
        syntax::{
            declaration_scan::DeclarationScanLimits, parse::ParsedFile, visibility::VisibilityScope,
        },
    };

    // LRR adaptations of rustc import semantics; no native macro is prepared or executed.
    // Primary sources: tests/ui/self/self-shadowing-import.rs and
    // tests/ui/macros/use-macro-self.rs at ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
    fn scan(source: &str, unit: Digest) -> DeclarationScan {
        ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
        .scan_declarations(
            SourcePath::new("edge.rs").unwrap(),
            ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            &CfgSet::default(),
            DeclarationScanLimits::default(),
        )
        .unwrap()
    }
    fn names(unit: Digest) -> (MacroNamespaces, MacroDefinition, MacroDefinition) {
        let mut names = MacroNamespaces::default();
        for components in [
            vec![],
            vec![MacroName::parse("dep", 2024).unwrap()],
            vec![MacroName::parse("other", 2024).unwrap()],
        ] {
            names
                .module_paths
                .declare(
                    ModulePath {
                        unit,
                        names: components,
                    },
                    VisibilityScope::Public,
                )
                .unwrap();
        }
        let definition = |byte| MacroDefinition {
            identity: Digest([byte; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let dep = definition(201);
        let other = definition(202);
        for (module, definition) in [("dep", dep.clone()), ("other", other.clone())] {
            names
                .define(
                    MacroPath::new(
                        unit,
                        vec![
                            MacroName::parse(module, 2024).unwrap(),
                            MacroName::parse("m", 2024).unwrap(),
                        ],
                    )
                    .unwrap(),
                    definition,
                )
                .unwrap();
        }
        (names, dep, other)
    }
    fn lookup(scan: &DeclarationScan, names: &MacroNamespaces, path: &[&str]) -> PathLookup {
        let producer = scan.producers().last().unwrap().identity;
        DeclarationImportIndex::new(scan)
            .unwrap()
            .lookup(
                producer,
                &path
                    .iter()
                    .map(|s| ImportSegment::Name((*s).into()))
                    .collect::<Vec<_>>(),
                false,
                names,
            )
            .unwrap()
    }
    // Pure unchanged-source adaptation to LRR's fixed 2024 profile.
    // Original compiletest default-2015 acceptance remains unexecuted.
    fn upstream_parsed(source: &str) -> ParsedFile {
        ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
    }
    // Pure source/hygiene catalog construction, with no procedural execution.
    fn upstream_catalog(
        parsed: &ParsedFile,
        unit: Digest,
        dependencies: &BTreeMap<MacroName, Digest>,
    ) -> crate::frontend::source_macro_routes::SourceMacroRoutes {
        use crate::{
            frontend::{hygiene::HygieneFacts, source_macro_routes::SourceMacroRoutes},
            syntax::token_wire::{
                ContextRecord, Edition, Key, NameContextKey, NameResolutionContext,
                ResolutionContext, Transparency,
            },
        };
        let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
        let context = ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key(unit.0),
            edition: Edition::E2024,
        };
        let root = context.key();
        let facts = HygieneFacts::new(
            vec![context],
            vec![ResolutionContext {
                original: root,
                item: root,
                local: root,
            }],
        )
        .unwrap()
        .with_name_contexts(vec![NameResolutionContext {
            original: root,
            item: NameContextKey(root),
            local: NameContextKey(root),
        }])
        .unwrap();
        SourceMacroRoutes::from_outline(&outline, root, facts, dependencies).unwrap()
    }
    // Scoped namespace adaptation of rustc duplicate.rs: preserve its module
    // headers and use leaves; replace only the selected value call in harness
    // text with a cold attribute so declaration lookup can observe the prefix.
    fn upstream_qualified_prefix_fixture(
        observation: &str,
    ) -> (DeclarationScan, MacroNamespaces, MacroDefinition) {
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/duplicate.rs"
        );
        assert_eq!(CASE.matches("        foo::bar();").count(), 1);
        let source = CASE.replacen("        foo::bar();", observation, 1);
        let unit = Digest([230; 32]);
        let outline = upstream_parsed(CASE)
            .outline(unit, vec![], &CfgSet::default())
            .unwrap();
        let scan = scan(&source, unit);
        let mut names = MacroNamespaces::default();
        let path = |parts: &[&str]| ModulePath {
            unit,
            names: parts
                .iter()
                .map(|s| MacroName::parse(s, 2024).unwrap())
                .collect(),
        };
        for parts in [
            vec![],
            vec!["ambiguous_module_errors"],
            vec!["ambiguous_module_errors", "m1"],
            vec!["ambiguous_module_errors", "m2"],
        ] {
            names
                .module_paths
                .declare(path(&parts), VisibilityScope::Public)
                .unwrap();
        }
        // These exact self aliases belong to the two unchanged source modules.
        for (module, target) in [("m1", "m1"), ("m2", "m2")] {
            assert!(outline.imports.iter().any(|row| {
                row.scope.module_path == ["ambiguous_module_errors", module]
                    && row.path == [ImportSegment::Super, ImportSegment::Name(target.into())]
                    && row.kind
                        == ImportKind::Named {
                            alias: Some("foo".into()),
                        }
            }));
            names
                .module_paths
                .alias(
                    path(&["ambiguous_module_errors", module, "foo"]),
                    path(&["ambiguous_module_errors", target]),
                    VisibilityScope::Public,
                )
                .unwrap();
        }
        let header = outline
            .items
            .iter()
            .find(|row| {
                row.def_key.module_path == ["ambiguous_module_errors", "m1"]
                    && row.def_key.declared_name == "bar"
            })
            .unwrap();
        // LRR-specific macro namespace sentinel carries the unchanged source
        // member identity. No value function body or macro body is executed.
        let definition = MacroDefinition {
            identity: header.def_key.fingerprint(),
            defining_unit: unit,
            definition_edition: 2024,
        };
        names
            .define(
                MacroPath::new(unit, path(&["ambiguous_module_errors", "m1", "bar"]).names)
                    .unwrap(),
                definition.clone(),
            )
            .unwrap();
        (scan, names, definition)
    }
    #[test]
    fn upstream_qualified_glob_prefix_is_ambiguous_before_member_lookup() {
        let (scan, names, _) =
            upstream_qualified_prefix_fixture("        #[foo::bar] fn cold_probe() {}");
        let producer = scan.producers().last().unwrap();
        assert_eq!(producer.scope.module_path, ["ambiguous_module_errors"]);
        let result = DeclarationImportIndex::new(&scan).unwrap().lookup(
            producer.identity,
            &[
                ImportSegment::Name("foo".into()),
                ImportSegment::Name("bar".into()),
            ],
            false,
            &names,
        );
        let diagnostic =
            result.expect_err("ambiguous module foo cannot be selected using member bar");
        assert_eq!(diagnostic.code, "LRR-E-MACRO-AMBIGUOUS");
    }
    #[test]
    fn upstream_qualified_prefix_conflict_survives_absent_and_shared_members() {
        let (scan, mut names, definition) =
            upstream_qualified_prefix_fixture("        #[foo::bar] fn cold_probe() {}");
        let producer = scan.producers().last().unwrap().identity;
        let index = DeclarationImportIndex::new(&scan).unwrap();
        let path = |last: &str| {
            [
                ImportSegment::Name("foo".into()),
                ImportSegment::Name(last.into()),
            ]
        };
        assert_eq!(
            index
                .lookup(producer, &path("absent"), false, &names)
                .unwrap_err()
                .code,
            "LRR-E-MACRO-AMBIGUOUS"
        );
        names
            .define(
                MacroPath::new(
                    scan.scope().crate_key,
                    ["ambiguous_module_errors", "m2", "bar"]
                        .iter()
                        .map(|s| MacroName::parse(s, 2024).unwrap())
                        .collect(),
                )
                .unwrap(),
                definition,
            )
            .unwrap();
        assert_eq!(
            index
                .lookup(producer, &path("bar"), false, &names)
                .unwrap_err()
                .code,
            "LRR-E-MACRO-AMBIGUOUS"
        );
        // Checking only the full prefix foo::nested would incorrectly select
        // m1 here; ambiguity already occurs at the first component foo.
        names
            .module_paths
            .declare(
                ModulePath {
                    unit: scan.scope().crate_key,
                    names: ["ambiguous_module_errors", "m1", "nested"]
                        .iter()
                        .map(|s| MacroName::parse(s, 2024).unwrap())
                        .collect(),
                },
                VisibilityScope::Public,
            )
            .unwrap();
        assert_eq!(
            index
                .lookup(
                    producer,
                    &[
                        ImportSegment::Name("foo".into()),
                        ImportSegment::Name("nested".into()),
                        ImportSegment::Name("absent".into()),
                    ],
                    false,
                    &names
                )
                .unwrap_err()
                .code,
            "LRR-E-MACRO-AMBIGUOUS"
        );
    }
    #[test]
    fn upstream_qualified_prefix_keeps_pending_and_named_override() {
        let (scan, names, _) = upstream_qualified_prefix_fixture(
            "        #[pending] use self::m1::*; #[foo::bar] fn cold_probe() {}",
        );
        let pending = scan
            .imports()
            .iter()
            .find_map(|row| row.attribute_inputs.first())
            .copied()
            .unwrap();
        assert_eq!(
            lookup(&scan, &names, &["foo", "bar"]),
            PathLookup::NeedInput(pending)
        );
        let (scan, names, definition) = upstream_qualified_prefix_fixture(
            "        use self::m1::foo; #[foo::bar] fn cold_probe() {}",
        );
        assert_eq!(
            lookup(&scan, &names, &["foo", "bar"]),
            PathLookup::Defined(definition)
        );
    }
    #[test]
    fn qualified_prefix_candidates_preserve_input_and_inaccessible_precedence() {
        let pending = Digest([231; 32]);
        let mut candidates = DeclarationCandidates {
            macros: Candidates::default(),
            qualified_prefix_ambiguous: true,
        };
        candidates.add(PathLookup::NeedInput(pending)).unwrap();
        assert_eq!(
            candidates.finish(PathLookup::Absent).unwrap(),
            PathLookup::NeedInput(pending)
        );
        let denied = PathLookup::Inaccessible(vec![
            MacroPath::new(
                Digest([232; 32]),
                vec![MacroName::parse("private", 2024).unwrap()],
            )
            .unwrap(),
        ]);
        let candidates = DeclarationCandidates {
            macros: Candidates::default(),
            qualified_prefix_ambiguous: true,
        };
        assert_eq!(candidates.finish(denied.clone()).unwrap(), denied);
    }
    fn upstream_expanded_module_prefix_fixture() -> (DeclarationScan, MacroNamespaces) {
        use crate::frontend::source_macro_routes::SourceMacroRoutes;
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/macro-paths.rs"
        );
        const AUX: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/auxiliary/two_macros.rs"
        );
        let consumer = Digest([233; 32]);
        let provider = Digest([234; 32]);
        let parsed = upstream_parsed(CASE);
        let auxiliary = upstream_parsed(AUX);
        let mut names = SourceMacroRoutes::candidate_graph(&[
            upstream_catalog(
                &parsed,
                consumer,
                &BTreeMap::from([(MacroName::parse("two_macros", 2024).unwrap(), provider)]),
            ),
            upstream_catalog(&auxiliary, provider, &BTreeMap::new()),
        ])
        .unwrap();
        let scan = scan(CASE, consumer);
        // Pure exact argument-header capture; no expansion is executed and
        // block modules are never published into the global registry.
        let original = scan
            .modules()
            .iter()
            .position(|row| row.name == "baz" && row.scope.module_path.is_empty())
            .unwrap();
        names
            .declare_scoped_module_binding(
                DeclarationModuleBinding::from_scan(
                    &scan,
                    original,
                    DeclarationModuleOrigin::Original,
                )
                .unwrap(),
            )
            .unwrap();
        for producer in scan.producers() {
            let text = &CASE[producer.range.start as usize..producer.range.end as usize];
            let name = if text.starts_with("bar::m!") {
                "bar"
            } else if text.starts_with("baz::m!") {
                "baz"
            } else {
                continue;
            };
            let fragment = &text[text.find('{').unwrap() + 1..text.rfind('}').unwrap()];
            let kind = scan
                .lexical_scopes()
                .iter()
                .find(|row| row.identity == producer.lexical_scope)
                .unwrap()
                .kind;
            let generated = crate::syntax::declaration_scan::generated_declaration_fixture(
                fragment,
                crate::expand::identity::GrammarSlot::Items,
            );
            let captured = generated
                .scan_declarations_at_root(
                    scan.path().clone(),
                    producer.scope.clone(),
                    &CfgSet::default(),
                    DeclarationScanLimits::default(),
                    kind,
                )
                .unwrap();
            assert_eq!(captured.scope(), &producer.scope);
            assert_eq!(captured.lexical_scopes()[0].kind, kind);
            let index = captured
                .modules()
                .iter()
                .position(|row| row.name == name)
                .unwrap();
            assert_eq!(
                captured.modules()[index].lexical_scope,
                captured.lexical_scopes()[0].identity
            );
            let binding = DeclarationModuleBinding::from_generated_scan(
                &scan,
                producer.identity,
                &captured,
                index,
                crate::syntax::token_wire::Key(producer.identity.0),
            )
            .unwrap();
            assert_eq!(binding.lexical_scope, producer.lexical_scope);
            names.declare_scoped_module_binding(binding).unwrap();
        }
        (scan, names)
    }
    #[test]
    fn upstream_generated_module_prefix_is_ambiguous_with_less_expanded_glob() {
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/macro-paths.rs"
        );
        let (scan, names) = upstream_expanded_module_prefix_fixture();
        let producer = scan
            .producers()
            .iter()
            .find(|row| {
                CASE.get(row.range.start as usize..row.range.end as usize)
                    .is_some_and(|text| text.starts_with("bar::m!"))
            })
            .unwrap();
        let result = DeclarationImportIndex::new(&scan).unwrap().lookup(
            producer.identity,
            &[
                ImportSegment::Name("bar".into()),
                ImportSegment::Name("m".into()),
            ],
            false,
            &names,
        );
        assert_eq!(result.expect_err("generated bar and original glob prefix conflict even with identical macro targets").code, "LRR-E-MACRO-AMBIGUOUS");
    }
    #[test]
    fn upstream_generated_module_prefix_is_ambiguous_with_original_outer_module() {
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/macro-paths.rs"
        );
        let (scan, names) = upstream_expanded_module_prefix_fixture();
        let producer = scan
            .producers()
            .iter()
            .find(|row| {
                CASE.get(row.range.start as usize..row.range.end as usize)
                    .is_some_and(|text| text.starts_with("baz::m!"))
            })
            .unwrap();
        let index = DeclarationImportIndex::new(&scan).unwrap();
        assert_eq!(
            index
                .lookup(
                    producer.identity,
                    &[
                        ImportSegment::Name("baz".into()),
                        ImportSegment::Name("m".into())
                    ],
                    false,
                    &names
                )
                .unwrap_err()
                .code,
            "LRR-E-MACRO-AMBIGUOUS"
        );
        // The upstream diagnostic explicitly suggests crate::baz to select
        // the original module rather than the expanded block module.
        let expected = names
            .lookup_invocation_segments(
                &[
                    ImportSegment::Crate,
                    ImportSegment::Name("baz".into()),
                    ImportSegment::Name("m".into()),
                ],
                false,
                &producer.scope,
                2024,
            )
            .unwrap();
        assert!(matches!(expected, PathLookup::Defined(_)));
        assert_eq!(
            index
                .lookup(
                    producer.identity,
                    &[
                        ImportSegment::Crate,
                        ImportSegment::Name("baz".into()),
                        ImportSegment::Name("m".into())
                    ],
                    false,
                    &names
                )
                .unwrap(),
            expected
        );
        assert_eq!(
            index
                .lookup(
                    producer.identity,
                    &[
                        ImportSegment::Crate,
                        ImportSegment::Name("bar".into()),
                        ImportSegment::Name("m".into())
                    ],
                    false,
                    &names
                )
                .unwrap(),
            PathLookup::Absent
        );
    }
    #[test]
    fn original_module_priority_requires_registered_origin_and_keeps_glob_inputs() {
        use crate::frontend::source_macro_routes::SourceMacroRoutes;
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/macro-paths.rs"
        );
        const AUX: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/auxiliary/two_macros.rs"
        );
        // Harness-only relocation of the fixture's exact module argument and
        // glob into the same original root scope; licensed input stays unchanged.
        assert_eq!(CASE.matches("fn f() {").count(), 1);
        assert_eq!(CASE.matches("    use foo::*;").count(), 1);
        let source = CASE
            .replacen(
                "fn f() {",
                "mod bar { pub use two_macros::m; }\nuse foo::*;\nfn f() {",
                1,
            )
            .replacen("    use foo::*;", "", 1);
        let consumer = Digest([235; 32]);
        let provider = Digest([236; 32]);
        let auxiliary = upstream_parsed(AUX);
        let graph = |source: &str| {
            SourceMacroRoutes::discovery_graph(&[
                std::sync::Arc::new(upstream_catalog(
                    &upstream_parsed(source),
                    consumer,
                    &BTreeMap::from([(MacroName::parse("two_macros", 2024).unwrap(), provider)]),
                )),
                std::sync::Arc::new(upstream_catalog(&auxiliary, provider, &BTreeMap::new())),
            ])
            .unwrap()
        };
        let register = |source: &str, scan: &DeclarationScan, names: &mut MacroNamespaces| {
            let index = scan
                .modules()
                .iter()
                .position(|row| row.name == "bar" && row.scope.module_path.is_empty())
                .unwrap();
            let row = &scan.modules()[index];
            assert!(
                source
                    .get(row.range.start as usize..row.range.end as usize)
                    .unwrap()
                    .starts_with("mod bar")
            );
            names
                .declare_scoped_module_binding(
                    DeclarationModuleBinding::from_scan(
                        scan,
                        index,
                        DeclarationModuleOrigin::Original,
                    )
                    .unwrap(),
                )
                .unwrap();
        };
        let caller = |scan: &DeclarationScan, source: &str| {
            scan.producers()
                .iter()
                .find(|row| {
                    source
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("bar::m!"))
                })
                .unwrap()
                .identity
        };
        let source_scan = scan(&source, consumer);
        let producer = caller(&source_scan, &source);
        let path = [
            ImportSegment::Name("bar".into()),
            ImportSegment::Name("m".into()),
        ];
        let mut names = graph(&source);
        let index = DeclarationImportIndex::new(&source_scan).unwrap();
        assert_eq!(
            index.lookup(producer, &path, false, &names).unwrap(),
            PathLookup::NeedInput(producer)
        );
        register(&source, &source_scan, &mut names);
        let auxiliary_outline = auxiliary
            .outline(provider, vec![], &CfgSet::default())
            .unwrap();
        let definition = MacroDefinition {
            identity: auxiliary_outline
                .items
                .iter()
                .find(|row| row.def_key.declared_name == "m")
                .unwrap()
                .def_key
                .fingerprint(),
            defining_unit: provider,
            definition_edition: 2024,
        };
        assert_eq!(
            index.lookup(producer, &path, false, &names).unwrap(),
            PathLookup::Defined(definition)
        );
        let pending_source = source.replacen("use foo::*;", "#[pending] use foo::*;", 1);
        let pending_scan = scan(&pending_source, consumer);
        // Disclosed ready namespace facts are unchanged by an unexecuted
        // annotation. Its exact pending source demand remains in the new scan;
        // no complete candidate graph is claimed for that annotated source.
        let mut pending_names = graph(&source);
        register(&pending_source, &pending_scan, &mut pending_names);
        let pending = *pending_scan
            .imports()
            .iter()
            .find_map(|row| row.attribute_inputs.first())
            .unwrap();
        assert_eq!(
            DeclarationImportIndex::new(&pending_scan)
                .unwrap()
                .lookup(
                    caller(&pending_scan, &pending_source),
                    &path,
                    false,
                    &pending_names
                )
                .unwrap(),
            PathLookup::NeedInput(pending)
        );
    }
    #[test]
    fn scoped_module_binding_does_not_publish_or_leak_to_sibling_blocks() {
        let unit = Digest([237; 32]);
        let scan = scan(
            "fn cold(){ {#[local::m] fn a(){}} {#[local::m] fn b(){}} }",
            unit,
        );
        let first = &scan.producers()[0];
        let second = &scan.producers()[1];
        assert_eq!(first.scope.owner, second.scope.owner);
        assert_ne!(first.lexical_scope, second.lexical_scope);
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        names
            .declare_scoped_module_binding(DeclarationModuleBinding {
                identity: Digest([238; 32]),
                name: MacroName::parse("local", 2024).unwrap(),
                scope: first.scope.clone(),
                lexical_scope: first.lexical_scope,
                scope_kind: DeclarationScopeKind::Block,
                canonical_attachment: false,
                route_unit: unit,
                route_components: vec![MacroName::parse("local", 2024).unwrap()],
                visibility: VisibilityScope::Public,
                origin: DeclarationModuleOrigin::Generated {
                    producer: first.identity,
                    generation: Digest([239; 32]),
                    context: crate::syntax::token_wire::Key(first.identity.0),
                },
                pending: None,
            })
            .unwrap();
        let index = DeclarationImportIndex::new(&scan).unwrap();
        let path = [
            ImportSegment::Name("local".into()),
            ImportSegment::Name("m".into()),
        ];
        assert_eq!(
            index.lookup(first.identity, &path, false, &names).unwrap(),
            PathLookup::NeedInput(first.identity)
        );
        assert_eq!(
            index.lookup(second.identity, &path, false, &names).unwrap(),
            PathLookup::Absent
        );
        assert_eq!(
            index
                .lookup(
                    first.identity,
                    &[
                        ImportSegment::Crate,
                        ImportSegment::Name("local".into()),
                        ImportSegment::Name("m".into())
                    ],
                    false,
                    &names
                )
                .unwrap(),
            PathLookup::Absent
        );
    }
    #[test]
    fn nested_block_module_member_cannot_use_an_unrelated_flattened_registry_route() {
        let unit = Digest([231; 32]);
        let source = scan(
            "fn cold(){mod local{mod nested{} fn inner(){nested::m!{}}}}",
            unit,
        );
        let producer = &source.producers()[0];
        let module = source
            .modules()
            .iter()
            .position(|row| row.name == "nested")
            .unwrap();
        let (mut names, definition, _) = names(unit);
        // Deliberately disclosed unrelated global route: immediate Module kind
        // and owner=None do not prove a block ancestor is attached to this route.
        for parts in [vec!["local"], vec!["local", "nested"]] {
            names
                .module_paths
                .declare(
                    ModulePath {
                        unit,
                        names: parts
                            .into_iter()
                            .map(|name| MacroName::parse(name, 2024).unwrap())
                            .collect(),
                    },
                    VisibilityScope::Public,
                )
                .unwrap();
        }
        names
            .define(
                MacroPath::new(
                    unit,
                    ["local", "nested", "m"]
                        .into_iter()
                        .map(|name| MacroName::parse(name, 2024).unwrap())
                        .collect(),
                )
                .unwrap(),
                definition,
            )
            .unwrap();
        let binding =
            DeclarationModuleBinding::from_scan(&source, module, DeclarationModuleOrigin::Original)
                .unwrap();
        assert_eq!(binding.scope_kind, DeclarationScopeKind::Module);
        assert_eq!(binding.scope.owner, None);
        let demand = binding.identity;
        names.declare_scoped_module_binding(binding).unwrap();
        assert_eq!(
            DeclarationImportIndex::new(&source)
                .unwrap()
                .lookup(
                    producer.identity,
                    &[
                        ImportSegment::Name("nested".into()),
                        ImportSegment::Name("m".into())
                    ],
                    false,
                    &names
                )
                .unwrap(),
            PathLookup::NeedInput(demand)
        );
    }
    #[test]
    fn upstream_use_macro_self_keeps_exact_auxiliary_export_identity() {
        use crate::frontend::source_macro_routes::SourceMacroRoutes;
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/use-macro-self.rs"
        );
        const AUX: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/auxiliary/use-macro-self.rs"
        );
        let consumer = Digest([214; 32]);
        let provider = Digest([215; 32]);
        let case = upstream_parsed(CASE);
        let auxiliary = upstream_parsed(AUX);
        let provider_outline = auxiliary
            .outline(provider, vec![], &CfgSet::default())
            .unwrap();
        let exported = provider_outline
            .items
            .iter()
            .find(|item| {
                item.def_key.declared_name == "foobarius"
                    && item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
            })
            .unwrap()
            .def_key
            .fingerprint();
        let provider_catalog = upstream_catalog(&auxiliary, provider, &BTreeMap::new());
        let consumer_catalog = upstream_catalog(
            &case,
            consumer,
            &BTreeMap::from([(MacroName::parse("use_macro_self", 2024).unwrap(), provider)]),
        );
        let names =
            SourceMacroRoutes::candidate_graph(&[provider_catalog, consumer_catalog]).unwrap();
        let scan = case
            .scan_declarations(
                SourcePath::new("upstream/use-macro-self.rs").unwrap(),
                ScopeKey {
                    crate_key: consumer,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        assert_eq!(scan.imports().len(), 1);
        assert_eq!(
            scan.imports()[0].source.path.last(),
            Some(&ImportSegment::SelfModule)
        );
        let producer = scan
            .producers()
            .iter()
            .find(|row| row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
            .unwrap();
        let found = DeclarationImportIndex::new(&scan)
            .unwrap()
            .lookup(
                producer.identity,
                &[ImportSegment::Name("foobarius".into())],
                false,
                &names,
            )
            .unwrap();
        let PathLookup::Defined(definition) = found else {
            panic!("unchanged source macro invocation lost its auxiliary export: {found:?}")
        };
        assert_eq!(definition.identity, exported);
        assert_eq!(definition.defining_unit, provider);
        assert_eq!(definition.definition_edition, 2024);
    }
    #[test]
    fn upstream_self_shadowing_import_routes_original_module_target() {
        use crate::frontend::source_macro_routes::SourceMacroRoutes;
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/self/self-shadowing-import.rs"
        );
        let unit = Digest([216; 32]);
        let parsed = upstream_parsed(CASE);
        let names = SourceMacroRoutes::candidate_graph(&[upstream_catalog(
            &parsed,
            unit,
            &BTreeMap::new(),
        )])
        .unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("upstream/self-shadowing-import.rs").unwrap(),
                ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        assert_eq!(scan.imports().len(), 1);
        let import = &scan.imports()[0];
        assert_eq!(import.source.scope.module_path, ["c"]);
        assert_eq!(
            import.source.path,
            [
                ImportSegment::Crate,
                ImportSegment::Name("a".into()),
                ImportSegment::Name("b".into()),
                ImportSegment::Name("a".into())
            ]
        );
        let mut bytes = 0;
        let target = DeclarationImportIndex::new(&scan)
            .unwrap()
            .glob_module(import, &names, &|_| Ok(true), &mut bytes)
            .unwrap();
        let DeclarationModuleLookup::Module(ModuleLookup::Defined {
            unit: selected,
            components,
            ..
        }) = target
        else {
            panic!("unchanged upstream module alias target was not retained")
        };
        assert_eq!(selected, unit);
        assert_eq!(
            components.iter().map(MacroName::as_str).collect::<Vec<_>>(),
            ["a", "b", "a"]
        );
    }
    #[test]
    fn upstream_shadowed_glob_module_cycle_stays_pending_on_exact_import() {
        use crate::frontend::source_macro_routes::SourceMacroRoutes;
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/shadow-glob-module-resolution-1.rs"
        );
        let unit = Digest([229; 32]);
        let parsed = upstream_parsed(CASE);
        let graph_error = SourceMacroRoutes::candidate_graph(&[upstream_catalog(
            &parsed,
            unit,
            &BTreeMap::new(),
        )])
        .err()
        .expect("unchanged cyclic fixture must not claim a ready namespace");
        assert_eq!(graph_error.code, "LRR-E-MACRO-SCOPE");
        assert_eq!(
            graph_error.message,
            "cyclic strong/glob module selection has no stable result"
        );
        let source = scan(CASE, unit);
        assert_eq!(source.imports().len(), 3);
        assert!(source.producers().is_empty());
        // Disclosed ready original module headers only. The borrowed cyclic
        // import facts are resolved below; no complete ready graph is claimed.
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        for (index, _) in source.modules().iter().enumerate() {
            let row = DeclarationModuleBinding::from_scan(
                &source,
                index,
                DeclarationModuleOrigin::Original,
            )
            .unwrap();
            names
                .module_paths
                .declare(
                    ModulePath {
                        unit: row.route_unit,
                        names: row.route_components.clone(),
                    },
                    row.visibility.clone(),
                )
                .unwrap();
            names.declare_scoped_module_binding(row).unwrap();
        }
        let index = DeclarationImportIndex::new(&source).unwrap();
        let glob = &source.imports()[0];
        assert_eq!(glob.source.kind, ImportKind::Glob);
        let mut bytes = 0;
        assert!(
            matches!(index.glob_module(glob, &names, &|_| Ok(true), &mut bytes).unwrap(),
            DeclarationModuleLookup::Module(ModuleLookup::Defined { unit: found, components, .. })
                if found == unit && components.iter().map(MacroName::as_str).collect::<Vec<_>>() == ["a"])
        );
        for import in &source.imports()[1..] {
            let mut bytes = 0;
            let found = index
                .glob_module(import, &names, &|_| Ok(true), &mut bytes)
                .unwrap();
            assert!(
                matches!(found, DeclarationModuleLookup::NeedInput(input) if input == import.source.identity)
            );
        }
    }
    #[test]
    fn upstream_original_export_shadows_same_scope_glob_at_the_original_invocation() {
        // Header-only namespace adaptation: `pub macro` execution is not accepted.
        // The original macro_export lives in an opaque function/array body; the scan
        // supplies its exact declaration identity without entering that body.
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/local-modularized-tricky-pass-2.rs"
        );
        let unit = Digest([217; 32]);
        let parsed = upstream_parsed(CASE);
        let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("upstream/local-modularized-tricky-pass-2.rs").unwrap(),
                ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let original = scan
            .macro_definitions()
            .iter()
            .find(|row| row.name == "exported")
            .unwrap();
        let weak_header = outline
            .items
            .iter()
            .find(|item| {
                item.def_key.declared_name == "exported" && item.def_key.module_path == ["inner1"]
            })
            .unwrap();
        let module = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "inner1")
            .unwrap();
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![MacroName::parse("inner1", 2024).unwrap()],
                },
                VisibilityScope::for_declaration(&module.header.visibility, &module.scope, 2024)
                    .unwrap(),
            )
            .unwrap();
        let strong = MacroDefinition {
            identity: original.identity,
            defining_unit: unit,
            definition_edition: 2024,
        };
        names
            .define(
                MacroPath::new(unit, vec![MacroName::parse("exported", 2024).unwrap()]).unwrap(),
                strong.clone(),
            )
            .unwrap();
        names
            .define_visible(
                MacroPath::new(
                    unit,
                    vec![
                        MacroName::parse("inner1", 2024).unwrap(),
                        MacroName::parse("exported", 2024).unwrap(),
                    ],
                )
                .unwrap(),
                MacroDefinition {
                    identity: weak_header.def_key.fingerprint(),
                    defining_unit: unit,
                    definition_edition: 2024,
                },
                VisibilityScope::for_declaration(
                    &weak_header.header.visibility,
                    &weak_header.scope,
                    2024,
                )
                .unwrap(),
            )
            .unwrap();
        let invocation = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && CASE
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("exported!"))
            })
            .unwrap();
        let result = DeclarationImportIndex::new(&scan)
            .unwrap()
            .lookup(
                invocation.identity,
                &[ImportSegment::Name("exported".into())],
                false,
                &names,
            )
            .unwrap();
        assert_eq!(result, PathLookup::Defined(strong));
    }
    #[test]
    fn upstream_inner_glob_remains_ambiguous_with_original_outer_macro_header() {
        // Unchanged source and stderr are retained; only the fenv namespace edge is exercised.
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/glob-shadowing.rs"
        );
        let unit = Digest([218; 32]);
        let parsed = upstream_parsed(CASE);
        let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("upstream/glob-shadowing.rs").unwrap(),
                ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        for module in outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == crate::source::manifest::ItemKind::Module)
        {
            names
                .module_paths
                .declare(
                    ModulePath {
                        unit,
                        names: vec![MacroName::parse(&module.def_key.declared_name, 2024).unwrap()],
                    },
                    VisibilityScope::for_declaration(
                        &module.header.visibility,
                        &module.scope,
                        2024,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let mut expected = BTreeSet::new();
        for header in outline
            .items
            .iter()
            .filter(|item| item.def_key.declared_name == "fenv")
        {
            let definition = MacroDefinition {
                identity: header.def_key.fingerprint(),
                defining_unit: unit,
                definition_edition: 2024,
            };
            expected.insert(definition.clone());
            let path = MacroPath::new(
                unit,
                header
                    .def_key
                    .module_path
                    .iter()
                    .map(|name| MacroName::parse(name, 2024).unwrap())
                    .chain([MacroName::parse("fenv", 2024).unwrap()])
                    .collect(),
            )
            .unwrap();
            names
                .define_visible(
                    path,
                    definition,
                    VisibilityScope::for_declaration(
                        &header.header.visibility,
                        &header.scope,
                        2024,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        assert_eq!(expected.len(), 2);
        let invocation = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && CASE
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("fenv!"))
            })
            .unwrap();
        assert_eq!(invocation.scope.module_path, ["glob_shadows_item"]);
        assert!(invocation.scope.owner.is_some());
        let found = DeclarationImportIndex::new(&scan)
            .unwrap()
            .lookup(
                invocation.identity,
                &[ImportSegment::Name("fenv".into())],
                false,
                &names,
            )
            .unwrap();
        assert_eq!(found, PathLookup::Ambiguous(expected.into_iter().collect()));
    }
    #[test]
    fn root_glob_does_not_shadow_compiler_prelude_and_pending_glob_is_not_discarded() {
        let unit = Digest([200; 32]);
        let (mut names, dep, other) = names(unit);
        names
            .compiler_global(unit, MacroName::parse("m", 2024).unwrap(), other.clone())
            .unwrap();
        let source = scan("use dep::*; #[m] fn selected(){}", unit);
        assert_eq!(
            lookup(&source, &names, &["m"]),
            PathLookup::Ambiguous(vec![dep, other])
        );
        let strong = MacroDefinition {
            identity: Digest([219; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        names
            .define(
                MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap()]).unwrap(),
                strong,
            )
            .unwrap();
        let pending = scan("#[gate] use dep::*; #[m] fn selected(){}", unit);
        assert_eq!(
            lookup(&pending, &names, &["m"]),
            PathLookup::NeedInput(pending.imports()[0].attribute_inputs[0])
        );
    }
    #[test]
    fn upstream_generated_export_does_not_shadow_original_module_glob() {
        // Source-backed namespace/provenance adaptation only; macro 2.0 and
        // the upstream macro invocations are not prepared or executed.
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/local-modularized-tricky-fail-1.rs"
        );
        let unit = Digest([220; 32]);
        let parsed = upstream_parsed(CASE);
        let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("upstream/local-modularized-tricky-fail-1.rs").unwrap(),
                ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let generated_at = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && row.scope.module_path == ["inner2"]
                    && CASE
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("define_exported!"))
            })
            .unwrap();
        let invocation = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && CASE
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("exported!"))
            })
            .unwrap();
        let weak_header = outline
            .items
            .iter()
            .find(|item| {
                item.def_key.declared_name == "exported" && item.def_key.module_path == ["inner1"]
            })
            .unwrap();
        let module = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "inner1")
            .unwrap();
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![MacroName::parse("inner1", 2024).unwrap()],
                },
                VisibilityScope::for_declaration(&module.header.visibility, &module.scope, 2024)
                    .unwrap(),
            )
            .unwrap();
        let generated = MacroDefinition {
            identity: generated_at.identity,
            defining_unit: unit,
            definition_edition: 2024,
        };
        names
            .define_visible_origin(
                MacroPath::new(unit, vec![MacroName::parse("exported", 2024).unwrap()]).unwrap(),
                generated.clone(),
                VisibilityScope::Public,
                Some(crate::syntax::token_wire::Key(generated_at.identity.0)),
            )
            .unwrap();
        let weak = MacroDefinition {
            identity: weak_header.def_key.fingerprint(),
            defining_unit: unit,
            definition_edition: 2024,
        };
        names
            .define_visible(
                MacroPath::new(
                    unit,
                    vec![
                        MacroName::parse("inner1", 2024).unwrap(),
                        MacroName::parse("exported", 2024).unwrap(),
                    ],
                )
                .unwrap(),
                weak.clone(),
                VisibilityScope::for_declaration(
                    &weak_header.header.visibility,
                    &weak_header.scope,
                    2024,
                )
                .unwrap(),
            )
            .unwrap();
        let mut expected = vec![generated, weak];
        expected.sort();
        let found = DeclarationImportIndex::new(&scan)
            .unwrap()
            .lookup(
                invocation.identity,
                &[ImportSegment::Name("exported".into())],
                false,
                &names,
            )
            .unwrap();
        assert_eq!(found, PathLookup::Ambiguous(expected));
    }
    #[test]
    fn upstream_inline_module_import_attachment_shadows_glob_without_block_priority() {
        // Exact import-placement adaptation outside the unchanged licensed fixture.
        const CASE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/glob-shadowing.rs"
        );
        let block_import = "    fn block() {\n        use crate::m::*;";
        assert_eq!(CASE.matches(block_import).count(), 2);
        let adapted = CASE.replace(block_import, "    use crate::m::*;\n    fn block() {");
        let unit = Digest([225; 32]);
        let parsed = upstream_parsed(&adapted);
        let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
        let scan = parsed
            .scan_declarations(
                SourcePath::new("lrr-adapted/glob-shadowing-module.rs").unwrap(),
                ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        for module in outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == crate::source::manifest::ItemKind::Module)
        {
            names
                .module_paths
                .declare(
                    ModulePath {
                        unit,
                        names: vec![MacroName::parse(&module.def_key.declared_name, 2024).unwrap()],
                    },
                    VisibilityScope::for_declaration(
                        &module.header.visibility,
                        &module.scope,
                        2024,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let mut strong = None;
        for header in outline
            .items
            .iter()
            .filter(|item| item.def_key.declared_name == "fenv")
        {
            let definition = MacroDefinition {
                identity: header.def_key.fingerprint(),
                defining_unit: unit,
                definition_edition: 2024,
            };
            if header.def_key.module_path == ["glob_shadows_item"] {
                strong = Some(definition.clone());
            }
            let path = MacroPath::new(
                unit,
                header
                    .def_key
                    .module_path
                    .iter()
                    .map(|name| MacroName::parse(name, 2024).unwrap())
                    .chain([MacroName::parse("fenv", 2024).unwrap()])
                    .collect(),
            )
            .unwrap();
            names
                .define_visible(
                    path,
                    definition,
                    VisibilityScope::for_declaration(
                        &header.header.visibility,
                        &header.scope,
                        2024,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let invocation = scan
            .producers()
            .iter()
            .find(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && adapted
                        .get(row.range.start as usize..row.range.end as usize)
                        .is_some_and(|text| text.starts_with("fenv!"))
            })
            .unwrap();
        let found = DeclarationImportIndex::new(&scan)
            .unwrap()
            .lookup(
                invocation.identity,
                &[ImportSegment::Name("fenv".into())],
                false,
                &names,
            )
            .unwrap();
        assert_eq!(found, PathLookup::Defined(strong.unwrap()));
    }
    #[test]
    fn generated_module_priority_requires_matched_attachment_and_original_binding() {
        use crate::syntax::declaration_scan::generated_declaration_fixture as generated;
        use crate::{
            expand::scopes::{MacroDefinition, MacroName},
            frontend::{
                declaration_imports::DeclarationImportIndex,
                macro_names::{MacroNamespaces, MacroPath, PathLookup},
            },
            syntax::{imports::ImportSegment, visibility::VisibilityScope},
        };
        let unit = Digest([226; 32]);
        let capture = |parsed: &ParsedFile, kind: Option<DeclarationScopeKind>| {
            let scope = ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            };
            let path = SourcePath::new("generated-attachment.rs").unwrap();
            match kind {
                None => parsed
                    .scan_declarations(
                        path,
                        scope,
                        &CfgSet::default(),
                        DeclarationScanLimits::default(),
                    )
                    .unwrap(),
                Some(kind) => parsed
                    .scan_declarations_at_root(
                        path,
                        scope,
                        &CfgSet::default(),
                        DeclarationScanLimits::default(),
                        kind,
                    )
                    .unwrap(),
            }
        };
        let mut names = MacroNamespaces::default();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![MacroName::parse("dep", 2024).unwrap()],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        let strong = MacroDefinition {
            identity: Digest([227; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let weak = MacroDefinition {
            identity: Digest([228; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let path = MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap()]).unwrap();
        names.define(path.clone(), strong.clone()).unwrap();
        names
            .define(
                MacroPath::new(
                    unit,
                    vec![
                        MacroName::parse("dep", 2024).unwrap(),
                        MacroName::parse("m", 2024).unwrap(),
                    ],
                )
                .unwrap(),
                weak.clone(),
            )
            .unwrap();
        let parsed = generated(
            "use dep::*; #[m] fn cold(){}",
            crate::expand::identity::GrammarSlot::Items,
        );
        let lookup = |scan: &DeclarationScan, names: &MacroNamespaces| {
            DeclarationImportIndex::new(scan)
                .unwrap()
                .lookup(
                    scan.producers().last().unwrap().identity,
                    &[ImportSegment::Name("m".into())],
                    false,
                    names,
                )
                .unwrap()
        };
        let mut ambiguity = vec![strong.clone(), weak.clone()];
        ambiguity.sort();
        assert_eq!(
            lookup(&capture(&parsed, None), &names),
            PathLookup::Ambiguous(ambiguity.clone())
        );
        assert_eq!(
            lookup(&capture(&parsed, Some(DeclarationScopeKind::Block)), &names),
            PathLookup::Ambiguous(ambiguity)
        );
        let module = capture(&parsed, Some(DeclarationScopeKind::Module));
        assert_eq!(lookup(&module, &names), PathLookup::Defined(strong));
        let mut expanded = MacroNamespaces::default();
        expanded.module_paths = names.module_paths.clone();
        let generated_definition = MacroDefinition {
            identity: Digest([229; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        expanded
            .define_visible_origin(
                path,
                generated_definition.clone(),
                VisibilityScope::Public,
                Some(crate::syntax::token_wire::Key([230; 32])),
            )
            .unwrap();
        expanded
            .define(
                MacroPath::new(
                    unit,
                    vec![
                        MacroName::parse("dep", 2024).unwrap(),
                        MacroName::parse("m", 2024).unwrap(),
                    ],
                )
                .unwrap(),
                weak.clone(),
            )
            .unwrap();
        let mut ambiguity = vec![generated_definition, weak];
        ambiguity.sort();
        assert_eq!(lookup(&module, &expanded), PathLookup::Ambiguous(ambiguity));
        let pending = capture(
            &generated(
                "#[gate] use dep::*; #[m] fn cold(){}",
                crate::expand::identity::GrammarSlot::Items,
            ),
            Some(DeclarationScopeKind::Module),
        );
        assert_eq!(
            lookup(&pending, &names),
            PathLookup::NeedInput(pending.imports()[0].attribute_inputs[0])
        );
    }
    #[test]
    fn self_leaf_is_only_a_module_binding_and_does_not_import_a_same_named_macro() {
        let unit = Digest([200; 32]);
        let (mut names, dep, other) = names(unit);
        names
            .define(
                MacroPath::new(unit, vec![MacroName::parse("dep", 2024).unwrap()]).unwrap(),
                dep.clone(),
            )
            .unwrap();
        names
            .define(
                MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap()]).unwrap(),
                other.clone(),
            )
            .unwrap();
        let scan = scan(
            "fn cold(){ use dep::{self as m}; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(lookup(&scan, &names, &["m"]), PathLookup::Defined(other));
    }
    #[test]
    fn local_module_alias_glob_uses_lexical_alias_chain_and_keeps_cycle_pending() {
        let unit = Digest([200; 32]);
        let (names, dep, _) = names(unit);
        let direct = scan(
            "fn cold(){ use dep as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(
            lookup(&direct, &names, &["m"]),
            PathLookup::Defined(dep.clone())
        );
        let chain = scan(
            "fn cold(){ use dep::{self as p}; use p as q; use q::*; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(lookup(&chain, &names, &["m"]), PathLookup::Defined(dep));
        let cycle = scan(
            "fn cold(){ use p as q; use q as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        assert!(
            matches!(lookup(&cycle, &names, &["m"]), PathLookup::NeedInput(input)
            if cycle.imports().iter().any(|row| row.source.identity == input))
        );
    }
    #[test]
    fn local_alias_glob_preserves_pending_attribute_and_generated_context_identity() {
        let unit = Digest([200; 32]);
        let (names, _, _) = names(unit);
        let annotated = scan(
            "fn cold(){ #[gate] use dep as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        let annotation = annotated.imports()[0].attribute_inputs[0];
        assert_eq!(
            lookup(&annotated, &names, &["m"]),
            PathLookup::NeedInput(annotation)
        );
        let generated_context = scan(
            "fn cold(){ use dep as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        let alias = generated_context.imports()[0].source.identity;
        let found = DeclarationImportIndex::new(&generated_context)
            .unwrap()
            .lookup_validated(
                generated_context.producers().last().unwrap().identity,
                &[ImportSegment::Name("m".into())],
                false,
                &names,
                |row| Ok(row.source.identity != alias),
            )
            .unwrap();
        assert_eq!(found, PathLookup::NeedInput(alias));
    }
    #[test]
    fn globs_preserve_ambiguity_and_exclude_private_members() {
        let unit = Digest([200; 32]);
        let (names, dep, other) = names(unit);
        let ambiguous = scan(
            "fn cold(){ use dep::*; use other::*; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(
            lookup(&ambiguous, &names, &["m"]),
            PathLookup::Ambiguous(vec![dep.clone(), other])
        );
        let mut private = MacroNamespaces::default();
        private
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        private
            .module_paths
            .declare(
                ModulePath {
                    unit,
                    names: vec![MacroName::parse("dep", 2024).unwrap()],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        private
            .define_visible(
                MacroPath::new(
                    unit,
                    vec![
                        MacroName::parse("dep", 2024).unwrap(),
                        MacroName::parse("m", 2024).unwrap(),
                    ],
                )
                .unwrap(),
                dep,
                VisibilityScope::Within {
                    unit,
                    module_path: vec!["dep".into()],
                },
            )
            .unwrap();
        let hidden = scan(
            "fn cold(){ use dep as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(lookup(&hidden, &private, &["m"]), PathLookup::Absent);
    }
    #[test]
    fn local_alias_glob_does_not_bypass_external_module_privacy() {
        let unit = Digest([200; 32]);
        let provider = Digest([203; 32]);
        let mut names = MacroNamespaces::default();
        names
            .dependency(unit, MacroName::parse("dep", 2024).unwrap(), provider)
            .unwrap();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit: provider,
                    names: vec![],
                },
                VisibilityScope::Public,
            )
            .unwrap();
        names
            .module_paths
            .declare(
                ModulePath {
                    unit: provider,
                    names: vec![MacroName::parse("secret", 2024).unwrap()],
                },
                VisibilityScope::Within {
                    unit: provider,
                    module_path: vec![],
                },
            )
            .unwrap();
        let scan = scan(
            "fn cold(){ use ::dep::secret as p; use p::*; #[m] fn selected(){} }",
            unit,
        );
        let witness =
            MacroPath::new(provider, vec![MacroName::parse("secret", 2024).unwrap()]).unwrap();
        assert_eq!(
            lookup(&scan, &names, &["m"]),
            PathLookup::Inaccessible(vec![witness])
        );
    }
    #[test]
    fn self_module_import_resolves_its_target_without_reentering_itself() {
        let unit = Digest([200; 32]);
        let (names, dep, _) = names(unit);
        let scan = scan(
            "fn cold(){ use dep::{self}; #[dep::m] fn selected(){} }",
            unit,
        );
        assert_eq!(
            lookup(&scan, &names, &["dep", "m"]),
            PathLookup::Defined(dep)
        );
    }
    #[test]
    fn alias_chain_preserves_identity_and_real_cycle_remains_pending() {
        let unit = Digest([200; 32]);
        let (names, dep, _) = names(unit);
        let chain = scan(
            "fn cold(){ use dep as first; use first as second; #[second::m] fn selected(){} }",
            unit,
        );
        assert_eq!(
            lookup(&chain, &names, &["second", "m"]),
            PathLookup::Defined(dep)
        );
        let cycle = scan(
            "fn cold(){ use second as first; use first as second; #[second::m] fn selected(){} }",
            unit,
        );
        let found = lookup(&cycle, &names, &["second", "m"]);
        assert!(
            matches!(found, PathLookup::NeedInput(input) if cycle.imports().iter().any(|row| row.source.identity == input))
        );
    }
    #[test]
    fn nested_block_named_import_shadows_outer_and_sibling_does_not_leak() {
        let unit = Digest([200; 32]);
        let (names, dep, other) = names(unit);
        let shadow = scan(
            "fn cold(){ use dep::m as chosen; {use other::m as chosen; #[chosen] fn selected(){}} }",
            unit,
        );
        assert_eq!(
            lookup(&shadow, &names, &["chosen"]),
            PathLookup::Defined(other)
        );
        let sibling = scan(
            "fn cold(){ use dep::m as chosen; {use other::m as chosen;} {#[chosen] fn selected(){}} }",
            unit,
        );
        assert_eq!(
            lookup(&sibling, &names, &["chosen"]),
            PathLookup::Defined(dep)
        );
    }
    #[test]
    fn named_import_precedes_same_block_glob_and_child_module_is_a_boundary() {
        let unit = Digest([200; 32]);
        let (names, _, other) = names(unit);
        let explicit = scan(
            "fn cold(){ use dep::*; use other::m as m; #[m] fn selected(){} }",
            unit,
        );
        assert_eq!(
            lookup(&explicit, &names, &["m"]),
            PathLookup::Defined(other)
        );
        let child = scan(
            "use dep::m as chosen; mod child { #[chosen] fn selected(){} }",
            unit,
        );
        assert_eq!(lookup(&child, &names, &["chosen"]), PathLookup::Absent);
    }
}
