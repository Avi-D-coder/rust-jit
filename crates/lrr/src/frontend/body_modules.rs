//! Ephemeral, synchronous module-route tables for one selected entered body.
//! Original access routes remain separate from canonical module identity.
use super::{
    macro_names::{MacroNamespaces, MacroPath, ModuleLookup, PathLookup},
    module_paths::ModulePath,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::MacroName,
    syntax::{
        entered_macros::EnteredMacroFacts,
        imports::{ImportKind, ImportSegment},
        outline::ScopeKey,
        visibility::VisibilityScope,
    },
};
use std::collections::{BTreeMap, BTreeSet};
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn charge(bytes: &mut usize, n: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(|| error("body module metadata budget exceeded"))?;
    Ok(())
}
fn step(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .filter(|n| *n <= 100_000 * 257)
        .ok_or_else(|| error("body module work limit exceeded"))?;
    Ok(())
}
fn path_bytes(path: &ModulePath) -> Result<usize, Diagnostic> {
    ModulePath::validate_components(&path.names)?;
    path.names
        .iter()
        .try_fold(std::mem::size_of::<ModulePath>(), |n, s| {
            n.checked_add(std::mem::size_of::<MacroName>())
                .and_then(|n| n.checked_add(s.as_str().len()))
        })
        .ok_or_else(|| error("body module path size overflow"))
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Candidate {
    pub identity: ModulePath,
    pub route: ModulePath,
    visibility: Option<VisibilityScope>,
}
#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct State {
    pub candidates: BTreeSet<Candidate>,
    pub pending: BTreeSet<Digest>,
    pub denied: BTreeSet<ModulePath>,
    macro_facet: bool,
}
impl State {
    fn empty(&self) -> bool {
        self.candidates.is_empty() && self.pending.is_empty() && self.denied.is_empty()
    }
    pub fn ambiguous(&self) -> bool {
        let Some(first) = self.candidates.first() else {
            return false;
        };
        self.candidates.iter().any(|c| c.identity != first.identity)
    }
    fn candidate(&mut self, c: &Candidate, bytes: &mut usize) -> Result<(), Diagnostic> {
        if !self.candidates.contains(c) {
            let visibility = match &c.visibility {
                None | Some(VisibilityScope::Public) => 0,
                Some(VisibilityScope::Within { module_path, .. }) => module_path
                    .iter()
                    .try_fold(32usize, |n, s| {
                        s.len()
                            .checked_add(std::mem::size_of::<String>())
                            .and_then(|size| n.checked_add(size))
                    })
                    .ok_or_else(|| error("body module visibility size overflow"))?,
            };
            charge(
                bytes,
                path_bytes(&c.identity)?
                    .checked_add(path_bytes(&c.route)?)
                    .and_then(|n| n.checked_add(visibility))
                    .ok_or_else(|| error("body module candidate size overflow"))?,
            )?;
            self.candidates.insert(c.clone());
        }
        Ok(())
    }
    fn merge(&mut self, other: &Self, bytes: &mut usize) -> Result<(), Diagnostic> {
        self.macro_facet |= other.macro_facet;
        for c in &other.candidates {
            self.candidate(c, bytes)?;
        }
        for input in &other.pending {
            if !self.pending.contains(input) {
                charge(bytes, 32)?;
                self.pending.insert(*input);
            }
        }
        for route in &other.denied {
            if !self.denied.contains(route) {
                charge(bytes, path_bytes(route)?)?;
                self.denied.insert(route.clone());
            }
        }
        Ok(())
    }
    fn from_route(
        graph: &MacroNamespaces,
        route: ModulePath,
        caller: &ScopeKey,
        bytes: &mut usize,
    ) -> Result<Self, Diagnostic> {
        let mut result = Self::default();
        match graph.lookup_module_from(route.unit, &route.names, caller)? {
            ModuleLookup::Defined {
                unit,
                components,
                visibility,
            } => result.candidate(
                &Candidate {
                    identity: ModulePath {
                        unit,
                        names: components,
                    },
                    route,
                    visibility: Some(visibility),
                },
                bytes,
            )?,
            ModuleLookup::Ambiguous(paths) => {
                for (unit, names) in paths {
                    charge(bytes, path_bytes(&route)?)?;
                    result.candidate(
                        &Candidate {
                            identity: ModulePath { unit, names },
                            route: route.clone(),
                            visibility: None,
                        },
                        bytes,
                    )?;
                }
            }
            ModuleLookup::Inaccessible { .. } => {
                charge(bytes, path_bytes(&route)?)?;
                result.denied.insert(route);
            }
            ModuleLookup::Absent => {
                if !route.names.is_empty() {
                    match graph.lookup_from(&MacroPath::new(route.unit, route.names)?, caller)? {
                        PathLookup::Defined(_) | PathLookup::Ambiguous(_) => {
                            result.macro_facet = true
                        }
                        PathLookup::NeedInput(input) => {
                            charge(bytes, 32)?;
                            result.pending.insert(input);
                        }
                        PathLookup::Absent | PathLookup::Inaccessible(_) => {}
                    }
                }
            }
        }
        Ok(result)
    }
}
pub(super) struct Modules<'a> {
    facts: &'a EnteredMacroFacts,
    graph: &'a MacroNamespaces,
    caller: &'a ScopeKey,
    edition: u16,
    named: BTreeMap<(u32, MacroName), Vec<usize>>,
    globs: BTreeMap<u32, Vec<usize>>,
    pending: BTreeMap<u32, BTreeSet<Digest>>,
    states: BTreeMap<usize, State>,
}
impl<'a> Modules<'a> {
    pub fn build(
        facts: &'a EnteredMacroFacts,
        graph: &'a MacroNamespaces,
        caller: &'a ScopeKey,
        edition: u16,
        selected_scope: u32,
        work: &mut usize,
        bytes: &mut usize,
    ) -> Result<Self, Diagnostic> {
        let mut this = Self {
            facts,
            graph,
            caller,
            edition,
            named: BTreeMap::new(),
            globs: BTreeMap::new(),
            pending: BTreeMap::new(),
            states: BTreeMap::new(),
        };
        if facts.imports.len() > 100_000 {
            return Err(error("body module import count exceeded"));
        }
        let mut scopes = BTreeSet::new();
        let mut current = Some(selected_scope);
        while let Some(scope) = current {
            step(work)?;
            if !scopes.insert(scope) {
                return Err(error("cyclic body module scope ancestry"));
            }
            current = facts
                .scopes
                .iter()
                .find(|s| s.ordinal == scope)
                .ok_or_else(|| error("selected body module scope missing"))?
                .parent;
        }
        for (i, row) in facts.imports.iter().enumerate() {
            step(work)?;
            if !scopes.contains(&row.scope) {
                continue;
            }
            charge(bytes, std::mem::size_of::<(usize, State)>())?;
            if let Some(input) = row.attribute_input {
                charge(bytes, 32)?;
                this.pending.entry(row.scope).or_default().insert(input);
            }
            if row.source.kind == ImportKind::Glob {
                charge(bytes, std::mem::size_of::<usize>())?;
                let list = this.globs.entry(row.scope).or_default();
                list.try_reserve(1)
                    .map_err(|_| error("body module glob index allocation failed"))?;
                list.push(i);
            } else if let Some(name) = super::body_imports::name(row, edition)? {
                charge(
                    bytes,
                    name.as_str()
                        .len()
                        .checked_add(std::mem::size_of::<(u32, MacroName, usize)>())
                        .ok_or_else(|| error("body module named index size overflow"))?,
                )?;
                let list = this.named.entry((row.scope, name)).or_default();
                list.try_reserve(1)
                    .map_err(|_| error("body module named index allocation failed"))?;
                list.push(i);
            }
            this.states.insert(i, State::default());
        }
        let mut seen = BTreeSet::new();
        loop {
            let mut next = BTreeMap::new();
            let mut table_bytes = this
                .states
                .len()
                .checked_mul(std::mem::size_of::<(usize, State)>())
                .ok_or_else(|| error("body module table size overflow"))?;
            charge(&mut table_bytes, 0)?;
            for i in this.states.keys() {
                step(work)?;
                let row = &facts.imports[*i];
                let mut path = row.source.path.as_slice();
                if path.len() > 1 && path.last() == Some(&ImportSegment::SelfModule) {
                    path = &path[..path.len() - 1];
                }
                let mut state = this.resolve_using(
                    row.scope,
                    path,
                    row.source.absolute,
                    Some(*i),
                    work,
                    &mut table_bytes,
                )?;
                if let Some(input) = row.attribute_input {
                    charge(&mut table_bytes, 32)?;
                    state.pending.insert(input);
                }
                next.insert(*i, state);
            }
            if next == this.states {
                break;
            }
            let digest = state_digest(&next);
            if seen.contains(&digest) {
                return Err(error(
                    "cyclic body module strong/glob selection has no stable result",
                ));
            }
            charge(bytes, 32)?;
            seen.insert(digest);
            this.states = next;
        }
        Ok(this)
    }
    pub fn resolve(
        &self,
        scope: u32,
        parts: &[ImportSegment],
        absolute: bool,
        work: &mut usize,
        bytes: &mut usize,
    ) -> Result<State, Diagnostic> {
        self.resolve_using(scope, parts, absolute, None, work, bytes)
    }
    fn resolve_using(
        &self,
        mut scope: u32,
        parts: &[ImportSegment],
        absolute: bool,
        ignored: Option<usize>,
        work: &mut usize,
        bytes: &mut usize,
    ) -> Result<State, Diagnostic> {
        step(work)?;
        if absolute || !matches!(parts.first(), Some(ImportSegment::Name(_))) {
            return self.source(parts, absolute, bytes);
        }
        let ImportSegment::Name(first) = &parts[0] else {
            unreachable!()
        };
        let name = MacroName::parse(first, self.edition)?;
        let mut result = State::default();
        loop {
            step(work)?;
            if let Some(inputs) = self.pending.get(&scope) {
                for input in inputs {
                    if !result.pending.contains(input) {
                        charge(bytes, 32)?;
                        result.pending.insert(*input);
                    }
                }
                return Ok(result);
            }
            let mut strong = State::default();
            let mut module_binding = false;
            if let Some(indices) = self.named.get(&(scope, name.clone())) {
                for i in indices {
                    step(work)?;
                    if ignored == Some(*i) {
                        continue;
                    }
                    let state = &self.states[i];
                    // An unresolved strong import cannot acquire a module facet
                    // by feeding its own same-name weak contribution back into it.
                    module_binding |= !state.macro_facet || !state.empty();
                    strong.merge(state, bytes)?;
                }
            }
            if module_binding {
                result.merge(&strong, bytes)?;
                break;
            }
            result.macro_facet |= strong.macro_facet;
            if let Some(indices) = self.globs.get(&scope) {
                for i in indices {
                    step(work)?;
                    if ignored == Some(*i) {
                        continue;
                    }
                    let target = &self.states[i];
                    // Private members are filtered separately from private glob targets.
                    for input in &target.pending {
                        if !result.pending.contains(input) {
                            charge(bytes, 32)?;
                            result.pending.insert(*input);
                        }
                    }
                    for denied in &target.denied {
                        if !result.denied.contains(denied) {
                            charge(bytes, path_bytes(denied)?)?;
                            result.denied.insert(denied.clone());
                        }
                    }
                    if target.ambiguous() {
                        result.merge(target, bytes)?;
                        continue;
                    }
                    for c in &target.candidates {
                        let route = append(&c.route, std::slice::from_ref(&name), bytes)?;
                        let member = State::from_route(self.graph, route, self.caller, bytes)?;
                        for c in member.candidates {
                            result.candidate(&c, bytes)?;
                        }
                    }
                }
            }
            let current = self
                .facts
                .scopes
                .iter()
                .find(|s| s.ordinal == scope)
                .ok_or_else(|| error("body module scope missing"))?;
            let Some(parent) = current.parent else {
                result.merge(&self.source(&parts[..1], false, bytes)?, bytes)?;
                break;
            };
            scope = parent;
        }
        if parts.len() == 1 || result.ambiguous() {
            return Ok(result);
        }
        let mut tail = Vec::new();
        tail.try_reserve(parts.len() - 1)
            .map_err(|_| error("body module tail allocation failed"))?;
        for part in &parts[1..] {
            let ImportSegment::Name(name) = part else {
                return Err(error("invalid body module route qualifier"));
            };
            tail.push(MacroName::parse(name, self.edition)?);
        }
        let mut extended = State::default();
        for input in &result.pending {
            charge(bytes, 32)?;
            extended.pending.insert(*input);
        }
        for denied in &result.denied {
            charge(bytes, path_bytes(denied)?)?;
            extended.denied.insert(denied.clone());
        }
        for c in result.candidates {
            let route = append(&c.route, &tail, bytes)?;
            extended.merge(
                &State::from_route(self.graph, route, self.caller, bytes)?,
                bytes,
            )?;
        }
        Ok(extended)
    }
    fn source(
        &self,
        parts: &[ImportSegment],
        absolute: bool,
        bytes: &mut usize,
    ) -> Result<State, Diagnostic> {
        match self
            .graph
            .source_module_route(parts, absolute, self.caller, self.edition)?
        {
            Some(route) => State::from_route(self.graph, route, self.caller, bytes),
            None => Ok(State::default()),
        }
    }
}
fn append(
    route: &ModulePath,
    tail: &[MacroName],
    bytes: &mut usize,
) -> Result<ModulePath, Diagnostic> {
    let len = route
        .names
        .len()
        .checked_add(tail.len())
        .filter(|n| *n <= 256)
        .ok_or_else(|| error("body module route depth exceeded"))?;
    let mut names = Vec::new();
    let size = tail
        .iter()
        .try_fold(path_bytes(route)?, |n, s| {
            n.checked_add(std::mem::size_of::<MacroName>())
                .and_then(|n| n.checked_add(s.as_str().len()))
        })
        .ok_or_else(|| error("body module route size overflow"))?;
    charge(bytes, size)?;
    names
        .try_reserve(len)
        .map_err(|_| error("body module route allocation failed"))?;
    names.extend_from_slice(&route.names);
    names.extend_from_slice(tail);
    Ok(ModulePath {
        unit: route.unit,
        names,
    })
}
fn state_digest(states: &BTreeMap<usize, State>) -> Digest {
    fn path(e: &mut StableEncoder, p: &ModulePath) {
        e.digest(p.unit);
        e.sequence(&p.names, |e, n| e.string(n.as_str()));
    }
    let mut e = StableEncoder::new("lrr/v2/body-module-fixed-point");
    e.u64(u64::try_from(states.len()).expect("bounded state count"));
    for (index, s) in states {
        e.u64(u64::try_from(*index).expect("bounded import index"));
        e.u32(u32::from(s.macro_facet));
        e.u64(u64::try_from(s.candidates.len()).expect("bounded candidates"));
        for c in &s.candidates {
            path(&mut e, &c.identity);
            path(&mut e, &c.route);
            match &c.visibility {
                None => e.u32(0),
                Some(VisibilityScope::Public) => e.u32(1),
                Some(VisibilityScope::Within { unit, module_path }) => {
                    e.u32(2);
                    e.digest(*unit);
                    e.sequence(module_path, |e, n| e.string(n));
                }
            }
        }
        e.u64(u64::try_from(s.pending.len()).expect("bounded pending"));
        for d in &s.pending {
            e.digest(*d);
        }
        e.u64(u64::try_from(s.denied.len()).expect("bounded denied"));
        for p in &s.denied {
            path(&mut e, p);
        }
    }
    e.finish()
}
