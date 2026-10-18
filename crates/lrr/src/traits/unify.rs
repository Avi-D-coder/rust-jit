//! Local structural equality; no trait solving, runtime tags or implicit defaults.
use super::term::{ConstTerm, GenericArg, InferVarKey, PlaceholderKey, RegionTerm, TyTerm};
use crate::core::{diagnostic::Diagnostic, digest::Digest};
use std::collections::BTreeMap;
// Structural-nesting guard for symbolic type terms (type recursion; rustc bounds
// type recursion by `recursion_limit`, compiler/rustc_interface/src/limits.rs).
const MAX_DEPTH: usize = 256;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERIC-BINDING", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "signature unification budget exceeded".into(),
    )
}
fn children(term: &TyTerm) -> Vec<&TyTerm> {
    match term {
        TyTerm::Tuple(ts) => ts.iter().collect(),
        TyTerm::Nominal(_, args) => args
            .iter()
            .filter_map(|a| {
                if let GenericArg::Type(t) = a {
                    Some(t)
                } else {
                    None
                }
            })
            .collect(),
        TyTerm::Array(t, _) | TyTerm::Slice(t) | TyTerm::Ref(_, _, t) => vec![t],
        // A fn pointer without bound regions (E-R5-008).
        TyTerm::Function(function) if function.variables.is_empty() => function
            .value
            .parameters
            .iter()
            .chain(std::iter::once(function.value.result.as_ref()))
            .collect(),
        _ => vec![],
    }
}
fn validate(term: &TyTerm, transaction: Digest, universe: u32) -> Result<usize, Diagnostic> {
    let (node_count, _) = term.measure().ok_or_else(limit)?;
    let mut stack = vec![(term, 0)];
    let mut count = 0usize;
    while let Some((term, depth)) = stack.pop() {
        count = count.checked_add(1).ok_or_else(limit)?;
        if depth > MAX_DEPTH {
            return Err(limit());
        }
        if let TyTerm::Infer(v) = term
            && v.transaction != transaction
        {
            return Err(fault("inference variable belongs to another transaction"));
        }
        if matches!(
            term,
            TyTerm::Bound { .. } | TyTerm::Projection(_) | TyTerm::Dyn(_) | TyTerm::Opaque(_, _)
        ) || matches!(term, TyTerm::Function(function) if !function.variables.is_empty())
        {
            return Err(fault(
                "symbolic function/bound/projection/dyn/opaque terms require ordered schema, universe or normalization integration",
            ));
        }
        for key in placeholders(term) {
            if key.transaction != transaction || key.universe == 0 || key.universe > universe {
                return Err(fault(
                    "placeholder belongs to a foreign or invisible universe",
                ));
            }
        }
        let bound_region = match term {
            TyTerm::Ref(RegionTerm::Bound { .. }, _, _) => true,
            TyTerm::Nominal(_, args) => args
                .iter()
                .any(|arg| matches!(arg, GenericArg::Region(RegionTerm::Bound { .. }))),
            _ => false,
        };
        let bound_const = match term {
            TyTerm::Array(_, ConstTerm::Bound { .. }) => true,
            TyTerm::Nominal(_, args) => args
                .iter()
                .any(|arg| matches!(arg, GenericArg::Const(ConstTerm::Bound { .. }))),
            _ => false,
        };
        if bound_const {
            return Err(fault("bound constants require ordered universe matching"));
        }
        if bound_region {
            return Err(fault(
                "bound regions require the ordered binder/universe integration",
            ));
        }
        stack.extend(children(term).into_iter().map(|t| (t, depth + 1)));
    }
    Ok(node_count)
}
#[derive(Debug, Clone)]
pub struct Unifier {
    transaction: Digest,
    universe: u32,
    variable_universes: BTreeMap<InferVarKey, u32>,
    bindings: BTreeMap<InferVarKey, TyTerm>,
}
impl Unifier {
    pub fn new(transaction: Digest) -> Self {
        Self {
            transaction,
            universe: 0,
            variable_universes: BTreeMap::new(),
            bindings: BTreeMap::new(),
        }
    }
    pub fn with_universe(transaction: Digest, universe: u32) -> Result<Self, Diagnostic> {
        Ok(Self {
            universe,
            ..Self::new(transaction)
        })
    }
    pub fn fresh_universe(&mut self) -> Result<u32, Diagnostic> {
        let next = self
            .universe
            .checked_add(1)
            .ok_or_else(limit)?;
        self.universe = next;
        Ok(next)
    }
    pub fn register_variable(&mut self, key: InferVarKey, universe: u32) -> Result<(), Diagnostic> {
        if key.transaction != self.transaction || universe > self.universe {
            return Err(fault(
                "inference variable has foreign or invisible universe",
            ));
        }
        if self.variable_universes.contains_key(&key) || self.bindings.contains_key(&key) {
            return Err(fault("inference universe cannot be redeclared"));
        }
        self.variable_universes.insert(key, universe);
        Ok(())
    }
    /// Existing unregistered elaboration variables belong to the root universe.
    pub fn variable_universe(&self, key: InferVarKey) -> u32 {
        self.variable_universes.get(&key).copied().unwrap_or(0)
    }
    pub fn binding(&self, key: InferVarKey) -> Option<&TyTerm> {
        self.bindings.get(&key)
    }
    pub fn unify(&mut self, left: &TyTerm, right: &TyTerm) -> Result<(), Diagnostic> {
        validate(left, self.transaction, self.universe)?;
        validate(right, self.transaction, self.universe)?;
        // Rejected constraints must not publish a prefix of their bindings.
        let mut staged = self.bindings.clone();
        let mut universes = self.variable_universes.clone();
        // Admission fixes the identity of legacy variables at universe zero,
        // including variables nested inside a published substitution. Otherwise
        // register_variable could grant a previously used variable newer scope.
        admit_variables(left, &mut universes)?;
        admit_variables(right, &mut universes)?;
        let mut work = vec![(left.clone(), right.clone())];
        let mut steps = 0usize;
        while let Some((left, right)) = work.pop() {
            charge(&mut steps)?;
            let left = resolve_head(&staged, &left, &mut steps)?.clone();
            let right = resolve_head(&staged, &right, &mut steps)?.clone();
            if left == right {
                continue;
            }
            match (left, right) {
                (TyTerm::Infer(v), t) | (t, TyTerm::Infer(v)) => {
                    if occurs(&staged, v, &t, &mut steps)? {
                        return Err(fault("recursive inference substitution"));
                    }
                    visible_substitution(&staged, &mut universes, v, &t, &mut steps)?;
                    staged.insert(v, t);
                }
                (TyTerm::Tuple(a), TyTerm::Tuple(b)) if a.len() == b.len() => {
                    work.extend(a.into_iter().zip(b))
                }
                // A fn pointer type unifies structurally (E-R5-008).
                (TyTerm::Function(a), TyTerm::Function(b))
                    if a.variables.is_empty()
                        && b.variables.is_empty()
                        && a.value.parameters.len() == b.value.parameters.len() =>
                {
                    let (a, b) = (a.value, b.value);
                    work.extend(a.parameters.into_iter().zip(b.parameters));
                    work.push((*a.result, *b.result));
                }
                // `*mut T` coerces to `*const T`: the two pointer identities
                // unify through their pointees (rustc checked mutability).
                (TyTerm::Nominal(a, x), TyTerm::Nominal(b, y))
                    if (a == b || super::pointer_terms::both_pointer_identities(&a, &b))
                        && x.len() == y.len() =>
                {
                    for (a, b) in x.into_iter().zip(y) {
                        match (a, b) {
                            (GenericArg::Type(a), GenericArg::Type(b)) => work.push((a, b)),
                            // Regions are checked by rustc, never by the
                            // interpreter: two reached regions always agree.
                            (GenericArg::Region(_), GenericArg::Region(_)) => {}
                            (a, b) if a == b => {}
                            _ => {
                                return Err(fault(
                                    "generic argument namespace or identity mismatch",
                                ));
                            }
                        }
                    }
                }
                (TyTerm::Slice(a), TyTerm::Slice(b)) => work.push((*a, *b)),
                (TyTerm::Array(a, x), TyTerm::Array(b, y)) if x == y => work.push((*a, *b)),
                // A const generic length (`[T; N]` of an impl's `const N`)
                // is the reached array's own length; rustc checked it.
                (TyTerm::Array(a, ConstTerm::Param(_)), TyTerm::Array(b, _))
                | (TyTerm::Array(a, _), TyTerm::Array(b, ConstTerm::Param(_))) => {
                    work.push((*a, *b))
                }
                // The pointees unify; the regions are rustc's concern (a
                // reference reached from a descriptor is `'static`-erased,
                // an argument's carries its local region).
                (TyTerm::Ref(_, m, a), TyTerm::Ref(_, n, b)) if m == n => work.push((*a, *b)),
                _ => return Err(fault("reached structural type contracts disagree")),
            }
        }
        for term in staged.values() {
            validate(term, self.transaction, self.universe)?;
        }
        self.bindings = staged;
        self.variable_universes = universes;
        Ok(())
    }
}
fn charge(steps: &mut usize) -> Result<(), Diagnostic> {
    *steps = steps.checked_add(1).ok_or_else(limit)?;
    Ok(())
}
fn resolve_head<'a>(
    bindings: &'a BTreeMap<InferVarKey, TyTerm>,
    mut term: &'a TyTerm,
    steps: &mut usize,
) -> Result<&'a TyTerm, Diagnostic> {
    while let TyTerm::Infer(v) = term {
        charge(steps)?;
        let Some(next) = bindings.get(v) else { break };
        term = next;
    }
    Ok(term)
}
fn occurs(
    bindings: &BTreeMap<InferVarKey, TyTerm>,
    variable: InferVarKey,
    term: &TyTerm,
    steps: &mut usize,
) -> Result<bool, Diagnostic> {
    let mut stack = vec![(term, 0)];
    while let Some((term, depth)) = stack.pop() {
        charge(steps)?;
        if depth > MAX_DEPTH {
            return Err(limit());
        }
        let term = resolve_head(bindings, term, steps)?;
        if matches!(term,TyTerm::Infer(v)if *v==variable) {
            return Ok(true);
        }
        stack.extend(children(term).into_iter().map(|t| (t, depth + 1)));
    }
    Ok(false)
}

// Type children are visited separately. Region/const namespaces retain their
// own tagged placeholders even where runtime layout erases region data.
fn placeholders(term: &TyTerm) -> Vec<PlaceholderKey> {
    match term {
        TyTerm::Placeholder(key) => vec![*key],
        TyTerm::Ref(RegionTerm::Placeholder(key), _, _) => vec![*key],
        TyTerm::Array(_, ConstTerm::Placeholder(key)) => vec![*key],
        TyTerm::Nominal(_, args) => args
            .iter()
            .filter_map(|arg| match arg {
                GenericArg::Region(RegionTerm::Placeholder(key))
                | GenericArg::Const(ConstTerm::Placeholder(key)) => Some(*key),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}
fn visible_substitution(
    bindings: &BTreeMap<InferVarKey, TyTerm>,
    universes: &mut BTreeMap<InferVarKey, u32>,
    variable: InferVarKey,
    term: &TyTerm,
    steps: &mut usize,
) -> Result<(), Diagnostic> {
    let visible = universes.get(&variable).copied().unwrap_or(0);
    let mut work = vec![(term, 0usize)];
    while let Some((term, depth)) = work.pop() {
        charge(steps)?;
        if depth > MAX_DEPTH {
            return Err(limit());
        }
        let term = resolve_head(bindings, term, steps)?;
        for key in placeholders(term) {
            if key.universe > visible {
                return Err(fault("newer placeholder would escape its universe"));
            }
        }
        if let TyTerm::Infer(key) = term
            && universes.get(key).copied().unwrap_or(0) > visible
        {
            // Lower unbound local variables before publication, so a subsequent
            // younger-placeholder binding cannot leak through this alias.
            universes.insert(*key, visible);
        }
        work.extend(children(term).into_iter().map(|t| (t, depth + 1)));
    }
    Ok(())
}

fn admit_variables(
    term: &TyTerm,
    universes: &mut BTreeMap<InferVarKey, u32>,
) -> Result<(), Diagnostic> {
    // The caller validated shape, depth, transaction and input size first.
    let mut work = vec![term];
    while let Some(term) = work.pop() {
        if let TyTerm::Infer(key) = term {
            universes.entry(*key).or_insert(0);
        }
        work.extend(children(term));
    }
    Ok(())
}
