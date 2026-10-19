//! Pure reference receiver-form probing, shared by source constraints and calls.
//! Selected functions remain ordinary members; no guest operation runs here.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::member_index::{BuiltinSelf, MemberIndex, MethodAlternatives},
    source::changes::PreparedSourceView,
    syntax::signatures::FunctionSignature,
    traits::term::TyTerm,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Form {
    Value,
    Shared,
    Mutable,
    Explicit,
}
pub(crate) struct ReferenceMethod {
    pub(crate) alternatives: MethodAlternatives,
    pub(crate) depth: usize,
    pub(crate) self_type: Result<Digest, BuiltinSelf>,
    pub(crate) reference_self: bool,
}
fn bucket(term: &TyTerm) -> Option<Result<Digest, BuiltinSelf>> {
    Some(match term {
        TyTerm::Ref(_, mutable, _) => Err(BuiltinSelf::Reference(*mutable)),
        TyTerm::Nominal(id, _) => Ok(*id),
        TyTerm::Str => Err(BuiltinSelf::Str),
        TyTerm::Slice(_) => Err(BuiltinSelf::Slice),
        TyTerm::Array(..) => Err(BuiltinSelf::Array),
        TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit) => Err(BuiltinSelf::Tuple(0)),
        TyTerm::Primitive(p) => Err(BuiltinSelf::Primitive(*p)),
        TyTerm::Tuple(fields) => Err(BuiltinSelf::Tuple(u32::try_from(fields.len()).ok()?)),
        _ => return None,
    })
}
struct Candidate {
    form: Option<Form>,
    signature: std::sync::Arc<QueryValue>,
    self_type: Option<TyTerm>,
}
impl Candidate {
    fn signature(&self) -> &FunctionSignature {
        let QueryValue::FunctionSignature(signature) = self.signature.as_ref() else {
            unreachable!("private checked candidate signature")
        };
        signature
    }
}
fn candidate(
    db: &mut Database,
    graph: &super::names::NameNamespaces,
    sources: &[&PreparedSourceView],
    function: Digest,
) -> QueryPoll<Candidate> {
    let answer = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(function)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::FunctionSignature(signature) = answer.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-CALL-CONTEXT",
            "reference probe signature outcome differs".into(),
        )));
    };
    let form = signature
        .parameters
        .first()
        .filter(|p| p.receiver)
        .map(|p| receiver_form(&p.ty));
    if form.is_none() || form == Some(Form::Explicit) {
        return QueryPoll::Ready(Ok(Candidate {
            form,
            signature: std::sync::Arc::clone(&answer.value),
            self_type: None,
        }));
    }
    let self_type = match super::source_types::resolve_written_type(
        db,
        graph,
        sources,
        signature,
        &[crate::syntax::ast_adapter::TokenRecipe {
            kind: 0,
            text: "Self".into(),
        }],
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    QueryPoll::Ready(Ok(Candidate {
        form,
        signature: std::sync::Arc::clone(&answer.value),
        self_type,
    }))
}
/// The normal `Self` receiver forms from the actual declared type recipe.
/// Arbitrary explicit receiver types remain with their existing source service.
pub(crate) fn receiver_form(tokens: &[crate::syntax::ast_adapter::TokenRecipe]) -> Form {
    let mut tokens = tokens;
    let form = if tokens.first().is_some_and(|t| t.text == "&") {
        tokens = &tokens[1..];
        if tokens.first().is_some_and(|t| t.text.starts_with('\'')) {
            tokens = &tokens[1..];
        }
        if tokens.first().is_some_and(|t| t.text == "mut") {
            tokens = &tokens[1..];
            Form::Mutable
        } else {
            Form::Shared
        }
    } else {
        Form::Value
    };
    if matches!(tokens, [token] if token.text == "Self") {
        form
    } else {
        Form::Explicit
    }
}
/// Candidate receiver forms at every actual reference step, before continuing
/// with the existing nominal/overloaded/unsizing probe. Inherent methods win
/// within a receiver form, after their declared form has actually matched.
pub(crate) fn probe(
    db: &mut Database,
    members: &MemberIndex,
    graph: &super::names::NameNamespaces,
    sources: &[&PreparedSourceView],
    receiver: &TyTerm,
    name: &str,
) -> QueryPoll<Option<ReferenceMethod>> {
    let mut current = receiver;
    let mut depth = 0;
    let mut signatures = BTreeMap::new();
    while let TyTerm::Ref(_, mutable, inner) = current {
        let Some(own) = bucket(current) else {
            return QueryPoll::Ready(Ok(None));
        };
        let pointee = bucket(inner);
        let own_candidates = members.receiver_functions(own, pointee, name);
        let inner_candidates = pointee.map(|head| {
            let referent = if let TyTerm::Ref(_, _, target) = inner.as_ref() {
                bucket(target)
            } else {
                None
            };
            members.receiver_functions(head, referent, name)
        });
        // (self-type depth, expected receiver form, inherent functions, trait functions).
        for receiver_form in [Form::Value, Form::Shared, Form::Mutable] {
            let mut lanes = vec![(depth, own, true, receiver_form, &own_candidates, current)];
            if receiver_form == Form::Value {
                if let (Some(head), Some(candidates)) = (pointee, &inner_candidates) {
                    lanes.push((
                        depth + 1,
                        head,
                        matches!(inner.as_ref(), TyTerm::Ref(..)),
                        if *mutable {
                            Form::Mutable
                        } else {
                            Form::Shared
                        },
                        candidates,
                        inner.as_ref(),
                    ));
                }
            }
            let mut inherent = Vec::new();
            let mut traits = Vec::new();
            for (selected_depth, head, reference_self, expected, candidates, actual_self) in lanes {
                for (functions, out) in
                    [(&candidates.0, &mut inherent), (&candidates.1, &mut traits)]
                {
                    for function in functions {
                        if !signatures.contains_key(function) {
                            let prepared = match candidate(db, graph, sources, *function) {
                                QueryPoll::Ready(Ok(candidate)) => candidate,
                                QueryPoll::Ready(Err(error)) => {
                                    return QueryPoll::Ready(Err(error));
                                }
                                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                            };
                            signatures.insert(*function, prepared);
                        }
                        let prepared = &signatures[function];
                        if prepared.form == Some(Form::Explicit) {
                            return QueryPoll::Ready(Ok(None));
                        }
                        if prepared.form != Some(expected) {
                            continue;
                        }
                        let Some(pattern) = &prepared.self_type else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        let mut substitution =
                            match super::signature_subst::SignatureSubstitution::new(
                                prepared.signature(),
                                *function,
                            ) {
                                Ok(substitution) => substitution,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            };
                        match substitution.constrain_equation(
                            &super::signature_subst::erase_regions(pattern),
                            &super::signature_subst::erase_regions(actual_self),
                        ) {
                            Ok(()) => {}
                            Err(error) if error.code == "LRR-E-GENERIC-BINDING" => continue,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        }
                        out.push((*function, selected_depth, head, reference_self));
                    }
                }
            }
            let is_inherent = !inherent.is_empty();
            let selected = if is_inherent { inherent } else { traits };
            if let Some((_, selected_depth, self_type, reference_self)) = selected.first().copied()
            {
                let mut functions = Vec::new();
                for (function, ..) in selected {
                    if !functions.contains(&function) {
                        functions.push(function);
                    }
                }
                let same_trait = members.same_trait_of(&functions);
                return QueryPoll::Ready(Ok(Some(ReferenceMethod {
                    alternatives: MethodAlternatives {
                        functions,
                        same_trait,
                        inherent: is_inherent,
                        unsize: false,
                    },
                    depth: selected_depth,
                    self_type,
                    reference_self,
                })));
            }
        }
        depth += 1;
        current = inner;
    }
    QueryPoll::Ready(Ok(None))
}
