use super::key::QueryKey;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::{
        outline::{HeaderRecipe, Outline, ScopeBlocker},
        parse::ParsedFile,
    },
};
use std::sync::Arc;

/// Borrowed immutable preparation allocations. These are private accounting
/// locators, never a source/name authority or part of a stable key.
#[derive(Clone, Copy)]
pub(crate) enum SourceArtifactRef<'a> {
    SourceSyntaxInput(&'a Arc<super::memo::source_syntax_inputs::CompletedSourceSyntaxInput>),
    CompletedSourceSyntax(&'a Arc<crate::expand::source_discovery::CompletedSourceSyntax>),
    PreparedBodySyntax(&'a Arc<super::memo::source_syntax_inputs::PreparedBodySyntax>),
    LocalPlan(&'a Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>),
    BodyEnvironment(&'a Arc<super::BodyEnvironmentInput>),
    CompletedEnvironment(&'a Arc<crate::expand::source_discovery::CompletedSourceEnvironment>),
    Pattern(&'a Arc<crate::syntax::pattern_tree::PatternTree>),
    StringLiteral(&'a Arc<crate::syntax::string_literal::PreparedStringLiteral>),
}
impl<'a> SourceArtifactRef<'a> {
    pub(crate) fn allocation(self) -> usize {
        match self {
            Self::SourceSyntaxInput(value) => Arc::as_ptr(value) as usize,
            Self::CompletedSourceSyntax(value) => Arc::as_ptr(value) as usize,
            Self::PreparedBodySyntax(value) => Arc::as_ptr(value) as usize,
            Self::LocalPlan(value) => Arc::as_ptr(value) as usize,
            Self::BodyEnvironment(value) => Arc::as_ptr(value) as usize,
            Self::CompletedEnvironment(value) => Arc::as_ptr(value) as usize,
            Self::Pattern(value) => Arc::as_ptr(value) as usize,
            Self::StringLiteral(value) => Arc::as_ptr(value) as usize,
        }
    }
    pub(crate) fn strong_count(self) -> usize {
        match self {
            Self::SourceSyntaxInput(value) => Arc::strong_count(value),
            Self::CompletedSourceSyntax(value) => Arc::strong_count(value),
            Self::PreparedBodySyntax(value) => Arc::strong_count(value),
            Self::LocalPlan(value) => Arc::strong_count(value),
            Self::BodyEnvironment(value) => Arc::strong_count(value),
            Self::CompletedEnvironment(value) => Arc::strong_count(value),
            Self::Pattern(value) => Arc::strong_count(value),
            Self::StringLiteral(value) => Arc::strong_count(value),
        }
    }
    pub(crate) fn metadata_bytes(self) -> Result<usize, Diagnostic> {
        let bytes = match self {
            Self::SourceSyntaxInput(value) => Some(value.exclusive_metadata_bytes()),
            Self::CompletedSourceSyntax(value) => value
                .metadata_bytes()
                .checked_add(2 * std::mem::size_of::<usize>()),
            Self::PreparedBodySyntax(value) => value
                .metadata_bytes()
                .checked_add(2 * std::mem::size_of::<usize>()),
            Self::LocalPlan(value) => value
                .exclusive_metadata_bytes()
                .checked_add(2 * std::mem::size_of::<usize>()),
            // The same environment can also be owned transitively by a local
            // plan. The shared root traversal charges its allocation once.
            Self::BodyEnvironment(_) | Self::CompletedEnvironment(_) => Some(0),
            Self::Pattern(value) => value
                .metadata_bytes()
                .checked_add(2 * std::mem::size_of::<usize>()),
            Self::StringLiteral(value) => value.metadata_bytes(),
        };
        bytes.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "immutable source artifact accounting overflow".into(),
            )
        })
    }
    pub(crate) fn admission_bytes(self) -> Result<usize, Diagnostic> {
        match self {
            Self::CompletedEnvironment(environment) => environment.retained_metadata_bytes(),
            _ => self.metadata_bytes(),
        }
    }
    pub(crate) fn index(self) -> Option<&'a Arc<crate::syntax::body_index::BodyIndex>> {
        match self {
            Self::Pattern(_)
            | Self::StringLiteral(_)
            | Self::SourceSyntaxInput(_)
            | Self::CompletedSourceSyntax(_)
            | Self::PreparedBodySyntax(_)
            | Self::LocalPlan(_)
            | Self::BodyEnvironment(_)
            | Self::CompletedEnvironment(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bucket {
    pub candidates: Vec<Digest>,
    pub blockers: Vec<ScopeBlocker>,
}
#[derive(Debug)]
pub enum QueryValue {
    TraitDeclaration(Box<crate::frontend::trait_declarations::TraitDeclaration>),
    DeclarationGenerics(Box<crate::frontend::declaration_generics::DeclarationGenerics>),
    DeclarationGenericTypes(Box<crate::frontend::declaration_generic_types::DeclarationGenericTypes>),
    PreparedScopeImplHeaders(Box<crate::traits::index::PreparedScopeImplHeaders>),
    ImplHeaderSyntax(Box<crate::traits::header_syntax::ImplHeaderSyntax>),
    ImplHeadTypes(Box<crate::frontend::impl_head_types::PreparedImplHeadTypes>),
    ImplTraitIdentity(Box<crate::frontend::impl_trait_identity::PreparedImplTraitIdentity>),
    SolverConfiguration(Box<crate::traits::upstream::configuration::PreparedSolverConfiguration>),
    AdtSourceFields(Box<crate::traits::upstream::source_adt_fields::PreparedAdtSourceFields>),
    DefinitionHeaderPlan(Box<crate::traits::upstream::source_definition_headers::PreparedDefinitionHeader>),
    TraitAssociatedTypes(Box<crate::frontend::trait_associated_types::PreparedTraitAssociatedTypes>),
    DeclarationSourcePredicates(Box<crate::frontend::declaration_source_predicates::PreparedDeclarationSourcePredicates>),
    SourceVarianceQueries(Box<crate::traits::upstream::source_variance_queries::PreparedSourceVarianceQueries>),
    SourceDeclarationClauses(Box<crate::traits::upstream::source_declaration_clauses::PreparedSourceDeclarationClauses>),
    SourceDeclarationClauseHierarchy(Box<crate::traits::upstream::source_declaration_clauses::PreparedSourceDeclarationClauseHierarchy>),
    SourceItemBounds(Box<crate::frontend::source_item_bounds::PreparedSourceItemBounds>),
    SourceLangIdentity(Box<crate::traits::upstream::source_lang_identity::PreparedSourceLangIdentity>),
    ImplSourceTypes(Box<crate::traits::upstream::source_terms::PreparedImplSourceTypes>),
    ImplSourcePredicates(Box<crate::traits::upstream::source_predicates::PreparedImplSourcePredicates>),
    ImplTraitCatalog(Box<crate::traits::trait_catalog::PreparedImplTraitCatalog>),
    ImplPredicates(Box<crate::frontend::impl_predicates::PreparedImplPredicates>),
    PreparedBodySyntax(Arc<super::memo::source_syntax_inputs::PreparedBodySyntax>),
    PreparedExplicitTypeArguments(
        Box<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
    ),
    EnteredCall(Box<crate::frontend::entered_call_goals::EnteredCallGoal>),
    SourcePatternName(Box<crate::frontend::pattern_names::PreparedSourcePatternName>),
    PreparedBodyTypeApplication(Box<crate::frontend::body_type_applications::PreparedBodyTypeApplication>),
    DeclaredReturnGoal(Box<crate::frontend::source_return_goals::ReturnGoalObservation>),
    PreparedReturnPlan(Box<crate::frontend::source_return_goals::PreparedReturnPlan>),
    EnteredType(Box<crate::frontend::entered_type_goals::EnteredTypeGoal>),
    PreparedTypePlan(Box<crate::frontend::entered_type_goals::PreparedTypePlan>),
    PreparedBindingPlan(Box<crate::frontend::prepared_binding_plan::PreparedBindingPlan>),
    BindingConstraints(Box<crate::frontend::binding_constraints::PreparedBindingConstraints>),
    EnteredValue(Box<crate::frontend::source_value_goals::EnteredValueGoal>),
    LexicalValue(Box<crate::frontend::source_value_goals::EnteredValueGoal>),
    DeclaredParameterGoal(Box<crate::frontend::source_parameter_goals::ParameterGoalObservation>),
    SourceConstraints(Box<crate::syntax::source_constraints::SourceConstraintPlan>),
    QualifiedBodyType(Box<crate::frontend::qualified_type_names::QualifiedTypeObservation>),
    AssociatedItemQualifier(Box<crate::frontend::associated_constant_applications::PreparedAssociatedItemQualifier>),
    ArrayRepeatPlan(Box<crate::frontend::array_repeats::ArrayRepeatPlan>),
    PreparedArrayRepeat(Box<crate::frontend::array_repeats::PreparedArrayRepeat>),
    DeclarationOwnerTemplate(Box<crate::frontend::declaration_owner_templates::PreparedDeclarationOwnerTemplate>),
    AssociatedEquations(Box<crate::frontend::associated_equations::PreparedAssociatedEquations>),
    NominalMember(Box<crate::frontend::nominal_member_goals::PreparedNominalMember>),
    BoundRecordConstructor(Box<crate::frontend::prepared_constructors::PreparedRecordConstructor>),
    SourceLocalPlan(Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>),
    MacroDefinitionModule(Arc<crate::frontend::macro_definition_modules::PreparedDefinitionModule>),
    CurrentModuleName(Arc<crate::frontend::current_module_names::CurrentModuleNameObservation>),
    SealedName {
        key: Box<super::key::SealedNameKey>,
        lookup: crate::frontend::names::NameLookup,
    },
    DiscoverySeal(Box<super::seals::Seal>),
    ScopeDeclarationInventory {
        key: super::seals::SealKey,
        inventory: Arc<crate::frontend::impl_index::ImplHeaderIndex>,
    },
    BodyStructure(Arc<crate::syntax::body_index::BodyIndex>),
    Thunk(Box<crate::ir::recipe::VerifiedRecipe>),
    ExpansionInput(Box<crate::expand::record::ExpansionDescriptor>),
    ParseFile(Arc<ParsedFile>),
    Outline(Arc<Outline>),
    ScopeBucket(Bucket),
    Signature(Arc<HeaderRecipe>),
    FunctionSignature(Box<crate::syntax::signatures::FunctionSignature>),
    ReferenceHeader(Arc<crate::syntax::closed_reference_signatures::ReferenceHeaderPlan>),
    PatternPlan(Arc<crate::syntax::pattern_plan::PatternSourcePlan>),
    PatternAccess(Arc<crate::frontend::pattern_access::PreparedPatternAccess>),
    TypeAlias(Box<crate::syntax::type_aliases::TypeAliasRecipe>),
    TypeSchema(Box<crate::frontend::types::NominalSchema>),
    ConstructorPlan(Box<crate::syntax::constructors::ConstructorPlan>),
    Constant(Box<crate::frontend::constants::query_value::ConstantQueryValue>),
    ConstantFailure(Box<crate::frontend::constants::query_failure::ConstantQueryFailure>),
    ConstantMacroDemand(Box<crate::frontend::constants::query_macro_demand::ConstantMacroDemand>),
    Error(Diagnostic),
}
impl QueryValue {
    pub(crate) fn source_roots(&self) -> impl Iterator<Item = SourceArtifactRef<'_>> {
        let recipe = match self {
            Self::Thunk(recipe) => Some(recipe),
            _ => None,
        };
        let local = match self {
            Self::ArrayRepeatPlan(value) => value.parent().map(SourceArtifactRef::LocalPlan),
            Self::PreparedArrayRepeat(value) => value.plan().parent().map(SourceArtifactRef::LocalPlan),
            Self::DeclarationOwnerTemplate(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::AssociatedItemQualifier(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::NominalMember(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::PreparedBodySyntax(value) => Some(SourceArtifactRef::PreparedBodySyntax(value)),
            Self::PreparedExplicitTypeArguments(value) => {
                Some(SourceArtifactRef::LocalPlan(value.retained_plan()))
            }
            Self::EnteredCall(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::EnteredType(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::PreparedBodyTypeApplication(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::PreparedTypePlan(value) => {
                Some(SourceArtifactRef::LocalPlan(value.retained_plan()))
            }
            Self::EnteredValue(value) | Self::LexicalValue(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::SourceLocalPlan(plan) => Some(SourceArtifactRef::LocalPlan(plan)),
            Self::MacroDefinitionModule(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::QualifiedBodyType(value) => {
                Some(SourceArtifactRef::LocalPlan(value.retained_plan()))
            }
            _ => None,
        };
        local
            .into_iter()
            .chain(
                recipe
                    .into_iter()
                    .filter_map(|recipe| recipe.source_call())
                    .map(|plan| SourceArtifactRef::LocalPlan(plan.retained_plan())),
            )
            .chain(self.pattern_roots().map(SourceArtifactRef::Pattern))
            .chain(
                recipe
                    .into_iter()
                    .flat_map(|recipe| recipe.string_literal_roots())
                    .map(SourceArtifactRef::StringLiteral),
            )
    }
    /// Actual handles, without cloning or allocation. Shared allocation
    /// charging and owner deduplication belong to the coordinator.
    pub(crate) fn pattern_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<crate::syntax::pattern_tree::PatternTree>> {
        let plan = match self {
            Self::PatternPlan(plan) => Some(plan),
            _ => None,
        };
        let recipe = match self {
            Self::Thunk(recipe) => Some(recipe),
            _ => None,
        };
        plan.into_iter()
            .flat_map(|plan| plan.trees())
            .chain(recipe.into_iter().flat_map(|recipe| recipe.pattern_roots()))
    }
    pub fn parse_bytes(&self) -> usize {
        match self {
            Self::ParseFile(v) => v.estimated_bytes(),
            _ => 0,
        }
    }
    pub fn matches(&self, key: &QueryKey) -> bool {
        if let (Self::PreparedScopeImplHeaders(value), QueryKey::PreparedScopeImplHeaders(key)) = (self, key) { return value.key() == key; }
        if let (Self::ImplHeaderSyntax(value), QueryKey::ImplHeaderSyntax(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::ImplHeadTypes(value), QueryKey::ImplHeadTypes(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::ImplTraitIdentity(value), QueryKey::ImplTraitIdentity(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::SolverConfiguration(value), QueryKey::SolverConfiguration(key)) = (self, key) { return value.matches_key(key); }
        if let (Self::DeclarationSourcePredicates(value), QueryKey::DeclarationSourcePredicates(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::SourceVarianceQueries(value), QueryKey::SourceVarianceQueries(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::SourceDeclarationClauses(value), QueryKey::SourceDeclarationClauses(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::SourceDeclarationClauseHierarchy(value), QueryKey::SourceDeclarationClauseHierarchy(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::SourceItemBounds(value), QueryKey::SourceItemBounds(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::TraitAssociatedTypes(value), QueryKey::TraitAssociatedTypes(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::DefinitionHeaderPlan(value), QueryKey::DefinitionHeaderPlan(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::SourceLangIdentity(value), QueryKey::SourceLangIdentity(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::AdtSourceFields(value), QueryKey::AdtSourceFields(key)) = (self,key) { return value.key()==key.as_ref(); }
        if let (Self::ImplSourceTypes(value), QueryKey::ImplSourceTypes(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::SourcePatternName(value), QueryKey::SourcePatternName(key)) = (self,key) { return value.key()==key; }
        if let (Self::PreparedBodyTypeApplication(value), QueryKey::PreparedBodyTypeApplication(key)) = (self,key) { return value.key()==key; }
        if let (Self::ImplSourcePredicates(value), QueryKey::ImplSourcePredicates(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::ImplTraitCatalog(value), QueryKey::ImplTraitCatalog(key)) = (self, key) { return value.key() == key; }
        if let (Self::ImplPredicates(value), QueryKey::ImplPredicates(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::DeclarationGenericTypes(value), QueryKey::DeclarationGenericTypes(key)) = (self, key) { return value.key() == key.as_ref(); }
        if let (Self::DeclarationGenerics(value), QueryKey::DeclarationGenerics(key)) = (self, key) { return value.definition() == *key; }
        if let (Self::TraitDeclaration(value), QueryKey::TraitDeclaration(key)) = (self, key) { return value.definition() == *key; }
        if let (Self::ArrayRepeatPlan(value), QueryKey::ArrayRepeatPlan(key)) = (self, key) { return value.key() == key; }
        if let (Self::PreparedArrayRepeat(value), QueryKey::PreparedArrayRepeat(key)) = (self, key) { return value.plan().key() == key; }
        if let (Self::DeclarationOwnerTemplate(value), QueryKey::DeclarationOwnerTemplate(key)) = (self, key) { return value.key() == key; }
        if let (Self::AssociatedItemQualifier(value), QueryKey::AssociatedItemQualifier(key)) = (self, key) { return value.key() == key; }
        if let (Self::AssociatedEquations(value), QueryKey::AssociatedEquations(key)) = (self, key)
        {
            return value.key() == key.as_ref();
        }
        if let (Self::NominalMember(value), QueryKey::NominalMember(key)) = (self, key) {
            return value.key() == key.as_ref();
        }
        if let (Self::PreparedBodySyntax(value), QueryKey::PreparedBodySyntax(key)) = (self, key) {
            return value.key() == key.as_ref();
        }
        if let (
            Self::PreparedExplicitTypeArguments(value),
            QueryKey::PreparedExplicitTypeArguments(key),
        ) = (self, key)
        {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::EnteredCall(value), QueryKey::EnteredCall(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::PreparedReturnPlan(value), QueryKey::PreparedReturnPlan(key)) = (self, key) {
            return value.definition() == *key;
        }
        if let (Self::DeclaredReturnGoal(value), QueryKey::DeclaredReturnGoal(key)) = (self, key) {
            return value.matches(*key);
        }
        if let (Self::EnteredType(value), QueryKey::EnteredType(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::PreparedTypePlan(value), QueryKey::PreparedTypePlan(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::PreparedBindingPlan(value), QueryKey::PreparedBindingPlan(key)) = (self, key)
        {
            return value.constraints().key() == key && key.phase == 0;
        }
        if let (Self::BindingConstraints(value), QueryKey::BindingConstraints(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::EnteredValue(value), QueryKey::EnteredValue(key))
            | (Self::LexicalValue(value), QueryKey::LexicalValue(key)) = (self, key) {
            return value.body() == &key.body && value.node().0 == key.node && key.phase == 0;
        }
        if let (
            Self::DeclaredParameterGoal(value),
            QueryKey::DeclaredParameterGoal {
                definition,
                ordinal,
            },
        ) = (self, key)
        {
            return value.matches_local(*definition, *ordinal);
        }
        if let (
            Self::DeclaredParameterGoal(value),
            QueryKey::ContextParameterGoal { body, definition, ordinal },
        ) = (self, key) {
            return value.matches_context(body, *definition, *ordinal);
        }
        if let (Self::QualifiedBodyType(value), QueryKey::QualifiedBodyType(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::BoundRecordConstructor(value), QueryKey::BoundRecordConstructor(key)) =
            (self, key)
        {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::MacroDefinitionModule(value), QueryKey::MacroDefinitionModule { body, context }) = (self, key) { return value.matches(body, *context); }
        if let (Self::SourceLocalPlan(plan), QueryKey::SourceLocalPlan(body)) = (self, key) {
            return plan.context().source().index().body() == body;
        }
        if let (Self::CurrentModuleName(value), QueryKey::CurrentModuleName(request)) = (self, key)
        {
            return value.matches_request(request);
        }
        if let (Self::SealedName { key: actual, .. }, QueryKey::SealedName(expected)) = (self, key)
        {
            return actual == expected;
        }
        if let (
            Self::ScopeDeclarationInventory { key: actual, .. },
            QueryKey::ScopeDeclarationInventory(expected),
        ) = (self, key)
        {
            return actual == expected;
        }
        if let (Self::DiscoverySeal(seal), QueryKey::DiscoverySeal(key)) = (self, key) {
            return seal.key() == *key;
        }
        if let (Self::TypeAlias(value), QueryKey::TypeAlias(key)) = (self, key) {
            return value.definition == *key;
        }
        if let (Self::TypeSchema(value), QueryKey::TypeSchema(key)) = (self, key) {
            return value.definition == *key;
        }
        if let (Self::ConstructorPlan(value), QueryKey::ConstructorPlan(key)) = (self, key) {
            return value.origin().body_key == key.body
                && value.node().0 == key.node
                && key.phase == 0;
        }
        if let (Self::FunctionSignature(value), QueryKey::FunctionSignature(key)) = (self, key) {
            return value.definition == *key;
        }
        if let (Self::ReferenceHeader(value), QueryKey::ReferenceHeader(key)) = (self, key) {
            return value.definition() == *key;
        }
        if let (Self::PatternAccess(value), QueryKey::PatternAccess(key)) = (self, key) {
            return value.key() == key.as_ref();
        }
        if let (Self::PatternPlan(value), QueryKey::PatternPlan(key)) = (self, key) {
            return value.owner() == key && key.phase == 0;
        }
        if let (Self::BodyStructure(value), QueryKey::BodyStructure(key)) = (self, key) {
            return value.body() == key;
        }
        if let (Self::SourceConstraints(value), QueryKey::SourceConstraints(key)) = (self, key) {
            return value.body() == key;
        }
        if let (Self::Thunk(value), QueryKey::Thunk(key)) = (self, key) {
            return &value.recipe().key == key;
        }
        if let (
            Self::ExpansionInput(descriptor),
            QueryKey::ExpansionInput(key) | QueryKey::CurrentExpansionInput(key),
        ) = (self, key)
        {
            return descriptor.key == **key;
        }
        if let (Self::Constant(value), QueryKey::Constant(key)) = (self, key) {
            return value.key() == key;
        }
        if let (Self::ConstantFailure(value), QueryKey::Constant(key)) = (self, key) {
            return value.key() == key;
        }
        if let (Self::ConstantMacroDemand(value), QueryKey::Constant(key)) = (self, key) {
            return value.key() == key;
        }
        matches!(self, Self::Error(_))
            || matches!(
                (self, key),
                (Self::ParseFile(_), QueryKey::ParseFile(_))
                    | (Self::Outline(_), QueryKey::Outline(_))
                    | (Self::ScopeBucket(_), QueryKey::ScopeBucket(_))
                    | (Self::Signature(_), QueryKey::Signature(_))
            )
    }
    pub fn fingerprint(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/query-value");
        match self {
            Self::PreparedBodySyntax(value) => {
                e.u32(48);
                e.digest(value.fingerprint());
            }
            Self::PreparedExplicitTypeArguments(value) => {
                e.u32(47);
                e.digest(value.fingerprint());
            }
            Self::PreparedReturnPlan(value) => {
                e.u32(46);
                e.record(value.as_ref());
            }
            Self::EnteredCall(value) => {
                e.u32(44);
                e.record(value.as_ref());
            }
            Self::DeclaredReturnGoal(value) => {
                e.u32(45);
                e.record(value.as_ref());
            }
            Self::EnteredType(value) => {
                e.u32(42);
                e.record(value.as_ref());
            }
            Self::PreparedTypePlan(value) => {
                e.u32(43);
                e.record(value.as_ref());
            }
            Self::PreparedBindingPlan(value) => {
                e.u32(41);
                e.record(value.as_ref());
            }
            Self::BindingConstraints(value) => {
                e.u32(40);
                e.record(value.as_ref());
            }
            Self::LexicalValue(value) => { e.u32(62); e.digest(value.fingerprint()); }
            Self::EnteredValue(value) => {
                e.u32(39);
                e.digest(value.fingerprint());
            }
            Self::DeclaredParameterGoal(value) => {
                e.u32(38);
                e.record(value.as_ref());
            }
            Self::SourceConstraints(value) => {
                e.u32(37);
                e.record(value.as_ref());
            }
            Self::QualifiedBodyType(value) => {
                e.u32(36);
                e.digest(value.fingerprint());
            }
            Self::ArrayRepeatPlan(value) => { e.u32(54); e.record(value.as_ref()); }
            Self::PreparedArrayRepeat(value) => { e.u32(55); e.record(value.as_ref()); }
            Self::DeclarationOwnerTemplate(value) => { e.u32(53); e.record(value.as_ref()); }
            Self::PreparedScopeImplHeaders(value) => { e.u32(56); e.digest(value.fingerprint()); }
            Self::ImplHeaderSyntax(value) => { e.u32(57); e.record(value.as_ref()); }
            Self::ImplHeadTypes(value) => { e.u32(58); e.record(value.as_ref()); }
            Self::ImplTraitIdentity(value) => { e.u32(65); e.record(value.as_ref()); }
            Self::SolverConfiguration(value) => { e.u32(67); e.record(value.as_ref()); }
            Self::AdtSourceFields(value) => { e.u32(74); e.record(value.as_ref()); }
            Self::SourceLangIdentity(value) => { e.u32(76); e.record(value.as_ref()); }
            Self::DefinitionHeaderPlan(value) => { e.u32(75); e.record(value.as_ref()); }
            Self::TraitAssociatedTypes(value) => { e.u32(77); e.record(value.as_ref()); }
            Self::DeclarationSourcePredicates(value) => { e.u32(78); e.record(value.as_ref()); }
            Self::SourceVarianceQueries(value) => { e.u32(79); e.record(value.as_ref()); }
            Self::SourceDeclarationClauses(value) => { e.u32(80); e.record(value.as_ref()); }
            Self::SourceDeclarationClauseHierarchy(value) => { e.u32(81); e.record(value.as_ref()); }
            Self::SourceItemBounds(value) => { e.u32(82); e.record(value.as_ref()); }
            Self::ImplSourceTypes(value) => { e.u32(68); e.record(value.as_ref()); }
            Self::SourcePatternName(value) => { e.u32(72); e.record(value.as_ref()); }
            Self::PreparedBodyTypeApplication(value) => { e.u32(73); e.record(value.as_ref()); }
            Self::ImplSourcePredicates(value) => { e.u32(71); e.record(value.as_ref()); }
            Self::ImplTraitCatalog(value) => { e.u32(66); e.record(value.as_ref()); }
            Self::ImplPredicates(value) => { e.u32(59); e.record(value.as_ref()); }
            Self::TraitDeclaration(value) => { e.u32(60); e.record(value.as_ref()); }
            Self::DeclarationGenerics(value) => { e.u32(69); e.record(value.as_ref()); }
            Self::DeclarationGenericTypes(value) => { e.u32(70); e.record(value.as_ref()); }
            Self::AssociatedItemQualifier(value) => { e.u32(51); e.record(value.as_ref()); }
            Self::AssociatedEquations(value) => {
                e.u32(50);
                e.record(value.as_ref());
            }
            Self::NominalMember(value) => {
                e.u32(49);
                e.record(value.as_ref());
            }
            Self::BoundRecordConstructor(value) => {
                e.u32(35);
                e.record(value.as_ref());
            }
            Self::MacroDefinitionModule(value) => { e.u32(61); e.record(value.as_ref()); }
            Self::SourceLocalPlan(plan) => {
                e.u32(34);
                e.digest(plan.source_identity());
                e.option(&plan.environment(), |e, environment| {
                    e.digest(environment.identity())
                });
            }
            Self::CurrentModuleName(value) => {
                e.u32(33);
                e.digest(value.semantic_identity());
            }
            Self::SealedName { key, lookup } => {
                e.u32(25);
                e.record(key.as_ref());
                e.record(lookup);
            }
            Self::ScopeDeclarationInventory { key, inventory } => {
                e.u32(26);
                e.record(key);
                e.digest(inventory.fingerprint());
            }
            Self::DiscoverySeal(seal) => {
                e.u32(24);
                e.record(seal.as_ref());
            }
            Self::BodyStructure(value) => {
                e.u32(6);
                e.record(value.as_ref());
            }
            Self::Thunk(value) => {
                e.u32(7);
                e.record(value.recipe());
                e.option(&value.prepared_inputs_fingerprint(), |e, fingerprint| {
                    e.digest(*fingerprint)
                });
                // Preserve encoding of recipes without this new retained facet;
                // a present facet has an explicit domain tag and cold identity.
                if let Some(plan) = value.source_call() {
                    e.u32(47);
                    e.digest(plan.fingerprint());
                }
            }
            Self::ExpansionInput(descriptor) => {
                e.u32(20);
                e.record(descriptor.as_ref());
            }
            Self::ParseFile(v) => {
                e.u32(1);
                e.record(&v.key());
                e.sequence(&v.errors, |e, d| {
                    e.u32(d.range.start);
                    e.u32(d.range.end);
                    e.string(&d.message);
                });
            }
            Self::Outline(v) => {
                e.u32(2);
                e.record(v.as_ref());
            }
            Self::ScopeBucket(v) => {
                e.u32(3);
                e.sequence(&v.candidates, |e, d| e.digest(*d));
                e.sequence(&v.blockers, |e, b| e.record(b));
            }
            Self::Signature(v) => {
                e.u32(5);
                e.record(v.as_ref());
            }
            Self::FunctionSignature(v) => {
                e.u32(8);
                e.record(v.as_ref());
            }
            Self::ReferenceHeader(v) => {
                e.u32(31);
                e.record(v.as_ref());
            }
            Self::PatternAccess(v) => { e.u32(63); e.record(v.as_ref()); }
            Self::PatternPlan(v) => {
                e.u32(32);
                e.digest(v.fingerprint());
            }
            Self::TypeAlias(v) => {
                e.u32(9);
                e.record(v.as_ref());
            }
            Self::TypeSchema(v) => {
                e.u32(27);
                e.record(v.as_ref());
            }
            Self::ConstructorPlan(v) => {
                e.u32(28);
                e.record(v.as_ref());
            }
            Self::Constant(v) => {
                e.u32(10);
                e.record(v.as_ref());
            }
            Self::ConstantFailure(value) => {
                e.u32(29);
                e.record(value.as_ref());
            }
            Self::ConstantMacroDemand(value) => {
                e.u32(30);
                e.record(value.as_ref());
            }
            Self::Error(v) => {
                e.u32(255);
                e.string(v.code);
                e.string(&v.message);
                e.option(&v.macro_origin, |e, origin| e.record(origin.as_ref()));
                e.option(&v.generated_origin, |e, origin| e.record(origin.as_ref()));
                e.option(&v.guest_stack, |e, stack| e.record(stack.as_ref()));
                e.option(&v.origin_attachment_failure, |e, code| e.string(code));
                if let Some(change) = &v.prepared_input_change {
                    // Preserve existing error encodings when evidence is absent.
                    e.string("lrr/v1/diagnostic-prepared-input-change");
                    e.record(&change.key);
                    e.digest(change.expected);
                    e.digest(change.actual);
                }
            }
        };
        e.finish()
    }
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::PreparedBodySyntax(_) => 0, // Actual result allocation is retained by source artifact pins.
            Self::PreparedExplicitTypeArguments(value) => value.metadata_bytes(),
            Self::EnteredCall(value) => value.metadata_bytes(),
            Self::DeclaredReturnGoal(value) => value.estimated_bytes(),
            Self::PreparedReturnPlan(value) => value.metadata_bytes(),
            Self::EnteredType(value) => value.metadata_bytes(),
            Self::PreparedTypePlan(value) => value.metadata_bytes(),
            Self::PreparedBindingPlan(value) => value.metadata_bytes(),
            Self::BindingConstraints(value) => value.metadata_bytes(),
            Self::EnteredValue(value) | Self::LexicalValue(value) => value.estimated_bytes(),
            Self::DeclaredParameterGoal(value) => value.estimated_bytes(),
            Self::SourceConstraints(value) => value.estimated_bytes(),
            Self::CurrentModuleName(value) => value
                .exclusive_metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::SealedName { key, lookup } => std::mem::size_of::<super::key::SealedNameKey>()
                .saturating_add(key.bucket.scope.heap_bytes())
                .saturating_add(key.bucket.name.capacity())
                .saturating_add(lookup.metadata_bytes().unwrap_or(usize::MAX)),
            Self::DiscoverySeal(seal) => seal.metadata_bytes().unwrap_or(usize::MAX),
            Self::ScopeDeclarationInventory { inventory, .. } => inventory.metadata_bytes(),
            Self::ExpansionInput(descriptor) => descriptor.estimated_bytes(),
            Self::BodyStructure(value) => value
                .estimated_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::Thunk(value) => value
                .exclusive_metadata_bytes()
                .checked_sub(value.string_literal_admission_bytes())
                .unwrap_or(usize::MAX),
            Self::ParseFile(v) => v.estimated_bytes(),
            Self::Outline(v) => v.estimated_bytes(),
            Self::Signature(v) => v.estimated_bytes(),
            Self::FunctionSignature(v) => v.estimated_bytes(),
            // Artifact retirement owns and charges the actual inner plan.
            Self::SourceLocalPlan(_) | Self::MacroDefinitionModule(_) => 0,
            Self::ReferenceHeader(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::SourcePatternName(_) | Self::PreparedBodyTypeApplication(_) => 0,
            Self::PatternAccess(_) => 0, // E006: no retained-admission resource accounting.
            Self::PatternPlan(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::TypeAlias(v) => v.estimated_bytes(),
            Self::TypeSchema(v) => v.estimated_bytes(),
            Self::ConstructorPlan(v) => v.estimated_bytes().unwrap_or(usize::MAX),
            // E006: no new retained-admission memory accounting.
            Self::ArrayRepeatPlan(_) | Self::PreparedArrayRepeat(_) | Self::DeclarationOwnerTemplate(_)
            | Self::PreparedScopeImplHeaders(_) | Self::ImplHeaderSyntax(_) | Self::ImplHeadTypes(_) | Self::ImplPredicates(_) | Self::TraitDeclaration(_) | Self::DeclarationGenerics(_) | Self::DeclarationGenericTypes(_) | Self::ImplTraitIdentity(_) | Self::ImplTraitCatalog(_) | Self::SolverConfiguration(_) | Self::AdtSourceFields(_) | Self::DefinitionHeaderPlan(_) | Self::TraitAssociatedTypes(_) | Self::DeclarationSourcePredicates(_) | Self::SourceVarianceQueries(_) | Self::SourceDeclarationClauses(_) | Self::SourceDeclarationClauseHierarchy(_) | Self::SourceItemBounds(_) | Self::SourceLangIdentity(_) | Self::ImplSourceTypes(_) | Self::ImplSourcePredicates(_) => 0,
            Self::AssociatedItemQualifier(value) => value.metadata_bytes().unwrap_or(usize::MAX),
            Self::AssociatedEquations(value) => value.metadata_bytes().unwrap_or(usize::MAX),
            Self::NominalMember(value) => value.metadata_bytes().unwrap_or(usize::MAX),
            Self::BoundRecordConstructor(v) => v.metadata_bytes().unwrap_or(usize::MAX),
            Self::QualifiedBodyType(value) => value.estimated_bytes(),
            Self::ScopeBucket(v) => {
                v.candidates.capacity() * std::mem::size_of::<Digest>()
                    + v.blockers.capacity() * std::mem::size_of::<ScopeBlocker>()
            }
            Self::Error(v) => v.heap_bytes().unwrap_or(usize::MAX),
            Self::Constant(value) => value.metadata_bytes().unwrap_or(usize::MAX),
            Self::ConstantFailure(value) => value.metadata_bytes().unwrap_or(usize::MAX),
            Self::ConstantMacroDemand(value) => value.metadata_bytes().unwrap_or(usize::MAX),
        })
    }
}
