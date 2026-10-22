//! Trait Type parameters a call can fix before its arguments exist.
//!
//! A trait-provided body is generic in the trait's own parameters
//! (`PartialEq::ne(&self, other: &Rhs)`). Its arguments are hinted by those
//! parameters, so an unsuffixed literal argument (`a.ne(&3)`) has no Type to
//! take until `Rhs` is known. rustc fixes `Rhs` by selecting the trait's impl
//! for `Self`; when every impl for the reached `Self` writes one and the same
//! closed argument (the impl index retains them, `ImplMembers::trait_arguments`)
//! the call binds that argument as it binds an explicit turbofish argument.
use super::*;
use crate::frontend::member_index::BuiltinSelf;

/// An argument of the call written as an unsuffixed numeric literal under
/// `borrows` shared `&`, and the Type its parameter is declared with.
pub(super) struct LiteralArgument {
    pub(super) integer: bool,
    pub(super) borrows: usize,
    pub(super) declared: Option<crate::traits::term::TyTerm>,
}

/// The bindings (callee generic ordinal, Type) of the trait Type
/// parameters of the trait-provided `callee` that every impl of its trait
/// for `self_type` agrees on, beyond those already `bound`.
///
/// `literals` are the call's arguments still written as unsuffixed numeric
/// literals: an impl whose trait arguments make that parameter a Type no such
/// literal can take is not among the candidates (`a.ne(&3)` is
/// `PartialEq<i32>`, not `PartialEq<M>`).
pub(super) fn agreed_trait_bindings(
    members: Option<&Arc<crate::frontend::member_index::MemberIndex>>,
    store: &mut GuestStore,
    callee: &SourceCallable,
    self_type: Result<crate::core::digest::Digest, BuiltinSelf>,
    bound: &[(u32, ArgumentType)],
    literals: &[LiteralArgument],
) -> Result<Vec<(u32, ArgumentType)>, Diagnostic> {
    {
        let signature = callee.signature();
        let Some(layout) = signature.generic_source_layout else {
            return Ok(Vec::new());
        };
        if !layout.implicit_self || layout.inherited == 0 {
            return Ok(Vec::new());
        }
        let Some(members) = members else {
            return Ok(Vec::new());
        };
        let Some(trait_key) = members.trait_of_declaration(signature.definition) else {
            return Ok(Vec::new());
        };
        let name = &callee.definition().declared_name;
        let mut impls = members.trait_method(self_type, trait_key, name);
        if impls.is_empty() {
            return Ok(Vec::new());
        }
        let trait_parameters: Vec<_> = signature
            .generics
            .iter()
            .skip(layout.own as usize)
            .take(layout.inherited as usize)
            .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
            .collect();
        if !literals.is_empty() && impls.len() > 1 {
            let narrowed: Vec<_> = impls
                .iter()
                .filter(|(implementation, _)| {
                    literals.iter().all(|literal| {
                        literal_parameter_admitted(
                            signature,
                            &trait_parameters,
                            implementation,
                            literal,
                        )
                    })
                })
                .cloned()
                .collect();
            if !narrowed.is_empty() {
                impls = narrowed;
            }
        }
        let mut bindings = Vec::new();
        for (position, parameter) in trait_parameters.iter().copied().enumerate() {
            if bound.iter().any(|(ordinal, _)| *ordinal == parameter.ordinal) {
                continue;
            }
            let default_is_self =
                matches!(parameter.default.as_slice(), [token] if token.text == "Self");
            let mut agreed: Option<crate::frontend::member_index::ArgShape> = None;
            let mut unanimous = true;
            for (implementation, _) in &impls {
                let shape = implementation.trait_argument(position, default_is_self);
                match &agreed {
                    None => agreed = Some(shape),
                    Some(previous) if *previous == shape => {}
                    Some(_) => {
                        unanimous = false;
                        break;
                    }
                }
            }
            let Some(term) = agreed.filter(|_| unanimous).and_then(|shape| shape.to_term()) else {
                continue;
            };
            let Some(descriptor) = store.intern_structural_term(&term)? else {
                continue;
            };
            bindings.push((
                parameter.ordinal,
                ArgumentType {
                    source: term,
                    descriptor,
                },
            ));
        }
        Ok(bindings)
    }
}

/// Whether the trait-provided body's parameter `ordinal`, with the impl's
/// trait arguments substituted, can take an unsuffixed numeric literal under
/// `borrows` shared `&`. A parameter still open (it names `Self` or an
/// argument the impl leaves generic) admits it.
fn literal_parameter_admitted(
    signature: &crate::syntax::signatures::FunctionSignature,
    trait_parameters: &[&crate::syntax::signatures::DefinitionParameter],
    implementation: &crate::frontend::member_index::ImplMembers,
    literal: &LiteralArgument,
) -> bool {
    use crate::traits::{
        subst::Substitution,
        term::{GenericArg, ParamKey, TyTerm},
    };
    let (integer, borrows) = (literal.integer, literal.borrows);
    let Some(declared) = &literal.declared else {
        return true;
    };
    let mut substitution = Substitution::default();
    for (position, parameter) in trait_parameters.iter().enumerate() {
        let default_is_self =
            matches!(parameter.default.as_slice(), [token] if token.text == "Self");
        if let Some(term) = implementation
            .trait_argument(position, default_is_self)
            .to_term()
        {
            let key = ParamKey {
                definition: signature.definition,
                ordinal: parameter.ordinal,
            };
            if substitution.insert(key, GenericArg::Type(term)).is_err() {
                return true;
            }
        }
    }
    let Ok(applied) = substitution.apply(declared) else {
        return true;
    };
    fn admits(term: &TyTerm, integer: bool, borrows: usize) -> bool {
        use crate::ir::op::PrimitiveType as P;
        match term {
            TyTerm::Ref(_, false, inner) if borrows > 0 => admits(inner, integer, borrows - 1),
            TyTerm::Primitive(P::F32 | P::F64) if borrows == 0 => !integer,
            TyTerm::Primitive(P::Unit | P::Bool | P::Char) if borrows == 0 => false,
            TyTerm::Primitive(_) if borrows == 0 => integer,
            term if !crate::frontend::call_contracts::closed_term(term) => true,
            _ => false,
        }
    }
    admits(&applied, integer, borrows)
}
