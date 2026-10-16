//! Pure owned textual-scope facts. Pending expansion is input demand, never a
//! cached negative result or permission to run an expander from a query.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::ast_adapter,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct MacroName(String);
impl MacroName {
    pub fn parse(text: &str, edition: u16) -> Result<Self, Diagnostic> {
        ast_adapter::macro_name(text, edition)
            .map(Self)
            .ok_or_else(|| error("invalid macro identifier or edition"))
    }
    // Receiver names use the local namespace's keyword identity. Macro source
    // identifiers still go through the pinned identifier validator above.
    pub(crate) fn local(text: &str, edition: u16) -> Result<Self, Diagnostic> {
        if text == "self" && matches!(edition, 2015 | 2018 | 2021 | 2024) {
            Ok(Self(text.into()))
        } else {
            Self::parse(text, edition)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct ScopeKey(Digest);
impl StableEncode for ScopeKey {
    const DOMAIN: &'static str = "lrr/v2/textual-scope-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.0);
    }
}
impl ScopeKey {
    pub fn root(unit: Digest) -> Self {
        let mut encoder = StableEncoder::new("lrr/v2/macro-root-scope");
        encoder.digest(unit);
        Self(encoder.finish())
    }
    pub(crate) fn child(self, at: u64, ordinal: u32) -> Self {
        let mut encoder = StableEncoder::new("lrr/v2/macro-child-scope");
        encoder.digest(self.0);
        encoder.u64(at);
        encoder.u32(ordinal);
        Self(encoder.finish())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct MacroDefinition {
    pub identity: Digest,
    pub defining_unit: Digest,
    pub definition_edition: u16,
}
impl MacroDefinition {
    pub(crate) fn validate(&self) -> Result<(), Diagnostic> {
        if !matches!(self.definition_edition, 2015 | 2018 | 2021 | 2024) {
            return Err(error("unsupported macro definition edition"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
/// Lexical source facts. `Defined` remains a candidate until the pinned hygiene
/// resolver validates its expansion context; it is not final Rust resolution.
pub enum ScopeLookup {
    Defined(MacroDefinition),
    Absent,
    NeedInput(Digest),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ScopeCandidates {
    Definitions(Vec<MacroDefinition>),
    NeedInput(Digest),
}

pub(crate) enum LocalBindings {
    Ready(Vec<(MacroName, MacroDefinition)>),
    NeedInput(Digest),
}
#[derive(Default, Clone)]
struct Scope {
    // Parent visibility freezes at the point at which this lexical scope opens.
    parent: Option<(ScopeKey, u64)>,
    definitions: BTreeMap<MacroName, BTreeMap<u64, Vec<MacroDefinition>>>,
    pending: BTreeMap<u64, Digest>,
    exports: BTreeMap<u64, ScopeKey>,
    exported_children: BTreeSet<ScopeKey>,
}
#[derive(Default, Clone)]
pub struct TextualScopes {
    scopes: BTreeMap<ScopeKey, Scope>,
    headers: usize,
}
const MAX_HEADERS: usize = 1_000_000;
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl TextualScopes {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(|| error("textual scope metadata overflow"))?;
            Ok(())
        };
        for scope in self.scopes.values() {
            add(192)?;
            add(scope
                .exports
                .len()
                .checked_mul(192)
                .ok_or_else(|| error("macro-use scope metadata overflow"))?)?;
            add(scope
                .pending
                .len()
                .checked_mul(128)
                .ok_or_else(|| error("textual pending metadata overflow"))?)?;
            for (name, positions) in &scope.definitions {
                add(name
                    .as_str()
                    .len()
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| error("textual name metadata overflow"))?)?;
                for definitions in positions.values() {
                    add(192)?;
                    add(definitions
                        .capacity()
                        .checked_mul(std::mem::size_of::<MacroDefinition>())
                        .ok_or_else(|| error("textual binding metadata overflow"))?)?;
                }
            }
        }
        Ok(bytes)
    }
    fn admit(&self, count: usize) -> Result<usize, Diagnostic> {
        self.headers
            .checked_add(count)
            .filter(|n| *n <= MAX_HEADERS)
            .ok_or_else(|| error("macro scope header limit exceeded"))
    }
    fn get(&self, key: ScopeKey) -> Result<&Scope, Diagnostic> {
        self.scopes
            .get(&key)
            .ok_or_else(|| error("unknown textual scope"))
    }
    pub(crate) fn parent(&self, key: ScopeKey) -> Result<Option<(ScopeKey, u64)>, Diagnostic> {
        Ok(self.get(key)?.parent)
    }
    pub fn add_root(&mut self, key: ScopeKey) -> Result<(), Diagnostic> {
        self.insert_scope(key, Scope::default())
    }
    pub fn add_child(
        &mut self,
        parent: ScopeKey,
        at: u64,
        ordinal: u32,
    ) -> Result<ScopeKey, Diagnostic> {
        self.get(parent)?;
        let key = parent.child(at, ordinal);
        self.insert_scope(
            key,
            Scope {
                parent: Some((parent, at)),
                ..Scope::default()
            },
        )?;
        Ok(key)
    }
    fn insert_scope(&mut self, key: ScopeKey, scope: Scope) -> Result<(), Diagnostic> {
        if self.scopes.contains_key(&key) {
            return Err(error("duplicate textual scope"));
        }
        let count = self.admit(1)?;
        self.scopes.insert(key, scope);
        self.headers = count;
        Ok(())
    }
    pub fn define(
        &mut self,
        key: ScopeKey,
        at: u64,
        name: MacroName,
        definition: MacroDefinition,
    ) -> Result<(), Diagnostic> {
        definition.validate()?;
        let scope = self.get(key)?;
        if scope.pending.contains_key(&at)
            || scope
                .definitions
                .get(&name)
                .is_some_and(|d| d.contains_key(&at))
        {
            return Err(error("duplicate or pending macro declaration position"));
        }
        let count = self.admit(1)?;
        self.scopes
            .get_mut(&key)
            .expect("validated scope")
            .definitions
            .entry(name)
            .or_default()
            .insert(at, vec![definition]);
        self.headers = count;
        Ok(())
    }
    pub fn block(&mut self, key: ScopeKey, at: u64, invocation: Digest) -> Result<(), Diagnostic> {
        let scope = self.get(key)?;
        if scope.pending.contains_key(&at)
            || scope.definitions.values().any(|d| d.contains_key(&at))
        {
            return Err(error("duplicate expansion position"));
        }
        let count = self.admit(1)?;
        self.scopes
            .get_mut(&key)
            .expect("validated scope")
            .pending
            .insert(at, invocation);
        self.headers = count;
        Ok(())
    }
    /// Argument evaluation precedes this invocation's generated bindings.
    pub(crate) fn omit_invocation_position(
        &mut self,
        key: ScopeKey,
        at: u64,
    ) -> Result<(), Diagnostic> {
        let scope = self
            .scopes
            .get_mut(&key)
            .ok_or_else(|| error("argument textual scope absent"))?;
        scope.pending.remove(&at);
        for positions in scope.definitions.values_mut() {
            positions.remove(&at);
        }
        Ok(())
    }
    /// Validate the complete declaration batch before removing its blocker.
    /// Positions are source-order ordinals supplied by the source index, not
    /// upstream IDs or byte offsets used as persistent item identities.
    /// This publishes the outward bindings after an expansion completes.
    /// Calls within its generated source require their own ordered scope facts.
    pub fn publish(
        &mut self,
        key: ScopeKey,
        at: u64,
        invocation: Digest,
        definitions: Vec<(MacroName, MacroDefinition)>,
    ) -> Result<(), Diagnostic> {
        let count = self.preflight_publish(key, at, invocation, &definitions)?;
        let scope = self.scopes.get_mut(&key).expect("validated scope");
        // Preserve every emitted binding: the latest compatible name shadows
        // predecessors only after hygiene filtering. Lexical lookup uses last.
        for (name, definition) in definitions {
            scope
                .definitions
                .entry(name)
                .or_default()
                .entry(at)
                .or_default()
                .push(definition);
        }
        scope.pending.remove(&at);
        self.headers = count;
        Ok(())
    }
    /// Check the same publication without changing the scope. Declaration
    /// registration may join this check before either table publishes a row.
    pub(crate) fn preflight_publish(
        &self,
        key: ScopeKey,
        at: u64,
        invocation: Digest,
        definitions: &[(MacroName, MacroDefinition)],
    ) -> Result<usize, Diagnostic> {
        let scope = self.get(key)?;
        if scope.pending.get(&at) != Some(&invocation) {
            return Err(error("stale or missing expansion input"));
        }
        let count = self.admit(definitions.len())?;
        for (name, definition) in definitions {
            definition.validate()?;
            if scope
                .definitions
                .get(name)
                .is_some_and(|d| d.contains_key(&at))
            {
                return Err(error("conflicting generated declaration position"));
            }
        }
        Ok(count)
    }
    pub(crate) fn publish_prefix(
        &mut self,
        key: ScopeKey,
        at: u64,
        invocation: Digest,
        definitions: Vec<(MacroName, MacroDefinition)>,
    ) -> Result<(), Diagnostic> {
        if self.get(key)?.pending.get(&at) != Some(&invocation) {
            return Err(error("stale generated prefix producer"));
        }
        let count = self.admit(definitions.len())?;
        for (_, definition) in &definitions {
            definition.validate()?;
        }
        let scope = self.scopes.get_mut(&key).expect("validated prefix scope");
        for (name, definition) in definitions {
            scope
                .definitions
                .entry(name)
                .or_default()
                .entry(at)
                .or_default()
                .push(definition);
        }
        self.headers = count;
        Ok(())
    }
    pub(crate) fn complete_prefix(
        &mut self,
        key: ScopeKey,
        at: u64,
        invocation: Digest,
    ) -> Result<(), Diagnostic> {
        if self.get(key)?.pending.get(&at) != Some(&invocation) {
            return Err(error("stale completed prefix producer"));
        }
        self.scopes
            .get_mut(&key)
            .expect("validated prefix scope")
            .pending
            .remove(&at);
        Ok(())
    }
    /// A derive child sees preceding outputs, but not its own or later outputs.
    /// Retire only this producer in the immutable view; keep every other frontier.
    pub(crate) fn prefix_view(
        &self,
        key: ScopeKey,
        at: u64,
        invocation: Digest,
    ) -> Result<Self, Diagnostic> {
        if self.get(key)?.pending.get(&at) != Some(&invocation) {
            return Err(error("stale derive prefix view producer"));
        }
        if self.metadata_bytes()? > 32 * 1024 * 1024 {
            return Err(error("derive prefix view metadata limit"));
        }
        let mut view = self.clone();
        view.complete_prefix(key, at, invocation)?;
        Ok(view)
    }
    /// A module's textual macros escape only through this explicit lexical
    /// edge. Its child candidates are read locally, never through its parent.
    pub(crate) fn macro_use_module(
        &mut self,
        parent: ScopeKey,
        child: ScopeKey,
        close: u64,
    ) -> Result<(), Diagnostic> {
        let Some((actual_parent, opening)) = self.get(child)?.parent else {
            return Err(error("macro-use module has no lexical parent"));
        };
        // `close` is an exclusive source-range end. The last closing token
        // precedes even a following invocation with no intervening whitespace.
        let at = close
            .checked_sub(1)
            .ok_or_else(|| error("macro-use closing position underflow"))?;
        let scope = self.get(parent)?;
        if actual_parent != parent
            || at < opening
            || scope.exports.contains_key(&at)
            || scope.exported_children.contains(&child)
        {
            return Err(error(
                "macro-use module edge is duplicated or has the wrong parent/position",
            ));
        }
        let count = self.admit(1)?;
        let scope = self.scopes.get_mut(&parent).expect("validated parent");
        scope.exports.insert(at, child);
        scope.exported_children.insert(child);
        self.headers = count;
        Ok(())
    }
    pub(crate) fn local_candidates(
        &self,
        key: ScopeKey,
        before: u64,
        name: &MacroName,
        require_frontiers: bool,
    ) -> Result<ScopeCandidates, Diagnostic> {
        enum Work<'a> {
            Scope(ScopeKey, u64, usize),
            Definition(&'a MacroDefinition),
        }
        let mut work = Vec::new();
        work.try_reserve_exact(1)
            .map_err(|_| error("macro-use candidate work allocation"))?;
        work.push(Work::Scope(key, before, 0));
        let mut candidates = Vec::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Definition(definition) => {
                    if candidates.len() >= MAX_HEADERS {
                        return Err(error("macro-use candidate count limit"));
                    }
                    candidates
                        .try_reserve(1)
                        .map_err(|_| error("macro-use candidate allocation"))?;
                    candidates.push(definition.clone());
                }
                Work::Scope(key, before, depth) => {
                    if depth >= 256 {
                        return Err(error("macro-use scope depth limit"));
                    }
                    let scope = self.get(key)?;
                    if require_frontiers
                        && let Some((_, invocation)) = scope.pending.range(..before).next()
                    {
                        return Ok(ScopeCandidates::NeedInput(*invocation));
                    }
                    let definitions = scope.definitions.get(name);
                    let count = scope
                        .exports
                        .range(..before)
                        .count()
                        .checked_add(
                            definitions.map_or(0, |positions| positions.range(..before).count()),
                        )
                        .filter(|n| *n <= MAX_HEADERS)
                        .ok_or_else(|| error("macro-use candidate event count limit"))?;
                    count
                        .checked_mul(64)
                        .filter(|n| *n <= 64 * 1024 * 1024)
                        .ok_or_else(|| error("macro-use candidate event metadata limit"))?;
                    let mut positions = BTreeSet::new();
                    positions.extend(scope.exports.range(..before).map(|(at, _)| *at));
                    if let Some(definitions) = definitions {
                        positions.extend(definitions.range(..before).map(|(at, _)| *at));
                    }
                    for at in positions {
                        if let Some(child) = scope.exports.get(&at) {
                            if work.len() >= MAX_HEADERS {
                                return Err(error("macro-use candidate work limit"));
                            }
                            work.try_reserve(1)
                                .map_err(|_| error("macro-use candidate work allocation"))?;
                            work.push(Work::Scope(*child, at, depth + 1));
                        }
                        if let Some(batch) = definitions.and_then(|positions| positions.get(&at)) {
                            work.len()
                                .checked_add(batch.len())
                                .filter(|n| *n <= MAX_HEADERS)
                                .ok_or_else(|| error("macro-use candidate work limit"))?;
                            work.try_reserve(batch.len())
                                .map_err(|_| error("macro-use candidate work allocation"))?;
                            work.extend(batch.iter().map(Work::Definition));
                        }
                    }
                }
            }
        }
        Ok(ScopeCandidates::Definitions(candidates))
    }
    pub(crate) fn local_bindings(
        &self,
        key: ScopeKey,
        before: u64,
    ) -> Result<LocalBindings, Diagnostic> {
        let mut work = Vec::new();
        work.try_reserve_exact(1)
            .map_err(|_| error("local binding scope allocation"))?;
        work.push((key, before, 0usize));
        let mut names = BTreeSet::new();
        let mut bytes = 0usize;
        while let Some((key, before, depth)) = work.pop() {
            if depth >= 256 {
                return Err(error("local binding scope depth limit"));
            }
            let scope = self.get(key)?;
            if let Some((_, input)) = scope.pending.range(..before).next() {
                return Ok(LocalBindings::NeedInput(*input));
            }
            for (name, positions) in &scope.definitions {
                if positions.range(..before).next().is_some() && !names.contains(name) {
                    bytes = bytes
                        .checked_add(name.as_str().len())
                        .and_then(|n| n.checked_add(96))
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| error("local binding name metadata limit"))?;
                    names.insert(name.clone());
                }
            }
            work.len()
                .checked_add(scope.exports.range(..before).count())
                .filter(|n| *n <= MAX_HEADERS)
                .ok_or_else(|| error("local binding scope work limit"))?;
            work.try_reserve(scope.exports.range(..before).count())
                .map_err(|_| error("local binding scope work allocation"))?;
            work.extend(
                scope
                    .exports
                    .range(..before)
                    .map(|(at, child)| (*child, *at, depth + 1)),
            );
        }
        let mut bindings = Vec::new();
        for name in names {
            let candidates = match self.local_candidates(key, before, &name, true)? {
                ScopeCandidates::NeedInput(input) => return Ok(LocalBindings::NeedInput(input)),
                ScopeCandidates::Definitions(candidates) => candidates,
            };
            bindings
                .len()
                .checked_add(candidates.len())
                .filter(|n| *n <= MAX_HEADERS)
                .ok_or_else(|| error("local binding count limit"))?;
            bytes = candidates
                .len()
                .checked_mul(
                    name.as_str()
                        .len()
                        .checked_add(std::mem::size_of::<(MacroName, MacroDefinition)>())
                        .ok_or_else(|| error("local binding size overflow"))?,
                )
                .and_then(|n| bytes.checked_add(n))
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| error("local binding metadata limit"))?;
            bindings
                .try_reserve_exact(candidates.len())
                .map_err(|_| error("local binding allocation"))?;
            bindings.extend(
                candidates
                    .into_iter()
                    .rev()
                    .map(|definition| (name.clone(), definition)),
            );
        }
        Ok(LocalBindings::Ready(bindings))
    }
    /// Walk all visible bindings nearest-first. An outer producer can still
    /// affect origin ambiguity even when an inner lexical candidate exists.
    pub fn candidates(
        &self,
        key: ScopeKey,
        before: u64,
        name: &MacroName,
    ) -> Result<ScopeCandidates, Diagnostic> {
        self.candidates_using(key, before, name, true)
    }
    /// Unsealed pure discovery candidates require final strict selection revalidation.
    pub(crate) fn candidates_for_discovery(
        &self,
        key: ScopeKey,
        before: u64,
        name: &MacroName,
    ) -> Result<ScopeCandidates, Diagnostic> {
        self.candidates_using(key, before, name, false)
    }
    fn candidates_using(
        &self,
        mut key: ScopeKey,
        mut before: u64,
        name: &MacroName,
        require_frontiers: bool,
    ) -> Result<ScopeCandidates, Diagnostic> {
        let mut candidates = Vec::new();
        loop {
            let scope = self.get(key)?;
            match self.local_candidates(key, before, name, require_frontiers)? {
                ScopeCandidates::NeedInput(input) => return Ok(ScopeCandidates::NeedInput(input)),
                ScopeCandidates::Definitions(local) => {
                    candidates
                        .len()
                        .checked_add(local.len())
                        .filter(|n| *n <= MAX_HEADERS)
                        .ok_or_else(|| error("macro candidate limit exceeded"))?;
                    candidates
                        .try_reserve(local.len())
                        .map_err(|_| error("macro candidate allocation failed"))?;
                    candidates.extend(local);
                }
            }
            let Some((parent, at)) = scope.parent else {
                return Ok(ScopeCandidates::Definitions(candidates));
            };
            key = parent;
            before = at;
        }
    }
    pub fn lookup(
        &self,
        mut key: ScopeKey,
        mut before: u64,
        name: &MacroName,
    ) -> Result<ScopeLookup, Diagnostic> {
        loop {
            let scope = self.get(key)?;
            match self.local_candidates(key, before, name, true)? {
                ScopeCandidates::NeedInput(input) => return Ok(ScopeLookup::NeedInput(input)),
                ScopeCandidates::Definitions(definitions) => {
                    if let Some(definition) = definitions.into_iter().next() {
                        return Ok(ScopeLookup::Defined(definition));
                    }
                }
            }
            let Some((parent, at)) = scope.parent else {
                return Ok(ScopeLookup::Absent);
            };
            key = parent;
            before = at;
        }
    }
}

#[cfg(test)]
mod macro_use_tests {
    use super::*;
    #[test]
    fn macro_use_edges_preserve_pending_children_shadow_order_and_parent_identity() {
        let mut scopes = TextualScopes::default();
        let root = ScopeKey::root(Digest([71; 32]));
        scopes.add_root(root).unwrap();
        let child = scopes.add_child(root, 5, 0).unwrap();
        let sibling = scopes.add_child(root, 6, 1).unwrap();
        let name = MacroName::parse("leaf", 2024).unwrap();
        let definition = |n| MacroDefinition {
            identity: Digest([n; 32]),
            defining_unit: Digest([71; 32]),
            definition_edition: 2024,
        };
        scopes
            .define(root, 1, name.clone(), definition(72))
            .unwrap();
        scopes
            .define(child, 7, name.clone(), definition(73))
            .unwrap();
        let pending = Digest([74; 32]);
        scopes.block(child, 15, pending).unwrap();
        scopes.macro_use_module(root, child, 30).unwrap();
        assert_eq!(
            scopes.lookup(root, 20, &name).unwrap(),
            ScopeLookup::Defined(definition(72))
        );
        assert_eq!(
            scopes.lookup(root, 30, &name).unwrap(),
            ScopeLookup::NeedInput(pending)
        );
        let ScopeCandidates::Definitions(available) =
            scopes.candidates_for_discovery(root, 30, &name).unwrap()
        else {
            panic!("available discovery did not retain child candidate");
        };
        assert_eq!(available, vec![definition(73), definition(72)]);
        let before = scopes.metadata_bytes().unwrap();
        assert!(scopes.macro_use_module(child, sibling, 31).is_err());
        assert!(scopes.macro_use_module(root, child, 31).is_err());
        assert_eq!(scopes.metadata_bytes().unwrap(), before);
        scopes
            .publish(child, 15, pending, vec![(name.clone(), definition(75))])
            .unwrap();
        assert_eq!(
            scopes.lookup(root, 30, &name).unwrap(),
            ScopeLookup::Defined(definition(75))
        );
        scopes
            .define(root, 31, name.clone(), definition(76))
            .unwrap();
        assert_eq!(
            scopes.lookup(root, 32, &name).unwrap(),
            ScopeLookup::Defined(definition(76))
        );
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::*;
    #[test]
    fn generated_prefix_retains_producer_until_explicit_completion_and_preserves_binding_order() {
        let scope = ScopeKey::root(Digest([91; 32]));
        let producer = Digest([92; 32]);
        let name = MacroName::parse("helper", 2024).unwrap();
        let definition = |n| MacroDefinition {
            identity: Digest([n; 32]),
            defining_unit: Digest([91; 32]),
            definition_edition: 2024,
        };
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope).unwrap();
        scopes.block(scope, 5, producer).unwrap();
        assert!(
            scopes
                .publish_prefix(
                    scope,
                    5,
                    Digest([93; 32]),
                    vec![(name.clone(), definition(94))]
                )
                .is_err()
        );
        scopes
            .publish_prefix(scope, 5, producer, vec![(name.clone(), definition(94))])
            .unwrap();
        assert!(
            matches!(scopes.lookup(scope,6,&name).unwrap(),ScopeLookup::NeedInput(id) if id==producer)
        );
        scopes
            .publish_prefix(scope, 5, producer, vec![(name.clone(), definition(95))])
            .unwrap();
        assert!(scopes.complete_prefix(scope, 5, Digest([93; 32])).is_err());
        assert!(matches!(
            scopes.lookup(scope, 6, &name).unwrap(),
            ScopeLookup::NeedInput(_)
        ));
        scopes.complete_prefix(scope, 5, producer).unwrap();
        assert!(
            matches!(scopes.lookup(scope,6,&name).unwrap(),ScopeLookup::Defined(d) if d==definition(95))
        );
        assert!(scopes.complete_prefix(scope, 5, producer).is_err());
    }
    #[test]
    fn derive_prefix_views_preserve_prior_bindings_and_other_frontiers_after_shadow() {
        let scope = ScopeKey::root(Digest([81; 32]));
        let producer = Digest([82; 32]);
        let other = Digest([83; 32]);
        let name = MacroName::parse("prefix", 2024).unwrap();
        let definition = |n| MacroDefinition {
            identity: Digest([n; 32]),
            defining_unit: Digest([81; 32]),
            definition_edition: 2024,
        };
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope).unwrap();
        scopes.block(scope, 5, producer).unwrap();
        scopes.block(scope, 9, other).unwrap();
        assert!(scopes.prefix_view(scope, 5, other).is_err());
        let first = scopes.prefix_view(scope, 5, producer).unwrap();
        scopes
            .publish_prefix(scope, 5, producer, vec![(name.clone(), definition(84))])
            .unwrap();
        let second = scopes.prefix_view(scope, 5, producer).unwrap();
        scopes
            .publish_prefix(scope, 5, producer, vec![(name.clone(), definition(85))])
            .unwrap();
        scopes.complete_prefix(scope, 5, producer).unwrap();
        assert!(matches!(
            first.lookup(scope, 6, &name).unwrap(),
            ScopeLookup::Absent
        ));
        assert!(
            matches!(second.lookup(scope, 6, &name).unwrap(), ScopeLookup::Defined(d) if d == definition(84))
        );
        assert!(
            matches!(scopes.lookup(scope, 6, &name).unwrap(), ScopeLookup::Defined(d) if d == definition(85))
        );
        assert!(
            matches!(second.lookup(scope, 10, &name).unwrap(), ScopeLookup::NeedInput(id) if id == other)
        );
    }
}
