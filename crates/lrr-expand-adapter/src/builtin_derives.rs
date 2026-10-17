//! Compiler-owned builtin derive facets. Discovery is not call-site name resolution.
use crate::{AdapterError, UPSTREAM_COMMIT, token_wire::Key};
use hir_expand::{
    builtin::{BuiltinDeriveExpander, find_builtin_derive},
    name::Name,
};

#[derive(Debug, Clone)]
pub struct BuiltinDeriveDescriptor {
    name: &'static str,
    helpers: &'static [&'static str],
}
impl BuiltinDeriveDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn helpers(&self) -> &'static [&'static str] {
        self.helpers
    }
    pub fn engine_identity(&self) -> Key {
        identity(
            b"lrr-builtin-derive-engine-v1",
            &[UPSTREAM_COMMIT, "Rust-1.99.0/stable-helpers-v1"],
        )
    }
    pub fn definition_identity(&self) -> Key {
        let mut fields = vec![UPSTREAM_COMMIT, self.name, "Rust-1.99.0/stable-helpers-v1"];
        fields.extend_from_slice(self.helpers);
        identity(b"lrr-builtin-derive-definition-v1", &fields)
    }
}
fn identity(domain: &[u8], fields: &[&str]) -> Key {
    let mut hash = blake3::Hasher::new();
    hash.update(&(domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(&(fields.len() as u64).to_le_bytes());
    for field in fields {
        hash.update(&(field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    Key(*hash.finalize().as_bytes())
}
/// Used to build compiler-owned facets. A user macro of this spelling is never
/// resolved through this API: the resolver must retain the exact descriptor identity.
pub fn descriptor(name: &str) -> Result<Option<BuiltinDeriveDescriptor>, AdapterError> {
    let Some(derive) = find_builtin_derive(&Name::new_root(name)) else {
        return Ok(None);
    };
    let name = match derive {
        BuiltinDeriveExpander::Copy => "Copy",
        BuiltinDeriveExpander::Clone => "Clone",
        BuiltinDeriveExpander::Default => "Default",
        BuiltinDeriveExpander::Debug => "Debug",
        BuiltinDeriveExpander::Hash => "Hash",
        BuiltinDeriveExpander::Ord => "Ord",
        BuiltinDeriveExpander::PartialOrd => "PartialOrd",
        BuiltinDeriveExpander::Eq => "Eq",
        BuiltinDeriveExpander::PartialEq => "PartialEq",
        BuiltinDeriveExpander::CoercePointee
        | BuiltinDeriveExpander::Reborrow
        | BuiltinDeriveExpander::CoerceShared => {
            return Err(AdapterError {
                code: "LRR-E-MACRO-PROFILE",
                message: "unstable builtin derive is outside the selected stable profile".into(),
            });
        }
    };
    // The pinned default expander consumes the default-variant annotation.
    // This owned helper recipe is covered by native compiler acceptance.
    Ok(Some(BuiltinDeriveDescriptor {
        name,
        helpers: if name == "Default" { &["default"] } else { &[] },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_builtin_derive_facets_are_stable_and_exclude_unstable_profile() {
        let mut seen = std::collections::BTreeSet::new();
        for name in [
            "Copy",
            "Clone",
            "Default",
            "Debug",
            "Hash",
            "Ord",
            "PartialOrd",
            "Eq",
            "PartialEq",
        ] {
            let d = descriptor(name).unwrap().unwrap();
            assert_eq!(d.name(), name);
            assert!(seen.insert(d.definition_identity()));
            assert_eq!(
                d.definition_identity(),
                descriptor(name).unwrap().unwrap().definition_identity()
            );
            assert_eq!(
                d.helpers(),
                if name == "Default" {
                    &["default"][..]
                } else {
                    &[][..]
                }
            );
        }
        assert!(descriptor("UserDefault").unwrap().is_none());
        for name in ["CoercePointee", "Reborrow", "CoerceShared"] {
            assert!(descriptor(name).is_err());
        }
    }
}
