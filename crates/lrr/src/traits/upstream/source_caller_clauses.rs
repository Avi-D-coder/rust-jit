//! Retained caller assumptions from actual source declarations, never callee
//! obligations. Explicit source hierarchy alone is not a complete ParamEnv.
use super::{
    configuration::{PreparedSolverConfiguration, SolverConfigurationKey},
    model::{self, AllocateList},
    parameters::{DeclarationParameters, DeclaredParameterPreparation, ParameterPending},
    prepared::{self, Cx},
    root_input::CallerSourceContext,
    source_declaration_clauses::{
        EncodedSourceClauseHierarchy, PreparedSourceDeclarationClauseHierarchy,
        SourceDeclarationClausesKey, SourceDeclarationClausesPending,
    },
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader, SourceHeaderParent,
    },
    source_definition_semantics::{DefinitionBooleanQuery, SourceSemantic},
    source_generics::SourceGenericLayouts,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    syntax::outline::AttributeClass,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams},
};
use std::sync::Arc;

pub(crate) type SourceCallerClausesKey = DefinitionHeaderKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceCallerClausesPending {
    Header,
    Hierarchy(SourceDeclarationClausesPending),
    UnsupportedOwner,
    ParentClosure,
    Parameters(ParameterPending),
    NonemptyParameters,
    WrittenClauses,
    AttributeEffects,
    Configuration,
    GenericConstExpressions,
    ConditionalConstness,
    CallerIdentity,
    /// A supplied GenericEnv/application still needs its actual instantiation
    /// receipt. Neither an absent nor an incomplete map means empty assumptions.
    CallerApplication,
    SourceWorld,
}
#[derive(Debug)]
struct RetainedQuery {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
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
impl RetainedQuery {
    fn capture(db: &mut Database, key: QueryKey) -> QueryPoll<Self> {
        let outcome = ready!(db.fetch(&SyntaxProvider, key.clone()));
        if !outcome.cacheable {
            return QueryPoll::Ready(Err(fault("caller source query is not immutable")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &key, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
}
/// A demanded source plan suitable for retention by a root-input owner. The
/// encoded empty branch below is source-proven; no root token is issued here.
#[derive(Debug)]
pub(crate) struct PreparedSourceCallerClauses {
    key: SourceCallerClausesKey,
    header: RetainedQuery,
    hierarchy: RetainedQuery,
    semantics: Option<RetainedQuery>,
    configuration: Option<RetainedQuery>,
    catalog: Arc<InputValue>,
    pending: Option<SourceCallerClausesPending>,
}
impl PreparedSourceCallerClauses {
    pub(crate) fn key(&self) -> &SourceCallerClausesKey {
        &self.key
    }
    pub(crate) fn pending(&self) -> Option<SourceCallerClausesPending> {
        self.pending
    }
    pub(crate) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(header) = self.header.value.as_ref() else {
            unreachable!("retained caller definition header")
        };
        header
    }
    pub(crate) fn hierarchy(&self) -> &PreparedSourceDeclarationClauseHierarchy {
        let QueryValue::SourceDeclarationClauseHierarchy(plan) = self.hierarchy.value.as_ref()
        else {
            unreachable!("retained actual caller clause hierarchy")
        };
        plan
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        self.hierarchy().generic_layouts()
    }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> {
        &self.catalog
    }
    pub(crate) fn configuration(&self) -> Option<&PreparedSolverConfiguration> {
        self.configuration.as_ref().map(|query| {
            let QueryValue::SolverConfiguration(configuration) = query.value.as_ref() else {
                unreachable!("retained actual caller configuration")
            };
            configuration.as_ref()
        })
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        for query in [
            Some(&self.header),
            Some(&self.hierarchy),
            self.semantics.as_ref(),
            self.configuration.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            query.guard.require_current(db)?;
        }
        if !db
            .published_input(&InputKey::DiscoverySeal(self.key.inventory.clone()))
            .is_some_and(|(_, fingerprint)| fingerprint == self.catalog.fingerprint())
        {
            return Err(fault("caller source catalog changed"));
        }
        Ok(())
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceCallerClausesKey) -> QueryPoll<Self> {
        let header = ready!(RetainedQuery::capture(
            db,
            QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))
        ));
        let own_key = SourceDeclarationClausesKey {
            inventory: key.inventory.clone(),
            definition: key.definition,
        };
        let hierarchy = ready!(RetainedQuery::capture(
            db,
            QueryKey::SourceDeclarationClauseHierarchy(Box::new(own_key.clone()))
        ));
        let Some(catalog) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("caller source catalog absent")));
        };
        let InputValue::DiscoverySeal(seal) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("caller source catalog input differs")));
        };
        if let Err(error) = prepared::catalog_world(seal) {
            return QueryPoll::Ready(Err(error));
        }
        if !matches!(header.value.as_ref(), QueryValue::DefinitionHeaderPlan(plan) if plan.key()==key)
            || !matches!(hierarchy.value.as_ref(), QueryValue::SourceDeclarationClauseHierarchy(plan) if plan.key()==&own_key)
        {
            return QueryPoll::Ready(Err(fault("caller source query identity differs")));
        }
        let mut result = Self {
            key: key.clone(),
            header,
            hierarchy,
            semantics: None,
            configuration: None,
            catalog: Arc::clone(&catalog),
            pending: None,
        };
        macro_rules! pending {
            ($why:expr) => {{
                result.pending = Some($why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if result.header().pending().is_some() {
            pending!(SourceCallerClausesPending::Header);
        }
        let source = result.hierarchy().own().source();
        let Some((parse, recipe, context, parsed)) = result.header().declaration().source_parts()
        else {
            pending!(SourceCallerClausesPending::Header);
        };
        if !source
            .declaration()
            .matches_metadata(parse, recipe, context)
        {
            return QueryPoll::Ready(Err(fault("caller hierarchy declaration receipt differs")));
        }
        if let Some(why) = result.hierarchy().pending() {
            pending!(SourceCallerClausesPending::Hierarchy(why));
        }
        if result.header().role() != DefinitionHeaderRole::Function {
            pending!(SourceCallerClausesPending::UnsupportedOwner);
        }
        // A body-local item does not inherit its surrounding function's generic
        // assumptions. Its local source owner still requires a separate proof;
        // do not flatten that scope into a module or a parent text fallback.
        match result.header().source_parent() {
            SourceHeaderParent::Lexical(scope) if scope.owner.is_none() => {}
            _ => pending!(SourceCallerClausesPending::ParentClosure),
        }
        if source.parent_definition().is_some() {
            pending!(SourceCallerClausesPending::ParentClosure);
        }
        match DeclarationParameters::prepare(result.header().declaration()) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) if parameters.count() == 0 => {}
            Ok(DeclaredParameterPreparation::Ready(_)) => {
                pending!(SourceCallerClausesPending::NonemptyParameters)
            }
            Ok(DeclaredParameterPreparation::Pending(why)) => {
                pending!(SourceCallerClausesPending::Parameters(why))
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
        let node = match parsed.syntax_at(context.range, |node| ast::Fn::can_cast(node.kind())) {
            Ok(Some(node)) => ast::Fn::cast(node).expect("selected actual function"),
            Ok(None) => pending!(SourceCallerClausesPending::Header),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if node
            .generic_param_list()
            .is_some_and(|list| list.generic_params().next().is_some())
        {
            return QueryPoll::Ready(Err(fault(
                "zero caller parameter count differs from syntax",
            )));
        }
        if node
            .where_clause()
            .is_some_and(|clause| clause.predicates().next().is_some())
            || !source.recipes().expect("ready source hierarchy").is_empty()
        {
            pending!(SourceCallerClausesPending::WrittenClauses);
        }
        if recipe.attributes.iter().any(|attribute| {
            attribute.name == "unstable_feature_bound"
                || attribute.class == AttributeClass::TransformUnsupported
        }) {
            pending!(SourceCallerClausesPending::AttributeEffects);
        }
        // A retained ordinary Fn excludes synthetic delegated/RPITIT/opaque
        // declarations. No Fn signature or executable body is prepared here.
        let Some(root) = seal.impl_catalog.as_ref().map(|catalog| catalog.root()) else {
            pending!(SourceCallerClausesPending::Configuration);
        };
        if root.outline.crate_key != context.scope.crate_key
            || root.outline.parse.profile != parse.profile
        {
            pending!(SourceCallerClausesPending::Configuration);
        }
        let configuration = ready!(RetainedQuery::capture(
            db,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(root)))
        ));
        let QueryValue::SolverConfiguration(config) = configuration.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("caller configuration query differs")));
        };
        let gce = config.features().contains("generic_const_exprs");
        result.configuration = Some(configuration);
        if gce {
            pending!(SourceCallerClausesPending::GenericConstExpressions);
        }
        let semantics = ready!(RetainedQuery::capture(
            db,
            QueryKey::SourceDefinitionSemantics(Box::new(key.clone()))
        ));
        let QueryValue::SourceDefinitionSemantics(semantic) = semantics.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("caller scalar semantic query differs")));
        };
        if semantic.key() != key || semantic.header().fingerprint() != result.header().fingerprint()
        {
            return QueryPoll::Ready(Err(fault("caller scalar source receipt differs")));
        }
        let nonconditional = semantic.boolean(DefinitionBooleanQuery::IsConditionallyConst)
            == SourceSemantic::Known(false);
        result.semantics = Some(semantics);
        if !nonconditional {
            pending!(SourceCallerClausesPending::ConditionalConstness);
        }
        // Pinned clauses_of has no remaining clause-producing branch for this
        // zero-parameter ordinary Fn; inferred_outlives_of(Fn) is independently
        // empty. ParamEnv adds neither trait-default RPITIT nor const effects.
        QueryPoll::Ready(Ok(result))
    }
    /// Optional observation of the retained explicit source hierarchy only.
    /// This method does not promote it to complete caller assumptions.
    pub(super) fn encode_written<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<EncodedSourceClauseHierarchy<'cx>, SourceDeclarationClausesPending> {
        self.hierarchy().encode(cx, configuration)
    }
    /// Construct only the genuine empty caller branch. A source descriptor is
    /// checked for identity, not accepted as a generic application certificate.
    /// Encoding performs no DB query, normalization or solver callback.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
        caller: &CallerSourceContext,
    ) -> Result<model::ParamEnv<'cx, prepared::PreparedProvider<'cx>>, SourceCallerClausesPending>
    {
        if let Some(why) = self.pending {
            return Err(why);
        }
        if caller.body_definition != self.key.definition {
            return Err(SourceCallerClausesPending::CallerIdentity);
        }
        if caller.generic_definition.is_some()
            || caller.signature.is_some()
            || caller.application.is_some()
            || !caller.substitutions.is_empty()
        {
            return Err(SourceCallerClausesPending::CallerApplication);
        }
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else {
            unreachable!("actual source catalog")
        };
        if cx.services().source_world() != catalog.seal.fingerprint()
            || cx.services().source_configuration() != configuration.fingerprint()
            || self
                .configuration()
                .is_none_or(|actual| actual.fingerprint() != configuration.fingerprint())
        {
            return Err(SourceCallerClausesPending::SourceWorld);
        }
        Ok(model::ParamEnv(cx.clauses(cx.list(Vec::new()))))
    }
}
impl StableEncode for PreparedSourceCallerClauses {
    const DOMAIN: &'static str = "lrr/v1/source-caller-clauses";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.header());
        e.record(self.hierarchy());
        e.option(&self.semantics, |e, root| {
            let QueryValue::SourceDefinitionSemantics(plan) = root.value.as_ref() else {
                unreachable!()
            };
            e.record(plan.as_ref());
        });
        e.option(&self.configuration(), |e, plan| e.record(*plan));
        // Exact detailed causes remain encoded in retained header/hierarchy/
        // semantic roots; distinguish independent local branch holds as well.
        e.option(&self.pending, |e, why| {
            e.u32(match why {
                SourceCallerClausesPending::Header => 0,
                SourceCallerClausesPending::Hierarchy(_) => 1,
                SourceCallerClausesPending::UnsupportedOwner => 2,
                SourceCallerClausesPending::ParentClosure => 3,
                SourceCallerClausesPending::Parameters(_) => 4,
                SourceCallerClausesPending::NonemptyParameters => 5,
                SourceCallerClausesPending::WrittenClauses => 6,
                SourceCallerClausesPending::AttributeEffects => 7,
                SourceCallerClausesPending::Configuration => 8,
                SourceCallerClausesPending::GenericConstExpressions => 9,
                SourceCallerClausesPending::ConditionalConstness => 10,
                SourceCallerClausesPending::CallerIdentity => 11,
                SourceCallerClausesPending::CallerApplication => 12,
                SourceCallerClausesPending::SourceWorld => 13,
            })
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-CALLER-CLAUSES", message.into())
}
