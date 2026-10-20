//! Actual owning-source default-bound policy; not a complete predicate query.
//! The marked policy region is generated from exact licensed pinned rustc.
use super::{
    configuration::{PreparedSolverConfiguration, SolverConfigurationKey},
    model::Span,
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader, SourceHeaderParent,
    },
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::declaration_source_predicates::DeclarationSourcePredicatesKey,
    syntax::{signatures::GenericKind, spans::SourceRange},
    traits::{
        predicate::{PredicateResult, SourcePredicateRecipe},
        term::{ConstTerm, GenericArg, ParamKey, RegionTerm, TraitRef, TyTerm},
    },
};
use ra_ap_syntax::{AstNode, SyntaxKind, ast};
use std::{collections::BTreeMap, sync::Arc};

pub type SourceDefaultBoundsKey = DeclarationSourcePredicatesKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum DefaultBoundsPending {
    Header,
    Environment,
    Configuration,
    LanguageIdentity,
    Context,
    WrittenRecipe,
    WrittenPolarity,
    SourceSite,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DefaultBoundsContext {
    TypeParameter(ParamKey),
    TraitSelf,
    AssociatedType,
}
#[derive(Debug)]
pub struct PreparedSourceDefaultBounds {
    key: SourceDefaultBoundsKey,
    header: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    configuration: Option<Arc<QueryValue>>,
    parent: Option<Arc<QueryValue>>,
    members: Option<Arc<QueryValue>>,
    languages: Vec<Arc<QueryValue>>,
    identities: BTreeMap<LangItem, Digest>,
    pending: Option<DefaultBoundsPending>,
}
macro_rules! ready {
    ($e:expr) => {
        match $e {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
impl PreparedSourceDefaultBounds {
    pub fn key(&self) -> &SourceDefaultBoundsKey {
        &self.key
    }
    /// Preparation hold; application also checks context-specific language roles.
    pub fn pending(&self) -> Option<DefaultBoundsPending> {
        self.pending
    }
    /// A present value retains the actual source language identity receipt.
    /// Absence is a policy dependency, never a complete language registry.
    pub(crate) fn sizedness_identity(&self, compiler_name: &str) -> Option<Digest> {
        let role = match compiler_name {
            "sized" => LangItem::Sized,
            "meta_sized" => LangItem::MetaSized,
            "pointee_sized" => LangItem::PointeeSized,
            _ => return None,
        };
        self.identities.get(&role).copied()
    }
    fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(h) = self.header.as_ref() else {
            unreachable!("captured default-bound header")
        };
        h
    }
    fn configuration(&self) -> &PreparedSolverConfiguration {
        let Some(QueryValue::SolverConfiguration(c)) = self.configuration.as_deref() else {
            unreachable!("validated owning-crate configuration")
        };
        c
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceDefaultBoundsKey) -> QueryPoll<Self> {
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition: key.definition
            }))
        ))
        .value;
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            header,
            environment,
            configuration: None,
            parent: None,
            members: None,
            languages: Vec::new(),
            identities: BTreeMap::new(),
            pending: None,
        };
        macro_rules! pending {
            ($p:expr) => {{
                result.pending = Some($p);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if result.header().pending().is_some() {
            pending!(DefaultBoundsPending::Header);
        }
        let header_root = Arc::clone(&result.header);
        let QueryValue::DefinitionHeaderPlan(header) = header_root.as_ref() else {
            unreachable!()
        };
        let Some((parse, _, _, _)) = header.declaration().source_parts() else {
            pending!(DefaultBoundsPending::Header);
        };
        let environment_root = Arc::clone(&result.environment);
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment_root.as_ref() else {
            return QueryPoll::Ready(Err(fault("default-bound environment differs")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            pending!(DefaultBoundsPending::Environment);
        };
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let mut roots = sources.iter().filter(|s| {
            let o = &s.selection().outline;
            o.crate_key == header.scope().crate_key
                && o.module_path.is_empty()
                && o.parse.profile == parse.profile
        });
        let Some(root) = roots.next() else {
            pending!(DefaultBoundsPending::Configuration);
        };
        if roots.next().is_some() {
            pending!(DefaultBoundsPending::Configuration);
        }
        let configuration = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
                root.selection()
            )))
        ))
        .value;
        let QueryValue::SolverConfiguration(config) = configuration.as_ref() else {
            return QueryPoll::Ready(Err(fault("default-bound configuration differs")));
        };
        if config.default_bound_options().is_none() {
            pending!(DefaultBoundsPending::Configuration);
        }
        result.configuration = Some(Arc::clone(&configuration));
        if let SourceHeaderParent::Associated(parent) = header.source_parent() {
            result.parent = Some(
                ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                        inventory: key.inventory.clone(),
                        definition: *parent
                    }))
                ))
                .value,
            );
            if header.role() == DefinitionHeaderRole::TypeAlias
                && matches!(result.parent.as_deref(), Some(QueryValue::DefinitionHeaderPlan(parent)) if parent.role()==DefinitionHeaderRole::Trait)
            {
                result.members = Some(
                    ready!(db.fetch(
                        &SyntaxProvider,
                        QueryKey::TraitAssociatedTypes(Box::new(
                            crate::frontend::trait_associated_types::TraitAssociatedTypesKey {
                                inventory: key.inventory.clone(),
                                trait_definition: *parent,
                            }
                        ))
                    ))
                    .value,
                );
            }
        }
        // Pinned policy returns before requiring language items for this actual
        // owning-crate attribute. It is not a guessed missing-lang fallback.
        if config.rustc_no_implicit_bounds() {
            return QueryPoll::Ready(Ok(result));
        }
        let core = environment.language_item_core(&parse.profile);
        for (role, source_name, compiler_name) in [
            (LangItem::Sized, "Sized", "sized"),
            (LangItem::MetaSized, "MetaSized", "meta_sized"),
            (LangItem::PointeeSized, "PointeeSized", "pointee_sized"),
        ] {
            let Some(identity) = ready!(crate::frontend::language_items::marker_trait(
                db,
                &sources,
                &parse.profile,
                core,
                source_name
            )) else {
                // The exact missing-role namespace dependency is retained by
                // this memo; applicability is checked per application below.
                continue;
            };
            let language = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceLangIdentity(Box::new(DefinitionHeaderKey {
                    inventory: key.inventory.clone(),
                    definition: identity
                }))
            ))
            .value;
            let QueryValue::SourceLangIdentity(actual) = language.as_ref() else {
                return QueryPoll::Ready(Err(fault("default-bound language identity differs")));
            };
            if matches!(actual.classification(), Ok(Some((name,_))) if name==compiler_name) {
                result.identities.insert(role, identity);
            }
            result.languages.push(language);
        }
        QueryPoll::Ready(Ok(result))
    }
    /// Only implicit rows are returned. The caller keeps actual written rows,
    /// and replaces Q78's legacy Sized defaults rather than appending both.
    pub(crate) fn apply(
        &self,
        subject: &TyTerm,
        site: (Digest, SourceRange),
        context: DefaultBoundsContext,
        written: &[SourcePredicateRecipe],
    ) -> Result<Vec<SourcePredicateRecipe>, DefaultBoundsPending> {
        if let Some(p) = self.pending {
            return Err(p);
        }
        let header = self.header();
        let declaration = header.declaration();
        let Some((parse, _, _, parsed)) = declaration.source_parts() else {
            return Err(DefaultBoundsPending::Header);
        };
        let header_span = header.span();
        if site.0 != parse.content
            || site.1.start < header_span.start
            || site.1.end > header_span.end
            || site.1.start > site.1.end
        {
            return Err(DefaultBoundsPending::SourceSite);
        }
        let implied = match context {
            DefaultBoundsContext::TypeParameter(key)
                if key.definition == self.key.definition
                    && declaration
                        .layout()
                        .is_some_and(|layout| key.ordinal < layout.own)
                    && *subject == TyTerm::Param(key)
                    && declaration
                        .parameters()
                        .iter()
                        .any(|p| p.ordinal == key.ordinal && p.kind == GenericKind::Type) =>
            {
                ImpliedBoundsContext::TyParam(key)
            }
            DefaultBoundsContext::TraitSelf
                if header.role() == DefinitionHeaderRole::Trait
                    && declaration.has_trait_self()
                    && header.trait_header().is_some()
                    && *subject
                        == TyTerm::Param(ParamKey {
                            definition: self.key.definition,
                            ordinal: u32::try_from(declaration.parameters().len())
                                .map_err(|_| DefaultBoundsPending::Context)?,
                        }) =>
            {
                ImpliedBoundsContext::TraitDef(self.key.definition)
            }
            DefaultBoundsContext::AssociatedType
                if header.role() == DefinitionHeaderRole::TypeAlias =>
            {
                let Some(QueryValue::DefinitionHeaderPlan(parent)) = self.parent.as_deref() else {
                    return Err(DefaultBoundsPending::Context);
                };
                let TyTerm::Projection(projection) = subject else {
                    return Err(DefaultBoundsPending::Context);
                };
                let Some(layout) = declaration.layout() else {
                    return Err(DefaultBoundsPending::Context);
                };
                let parent_declaration = parent.declaration();
                let Some(QueryValue::TraitAssociatedTypes(members)) = self.members.as_deref()
                else {
                    return Err(DefaultBoundsPending::Context);
                };
                if parent.pending().is_some()
                    || parent.role() != DefinitionHeaderRole::Trait
                    || projection.trait_ref.trait_key != parent.key().definition
                    || members.lookup(projection.item) != Ok(self.key.definition)
                    || header
                        .name()
                        .map(crate::frontend::member_index::associated_name_key)
                        != Some(projection.item)
                    || !layout.implicit_self
                    || !parent_declaration.has_trait_self()
                    || usize::try_from(layout.inherited).ok()
                        != Some(parent_declaration.parameters().len())
                    || declaration.source_parts().map(|(_, h, _, _)| &h.context)
                        != parent_declaration
                            .source_parts()
                            .map(|(_, h, _, _)| &h.tokens)
                {
                    return Err(DefaultBoundsPending::Context);
                }
                let child_self = ParamKey {
                    definition: self.key.definition,
                    ordinal: layout
                        .own
                        .checked_add(layout.inherited)
                        .ok_or(DefaultBoundsPending::Context)?,
                };
                if projection.trait_ref.self_ty != TyTerm::Param(child_self) {
                    return Err(DefaultBoundsPending::Context);
                }
                let mut inherited = Vec::new();
                for parameter in parent_declaration.parameters() {
                    let ordinal = layout
                        .own
                        .checked_add(parameter.ordinal)
                        .ok_or(DefaultBoundsPending::Context)?;
                    let Some(child) = declaration
                        .parameters()
                        .iter()
                        .find(|p| p.ordinal == ordinal)
                    else {
                        return Err(DefaultBoundsPending::Context);
                    };
                    if child.kind != parameter.kind || child.name != parameter.name {
                        return Err(DefaultBoundsPending::Context);
                    }
                    inherited.push(source_argument(
                        child.kind,
                        ParamKey {
                            definition: self.key.definition,
                            ordinal,
                        },
                    ));
                }
                let own = declaration
                    .parameters()
                    .iter()
                    .filter(|p| p.ordinal < layout.own)
                    .map(|p| {
                        source_argument(
                            p.kind,
                            ParamKey {
                                definition: self.key.definition,
                                ordinal: p.ordinal,
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                if projection.trait_ref.args != inherited || projection.item_args != own {
                    return Err(DefaultBoundsPending::Context);
                }
                ImpliedBoundsContext::AssociatedTypeOrImplTrait
            }
            _ => return Err(DefaultBoundsPending::Context),
        };
        let cx = PolicyCx { plan: self };
        // Authentication above is independent of semantic applicability. The
        // pinned no-implicit early return does not inspect written HIR bounds.
        // Written pending rows remain with the caller; this returns defaults
        // only and cannot certify the caller's complete clause bundle.
        if cx.no_implicit() {
            return Ok(Vec::new());
        }
        let Some(pointee) = self.identities.get(&LangItem::PointeeSized) else {
            return Err(DefaultBoundsPending::LanguageIdentity);
        };
        if !self.identities.contains_key(&LangItem::MetaSized) {
            return Err(DefaultBoundsPending::LanguageIdentity);
        }
        let skips_written = matches!(implied,ImpliedBoundsContext::TraitDef(id) if id==*pointee || cx.trait_is_auto(id));
        if !skips_written && !self.identities.contains_key(&LangItem::Sized) {
            return Err(DefaultBoundsPending::LanguageIdentity);
        }
        let mut selected = Vec::new();
        for recipe in written.iter().filter(|_| !skips_written) {
            match recipe {
                SourcePredicateRecipe::Explicit { source, result } => {
                    if source.start < header_span.start || source.end > header_span.end {
                        return Err(DefaultBoundsPending::SourceSite);
                    }
                    let (reference, polarity) = match result {
                        PredicateResult::Implemented(r) => (r, BoundPolarity::Positive),
                        PredicateResult::Relaxed { trait_ref, .. } => {
                            (trait_ref, BoundPolarity::Maybe)
                        }
                        PredicateResult::Pending(_) => {
                            return Err(DefaultBoundsPending::WrittenRecipe);
                        }
                        _ => continue,
                    };
                    // Resolve polarity from the retained exact bound node. The
                    // common collector has no negative-clause transport yet;
                    // never reinterpret !Trait as a positive/default clause.
                    let bound = parsed
                        .syntax_at(*source, |n| ast::TypeBound::can_cast(n.kind()))
                        .map_err(|_| DefaultBoundsPending::SourceSite)?
                        .and_then(ast::TypeBound::cast)
                        .ok_or(DefaultBoundsPending::SourceSite)?;
                    if bound
                        .syntax()
                        .children_with_tokens()
                        .any(|n| n.kind() == SyntaxKind::BANG)
                    {
                        return Err(DefaultBoundsPending::WrittenPolarity);
                    }
                    if bound.question_mark_token().is_some()
                        != matches!(polarity, BoundPolarity::Maybe)
                    {
                        return Err(DefaultBoundsPending::WrittenPolarity);
                    }
                    if reference.self_ty == *subject {
                        selected.push(SourceBound {
                            trait_id: reference.trait_key,
                            polarity,
                            span: Span {
                                origin: parse.content,
                                start: source.start,
                                end: source.end,
                            },
                        });
                    }
                }
                SourcePredicateRecipe::ImplicitSizedDefault(_)
                | SourcePredicateRecipe::ResolvedSizedDefault(_) => {}
            }
        }
        let lowerer = PolicyLowerer {
            cx: PolicyCx { plan: self },
        };
        let span = Span {
            origin: site.0,
            start: site.1.start,
            end: site.1.end,
        };
        let mut bounds = Vec::new();
        lowerer.add_implicit_sizedness_bounds(&mut bounds, subject, &selected, implied, span);
        lowerer.add_default_traits(&mut bounds, subject, &selected, implied, span);
        Ok(bounds
            .into_iter()
            .map(|(trait_ref, span)| SourcePredicateRecipe::Explicit {
                source: SourceRange {
                    start: span.start,
                    end: span.end,
                },
                result: PredicateResult::Implemented(trait_ref),
            })
            .collect())
    }
}
impl StableEncode for PreparedSourceDefaultBounds {
    const DOMAIN: &'static str = "lrr/v1/source-default-bounds";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.header.fingerprint());
        e.digest(self.environment.fingerprint());
        e.option(&self.configuration, |e, v| e.digest(v.fingerprint()));
        e.option(&self.parent, |e, v| e.digest(v.fingerprint()));
        e.option(&self.members, |e, v| e.digest(v.fingerprint()));
        e.sequence(&self.languages, |e, v| e.digest(v.fingerprint()));
        e.u64(self.identities.len() as u64);
        for (role, id) in &self.identities {
            e.u8(*role as u8);
            e.digest(*id);
        }
        e.option(&self.pending, |e, p| e.u8(*p as u8));
    }
}
fn source_argument(kind: GenericKind, key: ParamKey) -> GenericArg {
    match kind {
        GenericKind::Type => GenericArg::Type(TyTerm::Param(key)),
        GenericKind::Lifetime => GenericArg::Region(RegionTerm::Param(key)),
        GenericKind::Const => GenericArg::Const(ConstTerm::Param(key)),
    }
}
fn fault(s: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", s.into())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
enum LangItem {
    Sized,
    MetaSized,
    PointeeSized,
    DefaultTrait1,
    DefaultTrait2,
    DefaultTrait3,
    DefaultTrait4,
}
#[derive(Debug, Clone, Copy)]
enum BoundPolarity {
    Positive,
    Maybe,
    Negative,
}
#[derive(Debug)]
struct SourceBound {
    trait_id: Digest,
    polarity: BoundPolarity,
    span: Span,
}
#[derive(Debug, Clone, Copy)]
enum ImpliedBoundsContext {
    TraitDef(Digest),
    TyParam(ParamKey),
    AssociatedTypeOrImplTrait,
}
#[derive(Clone, Copy)]
struct PolicyCx<'a> {
    plan: &'a PreparedSourceDefaultBounds,
}
impl PolicyCx<'_> {
    fn no_implicit(self) -> bool {
        self.plan.configuration().rustc_no_implicit_bounds()
    }
    fn require_lang_item(self, role: LangItem, _: Span) -> Digest {
        self.plan.identities[&role]
    }
    fn trait_is_auto(self, id: Digest) -> bool {
        // Only the authenticated current trait is queried by pinned policy.
        let header = self.plan.header();
        assert_eq!(id, header.key().definition);
        header
            .trait_header()
            .expect("validated actual trait header")
            .is_auto()
    }
    fn experimental_default_bounds(self) -> bool {
        self.plan
            .configuration()
            .default_bound_options()
            .expect("validated profile")
            .experimental_default_bounds()
    }
    fn lang_item(self, role: LangItem) -> Option<Digest> {
        self.plan.identities.get(&role).copied()
    }
}
struct PolicyLowerer<'a> {
    cx: PolicyCx<'a>,
}
impl<'a> PolicyLowerer<'a> {
    fn tcx(&self) -> PolicyCx<'a> {
        self.cx
    }
}

// BEGIN PINNED DEFAULT BOUNDS POLICY
#[derive(Debug, Default)]
struct CollectedBound {
    /// `Trait`
    positive: Option<Span>,
    /// `?Trait`
    maybe: Option<Span>,
    /// `!Trait`
    negative: Option<Span>,
}

impl CollectedBound {
    /// Returns `true` if any of `Trait`, `?Trait` or `!Trait` were encountered.
    fn any(&self) -> bool {
        self.positive.is_some() || self.maybe.is_some() || self.negative.is_some()
    }
}

#[derive(Debug)]
struct CollectedSizednessBounds {
    // Collected `Sized` bounds
    sized: CollectedBound,
    // Collected `MetaSized` bounds
    meta_sized: CollectedBound,
    // Collected `PointeeSized` bounds
    pointee_sized: CollectedBound,
}

impl CollectedSizednessBounds {
    /// Returns `true` if any of `Trait`, `?Trait` or `!Trait` were encountered for `Sized`,
    /// `MetaSized` or `PointeeSized`.
    fn any(&self) -> bool {
        self.sized.any() || self.meta_sized.any() || self.pointee_sized.any()
    }
}

fn search_bounds_for(
    hir_bounds: &[SourceBound],
    _context: ImpliedBoundsContext,
    mut f: impl FnMut(&SourceBound),
) {
    for bound in hir_bounds {
        f(bound);
    }
}
fn collect_bounds(
    hir_bounds: &[SourceBound],
    context: ImpliedBoundsContext,
    target_did: Digest,
) -> CollectedBound {
    let mut collect_into = CollectedBound::default();
    search_bounds_for(hir_bounds, context, |ptr| {
        if ptr.trait_id != target_did {
            return;
        }

        match ptr.polarity {
            BoundPolarity::Maybe => collect_into.maybe = Some(ptr.span),
            BoundPolarity::Negative => collect_into.negative = Some(ptr.span),
            BoundPolarity::Positive => collect_into.positive = Some(ptr.span),
        }
    });
    collect_into
}

fn collect_sizedness_bounds(
    tcx: PolicyCx<'_>,
    hir_bounds: &[SourceBound],
    context: ImpliedBoundsContext,
    span: Span,
) -> CollectedSizednessBounds {
    let sized_did = tcx.require_lang_item(LangItem::Sized, span);
    let sized = collect_bounds(hir_bounds, context, sized_did);

    let meta_sized_did = tcx.require_lang_item(LangItem::MetaSized, span);
    let meta_sized = collect_bounds(hir_bounds, context, meta_sized_did);

    let pointee_sized_did = tcx.require_lang_item(LangItem::PointeeSized, span);
    let pointee_sized = collect_bounds(hir_bounds, context, pointee_sized_did);

    CollectedSizednessBounds {
        sized,
        meta_sized,
        pointee_sized,
    }
}

fn add_trait_bound(
    tcx: PolicyCx<'_>,
    bounds: &mut Vec<(TraitRef, Span)>,
    self_ty: &TyTerm,
    did: Digest,
    span: Span,
) {
    let _ = tcx;
    let trait_ref = TraitRef {
        trait_key: did,
        self_ty: self_ty.clone(),
        args: Vec::new(),
    };
    // Preferable to put sizedness obligations first, since we report better errors for `Sized`
    // ambiguity.
    bounds.insert(0, (trait_ref, span));
}

impl PolicyLowerer<'_> {
    /// Adds sizedness bounds to a trait, trait alias, parameter, opaque type or associated type.
    ///
    /// - On parameters, opaque type and associated types, add default `Sized` bound if no explicit
    ///   sizedness bounds are present.
    /// - On traits and trait aliases, add default `MetaSized` supertrait if no explicit sizedness
    ///   bounds are present.
    /// - On parameters, opaque type, associated types and trait aliases, add a `MetaSized` bound if
    ///   a `?Sized` bound is present.
    pub(crate) fn add_implicit_sizedness_bounds(
        &self,
        bounds: &mut Vec<(TraitRef, Span)>,
        self_ty: &TyTerm,
        hir_bounds: &[SourceBound],
        context: ImpliedBoundsContext,
        span: Span,
    ) {
        let tcx = self.tcx();

        // Skip adding any default bounds if `#![rustc_no_implicit_bounds]`
        if tcx.no_implicit() {
            return;
        }

        let meta_sized_did = tcx.require_lang_item(LangItem::MetaSized, span);
        let pointee_sized_did = tcx.require_lang_item(LangItem::PointeeSized, span);

        // If adding sizedness bounds to a trait, then there are some relevant early exits
        match context {
            ImpliedBoundsContext::TraitDef(trait_did) => {
                let trait_did = trait_did;
                // Never add a default supertrait to `PointeeSized`.
                if trait_did == pointee_sized_did {
                    return;
                }
                // Don't add default sizedness supertraits to auto traits because it isn't possible to
                // relax an automatically added supertrait on the defn itself.
                if tcx.trait_is_auto(trait_did) {
                    return;
                }
            }
            ImpliedBoundsContext::TyParam(..) | ImpliedBoundsContext::AssociatedTypeOrImplTrait => {
            }
        }
        let collected = collect_sizedness_bounds(tcx, hir_bounds, context, span);
        if let Some(span) = collected.sized.maybe.or(collected.sized.negative)
            && collected.sized.positive.is_none()
            && !collected.meta_sized.any()
            && !collected.pointee_sized.any()
        {
            // `?Sized` is equivalent to `MetaSized` (but only add the bound if there aren't any
            // other explicit ones) - this can happen for trait aliases as well as bounds.
            add_trait_bound(tcx, bounds, self_ty, meta_sized_did, span);
        } else if !collected.any() {
            match context {
                ImpliedBoundsContext::TraitDef(..) => {
                    // If there are no explicit sizedness bounds on a trait then add a default
                    // `MetaSized` supertrait.
                    add_trait_bound(tcx, bounds, self_ty, meta_sized_did, span);
                }
                ImpliedBoundsContext::TyParam(..)
                | ImpliedBoundsContext::AssociatedTypeOrImplTrait => {
                    // If there are no explicit sizedness bounds on a parameter then add a default
                    // `Sized` bound.
                    let sized_did = tcx.require_lang_item(LangItem::Sized, span);
                    add_trait_bound(tcx, bounds, self_ty, sized_did, span);
                }
            }
        }
    }

    pub(crate) fn add_default_traits(
        &self,
        bounds: &mut Vec<(TraitRef, Span)>,
        self_ty: &TyTerm,
        hir_bounds: &[SourceBound],
        context: ImpliedBoundsContext,
        span: Span,
    ) {
        self.tcx()
            .default_traits()
            .iter()
            .for_each(|default_trait| {
                self.add_default_trait(*default_trait, bounds, self_ty, hir_bounds, context, span);
            });
    }

    /// Add a `experimental_default_bounds` bound to the `bounds` if appropriate.
    ///
    /// Doesn't add the bound if the HIR bounds contain any of `Trait`, `?Trait` or `!Trait`.
    pub(crate) fn add_default_trait(
        &self,
        trait_: LangItem,
        bounds: &mut Vec<(TraitRef, Span)>,
        self_ty: &TyTerm,
        hir_bounds: &[SourceBound],
        context: ImpliedBoundsContext,
        span: Span,
    ) {
        let tcx = self.tcx();

        // Supertraits for auto trait are unsound according to the unstable book:
        // https://doc.rust-lang.org/beta/unstable-book/language-features/auto-traits.html#supertraits
        if let ImpliedBoundsContext::TraitDef(trait_did) = context
            && self.tcx().trait_is_auto(trait_did.into())
        {
            return;
        }

        if let Some(trait_did) = tcx.lang_item(trait_)
            && self.should_add_default_traits(trait_did, hir_bounds, context)
        {
            add_trait_bound(tcx, bounds, self_ty, trait_did, span);
        }
    }

    /// Returns `true` if default trait bound should be added.
    fn should_add_default_traits(
        &self,
        trait_def_id: Digest,
        hir_bounds: &[SourceBound],
        context: ImpliedBoundsContext,
    ) -> bool {
        let collected = collect_bounds(hir_bounds, context, trait_def_id);
        !self.tcx().no_implicit() && !collected.any()
    }
}
impl PolicyCx<'_> {
    fn default_traits(self) -> &'static [LangItem] {
        if self.experimental_default_bounds() {
            &[
                LangItem::DefaultTrait1,
                LangItem::DefaultTrait2,
                LangItem::DefaultTrait3,
                LangItem::DefaultTrait4,
            ]
        } else {
            &[]
        }
    }
}
// END PINNED DEFAULT BOUNDS POLICY

/// Observe genuine retained Q82 defaults through the existing source-TypeIR
/// encoder. This does not publish a production item_bounds fact domain.
#[cfg(test)]
pub(crate) fn check_actual_item_default_encoding(
    db: &mut Database,
    key: &crate::frontend::source_item_bounds::SourceItemBoundsKey,
    expected_sized: Digest,
    expected_parent: Digest,
    expected_item: Digest,
) -> Result<usize, Diagnostic> {
    use super::{
        source_bindings::{DefinitionKind, SourceBindings, SourceRequirements},
        source_generics::SourceGenericLayouts,
    };
    use crate::{
        db::dependency::{InputKey, InputValue},
        frontend::source_item_bounds::CurrentSourceItemBounds,
    };
    use rustc_type_ir::{self as ty, inherent::*};
    fn poll<T>(value: QueryPoll<T>) -> Result<T, Diagnostic> {
        match value {
            QueryPoll::Ready(result) => result,
            QueryPoll::NeedInput(_) => Err(fault("actual item default encoding needs input")),
        }
    }
    let current = poll(CurrentSourceItemBounds::capture(db, key))?;
    current.require_current(db)?;
    let plan = current.plan();
    let recipes = plan
        .default_recipes()
        .map_err(|why| fault(&format!("actual output defaults pending: {why:?}")))?;
    let (origin, site) = plan
        .output_site()
        .ok_or_else(|| fault("actual output site absent"))?;
    let Some(TyTerm::Projection(projection)) = plan.projection_subject() else {
        return Err(fault("actual output projection absent"));
    };
    let TyTerm::Param(source_self) = projection.trait_ref.self_ty else {
        return Err(fault("this actual source control expects identity Self"));
    };
    let input = db
        .input(&InputKey::DiscoverySeal(key.inventory.clone()))
        .ok_or_else(|| fault("actual output catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("actual output catalog differs"));
    };
    let mut required = SourceRequirements::new();
    required.insert(key.definition, DefinitionKind::Declaration);
    for recipe in recipes {
        super::source_predicates::require_recipe(&mut required, recipe)
            .map_err(|why| fault(&format!("actual output recipe pending: {why:?}")))?;
    }
    let bindings = poll(SourceBindings::prepare(db, Arc::clone(&input), required))?;
    if let Some(why) = bindings.pending {
        return Err(fault(&format!(
            "actual output source closure pending: {why:?}"
        )));
    }
    let layouts = SourceGenericLayouts::from_bindings(Arc::clone(&input), [&bindings])?;
    let configuration = poll(
        db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
                catalog
                    .impl_catalog
                    .as_ref()
                    .ok_or_else(|| fault("actual output source catalog absent"))?
                    .root(),
            ))),
        ),
    )?
    .value;
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        unreachable!("actual output configuration")
    };
    let before = db.trace.events.len();
    let count =
        configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| {
            assert!(
                cx.services().ready(cx).is_err(),
                "source metadata cannot authorize a complete solver root"
            );
            let mut count = 0;
            for recipe in recipes {
                let SourcePredicateRecipe::Explicit {
                    source,
                    result: PredicateResult::Implemented(reference),
                } = recipe
                else {
                    panic!("actual default must be a real source trait recipe")
                };
                assert_eq!(reference.trait_key, expected_sized);
                assert_eq!(Some(&reference.self_ty), plan.projection_subject());
                assert_eq!(*source, site);
                let (clause, span) = super::source_predicates::encode_recipe(
                    cx,
                    configuration,
                    bindings.type_bindings(),
                    recipe,
                    origin,
                    |_| None,
                )
                .unwrap_or_else(|why| panic!("actual output default encode pending: {why:?}"))
                .expect("implicit trait default contributes a clause");
                assert_eq!(
                    (span.origin, span.start, span.end),
                    (origin, site.start, site.end)
                );
                let ty::ClauseKind::Trait(encoded) = clause.kind().skip_binder() else {
                    panic!("actual default lost trait clause")
                };
                assert_eq!(encoded.trait_ref.def_id.source(), expected_sized);
                assert_eq!(encoded.polarity, ty::ClausePolarity::Positive);
                let [super::model::Arg::Type(self_ty)] = encoded.trait_ref.args.items() else {
                    panic!("actual default lost Self argument")
                };
                super::source_terms::check_actual_projection_alias(
                    cx,
                    *self_ty,
                    source_self,
                    expected_parent,
                    expected_item,
                );
                count += 1;
            }
            count
        })?;
    assert_eq!(
        db.trace.events.len(),
        before,
        "retained source-default encoding performs no queries"
    );
    Ok(count)
}
