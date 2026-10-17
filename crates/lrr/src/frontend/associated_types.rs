//! Shorthand associated-type projections (`T::Name`, `Self::Name`), normalized
//! once their self type is a reached concrete type: the unique trait impl of
//! that type providing `Name` (of the projection's trait, when known) selects
//! its written alias target, resolved in the impl's module scope.
use super::{
    member_index::{BuiltinSelf, MemberIndex},
    names::NameNamespaces,
};
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll},
    source::changes::PreparedSourceView,
    syntax::signatures::FunctionSignature,
    traits::term::{GenericArg, TyTerm},
};

/// Whether `term` contains a projection.
pub(crate) fn contains_projection(term: &TyTerm) -> bool {
    match term {
        TyTerm::Projection(_) => true,
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            contains_projection(inner)
        }
        TyTerm::Tuple(members) => members.iter().any(contains_projection),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(ty) => contains_projection(ty),
            _ => false,
        }),
        _ => false,
    }
}

/// Normalize every projection in `term` whose self type is concrete. `None`
/// when some projection is not yet normalizable (its self type is still a
/// parameter, or no unique impl/target is selectable).
pub(crate) fn normalize(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    members: &MemberIndex,
    term: &TyTerm,
) -> QueryPoll<Option<TyTerm>> {
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(Some(value))) => value,
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    QueryPoll::Ready(Ok(Some(match term {
        TyTerm::Projection(key) => {
            let self_ty = ready!(normalize(db, graph, sources, members, &key.trait_ref.self_ty));
            let selected: Result<Digest, BuiltinSelf> = match &self_ty {
                TyTerm::Nominal(nominal, _) => Ok(*nominal),
                TyTerm::Primitive(primitive) => Err(BuiltinSelf::Primitive(*primitive)),
                TyTerm::Str => Err(BuiltinSelf::Str),
                TyTerm::Slice(_) => Err(BuiltinSelf::Slice),
                TyTerm::Array(..) => Err(BuiltinSelf::Array),
                // A reference self type (`&Vec<T>`, `&[T]`): the reference
                // impls are indexed together, the referent selects.
                TyTerm::Ref(_, mutable, _) => Err(BuiltinSelf::Reference(*mutable)),
                _ => return QueryPoll::Ready(Ok(None)),
            };
            let pointee: Option<Result<Digest, BuiltinSelf>> = match &self_ty {
                TyTerm::Ref(_, _, inner) => match inner.as_ref() {
                    TyTerm::Nominal(nominal, _) => Some(Ok(*nominal)),
                    TyTerm::Primitive(primitive) => Some(Err(BuiltinSelf::Primitive(*primitive))),
                    TyTerm::Str => Some(Err(BuiltinSelf::Str)),
                    TyTerm::Slice(_) => Some(Err(BuiltinSelf::Slice)),
                    TyTerm::Array(..) => Some(Err(BuiltinSelf::Array)),
                    _ => None,
                },
                _ => None,
            };
            let trait_key = Some(key.trait_ref.trait_key).filter(|key| key.0 != [0; 32]);
            let Some(name) = members.associated_type_name(key.item) else {
                return QueryPoll::Ready(Ok(None));
            };
            let (alias, row) = match members.associated_type_of(selected, pointee, trait_key, name) {
                Ok(Some(found)) => found,
                Ok(None) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let Some(target) = alias_target(&row.header.tokens) else {
                return QueryPoll::Ready(Ok(None));
            };
            // A generic impl (`impl<A: Step> Iterator for Range<A>`): the
            // alias target is written in the impl's generic scope. Its
            // parameters are bound by matching the impl's written self type
            // against the projection's concrete self type.
            let owner = if row.impl_generic_count == 0 {
                None
            } else {
                match impl_owner(sources, &row.scope, row.header.profile.edition) {
                    Some(owner) => Some(owner),
                    None => return QueryPoll::Ready(Ok(None)),
                }
            };
            let signature = FunctionSignature {
                definition: alias,
                scope: row.scope.clone(),
                header: row.header.clone(),
                generics: owner
                    .as_ref()
                    .map(|(generics, _)| generics.clone())
                    .unwrap_or_default(),
                parameters: Vec::new(),
                result: Vec::new(),
                result_symbolic: None,
                predicates: Vec::new(),
            };
            let mut resolved = ready!(super::source_types::resolve_written_type(
                db, graph, sources, &signature, &target
            ));
            if let Some((_, written_self)) = &owner {
                let template = ready!(super::source_types::resolve_written_type(
                    db,
                    graph,
                    sources,
                    &signature,
                    written_self
                ));
                let mut bindings = std::collections::BTreeMap::new();
                if !bind(&template, &self_ty, alias, &mut bindings) {
                    return QueryPoll::Ready(Ok(None));
                }
                resolved = substitute(&resolved, &bindings);
                if mentions(&resolved, alias) {
                    return QueryPoll::Ready(Ok(None));
                }
            }
            ready!(normalize(db, graph, sources, members, &resolved))
        }
        TyTerm::Ref(region, mutable, inner) => TyTerm::Ref(
            region.clone(),
            *mutable,
            Box::new(ready!(normalize(db, graph, sources, members, inner))),
        ),
        TyTerm::Slice(inner) => {
            TyTerm::Slice(Box::new(ready!(normalize(db, graph, sources, members, inner))))
        }
        TyTerm::Array(inner, len) => TyTerm::Array(
            Box::new(ready!(normalize(db, graph, sources, members, inner))),
            len.clone(),
        ),
        TyTerm::Tuple(items) => {
            let mut normalized = Vec::with_capacity(items.len());
            for item in items {
                normalized.push(ready!(normalize(db, graph, sources, members, item)));
            }
            TyTerm::Tuple(normalized)
        }
        TyTerm::Nominal(nominal, args) => {
            let mut normalized = Vec::with_capacity(args.len());
            for arg in args {
                normalized.push(match arg {
                    GenericArg::Type(ty) => {
                        GenericArg::Type(ready!(normalize(db, graph, sources, members, ty)))
                    }
                    other => other.clone(),
                });
            }
            TyTerm::Nominal(*nominal, normalized)
        }
        other => other.clone(),
    })))
}

/// The generic parameters and written self type tokens of the impl owning
/// an associated item in `scope`, from that impl's own declaration header.
fn impl_owner(
    sources: &[&PreparedSourceView],
    scope: &crate::syntax::outline::ScopeKey,
    edition: u16,
) -> Option<(
    Vec<crate::syntax::signatures::DefinitionParameter>,
    Vec<crate::syntax::ast_adapter::TokenRecipe>,
)> {
    use ra_ap_syntax::{
        AstNode,
        ast::{self, HasGenericParams, HasModuleItem},
    };
    let owner = scope.owner?;
    let item = sources.iter().find_map(|source| {
        source.outline().items.iter().find(|item| {
            item.def_key.item_kind == crate::source::manifest::ItemKind::Impl
                && item.def_key.enclosing_impl == Some(owner)
        })
    })?;
    let tokens = &item.header.tokens;
    let parser = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        _ => ra_ap_syntax::Edition::Edition2024,
    };
    let header = crate::syntax::ast_adapter::token_text(tokens);
    let parsed = ast::SourceFile::parse(&format!("{header} {{}}"), parser);
    let Some(ast::Item::Impl(implementation)) = parsed.tree().items().next() else {
        return None;
    };
    let generics =
        crate::syntax::signatures::capture_generics(implementation.generic_param_list()).ok()?;
    // The header text joins its tokens with one space: token `i` starts at
    // the sum of the earlier token lengths plus `i`.
    let range = implementation.self_ty()?.syntax().text_range();
    let mut at = 0usize;
    let mut written = Vec::new();
    for token in tokens {
        let start = u32::try_from(at).ok()?;
        if range.contains(start.into()) {
            written.push(token.clone());
        }
        at = at.checked_add(token.text.len())?.checked_add(1)?;
    }
    (!written.is_empty()).then_some((generics, written))
}

/// Bind the parameters of `owner` in `template` so that it is `actual`.
fn bind(
    template: &TyTerm,
    actual: &TyTerm,
    owner: Digest,
    map: &mut std::collections::BTreeMap<crate::traits::term::ParamKey, TyTerm>,
) -> bool {
    let unit = |term: &TyTerm| {
        matches!(term, TyTerm::Tuple(items) if items.is_empty())
            || matches!(term, TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit))
    };
    match (template, actual) {
        (TyTerm::Param(key), _) if key.definition == owner => match map.get(key) {
            Some(bound) => bound == actual,
            None => {
                map.insert(*key, actual.clone());
                true
            }
        },
        (TyTerm::Ref(_, mutable, inner), TyTerm::Ref(_, other, value)) => {
            mutable == other && bind(inner, value, owner, map)
        }
        (TyTerm::Slice(inner), TyTerm::Slice(value)) => bind(inner, value, owner, map),
        (TyTerm::Array(inner, length), TyTerm::Array(value, other)) => {
            length == other && bind(inner, value, owner, map)
        }
        (TyTerm::Tuple(items), TyTerm::Tuple(values)) => {
            items.len() == values.len()
                && items
                    .iter()
                    .zip(values)
                    .all(|(item, value)| bind(item, value, owner, map))
        }
        (TyTerm::Nominal(nominal, arguments), TyTerm::Nominal(other, values)) => {
            nominal == other
                && arguments.len() == values.len()
                && arguments.iter().zip(values).all(|pair| match pair {
                    (GenericArg::Type(item), GenericArg::Type(value)) => {
                        bind(item, value, owner, map)
                    }
                    (item, value) => item == value,
                })
        }
        _ => template == actual || (unit(template) && unit(actual)),
    }
}

/// Whether a parameter of `owner` still occurs in `term`.
fn mentions(term: &TyTerm, owner: Digest) -> bool {
    match term {
        TyTerm::Param(key) => key.definition == owner,
        TyTerm::Projection(key) => mentions(&key.trait_ref.self_ty, owner),
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            mentions(inner, owner)
        }
        TyTerm::Tuple(items) => items.iter().any(|item| mentions(item, owner)),
        TyTerm::Nominal(_, arguments) => arguments.iter().any(|argument| match argument {
            GenericArg::Type(ty) => mentions(ty, owner),
            _ => false,
        }),
        _ => false,
    }
}

/// The written target of `type Name = Target;` (tokens between `=` and `;`).
fn alias_target(
    tokens: &[crate::syntax::ast_adapter::TokenRecipe],
) -> Option<Vec<crate::syntax::ast_adapter::TokenRecipe>> {
    let equals = tokens.iter().position(|token| token.text == "=")?;
    let end = tokens
        .iter()
        .rposition(|token| token.text == ";")
        .filter(|end| *end > equals)
        .unwrap_or(tokens.len());
    let target = tokens[equals + 1..end].to_vec();
    (!target.is_empty()).then_some(target)
}

/// Replace bound generic parameters (`map`) throughout `term`, including the
/// self types of projections.
pub(crate) fn substitute(
    term: &TyTerm,
    map: &std::collections::BTreeMap<crate::traits::term::ParamKey, TyTerm>,
) -> TyTerm {
    match term {
        TyTerm::Param(key) => map.get(key).cloned().unwrap_or_else(|| term.clone()),
        TyTerm::Projection(key) => {
            let mut key = key.clone();
            key.trait_ref.self_ty = substitute(&key.trait_ref.self_ty, map);
            TyTerm::Projection(key)
        }
        TyTerm::Ref(region, mutable, inner) => {
            TyTerm::Ref(region.clone(), *mutable, Box::new(substitute(inner, map)))
        }
        TyTerm::Slice(inner) => TyTerm::Slice(Box::new(substitute(inner, map))),
        TyTerm::Array(inner, len) => TyTerm::Array(Box::new(substitute(inner, map)), len.clone()),
        TyTerm::Tuple(items) => TyTerm::Tuple(items.iter().map(|item| substitute(item, map)).collect()),
        TyTerm::Nominal(nominal, args) => TyTerm::Nominal(
            *nominal,
            args.iter()
                .map(|arg| match arg {
                    GenericArg::Type(ty) => GenericArg::Type(substitute(ty, map)),
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}
