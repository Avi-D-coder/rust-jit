//! Static qualified-path anchors, `<Type as Trait<Args>>::name` and
//! `<Type>::name`: the impl functions the member index offers for the written
//! self type, and, when several impls of one trait differ only by the trait's
//! arguments (`Mul<i32>` and `Mul<&i32>` for `i32`), the alternatives the
//! call's evaluated arguments select among (`select_by_arguments`), exactly as
//! a method call or a `Type::name` path call selects them.
use super::*;
use crate::traits::term::TyTerm;

/// Several impls of one trait providing an anchored function.
pub(crate) struct AnchoredAlternatives {
    /// The written `Trait<Args>`'s explicit Type arguments, as spelled.
    pub(crate) explicit_arguments: Vec<String>,
    pub(crate) trait_key: Digest,
    /// The trait's declaration of the function.
    pub(crate) declaration: Digest,
    /// The impls' functions (distinct, at least two).
    pub(crate) functions: Vec<Digest>,
    /// The written Self when it is a closed primitive.
    pub(crate) self_term: Option<TyTerm>,
}

impl<'a> CallResolver<'a> {
    /// The functions of an anchored path's selected tier: the named trait's
    /// impls for the written self type, else (no trait written) the inherent
    /// functions, else the self type's trait impls. `None` when the path's
    /// types or trait do not resolve.
    pub(super) fn anchored_candidates(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<Option<(Result<Digest, super::super::member_index::BuiltinSelf>, Vec<Digest>)>> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some((ty, tr, function)) = split_qualified_anchor(name) else {
            return QueryPoll::Ready(Ok(None));
        };
        if function.contains("::") {
            return QueryPoll::Ready(Ok(None));
        }
        let self_type = match self.written_self_type(db, caller, ty) {
            QueryPoll::Ready(Ok(Some(self_type))) => self_type,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let trait_key = match tr {
            Some(tr) => match self.written_trait(db, caller, tr) {
                QueryPoll::Ready(Ok(Some(trait_key))) => Some(trait_key),
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
            None => None,
        };
        let candidates = match trait_key {
            Some(trait_key) => members
                .trait_method(self_type, trait_key, function)
                .into_iter()
                .map(|(_, function)| function)
                .collect::<Vec<_>>(),
            None => {
                let inherent = match self_type {
                    Ok(nominal) => members.inherent(nominal, function),
                    Err(builtin) => members.builtin_inherent(builtin, function),
                };
                if inherent.is_empty() {
                    members.trait_candidates(self_type, function)
                } else {
                    inherent
                }
            }
        };
        QueryPoll::Ready(Ok(Some((self_type, candidates))))
    }

    /// Several impls of one trait providing the anchored function, differing
    /// only by the trait's arguments; `None` for any other shape (a single
    /// candidate, inherent functions, impls of different traits).
    pub(crate) fn anchored_alternatives(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<Option<AnchoredAlternatives>> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some((_, _, explicit, function)) = split_qualified_anchor_arguments(name) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (self_type, candidates) = match self.anchored_candidates(db, caller, name) {
            QueryPoll::Ready(Ok(Some(found))) => found,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut functions = Vec::new();
        for candidate in candidates {
            if !functions.contains(&candidate) {
                functions.push(candidate);
            }
        }
        if functions.len() < 2 {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(trait_key) = members.same_trait_of(&functions) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(declaration) = members.trait_declaration(trait_key, function) else {
            return QueryPoll::Ready(Ok(None));
        };
        let self_term = match self_type {
            Err(super::super::member_index::BuiltinSelf::Primitive(primitive)) => {
                Some(TyTerm::Primitive(primitive))
            }
            _ => None,
        };
        QueryPoll::Ready(Ok(Some(AnchoredAlternatives {
            explicit_arguments: explicit.into_iter().map(str::to_owned).collect(),
            trait_key,
            declaration,
            functions,
            self_term,
        })))
    }
}

/// A written trait argument as a closed source Type, when it is a plain
/// primitive name, `Self` (a closed primitive Self) or a reference to one;
/// `None` for every other spelling, which then constrains nothing.
pub(crate) fn explicit_argument_term(text: &str, self_term: Option<&TyTerm>) -> Option<TyTerm> {
    use crate::traits::term::RegionTerm;
    let mut rest = text.trim();
    let mut references = Vec::new();
    while let Some(tail) = rest.strip_prefix('&') {
        let mut tail = tail.trim_start();
        if tail.starts_with('\'') {
            tail = tail[tail.find(char::is_whitespace)?..].trim_start();
        }
        let mutable = match tail.strip_prefix("mut ") {
            Some(inner) => {
                tail = inner.trim_start();
                true
            }
            None => false,
        };
        references.push(mutable);
        rest = tail;
    }
    let mut term = match rest {
        "Self" => self_term?.clone(),
        name => match primitive_self(name)? {
            super::super::member_index::BuiltinSelf::Primitive(primitive) => {
                TyTerm::Primitive(primitive)
            }
            super::super::member_index::BuiltinSelf::Str => TyTerm::Str,
            _ => return None,
        },
    };
    for mutable in references.into_iter().rev() {
        term = TyTerm::Ref(RegionTerm::Static, mutable, Box::new(term));
    }
    Some(term)
}
