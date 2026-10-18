//! Bound metadata through the one declaration collector and ordinary Type walk.
//! No source evaluation, static checking or predicate fulfillment occurs here.
use super::{
    names::NameNamespaces,
    source_types::{self, ActualBoundSubject, WrittenTypeDeclaration},
};
use crate::{
    db::{memo::Database, poll::QueryPoll},
    source::changes::PreparedSourceView,
    syntax::{
        ast_adapter::range,
        signatures::{DefinitionParameter, GenericKind},
    },
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{ParamKey, RegionTerm, TraitRef, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};

fn region(
    definition: crate::core::digest::Digest,
    generics: &[DefinitionParameter],
    lifetime: &ast::Lifetime,
) -> Option<RegionTerm> {
    let name =
        crate::syntax::signatures::canonical_lifetime(&lifetime.syntax().text().to_string())?;
    if name == "'static" {
        return Some(RegionTerm::Static);
    }
    if name == "'_" {
        let source = range(lifetime.syntax().text_range());
        return Some(RegionTerm::Local {
            definition,
            start: source.start,
            end: source.end,
        });
    }
    let parameter = generics
        .iter()
        .find(|parameter| parameter.kind == GenericKind::Lifetime && parameter.name == name)?;
    Some(RegionTerm::Param(ParamKey {
        definition,
        ordinal: parameter.ordinal,
    }))
}

pub(crate) fn prepare(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    definition: crate::core::digest::Digest,
    generics: &[DefinitionParameter],
    owner: &ra_ap_syntax::SyntaxNode,
    core: Option<crate::core::digest::Digest>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let actual = source_types::actual_declaration_bound_syntax(declaration, owner);
    let sized = if actual.implicit_sized.is_empty() {
        None
    } else {
        ready!(super::language_items::sized(
            db,
            sources,
            declaration.profile(),
            core
        ))
    };
    let mut unresolved_relaxation = false;
    let mut recipes = Vec::new();
    recipes.extend(
        actual
            .implicit_sized
            .into_iter()
            .map(SourcePredicateRecipe::ImplicitSizedDefault),
    );
    for row in actual.bounds {
        let source = range(row.bound.syntax().text_range());
        macro_rules! pending {
            ($why:ident) => {{
                unresolved_relaxation |= row.bound.question_mark_token().is_some();
                recipes.push(SourcePredicateRecipe::Explicit {
                    source,
                    result: PredicateResult::Pending(PredicatePending::$why),
                });
                continue;
            }};
        }
        if row.where_binder.is_some() || row.bound.for_binder().is_some() {
            pending!(HigherRanked);
        }
        if row.bound.const_token().is_some()
            || row.bound.tilde_token().is_some()
            || row.bound.async_token().is_some()
            || row.bound.use_token().is_some()
        {
            pending!(QualifiedBound);
        }
        let relaxation_parameter = if row.bound.question_mark_token().is_some() {
            match &row.subject {
                ActualBoundSubject::Type(TyTerm::Param(key)) => Some(*key),
                ActualBoundSubject::TypeSyntax(ty) => match declaration.direct_type_parameter(ty) {
                    Ok(key) => key,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
                _ => None,
            }
        } else {
            None
        };
        enum Subject {
            Type(TyTerm),
            Region(RegionTerm),
        }
        let subject = match row.subject {
            ActualBoundSubject::Type(term) => Subject::Type(term),
            ActualBoundSubject::TypeSyntax(ty) => {
                if source_types::symbolic_type_pending(&ty).is_some() {
                    pending!(SymbolicType);
                }
                let Some(term) = ready!(source_types::resolve_actual_type(
                    db,
                    graph,
                    sources,
                    declaration,
                    &ty
                )) else {
                    pending!(Subject);
                };
                Subject::Type(term)
            }
            ActualBoundSubject::Region(term) => Subject::Region(term),
            ActualBoundSubject::RegionSyntax(lifetime) => {
                let Some(term) = region(definition, generics, &lifetime) else {
                    pending!(LifetimeName);
                };
                Subject::Region(term)
            }
            ActualBoundSubject::Missing => {
                pending!(Subject);
            }
        };
        if let Some(lifetime) = row.bound.lifetime() {
            let Some(target) = region(definition, generics, &lifetime) else {
                pending!(LifetimeName);
            };
            let result = match subject {
                Subject::Region(longer) => PredicateResult::RegionOutlives {
                    longer,
                    shorter: target,
                },
                Subject::Type(ty) => {
                    if super::associated_types::contains_projection(&ty) {
                        pending!(AssociatedIdentity);
                    }
                    PredicateResult::TypeOutlives { ty, region: target }
                }
            };
            recipes.push(SourcePredicateRecipe::Explicit { source, result });
            continue;
        }
        let Subject::Type(subject) = subject else {
            pending!(Subject);
        };
        if super::associated_types::contains_projection(&subject) {
            pending!(AssociatedIdentity);
        }
        let Some(ast::Type::PathType(ty)) = row.bound.ty() else {
            pending!(TraitName);
        };
        if source_types::symbolic_type_pending(&ast::Type::PathType(ty.clone())).is_some() {
            pending!(SymbolicType);
        }
        let Some(path) = ty.path() else {
            pending!(TraitName);
        };
        if path
            .syntax()
            .descendants()
            .any(|node| ast::AssocTypeArg::can_cast(node.kind()))
        {
            pending!(AssociatedIdentity);
        }
        if path
            .segment()
            .is_some_and(|segment| segment.parenthesized_arg_list().is_some())
        {
            pending!(CallableArguments);
        }
        let Some((trait_key, args)) = ready!(source_types::projection_trait(
            db,
            graph,
            sources,
            declaration,
            &path
        )) else {
            pending!(TraitName);
        };
        if args.iter().any(|arg| matches!(arg, crate::traits::term::GenericArg::Type(ty) if super::associated_types::contains_projection(ty))) { pending!(AssociatedIdentity); }
        let trait_ref = TraitRef {
            trait_key,
            self_ty: subject,
            args,
        };
        recipes.push(SourcePredicateRecipe::Explicit {
            source,
            result: if row.bound.question_mark_token().is_some() {
                PredicateResult::Relaxed {
                    trait_ref,
                    parameter: relaxation_parameter,
                }
            } else {
                PredicateResult::Implemented(trait_ref)
            },
        });
    }
    if let Some(sized) = sized.filter(|_| !unresolved_relaxation) {
        let relaxed: std::collections::BTreeSet<_> = recipes
            .iter()
            .filter_map(|recipe| match recipe {
                SourcePredicateRecipe::Explicit {
                    result:
                        PredicateResult::Relaxed {
                            trait_ref,
                            parameter,
                        },
                    ..
                } if trait_ref.trait_key == sized && trait_ref.args.is_empty() => *parameter,
                _ => None,
            })
            .collect();
        recipes = recipes
            .into_iter()
            .filter_map(|recipe| match recipe {
                SourcePredicateRecipe::ImplicitSizedDefault(key) if relaxed.contains(&key) => None,
                SourcePredicateRecipe::ImplicitSizedDefault(key) => {
                    Some(SourcePredicateRecipe::ResolvedSizedDefault(TraitRef {
                        trait_key: sized,
                        self_ty: TyTerm::Param(key),
                        args: Vec::new(),
                    }))
                }
                other => Some(other),
            })
            .collect();
    }
    QueryPoll::Ready(Ok(recipes))
}
