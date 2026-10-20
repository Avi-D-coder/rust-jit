//! Bound metadata through the one declaration collector and ordinary Type walk.
//! No source evaluation, static checking or predicate fulfillment occurs here.
use super::{
    names::NameNamespaces,
    source_types::{self, ActualBoundSubject, WrittenTypeDeclaration},
};
use crate::{
    db::{memo::Database, poll::QueryPoll},
    source::changes::PreparedSourceView,
    syntax::ast_adapter::range,
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{RegionTerm, TraitRef, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasTypeBounds},
};

pub(crate) fn prepare(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    owner: &ra_ap_syntax::SyntaxNode,
    core: Option<crate::core::digest::Digest>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    prepare_selected(db, graph, sources, declaration, owner, core, None)
}

/// Own declaration bounds through the same collector and Type walk. Parent
/// requirements are a separate closure, never silently merged into own clauses.
/// Associated callers supply their authenticated child-bound Self from the
/// declaration source plan. Only own written where/generic bounds are selected
/// here; Q78 applies the pinned default-bound policy separately. Output bounds belong to
/// the separate item-bounds query, not this explicit-own recipe transport.
pub(crate) fn prepare_own(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    owner: &ra_ap_syntax::SyntaxNode,
    core: Option<crate::core::digest::Digest>,
    own_parameters: u32,
    self_type: Option<&TyTerm>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    prepare_selected(
        db,
        graph,
        sources,
        declaration,
        owner,
        core,
        Some((own_parameters, self_type)),
    )
}

fn prepare_selected(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    owner: &ra_ap_syntax::SyntaxNode,
    core: Option<crate::core::digest::Digest>,
    own: Option<(u32, Option<&TyTerm>)>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    let missing_self = matches!(own, Some((_, None)));
    let mut actual = source_types::actual_declaration_bound_syntax(declaration, owner);
    if let Some((_own_parameters, self_type)) = own {
        let owner_range = owner.text_range();
        actual
            .bounds
            .retain(|row| owner_range.contains_range(row.bound.syntax().text_range()));
        // Own-source defaults are supplied by the retained Q84 compiler policy.
        // Do not run the legacy simple-Sized filter and discard its result.
        actual.implicit_sized.clear();
        // The collector's trait supertrait subject has no written parameter
        // node. Use only the actual declaration's supplied implicit Self slot.
        if ast::Trait::can_cast(owner.kind()) {
            for row in &mut actual.bounds {
                if matches!(row.subject, ActualBoundSubject::Missing)
                    && row
                        .bound
                        .syntax()
                        .parent()
                        .and_then(|list| list.parent())
                        .as_ref()
                        == Some(owner)
                    && let Some(self_type) = self_type
                {
                    row.subject = ActualBoundSubject::Type(self_type.clone());
                }
            }
        }
    }
    prepare_bound_rows(db, graph, sources, declaration, actual, core, missing_self)
}

/// Actual written associated output bounds with the caller's authenticated
/// projection subject. No generic-parameter/default or parent clauses are added.
pub(crate) fn prepare_output_bounds(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    alias: &ast::TypeAlias,
    subject: &TyTerm,
    core: Option<crate::core::digest::Digest>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    let actual = source_types::ActualDeclarationBounds {
        bounds: alias
            .type_bound_list()
            .into_iter()
            .flat_map(|list| list.bounds())
            .map(|bound| source_types::ActualDeclarationBound {
                subject: ActualBoundSubject::Type(subject.clone()),
                bound,
                where_binder: None,
            })
            .collect(),
        implicit_sized: Vec::new(),
    };
    prepare_bound_rows(db, graph, sources, declaration, actual, core, false)
}

/// Retain only actual parent where-bound candidates in its source/name frame.
/// The frame supplies the actual child-bound Self and parent parameter rebasing.
/// Item filtering and GAT binder remapping are distinct compiler producers.
pub(crate) fn prepare_parent_where_bounds(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    parent: &ra_ap_syntax::SyntaxNode,
    core: Option<crate::core::digest::Digest>,
) -> QueryPoll<Vec<SourcePredicateRecipe>> {
    let mut actual = source_types::actual_declaration_bound_syntax(declaration, parent);
    let where_range = parent
        .children()
        .find_map(ast::WhereClause::cast)
        .map(|clause| clause.syntax().text_range());
    actual.bounds.retain(|row| {
        where_range.is_some_and(|range| range.contains_range(row.bound.syntax().text_range()))
    });
    actual.implicit_sized.clear();
    prepare_bound_rows(db, graph, sources, declaration, actual, core, false)
}

fn prepare_bound_rows(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    actual: source_types::ActualDeclarationBounds,
    core: Option<crate::core::digest::Digest>,
    missing_self: bool,
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
    let has_self = |ty: &ast::Type| {
        ty.syntax()
            .descendants()
            .filter_map(ast::PathSegment::cast)
            .any(|segment| segment.kind() == Some(ast::PathSegmentKind::SelfTypeKw))
    };
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
            || crate::syntax::source_bound_polarity::polarity(&row.bound)
                == crate::syntax::source_bound_polarity::SourceBoundPolarity::Negative
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
                // A declaration without represented Self must not invoke the
                // legacy enclosing-impl text reconstruction fallback.
                if missing_self && has_self(&ty) {
                    pending!(Subject);
                }
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
                let Some(term) = declaration.region(&lifetime) else {
                    pending!(LifetimeName);
                };
                Subject::Region(term)
            }
            ActualBoundSubject::Missing => {
                pending!(Subject);
            }
        };
        if let Some(lifetime) = row.bound.lifetime() {
            let Some(target) = declaration.region(&lifetime) else {
                pending!(LifetimeName);
            };
            let result = match subject {
                Subject::Region(longer) => PredicateResult::RegionOutlives {
                    longer,
                    shorter: target,
                },
                Subject::Type(ty) => PredicateResult::TypeOutlives { ty, region: target },
            };
            recipes.push(SourcePredicateRecipe::Explicit { source, result });
            continue;
        }
        let Subject::Type(subject) = subject else {
            pending!(Subject);
        };
        let Some(ast::Type::PathType(ty)) = row.bound.ty() else {
            pending!(TraitName);
        };
        if missing_self && has_self(&ast::Type::PathType(ty.clone())) {
            pending!(SymbolicType);
        }
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
            &path,
            &subject
        )) else {
            pending!(TraitName);
        };
        // Preserve projected source terms here. The shared TypeIR source
        // binding closure demands actual associated declaration identities;
        // this recipe collector neither resolves those IDs nor normalizes.
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
