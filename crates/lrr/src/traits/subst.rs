//! Bounded simultaneous symbolic substitution; no runtime layout or goal effects.
use super::term::{
    Binder, BoundKind, ConstTerm, FunctionSignature, GenericArg, ParamKey, PlaceholderKey,
    ProjectionKey, RegionTerm, TraitRef, TyTerm,
};
use crate::core::diagnostic::Diagnostic;
use std::collections::BTreeMap;
const MAX_DEPTH: u32 = 256;
const MAX_NODES: usize = 100_000;
const MAX_BYTES: usize = 32 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERIC-BINDING", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "symbolic substitution budget exceeded".into(),
    )
}
#[derive(Default)]
struct Budget {
    nodes: usize,
    bytes: usize,
}
impl Budget {
    fn charge(&mut self, nodes: usize, bytes: usize) -> Result<(), Diagnostic> {
        self.nodes = self.nodes.checked_add(nodes).ok_or_else(limit)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(limit)?;
        if self.nodes > MAX_NODES || self.bytes > MAX_BYTES {
            return Err(limit());
        }
        Ok(())
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, Diagnostic> {
        self.charge(
            0,
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(limit)?,
        )?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| limit())?;
        Ok(values)
    }
}
fn kind(arg: &GenericArg) -> BoundKind {
    match arg {
        GenericArg::Type(_) => BoundKind::Type,
        GenericArg::Region(_) => BoundKind::Region,
        GenericArg::Const(_) => BoundKind::Const,
    }
}
/// Namespace is part of the key even when an owner/ordinal is identical.
#[derive(Default)]
pub struct Substitution {
    values: BTreeMap<(BoundKind, ParamKey), GenericArg>,
    nodes: usize,
    bytes: usize,
}
impl Substitution {
    pub fn insert(&mut self, key: ParamKey, value: GenericArg) -> Result<(), Diagnostic> {
        let entry = (kind(&value), key);
        if self.values.contains_key(&entry) {
            return Err(fault("duplicate symbolic substitution"));
        }
        // Pure typed-shape admission precedes both ownership and aggregate
        // accounting. Free enclosing binders remain valid substitution inputs.
        if let GenericArg::Type(term) = &value {
            term.validate_opaque_shape()?;
        }
        // Existing values are owned and immutable. Recount only the incoming
        // term, preserving the exact aggregate charge used by prior admission.
        let mut nodes = self.nodes.checked_add(1).ok_or_else(limit)?;
        let mut bytes = self
            .bytes
            .checked_add(std::mem::size_of::<((BoundKind, ParamKey), GenericArg)>() + 32)
            .ok_or_else(limit)?;
        if let GenericArg::Type(term) = &value {
            let (n, b) = term.measure().ok_or_else(limit)?;
            nodes = nodes.checked_add(n).ok_or_else(limit)?;
            bytes = bytes.checked_add(b).ok_or_else(limit)?;
        }
        if nodes > MAX_NODES || bytes > MAX_BYTES {
            return Err(limit());
        }
        self.values.insert(entry, value);
        self.nodes = nodes;
        self.bytes = bytes;
        Ok(())
    }
    pub fn apply(&self, term: &TyTerm) -> Result<TyTerm, Diagnostic> {
        term.measure().ok_or_else(limit)?;
        let result = transform(term, Some(self), 0, 0, 0, &mut Budget::default(), None)?;
        result.measure().ok_or_else(limit)?;
        Ok(result)
    }
}
/// Shift only variables free relative to cutoff; nested binders raise cutoff.
pub fn shift(term: &TyTerm, amount: u32, cutoff: u32) -> Result<TyTerm, Diagnostic> {
    if cutoff > MAX_DEPTH {
        return Err(limit());
    }
    term.measure().ok_or_else(limit)?;
    let result = transform(term, None, amount, cutoff, 0, &mut Budget::default(), None)?;
    result.measure().ok_or_else(limit)?;
    Ok(result)
}
fn shifted(depth: u32, amount: u32, cutoff: u32) -> Result<u32, Diagnostic> {
    let depth = if depth >= cutoff {
        depth.checked_add(amount).ok_or_else(limit)?
    } else {
        depth
    };
    if depth > MAX_DEPTH {
        Err(limit())
    } else {
        Ok(depth)
    }
}
fn region(
    term: &RegionTerm,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    inst: Option<&Instantiation<'_>>,
) -> Result<RegionTerm, Diagnostic> {
    if let (RegionTerm::Param(key), Some(subst)) = (term, subst)
        && let Some(GenericArg::Region(replacement)) = subst.values.get(&(BoundKind::Region, *key))
    {
        return region(replacement, None, lift, 0, None);
    }
    Ok(match term {
        RegionTerm::Bound { depth, index } if inst.is_some() => {
            match inst.expect("checked instantiation").bound(
                BoundKind::Region,
                *depth,
                *index,
                cutoff,
            )? {
                BoundReplacement::Bound(depth, index) => RegionTerm::Bound { depth, index },
                BoundReplacement::Placeholder(key) => RegionTerm::Placeholder(key),
            }
        }
        RegionTerm::Bound { depth, index } if subst.is_none() => RegionTerm::Bound {
            depth: shifted(*depth, lift, cutoff)?,
            index: *index,
        },
        _ => term.clone(),
    })
}
fn constant(
    term: &ConstTerm,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    inst: Option<&Instantiation<'_>>,
) -> Result<ConstTerm, Diagnostic> {
    if let (ConstTerm::Param(key), Some(subst)) = (term, subst)
        && let Some(GenericArg::Const(replacement)) = subst.values.get(&(BoundKind::Const, *key))
    {
        return constant(replacement, None, lift, 0, None);
    }
    Ok(match term {
        ConstTerm::Bound { depth, index } if inst.is_some() => {
            match inst.expect("checked instantiation").bound(
                BoundKind::Const,
                *depth,
                *index,
                cutoff,
            )? {
                BoundReplacement::Bound(depth, index) => ConstTerm::Bound { depth, index },
                BoundReplacement::Placeholder(key) => ConstTerm::Placeholder(key),
            }
        }
        ConstTerm::Bound { depth, index } if subst.is_none() => ConstTerm::Bound {
            depth: shifted(*depth, lift, cutoff)?,
            index: *index,
        },
        _ => term.clone(),
    })
}
fn transform(
    term: &TyTerm,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    depth: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    if depth > MAX_DEPTH {
        return Err(limit());
    }
    if let (TyTerm::Param(key), Some(subst)) = (term, subst)
        && let Some(GenericArg::Type(replacement)) = subst.values.get(&(BoundKind::Type, *key))
    {
        // Simultaneous substitution: do not recursively interpret keys inside
        // replacement as a second substitution. Shift before entering a binder.
        return transform(replacement, None, lift, 0, depth, budget, None);
    }
    budget.charge(1, 0)?;
    let next = depth.checked_add(1).ok_or_else(limit)?;
    match term {
        // Atomic unsized pointee; substitution never invents a numeric source,
        // allocation, region or sizedness witness for it.
        TyTerm::Str => Ok(TyTerm::Str),
        TyTerm::Bound { depth, index } if inst.is_some() => {
            match inst.expect("checked instantiation").bound(
                BoundKind::Type,
                *depth,
                *index,
                cutoff,
            )? {
                BoundReplacement::Bound(depth, index) => Ok(TyTerm::Bound { depth, index }),
                BoundReplacement::Placeholder(key) => Ok(TyTerm::Placeholder(key)),
            }
        }
        TyTerm::Bound { depth, index } if subst.is_none() => Ok(TyTerm::Bound {
            depth: shifted(*depth, lift, cutoff)?,
            index: *index,
        }),
        TyTerm::Function(function) => {
            transform_function(function, subst, lift, cutoff, next, budget, inst)
        }
        TyTerm::Tuple(fields) => transform_tuple(fields, subst, lift, cutoff, next, budget, inst),
        TyTerm::Projection(key) => {
            transform_projection(key, subst, lift, cutoff, next, budget, inst)
        }
        TyTerm::Opaque(key, args) => {
            transform_opaque((key, args), subst, lift, cutoff, next, budget, inst)
        }
        TyTerm::Nominal(identity, args) => {
            transform_nominal((*identity, args), subst, lift, cutoff, next, budget, inst)
        }
        TyTerm::Array(t, c) => {
            transform_array((t.as_ref(), c), subst, lift, cutoff, next, budget, inst)
        }
        TyTerm::Slice(t) => transform_slice(t.as_ref(), subst, lift, cutoff, next, budget, inst),
        TyTerm::Ref(r, mutable, t) => transform_reference(
            (r, *mutable, t.as_ref()),
            subst,
            lift,
            cutoff,
            next,
            budget,
            inst,
        ),
        _ => Ok(term.clone()),
    }
}
// Keep recursive dispatch results direct and large branch storage separate.
// All budget charges and source/binder transitions are unchanged.
fn transform_function(
    function: &Binder<FunctionSignature>,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    budget.charge(
        function.variables.len(),
        std::mem::size_of::<Binder<FunctionSignature>>(),
    )?;
    let mut variables = budget.vector(function.variables.len())?;
    variables.extend_from_slice(&function.variables);
    let mut parameters = budget.vector(function.value.parameters.len())?;
    let (inside_lift, inside_cutoff) = if subst.is_some() {
        (lift.checked_add(1).ok_or_else(limit)?, cutoff)
    } else {
        (lift, cutoff.checked_add(1).ok_or_else(limit)?)
    };
    for parameter in &function.value.parameters {
        parameters.push(transform(
            parameter,
            subst,
            inside_lift,
            inside_cutoff,
            next,
            budget,
            inst,
        )?);
    }
    budget.charge(0, std::mem::size_of::<TyTerm>())?;
    let result = Box::new(transform(
        &function.value.result,
        subst,
        inside_lift,
        inside_cutoff,
        next,
        budget,
        inst,
    )?);
    Ok(TyTerm::Function(Box::new(Binder {
        variables,
        value: FunctionSignature { parameters, result },
    })))
}

fn transform_tuple(
    fields: &[TyTerm],
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    let mut values = budget.vector(fields.len())?;
    for value in fields {
        values.push(transform(value, subst, lift, cutoff, next, budget, inst)?);
    }
    Ok(TyTerm::Tuple(values))
}

fn transform_projection(
    key: &ProjectionKey,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    budget.charge(0, std::mem::size_of::<ProjectionKey>())?;
    let self_ty = transform(
        &key.trait_ref.self_ty,
        subst,
        lift,
        cutoff,
        next,
        budget,
        inst,
    )?;
    let args = arguments(&key.trait_ref.args, subst, lift, cutoff, next, budget, inst)?;
    let item_args = arguments(&key.item_args, subst, lift, cutoff, next, budget, inst)?;
    Ok(TyTerm::Projection(Box::new(ProjectionKey {
        trait_ref: TraitRef {
            trait_key: key.trait_ref.trait_key,
            self_ty,
            args,
        },
        item: key.item,
        item_args,
    })))
}

fn transform_opaque(
    source: (&super::term::OpaqueKey, &[GenericArg]),
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    let (key, args) = source;
    key.validate_arguments(args)?;
    budget.charge(key.captures().len(), key.heap_bytes().ok_or_else(limit)?)?;
    Ok(TyTerm::Opaque(
        key.clone(),
        arguments(args, subst, lift, cutoff, next, budget, inst)?,
    ))
}

fn transform_array(
    source: (&TyTerm, &ConstTerm),
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    let (t, c) = source;
    budget.charge(1, std::mem::size_of::<TyTerm>())?;
    Ok(TyTerm::Array(
        Box::new(transform(t, subst, lift, cutoff, next, budget, inst)?),
        constant(c, subst, lift, cutoff, inst)?,
    ))
}

fn transform_slice(
    t: &TyTerm,
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    budget.charge(0, std::mem::size_of::<TyTerm>())?;
    Ok(TyTerm::Slice(Box::new(transform(
        t, subst, lift, cutoff, next, budget, inst,
    )?)))
}

fn transform_reference(
    source: (&RegionTerm, bool, &TyTerm),
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    let (r, mutable, t) = source;
    budget.charge(1, std::mem::size_of::<TyTerm>())?;
    Ok(TyTerm::Ref(
        region(r, subst, lift, cutoff, inst)?,
        mutable,
        Box::new(transform(t, subst, lift, cutoff, next, budget, inst)?),
    ))
}

fn transform_nominal(
    source: (crate::core::digest::Digest, &[GenericArg]),
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    next: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<TyTerm, Diagnostic> {
    Ok(TyTerm::Nominal(
        source.0,
        arguments(source.1, subst, lift, cutoff, next, budget, inst)?,
    ))
}

fn arguments(
    args: &[GenericArg],
    subst: Option<&Substitution>,
    lift: u32,
    cutoff: u32,
    depth: u32,
    budget: &mut Budget,
    inst: Option<&Instantiation<'_>>,
) -> Result<Vec<GenericArg>, Diagnostic> {
    let mut values = budget.vector(args.len())?;
    for argument in args {
        values.push(match argument {
            GenericArg::Type(t) => {
                GenericArg::Type(transform(t, subst, lift, cutoff, depth, budget, inst)?)
            }
            GenericArg::Region(r) => {
                budget.charge(1, 0)?;
                GenericArg::Region(region(r, subst, lift, cutoff, inst)?)
            }
            GenericArg::Const(c) => {
                budget.charge(1, 0)?;
                GenericArg::Const(constant(c, subst, lift, cutoff, inst)?)
            }
        });
    }
    Ok(values)
}
fn bound(
    scopes: &[&[BoundKind]],
    depth: u32,
    index: u32,
    kind: BoundKind,
) -> Result<(), Diagnostic> {
    let distance = usize::try_from(depth)
        .map_err(|_| limit())?
        .checked_add(1)
        .ok_or_else(limit)?;
    let scope = scopes
        .len()
        .checked_sub(distance)
        .and_then(|s| scopes.get(s));
    if scope.and_then(|s| s.get(usize::try_from(index).ok()?)) != Some(&kind) {
        return Err(fault("bound variable escapes its typed binder"));
    }
    Ok(())
}
/// Closed-term admission, separate from structural equality or region solving.
pub fn validate_closed(term: &TyTerm) -> Result<(), Diagnostic> {
    term.measure().ok_or_else(limit)?;
    closed(term, &mut Vec::new(), 0)
}
fn closed<'a>(
    term: &'a TyTerm,
    scopes: &mut Vec<&'a [BoundKind]>,
    depth: u32,
) -> Result<(), Diagnostic> {
    if depth > MAX_DEPTH {
        return Err(limit());
    }
    let region = |r: &RegionTerm, scopes: &[&[BoundKind]]| {
        if let RegionTerm::Bound { depth, index } = r {
            bound(scopes, *depth, *index, BoundKind::Region)
        } else {
            Ok(())
        }
    };
    let constant = |c: &ConstTerm, scopes: &[&[BoundKind]]| {
        if let ConstTerm::Bound { depth, index } = c {
            bound(scopes, *depth, *index, BoundKind::Const)
        } else {
            Ok(())
        }
    };
    if let TyTerm::Opaque(key, args) = term {
        key.validate_arguments(args)?;
    }
    match term {
        TyTerm::Bound { depth, index } => bound(scopes, *depth, *index, BoundKind::Type),
        TyTerm::Function(function) => {
            scopes.push(&function.variables);
            for term in &function.value.parameters {
                closed(term, scopes, depth + 1)?;
            }
            closed(&function.value.result, scopes, depth + 1)?;
            scopes.pop();
            Ok(())
        }
        TyTerm::Tuple(fields) => {
            for term in fields {
                closed(term, scopes, depth + 1)?;
            }
            Ok(())
        }
        TyTerm::Projection(key) => {
            closed(&key.trait_ref.self_ty, scopes, depth + 1)?;
            for arg in key.arguments() {
                match arg {
                    GenericArg::Type(t) => closed(t, scopes, depth + 1)?,
                    GenericArg::Region(r) => region(r, scopes)?,
                    GenericArg::Const(c) => constant(c, scopes)?,
                }
            }
            Ok(())
        }
        TyTerm::Nominal(_, arguments) | TyTerm::Opaque(_, arguments) => {
            for arg in arguments {
                match arg {
                    GenericArg::Type(t) => closed(t, scopes, depth + 1)?,
                    GenericArg::Region(r) => region(r, scopes)?,
                    GenericArg::Const(c) => constant(c, scopes)?,
                }
            }
            Ok(())
        }
        TyTerm::Array(t, c) => {
            constant(c, scopes)?;
            closed(t, scopes, depth + 1)
        }
        TyTerm::Slice(t) => closed(t, scopes, depth + 1),
        TyTerm::Ref(r, _, t) => {
            region(r, scopes)?;
            closed(t, scopes, depth + 1)
        }
        _ => Ok(()),
    }
}

struct Instantiation<'a> {
    variables: &'a [BoundKind],
    transaction: crate::core::digest::Digest,
    universe: u32,
}
enum BoundReplacement {
    Bound(u32, u32),
    Placeholder(PlaceholderKey),
}
impl Instantiation<'_> {
    fn bound(
        &self,
        kind: BoundKind,
        depth: u32,
        index: u32,
        cutoff: u32,
    ) -> Result<BoundReplacement, Diagnostic> {
        if depth > MAX_DEPTH {
            return Err(limit());
        }
        if depth == cutoff {
            if self
                .variables
                .get(usize::try_from(index).map_err(|_| limit())?)
                != Some(&kind)
            {
                return Err(fault(
                    "placeholder substitution has mismatched binder namespace",
                ));
            }
            Ok(BoundReplacement::Placeholder(PlaceholderKey {
                transaction: self.transaction,
                universe: self.universe,
                index,
            }))
        } else {
            Ok(BoundReplacement::Bound(
                if depth > cutoff {
                    depth.checked_sub(1).ok_or_else(limit)?
                } else {
                    depth
                },
                index,
            ))
        }
    }
}
/// The caller allocates a fresh higher universe in the same demand transaction.
/// This creates terms only; it does not publish a witness or execute a body.
pub fn instantiate_binder(
    binder: &Binder<TyTerm>,
    transaction: crate::core::digest::Digest,
    universe: u32,
) -> Result<TyTerm, Diagnostic> {
    if universe == 0 || universe > MAX_DEPTH || binder.variables.len() > MAX_NODES {
        return Err(limit());
    }
    validate_binder_type(binder)?;
    let inst = Instantiation {
        variables: &binder.variables,
        transaction,
        universe,
    };
    let term = transform(
        &binder.value,
        None,
        0,
        0,
        0,
        &mut Budget::default(),
        Some(&inst),
    )?;
    term.measure().ok_or_else(limit)?;
    Ok(term)
}
pub fn validate_binder_type(binder: &Binder<TyTerm>) -> Result<(), Diagnostic> {
    let (nodes, bytes) = binder.value.measure().ok_or_else(limit)?;
    // Pure metadata preflight: the outer binder owns its value inline and its
    // actual vector capacity. The value measurement covers only nested heaps.
    // Use the same aggregate limits as a binder embedded in a function term.
    Budget::default().charge(
        nodes
            .checked_add(binder.variables.len())
            .ok_or_else(limit)?,
        bytes
            .checked_add(std::mem::size_of::<Binder<TyTerm>>())
            .and_then(|bytes| {
                bytes.checked_add(
                    binder
                        .variables
                        .capacity()
                        .checked_mul(std::mem::size_of::<BoundKind>())?,
                )
            })
            .ok_or_else(limit)?,
    )?;
    closed(&binder.value, &mut vec![&binder.variables], 0)
}
