//! Owned runtime builtin metadata boundary. No upstream types enter the frontend.
use crate::core::{diagnostic::Diagnostic, digest::Digest};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BuiltinAttributeAction {
    Derive,
    Test,
}
#[derive(Debug, Clone)]
pub struct BuiltinAttributeDescriptor {
    name: &'static str,
    action: BuiltinAttributeAction,
    identity: Digest,
}
impl BuiltinAttributeDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn action(&self) -> BuiltinAttributeAction {
        self.action
    }
    pub fn definition_identity(&self) -> Digest {
        self.identity
    }
}
/// Compiler-facet discovery. The source resolver still selects exact definitions.
pub fn descriptor(name: &str) -> Result<Option<BuiltinAttributeDescriptor>, Diagnostic> {
    lrr_expand_adapter::builtin_attributes::descriptor(name)
        .map_err(|e| Diagnostic::new(e.code, e.message))
        .map(|metadata| {
            metadata.map(|metadata| BuiltinAttributeDescriptor {
                name: metadata.name(),
                identity: Digest(metadata.definition_identity().0),
                action: match metadata.action() {
                    lrr_expand_adapter::builtin_attributes::BuiltinAttributeAction::Derive => {
                        BuiltinAttributeAction::Derive
                    }
                    lrr_expand_adapter::builtin_attributes::BuiltinAttributeAction::Test => {
                        BuiltinAttributeAction::Test
                    }
                },
            })
        })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InertAttributeDescriptor {
    name: &'static str,
    identity: Digest,
}
impl InertAttributeDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn definition_identity(&self) -> Digest {
        self.identity
    }
}
/// Compiler metadata comes only from the pinned inert-attribute inventory.
pub fn inert_descriptor(name: &str) -> Result<Option<InertAttributeDescriptor>, Diagnostic> {
    lrr_expand_adapter::builtin_attributes::inert_descriptor(name)
        .map_err(|e| Diagnostic::new(e.code, e.message))
        .map(|descriptor| {
            descriptor.map(|descriptor| InertAttributeDescriptor {
                name: descriptor.name(),
                identity: Digest(descriptor.definition_identity().0),
            })
        })
}

/// Preparation does not select a binding. Known handlers remain pending rather
/// than being eagerly rejected while unrelated source rows are captured.
pub(crate) fn preparation_inert_descriptor(
    name: &str,
) -> Result<Option<InertAttributeDescriptor>, Diagnostic> {
    lrr_expand_adapter::builtin_attributes::preparation_inert_descriptor(name)
        .map_err(|e| Diagnostic::new(e.code, e.message))
        .map(|descriptor| {
            descriptor.map(|descriptor| InertAttributeDescriptor {
                name: descriptor.name(),
                identity: Digest(descriptor.definition_identity().0),
            })
        })
}

/// An unqualified builtin-inert spelling is retained only after its competing
/// Attribute facet is resolved. A bang-only candidate does not shadow it. The
/// negative binding identity is part of the seal proof and is replayed at seal.
pub(crate) fn certify_inert_binding(
    name: &str,
    lookup: &crate::frontend::macro_names::PathLookup,
    catalog: Option<&crate::frontend::procedural_exports::PreparedProcCatalog>,
    compiler: Option<&crate::frontend::builtin_attributes::BuiltinAttributeCatalog>,
) -> Result<Option<Digest>, Diagnostic> {
    use crate::{core::digest::StableEncoder, frontend::macro_names::PathLookup};
    let Some(descriptor) = inert_descriptor(name)? else {
        return Ok(None);
    };
    let mut proof =
        StableEncoder::new("lrr/v2/builtin-inert-negative-attribute-facet-with-compiler");
    proof.digest(descriptor.definition_identity());
    proof.u64(match lookup {
        PathLookup::Defined(_) => 1,
        PathLookup::Ambiguous(definitions) => u64::try_from(definitions.len()).map_err(|_| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "inert attribute candidate count overflow".into(),
            )
        })?,
        _ => 0,
    });
    let mut encode =
        |definition: &crate::expand::scopes::MacroDefinition| -> Result<(), Diagnostic> {
            if compiler
                .map(|c| c.select(definition).map(|descriptor| descriptor.is_some()))
                .transpose()?
                .unwrap_or(false)
                || catalog
                    .map(|c| c.attribute_candidate(definition))
                    .transpose()?
                    .unwrap_or(false)
            {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    format!("`{name}` conflicts with a builtin inert attribute"),
                ));
            }
            definition.validate()?;
            proof.digest(definition.identity);
            proof.digest(definition.defining_unit);
            proof.u32(u32::from(definition.definition_edition));
            Ok(())
        };
    match lookup {
        PathLookup::Absent => {}
        PathLookup::Defined(definition) => encode(definition)?,
        PathLookup::Ambiguous(definitions) => {
            // Namespace lookup supplies sorted definitions. Refuse a malformed
            // candidate set rather than encoding unstable or duplicate rows.
            if definitions.windows(2).any(|rows| rows[0] >= rows[1]) {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-SCOPE",
                    "inert attribute candidates are not canonical".into(),
                ));
            }
            for definition in definitions {
                encode(definition)?;
            }
        }
        PathLookup::NeedInput(_) => return Ok(None),
        PathLookup::Inaccessible(_) => {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-INACCESSIBLE",
                "inert attribute competing namespace is inaccessible".into(),
            ));
        }
    }
    Ok(Some(proof.finish()))
}

/// A `test` spelling can retain compiler Test semantics only when its exact
/// binding selects Test or has no competing Attribute facet. Other compiler
/// attributes retain their own handler and cannot become test metadata.
pub(crate) fn permits_builtin_test_marker(
    definition: &crate::expand::scopes::MacroDefinition,
    procedural: Option<&crate::frontend::procedural_exports::PreparedProcCatalog>,
    compiler: Option<&crate::frontend::builtin_attributes::BuiltinAttributeCatalog>,
) -> Result<bool, Diagnostic> {
    definition.validate()?;
    if let Some(descriptor) = compiler
        .map(|c| c.select(definition))
        .transpose()?
        .flatten()
    {
        return Ok(descriptor.action() == BuiltinAttributeAction::Test);
    }
    Ok(!procedural
        .map(|c| c.attribute_candidate(definition))
        .transpose()?
        .unwrap_or(false))
}

/// Compiler attributes retained for LRR's own test metadata handler. Test is
/// identified through the pinned compiler facet; it is not a no-op user macro.
pub fn retained_descriptor(name: &str) -> Result<Option<InertAttributeDescriptor>, Diagnostic> {
    if let Some(inert) = inert_descriptor(name)? {
        return Ok(Some(inert));
    }
    if name != "test" {
        return Ok(None);
    }
    let Some(test) = descriptor(name)? else {
        return Ok(None);
    };
    if test.action() != BuiltinAttributeAction::Test {
        return Ok(None);
    }
    Ok(Some(InertAttributeDescriptor {
        name: test.name(),
        identity: test.definition_identity(),
    }))
}
