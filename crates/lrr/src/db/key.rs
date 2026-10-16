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
pub use crate::frontend::constants::query_identity::ConstQueryIdentity as ConstantKey;
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum QueryKey {
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
            Self::ConstructorPlan(_)
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
