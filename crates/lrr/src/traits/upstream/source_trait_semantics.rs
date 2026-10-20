//! Immutable source trait flags from pinned collect.rs::trait_def.
//! These query domains are not complete TraitFacts or solver readiness.
use super::{
    configuration::SolverConfigurationKey,
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader,
    },
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    syntax::{
        outline::{AttributeClass, AttributeRecipe},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

pub type SourceTraitSemanticsKey = DefinitionHeaderKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TraitSemantic<T> {
    Known(T),
    NotApplicable,
    Pending(TraitSemanticsPending),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum TraitSemanticsPending {
    Header = 0,
    TraitDeclaration = 1,
    AttributeExpansion = 2,
    AttributeSyntax = 3,
    DynCompatibility = 4,
    Configuration = 5,
    LanguageIdentity = 6,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum TraitBooleanQuery {
    Auto = 0,
    Marker = 1,
    Coinductive = 2,
    Alias = 3,
    Fundamental = 4,
    Unsafe = 5,
    DenyExplicitImpl = 6,
    DefaultTrait = 7,
    SizednessTrait = 8,
}
#[derive(Debug)]
pub struct PreparedSourceTraitSemantics {
    key: SourceTraitSemanticsKey,
    header: Arc<QueryValue>,
    declaration: Option<Arc<QueryValue>>,
    environment: Option<Arc<QueryValue>>,
    configuration: Option<Arc<QueryValue>>,
    language: Option<Arc<QueryValue>>,
    booleans: [TraitSemantic<bool>; 9],
    force_dyn_incompatible: TraitSemantic<Option<SourceRange>>,
    /// Source capture only. Individual query availability is returned below.
    pending: Option<TraitSemanticsPending>,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl PreparedSourceTraitSemantics {
    pub fn key(&self) -> &SourceTraitSemanticsKey {
        &self.key
    }
    pub fn pending(&self) -> Option<TraitSemanticsPending> {
        self.pending
    }
    pub(crate) fn boolean(&self, query: TraitBooleanQuery) -> TraitSemantic<bool> {
        self.booleans[query as usize]
    }
    /// Known(None) means actual absence of this attribute in the captured
    /// effective inventory, not absence of compiler dyn incompatibilities.
    pub(crate) fn force_dyn_incompatible(&self) -> TraitSemantic<Option<SourceRange>> {
        self.force_dyn_incompatible
    }
    pub(crate) fn dyn_compatible(&self) -> TraitSemantic<bool> {
        match self.force_dyn_incompatible {
            TraitSemantic::Known(Some(_)) => TraitSemantic::Known(false),
            TraitSemantic::Known(None) => {
                TraitSemantic::Pending(TraitSemanticsPending::DynCompatibility)
            }
            TraitSemantic::NotApplicable => TraitSemantic::NotApplicable,
            TraitSemantic::Pending(why) => TraitSemantic::Pending(why),
        }
    }
    pub(crate) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(header) = self.header.as_ref() else {
            unreachable!("captured actual trait header")
        };
        header
    }
    pub(crate) fn source_origin(&self) -> Digest {
        self.header().span().origin
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceTraitSemanticsKey) -> QueryPoll<Self> {
        use TraitSemantic::{Known, NotApplicable, Pending};
        use TraitSemanticsPending as P;
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            header,
            declaration: None,
            environment: None,
            configuration: None,
            language: None,
            booleans: [Pending(P::Header); 9],
            force_dyn_incompatible: Pending(P::Header),
            pending: None,
        };
        macro_rules! pending {
            ($why:expr) => {{
                let why = $why;
                result.pending = Some(why);
                result.booleans = [Pending(why); 9];
                result.force_dyn_incompatible = Pending(why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if result.header().pending().is_some() {
            pending!(P::Header);
        }
        if result.header().role() != DefinitionHeaderRole::Trait {
            result.booleans = [NotApplicable; 9];
            result.force_dyn_incompatible = NotApplicable;
            return QueryPoll::Ready(Ok(result));
        }
        let root = Arc::clone(&result.header);
        let QueryValue::DefinitionHeaderPlan(header) = root.as_ref() else {
            unreachable!()
        };
        let Some((parse, recipe, context, parsed)) = header.declaration().source_parts() else {
            pending!(P::Header);
        };
        let declaration =
            ready!(db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(key.definition))).value;
        result.declaration = Some(Arc::clone(&declaration));
        let QueryValue::TraitDeclaration(actual) = declaration.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait semantics declaration query differs")));
        };
        if !actual.matches_owner(key.definition, parse, recipe, &context.scope) {
            pending!(P::TraitDeclaration);
        }
        let Some(trait_header) = actual.header() else {
            pending!(P::TraitDeclaration);
        };
        let Some(captured) = header.trait_header() else {
            pending!(P::TraitDeclaration);
        };
        if captured.fingerprint() != trait_header.fingerprint() {
            pending!(P::TraitDeclaration);
        }
        if recipe.attributes.iter().any(|attribute| {
            matches!(
                attribute.class,
                AttributeClass::TransformUnsupported
                    | AttributeClass::Derive
                    | AttributeClass::Test
            )
        }) {
            pending!(P::AttributeExpansion);
        }
        // Exact flag policy from pinned rustc_hir_analysis/src/collect.rs
        // trait_def:1088-1166 (ae0fe511b15ab760d7e963b13389d1bc2ab42b18).
        // TraitDeclaration/actual attributes replace HIR/attr-context reads;
        // no candidate selection or independent trait algorithm lives here.
        let is_alias = trait_header.is_alias();
        let is_auto = if is_alias {
            false
        } else {
            trait_header.is_auto()
        };
        let safety = if is_alias {
            false
        } else {
            trait_header.is_unsafe()
        };
        let attribute = |name| flag_attribute(&recipe.attributes, name, parsed, context.range);
        let marker = attribute("marker");
        let rustc_coinductive = attribute("rustc_coinductive");
        let fundamental = attribute("fundamental");
        let deny_explicit_impl = attribute("rustc_deny_explicit_impl");
        let force_dyn_incompatible = attribute("rustc_dyn_incompatible_trait");
        result.booleans[TraitBooleanQuery::Auto as usize] = Known(is_auto);
        result.booleans[TraitBooleanQuery::Alias as usize] = Known(is_alias);
        result.booleans[TraitBooleanQuery::Unsafe as usize] = Known(safety);
        // Pinned short-circuiting: only regular traits can be marker, and an
        // auto trait is coinductive independently of a coinductive attribute.
        result.booleans[TraitBooleanQuery::Marker as usize] = if is_alias {
            Known(false)
        } else {
            presence(marker)
        };
        result.booleans[TraitBooleanQuery::Coinductive as usize] = if is_auto {
            Known(true)
        } else {
            presence(rustc_coinductive)
        };
        result.booleans[TraitBooleanQuery::Fundamental as usize] = presence(fundamental);
        result.booleans[TraitBooleanQuery::DenyExplicitImpl as usize] =
            presence(deny_explicit_impl);
        result.force_dyn_incompatible = force_dyn_incompatible;
        // Pinned ty/context.rs::is_sizedness_trait asks compiler lang roles,
        // never the declaration name. Q76 authenticates source/facet identity.
        let language = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceLangIdentity(Box::new(key.clone()))
        ))
        .value;
        let QueryValue::SourceLangIdentity(identity) = language.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait semantics language query differs")));
        };
        result.booleans[TraitBooleanQuery::SizednessTrait as usize] =
            match identity.classification() {
                Ok(Some(("sized" | "meta_sized", _))) => Known(true),
                Ok(_) => Known(false),
                Err(_) => Pending(P::LanguageIdentity),
            };
        result.language = Some(language);
        // Pinned ty/context.rs::default_traits/is_default_trait. The private
        // no-override receipt is issued only for the actual admitted profile;
        // no absent option/fact-map key is interpreted as false.
        result.booleans[TraitBooleanQuery::DefaultTrait as usize] = Pending(P::Configuration);
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        result.environment = Some(Arc::clone(&environment));
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait semantics source inventory differs")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            return QueryPoll::Ready(Ok(result));
        };
        let mut roots = environment.sources().iter().filter(|source| {
            let outline = &source.selection().outline;
            outline.crate_key == context.scope.crate_key
                && outline.module_path.is_empty()
                && outline.parse.profile == parse.profile
        });
        let Some(root) = roots.next() else {
            return QueryPoll::Ready(Ok(result));
        };
        if roots.next().is_some() {
            return QueryPoll::Ready(Ok(result));
        }
        let configuration = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
                root.selection()
            )))
        ))
        .value;
        let QueryValue::SolverConfiguration(configuration_plan) = configuration.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait semantics configuration differs")));
        };
        result.booleans[TraitBooleanQuery::DefaultTrait as usize] =
            match configuration_plan.default_bound_options() {
                Some(options) if !options.experimental_default_bounds() => Known(false),
                // A future option-enabled/Cargo profile needs its actual option
                // and language producer; it is not an empty default-trait list.
                _ => Pending(P::Configuration),
            };
        result.configuration = Some(configuration);
        QueryPoll::Ready(Ok(result))
    }
}
fn presence(value: TraitSemantic<Option<SourceRange>>) -> TraitSemantic<bool> {
    match value {
        TraitSemantic::Known(value) => TraitSemantic::Known(value.is_some()),
        TraitSemantic::NotApplicable => TraitSemantic::NotApplicable,
        TraitSemantic::Pending(why) => TraitSemantic::Pending(why),
    }
}
/// Effective cfg attributes already come from the shared outline service.
/// Inspect their retained AST node; do not parse source text or invent a
/// positive flag from a malformed attribute. Absence is authenticated here.
fn flag_attribute(
    attributes: &[AttributeRecipe],
    name: &str,
    parsed: &crate::syntax::parse::ParsedFile,
    declaration: SourceRange,
) -> TraitSemantic<Option<SourceRange>> {
    let mut selected = None;
    for attribute in attributes.iter().filter(|attribute| attribute.name == name) {
        let Some(source) = attribute.source_meta else {
            return TraitSemantic::Pending(TraitSemanticsPending::AttributeSyntax);
        };
        if source.start < declaration.start
            || source.end > declaration.end
            || source.start > source.end
        {
            return TraitSemantic::Pending(TraitSemanticsPending::AttributeSyntax);
        }
        if !matches!(attribute.tokens.as_slice(),[token] if token.text==name) {
            return TraitSemantic::Pending(TraitSemanticsPending::AttributeSyntax);
        }
        let Ok(Some(node)) = parsed.syntax_at(source, |node| ast::Meta::can_cast(node.kind()))
        else {
            return TraitSemantic::Pending(TraitSemanticsPending::AttributeSyntax);
        };
        if crate::syntax::ast_adapter::tokens(&node, None) != attribute.tokens
            || !matches!(ast::Meta::cast(node), Some(ast::Meta::PathMeta(_)))
        {
            return TraitSemantic::Pending(TraitSemanticsPending::AttributeSyntax);
        }
        // Compiler inert flag attributes contain no value. Duplicate flags
        // have the same semantic presence; keep an actual diagnostic site.
        selected = Some(source);
    }
    TraitSemantic::Known(selected)
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
impl StableEncode for PreparedSourceTraitSemantics {
    const DOMAIN: &'static str = "lrr/v1/source-trait-semantics";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.header.fingerprint());
        e.option(&self.declaration, |e, value| e.digest(value.fingerprint()));
        for root in [&self.environment, &self.configuration, &self.language] {
            e.option(root, |e, value| e.digest(value.fingerprint()));
        }
        e.sequence(&self.booleans, |e, value| match value {
            TraitSemantic::Known(value) => {
                e.u8(0);
                e.bool(*value);
            }
            TraitSemantic::NotApplicable => e.u8(1),
            TraitSemantic::Pending(why) => {
                e.u8(2);
                e.u8(*why as u8);
            }
        });
        match self.force_dyn_incompatible {
            TraitSemantic::Known(value) => {
                e.u8(0);
                e.option(&value, |e, source| {
                    e.digest(self.source_origin());
                    e.u32(source.start);
                    e.u32(source.end);
                });
            }
            TraitSemantic::NotApplicable => e.u8(1),
            TraitSemantic::Pending(why) => {
                e.u8(2);
                e.u8(why as u8);
            }
        }
        e.option(&self.pending, |e, why| e.u8(*why as u8));
    }
}
#[derive(Debug)]
pub(crate) struct CurrentSourceTraitSemantics {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceTraitSemantics {
    pub(crate) fn capture(db: &mut Database, key: &SourceTraitSemanticsKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceTraitSemantics(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(),QueryValue::SourceTraitSemantics(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("actual trait semantics memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceTraitSemantics {
        let QueryValue::SourceTraitSemantics(plan) = self.value.as_ref() else {
            unreachable!("captured trait semantics memo")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
