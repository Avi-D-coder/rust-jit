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
    LocalPlan(&'a Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>),
    BodyEnvironment(&'a Arc<super::BodyEnvironmentInput>),
    CompletedEnvironment(&'a Arc<crate::expand::source_discovery::CompletedSourceEnvironment>),
    Pattern(&'a Arc<crate::syntax::pattern_product::PatternProductPlan>),
    StringLiteral(&'a Arc<crate::syntax::string_literal::PreparedStringLiteral>),
}
impl<'a> SourceArtifactRef<'a> {
    pub(crate) fn allocation(self) -> usize {
        match self {
            Self::LocalPlan(value) => Arc::as_ptr(value) as usize,
            Self::BodyEnvironment(value) => Arc::as_ptr(value) as usize,
            Self::CompletedEnvironment(value) => Arc::as_ptr(value) as usize,
            Self::Pattern(value) => Arc::as_ptr(value) as usize,
            Self::StringLiteral(value) => Arc::as_ptr(value) as usize,
        }
    }
    pub(crate) fn strong_count(self) -> usize {
        match self {
            Self::LocalPlan(value) => Arc::strong_count(value),
            Self::BodyEnvironment(value) => Arc::strong_count(value),
            Self::CompletedEnvironment(value) => Arc::strong_count(value),
            Self::Pattern(value) => Arc::strong_count(value),
            Self::StringLiteral(value) => Arc::strong_count(value),
        }
    }
    pub(crate) fn metadata_bytes(self) -> Result<usize, Diagnostic> {
        let bytes = match self {
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
            Self::Pattern(value) => Some(value.index()),
            Self::StringLiteral(_)
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
#[derive(Debug, Clone)]
pub enum QueryValue {
    PreparedBindingPlan(Box<crate::frontend::prepared_binding_plan::PreparedBindingPlan>),
    BindingConstraints(Box<crate::frontend::binding_constraints::PreparedBindingConstraints>),
    EnteredValue(Box<crate::frontend::source_value_goals::EnteredValueGoal>),
    DeclaredParameterGoal(Box<crate::frontend::source_parameter_goals::ParameterGoalObservation>),
    SourceConstraints(Box<crate::syntax::source_constraints::SourceConstraintPlan>),
    QualifiedBodyType(Box<crate::frontend::qualified_type_names::QualifiedTypeObservation>),
    BoundRecordConstructor(Box<crate::frontend::prepared_constructors::PreparedRecordConstructor>),
    SourceLocalPlan(Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>),
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
            Self::EnteredValue(value) => Some(SourceArtifactRef::LocalPlan(value.retained_plan())),
            Self::SourceLocalPlan(plan) => Some(SourceArtifactRef::LocalPlan(plan)),
            Self::QualifiedBodyType(value) => {
                Some(SourceArtifactRef::LocalPlan(value.retained_plan()))
            }
            _ => None,
        };
        local
            .into_iter()
            .chain(self.product_roots().map(SourceArtifactRef::Pattern))
            .chain(
                recipe
                    .into_iter()
                    .flat_map(|recipe| recipe.string_literal_roots())
                    .map(SourceArtifactRef::StringLiteral),
            )
    }
    /// Actual handles, without cloning or allocation. Shared allocation
    /// charging and owner deduplication belong to the coordinator.
    pub(crate) fn product_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<crate::syntax::pattern_product::PatternProductPlan>> {
        let plan = match self {
            Self::PatternPlan(plan) => Some(plan),
            _ => None,
        };
        let recipe = match self {
            Self::Thunk(recipe) => Some(recipe),
            _ => None,
        };
        plan.into_iter()
            .flat_map(|plan| plan.products())
            .chain(recipe.into_iter().flat_map(|recipe| recipe.product_roots()))
    }
    pub fn parse_bytes(&self) -> usize {
        match self {
            Self::ParseFile(v) => v.estimated_bytes(),
            _ => 0,
        }
    }
    pub fn matches(&self, key: &QueryKey) -> bool {
        if let (Self::PreparedBindingPlan(value), QueryKey::PreparedBindingPlan(key)) = (self, key) {
            return value.constraints().key() == key && key.phase == 0;
        }
        if let (Self::BindingConstraints(value), QueryKey::BindingConstraints(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::EnteredValue(value), QueryKey::EnteredValue(key)) = (self, key) {
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
            return value.matches(*definition, *ordinal);
        }
        if let (Self::QualifiedBodyType(value), QueryKey::QualifiedBodyType(key)) = (self, key) {
            return value.key() == key && key.phase == 0;
        }
        if let (Self::BoundRecordConstructor(value), QueryKey::BoundRecordConstructor(key)) =
            (self, key)
        {
            return value.key() == key && key.phase == 0;
        }
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
            Self::PreparedBindingPlan(value) => { e.u32(41); e.record(value.as_ref()); }
            Self::BindingConstraints(value) => { e.u32(40); e.record(value.as_ref()); }
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
            Self::BoundRecordConstructor(value) => {
                e.u32(35);
                e.record(value.as_ref());
            }
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
            }
        };
        e.finish()
    }
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::PreparedBindingPlan(value) => value.metadata_bytes(),
            Self::BindingConstraints(value) => value.metadata_bytes(),
            Self::EnteredValue(value) => value.estimated_bytes(),
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
            Self::SourceLocalPlan(_) => 0,
            Self::ReferenceHeader(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::PatternPlan(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::TypeAlias(v) => v.estimated_bytes(),
            Self::TypeSchema(v) => v.estimated_bytes(),
            Self::ConstructorPlan(v) => v.estimated_bytes().unwrap_or(usize::MAX),
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
