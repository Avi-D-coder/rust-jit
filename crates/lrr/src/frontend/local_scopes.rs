//! Source-ordered local lookup over owned pinned hygiene facts. As in the
//! pinned resolver's MacroDefScope handling, only crossing the exact macro
//! declaration rib removes its mark. These facts are supplied by the source
//! index; this component does not discover declarations or execute expansion.
use super::hygiene::HygieneFacts;
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::{MacroDefinition, MacroName, ScopeKey},
    syntax::token_wire::{Edition, Key, NameContextKey},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LocalLookup {
    Binding(Digest),
    Absent,
    NeedInput(Digest),
}
#[derive(Debug, Clone)]
pub enum LocalDeclaration {
    Binding {
        at: u64,
        name: MacroName,
        context: Key,
        identity: Digest,
    },
    Macro {
        at: u64,
        definition: MacroDefinition,
    },
    Pending {
        at: u64,
        invocation: Digest,
    },
}
impl LocalDeclaration {
    fn position(&self) -> u64 {
        match self {
            Self::Binding { at, .. } | Self::Macro { at, .. } | Self::Pending { at, .. } => *at,
        }
    }
}
#[derive(Default)]
struct Scope {
    parent: Option<(ScopeKey, u64)>,
    function_boundary: bool,
    occupied: BTreeSet<u64>,
    frontiers: BTreeSet<u64>,
    bindings: BTreeMap<MacroName, BTreeMap<NameContextKey, BTreeMap<u64, Digest>>>,
    unresolved: BTreeMap<MacroName, BTreeMap<NameContextKey, BTreeMap<u64, Digest>>>,
    definitions: BTreeMap<Digest, BTreeMap<u64, MacroDefinition>>,
    pending: BTreeMap<u64, Digest>,
}
pub struct LocalScopes {
    facts: Arc<HygieneFacts>,
    scopes: BTreeMap<ScopeKey, Scope>,
    definitions: BTreeMap<Digest, MacroDefinition>,
    expansions: BTreeMap<Key, MacroDefinition>,
    records: usize,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn edition(value: Edition) -> u16 {
    match value {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    }
}
impl LocalScopes {
    pub(crate) fn facts_root(&self) -> &Arc<HygieneFacts> {
        &self.facts
    }
    /// Conservative owned BTree node/string storage; shared hygiene is separate.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        fn map<K, V>(rows: &BTreeMap<K, V>) -> Result<usize, Diagnostic> {
            rows.len()
                .checked_mul(
                    std::mem::size_of::<(K, V)>()
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)
        }
        fn set<T>(rows: &BTreeSet<T>) -> Result<usize, Diagnostic> {
            rows.len()
                .checked_mul(
                    std::mem::size_of::<T>()
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)
        }
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(crate::runtime::arena::resource)?;
            Ok(())
        };
        add(map(&self.scopes)?)?;
        add(map(&self.definitions)?)?;
        add(map(&self.expansions)?)?;
        for scope in self.scopes.values() {
            add(set(&scope.occupied)?)?;
            add(set(&scope.frontiers)?)?;
            add(map(&scope.pending)?)?;
            add(map(&scope.definitions)?)?;
            for rows in scope.definitions.values() {
                add(map(rows)?)?;
            }
            for names in [&scope.bindings, &scope.unresolved] {
                add(map(names)?)?;
                for (name, contexts) in names {
                    add(name
                        .as_str()
                        .len()
                        .checked_mul(2)
                        .ok_or_else(crate::runtime::arena::resource)?)?;
                    add(map(contexts)?)?;
                    for rows in contexts.values() {
                        add(map(rows)?)?;
                    }
                }
            }
        }
        Ok(bytes)
    }
    pub fn new(facts: Arc<HygieneFacts>) -> Self {
        Self {
            facts,
            scopes: BTreeMap::new(),
            definitions: BTreeMap::new(),
            expansions: BTreeMap::new(),
            records: 0,
        }
    }
    fn admit(&self, count: usize) -> Result<usize, Diagnostic> {
        self.records
            .checked_add(count)
            .ok_or_else(|| error("local scope record limit exceeded"))
    }
    fn get(&self, key: ScopeKey) -> Result<&Scope, Diagnostic> {
        self.scopes
            .get(&key)
            .ok_or_else(|| error("unknown local scope"))
    }
    fn vacant(&self, key: ScopeKey, at: u64) -> Result<(), Diagnostic> {
        if self.get(key)?.occupied.contains(&at) {
            return Err(error("duplicate local source-order position"));
        }
        Ok(())
    }
    fn validate_definition(&self, definition: &MacroDefinition) -> Result<bool, Diagnostic> {
        definition.validate()?;
        match self.definitions.get(&definition.identity) {
            Some(old) if old != definition => {
                Err(error("conflicting local macro definition identity"))
            }
            Some(_) => Ok(false),
            None => Ok(true),
        }
    }
    pub fn add_root(&mut self, key: ScopeKey) -> Result<(), Diagnostic> {
        self.insert_scope(key, Scope::default())
    }
    pub fn add_child(
        &mut self,
        parent: ScopeKey,
        at: u64,
        ordinal: u32,
        function_boundary: bool,
    ) -> Result<ScopeKey, Diagnostic> {
        let new_frontier = !self.get(parent)?.frontiers.contains(&at);
        let key = parent.child(at, ordinal);
        if self.scopes.contains_key(&key) {
            return Err(error("duplicate local scope"));
        }
        let count = self.admit(1 + usize::from(new_frontier))?;
        self.scopes.insert(
            key,
            Scope {
                parent: Some((parent, at)),
                function_boundary,
                ..Scope::default()
            },
        );
        self.scopes
            .get_mut(&parent)
            .expect("validated parent")
            .frontiers
            .insert(at);
        self.records = count;
        Ok(key)
    }
    fn insert_scope(&mut self, key: ScopeKey, scope: Scope) -> Result<(), Diagnostic> {
        if self.scopes.contains_key(&key) {
            return Err(error("duplicate local scope"));
        }
        let count = self.admit(1)?;
        self.scopes.insert(key, scope);
        self.records = count;
        Ok(())
    }
    pub fn bind(
        &mut self,
        scope: ScopeKey,
        at: u64,
        name: MacroName,
        context: Key,
        binding: Digest,
    ) -> Result<(), Diagnostic> {
        let context = self.facts.name_local(context)?;
        self.vacant(scope, at)?;
        let count = self.admit(1)?;
        let scope = self.scopes.get_mut(&scope).expect("validated scope");
        scope.occupied.insert(at);
        scope
            .bindings
            .entry(name)
            .or_default()
            .entry(context)
            .or_default()
            .insert(at, binding);
        self.records = count;
        Ok(())
    }

    /// A known pattern name awaiting resolution blocks only that name/context.
    pub fn defer_binding(
        &mut self,
        scope: ScopeKey,
        at: u64,
        name: MacroName,
        context: Key,
        input: Digest,
    ) -> Result<(), Diagnostic> {
        let context = self.facts.name_local(context)?;
        self.vacant(scope, at)?;
        let count = self.admit(1)?;
        let record = self.scopes.get_mut(&scope).expect("validated scope");
        record.occupied.insert(at);
        record
            .unresolved
            .entry(name)
            .or_default()
            .entry(context)
            .or_default()
            .insert(at, input);
        self.records = count;
        Ok(())
    }
    pub fn define_macro(
        &mut self,
        scope: ScopeKey,
        at: u64,
        definition: MacroDefinition,
    ) -> Result<(), Diagnostic> {
        self.vacant(scope, at)?;
        let new = self.validate_definition(&definition)?;
        let count = self.admit(1 + usize::from(new))?;
        if new {
            self.definitions
                .insert(definition.identity, definition.clone());
        }
        let scope = self.scopes.get_mut(&scope).expect("validated scope");
        scope.occupied.insert(at);
        scope
            .definitions
            .entry(definition.identity)
            .or_default()
            .insert(at, definition);
        self.records = count;
        Ok(())
    }
    pub fn register_expansion(
        &mut self,
        expansion: Key,
        definition: MacroDefinition,
    ) -> Result<(), Diagnostic> {
        let new = self.validate_definition(&definition)?;
        if let Some(old) = self.expansions.get(&expansion) {
            return if old == &definition {
                Ok(())
            } else {
                Err(error("conflicting local expansion definition"))
            };
        }
        let count = self.admit(1 + usize::from(new))?;
        if new {
            self.definitions
                .insert(definition.identity, definition.clone());
        }
        self.expansions.insert(expansion, definition);
        self.records = count;
        Ok(())
    }
    pub fn block(
        &mut self,
        scope: ScopeKey,
        at: u64,
        invocation: Digest,
    ) -> Result<(), Diagnostic> {
        self.vacant(scope, at)?;
        let count = self.admit(1)?;
        let scope = self.scopes.get_mut(&scope).expect("validated scope");
        scope.occupied.insert(at);
        scope.pending.insert(at, invocation);
        self.records = count;
        Ok(())
    }
    /// Validate a complete producer batch before publishing any name or
    /// clearing demand. Newly discovered producers remain explicit inputs.
    pub fn publish(
        &mut self,
        scope: ScopeKey,
        at: u64,
        invocation: Digest,
        declarations: Vec<LocalDeclaration>,
    ) -> Result<(), Diagnostic> {
        let current = self.get(scope)?;
        if current.pending.get(&at) != Some(&invocation) {
            return Err(error("stale local producer publication"));
        }
        let upper = current
            .occupied
            .range((std::ops::Bound::Excluded(at), std::ops::Bound::Unbounded))
            .next()
            .into_iter()
            .chain(
                current
                    .frontiers
                    .range((std::ops::Bound::Excluded(at), std::ops::Bound::Unbounded))
                    .next(),
            )
            .copied()
            .min();
        let mut staged = Vec::new();
        staged
            .try_reserve(declarations.len())
            .map_err(|_| error("local publication allocation failed"))?;
        let mut positions = BTreeSet::new();
        let mut definitions = BTreeMap::new();
        for mut declaration in declarations {
            let position = declaration.position();
            if position <= at
                || upper.is_some_and(|upper| position >= upper)
                || current.occupied.contains(&position)
                || !positions.insert(position)
            {
                return Err(error("invalid local publication source-order position"));
            }
            match &mut declaration {
                LocalDeclaration::Binding { context, .. } => {
                    self.facts.name_local(*context)?;
                }
                LocalDeclaration::Macro { definition, .. } => {
                    if self.validate_definition(definition)?
                        && let Some(old) =
                            definitions.insert(definition.identity, definition.clone())
                        && old != *definition
                    {
                        return Err(error("conflicting published macro definition identity"));
                    }
                }
                LocalDeclaration::Pending { .. } => {}
            }
            staged.push(declaration);
        }
        let count = self
            .records
            .checked_sub(1)
            .and_then(|n| n.checked_add(staged.len()))
            .and_then(|n| n.checked_add(definitions.len()))
            .ok_or_else(|| error("local publication record limit exceeded"))?;
        self.definitions.extend(definitions);
        let current = self.scopes.get_mut(&scope).expect("validated scope");
        current.pending.remove(&at);
        current.occupied.remove(&at);
        for declaration in staged {
            current.occupied.insert(declaration.position());
            match declaration {
                LocalDeclaration::Binding {
                    at,
                    name,
                    context,
                    identity,
                } => {
                    current
                        .bindings
                        .entry(name)
                        .or_default()
                        .entry(
                            self.facts
                                .name_local(context)
                                .expect("validated name comparison"),
                        )
                        .or_default()
                        .insert(at, identity);
                }
                LocalDeclaration::Macro { at, definition } => {
                    current
                        .definitions
                        .entry(definition.identity)
                        .or_default()
                        .insert(at, definition);
                }
                LocalDeclaration::Pending { at, invocation } => {
                    current.pending.insert(at, invocation);
                }
            }
        }
        self.records = count;
        Ok(())
    }
    pub fn lookup(
        &self,
        scope: ScopeKey,
        before: u64,
        name: &MacroName,
        context: Key,
    ) -> Result<LocalLookup, Diagnostic> {
        self.lookup_with_scope(scope, before, name, context)
            .map(|(found, _)| found)
    }
    pub fn lookup_with_scope(
        &self,
        mut scope: ScopeKey,
        mut before: u64,
        name: &MacroName,
        context: Key,
    ) -> Result<(LocalLookup, ScopeKey), Diagnostic> {
        let mut comparison = self.facts.name_local(context)?;
        let mut context = self.facts.local(context)?;
        // Each iteration moves to an older rib or an acyclic parent scope.
        // The source index cannot cause unbounded host recursion.
        for _ in 0..=self.records {
            let record = self.facts.context(context)?;
            let outer = if let Some(expansion) = record.expansion {
                let definition = self
                    .expansions
                    .get(&expansion)
                    .ok_or_else(|| error("local expansion definition input is missing"))?;
                if record.defining_unit.0 != definition.defining_unit.0
                    || edition(record.edition) != definition.definition_edition
                {
                    return Err(error("local expansion definition authority mismatch"));
                }
                Some(definition.identity)
            } else {
                None
            };
            let current = self.get(scope)?;
            let binding = current
                .bindings
                .get(name)
                .and_then(|contexts| contexts.get(&comparison))
                .and_then(|rows| rows.range(..before).next_back());
            let unresolved = current
                .unresolved
                .get(name)
                .and_then(|contexts| contexts.get(&comparison))
                .and_then(|rows| rows.range(..before).next_back());
            let rib = outer
                .and_then(|identity| current.definitions.get(&identity))
                .and_then(|rows| rows.range(..before).next_back());
            let latest = binding
                .map(|(at, _)| *at)
                .into_iter()
                .chain(rib.map(|(at, _)| *at))
                .chain(unresolved.map(|(at, _)| *at))
                .max();
            if let Some((at, invocation)) = current.pending.range(..before).next_back()
                && latest.is_none_or(|latest| *at > latest)
            {
                return Ok((LocalLookup::NeedInput(*invocation), scope));
            }
            if let Some((at, input)) = unresolved
                && binding.is_none_or(|(binding, _)| at > binding)
                && rib.is_none_or(|(rib, _)| at > rib)
            {
                return Ok((LocalLookup::NeedInput(*input), scope));
            }
            if let Some((at, binding)) = binding
                && rib.is_none_or(|(rib, _)| at > rib)
            {
                return Ok((LocalLookup::Binding(*binding), scope));
            }
            if let Some((at, _)) = rib {
                let parent = record
                    .parent
                    .ok_or_else(|| error("marked local context has no parent"))?;
                comparison = self.facts.name_local(parent)?;
                context = self.facts.local(parent)?;
                before = *at;
                continue;
            }
            if current.function_boundary {
                return Ok((LocalLookup::Absent, scope));
            }
            let Some((parent, at)) = current.parent else {
                return Ok((LocalLookup::Absent, scope));
            };
            scope = parent;
            before = at;
        }
        Err(error("local scope traversal bound exceeded"))
    }
}
