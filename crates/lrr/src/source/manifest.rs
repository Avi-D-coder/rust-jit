//! Stable source identities. Locations are intentionally absent from keys.
use crate::core::digest::{Digest, StableEncode, StableEncoder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PackageKey {
    pub source_identity: String,
    pub package_name: String,
    pub version: String,
}
impl StableEncode for PackageKey {
    const DOMAIN: &'static str = "lrr/v2/package";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.source_identity);
        e.string(&self.package_name);
        e.string(&self.version);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum TargetKind {
    Library,
    Binary,
    LibraryTests,
    IntegrationTest,
}
impl TargetKind {
    fn tag(self) -> u32 {
        match self {
            Self::Library => 1,
            Self::Binary => 2,
            Self::LibraryTests => 3,
            Self::IntegrationTest => 4,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CrateKey {
    pub package: PackageKey,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub guest_edition: u16,
    pub semantics_profile: Digest,
    pub cfg_fingerprint: Digest,
    pub dependency_aliases: Digest,
}
impl StableEncode for CrateKey {
    const DOMAIN: &'static str = "lrr/v2/crate";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.package);
        e.u32(self.target_kind.tag());
        e.string(&self.target_name);
        e.u16(self.guest_edition);
        e.digest(self.semantics_profile);
        e.digest(self.cfg_fingerprint);
        e.digest(self.dependency_aliases);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Namespace {
    Value,
    Type,
    Macro,
}
impl Namespace {
    pub fn tag(self) -> u32 {
        match self {
            Self::Value => 1,
            Self::Type => 2,
            Self::Macro => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ItemKind {
    Function,
    Struct,
    Enum,
    Module,
    Constant,
    Static,
    Impl,
    Trait,
    Macro,
    TypeAlias,
    Import,
    Union,
    ExternBlock,
}
impl ItemKind {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Function => 1,
            Self::Struct => 2,
            Self::Enum => 3,
            Self::Module => 4,
            Self::Constant => 5,
            Self::Static => 6,
            Self::Impl => 7,
            Self::Trait => 8,
            Self::Macro => 9,
            Self::TypeAlias => 10,
            Self::Import => 11,
            Self::Union => 12,
            Self::ExternBlock => 13,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct DefKey {
    pub crate_key: Digest,
    pub module_path: Vec<String>,
    pub namespace: Namespace,
    pub item_kind: ItemKind,
    pub declared_name: String,
    pub enclosing_impl: Option<Digest>,
    pub duplicate_ordinal: u32,
}
impl StableEncode for DefKey {
    const DOMAIN: &'static str = "lrr/v2/definition";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.crate_key);
        e.sequence(&self.module_path, |e, s| e.string(s));
        e.u32(self.namespace.tag());
        e.u32(self.item_kind.tag());
        e.string(&self.declared_name);
        e.option(&self.enclosing_impl, |e, d| e.digest(*d));
        e.u32(self.duplicate_ordinal);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BodyKey {
    pub definition: Digest,
    pub exact_body_bytes: Digest,
}
impl StableEncode for BodyKey {
    const DOMAIN: &'static str = "lrr/v2/body";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.exact_body_bytes);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PcKey {
    pub body: BodyKey,
    pub node: u32,
    pub phase: u16,
}
impl StableEncode for PcKey {
    const DOMAIN: &'static str = "lrr/v2/program-counter";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.body);
        e.u32(self.node);
        e.u16(self.phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn definition() -> DefKey {
        DefKey {
            crate_key: Digest::of_bytes(b"crate"),
            module_path: vec!["module".into()],
            namespace: Namespace::Value,
            item_kind: ItemKind::Function,
            declared_name: "call".into(),
            enclosing_impl: None,
            duplicate_ordinal: 0,
        }
    }
    #[test]
    fn semantic_keys_exclude_locations_and_distinguish_families() {
        let a = definition();
        let b = a.clone();
        // Locations are attached separately; neither key has an offset field.
        assert_eq!(a.fingerprint(), b.fingerprint());
        let body = BodyKey {
            definition: a.fingerprint(),
            exact_body_bytes: Digest::of_bytes(b"{1_i64}"),
        };
        let pc = PcKey {
            body: body.clone(),
            node: 1,
            phase: 0,
        };
        assert_ne!(a.fingerprint(), body.fingerprint());
        assert_ne!(body.fingerprint(), pc.fingerprint());
        let mut edited = body.clone();
        edited.exact_body_bytes = Digest::of_bytes(b"{2_i64}");
        assert_ne!(body.fingerprint(), edited.fingerprint());
        assert_eq!(body.definition, edited.definition);
    }
    #[test]
    fn edition_changes_crate_and_definition_identity() {
        let mut key = CrateKey {
            package: PackageKey {
                source_identity: "root".into(),
                package_name: "app".into(),
                version: "0.1.0".into(),
            },
            target_kind: TargetKind::LibraryTests,
            target_name: "app".into(),
            guest_edition: 2024,
            semantics_profile: crate::core::profile::ProfileIdentity::current().fingerprint(),
            cfg_fingerprint: Digest::default(),
            dependency_aliases: Digest::default(),
        };
        let current = key.fingerprint();
        key.guest_edition = 2021;
        assert_ne!(current, key.fingerprint());
    }
    #[test]
    fn definition_changes_name_namespace_and_duplicate_ordinal() {
        let key = definition();
        let base = key.fingerprint();
        let mut renamed = key.clone();
        renamed.declared_name = "other".into();
        assert_ne!(base, renamed.fingerprint());
        let mut namespace = key.clone();
        namespace.namespace = Namespace::Type;
        assert_ne!(base, namespace.fingerprint());
        let mut duplicate = key;
        duplicate.duplicate_ordinal = 1;
        assert_ne!(base, duplicate.fingerprint());
    }
}
