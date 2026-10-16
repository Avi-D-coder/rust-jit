//! Per-walk source candidate tables. SCCs use synchronous updates so cyclic
//! strong/weak choices never depend on worklist order. No results are persisted.
use super::{
    macro_names::{MacroNamespaces, MacroPath, NameBindingClass, PathLookup},
    module_paths::Resolution,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::MacroDefinition,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Candidate {
    definition: MacroDefinition,
    visibility: VisibilityScope,
}
#[derive(Clone, Default, Eq, PartialEq)]
struct State {
    definitions: BTreeSet<Candidate>,
    denied: BTreeSet<MacroPath>,
    pending: BTreeSet<Digest>,
    candidate_bytes: usize,
}
impl State {
    fn candidate(
        &mut self,
        definition: &MacroDefinition,
        visibility: &VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if self
            .definitions
            .iter()
            .any(|c| &c.definition == definition && &c.visibility == visibility)
        {
            return Ok(());
        }
        checked_add(
            &mut self.candidate_bytes,
            std::mem::size_of::<Candidate>() + visibility_bytes(visibility),
        )?;
        self.definitions.insert(Candidate {
            definition: definition.clone(),
            visibility: visibility.clone(),
        });
        Ok(())
    }
}
struct Edge {
    target: usize,
    visibility: VisibilityScope,
}
struct Node {
    key: (MacroPath, Option<ScopeKey>, NameBindingClass),
    own: State,
    strong: Vec<Edge>,
    weak: Vec<Edge>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn origin(path: &MacroPath) -> ScopeKey {
    ScopeKey {
        crate_key: path.unit,
        module_path: path.components()[..path.components().len() - 1]
            .iter()
            .map(|n| n.as_str().into())
            .collect(),
        owner: None,
    }
}
pub(super) fn intersection(a: &VisibilityScope, b: &VisibilityScope) -> Option<VisibilityScope> {
    match (a, b) {
        (VisibilityScope::Public, v) | (v, VisibilityScope::Public) => Some(v.clone()),
        (
            VisibilityScope::Within {
                unit: a_unit,
                module_path: a_path,
            },
            VisibilityScope::Within {
                unit: b_unit,
                module_path: b_path,
            },
        ) if a_unit == b_unit => {
            if a_path.starts_with(b_path) {
                Some(a.clone())
            } else if b_path.starts_with(a_path) {
                Some(b.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
fn checked_add(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(|| error("glob lookup metadata limit exceeded"))?;
    Ok(())
}
fn visibility_bytes(visibility: &VisibilityScope) -> usize {
    match visibility {
        VisibilityScope::Public => std::mem::size_of::<VisibilityScope>(),
        VisibilityScope::Within { module_path, .. } => {
            std::mem::size_of::<VisibilityScope>()
                + module_path
                    .iter()
                    .map(|name| name.len() + std::mem::size_of::<String>())
                    .sum::<usize>()
        }
    }
}
fn state_bytes(state: &State) -> Result<usize, Diagnostic> {
    let mut bytes = std::mem::size_of::<State>();
    for candidate in &state.definitions {
        checked_add(
            &mut bytes,
            std::mem::size_of::<Candidate>() + visibility_bytes(&candidate.visibility),
        )?;
    }
    for path in &state.denied {
        checked_add(&mut bytes, std::mem::size_of::<MacroPath>())?;
        for name in path.components() {
            checked_add(
                &mut bytes,
                name.as_str().len() + std::mem::size_of_val(name),
            )?;
        }
    }
    checked_add(
        &mut bytes,
        state
            .pending
            .len()
            .checked_mul(std::mem::size_of::<Digest>())
            .ok_or_else(|| error("glob state size overflow"))?,
    )?;
    Ok(bytes)
}
fn intern(
    key: (MacroPath, Option<ScopeKey>, NameBindingClass),
    ids: &mut BTreeMap<(MacroPath, Option<ScopeKey>, NameBindingClass), usize>,
    nodes: &mut Vec<Node>,
    bytes: &mut usize,
) -> Result<usize, Diagnostic> {
    if let Some(id) = ids.get(&key) {
        return Ok(*id);
    }
    if nodes.len() >= 1_000_000 {
        return Err(error("glob lookup goal limit exceeded"));
    }
    checked_add(bytes, std::mem::size_of::<Node>())?;
    for name in key.0.components() {
        checked_add(
            bytes,
            (name.as_str().len() + std::mem::size_of_val(name))
                .checked_mul(2)
                .ok_or_else(|| error("glob key size overflow"))?,
        )?;
    }
    if let Some(caller) = &key.1 {
        for name in &caller.module_path {
            checked_add(
                bytes,
                (name.len() + std::mem::size_of::<String>())
                    .checked_mul(2)
                    .ok_or_else(|| error("glob caller size overflow"))?,
            )?;
        }
    }
    let id = nodes.len();
    nodes
        .try_reserve(1)
        .map_err(|_| error("glob lookup goal allocation failed"))?;
    ids.insert(key.clone(), id);
    nodes.push(Node {
        key,
        own: State::default(),
        strong: vec![],
        weak: vec![],
    });
    Ok(id)
}
fn evaluate(node: &Node, states: &[State]) -> Result<State, Diagnostic> {
    let mut result = node.own.clone();
    for edge in &node.strong {
        let child = &states[edge.target];
        result.pending.extend(child.pending.iter().copied());
        result.denied.extend(child.denied.iter().cloned());
        for candidate in &child.definitions {
            if candidate.visibility.permits_reexport_as(&edge.visibility) {
                result.candidate(&candidate.definition, &edge.visibility)?;
            } else {
                result.denied.insert(node.key.0.clone());
            }
        }
    }
    if !result.definitions.is_empty() || !result.denied.is_empty() || !result.pending.is_empty() {
        return Ok(result);
    }
    for edge in &node.weak {
        let child = &states[edge.target];
        result.pending.extend(child.pending.iter().copied());
        for candidate in &child.definitions {
            let Some(visibility) = intersection(&candidate.visibility, &edge.visibility) else {
                continue;
            };
            if let Some(caller) = &node.key.1
                && !visibility.allows(caller)?
            {
                result.denied.insert(node.key.0.clone());
            }
            result.candidate(&candidate.definition, &visibility)?;
        }
    }
    Ok(result)
}
fn fingerprint(component: &[usize], states: &[State]) -> Digest {
    let mut e = StableEncoder::new("lrr/v2/glob-component-state");
    e.u64(u64::try_from(component.len()).expect("bounded component count"));
    for &id in component {
        let s = &states[id];
        e.u64(u64::try_from(s.definitions.len()).expect("bounded candidate count"));
        for c in &s.definitions {
            e.digest(c.definition.identity);
            e.digest(c.definition.defining_unit);
            e.u32(u32::from(c.definition.definition_edition));
            e.record(&c.visibility);
        }
        e.u64(u64::try_from(s.denied.len()).expect("bounded denial count"));
        for path in &s.denied {
            e.digest(path.unit);
            e.sequence(path.components(), |e, n| e.string(n.as_str()));
        }
        e.u64(u64::try_from(s.pending.len()).expect("bounded producer count"));
        for pending in &s.pending {
            e.digest(*pending);
        }
    }
    e.finish()
}
pub(super) fn lookup(
    graph: &MacroNamespaces,
    path: &MacroPath,
    caller: Option<&ScopeKey>,
) -> Result<PathLookup, Diagnostic> {
    lookup_in_class(graph, path, caller, NameBindingClass::Root)
}
pub(super) fn lookup_in_class(
    graph: &MacroNamespaces,
    path: &MacroPath,
    caller: Option<&ScopeKey>,
    class: NameBindingClass,
) -> Result<PathLookup, Diagnostic> {
    lookup_in_class_observed(
        graph,
        path,
        caller,
        class,
        crate::source::manifest::Namespace::Macro,
        &mut crate::frontend::names::reads::NamespaceReads::disabled(),
    )
}
pub(super) fn lookup_in_class_observed(
    graph: &MacroNamespaces,
    path: &MacroPath,
    caller: Option<&ScopeKey>,
    class: NameBindingClass,
    namespace: crate::source::manifest::Namespace,
    reads: &mut crate::frontend::names::reads::NamespaceReads,
) -> Result<PathLookup, Diagnostic> {
    let caller = caller
        .map(|s| {
            Ok(ScopeKey {
                crate_key: s.crate_key,
                module_path: crate::syntax::ast_adapter::semantic_module_path(&s.module_path)
                    .ok_or_else(|| error("invalid glob caller scope"))?,
                owner: None,
            })
        })
        .transpose()?;
    let mut bytes = 0usize;
    let mut ids = BTreeMap::new();
    let mut nodes = Vec::new();
    intern(
        (path.clone(), caller, class),
        &mut ids,
        &mut nodes,
        &mut bytes,
    )?;
    let mut next = 0;
    while next < nodes.len() {
        let (path, caller, class) = nodes[next].key.clone();
        let mut own = State::default();
        reads.name(namespace, &path)?;
        if let Some(raw) = graph.bindings.get(&path) {
            own.pending.extend(raw.pending.iter().copied());
        }
        let canonical = match graph
            .module_paths
            .resolve_observed(&path, caller.as_ref(), reads)?
        {
            Resolution::Known(p) => p,
            Resolution::Absent => {
                if class != NameBindingClass::Root {
                    return Err(Diagnostic::new(
                        "LRR-E-NAME-CONTEXT",
                        "contextual name requires actual module-prefix authority".into(),
                    ));
                }
                nodes[next].own = own;
                next += 1;
                continue;
            }
            Resolution::Denied(p) => {
                own.denied.insert(p);
                nodes[next].own = own;
                next += 1;
                continue;
            }
        };
        let mut strong = Vec::new();
        let mut weak = Vec::new();
        let scope = origin(&canonical);
        reads.name(namespace, &canonical)?;
        if let Some(all) = graph.bindings.get(&canonical) {
            // An unresolved source producer has no proved output name class.
            own.pending.extend(all.pending.iter().copied());
        }
        if class != NameBindingClass::Root
            && own.pending.is_empty()
            && graph.bindings.get(&canonical).is_some_and(|all| {
                all.in_class(class).is_none_or(|selected| {
                    selected.definitions.is_empty() && selected.reexports.is_empty()
                }) && (!all.definitions.is_empty() || !all.reexports.is_empty())
            })
        {
            return Err(Diagnostic::new(
                "LRR-E-NAME-CONTEXT",
                "actual declaration/import name adjustment is required".into(),
            ));
        }
        if let Some(binding) = graph
            .bindings
            .get(&canonical)
            .and_then(|b| b.in_class(class))
        {
            own.pending.extend(binding.pending.iter().copied());
            for definition in &binding.definitions {
                let ranges = binding
                    .definition_access
                    .get(definition)
                    .ok_or_else(|| error("glob definition visibility missing"))?;
                let mut allowed = false;
                for visibility in ranges {
                    if caller
                        .as_ref()
                        .map(|c| visibility.allows(c))
                        .transpose()?
                        .unwrap_or(true)
                    {
                        allowed = true;
                        own.candidate(definition, visibility)?;
                    }
                }
                if !allowed {
                    own.denied.insert(canonical.clone());
                }
            }
            if class != NameBindingClass::Root && !binding.reexports.is_empty() {
                return Err(error(
                    "contextual import requires its actual target-name occurrence",
                ));
            }
            for target in &binding.reexports {
                let ranges = binding
                    .reexport_access
                    .get(target)
                    .ok_or_else(|| error("glob import visibility missing"))?;
                let mut allowed = false;
                for visibility in ranges {
                    if caller
                        .as_ref()
                        .map(|c| visibility.allows(c))
                        .transpose()?
                        .unwrap_or(true)
                    {
                        allowed = true;
                        let id = intern(
                            (target.clone(), Some(scope.clone()), class),
                            &mut ids,
                            &mut nodes,
                            &mut bytes,
                        )?;
                        checked_add(
                            &mut bytes,
                            std::mem::size_of::<Edge>() + visibility_bytes(visibility),
                        )?;
                        strong
                            .try_reserve(1)
                            .map_err(|_| error("glob strong edge allocation failed"))?;
                        strong.push(Edge {
                            target: id,
                            visibility: visibility.clone(),
                        });
                    }
                }
                if !allowed {
                    own.denied.insert(canonical.clone());
                }
            }
        }
        if class != NameBindingClass::Root
            && own.pending.is_empty()
            && own.definitions.is_empty()
            && graph
                .glob_reexports
                .get(&canonical)
                .is_some_and(|targets| !targets.is_empty())
        {
            return Err(Diagnostic::new(
                "LRR-E-NAME-CONTEXT",
                "actual glob binding name adjustment is required".into(),
            ));
        }
        // Existing glob occurrences are root bindings. Opaque import aliases
        // remain explicit producer/occurrence demands until their target proof
        // exists; they must never inherit root edges by written spelling.
        if let Some(targets) = (class == NameBindingClass::Root)
            .then(|| graph.glob_reexports.get(&canonical))
            .flatten()
        {
            for (target, ranges) in targets {
                for visibility in ranges {
                    if caller
                        .as_ref()
                        .map(|c| visibility.allows(c))
                        .transpose()?
                        .unwrap_or(true)
                    {
                        let id = intern(
                            (target.clone(), Some(scope.clone()), class),
                            &mut ids,
                            &mut nodes,
                            &mut bytes,
                        )?;
                        checked_add(
                            &mut bytes,
                            std::mem::size_of::<Edge>() + visibility_bytes(visibility),
                        )?;
                        weak.try_reserve(1)
                            .map_err(|_| error("glob weak edge allocation failed"))?;
                        weak.push(Edge {
                            target: id,
                            visibility: visibility.clone(),
                        });
                    }
                }
            }
        }
        checked_add(&mut bytes, state_bytes(&own)?)?;
        nodes[next].own = own;
        nodes[next].strong = strong;
        nodes[next].weak = weak;
        next += 1;
    }
    // Iterative Kosaraju gives dependency-first SCC evaluation after reversal.
    let mut adjacency = Vec::new();
    adjacency
        .try_reserve(nodes.len())
        .map_err(|_| error("glob adjacency allocation failed"))?;
    for node in &nodes {
        adjacency.push(
            node.strong
                .iter()
                .chain(&node.weak)
                .map(|e| e.target)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>(),
        );
    }
    let mut reverse = vec![Vec::new(); nodes.len()];
    for (i, children) in adjacency.iter().enumerate() {
        for &child in children {
            reverse[child]
                .try_reserve(1)
                .map_err(|_| error("glob reverse edge allocation failed"))?;
            reverse[child].push(i);
        }
    }
    let mut visited = vec![false; nodes.len()];
    let mut finish = Vec::new();
    for root in 0..nodes.len() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut stack = vec![(root, 0usize)];
        while let Some((node, index)) = stack.pop() {
            if index < adjacency[node].len() {
                stack.push((node, index + 1));
                let child = adjacency[node][index];
                if !visited[child] {
                    visited[child] = true;
                    stack
                        .try_reserve(1)
                        .map_err(|_| error("glob SCC stack allocation failed"))?;
                    stack.push((child, 0));
                }
            } else {
                finish.push(node);
            }
        }
    }
    visited.fill(false);
    let mut components = Vec::new();
    for &root in finish.iter().rev() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component.push(node);
            for &parent in &reverse[node] {
                if !visited[parent] {
                    visited[parent] = true;
                    stack
                        .try_reserve(1)
                        .map_err(|_| error("glob SCC reverse stack allocation failed"))?;
                    stack.push(parent);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    let mut states = vec![State::default(); nodes.len()];
    let mut updates = 0usize;
    for component in components.iter().rev() {
        let cyclic = component.len() > 1 || adjacency[component[0]].contains(&component[0]);
        let mut seen = BTreeSet::new();
        let mut seen_bytes = 0usize;
        if cyclic {
            seen.insert(fingerprint(component, &states));
        }
        loop {
            let mut changed = false;
            let mut proposed = Vec::new();
            for &id in component {
                updates = updates
                    .checked_add(1)
                    .filter(|n| *n <= 1_000_000 * 257)
                    .ok_or_else(|| error("glob candidate update limit exceeded"))?;
                state_bytes(&states[id])?;
                let state = evaluate(&nodes[id], &states)?;
                state_bytes(&state)?;
                changed |= state != states[id];
                proposed
                    .try_reserve(1)
                    .map_err(|_| error("glob state allocation failed"))?;
                proposed.push((id, state));
            }
            for (id, state) in proposed {
                states[id] = state;
            }
            if !changed || !cyclic {
                break;
            }
            checked_add(&mut seen_bytes, std::mem::size_of::<Digest>())?;
            if !seen.insert(fingerprint(component, &states)) {
                return Err(error(
                    "cyclic strong/glob candidate selection has no stable result",
                ));
            }
        }
    }
    let state = &states[0];
    if let Some(pending) = state.pending.first() {
        return Ok(PathLookup::NeedInput(*pending));
    }
    if !state.denied.is_empty() {
        return Ok(PathLookup::Inaccessible(
            state.denied.iter().cloned().collect(),
        ));
    }
    let mut definitions: BTreeSet<_> = state
        .definitions
        .iter()
        .map(|c| c.definition.clone())
        .collect();
    Ok(match definitions.len() {
        0 => PathLookup::Absent,
        1 => PathLookup::Defined(definitions.pop_first().expect("one candidate")),
        _ => PathLookup::Ambiguous(definitions.into_iter().collect()),
    })
}
