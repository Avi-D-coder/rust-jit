//! Compiler-owned attribute facets; caller resolution is by exact identity.
use crate::{AdapterError, UPSTREAM_COMMIT, token_wire::Key};
use hir_expand::{
    builtin::{BuiltinAttrExpander, find_builtin_attr},
    name::Name,
};
#[path = "compiler_inert_inventory.rs"]
mod compiler_inventory;

/// Metadata source version for the macro engine/preparation identity. This
/// supplies compiler facts only, never a resolved user attribute binding.
pub fn compiler_inert_inventory_identity() -> Key {
    let mut hash = blake3::Hasher::new();
    for field in [
        "lrr/pinned-compiler-inert-inventory/v2",
        compiler_inventory::COMPILER_COMMIT,
        compiler_inventory::SOURCE_SHA256,
        compiler_inventory::DIAGNOSTIC_SOURCE_IDENTITY,
        "Rust-1.99.0",
    ] {
        hash.update(&(field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    Key(*hash.finalize().as_bytes())
}
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
/// Complete compiler-name recognition and semantic admission are separate.
/// Diagnostic metadata remains attached to the source; implementing its exact
/// diagnostics belongs to the separately reported diagnostic gate.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompilerAttributeRequirement {
    DiagnosticMetadata,
    SemanticHandler,
}
pub fn compiler_attribute_requirement(
    name: &str,
) -> Result<Option<CompilerAttributeRequirement>, AdapterError> {
    if name.len() > 64 * 1024 {
        return Err(AdapterError {
            code: "LRR-E-MACRO-SCOPE",
            message: "compiler attribute name limit".into(),
        });
    }
    if compiler_inventory::NAMES.binary_search(&name).is_err() {
        return Ok(None);
    }
    Ok(Some(
        if compiler_inventory::DIAGNOSTIC_NAMES
            .binary_search(&name)
            .is_ok()
        {
            CompilerAttributeRequirement::DiagnosticMetadata
        } else {
            CompilerAttributeRequirement::SemanticHandler
        },
    ))
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
    let upstream = hir_expand::inert_attr_macro::INERT_ATTRIBUTES
        .iter()
        .find(|attribute| attribute.name == name);
    // The pinned adapter inventory predates some compiler annotations. The
    // Complete name metadata cannot discharge semantic annotation handling.
    // Only the separately pinned diagnostic parser category can supplement
    // existing adapter admission without a new runtime semantic handler.
    let (name, compiler) = match upstream {
        Some(attribute) => (attribute.name, false),
        None => match compiler_inventory::NAMES.binary_search(&name) {
            Ok(index) => {
                if compiler_attribute_requirement(name)?
                    != Some(CompilerAttributeRequirement::DiagnosticMetadata)
                {
                    return Err(AdapterError {
                        code: "LRR-E-MACRO-PROFILE",
                        message: format!(
                            "compiler attribute `{name}` requires an unimplemented semantic handler"
                        ),
                    });
                }
                (compiler_inventory::NAMES[index], true)
            }
            Err(_) => return Ok(None),
        },
    };
    let mut hash = blake3::Hasher::new();
    if compiler {
        hash.update(b"lrr/compiler-inert-attribute/v1");
        hash.update(&compiler_inert_inventory_identity().0);
        let length = u64::try_from(name.len())
            .map_err(|_| fault("inert attribute identity length overflow"))?;
        hash.update(&length.to_le_bytes());
        hash.update(name.as_bytes());
    } else {
        // Preserve already issued adapter descriptor identities exactly.
        for field in [
            "lrr/inert-attribute/v1",
            UPSTREAM_COMMIT,
            "Rust-1.99.0",
            name,
        ] {
            let length = u64::try_from(field.len())
                .map_err(|_| fault("inert attribute identity length overflow"))?;
            hash.update(&length.to_le_bytes());
            hash.update(field.as_bytes());
        }
    }
    Ok(Some(InertAttributeDescriptor {
        name,
        identity: Key(*hash.finalize().as_bytes()),
    }))
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use std::collections::BTreeSet;
    use syntax::{
        AstNode,
        ast::{self, HasModuleItem, HasName},
    };

    #[test]
    fn complete_pinned_compiler_inventory_matches_actual_static_array() {
        let source = include_str!(
            "../../../fixtures/upstream/rustc-checkout/compiler/rustc_feature/src/builtin_attrs.rs"
        );
        let parsed = syntax::SourceFile::parse(source, syntax::Edition::Edition2024);
        assert!(parsed.errors().is_empty());
        let declarations: Vec<_> = parsed
            .tree()
            .items()
            .filter_map(|item| match item {
                ast::Item::Static(item)
                    if item
                        .name()
                        .is_some_and(|name| name.text() == "BUILTIN_ATTRIBUTES") =>
                {
                    Some(item)
                }
                _ => None,
            })
            .collect();
        assert_eq!(declarations.len(), 1);
        let Some(ast::Expr::RefExpr(reference)) = declarations[0].body() else {
            panic!("actual compiler inventory reference absent")
        };
        assert!(reference.mut_token().is_none() && reference.raw_token().is_none());
        let Some(ast::Expr::ArrayExpr(array)) = reference.expr() else {
            panic!("actual compiler symbol array absent")
        };
        assert!(array.semicolon_token().is_none());
        let mut actual = BTreeSet::new();
        for expression in array.exprs() {
            let ast::Expr::PathExpr(expression) = expression else {
                panic!("compiler metadata entry is not a symbol path")
            };
            let path = expression.path().unwrap();
            let qualifier = path.qualifier().unwrap();
            assert!(qualifier.qualifier().is_none());
            assert_eq!(
                qualifier.segment().unwrap().name_ref().unwrap().text(),
                "sym"
            );
            let name = path
                .segment()
                .unwrap()
                .name_ref()
                .unwrap()
                .text()
                .to_string();
            assert!(actual.insert(name));
        }
        let expected: BTreeSet<_> = compiler_inventory::NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(compiler_inventory::NAMES.len(), actual.len());
        assert!(
            compiler_inventory::NAMES
                .windows(2)
                .all(|names| names[0] < names[1])
        );
        for name in &actual {
            assert!(compiler_attribute_requirement(name).unwrap().is_some());
            if let Some(old) = hir_expand::inert_attr_macro::INERT_ATTRIBUTES
                .iter()
                .find(|entry| entry.name == name.as_str())
            {
                let descriptor = inert_descriptor(name).unwrap().unwrap();
                assert_eq!(descriptor.name(), name.as_str());
                let mut hash = blake3::Hasher::new();
                for field in [
                    "lrr/inert-attribute/v1",
                    UPSTREAM_COMMIT,
                    "Rust-1.99.0",
                    old.name,
                ] {
                    hash.update(&(field.len() as u64).to_le_bytes());
                    hash.update(field.as_bytes());
                }
                assert_eq!(
                    descriptor.definition_identity(),
                    Key(*hash.finalize().as_bytes())
                );
            } else if compiler_attribute_requirement(name).unwrap()
                == Some(CompilerAttributeRequirement::DiagnosticMetadata)
            {
                assert_eq!(
                    inert_descriptor(name).unwrap().unwrap().name(),
                    name.as_str()
                );
            } else {
                assert_eq!(
                    inert_descriptor(name).unwrap_err().code,
                    "LRR-E-MACRO-PROFILE"
                );
            }
        }
    }

    #[test]
    fn diagnostic_aliases_match_complete_pinned_parser_registry() {
        let mut actual = BTreeSet::new();
        let mut modules = BTreeSet::new();
        let mut files = BTreeSet::new();
        for (path, source) in compiler_inventory::DIAGNOSTIC_SOURCES {
            let file = path.rsplit('/').next().unwrap();
            assert!(files.insert(file));
            let parsed = syntax::SourceFile::parse(source, syntax::Edition::Edition2024);
            assert!(parsed.errors().is_empty(), "{path}");
            if file == "mod.rs" {
                for item in parsed.tree().items() {
                    if let ast::Item::Module(module) = item {
                        assert!(module.item_list().is_none());
                        assert!(modules.insert(format!("{}.rs", module.name().unwrap().text())));
                    }
                }
                continue;
            }
            let mut mappings = 0;
            for declaration in parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::Const::cast)
            {
                let Some(name) = declaration.name() else {
                    continue;
                };
                if !matches!(name.text().as_ref(), "ATTRIBUTES" | "PATH") {
                    continue;
                }
                mappings += 1;
                let body = declaration.body().unwrap();
                for array in body.syntax().descendants().filter_map(ast::ArrayExpr::cast) {
                    let expressions: Vec<_> = array.exprs().collect();
                    // The outer AcceptMapping array consists of tuples. Its
                    // nested symbol arrays carry the actual accepted paths.
                    if expressions
                        .iter()
                        .any(|expr| !matches!(expr, ast::Expr::PathExpr(_)))
                    {
                        continue;
                    }
                    assert!(!expressions.is_empty());
                    let mut segments = Vec::new();
                    for expression in expressions {
                        let ast::Expr::PathExpr(expression) = expression else {
                            unreachable!()
                        };
                        let path = expression.path().unwrap();
                        let qualifier = path.qualifier().unwrap();
                        assert!(qualifier.qualifier().is_none());
                        assert_eq!(
                            qualifier.segment().unwrap().name_ref().unwrap().text(),
                            "sym"
                        );
                        segments.push(
                            path.segment()
                                .unwrap()
                                .name_ref()
                                .unwrap()
                                .text()
                                .to_string(),
                        );
                    }
                    match segments.as_slice() {
                        [name] => {
                            assert!(actual.insert(name.clone()));
                        }
                        [namespace, _] if namespace == "diagnostic" => {}
                        _ => panic!("unexpected diagnostic compiler mapping in {path}"),
                    }
                }
            }
            assert_eq!(mappings, 1, "{path}");
        }
        assert_eq!(
            files,
            modules
                .iter()
                .map(String::as_str)
                .chain(["mod.rs"])
                .collect()
        );
        assert_eq!(
            actual,
            compiler_inventory::DIAGNOSTIC_NAMES
                .iter()
                .map(|s| (*s).to_owned())
                .collect()
        );
        assert!(
            compiler_inventory::DIAGNOSTIC_NAMES
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
    }
}
