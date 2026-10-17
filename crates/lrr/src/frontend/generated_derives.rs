//! Generated compiler annotation selection through the existing source namespace.
//! Unresolved candidates are data, never inert attributes or a completeness grant.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    expand::{
        builtin_attributes::BuiltinAttributeAction,
        generated_derives::{GeneratedDeriveAnnotations, GeneratedDerivePlan},
        generated_items::GeneratedItems,
        scopes::MacroDefinition,
    },
    frontend::{
        builtin_attributes::BuiltinAttributeCatalog,
        macro_names::{MacroNamespaces, PathLookup},
    },
    source::snapshot::CfgSet,
    syntax::{outline::ScopeKey, parse::ParsedFile, spans::SourceRange, token_wire::Key},
};
use std::collections::BTreeMap;
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub struct GeneratedDeriveResolution {
    scope: ScopeKey,
    proof: GeneratedDeriveAnnotations,
    candidates: BTreeMap<usize, PathLookup>,
    positive: BTreeMap<usize, MacroDefinition>,
}
impl GeneratedDeriveResolution {
    pub fn resolve(
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        scope: ScopeKey,
        names: &MacroNamespaces,
        builtin: &BuiltinAttributeCatalog,
    ) -> Result<Self, Diagnostic> {
        let header = items.items().get(index).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive resolution header absent",
            )
        })?;
        let parsed = ParsedFile::from_generated_items(items)?;
        let facts = parsed.derive_item_inputs_at(
            SourceRange {
                start: header.start,
                end: header.end,
            },
            cfg,
        )?;
        if !facts.item.enabled {
            return Err(fault(
                "LRR-E-DISCOVERY-LIMIT",
                "generated derive resolution inactive item or annotation limit",
            ));
        }

        let hygiene = items.hygiene_facts();
        let mut candidates = BTreeMap::new();
        let mut positive = BTreeMap::new();
        let mut certified = Vec::new();
        certified
            .try_reserve_exact(facts.item.attributes.len())
            .map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive certificate allocation",
                )
            })?;
        let mut bytes = scope.heap_bytes();
        for (at, attribute) in facts.item.attributes.iter().enumerate() {
            if attribute.name == "cfg"
                || crate::expand::builtin_attributes::retained_descriptor(&attribute.name)?
                    .is_some()
            {
                continue;
            }
            let syntax = parsed.attribute_path_at(attribute.range)?;
            let path =
                crate::expand::generated_derives::annotation_path(items, index, attribute.range)?;
            let [crate::syntax::token_wire::TokenTree::Group { children, .. }] = path.roots()
            else {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated annotation path root absent",
                ));
            };
            let mut class = None;
            let mut edition = None;
            let mut mixed = false;
            for token in children {
                let span = match token {
                    crate::syntax::token_wire::TokenTree::Ident { span, .. }
                    | crate::syntax::token_wire::TokenTree::Punct { span, .. } => span,
                    _ => {
                        return Err(fault(
                            "LRR-E-EXPANSION-INPUT",
                            "generated annotation path has non-path token",
                        ));
                    }
                };
                let row = path
                    .spans()
                    .iter()
                    .find(|s| s.key() == *span)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "generated annotation path span absent",
                        )
                    })?;
                let normalized = hygiene.name_item(Key(row.context.0))?;
                mixed |= class.is_some_and(|old| old != normalized);
                class = Some(normalized);
                edition.get_or_insert(row.edition);
            }
            let edition = match edition
                .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated annotation path empty"))?
            {
                crate::syntax::token_wire::Edition::E2015 => 2015,
                crate::syntax::token_wire::Edition::E2018 => 2018,
                crate::syntax::token_wire::Edition::E2021 => 2021,
                crate::syntax::token_wire::Edition::E2024 => 2024,
            };
            let candidate = if mixed {
                let mut e = StableEncoder::new("lrr/v2/generated-annotation-context-demand");
                e.record(&header.key);
                e.record(&scope);
                e.digest(cfg.fingerprint());
                e.u64(u64::try_from(at).map_err(|_| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated annotation index overflow",
                    )
                })?);
                PathLookup::NeedInput(e.finish())
            } else {
                names.lookup_invocation_segments(
                    syntax.segments(),
                    syntax.absolute(),
                    &scope,
                    edition,
                )?
            };
            let extra = match &candidate {
                PathLookup::Ambiguous(rows) => rows
                    .capacity()
                    .checked_mul(std::mem::size_of::<MacroDefinition>()),
                PathLookup::Inaccessible(paths) => paths.iter().try_fold(0usize, |n, p| {
                    p.components().iter().try_fold(n, |n, c| {
                        n.checked_add(c.as_str().len())
                            .and_then(|n| n.checked_add(64))
                    })
                }),
                _ => Some(128),
            }
            .and_then(|n| n.checked_add(256));
            bytes = extra
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated annotation selection metadata limit",
                    )
                })?;
            if let PathLookup::Defined(definition) = &candidate
                && let Some(descriptor) = builtin.select(definition)?
                && descriptor.action() == BuiltinAttributeAction::Derive
            {
                positive.insert(at, definition.clone());
                certified.push((at, descriptor.clone()));
            }
            candidates.insert(at, candidate);
        }
        let proof = GeneratedDeriveAnnotations::new(items, index, cfg, certified)?;
        Ok(Self {
            scope,
            proof,
            candidates,
            positive,
        })
    }
    pub fn candidates(&self) -> &BTreeMap<usize, PathLookup> {
        &self.candidates
    }
    /// Pure exact export selection. Helper metadata and execution stay separate.
    pub fn exports(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        names: &MacroNamespaces,
        builtin: &BuiltinAttributeCatalog,
        catalog: &super::procedural_exports::PreparedProcCatalog,
    ) -> Result<super::generated_derive_exports::GeneratedDeriveExports, Diagnostic> {
        self.revalidate(items, index, cfg, names, builtin)?;
        let plan = self.plan(items, index, cfg)?;
        super::generated_derive_exports::GeneratedDeriveExports::resolve(
            &plan,
            items,
            self.scope.clone(),
            names,
            catalog,
        )
    }
    pub fn helpers(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        names: &MacroNamespaces,
        builtin: &BuiltinAttributeCatalog,
        declarations: &super::generated_derive_helpers::GeneratedHelperDeclarations<'_>,
    ) -> Result<super::generated_derive_helpers::GeneratedHelperPoll, Diagnostic> {
        let exports = self.exports(items, index, cfg, names, builtin, declarations.catalog)?;
        let plan = self.plan(items, index, cfg)?;
        super::generated_derive_helpers::GeneratedDeriveHelpers::resolve(
            plan,
            items,
            index,
            &exports,
            declarations.catalog,
            declarations.receipts,
        )
    }
    pub fn plan(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
    ) -> Result<GeneratedDerivePlan, Diagnostic> {
        GeneratedDerivePlan::prepare(items, index, cfg, &self.proof)
    }
    /// Replay exact positive definitions; absence/pending cannot validate a selection.
    /// This validates annotation choices only, not helper visibility or catalog seals.
    pub fn revalidate(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        names: &MacroNamespaces,
        builtin: &BuiltinAttributeCatalog,
    ) -> Result<(), Diagnostic> {
        self.plan(items, index, cfg)?;
        let current = Self::resolve(items, index, cfg, self.scope.clone(), names, builtin)?;
        for (at, definition) in &self.positive {
            match current.candidates.get(at) {
                Some(PathLookup::Defined(found))
                    if found == definition && current.positive.get(at) == Some(definition) => {}
                Some(PathLookup::Ambiguous(_)) => {
                    return Err(fault(
                        "LRR-E-MACRO-AMBIGUOUS",
                        "completed generated derive annotation is now ambiguous",
                    ));
                }
                Some(PathLookup::Absent | PathLookup::NeedInput(_)) | None => {
                    return Err(fault(
                        "LRR-E-CATALOG-INCOMPLETE",
                        "completed generated derive annotation is now pending or absent",
                    ));
                }
                _ => {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "completed generated derive annotation identity changed",
                    ));
                }
            }
        }
        if current.positive != self.positive {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive annotation membership changed",
            ));
        }
        Ok(())
    }
}
