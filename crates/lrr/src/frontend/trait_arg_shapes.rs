//! The written Type arguments of an impl header's trait (`impl Mul<&i32> for
//! i32`), retained by the impl index as closed *shapes*. A shape states only
//! what the header names without the Type service: primitives, references,
//! declarations and structure around them; an impl parameter, an associated
//! projection or any other spelling is `Any` and admits every Type.
//!
//! Shapes serve impl selection that must run before the call's arguments
//! exist or when only the trait's explicit arguments name the impl: a qualified
//! `<u32 as Make<u16>>::make()` call, a trait-provided body whose `Rhs` the
//! unique impl fixes (`a.ne(&3)`), a literal argument that only some impls
//! accept. A shape never rejects what it cannot name, so it narrows a
//! candidate set and never selects a wrong impl by guessing.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    ir::op::PrimitiveType,
    traits::term::{GenericArg, RegionTerm, TyTerm},
};
use ra_ap_syntax::ast::{self, HasGenericArgs};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ArgShape {
    /// An impl parameter or a spelling the shape does not name.
    Any,
    Primitive(PrimitiveType),
    Str,
    Ref(bool, Box<ArgShape>),
    /// A declaration and the shapes of its written Type arguments (empty when
    /// the declaration is named without arguments).
    Nominal(Digest, Vec<ArgShape>),
    Tuple(Vec<ArgShape>),
    Slice(Box<ArgShape>),
    Array(Box<ArgShape>),
}

impl ArgShape {
    /// Whether both shapes may name one Type: `false` only for a definite
    /// mismatch. A declaration written with arguments on one side only is the
    /// same declaration.
    pub fn admits(&self, other: &ArgShape) -> bool {
        use ArgShape::*;
        match (self, other) {
            (Any, _) | (_, Any) => true,
            (Primitive(left), Primitive(right)) => left == right,
            (Str, Str) => true,
            (Ref(left_mut, left), Ref(right_mut, right)) => {
                left_mut == right_mut && left.admits(right)
            }
            (Nominal(left, left_args), Nominal(right, right_args)) => {
                left == right
                    && (left_args.is_empty()
                        || right_args.is_empty()
                        || (left_args.len() == right_args.len()
                            && left_args.iter().zip(right_args).all(|(l, r)| l.admits(r))))
            }
            (Tuple(left), Tuple(right)) => {
                left.len() == right.len() && left.iter().zip(right).all(|(l, r)| l.admits(r))
            }
            (Slice(left), Slice(right)) | (Array(left), Array(right)) => left.admits(right),
            _ => false,
        }
    }

    /// The closed source Type this shape names, when it names exactly one.
    pub fn to_term(&self) -> Option<TyTerm> {
        Some(match self {
            Self::Any => return None,
            Self::Primitive(primitive) => TyTerm::Primitive(*primitive),
            Self::Str => TyTerm::Str,
            Self::Ref(mutable, inner) => {
                TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(inner.to_term()?))
            }
            Self::Nominal(identity, arguments) => TyTerm::Nominal(
                *identity,
                arguments
                    .iter()
                    .map(|argument| argument.to_term().map(GenericArg::Type))
                    .collect::<Option<Vec<_>>>()?,
            ),
            Self::Tuple(members) => TyTerm::Tuple(
                members
                    .iter()
                    .map(ArgShape::to_term)
                    .collect::<Option<Vec<_>>>()?,
            ),
            Self::Slice(inner) => TyTerm::Slice(Box::new(inner.to_term()?)),
            Self::Array(_) => return None,
        })
    }

    /// The shape of a source Type term (an evaluated argument's reached Type).
    pub fn of_term(term: &TyTerm) -> ArgShape {
        match term {
            TyTerm::Primitive(primitive) => Self::Primitive(*primitive),
            TyTerm::Str => Self::Str,
            TyTerm::Ref(_, mutable, inner) => Self::Ref(*mutable, Box::new(Self::of_term(inner))),
            TyTerm::Nominal(identity, arguments) => Self::Nominal(
                *identity,
                arguments
                    .iter()
                    .filter_map(|argument| match argument {
                        GenericArg::Type(inner) => Some(Self::of_term(inner)),
                        _ => None,
                    })
                    .collect(),
            ),
            TyTerm::Tuple(members) if members.is_empty() => Self::Primitive(PrimitiveType::Unit),
            TyTerm::Tuple(members) => Self::Tuple(members.iter().map(Self::of_term).collect()),
            TyTerm::Slice(inner) => Self::Slice(Box::new(Self::of_term(inner))),
            TyTerm::Array(inner, _) => Self::Array(Box::new(Self::of_term(inner))),
            _ => Self::Any,
        }
    }
}

/// The shape of a written impl-header Type. `generics` are the impl's own type
/// and const parameter names; `Self` is `self_shape` (the impl's self type).
pub(crate) fn shape_of(
    ty: &ast::Type,
    generics: &BTreeSet<String>,
    self_shape: Option<&ArgShape>,
    resolve: &dyn Fn(Option<ast::Path>) -> Result<Option<Digest>, Diagnostic>,
) -> Result<ArgShape, Diagnostic> {
    let again = |ty: &ast::Type| shape_of(ty, generics, self_shape, resolve);
    Ok(match ty {
        ast::Type::ParenType(inner) => match inner.ty() {
            Some(inner) => again(&inner)?,
            None => ArgShape::Any,
        },
        ast::Type::RefType(reference) => match reference.ty() {
            Some(inner) => ArgShape::Ref(reference.mut_token().is_some(), Box::new(again(&inner)?)),
            None => ArgShape::Any,
        },
        ast::Type::SliceType(slice) => match slice.ty() {
            Some(inner) => ArgShape::Slice(Box::new(again(&inner)?)),
            None => ArgShape::Any,
        },
        ast::Type::ArrayType(array) => match array.ty() {
            Some(inner) => ArgShape::Array(Box::new(again(&inner)?)),
            None => ArgShape::Any,
        },
        ast::Type::TupleType(tuple) => {
            let mut members = Vec::new();
            for field in tuple.fields() {
                members.push(again(&field)?);
            }
            if members.is_empty() {
                ArgShape::Primitive(PrimitiveType::Unit)
            } else {
                ArgShape::Tuple(members)
            }
        }
        ast::Type::PathType(path) => {
            let Some(path) = path.path() else {
                return Ok(ArgShape::Any);
            };
            if path.qualifier().is_none()
                && let Some(segment) = path.segment()
                && let Some(name) = segment.name_ref()
            {
                let name = name.text().to_string();
                let name = name.as_str();
                if generics.contains(name) {
                    return Ok(ArgShape::Any);
                }
                if name == "Self" {
                    return Ok(self_shape.cloned().unwrap_or(ArgShape::Any));
                }
                if segment.generic_arg_list().is_none()
                    && let Some(builtin) = crate::frontend::member_index::BuiltinSelf::named(name)
                {
                    // A primitive name unshadowed by a declaration of that name.
                    if resolve(Some(path.clone()))?.is_none() {
                        return Ok(match builtin {
                            crate::frontend::member_index::BuiltinSelf::Primitive(primitive) => {
                                ArgShape::Primitive(primitive)
                            }
                            crate::frontend::member_index::BuiltinSelf::Str => ArgShape::Str,
                            _ => ArgShape::Any,
                        });
                    }
                }
            }
            let Some(identity) = resolve(Some(path.clone()))? else {
                return Ok(ArgShape::Any);
            };
            let mut arguments = Vec::new();
            if let Some(list) = path.segment().and_then(|segment| segment.generic_arg_list()) {
                for argument in list.generic_args() {
                    match argument {
                        ast::GenericArg::LifetimeArg(_) => {}
                        ast::GenericArg::TypeArg(argument) => match argument.ty() {
                            Some(inner) => arguments.push(again(&inner)?),
                            None => arguments.push(ArgShape::Any),
                        },
                        _ => arguments.push(ArgShape::Any),
                    }
                }
            }
            ArgShape::Nominal(identity, arguments)
        }
        _ => ArgShape::Any,
    })
}

/// The Type arguments written on an impl's trait path (`Mul<&i32>` -> `[&i32]`);
/// lifetimes are skipped, a const or associated-type argument is `Any`.
pub(crate) fn trait_argument_shapes(
    implementation: &ast::Impl,
    generics: &BTreeSet<String>,
    self_shape: &ArgShape,
    resolve: &dyn Fn(Option<ast::Path>) -> Result<Option<Digest>, Diagnostic>,
) -> Result<Vec<ArgShape>, Diagnostic> {
    let Some(ast::Type::PathType(trait_path)) = implementation.trait_() else {
        return Ok(Vec::new());
    };
    let Some(list) = trait_path
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.generic_arg_list())
    else {
        return Ok(Vec::new());
    };
    let mut shapes = Vec::new();
    for argument in list.generic_args() {
        match argument {
            ast::GenericArg::LifetimeArg(_) | ast::GenericArg::AssocTypeArg(_) => {}
            ast::GenericArg::TypeArg(argument) => shapes.push(match argument.ty() {
                Some(ty) => shape_of(&ty, generics, Some(self_shape), resolve)?,
                None => ArgShape::Any,
            }),
            _ => shapes.push(ArgShape::Any),
        }
    }
    Ok(shapes)
}
