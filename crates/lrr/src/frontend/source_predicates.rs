//! Bound metadata through the one declaration collector and ordinary Type walk.
//! No source evaluation, static checking or predicate fulfillment occurs here.
use super::{
    names::NameNamespaces,
    source_types::{self, ActualBoundSubject, WrittenTypeDeclaration},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{memo::Database, poll::QueryPoll},
    source::changes::PreparedSourceView,
    syntax::ast_adapter::range,
    syntax::spans::SourceRange,
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{ParamKey, RegionTerm, TraitRef, TyTerm},
        upstream::source_default_bounds::DefaultBoundsContext,
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceBoundFilter {
    All,
    SelfOnly,
    SelfAndAssociatedTypeBounds,
}
#[derive(Debug, Clone, Copy)]
struct BoundSite {
    site: source_types::ActualBoundSite,
    owner: SourceRange,
    parameter: Option<ParamKey>,
}
/// The compiler's two consumers use different lower_bounds scratch vectors.
/// Declaration clauses have a fresh vector per predicate; filtered trait
/// probes reuse one vector for all selected generic predicates.
#[derive(Debug, Clone, Copy)]
enum BoundOrdering {
    Declaration,
    TraitProbe,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundList {
    Source(source_types::ActualBoundSite),
    GenericProbe,
}
/// Source-site identity is retained beside the one common lowered recipe list.
/// Only actual source collection can construct a default input view.
#[derive(Debug)]
pub(crate) struct PreparedWrittenBounds {
    definition: Digest,
    header: Digest,
    owner: SourceRange,
    recipes: Vec<SourcePredicateRecipe>,
    sites: BTreeMap<(u32, u32), BoundSite>,
    ordering: BoundOrdering,
}
#[derive(Clone, Copy)]
pub(crate) struct DefaultBoundInputs<'a> {
    bounds: &'a PreparedWrittenBounds,
    context: DefaultBoundsContext,
}
impl PreparedWrittenBounds {
    pub(crate) fn recipes(&self) -> &[SourcePredicateRecipe] {
        &self.recipes
    }
    pub(crate) fn into_recipes(self) -> Vec<SourcePredicateRecipe> {
        self.recipes
    }
    pub(crate) fn default_inputs(&self, context: DefaultBoundsContext) -> DefaultBoundInputs<'_> {
        DefaultBoundInputs {
            bounds: self,
            context,
        }
    }
    /// Compiler order: TraitSelf defaults, colon bounds, parameter defaults,
    /// then inline/where predicates. No lowering/planning is repeated here.
    pub(crate) fn with_default_sets(
        &self,
        defaults: Vec<(DefaultBoundsContext, Vec<SourcePredicateRecipe>)>,
    ) -> Vec<SourcePredicateRecipe> {
        let colon = self
            .recipes
            .iter()
            .take_while(|recipe| self.is_colon(recipe))
            .count();
        let mut result = Vec::new();
        for (_, rows) in defaults
            .iter()
            .filter(|(context, _)| *context == DefaultBoundsContext::TraitSelf)
        {
            result.extend(rows.iter().cloned());
        }
        result.extend(self.recipes[..colon].iter().cloned());
        for (_, rows) in defaults
            .into_iter()
            .filter(|(context, _)| *context != DefaultBoundsContext::TraitSelf)
        {
            result.extend(rows);
        }
        result.extend(self.recipes[colon..].iter().cloned());
        result
    }
    /// rustc's lower_poly_trait_ref inserts each actual Sized clause at the
    /// front of its current lower_bounds scratch vector. Its actual consumer
    /// determines whether generic predicates share that vector. Retain
    /// distinct source spans.
    /// The role comes from the authenticated compiler-language receipt; a
    /// same-spelled user trait cannot select this ordering rule.
    pub(crate) fn order_explicit_sized(
        &mut self,
        policy: &crate::traits::upstream::source_default_bounds::PreparedSourceDefaultBounds,
    ) -> Result<(), crate::traits::upstream::source_default_bounds::DefaultBoundsPending> {
        use crate::traits::upstream::source_default_bounds::DefaultBoundsPending;
        if policy.key().definition != self.definition {
            return Err(DefaultBoundsPending::Context);
        }
        if let Some(why) = policy.pending() {
            return Err(why);
        }
        if !self.recipes.iter().any(|recipe| {
            matches!(
                recipe,
                SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Implemented(_),
                    ..
                }
            )
        }) {
            return Ok(());
        }
        let sized = policy
            .sizedness_identity("sized")
            .ok_or(DefaultBoundsPending::LanguageIdentity)?;
        if self.recipes.iter().any(|recipe| {
            matches!(recipe,
            SourcePredicateRecipe::Explicit { source, .. }
                if !self.sites.contains_key(&(source.start, source.end)))
        }) {
            return Err(DefaultBoundsPending::SourceSite);
        }
        let mut current_list = None;
        let mut list_start = 0;
        let mut ordered = Vec::with_capacity(self.recipes.len());
        for recipe in std::mem::take(&mut self.recipes) {
            let (list, front) = match &recipe {
                SourcePredicateRecipe::Explicit { source, result } => {
                    let site = &self.sites[&(source.start, source.end)];
                    let list = match (self.ordering, site.site) {
                        (
                            BoundOrdering::TraitProbe,
                            source_types::ActualBoundSite::Parameter(_)
                            | source_types::ActualBoundSite::Where { .. },
                        ) => BoundList::GenericProbe,
                        _ => BoundList::Source(site.site),
                    };
                    (
                        Some((site.owner, list)),
                        matches!(result,
                        PredicateResult::Implemented(trait_ref) if trait_ref.trait_key == sized),
                    )
                }
                _ => (None, false),
            };
            if list != current_list || list.is_none() {
                list_start = ordered.len();
                current_list = list;
            }
            if front {
                ordered.insert(list_start, recipe);
            } else {
                ordered.push(recipe);
            }
        }
        self.recipes = ordered;
        Ok(())
    }
    pub(crate) fn with_defaults(
        &self,
        context: DefaultBoundsContext,
        defaults: Vec<SourcePredicateRecipe>,
    ) -> Vec<SourcePredicateRecipe> {
        self.with_default_sets(vec![(context, defaults)])
    }
    fn is_colon(&self, recipe: &SourcePredicateRecipe) -> bool {
        let SourcePredicateRecipe::Explicit { source, .. } = recipe else {
            return false;
        };
        self.sites
            .get(&(source.start, source.end))
            .is_some_and(|site| matches!(site.site, source_types::ActualBoundSite::TraitColon))
    }
}
impl DefaultBoundInputs<'_> {
    pub(crate) fn matches(
        &self,
        definition: Digest,
        header: Digest,
        owner: SourceRange,
        context: DefaultBoundsContext,
    ) -> bool {
        self.bounds.definition == definition
            && self.bounds.header == header
            && self.bounds.owner == owner
            && self.context == context
    }
    pub(crate) fn recipes(&self) -> impl Iterator<Item = &SourcePredicateRecipe> {
        self.bounds.recipes.iter().filter(|recipe| {
            let SourcePredicateRecipe::Explicit { source, .. } = recipe else {
                return false;
            };
            let Some(site) = self.bounds.sites.get(&(source.start, source.end)) else {
                return false;
            };
            match self.context {
                DefaultBoundsContext::TraitSelf => {
                    matches!(site.site, source_types::ActualBoundSite::TraitColon)
                }
                DefaultBoundsContext::TypeParameter(key) => site.parameter == Some(key),
                DefaultBoundsContext::AssociatedType => {
                    matches!(site.site, source_types::ActualBoundSite::AssociatedOutput)
                }
            }
        })
    }
}
impl StableEncode for PreparedWrittenBounds {
    const DOMAIN: &'static str = "lrr/v2/source-written-bound-sites";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.header);
        e.u32(self.owner.start);
        e.u32(self.owner.end);
        e.u32(match self.ordering {
            BoundOrdering::Declaration => 0,
            BoundOrdering::TraitProbe => 1,
        });
        e.sequence(&self.recipes, |e, recipe| e.record(recipe));
        e.map(&self.sites, |e, (start, end), site| {
            e.u32(*start);
            e.u32(*end);
            e.u32(site.owner.start);
            e.u32(site.owner.end);
            match site.site {
                source_types::ActualBoundSite::TraitColon => e.u32(0),
                source_types::ActualBoundSite::Parameter(key) => {
                    e.u32(1);
                    e.record(&key);
                }
                source_types::ActualBoundSite::Where { predicate } => {
                    e.u32(2);
                    e.u32(predicate.start);
                    e.u32(predicate.end);
                }
                source_types::ActualBoundSite::AssociatedOutput => e.u32(3),
            }
            e.option(&site.parameter, |e, key| e.record(key));
        });
    }
}
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
    match prepare_selected(db, graph, sources, declaration, owner, core, None) {
        QueryPoll::Ready(Ok(bounds)) => QueryPoll::Ready(Ok(bounds.into_recipes())),
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
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
) -> QueryPoll<PreparedWrittenBounds> {
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
) -> QueryPoll<PreparedWrittenBounds> {
    let missing_self = matches!(own, Some((_, None)));
    let mut actual = source_types::actual_declaration_bound_syntax(declaration, owner);
    if let Some((_own_parameters, self_type)) = own {
        let owner_range = range(owner.text_range());
        actual.bounds.retain(|row| row.owner == owner_range);
        // Own-source defaults are supplied by the retained Q84 compiler policy.
        // Do not run the legacy simple-Sized filter and discard its result.
        actual.implicit_sized.clear();
        // The collector's trait supertrait subject has no written parameter
        // node. Use only the actual declaration's supplied implicit Self slot.
        if ast::Trait::can_cast(owner.kind()) {
            for row in &mut actual.bounds {
                if matches!(row.subject, ActualBoundSubject::Missing)
                    && row.site == source_types::ActualBoundSite::TraitColon
                    && let Some(self_type) = self_type
                {
                    row.subject = ActualBoundSubject::Type(self_type.clone());
                }
            }
        }
    }
    prepare_written_rows(
        db,
        graph,
        sources,
        declaration,
        owner,
        actual,
        core,
        missing_self,
        BoundOrdering::Declaration,
    )
}

/// Same source collector/lowerer as explicit-own preparation. The compiler's
/// requested subject domain is selected before Type/name/binder preparation.
pub(crate) fn prepare_filtered_trait_bounds(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    owner: &ra_ap_syntax::SyntaxNode,
    core: Option<Digest>,
    self_type: &TyTerm,
    filter: SourceBoundFilter,
) -> QueryPoll<PreparedWrittenBounds> {
    let mut actual = source_types::actual_declaration_bound_syntax(declaration, owner);
    actual.implicit_sized.clear();
    let own_range = range(owner.text_range());
    actual.bounds.retain(|row| {
        if row.owner != own_range {
            return false;
        }
        if row.site == source_types::ActualBoundSite::TraitColon {
            return true;
        }
        if filter == SourceBoundFilter::All {
            // probe_ty_param_bounds_in_generics skips region predicates.
            return !matches!(
                row.subject,
                ActualBoundSubject::Region(_) | ActualBoundSubject::RegionSyntax(_)
            );
        }
        if !matches!(row.site, source_types::ActualBoundSite::Where { .. }) {
            return false;
        }
        match &row.subject {
            ActualBoundSubject::Type(ty) => ty == self_type,
            ActualBoundSubject::TypeSyntax(ty) => direct_self_type(ty),
            _ => false,
        }
    });
    for row in &mut actual.bounds {
        if row.site == source_types::ActualBoundSite::TraitColon {
            row.subject = ActualBoundSubject::Type(self_type.clone());
        }
    }
    // Associated constraints still enter the one lowerer. Its explicit
    // AssociatedIdentity pending is retained, including equality constraints;
    // no pending row is dropped to claim complete filtered clauses.
    prepare_written_rows(
        db,
        graph,
        sources,
        declaration,
        owner,
        actual,
        core,
        false,
        BoundOrdering::TraitProbe,
    )
}
fn direct_self_type(ty: &ast::Type) -> bool {
    let mut ty = ty.clone();
    while let ast::Type::ParenType(paren) = &ty {
        let Some(inner) = paren.ty() else {
            return false;
        };
        ty = inner;
    }
    let ast::Type::PathType(path) = ty else {
        return false;
    };
    let Some(path) = path.path() else {
        return false;
    };
    path.qualifier().is_none()
        && path.coloncolon_token().is_none()
        && path.segment().is_some_and(|segment| {
            segment.kind() == Some(ast::PathSegmentKind::SelfTypeKw)
                && segment.generic_arg_list().is_none()
                && segment.parenthesized_arg_list().is_none()
        })
}
fn prepare_written_rows(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    declaration: &WrittenTypeDeclaration<'_>,
    owner: &ra_ap_syntax::SyntaxNode,
    actual: source_types::ActualDeclarationBounds,
    core: Option<Digest>,
    missing_self: bool,
    ordering: BoundOrdering,
) -> QueryPoll<PreparedWrittenBounds> {
    let mut sites = BTreeMap::new();
    for row in &actual.bounds {
        let source = range(row.bound.syntax().text_range());
        let parameter = match &row.subject {
            ActualBoundSubject::Type(TyTerm::Param(key)) => Some(*key),
            ActualBoundSubject::TypeSyntax(ty) => match declaration.direct_type_parameter(ty) {
                Ok(parameter) => parameter,
                Err(error) => return QueryPoll::Ready(Err(error)),
            },
            _ => None,
        };
        if sites
            .insert(
                (source.start, source.end),
                BoundSite {
                    site: row.site,
                    owner: row.owner,
                    parameter,
                },
            )
            .is_some()
        {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-SOURCE-BOUND-SITE",
                "duplicate actual bound site".into(),
            )));
        }
    }
    let recipes =
        match prepare_bound_rows(db, graph, sources, declaration, actual, core, missing_self) {
            QueryPoll::Ready(Ok(recipes)) => recipes,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
    if recipes.iter().any(|recipe| {
        matches!(recipe, SourcePredicateRecipe::Explicit { source,.. }
        if !sites.contains_key(&(source.start,source.end)))
    }) {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-SOURCE-BOUND-SITE",
            "lowered bound lacks actual site".into(),
        )));
    }
    QueryPoll::Ready(Ok(PreparedWrittenBounds {
        definition: declaration.definition(),
        header: declaration.header_identity(),
        owner: range(owner.text_range()),
        recipes,
        sites,
        ordering,
    }))
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
) -> QueryPoll<PreparedWrittenBounds> {
    let actual = source_types::ActualDeclarationBounds {
        bounds: alias
            .type_bound_list()
            .into_iter()
            .flat_map(|list| list.bounds())
            .map(|bound| source_types::ActualDeclarationBound {
                subject: ActualBoundSubject::Type(subject.clone()),
                bound,
                where_binder: None,
                site: source_types::ActualBoundSite::AssociatedOutput,
                owner: range(alias.syntax().text_range()),
            })
            .collect(),
        implicit_sized: Vec::new(),
    };
    prepare_written_rows(
        db,
        graph,
        sources,
        declaration,
        alias.syntax(),
        actual,
        core,
        false,
        BoundOrdering::Declaration,
    )
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
