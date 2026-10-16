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

#[derive(Debug, Clone)]
pub struct Bucket {
    pub candidates: Vec<Digest>,
    pub blockers: Vec<ScopeBlocker>,
}
#[derive(Debug, Clone)]
pub enum QueryValue {
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
    pub fn parse_bytes(&self) -> usize {
        match self {
            Self::ParseFile(v) => v.estimated_bytes(),
            _ => 0,
        }
    }
    pub fn matches(&self, key: &QueryKey) -> bool {
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
            Self::Thunk(value) => value.metadata_bytes(),
            Self::ParseFile(v) => v.estimated_bytes(),
            Self::Outline(v) => v.estimated_bytes(),
            Self::Signature(v) => v.estimated_bytes(),
            Self::FunctionSignature(v) => v.estimated_bytes(),
            Self::ReferenceHeader(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::PatternPlan(v) => v
                .metadata_bytes()
                .saturating_add(2 * std::mem::size_of::<usize>()),
            Self::TypeAlias(v) => v.estimated_bytes(),
            Self::TypeSchema(v) => v.estimated_bytes(),
            Self::ConstructorPlan(v) => v.estimated_bytes().unwrap_or(usize::MAX),
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
