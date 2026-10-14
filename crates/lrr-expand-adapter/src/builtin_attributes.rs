//! Compiler-owned attribute facets; caller resolution is by exact identity.
use crate::{AdapterError, UPSTREAM_COMMIT, token_wire::Key};
use hir_expand::{
    builtin::{BuiltinAttrExpander, find_builtin_attr},
    name::Name,
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BuiltinAttributeAction {
    Derive,
    Test,
}
#[derive(Debug, Clone)]
pub struct BuiltinAttributeDescriptor {
    name: &'static str,
    action: BuiltinAttributeAction,
}
impl BuiltinAttributeDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn action(&self) -> BuiltinAttributeAction {
        self.action
    }
    pub fn definition_identity(&self) -> Key {
        let mut hash = blake3::Hasher::new();
        for field in [
            "lrr/builtin-attribute/v1",
            UPSTREAM_COMMIT,
            "Rust-1.99.0",
            self.name,
        ] {
            hash.update(&(field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
        hash.update(&[match self.action {
            BuiltinAttributeAction::Derive => 0,
            BuiltinAttributeAction::Test => 1,
        }]);
        Key(*hash.finalize().as_bytes())
    }
}
/// Compiler-facet discovery never establishes the identity of a user attribute.
pub fn descriptor(name: &str) -> Result<Option<BuiltinAttributeDescriptor>, AdapterError> {
    if name.len() > 64 * 1024 {
        return Err(AdapterError {
            code: "LRR-E-MACRO-SCOPE",
            message: "builtin attribute name limit".into(),
        });
    }
    let Some(attribute) = find_builtin_attr(&Name::new_root(name)) else {
        return Ok(None);
    };
    let (name, action) = match attribute {
        BuiltinAttrExpander::Derive => ("derive", BuiltinAttributeAction::Derive),
        BuiltinAttrExpander::Test => ("test", BuiltinAttributeAction::Test),
        _ => {
            return Err(AdapterError {
                code: "LRR-E-MACRO-PROFILE",
                message: "builtin attribute requires an unimplemented or excluded profile handler"
                    .into(),
            });
        }
    };
    Ok(Some(BuiltinAttributeDescriptor { name, action }))
}

/// Immutable metadata for compiler inert attributes. This is a namespace
/// candidate, never an expansion handler or certification of a user definition.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InertAttributeDescriptor {
    name: &'static str,
    identity: Key,
}
impl InertAttributeDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn definition_identity(&self) -> Key {
        self.identity
    }
}
pub fn inert_descriptor(name: &str) -> Result<Option<InertAttributeDescriptor>, AdapterError> {
    let fault = |message: &str| AdapterError {
        code: "LRR-E-MACRO-SCOPE",
        message: message.into(),
    };
    if name.len() > 64 * 1024 {
        return Err(fault("inert attribute name limit"));
    }
    let Some(attribute) = hir_expand::inert_attr_macro::INERT_ATTRIBUTES
        .iter()
        .find(|attribute| attribute.name == name)
    else {
        return Ok(None);
    };
    let mut hash = blake3::Hasher::new();
    for field in [
        "lrr/inert-attribute/v1",
        UPSTREAM_COMMIT,
        "Rust-1.99.0",
        attribute.name,
    ] {
        let length = u64::try_from(field.len())
            .map_err(|_| fault("inert attribute identity length overflow"))?;
        hash.update(&length.to_le_bytes());
        hash.update(field.as_bytes());
    }
    Ok(Some(InertAttributeDescriptor {
        name: attribute.name,
        identity: Key(*hash.finalize().as_bytes()),
    }))
}
