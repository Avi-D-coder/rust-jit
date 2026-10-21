//! Simultaneous symbolic substitution; no runtime layout or goal effects.
use super::term::{
    Binder, BoundKind, ConstTerm, FunctionSignature, GenericArg, ParamKey, PlaceholderKey,
    ProjectionKey, RegionTerm, TraitRef, TyTerm,
};
use crate::core::diagnostic::Diagnostic;
use std::collections::BTreeMap;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERIC-BINDING", message.into())
}
fn index_overflow() -> Diagnostic {
    fault("symbolic binder index overflow")
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
}
impl Substitution {
    pub fn insert(&mut self, key: ParamKey, value: GenericArg) -> Result<(), Diagnostic> {
        let entry = (kind(&value), key);
        if self.values.contains_key(&entry) {
            return Err(fault("duplicate symbolic substitution"));
        }
        // Free enclosing binders remain valid substitution inputs.
        if let GenericArg::Type(term) = &value {
            term.validate_opaque_shape()?;
        }
        self.values.insert(entry, value);
        Ok(())
    }
    pub fn apply(&self, term: &TyTerm) -> Result<TyTerm, Diagnostic> {
        transform(term, Some(self), 0, 0, None)
    }
}
/// Shift only variables free relative to cutoff; nested binders raise cutoff.
pub fn shift(term: &TyTerm, amount: u32, cutoff: u32) -> Result<TyTerm, Diagnostic> {
    transform(term, None, amount, cutoff, None)
}
fn shifted(depth: u32, amount: u32, cutoff: u32) -> Result<u32, Diagnostic> {
    if depth >= cutoff {
        depth.checked_add(amount).ok_or_else(index_overflow)
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
#[derive(Clone, Copy)]
struct TransformContext<'a> {
    subst: Option<&'a Substitution>,
    lift: u32,
    cutoff: u32,
    inst: Option<&'a Instantiation<'a>>,
}
enum TransformTask<'a> {
    Visit(&'a TyTerm, TransformContext<'a>),
    Assemble(&'a TyTerm, TransformContext<'a>, usize),
}
// Child order is also the reconstruction order for mixed generic arguments.
fn type_children(term: &TyTerm) -> Vec<&TyTerm> {
    fn arguments(args: &[GenericArg]) -> Vec<&TyTerm> {
        args.iter()
            .filter_map(|arg| match arg {
                GenericArg::Type(term) => Some(term),
                _ => None,
            })
            .collect::<Vec<_>>()
    }
    match term {
        TyTerm::Function(function) => function
            .value
            .parameters
            .iter()
            .chain(std::iter::once(function.value.result.as_ref()))
            .collect(),
        TyTerm::Projection(key) => std::iter::once(&key.trait_ref.self_ty)
            .chain(key.arguments().filter_map(|arg| match arg {
                GenericArg::Type(term) => Some(term),
                _ => None,
            }))
            .collect(),
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => arguments(args),
        TyTerm::Tuple(fields) => fields.iter().collect(),
        TyTerm::Array(term, _) | TyTerm::Slice(term) | TyTerm::Ref(_, _, term) => vec![term],
        _ => Vec::new(),
    }
}
fn transform<'a>(
    term: &'a TyTerm,
    subst: Option<&'a Substitution>,
    lift: u32,
    cutoff: u32,
    inst: Option<&'a Instantiation<'a>>,
) -> Result<TyTerm, Diagnostic> {
    let mut work = vec![TransformTask::Visit(
        term,
        TransformContext {
            subst,
            lift,
            cutoff,
            inst,
        },
    )];
    let mut results = Vec::new();
    while let Some(task) = work.pop() {
        match task {
            TransformTask::Visit(term, context) => {
                if let (TyTerm::Param(key), Some(subst)) = (term, context.subst)
                    && let Some(GenericArg::Type(replacement)) =
                        subst.values.get(&(BoundKind::Type, *key))
                {
                    // Simultaneous substitution: replacement keys do not become
                    // another substitution. Shift free variables by this lift.
                    work.push(TransformTask::Visit(
                        replacement,
                        TransformContext {
                            subst: None,
                            cutoff: 0,
                            inst: None,
                            ..context
                        },
                    ));
                    continue;
                }
                if let TyTerm::Bound { depth, index } = term {
                    let value = if let Some(inst) = context.inst {
                        match inst.bound(BoundKind::Type, *depth, *index, context.cutoff)? {
                            BoundReplacement::Bound(depth, index) => TyTerm::Bound { depth, index },
                            BoundReplacement::Placeholder(key) => TyTerm::Placeholder(key),
                        }
                    } else if context.subst.is_none() {
                        TyTerm::Bound {
                            depth: shifted(*depth, context.lift, context.cutoff)?,
                            index: *index,
                        }
                    } else {
                        term.clone()
                    };
                    results.push(value);
                    continue;
                }
                if let TyTerm::Opaque(key, args) = term {
                    key.validate_arguments(args)?;
                }
                let mut inside = context;
                if matches!(term, TyTerm::Function(_)) {
                    if context.subst.is_some() {
                        inside.lift = context.lift.checked_add(1).ok_or_else(index_overflow)?;
                    } else {
                        inside.cutoff = context.cutoff.checked_add(1).ok_or_else(index_overflow)?;
                    }
                }
                let children = type_children(term);
                work.push(TransformTask::Assemble(term, context, children.len()));
                work.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| TransformTask::Visit(child, inside)),
                );
            }
            TransformTask::Assemble(term, context, count) => {
                let mut children = results.split_off(results.len() - count).into_iter();
                let mut next = || children.next().expect("scheduled symbolic child");
                let value = match term {
                    TyTerm::Function(function) => TyTerm::Function(Box::new(Binder {
                        variables: function.variables.clone(),
                        value: FunctionSignature {
                            parameters: (0..function.value.parameters.len())
                                .map(|_| next())
                                .collect(),
                            result: Box::new(next()),
                            source_header: function.value.source_header.clone(),
                        },
                    })),
                    TyTerm::Tuple(fields) => {
                        TyTerm::Tuple((0..fields.len()).map(|_| next()).collect())
                    }
                    TyTerm::Projection(key) => TyTerm::Projection(Box::new(ProjectionKey {
                        trait_ref: TraitRef {
                            trait_key: key.trait_ref.trait_key,
                            self_ty: next(),
                            args: transformed_arguments(&key.trait_ref.args, context, &mut next)?,
                        },
                        item: key.item,
                        item_args: transformed_arguments(&key.item_args, context, &mut next)?,
                    })),
                    TyTerm::Opaque(key, args) => TyTerm::Opaque(
                        key.clone(),
                        transformed_arguments(args, context, &mut next)?,
                    ),
                    TyTerm::Nominal(identity, args) => {
                        TyTerm::Nominal(*identity, transformed_arguments(args, context, &mut next)?)
                    }
                    TyTerm::Array(_, c) => TyTerm::Array(
                        Box::new(next()),
                        constant(c, context.subst, context.lift, context.cutoff, context.inst)?,
                    ),
                    TyTerm::Slice(_) => TyTerm::Slice(Box::new(next())),
                    TyTerm::Ref(r, mutable, _) => TyTerm::Ref(
                        region(r, context.subst, context.lift, context.cutoff, context.inst)?,
                        *mutable,
                        Box::new(next()),
                    ),
                    // Only childless variants reach this clone.
                    _ => term.clone(),
                };
                debug_assert!(children.next().is_none());
                results.push(value);
            }
        }
    }
    debug_assert_eq!(results.len(), 1);
    Ok(results.pop().expect("symbolic transform root"))
}
fn transformed_arguments(
    args: &[GenericArg],
    context: TransformContext<'_>,
    next: &mut impl FnMut() -> TyTerm,
) -> Result<Vec<GenericArg>, Diagnostic> {
    args.iter()
        .map(|arg| {
            Ok(match arg {
                GenericArg::Type(_) => GenericArg::Type(next()),
                GenericArg::Region(r) => GenericArg::Region(region(
                    r,
                    context.subst,
                    context.lift,
                    context.cutoff,
                    context.inst,
                )?),
                GenericArg::Const(c) => GenericArg::Const(constant(
                    c,
                    context.subst,
                    context.lift,
                    context.cutoff,
                    context.inst,
                )?),
            })
        })
        .collect()
}
fn bound(
    scopes: &[&[BoundKind]],
    depth: u32,
    index: u32,
    kind: BoundKind,
) -> Result<(), Diagnostic> {
    let distance = usize::try_from(depth)
        .map_err(|_| index_overflow())?
        .checked_add(1)
        .ok_or_else(index_overflow)?;
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
    closed(term, &mut Vec::new())
}
fn closed<'a>(term: &'a TyTerm, scopes: &mut Vec<&'a [BoundKind]>) -> Result<(), Diagnostic> {
    enum Task<'a> {
        Visit(&'a TyTerm),
        LeaveBinder,
    }
    let mut work = vec![Task::Visit(term)];
    while let Some(task) = work.pop() {
        let Task::Visit(term) = task else {
            scopes.pop();
            continue;
        };
        let check_region = |r: &RegionTerm| {
            if let RegionTerm::Bound { depth, index } = r {
                bound(scopes, *depth, *index, BoundKind::Region)
            } else {
                Ok(())
            }
        };
        let check_constant = |c: &ConstTerm| {
            if let ConstTerm::Bound { depth, index } = c {
                bound(scopes, *depth, *index, BoundKind::Const)
            } else {
                Ok(())
            }
        };
        let check_arguments = |args: &[GenericArg]| -> Result<(), Diagnostic> {
            for arg in args {
                match arg {
                    GenericArg::Region(r) => check_region(r)?,
                    GenericArg::Const(c) => check_constant(c)?,
                    GenericArg::Type(_) => {}
                }
            }
            Ok(())
        };
        match term {
            TyTerm::Bound { depth, index } => bound(scopes, *depth, *index, BoundKind::Type)?,
            TyTerm::Projection(key) => {
                check_arguments(&key.trait_ref.args)?;
                check_arguments(&key.item_args)?;
            }
            TyTerm::Opaque(key, args) => {
                key.validate_arguments(args)?;
                check_arguments(args)?;
            }
            TyTerm::Nominal(_, args) => check_arguments(args)?,
            TyTerm::Array(_, c) => check_constant(c)?,
            TyTerm::Ref(r, _, _) => check_region(r)?,
            _ => {}
        }
        if let TyTerm::Function(function) = term {
            scopes.push(&function.variables);
            work.push(Task::LeaveBinder);
        }
        work.extend(type_children(term).into_iter().rev().map(Task::Visit));
    }
    Ok(())
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
        if depth == cutoff {
            if self
                .variables
                .get(usize::try_from(index).map_err(|_| index_overflow())?)
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
                    depth.checked_sub(1).ok_or_else(index_overflow)?
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
    if universe == 0 {
        return Err(fault("binder instantiation requires a non-root universe"));
    }
    validate_binder_type(binder)?;
    let inst = Instantiation {
        variables: &binder.variables,
        transaction,
        universe,
    };
    transform(&binder.value, None, 0, 0, Some(&inst))
}
pub fn validate_binder_type(binder: &Binder<TyTerm>) -> Result<(), Diagnostic> {
    closed(&binder.value, &mut vec![&binder.variables])
}
