//! Structural sizedness constraints of source ADTs, from their declared tail
//! field Types (pinned `rustc_middle::ty::adt` `adt_sizedness_constraint` and
//! `sizedness_constraint_for_ty`). Source terms only: no solver, no goal and
//! no name list. A struct is `Sized` exactly when its last field Type is; an
//! enum or union always is; a library ADT (`Box`, `PhantomData`,
//! `ManuallyDrop`) follows its own declaration like any other.
use crate::{
    core::digest::Digest,
    traits::{
        pointer_terms::as_raw_pointer,
        subst::Substitution,
        term::{GenericArg, ParamKey, TyTerm},
    },
};
use rustc_type_ir::solve::SizedTraitKind;

/// What the bundle retains of one nominal declaration.
pub(super) enum AdtShape {
    /// An enum or union: its sizedness constraint is always `None`.
    Fieldless,
    /// A struct: its last declared field Type (identity-instantiated, over
    /// its own parameters) and its Type parameters in written order.
    Struct {
        tail: Option<TyTerm>,
        parameters: Vec<ParamKey>,
    },
}

/// The constraint of `term`, or the nominal declaration whose shape is not
/// retained. Cycles are infinitely represented types, whose constraint is
/// `None` as in rustc.
pub(super) fn constraint_for_ty(
    kind: SizedTraitKind,
    term: &TyTerm,
    shape: &dyn Fn(Digest) -> Option<AdtShape>,
    active: &mut Vec<Digest>,
) -> Result<Option<TyTerm>, Digest> {
    match term {
        TyTerm::Primitive(_) | TyTerm::Ref(..) | TyTerm::Array(..) | TyTerm::Function(_) | TyTerm::Never => {
            Ok(None)
        }
        // `str`, slices and trait objects are never `Sized`; only `dyn` and
        // `str`/slices differ from an extern type in being `MetaSized`.
        TyTerm::Str | TyTerm::Slice(_) | TyTerm::Dyn(_) => Ok(match kind {
            SizedTraitKind::Sized => Some(term.clone()),
            SizedTraitKind::MetaSized => None,
        }),
        TyTerm::Tuple(members) => match members.last() {
            Some(last) => constraint_for_ty(kind, last, shape, active),
            None => Ok(None),
        },
        TyTerm::Nominal(identity, arguments) => {
            if as_raw_pointer(term).is_some() {
                return Ok(None);
            }
            let Some(declared) = shape(*identity) else {
                return Err(*identity);
            };
            let AdtShape::Struct { tail, parameters } = declared else {
                return Ok(None);
            };
            if active.contains(identity) {
                return Ok(None);
            }
            let Some(tail) = tail else {
                return Ok(None);
            };
            active.push(*identity);
            let own = constraint_for_ty(kind, &tail, shape, active);
            active.pop();
            let Some(own) = own? else {
                return Ok(None);
            };
            let mut substitution = Substitution::default();
            let types = arguments.iter().filter_map(|argument| match argument {
                GenericArg::Type(ty) => Some(ty),
                _ => None,
            });
            for (parameter, argument) in parameters.iter().zip(types) {
                if substitution.insert(*parameter, GenericArg::Type(argument.clone())).is_err() {
                    return Err(*identity);
                }
            }
            match substitution.apply(&own) {
                Ok(instantiated) => constraint_for_ty(kind, &instantiated, shape, active),
                Err(_) => Err(*identity),
            }
        }
        // Rigid or still-open Types are the constraint themselves.
        TyTerm::Param(_)
        | TyTerm::Projection(_)
        | TyTerm::Opaque(..)
        | TyTerm::Placeholder(_)
        | TyTerm::Bound { .. }
        | TyTerm::Infer(_) => Ok(Some(term.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ir::op::PrimitiveType, traits::term::GenericArg};
    use std::collections::BTreeMap;

    fn id(name: &str) -> Digest {
        Digest::of_bytes(name.as_bytes())
    }

    /// A struct `name<T>(tail)` over one Type parameter.
    fn generic_struct(name: &str, tail: TyTerm) -> AdtShape {
        AdtShape::Struct {
            tail: Some(tail),
            parameters: vec![ParamKey { definition: id(name), ordinal: 0 }],
        }
    }

    fn constraint(
        kind: SizedTraitKind,
        term: &TyTerm,
        shapes: &BTreeMap<Digest, AdtShape>,
    ) -> Result<Option<TyTerm>, Digest> {
        let shape = |identity: Digest| {
            shapes.get(&identity).map(|shape| match shape {
                AdtShape::Fieldless => AdtShape::Fieldless,
                AdtShape::Struct { tail, parameters } => AdtShape::Struct {
                    tail: tail.clone(),
                    parameters: parameters.clone(),
                },
            })
        };
        constraint_for_ty(kind, term, &shape, &mut Vec::new())
    }

    /// The constraint follows the declared tail by structure, never a name:
    /// a struct is `Sized` iff its last field Type is; an enum always is; a
    /// recursive (infinitely represented) declaration constrains nothing; a
    /// declaration the bundle did not retain is reported, not guessed.
    #[test]
    fn sizedness_constraint_follows_the_declared_tail() {
        let slice = TyTerm::Slice(Box::new(TyTerm::Primitive(PrimitiveType::U8)));
        let generic = |name: &str| TyTerm::Param(ParamKey { definition: id(name), ordinal: 0 });
        let nominal = |name: &str, argument: TyTerm| TyTerm::Nominal(id(name), vec![GenericArg::Type(argument)]);
        let mut shapes = BTreeMap::new();
        // `struct Wrap<T: ?Sized>(T)`, `struct Outer(Wrap<[u8]>)`, `enum E`,
        // `struct Pair(i32, Wrap<T>)`-like tails over a parameter.
        shapes.insert(id("Wrap"), generic_struct("Wrap", generic("Wrap")));
        shapes.insert(id("Outer"), AdtShape::Struct { tail: Some(nominal("Wrap", slice.clone())), parameters: Vec::new() });
        shapes.insert(id("E"), AdtShape::Fieldless);
        shapes.insert(id("Unit"), AdtShape::Struct { tail: None, parameters: Vec::new() });
        shapes.insert(id("Rec"), AdtShape::Struct { tail: Some(TyTerm::Nominal(id("Rec"), Vec::new())), parameters: Vec::new() });

        use SizedTraitKind::{MetaSized, Sized};
        // A parameter-tailed struct is as sized as its argument.
        assert_eq!(constraint(Sized, &nominal("Wrap", TyTerm::Primitive(PrimitiveType::I32)), &shapes), Ok(None));
        assert_eq!(constraint(Sized, &nominal("Wrap", slice.clone()), &shapes), Ok(Some(slice.clone())));
        // A slice tail is `MetaSized`, so the weaker bound constrains nothing.
        assert_eq!(constraint(MetaSized, &nominal("Wrap", slice.clone()), &shapes), Ok(None));
        // The tail's tail: `Outer(Wrap<[u8]>)` is unsized through `Wrap`.
        assert_eq!(constraint(Sized, &TyTerm::Nominal(id("Outer"), Vec::new()), &shapes), Ok(Some(slice.clone())));
        assert_eq!(constraint(Sized, &TyTerm::Nominal(id("E"), Vec::new()), &shapes), Ok(None));
        assert_eq!(constraint(Sized, &TyTerm::Nominal(id("Unit"), Vec::new()), &shapes), Ok(None));
        assert_eq!(constraint(Sized, &TyTerm::Nominal(id("Rec"), Vec::new()), &shapes), Ok(None));
        // An open argument stays the constraint (the solver decides it later).
        let open = generic("Caller");
        assert_eq!(constraint(Sized, &nominal("Wrap", open.clone()), &shapes), Ok(Some(open)));
        // Unretained declarations are named, never assumed sized.
        assert_eq!(constraint(Sized, &TyTerm::Nominal(id("Absent"), Vec::new()), &shapes), Err(id("Absent")));
    }
}
