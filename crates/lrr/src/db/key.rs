use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    source::manifest::Namespace,
    syntax::{outline::ScopeKey, parse::ParseKey},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct OutlineKey {
    pub parse: ParseKey,
    pub crate_key: Digest,
    pub module_path: Vec<String>,
    pub cfg: Digest,
}
impl StableEncode for OutlineKey {
    const DOMAIN: &'static str = "lrr/v1/outline-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.parse);
        e.digest(self.crate_key);
        e.sequence(&self.module_path, |e, part| e.string(part));
        e.digest(self.cfg);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BucketKey {
    pub scope: ScopeKey,
    pub namespace: Namespace,
    pub name: String,
}
impl StableEncode for BucketKey {
    const DOMAIN: &'static str = "lrr/v2/namespace-bucket";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.scope);
        e.u32(self.namespace.tag());
        e.string(&self.name);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct SealedNameKey {
    pub seal: super::seals::SealKey,
    pub bucket: BucketKey,
}
impl StableEncode for SealedNameKey {
    const DOMAIN: &'static str = "lrr/v2/sealed-name-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.seal);
        e.record(&self.bucket);
    }
}
/// Exact source/module request. A key is a demand, never name authority.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CurrentModuleNameKey {
    pub source: OutlineKey,
    pub path: String,
    pub bucket: BucketKey,
}
impl CurrentModuleNameKey {
    pub(crate) fn heap_bytes(&self) -> usize {
        self.path
            .capacity()
            .saturating_add(self.source.parse.profile.semantics.capacity())
            .saturating_add(
                self.source
                    .module_path
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                self.source
                    .module_path
                    .iter()
                    .fold(0usize, |n, part| n.saturating_add(part.capacity())),
            )
            .saturating_add(self.bucket.scope.heap_bytes())
            .saturating_add(self.bucket.name.capacity())
    }
}
impl StableEncode for CurrentModuleNameKey {
    const DOMAIN: &'static str = "lrr/v1/current-module-name-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.string(&self.path);
        e.record(&self.bucket);
    }
}
/// Exact retained structural preparation demand; a key grants no entry authority.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PreparedBodySyntaxKey {
    pub source: OutlineKey,
    pub path: String,
    pub epoch: u64,
    pub plan: Digest,
    pub definition: Digest,
}
impl PreparedBodySyntaxKey {
    pub(crate) fn heap_bytes(&self) -> usize {
        self.path
            .capacity()
            .saturating_add(self.source.parse.profile.semantics.capacity())
            .saturating_add(
                self.source
                    .module_path
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                self.source
                    .module_path
                    .iter()
                    .fold(0usize, |n, s| n.saturating_add(s.capacity())),
            )
    }
    pub(crate) fn selection(
        &self,
    ) -> Result<crate::source::changes::SourceSelection, crate::core::diagnostic::Diagnostic> {
        let invalid = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-QUERY-REQUEST",
                "body syntax key is not canonical or bounded".into(),
            )
        };
        if self.epoch == 0 {
            return Err(invalid());
        }
        self.source.parse.profile.validate_source()?;
        let source = crate::source::changes::SourceSelection {
            path: crate::source::snapshot::SourcePath::new(self.path.clone())?,
            outline: self.source.clone(),
        };
        if source.canonical()? != source {
            return Err(invalid());
        }
        Ok(source)
    }
}
impl StableEncode for PreparedBodySyntaxKey {
    const DOMAIN: &'static str = "lrr/v1/prepared-body-syntax-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.string(&self.path);
        e.u64(self.epoch);
        e.digest(self.plan);
        e.digest(self.definition);
    }
}
pub use crate::frontend::constants::query_identity::ConstQueryIdentity as ConstantKey;
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum QueryKey {
    TraitDeclaration(Digest),
    DeclarationGenerics(Digest),
    DeclarationGenericTypes(Box<crate::frontend::declaration_generic_types::DeclarationGenericTypesKey>),
    PreparedBodySyntax(Box<PreparedBodySyntaxKey>),
    ArrayRepeatPlan(crate::source::manifest::PcKey),
    PreparedArrayRepeat(crate::source::manifest::PcKey),
    AssociatedItemQualifier(crate::source::manifest::PcKey),
    DeclarationOwnerTemplate(crate::frontend::declaration_owner_templates::DeclarationOwnerTemplateKey),
    AssociatedEquations(Box<crate::frontend::associated_equations::AssociatedEquationsKey>),
    NominalMember(Box<crate::frontend::nominal_member_goals::NominalMemberKey>),
    PreparedExplicitTypeArguments(crate::source::manifest::PcKey),
    EnteredCall(crate::source::manifest::PcKey),
    SourcePatternName(crate::source::manifest::PcKey),
    PreparedBodyTypeApplication(crate::source::manifest::PcKey),
    DeclaredReturnGoal(Digest),
    PreparedReturnPlan(Digest),
    EnteredType(crate::source::manifest::PcKey),
    PreparedTypePlan(crate::source::manifest::PcKey),
    PreparedBindingPlan(crate::source::manifest::PcKey),
    BindingConstraints(crate::source::manifest::PcKey),
    EnteredValue(crate::source::manifest::PcKey),
    LexicalValue(crate::source::manifest::PcKey),
    DeclaredParameterGoal { definition: Digest, ordinal: u32 },
    ContextParameterGoal { body: crate::source::manifest::BodyKey, definition: Digest, ordinal: u32 },
    SourceConstraints(crate::source::manifest::BodyKey),
    QualifiedBodyType(crate::source::manifest::PcKey),
    BoundRecordConstructor(crate::source::manifest::PcKey),
    SourceLocalPlan(crate::source::manifest::BodyKey),
    MacroDefinitionModule { body: crate::source::manifest::BodyKey, context: crate::syntax::token_wire::Key },
    CurrentModuleName(Box<CurrentModuleNameKey>),
    SealedName(Box<SealedNameKey>),
    DiscoverySeal(super::seals::SealKey),
    ScopeDeclarationInventory(super::seals::SealKey),
    PreparedScopeImplHeaders(super::seals::SealKey),
    ImplHeaderSyntax(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    ImplHeadTypes(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    ImplTraitIdentity(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    SolverConfiguration(Box<crate::traits::upstream::configuration::SolverConfigurationKey>),
    AdtSourceFields(Box<crate::traits::upstream::source_adt_fields::AdtSourceFieldsKey>),
    DefinitionHeaderPlan(Box<crate::traits::upstream::source_definition_headers::DefinitionHeaderKey>),
    TraitAssociatedTypes(Box<crate::frontend::trait_associated_types::TraitAssociatedTypesKey>),
    DeclarationSourcePredicates(Box<crate::frontend::declaration_source_predicates::DeclarationSourcePredicatesKey>),
    SourceVarianceQueries(Box<crate::traits::upstream::source_variance_queries::SourceVarianceQueriesKey>),
    SourceLangIdentity(Box<crate::traits::upstream::source_lang_identity::SourceLangIdentityKey>),
    ImplSourceTypes(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    ImplSourcePredicates(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    ImplTraitCatalog(super::seals::SealKey),
    ImplPredicates(Box<crate::traits::header_syntax::ImplHeaderSyntaxKey>),
    BodyStructure(crate::source::manifest::BodyKey),
    Thunk(crate::source::manifest::PcKey),
    CurrentExpansionInput(Box<crate::expand::identity::ExpansionKey>),
    ExpansionInput(Box<crate::expand::identity::ExpansionKey>),
    ParseFile(ParseKey),
    Outline(OutlineKey),
    ScopeBucket(BucketKey),
    Signature(Digest),
    FunctionSignature(Digest),
    ReferenceHeader(Digest),
    PatternPlan(crate::source::manifest::PcKey),
    PatternAccess(Box<crate::frontend::pattern_access::PatternAccessKey>),
    TypeAlias(Digest),
    TypeSchema(Digest),
    ConstructorPlan(crate::source::manifest::PcKey),
    Constant(ConstantKey),
}
impl QueryKey {
    pub fn heap_bytes(&self) -> usize {
        match self {
            Self::AssociatedEquations(key) => std::mem::size_of_val(key.as_ref())
                .saturating_add(key.heap_bytes().unwrap_or(usize::MAX)),
            Self::NominalMember(key) => std::mem::size_of_val(key.as_ref())
                .saturating_add(key.heap_bytes().unwrap_or(usize::MAX)),
            Self::PreparedBodySyntax(key) => {
                std::mem::size_of::<PreparedBodySyntaxKey>().saturating_add(key.heap_bytes())
            }
            Self::CurrentModuleName(key) => {
                std::mem::size_of::<CurrentModuleNameKey>().saturating_add(key.heap_bytes())
            }
            Self::SealedName(key) => {
                std::mem::size_of::<SealedNameKey>()
                    + key.bucket.scope.heap_bytes()
                    + key.bucket.name.capacity()
            }
            Self::DiscoverySeal(_) | Self::ScopeDeclarationInventory(_)
            | Self::PreparedScopeImplHeaders(_) | Self::ImplHeaderSyntax(_) | Self::ImplHeadTypes(_) | Self::ImplPredicates(_) | Self::TraitDeclaration(_) | Self::DeclarationGenerics(_) | Self::DeclarationGenericTypes(_) | Self::ImplTraitIdentity(_) | Self::ImplTraitCatalog(_) | Self::SolverConfiguration(_) | Self::AdtSourceFields(_) | Self::DefinitionHeaderPlan(_) | Self::TraitAssociatedTypes(_) | Self::DeclarationSourcePredicates(_) | Self::SourceVarianceQueries(_) | Self::SourceLangIdentity(_) | Self::ImplSourceTypes(_) | Self::ImplSourcePredicates(_) => 0,
            Self::ExpansionInput(k) | Self::CurrentExpansionInput(k) => {
                std::mem::size_of_val(k.as_ref()) + crate::expand::record::key_heap_bytes(k)
            }
            Self::ParseFile(k) => k.profile.semantics.capacity(),
            Self::Outline(k) => {
                k.parse.profile.semantics.capacity()
                    + k.module_path.capacity() * std::mem::size_of::<String>()
                    + k.module_path.iter().map(String::capacity).sum::<usize>()
            }
            Self::ScopeBucket(k) => k.scope.heap_bytes() + k.name.capacity(),
            Self::ArrayRepeatPlan(_) | Self::PreparedArrayRepeat(_)
            | Self::DeclarationOwnerTemplate(_)
            | Self::AssociatedItemQualifier(_)
            | Self::PreparedExplicitTypeArguments(_)
            | Self::SourcePatternName(_)
            | Self::PreparedBodyTypeApplication(_)
            | Self::EnteredCall(_)
            | Self::PreparedReturnPlan(_)
            | Self::DeclaredReturnGoal(_)
            | Self::DeclaredParameterGoal { .. }
            | Self::ContextParameterGoal { .. }
            | Self::EnteredType(_)
            | Self::PreparedTypePlan(_)
            | Self::PreparedBindingPlan(_)
            | Self::BindingConstraints(_)
            | Self::EnteredValue(_)
            | Self::LexicalValue(_)
            | Self::MacroDefinitionModule { .. }
            | Self::SourceLocalPlan(_)
            | Self::SourceConstraints(_)
            | Self::QualifiedBodyType(_)
            | Self::BoundRecordConstructor(_)
            | Self::ConstructorPlan(_)
            | Self::TypeSchema(_)
            | Self::TypeAlias(_)
            | Self::FunctionSignature(_)
            | Self::ReferenceHeader(_)
            | Self::PatternAccess(_)
            | Self::PatternPlan(_)
            | Self::Signature(_)
            | Self::BodyStructure(_)
            | Self::Thunk(_) => 0,
            Self::Constant(k) => k.heap_bytes().unwrap_or(usize::MAX),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ArrayRepeatPlan(_) => "ArrayRepeatPlan",
            Self::PreparedArrayRepeat(_) => "PreparedArrayRepeat",
            Self::DeclarationOwnerTemplate(_) => "DeclarationOwnerTemplate",
            Self::PreparedScopeImplHeaders(_) => "PreparedScopeImplHeaders",
            Self::ImplHeaderSyntax(_) => "ImplHeaderSyntax",
            Self::ImplHeadTypes(_) => "ImplHeadTypes",
            Self::ImplTraitIdentity(_) => "ImplTraitIdentity",
            Self::DeclarationSourcePredicates(_) => "DeclarationSourcePredicates",
            Self::SourceVarianceQueries(_) => "SourceVarianceQueries",
            Self::TraitAssociatedTypes(_) => "TraitAssociatedTypes",
            Self::DefinitionHeaderPlan(_) => "DefinitionHeaderPlan",
            Self::SourceLangIdentity(_) => "SourceLangIdentity",
            Self::AdtSourceFields(_) => "AdtSourceFields",
            Self::ImplSourceTypes(_) => "ImplSourceTypes",
            Self::ImplSourcePredicates(_) => "ImplSourcePredicates",
            Self::SolverConfiguration(_) => "SolverConfiguration",
            Self::ImplTraitCatalog(_) => "ImplTraitCatalog",
            Self::ImplPredicates(_) => "ImplPredicates",
            Self::TraitDeclaration(_) => "TraitDeclaration",
            Self::DeclarationGenerics(_) => "DeclarationGenerics",
            Self::DeclarationGenericTypes(_) => "DeclarationGenericTypes",
            Self::AssociatedItemQualifier(_) => "AssociatedItemQualifier",
            Self::AssociatedEquations(_) => "AssociatedEquations",
            Self::NominalMember(_) => "NominalMember",
            Self::PreparedBodySyntax(_) => "PreparedBodySyntax",
            Self::PreparedExplicitTypeArguments(_) => "PreparedExplicitTypeArguments",
            Self::EnteredCall(_) => "EnteredCall",
            Self::SourcePatternName(_) => "SourcePatternName",
            Self::PreparedBodyTypeApplication(_) => "PreparedBodyTypeApplication",
            Self::DeclaredReturnGoal(_) => "DeclaredReturnGoal",
            Self::PreparedReturnPlan(_) => "PreparedReturnPlan",
            Self::EnteredType(_) => "EnteredType",
            Self::PreparedTypePlan(_) => "PreparedTypePlan",
            Self::PreparedBindingPlan(_) => "PreparedBindingPlan",
            Self::BindingConstraints(_) => "BindingConstraints",
            Self::EnteredValue(_) => "EnteredValue",
            Self::LexicalValue(_) => "LexicalValue",
            Self::DeclaredParameterGoal { .. } => "DeclaredParameterGoal",
            Self::ContextParameterGoal { .. } => "ContextParameterGoal",
            Self::SourceConstraints(_) => "SourceConstraints",
            Self::QualifiedBodyType(_) => "QualifiedBodyType",
            Self::BoundRecordConstructor(_) => "BoundRecordConstructor",
            Self::SourceLocalPlan(_) => "SourceLocalPlan",
            Self::MacroDefinitionModule { .. } => "MacroDefinitionModule",
            Self::CurrentModuleName(_) => "CurrentModuleName",
            Self::SealedName(_) => "SealedName",
            Self::DiscoverySeal(_) => "DiscoverySeal",
            Self::ScopeDeclarationInventory(_) => "ScopeDeclarationInventory",
            Self::CurrentExpansionInput(_) => "CurrentExpansionInput",
            Self::ExpansionInput(_) => "ExpansionInput",
            Self::ParseFile(_) => "ParseFile",
            Self::Outline(_) => "Outline",
            Self::ScopeBucket(_) => "ScopeBucket",
            Self::Signature(_) => "Signature",
            Self::FunctionSignature(_) => "FunctionSignature",
            Self::ReferenceHeader(_) => "ReferenceHeader",
            Self::PatternPlan(_) => "PatternPlan",
            Self::PatternAccess(_) => "PatternAccess",
            Self::TypeAlias(_) => "TypeAlias",
            Self::TypeSchema(_) => "TypeSchema",
            Self::ConstructorPlan(_) => "ConstructorPlan",
            Self::BodyStructure(_) => "BodyStructure",
            Self::Thunk(_) => "Thunk",
            Self::Constant(_) => "Constant",
        }
    }
}
impl StableEncode for QueryKey {
    const DOMAIN: &'static str = "lrr/v2/query-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::ArrayRepeatPlan(key) => { e.u32(54); e.record(key); }
            Self::PreparedArrayRepeat(key) => { e.u32(55); e.record(key); }
            Self::DeclarationOwnerTemplate(key) => { e.u32(53); e.record(key); }
            Self::PreparedScopeImplHeaders(key) => { e.u32(56); e.record(key); }
            Self::ImplHeaderSyntax(key) => { e.u32(57); e.record(key.as_ref()); }
            Self::ImplHeadTypes(key) => { e.u32(58); e.record(key.as_ref()); }
            Self::ImplTraitIdentity(key) => { e.u32(65); e.record(key.as_ref()); }
            Self::SolverConfiguration(key) => { e.u32(67); e.record(key.as_ref()); }
            Self::AdtSourceFields(key) => { e.u32(74); e.record(key.as_ref()); }
            Self::SourceLangIdentity(key) => { e.u32(76); e.record(key.as_ref()); }
            Self::DefinitionHeaderPlan(key) => { e.u32(75); e.record(key.as_ref()); }
            Self::TraitAssociatedTypes(key) => { e.u32(77); e.record(key.as_ref()); }
            Self::DeclarationSourcePredicates(key) => { e.u32(78); e.record(key.as_ref()); }
            Self::SourceVarianceQueries(key) => { e.u32(79); e.record(key.as_ref()); }
            Self::ImplSourceTypes(key) => { e.u32(68); e.record(key.as_ref()); }
            Self::SourcePatternName(key) => { e.u32(72); e.record(key); }
            Self::PreparedBodyTypeApplication(key) => { e.u32(73); e.record(key); }
            Self::ImplSourcePredicates(key) => { e.u32(71); e.record(key.as_ref()); }
            Self::ImplTraitCatalog(key) => { e.u32(66); e.record(key); }
            Self::ImplPredicates(key) => { e.u32(59); e.record(key.as_ref()); }
            Self::TraitDeclaration(key) => { e.u32(60); e.digest(*key); }
            Self::DeclarationGenerics(key) => { e.u32(69); e.digest(*key); }
            Self::DeclarationGenericTypes(key) => { e.u32(70); e.record(key.as_ref()); }
            Self::AssociatedItemQualifier(key) => { e.u32(51); e.record(key); }
            Self::AssociatedEquations(key) => {
                e.u32(50);
                e.record(key.as_ref());
            }
            Self::NominalMember(key) => {
                e.u32(49);
                e.record(key.as_ref());
            }
            Self::PreparedBodySyntax(key) => {
                e.u32(48);
                e.record(key.as_ref());
            }
            Self::PreparedExplicitTypeArguments(key) => {
                e.u32(47);
                e.record(key);
            }
            Self::PreparedReturnPlan(definition) => {
                e.u32(46);
                e.digest(*definition);
            }
            Self::EnteredCall(key) => {
                e.u32(44);
                e.record(key);
            }
            Self::DeclaredReturnGoal(definition) => {
                e.u32(45);
                e.digest(*definition);
            }
            Self::EnteredType(key) => {
                e.u32(42);
                e.record(key);
            }
            Self::PreparedTypePlan(key) => {
                e.u32(43);
                e.record(key);
            }
            Self::PreparedBindingPlan(key) => {
                e.u32(41);
                e.record(key);
            }
            Self::BindingConstraints(key) => {
                e.u32(40);
                e.record(key);
            }
            Self::LexicalValue(key) => { e.u32(62); e.record(key); }
            Self::EnteredValue(key) => {
                e.u32(39);
                e.record(key);
            }
            Self::DeclaredParameterGoal {
                definition,
                ordinal,
            } => {
                e.u32(38);
                e.digest(*definition);
                e.u32(*ordinal);
            }
            Self::ContextParameterGoal { body, definition, ordinal } => {
                e.u32(64);
                e.record(body);
                e.digest(*definition);
                e.u32(*ordinal);
            }
            Self::SourceConstraints(key) => {
                e.u32(37);
                e.u32(crate::syntax::source_constraints::SOURCE_CONSTRAINT_SCHEMA);
                e.record(key);
            }
            Self::QualifiedBodyType(key) => {
                e.u32(36);
                e.record(key);
            }
            Self::BoundRecordConstructor(key) => {
                e.u32(35);
                e.record(key);
            }
            Self::MacroDefinitionModule { body, context } => { e.u32(61); e.record(body); e.digest(Digest(context.0)); }
            Self::SourceLocalPlan(key) => {
                e.u32(34);
                e.record(key);
            }
            Self::CurrentModuleName(key) => {
                e.u32(33);
                e.record(key.as_ref());
            }
            Self::SealedName(key) => {
                e.u32(25);
                e.record(key.as_ref());
            }
            Self::ScopeDeclarationInventory(key) => {
                e.u32(26);
                e.record(key);
            }
            Self::DiscoverySeal(key) => {
                e.u32(24);
                e.record(key);
            }
            Self::BodyStructure(k) => {
                e.u32(6);
                e.record(k);
            }
            Self::Thunk(k) => {
                e.u32(7);
                e.u32(crate::ir::op::LRIR_RECIPE_SCHEMA);
                e.u32(crate::ir::op::DECODED_INTEGER_SCHEMA);
                e.record(k);
            }
            Self::CurrentExpansionInput(k) => {
                e.u32(21);
                e.record(k.as_ref());
            }
            Self::ExpansionInput(k) => {
                e.u32(20);
                e.record(k.as_ref());
            }
            Self::ParseFile(k) => {
                e.u32(1);
                e.record(k);
            }
            Self::Outline(k) => {
                e.u32(2);
                e.record(&k.parse);
                e.digest(k.crate_key);
                e.sequence(&k.module_path, |e, s| e.string(s));
                e.digest(k.cfg);
            }
            Self::ScopeBucket(k) => {
                e.u32(3);
                e.record(&k.scope);
                e.u32(k.namespace.tag());
                e.string(&k.name);
            }
            Self::Signature(k) => {
                e.u32(5);
                e.digest(*k);
            }
            Self::FunctionSignature(k) => {
                e.u32(8);
                e.digest(*k);
            }
            Self::ReferenceHeader(k) => {
                e.u32(31);
                e.digest(*k);
            }
            Self::PatternAccess(k) => { e.u32(63); e.record(k.as_ref()); }
            Self::PatternPlan(k) => {
                e.u32(32);
                e.record(k);
            }
            Self::TypeAlias(k) => {
                e.u32(9);
                e.digest(*k);
            }
            Self::TypeSchema(k) => {
                e.u32(27);
                e.digest(*k);
            }
            Self::ConstructorPlan(k) => {
                e.u32(28);
                e.record(k);
            }
            Self::Constant(k) => {
                e.u32(10);
                e.record(k);
            }
        }
    }
}
