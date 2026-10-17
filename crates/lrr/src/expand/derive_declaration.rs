//! Source-facet derive declarations; no rustc metadata or native body analysis.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::{
        ast_adapter::{TokenRecipe, macro_name},
        attributes::ItemAttributeInputs,
    },
};
use std::collections::BTreeSet;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
#[derive(Debug, Clone)]
pub struct DeriveExportNames {
    export: String,
    helpers: BTreeSet<String>,
}
impl DeriveExportNames {
    pub fn export(&self) -> &str {
        &self.export
    }
    pub fn helpers(&self) -> &BTreeSet<String> {
        &self.helpers
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        // Conservative BTree node bookkeeping plus actual retained String capacities.
        self.helpers.iter().try_fold(
            std::mem::size_of::<Self>()
                .checked_add(self.export.capacity())
                .ok_or_else(|| fault("derive name accounting overflow"))?,
            |bytes, name| {
                bytes
                    .checked_add(128)
                    .and_then(|n| n.checked_add(name.capacity()))
                    .ok_or_else(|| fault("derive helper accounting overflow"))
            },
        )
    }
}
impl StableEncode for DeriveExportNames {
    const DOMAIN: &'static str = "lrr/v2/source-derive-export-names";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.export);
        e.u64(self.helpers.len() as u64);
        for h in &self.helpers {
            e.string(h);
        }
    }
}
/// Consume owned tokens produced by the pinned AST, including materialized cfg_attr.
/// Unexpanded metavariables and malformed nested/path/literal arguments refuse.
pub fn parse_derive_attribute(
    tokens: &[TokenRecipe],
    edition: u16,
) -> Result<DeriveExportNames, Diagnostic> {
    tokens
        .iter()
        .try_fold(0usize, |n, t| n.checked_add(t.text.capacity()))
        .ok_or_else(|| fault("derive declaration byte limit"))?;
    let mut cursor = 0usize;
    let take = |cursor: &mut usize, expected: &str| -> Result<(), Diagnostic> {
        if tokens.get(*cursor).is_none_or(|t| t.text != expected) {
            return Err(fault("invalid proc_macro_derive declaration grammar"));
        }
        *cursor += 1;
        Ok(())
    };
    let name = |cursor: &mut usize| -> Result<String, Diagnostic> {
        let t = tokens
            .get(*cursor)
            .ok_or_else(|| fault("missing derive declaration identifier"))?;
        let n = macro_name(&t.text, edition)
            .ok_or_else(|| fault("invalid derive declaration identifier"))?;
        *cursor += 1;
        Ok(n)
    };
    take(&mut cursor, "proc_macro_derive")?;
    take(&mut cursor, "(")?;
    let export = name(&mut cursor)?;
    let mut helpers = BTreeSet::new();
    if tokens.get(cursor).is_some_and(|t| t.text == ",") {
        cursor += 1;
        if tokens.get(cursor).is_some_and(|t| t.text != ")") {
            take(&mut cursor, "attributes")?;
            take(&mut cursor, "(")?;
            while tokens.get(cursor).is_some_and(|t| t.text != ")") {
                helpers.insert(name(&mut cursor)?);
                if tokens.get(cursor).is_some_and(|t| t.text == ",") {
                    cursor += 1;
                } else {
                    break;
                }
            }
            take(&mut cursor, ")")?;
            if tokens.get(cursor).is_some_and(|t| t.text == ",") {
                cursor += 1;
            }
        }
    }
    take(&mut cursor, ")")?;
    if cursor != tokens.len() {
        return Err(fault("trailing derive declaration tokens"));
    }
    Ok(DeriveExportNames { export, helpers })
}
/// A fragment is not a complete macro-host export catalog. Unknown transforming
/// attrs must be resolved before this function can establish a declaration.
pub fn from_item_inputs(inputs: &ItemAttributeInputs) -> Result<DeriveExportNames, Diagnostic> {
    if !inputs.enabled {
        return Err(fault("inactive source derive declaration"));
    }
    let mut declaration = None;
    for attr in &inputs.attributes {
        match attr.name.as_str() {
            "proc_macro_derive" => {
                if declaration.replace(attr).is_some() {
                    return Err(fault("duplicate proc_macro_derive attribute"));
                }
            }
            "cfg" | "doc" | "allow" | "warn" | "deny" | "forbid" | "deprecated" | "inline"
            | "cold" | "must_use" => {}
            _ => return Err(fault("source derive declaration needs attribute expansion")),
        }
    }
    parse_derive_attribute(
        &declaration
            .ok_or_else(|| fault("missing proc_macro_derive attribute"))?
            .tokens,
        inputs.parse.profile.edition,
    )
}
#[derive(Debug, Clone)]
pub struct SourceDeriveDeclaration {
    pub(crate) source: Digest,
    pub(crate) cfg: Digest,
    pub(crate) edition: u16,
    pub(crate) names: DeriveExportNames,
    pub(crate) header: Digest,
}
impl SourceDeriveDeclaration {
    pub fn names(&self) -> &DeriveExportNames {
        &self.names
    }
}
impl StableEncode for SourceDeriveDeclaration {
    const DOMAIN: &'static str = "lrr/v2/source-derive-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.source);
        e.digest(self.cfg);
        e.u16(self.edition);
        e.record(&self.names);
        e.digest(self.header);
    }
}

/// Original-source catalog facts. Pending producers prohibit a complete answer.
#[derive(Debug, Clone)]
pub struct SourceDeriveCatalog {
    pub(crate) declarations: Vec<(crate::syntax::spans::SourceRange, SourceDeriveDeclaration)>,
    pub(crate) pending: Vec<crate::syntax::spans::SourceRange>,
}
impl SourceDeriveCatalog {
    pub fn declarations(&self) -> &[(crate::syntax::spans::SourceRange, SourceDeriveDeclaration)] {
        &self.declarations
    }
    pub fn pending(&self) -> &[crate::syntax::spans::SourceRange] {
        &self.pending
    }
}
#[derive(Debug, Clone)]
pub enum SourceCatalogAdmission {
    Ready(Vec<Digest>),
    NeedExpansion(Vec<crate::syntax::spans::SourceRange>),
    NeedSource(Vec<String>),
}
