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
    PreparedBodySyntax(Box<PreparedBodySyntaxKey>),
    AssociatedEquations(Box<crate::frontend::associated_equations::AssociatedEquationsKey>),
    NominalMember(Box<crate::frontend::nominal_member_goals::NominalMemberKey>),
    PreparedExplicitTypeArguments(crate::source::manifest::PcKey),
    EnteredCall(crate::source::manifest::PcKey),
    DeclaredReturnGoal(Digest),
    PreparedReturnPlan(Digest),
    EnteredType(crate::source::manifest::PcKey),
    PreparedTypePlan(crate::source::manifest::PcKey),
    PreparedBindingPlan(crate::source::manifest::PcKey),
    BindingConstraints(crate::source::manifest::PcKey),
    EnteredValue(crate::source::manifest::PcKey),
    DeclaredParameterGoal { definition: Digest, ordinal: u32 },
    SourceConstraints(crate::source::manifest::BodyKey),
    QualifiedBodyType(crate::source::manifest::PcKey),
    BoundRecordConstructor(crate::source::manifest::PcKey),
    SourceLocalPlan(crate::source::manifest::BodyKey),
    CurrentModuleName(Box<CurrentModuleNameKey>),
    SealedName(Box<SealedNameKey>),
    DiscoverySeal(super::seals::SealKey),
    ScopeDeclarationInventory(super::seals::SealKey),
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
            Self::DiscoverySeal(_) | Self::ScopeDeclarationInventory(_) => 0,
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
            Self::PreparedExplicitTypeArguments(_)
            | Self::EnteredCall(_)
            | Self::PreparedReturnPlan(_)
            | Self::DeclaredReturnGoal(_)
            | Self::DeclaredParameterGoal { .. }
            | Self::EnteredType(_)
            | Self::PreparedTypePlan(_)
            | Self::PreparedBindingPlan(_)
            | Self::BindingConstraints(_)
            | Self::EnteredValue(_)
            | Self::SourceLocalPlan(_)
            | Self::SourceConstraints(_)
            | Self::QualifiedBodyType(_)
            | Self::BoundRecordConstructor(_)
            | Self::ConstructorPlan(_)
            | Self::TypeSchema(_)
            | Self::TypeAlias(_)
            | Self::FunctionSignature(_)
            | Self::ReferenceHeader(_)
            | Self::PatternPlan(_)
            | Self::Signature(_)
            | Self::BodyStructure(_)
            | Self::Thunk(_) => 0,
            Self::Constant(k) => k.heap_bytes().unwrap_or(usize::MAX),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AssociatedEquations(_) => "AssociatedEquations",
            Self::NominalMember(_) => "NominalMember",
            Self::PreparedBodySyntax(_) => "PreparedBodySyntax",
            Self::PreparedExplicitTypeArguments(_) => "PreparedExplicitTypeArguments",
            Self::EnteredCall(_) => "EnteredCall",
            Self::DeclaredReturnGoal(_) => "DeclaredReturnGoal",
            Self::PreparedReturnPlan(_) => "PreparedReturnPlan",
            Self::EnteredType(_) => "EnteredType",
            Self::PreparedTypePlan(_) => "PreparedTypePlan",
            Self::PreparedBindingPlan(_) => "PreparedBindingPlan",
            Self::BindingConstraints(_) => "BindingConstraints",
            Self::EnteredValue(_) => "EnteredValue",
            Self::DeclaredParameterGoal { .. } => "DeclaredParameterGoal",
            Self::SourceConstraints(_) => "SourceConstraints",
            Self::QualifiedBodyType(_) => "QualifiedBodyType",
            Self::BoundRecordConstructor(_) => "BoundRecordConstructor",
            Self::SourceLocalPlan(_) => "SourceLocalPlan",
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
