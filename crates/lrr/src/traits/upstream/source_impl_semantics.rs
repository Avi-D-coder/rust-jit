//! Source-authenticated impl query inputs, prepared before upstream solver entry.
//! A retained trait head is not applicability or an elaboration-closure proof.
use super::{
    configuration::{PreparedSolverConfiguration, SolverConfigurationKey},
    parameters::{DeclarationParameters, DeclaredParameterPreparation},
    prepared::Cx,
    source_definition_headers::{DefinitionHeaderKey, DefinitionHeaderRole},
    source_definition_semantics::{
        DefinitionBooleanQuery, PreparedSourceDefinitionSemantics, SourceSemantic,
    },
    source_generics::SourceGenericLayouts,
    source_terms::{EncodedImplTypes, PreparedImplSourceTypes, SourceTypePending},
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
    frontend::impl_trait_identity::{ImplTraitIdentityResult, PreparedImplTraitIdentity},
    syntax::outline::AttributeClass,
    traits::{header_syntax::ImplHeaderSyntaxKey, predicate::SourcePredicateRecipe},
};
use std::sync::Arc;

#[path = "source_impl_clauses.rs"]
mod full_clauses;
use full_clauses::PreparedSourceImplClauses;
pub(crate) use full_clauses::SourceImplClausesPending;
#[cfg(test)]
pub(crate) use full_clauses::{
    ImplClauseKindObservation, ImplClauseRowObservation, ImplClausesObservation,
    check_actual_full_impl_clauses,
};

pub type SourceImplSemanticsKey = ImplHeaderSyntaxKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ImplSemantic<T> {
    Known(T),
    /// The pinned query requires a trait impl, but this is an inherent impl.
    NotApplicable,
    Pending(ImplSemanticsPending),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub enum ImplSemanticsPending {
    Identity = 0,
    CanonicalOwner = 1,
    Header = 2,
    Constness = 3,
    /// No actual transitive elaborate-query closure has been admitted yet.
    ElaborationClosure = 4,
    ClauseParameterLayout = 5,
    ClauseParameters = 6,
    ClauseDefaultImpl = 7,
    ClauseWrittenPredicates = 8,
    ClauseAttributeEffects = 9,
    ClauseConfiguration = 10,
    ClauseGenericConstExpressions = 11,
    ClauseSourceRows = 12,
    ClauseEmptyWhere = 13,
}
/// Only the authenticated null branch of compiler `clauses_of(Impl)`.
/// This says nothing about super/implied/item/self/const-bound query families.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct EmptyImplClauses(());
impl EmptyImplClauses {
    pub(crate) fn parent(self) -> Option<Digest> {
        None
    }
    pub(crate) fn recipes(self) -> &'static [SourcePredicateRecipe] {
        &[]
    }
}
/// Written header evidence; `constant_keyword` is not `impl_is_const`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct SourceImplModifiers {
    pub(crate) negative: bool,
    pub(crate) constant_keyword: bool,
    pub(crate) unsafe_keyword: bool,
    pub(crate) default_keyword: bool,
}
/// Conditional/transitive reads of the pinned Filter::All elaborator.
/// Describing these families does not supply their keys, values or closure.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum ImplOutlivesQueryFamily {
    SizedLanguageIdentity = 0,
    ExplicitImpliedClauses = 1,
    ExplicitImpliedConstBounds = 2,
    AliasVariances = 3,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ImplSuperOutlivesDemand {
    pub(crate) implementation: Digest,
    pub(crate) trait_definition: Digest,
}
impl ImplSuperOutlivesDemand {
    pub(crate) fn query_families(&self) -> &'static [ImplOutlivesQueryFamily] {
        use ImplOutlivesQueryFamily::*;
        &[
            SizedLanguageIdentity,
            ExplicitImpliedClauses,
            ExplicitImpliedConstBounds,
            AliasVariances,
        ]
    }
}

#[derive(Debug)]
pub struct PreparedSourceImplSemantics {
    key: SourceImplSemanticsKey,
    identity: Arc<QueryValue>,
    semantics: Option<Arc<QueryValue>>,
    types: Option<Arc<QueryValue>>,
    environment: Option<Arc<QueryValue>>,
    configuration: Option<Arc<QueryValue>>,
    clauses: ImplSemantic<()>,
    full_clauses: Option<PreparedSourceImplClauses>,
    implementation: Option<Digest>,
    modifiers: Option<SourceImplModifiers>,
    /// Capture/authentication only; other query-specific holds are independent.
    pending: Option<ImplSemanticsPending>,
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
impl PreparedSourceImplSemantics {
    pub fn key(&self) -> &SourceImplSemanticsKey {
        &self.key
    }
    pub fn pending(&self) -> Option<ImplSemanticsPending> {
        self.pending
    }
    pub(crate) fn identity(&self) -> &PreparedImplTraitIdentity {
        let QueryValue::ImplTraitIdentity(identity) = self.identity.as_ref() else {
            unreachable!("captured actual impl identity")
        };
        identity
    }
    pub(crate) fn source_types(&self) -> Option<&PreparedImplSourceTypes> {
        self.types.as_ref().map(|root| {
            let QueryValue::ImplSourceTypes(types) = root.as_ref() else {
                unreachable!("captured actual impl source Types")
            };
            types.as_ref()
        })
    }
    pub(crate) fn source_semantics(&self) -> Option<&PreparedSourceDefinitionSemantics> {
        self.semantics.as_ref().map(|root| {
            let QueryValue::SourceDefinitionSemantics(semantics) = root.as_ref() else {
                unreachable!("captured impl scalar semantics")
            };
            semantics.as_ref()
        })
    }
    pub(crate) fn impl_definition(&self) -> ImplSemantic<Digest> {
        if let Some(why) = self.pending {
            return ImplSemantic::Pending(why);
        }
        match self.implementation {
            Some(id) => ImplSemantic::Known(id),
            None => ImplSemantic::Pending(ImplSemanticsPending::CanonicalOwner),
        }
    }
    pub(crate) fn trait_definition(&self) -> ImplSemantic<Digest> {
        if let Some(why) = self.pending {
            return ImplSemantic::Pending(why);
        }
        match self.identity().result() {
            ImplTraitIdentityResult::Trait(id) => ImplSemantic::Known(id),
            ImplTraitIdentityResult::Inherent => ImplSemantic::NotApplicable,
            ImplTraitIdentityResult::Pending => {
                ImplSemantic::Pending(ImplSemanticsPending::Identity)
            }
        }
    }
    pub(crate) fn modifiers(&self) -> ImplSemantic<SourceImplModifiers> {
        if let Some(why) = self.pending {
            return ImplSemantic::Pending(why);
        }
        match self.modifiers {
            Some(value) => ImplSemantic::Known(value),
            None => ImplSemantic::Pending(ImplSemanticsPending::Header),
        }
    }
    pub(crate) fn complete_clauses_of(&self) -> ImplSemantic<EmptyImplClauses> {
        match self.clauses {
            ImplSemantic::Known(()) => match self
                .full_clauses
                .as_ref()
                .map(PreparedSourceImplClauses::pending)
            {
                Some(None) => ImplSemantic::Known(EmptyImplClauses(())),
                Some(Some(SourceImplClausesPending::EmptyWhere)) => {
                    ImplSemantic::Pending(ImplSemanticsPending::ClauseEmptyWhere)
                }
                _ => ImplSemantic::Pending(ImplSemanticsPending::ClauseSourceRows),
            },
            ImplSemantic::NotApplicable => ImplSemantic::NotApplicable,
            ImplSemantic::Pending(why) => ImplSemantic::Pending(why),
        }
    }
    /// Actual retained own clauses and constant-Type inputs. The old null-branch
    /// API remains independent, so its consumers cannot publish an empty list
    /// for this producer's nonempty result.
    pub(super) fn full_clause_bindings(
        &self,
    ) -> Result<&super::source_bindings::SourceBindings, SourceImplClausesPending> {
        self.full_clauses
            .as_ref()
            .ok_or_else(|| {
                SourceImplClausesPending::Gate(match self.clauses {
                    ImplSemantic::Pending(why) => why,
                    _ => ImplSemanticsPending::Header,
                })
            })?
            .bindings()
    }
    pub(super) fn encode_complete_clauses<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<
        (
            super::identity::DefinitionId,
            Vec<(
                super::model::Clause<'cx, super::prepared::PreparedProvider<'cx>>,
                super::model::Span,
            )>,
        ),
        SourceImplClausesPending,
    > {
        if let Some(why) = self.pending {
            return Err(SourceImplClausesPending::Gate(why));
        }
        self.full_clauses
            .as_ref()
            .ok_or_else(|| {
                SourceImplClausesPending::Gate(match self.clauses {
                    ImplSemantic::Pending(why) => why,
                    _ => ImplSemanticsPending::Header,
                })
            })?
            .encode(cx, configuration)
    }
    fn trait_modifier<T>(&self, select: impl FnOnce(SourceImplModifiers) -> T) -> ImplSemantic<T> {
        match self.trait_definition() {
            ImplSemantic::Known(_) => match self.modifiers() {
                ImplSemantic::Known(value) => ImplSemantic::Known(select(value)),
                ImplSemantic::Pending(why) => ImplSemantic::Pending(why),
                ImplSemantic::NotApplicable => unreachable!("impl source modifiers have a domain"),
            },
            ImplSemantic::NotApplicable => ImplSemantic::NotApplicable,
            ImplSemantic::Pending(why) => ImplSemantic::Pending(why),
        }
    }
    /// collect.rs::polarity_of_impl: negative syntax maps to Negative, else Positive.
    pub(crate) fn polarity(&self) -> ImplSemantic<rustc_type_ir::ImplPolarity> {
        self.trait_modifier(|value| {
            if value.negative {
                rustc_type_ir::ImplPolarity::Negative
            } else {
                rustc_type_ir::ImplPolarity::Positive
            }
        })
    }
    /// ty_utils::defaultness only accepts trait impls (not inherent impls).
    pub(crate) fn is_default(&self) -> ImplSemantic<bool> {
        self.trait_modifier(|value| value.default_keyword)
    }
    /// collect.rs::impl_trait_header carries the actual trait header safety.
    pub(crate) fn is_unsafe(&self) -> ImplSemantic<bool> {
        self.trait_modifier(|value| value.unsafe_keyword)
    }
    /// impl_interner::impl_is_const delegates to is_conditionally_const.
    pub(crate) fn is_const(&self) -> ImplSemantic<bool> {
        match self.trait_definition() {
            ImplSemantic::Known(_) => {}
            ImplSemantic::NotApplicable => return ImplSemantic::NotApplicable,
            ImplSemantic::Pending(why) => return ImplSemantic::Pending(why),
        }
        let Some(root) = &self.semantics else {
            return ImplSemantic::Pending(ImplSemanticsPending::Constness);
        };
        let QueryValue::SourceDefinitionSemantics(semantics) = root.as_ref() else {
            unreachable!("captured impl scalar semantic query")
        };
        match semantics.boolean(DefinitionBooleanQuery::IsConditionallyConst) {
            SourceSemantic::Known(value) => ImplSemantic::Known(value),
            // This query is valid for the authenticated trait impl. A missing
            // scalar producer cannot become a false answer or invalid domain.
            SourceSemantic::Pending(_) | SourceSemantic::NotApplicable => {
                ImplSemantic::Pending(ImplSemanticsPending::Constness)
            }
        }
    }
    /// outlives/mod.rs::inferred_outlives_of: DefKind::Impl takes `_ => &[]`.
    /// The empty source transport requires neither field inference nor elaborate.
    pub(crate) fn inferred_outlives_of(&self) -> ImplSemantic<&[SourcePredicateRecipe]> {
        match self.impl_definition() {
            ImplSemantic::Known(_) => ImplSemantic::Known(&[]),
            ImplSemantic::Pending(why) => ImplSemantic::Pending(why),
            ImplSemantic::NotApplicable => unreachable!("authenticated impl definition domain"),
        }
    }
    pub(crate) fn super_outlives_demand(&self) -> ImplSemantic<ImplSuperOutlivesDemand> {
        match (self.impl_definition(), self.trait_definition()) {
            (ImplSemantic::Known(implementation), ImplSemantic::Known(trait_definition)) => {
                ImplSemantic::Known(ImplSuperOutlivesDemand {
                    implementation,
                    trait_definition,
                })
            }
            (_, ImplSemantic::NotApplicable) => ImplSemantic::NotApplicable,
            (ImplSemantic::Pending(why), _) | (_, ImplSemantic::Pending(why)) => {
                ImplSemantic::Pending(why)
            }
            _ => unreachable!("impl trait outlives domain"),
        }
    }
    /// Q86 records only the demanded query family. Q89 retains its actual
    /// callback inputs; PreparedProvider::prepare_impl_super_outlives runs the
    /// pinned elaborator and filter in their admitted preparation context.
    /// This source-only descriptor does not contain that derived query value.
    pub(crate) fn impl_super_outlives(&self) -> ImplSemantic<&[SourcePredicateRecipe]> {
        match self.super_outlives_demand() {
            ImplSemantic::Known(_) => {
                ImplSemantic::Pending(ImplSemanticsPending::ElaborationClosure)
            }
            ImplSemantic::NotApplicable => ImplSemantic::NotApplicable,
            ImplSemantic::Pending(why) => ImplSemantic::Pending(why),
        }
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        if self.pending.is_some() {
            return Err(fault("impl source identity remains pending"));
        }
        let types = self
            .source_types()
            .ok_or_else(|| fault("impl typed source remains pending"))?;
        if types.pending().is_some() {
            return Err(fault("impl typed source remains pending"));
        }
        SourceGenericLayouts::from_bindings(Arc::clone(types.catalog_input()), [types.bindings()])
    }
    /// Pure encoding from the retained common source plan; no DB/callback work.
    pub(super) fn encode_head<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<EncodedImplTypes<'cx>, SourceTypePending> {
        if self.pending.is_some() {
            return Err(SourceTypePending::SourceDefinition);
        }
        let types = self.source_types().ok_or(SourceTypePending::HeadTypes)?;
        let encoded = types.encode(cx, configuration)?;
        let actual = encoded
            .trait_arguments
            .as_ref()
            .map(|reference| reference.definition.source());
        let expected = match self.identity().result() {
            ImplTraitIdentityResult::Trait(id) => Some(id),
            ImplTraitIdentityResult::Inherent => None,
            ImplTraitIdentityResult::Pending => return Err(SourceTypePending::SourceDefinition),
        };
        if actual != expected {
            return Err(SourceTypePending::SourceDefinition);
        }
        Ok(encoded)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceImplSemanticsKey) -> QueryPoll<Self> {
        let identity = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ImplTraitIdentity(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            identity,
            semantics: None,
            types: None,
            environment: None,
            configuration: None,
            clauses: ImplSemantic::Pending(ImplSemanticsPending::Header),
            full_clauses: None,
            implementation: None,
            modifiers: None,
            pending: Some(ImplSemanticsPending::Identity),
        };
        if result.identity().key() != key {
            return QueryPoll::Ready(Err(fault("impl identity memo key differs")));
        }
        if result.identity().result() == ImplTraitIdentityResult::Pending {
            return QueryPoll::Ready(Ok(result));
        }
        let Some(implementation) = result
            .identity()
            .syntax()
            .canonical_owner()
            .map(|owner| owner.identity())
        else {
            result.pending = Some(ImplSemanticsPending::CanonicalOwner);
            return QueryPoll::Ready(Ok(result));
        };
        let header_key = DefinitionHeaderKey {
            inventory: key.inventory.clone(),
            definition: implementation,
        };
        let semantics = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceDefinitionSemantics(Box::new(header_key.clone()))
        ))
        .value;
        let QueryValue::SourceDefinitionSemantics(plan) = semantics.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl scalar semantic query variant differs")));
        };
        let header = plan.header();
        if plan.key() != &header_key
            || header.role() != DefinitionHeaderRole::Impl
            || header.impl_membership() != Some(result.identity().result())
        {
            return QueryPoll::Ready(Err(fault("impl scalar header identity differs")));
        }
        result.semantics = Some(Arc::clone(&semantics));
        if header.pending().is_some() {
            result.pending = Some(ImplSemanticsPending::Header);
            return QueryPoll::Ready(Ok(result));
        }
        let Some((parse, recipe, context, _)) = header.declaration().source_parts() else {
            result.pending = Some(ImplSemanticsPending::Header);
            return QueryPoll::Ready(Ok(result));
        };
        if !result.identity().matches_metadata(parse, recipe, context) {
            return QueryPoll::Ready(Err(fault("impl actual source receipt differs")));
        }
        let (negative, constant_keyword, unsafe_keyword, default_keyword) =
            result.identity().syntax().modifiers();
        result.implementation = Some(implementation);
        result.modifiers = Some(SourceImplModifiers {
            negative,
            constant_keyword,
            unsafe_keyword,
            default_keyword,
        });
        result.pending = None;
        let types = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ImplSourceTypes(Box::new(key.clone()))
        ))
        .value;
        if !matches!(types.as_ref(), QueryValue::ImplSourceTypes(types) if types.key()==key) {
            return QueryPoll::Ready(Err(fault("impl typed source memo key differs")));
        }
        result.types = Some(types);
        // Actual Impl
        // excludes RPITIT, delegated fn, Trait and OpaqueTy branches in pinned
        // collect/clauses_of.rs::gather_explicit_clauses_of. Impl generics_of
        // has no generic parent; inferred_outlives_of(Impl) is independently [].
        let parameter_count = match DeclarationParameters::prepare(header.declaration()) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) => parameters.count(),
            Ok(DeclaredParameterPreparation::Pending(_)) => {
                result.clauses = ImplSemantic::Pending(ImplSemanticsPending::ClauseParameterLayout);
                return QueryPoll::Ready(Ok(result));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let syntax = result.identity().syntax();
        if parameter_count == 0 && !syntax.generics().is_empty() {
            return QueryPoll::Ready(Err(fault(
                "zero impl parameter count differs from written generics",
            )));
        }
        if default_keyword {
            result.clauses = ImplSemantic::Pending(ImplSemanticsPending::ClauseDefaultImpl);
            return QueryPoll::Ready(Ok(result));
        }
        let has_written_predicates = !syntax.predicates().is_empty();
        if recipe.attributes.iter().any(|attribute| {
            attribute.name == "unstable_feature_bound"
                || attribute.class == AttributeClass::TransformUnsupported
        }) {
            result.clauses = ImplSemantic::Pending(ImplSemanticsPending::ClauseAttributeEffects);
            return QueryPoll::Ready(Ok(result));
        }
        result.clauses = ImplSemantic::Pending(ImplSemanticsPending::ClauseConfiguration);
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        result.environment = Some(Arc::clone(&environment));
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl clauses source inventory differs")));
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
            return QueryPoll::Ready(Err(fault("impl clauses configuration differs")));
        };
        if configuration_plan
            .features()
            .contains("generic_const_exprs")
        {
            result.clauses =
                ImplSemantic::Pending(ImplSemanticsPending::ClauseGenericConstExpressions);
            result.configuration = Some(configuration);
            return QueryPoll::Ready(Ok(result));
        }
        result.clauses = if parameter_count != 0 {
            ImplSemantic::Pending(ImplSemanticsPending::ClauseParameters)
        } else if has_written_predicates {
            ImplSemantic::Pending(ImplSemanticsPending::ClauseWrittenPredicates)
        } else {
            ImplSemantic::Known(())
        };
        result.configuration = Some(configuration);
        let full = ready!(PreparedSourceImplClauses::prepare(
            db,
            crate::frontend::declaration_source_predicates::DeclarationSourcePredicatesKey {
                inventory: key.inventory.clone(),
                definition: implementation,
            }
        ));
        if !full
            .source()
            .declaration()
            .matches_metadata(parse, recipe, context)
        {
            return QueryPoll::Ready(Err(fault("full impl clause source receipt differs")));
        }
        result.full_clauses = Some(full);
        QueryPoll::Ready(Ok(result))
    }
}
impl StableEncode for PreparedSourceImplSemantics {
    const DOMAIN: &'static str = "lrr/v2/source-impl-semantics";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.identity());
        e.option(&self.semantics, |e, root| {
            let QueryValue::SourceDefinitionSemantics(plan) = root.as_ref() else {
                unreachable!("captured impl semantic root")
            };
            e.record(plan.as_ref());
        });
        e.option(&self.types, |e, root| {
            let QueryValue::ImplSourceTypes(types) = root.as_ref() else {
                unreachable!("captured impl source Type root")
            };
            e.record(types.as_ref());
        });
        e.option(&self.environment, |e, root| {
            let QueryValue::PreparedScopeImplHeaders(environment) = root.as_ref() else {
                unreachable!("captured impl clause environment")
            };
            e.digest(environment.fingerprint());
        });
        e.option(&self.configuration, |e, root| {
            let QueryValue::SolverConfiguration(configuration) = root.as_ref() else {
                unreachable!("captured impl clause configuration")
            };
            e.record(configuration.as_ref());
        });
        e.option(&self.full_clauses, |e, full| e.record(full));
        match self.clauses {
            ImplSemantic::Known(()) => e.u32(0),
            ImplSemantic::NotApplicable => e.u32(1),
            ImplSemantic::Pending(why) => {
                e.u32(2);
                e.u32(why as u32);
            }
        }
        e.option(&self.implementation, |e, id| e.digest(*id));
        e.option(&self.modifiers, |e, value| {
            e.bool(value.negative);
            e.bool(value.constant_keyword);
            e.bool(value.unsafe_keyword);
            e.bool(value.default_keyword);
        });
        e.option(&self.pending, |e, why| e.u32(*why as u32));
    }
}
pub(crate) struct CurrentSourceImplSemantics {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceImplSemantics {
    pub(crate) fn capture(db: &mut Database, key: &SourceImplSemanticsKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceImplSemantics(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(),
            QueryValue::SourceImplSemantics(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("impl semantic memo identity differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceImplSemantics {
        let QueryValue::SourceImplSemantics(plan) = self.value.as_ref() else {
            unreachable!("captured impl semantic memo")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-IMPL-SEMANTICS", message.into())
}
