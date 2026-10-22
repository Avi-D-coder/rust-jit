//! Static qualified-path anchors, `<Type as Trait<Args>>::name` and
//! `<Type>::name`: the impl functions the member index offers for the written
//! self type. The trait's written Type arguments narrow the impls (their
//! retained header shapes, `ImplMembers::trait_arguments`); when several impls
//! of one trait still differ only by the trait's arguments (`Mul<i32>` and
//! `Mul<&i32>` for `i32`), the call's evaluated arguments select among them
//! (`select_by_arguments`), exactly as a method call or a `Type::name` path
//! call selects them.
use super::*;
use crate::{
    frontend::member_index::{ArgShape, BuiltinSelf},
    traits::term::TyTerm,
};

/// Several impls of one trait providing an anchored function.
pub(crate) struct AnchoredAlternatives {
    pub(crate) trait_key: Digest,
    /// The trait's declaration of the function.
    pub(crate) declaration: Digest,
    /// The impls' functions (distinct, at least two).
    pub(crate) functions: Vec<Digest>,
    /// The written Self when it is a closed primitive.
    pub(crate) self_term: Option<TyTerm>,
}

/// The member-index self type as a shape.
fn self_type_shape(self_type: Result<Digest, BuiltinSelf>) -> ArgShape {
    match self_type {
        Ok(identity) => ArgShape::Nominal(identity, Vec::new()),
        Err(builtin) => builtin_shape(builtin),
    }
}

fn builtin_shape(builtin: BuiltinSelf) -> ArgShape {
    match builtin {
        BuiltinSelf::Primitive(primitive) => ArgShape::Primitive(primitive),
        BuiltinSelf::Str => ArgShape::Str,
        BuiltinSelf::Tuple(0) => ArgShape::Primitive(crate::ir::op::PrimitiveType::Unit),
        _ => ArgShape::Any,
    }
}

impl<'a> CallResolver<'a> {
    /// The shape a trait argument spelled in a qualified path names (plain
    /// primitive, `Self`, module path or references to them); `Any` for any
    /// other spelling, which then narrows nothing.
    fn written_shape(
        &self,
        db: &mut Database,
        caller: Digest,
        text: &str,
        self_shape: &ArgShape,
    ) -> QueryPoll<ArgShape> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(ArgShape::Any));
        };
        let mut rest = text.trim();
        let mut references = Vec::new();
        while let Some(tail) = rest.strip_prefix('&') {
            let mut tail = tail.trim_start();
            if tail.starts_with('\'') {
                let Some(end) = tail.find(char::is_whitespace) else {
                    return QueryPoll::Ready(Ok(ArgShape::Any));
                };
                tail = tail[end..].trim_start();
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
        let mut shape = if rest == "Self" {
            self_shape.clone()
        } else {
            let (scope, edition) = match self.caller_context(db, caller) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if scope.owner.is_some() {
                return QueryPoll::Ready(Ok(ArgShape::Any));
            }
            let resolved = written_segments(rest, edition).and_then(|(absolute, segments)| {
                self.resolve_written_type(
                    members, caller, &scope, edition, &segments, absolute, true,
                )
            });
            match resolved {
                Ok(Some(found)) => self_type_shape(found),
                Ok(None) | Err(_) => ArgShape::Any,
            }
        };
        for mutable in references.into_iter().rev() {
            shape = ArgShape::Ref(mutable, Box::new(shape));
        }
        QueryPoll::Ready(Ok(shape))
    }

    /// For each Type parameter of the trait (as its declaration of a function
    /// lists them), whether its default is `Self` (`trait Mul<Rhs = Self>`).
    fn trait_defaults_are_self(
        &self,
        db: &mut Database,
        declaration: Digest,
    ) -> QueryPoll<Vec<bool>> {
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(declaration)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait declaration signature variant differs")));
        };
        let Some(layout) = signature.generic_source_layout else {
            return QueryPoll::Ready(Ok(Vec::new()));
        };
        QueryPoll::Ready(Ok(signature
            .generics
            .iter()
            .skip(layout.own as usize)
            .take(layout.inherited as usize)
            .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
            .map(|parameter| {
                matches!(parameter.default.as_slice(), [token] if token.text == "Self")
            })
            .collect()))
    }

    /// The member-index self type of a written anchor Type, and, for a
    /// reference (`&i32`, `&mut Foo`), the self type its referent names (the
    /// impls for `&U` are indexed together, so the referent selects).
    fn written_anchor_self(
        &self,
        db: &mut Database,
        caller: Digest,
        ty: &str,
    ) -> QueryPoll<
        Option<(
            Result<Digest, BuiltinSelf>,
            Option<Result<Digest, BuiltinSelf>>,
        )>,
    > {
        let Some(reference) = ty.strip_prefix('&') else {
            return match self.written_self_type(db, caller, ty) {
                QueryPoll::Ready(Ok(found)) => QueryPoll::Ready(Ok(found.map(|found| (found, None)))),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        };
        let reference = reference.trim_start();
        let (mutable, referent) = match reference.strip_prefix("mut ") {
            Some(referent) => (true, referent.trim_start()),
            None => (false, reference),
        };
        match self.written_self_type(db, caller, referent) {
            QueryPoll::Ready(Ok(Some(pointee))) => QueryPoll::Ready(Ok(Some((
                Err(BuiltinSelf::Reference(mutable)),
                Some(pointee),
            )))),
            QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    /// The functions of an anchored path's selected tier: the named trait's
    /// impls for the written self type (narrowed by the trait's written Type
    /// arguments), else (no trait written) the inherent functions, else the
    /// self type's trait impls. `None` when the path's types or trait do not
    /// resolve.
    pub(super) fn anchored_candidates(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<Option<(Result<Digest, BuiltinSelf>, Vec<Digest>)>> {
        match self.anchored_impls(db, caller, name) {
            QueryPoll::Ready(Ok(found)) => QueryPoll::Ready(Ok(found.map(|(self_type, pairs)| {
                (
                    self_type,
                    pairs.into_iter().map(|(_, function)| function).collect(),
                )
            }))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    /// `anchored_candidates` with the impl each function belongs to (`None`
    /// for an inherent or blanket-resolved function).
    fn anchored_impls(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<
        Option<(
            Result<Digest, BuiltinSelf>,
            Vec<(Option<&'a super::super::member_index::ImplMembers>, Digest)>,
        )>,
    > {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some((ty, tr, explicit, function)) = split_qualified_anchor_arguments(name) else {
            return QueryPoll::Ready(Ok(None));
        };
        if function.contains("::") {
            return QueryPoll::Ready(Ok(None));
        }
        let (self_type, pointee) = match self.written_anchor_self(db, caller, ty) {
            QueryPoll::Ready(Ok(Some(found))) => found,
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
            Some(trait_key) => {
                let mut impls = members.trait_method(self_type, trait_key, function);
                // `impl .. for &U`: the referent selects among the reference
                // impls; an impl naming it wins over one generic in it.
                if impls.len() > 1
                    && let Some(pointee) = pointee
                {
                    let exact: Vec<_> = impls
                        .iter()
                        .filter(|(implementation, _)| implementation.self_pointee == Some(pointee))
                        .cloned()
                        .collect();
                    let generic: Vec<_> = impls
                        .iter()
                        .filter(|(implementation, _)| implementation.self_pointee.is_none())
                        .cloned()
                        .collect();
                    impls = if exact.is_empty() { generic } else { exact };
                }
                if impls.len() > 1
                    && !explicit.is_empty()
                    && let Some(declaration) = members.trait_declaration(trait_key, function)
                {
                    let defaults = match self.trait_defaults_are_self(db, declaration) {
                        QueryPoll::Ready(Ok(defaults)) => defaults,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    };
                    let self_shape = self_type_shape(self_type);
                    let mut wanted = Vec::new();
                    for text in &explicit {
                        wanted.push(match self.written_shape(db, caller, text, &self_shape) {
                            QueryPoll::Ready(Ok(ArgShape::Any)) => None,
                            QueryPoll::Ready(Ok(shape)) => Some(shape),
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        });
                    }
                    let narrowed: Vec<_> = impls
                        .iter()
                        .filter(|(implementation, _)| {
                            implementation.admits_trait_arguments(&wanted, &defaults)
                        })
                        .cloned()
                        .collect();
                    // Arguments naming no impl narrow nothing (looser than rustc).
                    if !narrowed.is_empty() {
                        impls = narrowed;
                    }
                }
                impls
                    .into_iter()
                    .map(|(implementation, function)| (Some(implementation), function))
                    .collect()
            }
            None => {
                let inherent = match self_type {
                    Ok(nominal) => members.inherent(nominal, function),
                    Err(builtin) => members.builtin_inherent(builtin, function),
                };
                if inherent.is_empty() {
                    members.trait_candidates_of(self_type, pointee, function)
                } else {
                    inherent
                }
                .into_iter()
                .map(|function| (None, function))
                .collect()
            }
        };
        QueryPoll::Ready(Ok(Some((self_type, candidates))))
    }

    /// The one impl function of an ambiguous anchored path whose trait
    /// arguments are all its self type (`impl Mul<i32> for i32`, `impl Mul for
    /// i32`: `Rhs = Self`). A function item value (`let f = <i32 as Mul>::mul`)
    /// has no argument to select by, so it takes the trait's defaulted
    /// arguments (looser than rustc, which infers them from the value's use).
    pub(crate) fn anchored_default_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let (_, pairs) = match self.anchored_impls(db, caller, name) {
            QueryPoll::Ready(Ok(Some(found))) => found,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut homogeneous = pairs.iter().filter_map(|(implementation, function)| {
            let implementation = (*implementation)?;
            let arguments = implementation.trait_arguments.as_ref()?;
            arguments
                .iter()
                .all(|argument| *argument == implementation.self_shape)
                .then_some(*function)
        });
        let first = homogeneous.next();
        QueryPoll::Ready(Ok(first.filter(|first| homogeneous.all(|other| other == *first))))
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
        let Some((_, _, _, function)) = split_qualified_anchor_arguments(name) else {
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
            Err(BuiltinSelf::Primitive(primitive)) => Some(TyTerm::Primitive(primitive)),
            _ => None,
        };
        QueryPoll::Ready(Ok(Some(AnchoredAlternatives {
            trait_key,
            declaration,
            functions,
            self_term,
        })))
    }
}
