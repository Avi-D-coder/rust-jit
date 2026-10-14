//! Declaration subtree assembly for a coordinator-held discovery ticket.
//! Execution is external; a completed subtree is not a graph or impl seal.
use super::{
    budget::SharedExpansionBudget,
    generated_declarations::GeneratedDeclarations,
    record::ExpansionRecord,
    source_declarative::{SourceDeclarative, SourceDeclarativeDemand, SourceDeclarativeRequest},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    frontend::impl_index::{
        DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
        ImplHeaderIndex,
    },
    source::snapshot::CfgSet,
    syntax::declaration_scan::{DeclarationScan, DeclarationScanLimits},
};
use std::{collections::BTreeMap, sync::Arc};

pub struct DeclarationFragmentInput {
    pub invocation: Digest,
    pub request: Digest,
    pub graph: Digest,
    pub compile_env: Digest,
    pub engine: Digest,
    pub epoch: u64,
    pub cfg: CfgSet,
}
pub enum DeclarationFragmentStep {
    NeedAttributeSelection(
        Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
    ),
    NeedProceduralSelection(Arc<super::source_discovery::ProceduralSourceSelection>),
    NeedExpansion(Arc<SourceDeclarativeRequest>),
    NeedInput(Digest),
    Complete(Arc<CompletedDeclarationFragment>),
}
struct CompletedGeneratedArguments {
    output: Arc<GeneratedDeclarations>,
    invocation: Digest,
    definition: crate::expand::scopes::MacroDefinition,
    assembly: super::argument_discovery::ArgumentDiscovery,
}
struct ActiveGeneratedArguments {
    output: Arc<GeneratedDeclarations>,
    invocation: Digest,
    definition: crate::expand::scopes::MacroDefinition,
    assembly: Box<super::argument_discovery::ArgumentDiscovery>,
}
struct GeneratedItemReplacement {
    parent: Arc<GeneratedDeclarations>,
    invocation: Digest,
    child: Arc<GeneratedDeclarations>,
    range: crate::syntax::spans::SourceRange,
}
struct CompletedItemDerives {
    parent: Arc<GeneratedDeclarations>,
    invocation: Digest,
    receipt: Digest,
    outputs: Vec<Arc<GeneratedDeclarations>>,
}
struct ActiveItemDerives {
    proof: CompletedItemDerives,
    next: usize,
}
struct CompletedItemAttribute {
    output: Arc<GeneratedDeclarations>,
    invocation: Digest,
    definition: Digest,
    catalog: Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
}
struct GeneratedCompilerTestBinding {
    parent: Arc<GeneratedDeclarationMacroIndex>,
    definition: Option<crate::expand::scopes::MacroDefinition>,
}
struct GeneratedInertAttribute {
    output: Arc<GeneratedDeclarations>,
    invocation: Digest,
    definition: Digest,
    compiler_binding: Option<GeneratedCompilerTestBinding>,
    attribute_binding: Option<(Arc<GeneratedDeclarationMacroIndex>, Digest)>,
}
struct GeneratedIntrinsicArguments {
    output: Arc<GeneratedDeclarations>,
    invocation: Digest,
    definition: crate::expand::scopes::MacroDefinition,
    scans: Vec<Arc<DeclarationScan>>,
}
// A range excludes rows only from its exact owned capture, never by offsets alone.
struct ModuleReplacementRanges(BTreeMap<Digest, Vec<crate::syntax::spans::SourceRange>>);
impl ModuleReplacementRanges {
    fn new(masks: &[(Digest, crate::syntax::spans::SourceRange)]) -> Result<Self, Diagnostic> {
        if masks.len() > 100_000
            || masks
                .len()
                .checked_mul(128)
                .is_none_or(|n| n > 32 * 1024 * 1024)
        {
            return Err(limit());
        }
        let mut ranges = BTreeMap::<Digest, Vec<crate::syntax::spans::SourceRange>>::new();
        for (owner, range) in masks {
            if range.start >= range.end {
                return Err(limit());
            }
            let entries = ranges.entry(*owner).or_default();
            entries.try_reserve_exact(1).map_err(|_| limit())?;
            entries.push(*range);
        }
        for entries in ranges.values_mut() {
            entries.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
            let mut merged: Vec<crate::syntax::spans::SourceRange> = Vec::new();
            merged
                .try_reserve_exact(entries.len())
                .map_err(|_| limit())?;
            for range in entries.iter() {
                if let Some(last) = merged.last_mut()
                    && range.start <= last.end
                {
                    last.end = last.end.max(range.end);
                } else {
                    merged.push(*range);
                }
            }
            *entries = merged;
        }
        Ok(Self(ranges))
    }
    fn covers(&self, scan: Digest, range: crate::syntax::spans::SourceRange) -> bool {
        self.0.get(&scan).is_some_and(|ranges| {
            let at = ranges.partition_point(|r| r.start <= range.start);
            range.start < range.end && at > 0 && range.end <= ranges[at - 1].end
        })
    }
}
pub struct CompletedDeclarationFragment {
    source: Arc<DeclarationScan>,
    index: Arc<GeneratedDeclarationMacroIndex>,
    // Move completed child indexes here rather than discarding their owned facts.
    completed_indexes: Vec<Arc<GeneratedDeclarationMacroIndex>>,
    inventory: ImplHeaderIndex,
    records: Vec<Arc<ExpansionRecord>>,
    bound_procedural: Vec<(
        Arc<super::source_discovery::ProceduralSourceSelection>,
        crate::expand::identity::ExpansionKey,
    )>,
    bound_attributes: Vec<(
        Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
        crate::expand::identity::ExpansionKey,
    )>,
    retained_bytes: usize,
    inert_attributes: Vec<GeneratedInertAttribute>,
    item_attributes: Vec<CompletedItemAttribute>,
    item_derives: Vec<CompletedItemDerives>,
    item_replacements: Vec<GeneratedItemReplacement>,
    item_owner: Option<Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>>,
    intrinsic_arguments: Vec<GeneratedIntrinsicArguments>,
    completed_arguments: Vec<CompletedGeneratedArguments>,
}
impl std::fmt::Debug for CompletedDeclarationFragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedDeclarationFragment")
            .field("source", &self.source.fingerprint())
            .field("output", &self.index.output().fingerprint())
            .field("records", &self.records.len())
            .finish()
    }
}
impl CompletedDeclarationFragment {
    pub(crate) fn source_scan(&self) -> &DeclarationScan {
        &self.source
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn revalidate_procedural_selections(
        &self,
        provider: &SourceDeclarative,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        for (selection, _) in &self.bound_attributes {
            selection.revalidate_generated(provider, namespaces, owners)?;
        }
        for proof in &self.inert_attributes {
            if proof.attribute_binding.is_none() && provider.procedural.is_some() {
                let name = proof
                    .output
                    .scan()
                    .producers()
                    .iter()
                    .find(|row| row.identity == proof.invocation)
                    .and_then(|row| row.unqualified_attribute.as_deref())
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated inert facet source annotation absent",
                        )
                    })?;
                if super::builtin_attributes::inert_descriptor(name)?.is_some() {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "compiler-only scheduling proof lacks procedural namespace facet",
                    ));
                }
            }
            if let Some((parent, expected)) = &proof.attribute_binding {
                let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::generated_lookup(
                    parent, proof.invocation, namespaces, provider.module_source().captured_macro_cfg()?,
                )?;
                let name = parent
                    .output()
                    .scan()
                    .producers()
                    .iter()
                    .find(|row| row.identity == proof.invocation)
                    .and_then(|row| row.unqualified_attribute.as_deref())
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated inert facet annotation absent",
                        )
                    })?;
                let actual = super::builtin_attributes::certify_inert_binding(
                    name,
                    &lookup,
                    provider.procedural.as_deref(),
                    provider
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "generated inert facet incomplete before sealing",
                    )
                })?;
                if actual != *expected {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "generated inert facet changed before sealing",
                    ));
                }
            }
            if let Some(binding) = &proof.compiler_binding {
                let current=crate::service::source_attribute_requests::SourceAttributeSelection::generated_lookup(
                    &binding.parent,proof.invocation,namespaces,provider.module_source().captured_macro_cfg()?)?;
                let current = match current {
                    crate::frontend::macro_names::PathLookup::Absent => None,
                    crate::frontend::macro_names::PathLookup::Defined(definition) => {
                        Some(definition)
                    }
                    _ => {
                        return Err(fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated compiler test binding incomplete",
                        ));
                    }
                };
                if current != binding.definition {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "generated compiler test binding changed",
                    ));
                }
                if let Some(definition) = &current
                    && !super::builtin_attributes::permits_builtin_test_marker(
                        definition,
                        provider.procedural.as_deref(),
                        provider
                            .compiler
                            .as_ref()
                            .and_then(|c| c.attribute_catalog()),
                    )?
                {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "generated compiler Test facet changed",
                    ));
                }
            }
        }
        for (selection, _) in &self.bound_procedural {
            selection.revalidate_generated(provider, namespaces, owners)?;
        }
        for summary in &self.completed_arguments {
            for fragment in &summary.assembly.completed {
                fragment.revalidate_procedural_selections(provider, namespaces, owners)?;
            }
        }
        Ok(())
    }
    pub(crate) fn intrinsic_fingerprint(&self) -> Digest {
        let mut e = crate::core::digest::StableEncoder::new(
            "lrr/v2/generated-intrinsic-summaries-with-module-indexes-and-inert-facets",
        );
        e.sequence(&self.completed_indexes, |e, index| {
            e.digest(index.output().fingerprint());
        });
        e.sequence(&self.bound_procedural, |e, (_, key)| e.record(key));
        e.sequence(&self.bound_attributes, |e, (selection, key)| {
            e.digest(selection.fingerprint());
            e.record(key);
        });
        e.bool(self.item_owner.is_some());
        if let Some(owner) = &self.item_owner {
            e.digest(owner.record_identity());
        }
        e.sequence(&self.item_replacements, |e, proof| {
            e.digest(proof.parent.fingerprint());
            e.digest(proof.invocation);
            e.digest(proof.child.fingerprint());
            e.u32(proof.range.start);
            e.u32(proof.range.end);
        });
        e.sequence(&self.item_derives, |e, proof| {
            e.digest(proof.parent.fingerprint());
            e.digest(proof.invocation);
            e.digest(proof.receipt);
            e.sequence(&proof.outputs, |e, output| e.digest(output.fingerprint()));
        });
        e.sequence(&self.item_attributes, |e, proof| {
            e.digest(proof.output.fingerprint());
            e.digest(proof.invocation);
            e.digest(proof.definition);
            e.digest(proof.catalog.record_identity());
        });
        e.sequence(&self.inert_attributes, |e, proof| {
            e.digest(proof.output.fingerprint());
            e.digest(proof.invocation);
            e.digest(proof.definition);
            e.option(&proof.attribute_binding, |e, (parent, binding)| {
                e.digest(parent.output().fingerprint());
                e.digest(*binding);
            });
            e.option(&proof.compiler_binding, |e, binding| {
                e.digest(binding.parent.output().fingerprint());
                e.option(&binding.definition, |e, definition| {
                    e.digest(definition.identity);
                    e.digest(definition.defining_unit);
                });
            });
        });
        e.u64(u64::try_from(self.intrinsic_arguments.len()).expect("bounded intrinsic summaries"));
        for summary in &self.intrinsic_arguments {
            e.digest(summary.output.fingerprint());
            e.digest(summary.invocation);
            e.digest(summary.definition.identity);
            e.sequence(&summary.scans, |e, scan| e.digest(scan.fingerprint()));
        }
        e.u64(
            u64::try_from(self.completed_arguments.len())
                .expect("bounded completed generated arguments"),
        );
        for summary in &self.completed_arguments {
            e.digest(summary.output.fingerprint());
            e.digest(summary.invocation);
            e.digest(summary.definition.identity);
            e.sequence(&summary.assembly.arguments, |e, a| e.record(a.as_ref()));
            e.sequence(&summary.assembly.indexes, |e, index| {
                e.digest(index.inert_fingerprint())
            });
            for fragment in &summary.assembly.completed {
                e.digest(fragment.index().output().fingerprint());
                e.digest(fragment.intrinsic_fingerprint());
            }
        }
        e.finish()
    }
    pub(crate) fn validate_intrinsic_arguments(&self, cfg: &CfgSet) -> Result<(), Diagnostic> {
        if let Some(owner) = &self.item_owner
            && owner.record_identity() != self.index.output().record().key().fingerprint()
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "completed item owner record differs",
            ));
        }

        if self.bound_attributes.len() > 4096 {
            return Err(limit());
        }
        for (selection, key) in &self.bound_attributes {
            let parent = selection.generated_parent().ok_or_else(limit)?.output();
            if key.invocation != selection.producer()
                || key.procedural_epoch != Some(self.index.output().epoch())
                || !matches!(
                    key.macro_identity,
                    crate::expand::identity::MacroIdentity::Procedural {
                        kind: crate::expand::identity::ProceduralKind::Attribute,
                        ..
                    }
                )
                || !self.records.iter().any(|r| r.key() == key)
                || !self.records.iter().any(|r| {
                    r.descriptor().fingerprint() == parent.record().descriptor().fingerprint()
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated cold attribute receipt differs",
                ));
            }
        }
        if self.bound_procedural.len() > 4096 {
            return Err(limit());
        }
        for (selection, key) in &self.bound_procedural {
            let parent = selection.input().captured_parent().ok_or_else(limit)?;
            let origin_valid = match parent {
                crate::service::function_macro_requests::CapturedDeclarationParent::Generated(
                    index,
                ) => self.records.iter().any(|record| {
                    record.key() == index.output().record().key()
                        && record.descriptor().fingerprint()
                            == index.output().record().descriptor().fingerprint()
                }),
                crate::service::function_macro_requests::CapturedDeclarationParent::Argument(
                    index,
                ) => {
                    matches!(&self.index.parent(), crate::frontend::impl_index::DeclarationMacroParent::Argument(actual) if actual.argument().fingerprint()==index.argument().fingerprint())
                }
            };
            if !selection.validates_binding(key)?
                || key.procedural_epoch != Some(self.index.output().epoch())
                || parent.cfg() != cfg.fingerprint()
                || !self.records.iter().any(|record| record.key() == key)
                || !origin_valid
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated procedural binding is absent or differs",
                ));
            }
        }
        if self.item_replacements.len() > 4096 {
            return Err(limit());
        }
        for proof in &self.item_replacements {
            let owner = self.item_owner.as_ref().ok_or_else(limit)?;
            let Some((child, range)) =
                owner.completed_declaration_attribute(&proof.parent, proof.invocation)?
            else {
                return Err(limit());
            };
            if !self.records.iter().any(|r| {
                r.descriptor().fingerprint() == proof.parent.record().descriptor().fingerprint()
            }) || child.fingerprint() != proof.child.fingerprint()
                || range != proof.range
                || proof.parent.epoch() != self.index.output().epoch()
                || proof.parent.scan().cfg() != cfg.fingerprint()
                || !self.records.iter().any(|r| {
                    r.descriptor().fingerprint() == child.record().descriptor().fingerprint()
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated replacement receipt differs",
                ));
            }
        }
        if self.item_derives.len() > 4096 {
            return Err(limit());
        }
        for proof in &self.item_derives {
            let owner = self.item_owner.as_ref().ok_or_else(limit)?;
            let Some((receipt, outputs)) =
                owner.completed_declaration_derives(&proof.parent, proof.invocation)?
            else {
                return Err(limit());
            };
            if receipt != proof.receipt
                || outputs.len() != proof.outputs.len()
                || outputs
                    .iter()
                    .zip(&proof.outputs)
                    .any(|(a, b)| a.fingerprint() != b.fingerprint())
                || !self.records.iter().any(|r| {
                    r.descriptor().fingerprint() == proof.parent.record().descriptor().fingerprint()
                })
                || outputs.iter().any(|o| {
                    !self.records.iter().any(|r| {
                        r.descriptor().fingerprint() == o.record().descriptor().fingerprint()
                    })
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "derive declaration append receipt differs",
                ));
            }
        }
        if self.item_attributes.len() > 100_000 {
            return Err(limit());
        }
        for proof in &self.item_attributes {
            if proof.output.epoch() != self.index.output().epoch()
                || proof.output.scan().cfg() != cfg.fingerprint()
                || !self.records.iter().any(|record| {
                    record.descriptor().fingerprint()
                        == proof.output.record().descriptor().fingerprint()
                })
                || proof
                    .catalog
                    .completed_helper_declaration(&proof.output, proof.invocation)?
                    != Some(proof.definition)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "completed item attribute receipt differs",
                ));
            }
        }
        if self.inert_attributes.len() > 100_000 {
            return Err(limit());
        }
        for proof in &self.inert_attributes {
            if proof.output.epoch() != self.index.output().epoch()
                || proof.output.scan().cfg() != cfg.fingerprint()
                || !self.records.iter().any(|record| {
                    record.key() == proof.output.record().key()
                        && record.descriptor().fingerprint()
                            == proof.output.record().descriptor().fingerprint()
                })
                || proof
                    .output
                    .inert_attribute_definition(proof.invocation, cfg)?
                    != Some(proof.definition)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated inert attribute source/compiler record differs",
                ));
            }
        }
        if self.intrinsic_arguments.len() > 100_000 {
            return Err(limit());
        }
        for summary in &self.intrinsic_arguments {
            if !self.records.iter().any(|record| {
                record.key() == summary.output.record().key()
                    && record.descriptor().fingerprint()
                        == summary.output.record().descriptor().fingerprint()
            }) || summary.output.epoch() != self.index.output().epoch()
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated intrinsic output not in fragment",
                ));
            }
            let profile = &summary.output.scan().parse().profile;
            let catalog =
                crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(
                    profile,
                    &summary.definition,
                )?;
            let Some(kind) = catalog.select(&summary.definition)? else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated intrinsic compiler definition differs",
                ));
            };
            let scans = summary
                .output
                .intrinsic_argument_scans(summary.invocation, cfg, kind)?;
            if scans.len() != summary.scans.len()
                || scans.iter().zip(&summary.scans).any(|(actual, retained)| {
                    !retained.producers().is_empty()
                        || actual.fingerprint() != retained.fingerprint()
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated intrinsic argument inventories differ",
                ));
            }
        }
        for summary in &self.completed_arguments {
            let arguments = summary.output.intrinsic_arguments(
                summary.invocation,
                summary.definition.clone(),
                cfg,
            )?;
            if arguments.len() != summary.assembly.arguments.len()
                || summary.assembly.indexes.len() != arguments.len()
                || !self
                    .records
                    .iter()
                    .any(|r| r.key() == summary.output.record().key())
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated argument completion source differs",
                ));
            }
            for (actual, index) in arguments.iter().zip(&summary.assembly.indexes) {
                if !index.completed_for(actual) {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "generated argument index differs",
                    ));
                }
                for producer in actual.scan().producers() {
                    if index.validates_inert(actual.scan(), producer.identity)? {
                        continue;
                    }
                    if !summary.assembly.completed.iter().any(|fragment| {
                        fragment.index().is_complete()
                            && fragment.index().output().parent_inventory()
                                == actual.scan().fingerprint()
                            && fragment.index().output().record().key().invocation
                                == producer.identity
                    }) {
                        return Err(fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated argument producer completion absent",
                        ));
                    }
                }
            }
            for fragment in &summary.assembly.completed {
                fragment.validate_intrinsic_arguments(cfg)?;
                for record in fragment.records() {
                    if !self.records.iter().any(|r| {
                        r.key() == record.key()
                            && r.descriptor().fingerprint() == record.descriptor().fingerprint()
                    }) {
                        return Err(fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated argument child record absent",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn index(&self) -> &Arc<GeneratedDeclarationMacroIndex> {
        &self.index
    }
    /// Exact owned module facts from every completed expansion in this subtree.
    /// Scheduling completion grants no annotation proof, route or namespace seal.
    /// Removed source rows are masked only in their matching capture scan.
    pub fn module_bindings(
        &self,
        expected_epoch: u64,
    ) -> Result<Vec<crate::frontend::macro_names::DeclarationModuleBinding>, Diagnostic> {
        if expected_epoch == 0 || self.index.output().epoch() != expected_epoch {
            return Err(fault(
                "LRR-E-EXPANSION-EPOCH",
                "completed module binding epoch differs",
            ));
        }
        let mut bindings = Vec::new();
        let mut pending = vec![self];
        let mut traversed = 0usize;
        let mut bytes = 0usize;
        while let Some(fragment) = pending.pop() {
            let ranges = ModuleReplacementRanges::new(&fragment.generated_replacement_masks()?)?;
            for index in std::iter::once(&fragment.index).chain(&fragment.completed_indexes) {
                traversed = traversed
                    .checked_add(1)
                    .filter(|n| *n <= 4096)
                    .ok_or_else(limit)?;
                let rows = index.module_bindings(expected_epoch)?;
                let scan = index.output().scan();
                if rows.len() != scan.modules().len() {
                    return Err(limit());
                }
                for (binding, row) in rows.into_iter().zip(scan.modules()) {
                    let masked = ranges.covers(scan.fingerprint(), row.range);
                    if masked {
                        continue;
                    }
                    if bindings.len() >= 100_000 {
                        return Err(limit());
                    }
                    bytes = bytes
                        .checked_add(binding.metadata_bytes()?)
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(limit)?;
                    bindings.try_reserve_exact(1).map_err(|_| limit())?;
                    bindings.push(binding);
                }
            }
            for argument in &fragment.completed_arguments {
                pending
                    .len()
                    .checked_add(argument.assembly.completed.len())
                    .filter(|n| *n <= 4096)
                    .ok_or_else(limit)?;
                pending
                    .try_reserve_exact(argument.assembly.completed.len())
                    .map_err(|_| limit())?;
                pending.extend(argument.assembly.completed.iter().map(Arc::as_ref));
            }
        }
        Ok(bindings)
    }
    pub fn inventory(&self) -> &ImplHeaderIndex {
        &self.inventory
    }
    pub(crate) fn generated_replacement_masks(
        &self,
    ) -> Result<Vec<(Digest, crate::syntax::spans::SourceRange)>, Diagnostic> {
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(
                self.item_replacements
                    .len()
                    .checked_add(self.bound_attributes.len())
                    .ok_or_else(limit)?,
            )
            .map_err(|_| limit())?;
        masks.extend(
            self.item_replacements
                .iter()
                .map(|p| (p.parent.scan().fingerprint(), p.range)),
        );
        for (selection, _) in &self.bound_attributes {
            masks.push((
                selection
                    .generated_parent()
                    .ok_or_else(limit)?
                    .output()
                    .scan()
                    .fingerprint(),
                selection.item_range(),
            ));
        }
        Ok(masks)
    }
    pub fn records(&self) -> &[Arc<ExpansionRecord>] {
        &self.records
    }
}
pub struct DeclarationFragment {
    parent: Arc<DeclarationMacroIndex>,
    root_parent: DeclarationMacroParent,
    epoch: u64,
    cfg: CfgSet,
    frames: Vec<GeneratedDeclarationMacroIndex>,
    completed_indexes: Vec<Arc<GeneratedDeclarationMacroIndex>>,
    pending: Option<Arc<SourceDeclarativeRequest>>,
    pending_procedural: Option<Arc<super::source_discovery::ProceduralSourceSelection>>,
    pending_attribute:
        Option<Arc<crate::service::source_attribute_requests::SourceAttributeSelection>>,
    bound_procedural: Vec<(
        Arc<super::source_discovery::ProceduralSourceSelection>,
        crate::expand::identity::ExpansionKey,
    )>,
    bound_attributes: Vec<(
        Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
        crate::expand::identity::ExpansionKey,
    )>,
    scans: Vec<Arc<DeclarationScan>>,
    records: Vec<Arc<ExpansionRecord>>,
    budget: SharedExpansionBudget,
    retained_bytes: usize,
    complete: Option<Arc<CompletedDeclarationFragment>>,
    inert_attributes: Vec<GeneratedInertAttribute>,
    item_attributes: Vec<CompletedItemAttribute>,
    item_derives: Vec<CompletedItemDerives>,
    active_derives: Vec<ActiveItemDerives>,
    item_replacements: Vec<GeneratedItemReplacement>,
    item_owner: Option<Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>>,
    intrinsic_arguments: Vec<GeneratedIntrinsicArguments>,
    active_arguments: Option<ActiveGeneratedArguments>,
    completed_arguments: Vec<CompletedGeneratedArguments>,
    run: DeclarationFragmentInput,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn limit() -> Diagnostic {
    fault(
        "LRR-E-DISCOVERY-LIMIT",
        "nested declaration fragment metadata/count limit",
    )
}
pub(crate) struct BlockedMacroSite<'a> {
    pub name: &'a str,
    pub invocation: Digest,
    pub expansion: Digest,
    pub span: &'a crate::syntax::token_wire::SpanRecord,
}
impl DeclarationFragment {
    /// Diagnostic facts from already admitted frames. This never resolves a
    /// name, expands a producer, changes a frontier or prepares a body.
    pub(crate) fn blocked_macro_site(
        &self,
        input: Digest,
    ) -> Result<Option<BlockedMacroSite<'_>>, Diagnostic> {
        for frame in self.frames.iter().rev() {
            let output = frame.output();
            let Some(producer) = output.scan().producers().iter().find(|producer| {
                producer.identity == input
                    && producer.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            }) else {
                continue;
            };
            let capture = output.macro_call(input)?;
            // Admission checked this exact pinned span-map key against the
            // original record; reconstructed path tokens are not that map.
            let name_span = capture.name_span.0;
            let span = output
                .tokens()
                .spans()
                .iter()
                .find(|span| span.key().0 == name_span)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SOURCE-MAP",
                        "blocked generated call site absent",
                    )
                })?;
            let name = producer
                .path
                .last()
                .filter(|token| {
                    crate::syntax::ast_adapter::macro_name(&token.text, capture.edition).is_some()
                })
                .map(|token| token.text.as_str())
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SOURCE-MAP",
                        "blocked generated macro name absent",
                    )
                })?;
            return Ok(Some(BlockedMacroSite {
                name,
                invocation: output.record().key().invocation,
                expansion: output.record().key().fingerprint(),
                span,
            }));
        }
        Ok(None)
    }

    pub fn new(
        provider: &SourceDeclarative,
        parent: Arc<DeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
    ) -> Result<Self, Diagnostic> {
        if input.epoch == 0
            || input.cfg.fingerprint() != parent.scan().cfg()
            || !budget.matches(input.request, input.graph, input.epoch)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "declaration fragment epoch/cfg differs",
            ));
        }
        let demand = provider.declaration_demand(
            &parent,
            input.invocation,
            input.compile_env,
            input.engine,
        )?;
        let SourceDeclarativeDemand::Ready(request) = demand else {
            return Err(fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "root declaration producer requires input",
            ));
        };
        Self::from_prepared_source(parent, input, budget, Arc::from(request))
    }
    pub(crate) fn from_prepared_source(
        parent: Arc<DeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
        request: Arc<SourceDeclarativeRequest>,
    ) -> Result<Self, Diagnostic> {
        if input.epoch == 0
            || input.cfg.fingerprint() != parent.scan().cfg()
            || !budget.matches(input.request, input.graph, input.epoch)
            || request.key().invocation != input.invocation
            || request.key().grammar != parent.producer(input.invocation)?.grammar
            || request.key().consumer_compile_env != input.compile_env
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "prepared declaration root context differs",
            ));
        }
        budget.freeze()?;
        let run = DeclarationFragmentInput {
            invocation: input.invocation,
            request: input.request,
            graph: input.graph,
            compile_env: input.compile_env,
            engine: input.engine,
            epoch: input.epoch,
            cfg: input.cfg.clone(),
        };
        Ok(Self {
            run,
            active_arguments: None,
            completed_arguments: Vec::new(),
            root_parent: DeclarationMacroParent::Source(parent.clone()),
            parent,
            epoch: input.epoch,
            cfg: input.cfg,
            frames: Vec::new(),
            completed_indexes: Vec::new(),
            pending: Some(request),
            pending_procedural: None,
            pending_attribute: None,
            bound_procedural: Vec::new(),
            bound_attributes: Vec::new(),
            scans: Vec::new(),
            records: Vec::new(),
            budget,
            retained_bytes: 0,
            complete: None,
            inert_attributes: Vec::new(),
            item_attributes: Vec::new(),
            item_derives: Vec::new(),
            active_derives: Vec::new(),
            item_replacements: Vec::new(),
            item_owner: None,
            intrinsic_arguments: Vec::new(),
        })
    }
    /// Adopt an already admitted item record into declaration-only traversal.
    /// The immutable record shares the same budget ledger with ordinary items.
    pub fn from_item_output(
        parent: Arc<DeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
        output: Arc<GeneratedDeclarations>,
    ) -> Result<Self, Diagnostic> {
        if input.epoch == 0
            || output.epoch() != input.epoch
            || input.cfg.fingerprint() != parent.scan().cfg()
            || !budget.matches(input.request, input.graph, input.epoch)
            || output.parent_inventory() != parent.scan().fingerprint()
            || output.record().key().invocation != input.invocation
            || output.record().key().grammar != crate::expand::identity::GrammarSlot::Items
            || output.record().key().consumer_compile_env != input.compile_env
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "item declaration traversal attachment differs",
            ));
        }
        let root_parent = DeclarationMacroParent::Source(parent.clone());
        let index = GeneratedDeclarationMacroIndex::new(output.clone(), root_parent.clone())?;
        let record = output.record_handle();
        let mut retained_bytes = output
            .metadata_bytes()?
            .checked_add(index.retained_metadata_bytes())
            .and_then(|n| n.checked_add(record.wire().len().checked_mul(2)?))
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        let mut frames = Vec::new();
        frames.try_reserve_exact(1).map_err(|_| limit())?;
        let mut scans = Vec::new();
        scans.try_reserve_exact(1).map_err(|_| limit())?;
        let mut records = Vec::new();
        records.try_reserve_exact(1).map_err(|_| limit())?;
        retained_bytes = retained_bytes
            .checked_add(
                frames
                    .capacity()
                    .checked_mul(std::mem::size_of::<GeneratedDeclarationMacroIndex>())
                    .ok_or_else(limit)?,
            )
            .and_then(|n| {
                n.checked_add(
                    scans
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<DeclarationScan>>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    records
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<ExpansionRecord>>())?,
                )
            })
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        budget.freeze()?;
        budget.transaction(
            &BTreeMap::from([(record.key().fingerprint(), record.usage())]),
            || Ok(()),
        )?;
        frames.push(index);
        scans.push(output.scan().clone());
        records.push(record);
        Ok(Self {
            run: DeclarationFragmentInput {
                invocation: input.invocation,
                request: input.request,
                graph: input.graph,
                compile_env: input.compile_env,
                engine: input.engine,
                epoch: input.epoch,
                cfg: input.cfg.clone(),
            },
            active_arguments: None,
            completed_arguments: Vec::new(),
            root_parent,
            parent,
            epoch: input.epoch,
            cfg: input.cfg,
            frames,
            completed_indexes: Vec::new(),
            pending: None,
            pending_procedural: None,
            pending_attribute: None,
            bound_procedural: Vec::new(),
            bound_attributes: Vec::new(),
            scans,
            records,
            budget,
            retained_bytes,
            complete: None,
            inert_attributes: Vec::new(),
            item_attributes: Vec::new(),
            item_derives: Vec::new(),
            active_derives: Vec::new(),
            item_replacements: Vec::new(),
            item_owner: None,
            intrinsic_arguments: Vec::new(),
        })
    }
    pub(crate) fn with_completed_item_owner(
        mut self,
        catalog: Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
    ) -> Result<Self, Diagnostic> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(catalog.metadata_bytes()?)
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        self.item_owner = Some(catalog.clone());
        let frame = self.frames.last().ok_or_else(limit)?;
        let mut candidates = Vec::new();
        let count = frame.output().scan().producers().len();
        count
            .checked_mul(std::mem::size_of::<Digest>())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(limit)?;
        candidates.try_reserve_exact(count).map_err(|_| limit())?;
        candidates.extend(
            frame
                .output()
                .scan()
                .producers()
                .iter()
                .filter(|p| {
                    p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                })
                .map(|p| p.identity),
        );
        let mut updated = frame.clone();
        for invocation in candidates {
            if let Some(definition) =
                catalog.completed_helper_declaration(updated.output(), invocation)?
            {
                let bytes = self
                    .retained_bytes
                    .checked_add(std::mem::size_of::<CompletedItemAttribute>())
                    .and_then(|n| n.checked_add(catalog.metadata_bytes().ok()?))
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                self.item_attributes
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                updated.retire_completed_item_attribute(invocation, &catalog)?;
                self.item_attributes.push(CompletedItemAttribute {
                    output: updated.output().clone(),
                    invocation,
                    definition,
                    catalog: catalog.clone(),
                });
                self.retained_bytes = bytes;
            }
        }
        *self.frames.last_mut().ok_or_else(limit)? = updated;
        Ok(self)
    }
    pub fn from_argument(
        provider: &SourceDeclarative,
        parent: Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
    ) -> Result<Self, Diagnostic> {
        if input.epoch == 0
            || input.cfg.fingerprint() != parent.argument().scan().cfg()
            || !budget.matches(input.request, input.graph, input.epoch)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "argument fragment epoch/cfg/run differs",
            ));
        }
        let SourceDeclarativeDemand::Ready(request) = provider.argument_declaration_demand(
            &parent,
            input.invocation,
            input.compile_env,
            input.engine,
        )?
        else {
            return Err(fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "argument fragment requires input",
            ));
        };
        Self::from_prepared_argument_inner(parent, input, budget, Arc::from(request), None)
    }
    pub(crate) fn from_prepared_argument(
        parent: Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
        request: Arc<SourceDeclarativeRequest>,
        selection: Arc<super::source_discovery::ProceduralSourceSelection>,
    ) -> Result<Self, Diagnostic> {
        if !selection.validates_binding(request.key())? {
            return Err(limit());
        }
        Self::from_prepared_argument_inner(parent, input, budget, request, Some(selection))
    }
    pub(crate) fn from_compiler_argument(
        parent: Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
        request: Arc<SourceDeclarativeRequest>,
    ) -> Result<Self, Diagnostic> {
        if !request.is_compiler() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "argument request lacks exact compiler facet",
            ));
        }
        Self::from_prepared_argument_inner(parent, input, budget, request, None)
    }
    fn from_prepared_argument_inner(
        parent: Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        input: DeclarationFragmentInput,
        budget: SharedExpansionBudget,
        request: Arc<SourceDeclarativeRequest>,
        selection: Option<Arc<super::source_discovery::ProceduralSourceSelection>>,
    ) -> Result<Self, Diagnostic> {
        if input.epoch == 0
            || input.cfg.fingerprint() != parent.argument().scan().cfg()
            || !budget.matches(input.request, input.graph, input.epoch)
            || request.key().invocation != input.invocation
            || request.key().grammar
                != parent
                    .argument()
                    .scan()
                    .producers()
                    .iter()
                    .find(|producer| producer.identity == input.invocation)
                    .ok_or_else(limit)?
                    .grammar
            || request.key().consumer_compile_env != input.compile_env
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "prepared argument context differs",
            ));
        }
        let mut bound_procedural = Vec::new();
        let mut retained_bytes = 0;
        if let Some(selection) = selection {
            let export_bytes = match &request.key().macro_identity {
                crate::expand::identity::MacroIdentity::Procedural { export, .. } => {
                    export.capacity()
                }
                _ => return Err(limit()),
            };
            bound_procedural.try_reserve_exact(1).map_err(|_| limit())?;
            retained_bytes =
                selection
                    .metadata_bytes()?
                    .checked_add(export_bytes)
                    .and_then(|n| {
                        n.checked_add(bound_procedural.capacity().checked_mul(
                            std::mem::size_of::<(
                                Arc<super::source_discovery::ProceduralSourceSelection>,
                                crate::expand::identity::ExpansionKey,
                            )>(),
                        )?)
                    })
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
            bound_procedural.push((selection, request.key().clone()));
        }
        budget.freeze()?;
        let run = DeclarationFragmentInput {
            invocation: input.invocation,
            request: input.request,
            graph: input.graph,
            compile_env: input.compile_env,
            engine: input.engine,
            epoch: input.epoch,
            cfg: input.cfg.clone(),
        };
        Ok(Self {
            run,
            active_arguments: None,
            completed_arguments: Vec::new(),
            parent: parent.source_index().clone(),
            root_parent: DeclarationMacroParent::Argument(parent),
            epoch: input.epoch,
            cfg: input.cfg,
            frames: Vec::new(),
            completed_indexes: Vec::new(),
            bound_procedural,
            bound_attributes: Vec::new(),
            pending: Some(request),
            pending_procedural: None,
            pending_attribute: None,

            scans: Vec::new(),
            records: Vec::new(),
            budget,
            retained_bytes,
            complete: None,
            inert_attributes: Vec::new(),
            item_attributes: Vec::new(),
            item_derives: Vec::new(),
            active_derives: Vec::new(),
            item_replacements: Vec::new(),
            item_owner: None,
            intrinsic_arguments: Vec::new(),
        })
    }
    pub(crate) fn bind_attribute_request(
        &mut self,
        request: Arc<SourceDeclarativeRequest>,
    ) -> Result<(), Diagnostic> {
        if let Some(arguments) = &mut self.active_arguments {
            return arguments.assembly.bind_attribute_request(request);
        }
        let selection = self.pending_attribute.as_ref().ok_or_else(limit)?;
        if request.key().invocation != selection.producer()
            || request.procedural_request().is_none()
            || request.key().grammar != selection.grammar()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "bound generated attribute request differs",
            ));
        }
        if self.bound_attributes.len() >= 4096 {
            return Err(limit());
        }
        let bytes = self
            .retained_bytes
            .checked_add(selection.metadata_bytes()?)
            .and_then(|n| n.checked_add(256))
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        self.bound_attributes.try_reserve(1).map_err(|_| limit())?;
        self.bound_attributes
            .push((selection.clone(), request.key().clone()));
        self.pending = Some(request);
        self.pending_attribute = None;
        self.retained_bytes = bytes;
        Ok(())
    }
    pub(crate) fn bind_procedural_request(
        &mut self,
        request: Arc<SourceDeclarativeRequest>,
    ) -> Result<(), Diagnostic> {
        if let Some(arguments) = &mut self.active_arguments {
            return arguments.assembly.bind_procedural_request(request);
        }
        let selection = self.pending_procedural.as_ref().ok_or_else(limit)?;
        if request.key().invocation != selection.input().invocation_identity()
            || request.key().grammar != selection.input().grammar()
            || request.procedural_request().is_none()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "bound generated procedural request differs",
            ));
        }
        if self.bound_procedural.len() >= 4096 || !selection.validates_binding(request.key())? {
            return Err(limit());
        }
        let export_bytes = match &request.key().macro_identity {
            crate::expand::identity::MacroIdentity::Procedural { export, .. } => export.capacity(),
            _ => return Err(limit()),
        };
        let bytes = self
            .retained_bytes
            .checked_add(export_bytes)
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<(
                    Arc<super::source_discovery::ProceduralSourceSelection>,
                    crate::expand::identity::ExpansionKey,
                )>())
            })
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        self.bound_procedural
            .try_reserve_exact(1)
            .map_err(|_| limit())?;
        self.retained_bytes = bytes;
        self.bound_procedural
            .push((selection.clone(), request.key().clone()));
        self.pending = Some(request);
        self.pending_procedural = None;
        Ok(())
    }
    pub(crate) fn pending_request(&self) -> Option<&Arc<SourceDeclarativeRequest>> {
        self.pending.as_ref().or_else(|| {
            self.active_arguments
                .as_ref()
                .and_then(|active| active.assembly.pending_request())
        })
    }
    /// Repeated polling returns the same immutable demand; it never expands it.
    pub fn poll(
        &mut self,
        provider: &SourceDeclarative,
    ) -> Result<DeclarationFragmentStep, Diagnostic> {
        self.poll_using(provider, None)
    }
    pub(crate) fn poll_with_namespaces(
        &mut self,
        provider: &SourceDeclarative,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<DeclarationFragmentStep, Diagnostic> {
        self.poll_using(provider, Some(namespaces))
    }
    fn poll_using(
        &mut self,
        provider: &SourceDeclarative,
        namespaces: Option<&crate::frontend::macro_names::MacroNamespaces>,
    ) -> Result<DeclarationFragmentStep, Diagnostic> {
        if let Some(complete) = &self.complete {
            return Ok(DeclarationFragmentStep::Complete(Arc::clone(complete)));
        }
        self.budget.check_deadline()?;
        if let Some(selection) = &self.pending_attribute {
            return Ok(DeclarationFragmentStep::NeedAttributeSelection(
                selection.clone(),
            ));
        }
        if let Some(selection) = &self.pending_procedural {
            return Ok(DeclarationFragmentStep::NeedProceduralSelection(
                selection.clone(),
            ));
        }
        if let Some(arguments) = &mut self.active_arguments {
            let namespaces = namespaces.ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated argument namespace absent",
                )
            })?;
            match arguments
                .assembly
                .poll(provider, &self.run, &self.budget, namespaces)?
            {
                super::argument_discovery::ArgumentDiscoveryStep::NeedAttributeSelection(
                    selection,
                ) => return Ok(DeclarationFragmentStep::NeedAttributeSelection(selection)),
                super::argument_discovery::ArgumentDiscoveryStep::NeedProceduralSelection(
                    selection,
                ) => return Ok(DeclarationFragmentStep::NeedProceduralSelection(selection)),
                super::argument_discovery::ArgumentDiscoveryStep::NeedExpansion(request) => {
                    return Ok(DeclarationFragmentStep::NeedExpansion(request));
                }
                super::argument_discovery::ArgumentDiscoveryStep::NeedInput(input) => {
                    return Ok(DeclarationFragmentStep::NeedInput(input));
                }
                super::argument_discovery::ArgumentDiscoveryStep::Complete => {
                    let args = self.active_arguments.as_ref().ok_or_else(limit)?;
                    let mut frame = self.frames.last().ok_or_else(limit)?.clone();
                    let invocation = args.invocation;
                    let scans = args
                        .assembly
                        .arguments
                        .iter()
                        .map(|a| a.scan().clone())
                        .collect::<Vec<_>>();
                    frame.complete_resolved_intrinsic_arguments(
                        invocation,
                        &args.assembly.indexes,
                    )?;
                    let mut bytes = self
                        .retained_bytes
                        .checked_add(args.assembly.metadata_bytes())
                        .filter(|n| *n <= 512 * 1024 * 1024)
                        .ok_or_else(limit)?;
                    let scan_count =
                        args.assembly
                            .completed
                            .iter()
                            .try_fold(scans.len(), |n, fragment| {
                                n.checked_add(fragment.inventory().scan_handles().len())
                                    .ok_or_else(limit)
                            })?;
                    let record_count = args
                        .assembly
                        .completed
                        .iter()
                        .try_fold(0usize, |n, fragment| {
                            n.checked_add(fragment.records().len()).ok_or_else(limit)
                        })?;
                    if self
                        .records
                        .len()
                        .checked_add(record_count)
                        .is_none_or(|n| n > 4096)
                    {
                        return Err(limit());
                    }
                    self.scans.try_reserve(scan_count).map_err(|_| limit())?;
                    self.records
                        .try_reserve(record_count)
                        .map_err(|_| limit())?;
                    self.completed_arguments
                        .try_reserve(1)
                        .map_err(|_| limit())?;
                    for fragment in &args.assembly.completed {
                        bytes = bytes
                            .checked_add(fragment.metadata_bytes())
                            .filter(|n| *n <= 512 * 1024 * 1024)
                            .ok_or_else(limit)?;
                    }
                    let args = self.active_arguments.take().ok_or_else(limit)?;
                    *self.frames.last_mut().ok_or_else(limit)? = frame;
                    self.scans.extend(scans);
                    for fragment in &args.assembly.completed {
                        self.scans
                            .extend(fragment.inventory().scan_handles().iter().cloned());
                        self.records.extend(fragment.records().iter().cloned());
                    }
                    self.retained_bytes = bytes;
                    self.completed_arguments.push(CompletedGeneratedArguments {
                        output: args.output,
                        invocation,
                        definition: args.definition,
                        assembly: *args.assembly,
                    });
                }
            }
        }
        if let Some(request) = &self.pending {
            return Ok(DeclarationFragmentStep::NeedExpansion(Arc::clone(request)));
        }
        loop {
            let frame = self.frames.last().ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "declaration fragment has no admitted root",
                )
            })?;
            if frame.is_complete() {
                if self.frames.len() == 1 {
                    // Preflight inventory construction before consuming the completed root.
                    let mut masks = Vec::new();
                    masks
                        .try_reserve_exact(
                            self.item_replacements
                                .len()
                                .checked_add(self.bound_attributes.len())
                                .ok_or_else(limit)?,
                        )
                        .map_err(|_| limit())?;
                    masks.extend(
                        self.item_replacements
                            .iter()
                            .map(|p| (p.parent.scan().fingerprint(), p.range)),
                    );
                    for (selection, _) in &self.bound_attributes {
                        masks.push((
                            selection
                                .generated_parent()
                                .ok_or_else(limit)?
                                .output()
                                .scan()
                                .fingerprint(),
                            selection.item_range(),
                        ));
                    }
                    let inventory = ImplHeaderIndex::from_scans(self.scans.clone())?
                        .exclude_generated_replacements(&masks)?;
                    let retained_bytes = self
                        .retained_bytes
                        .checked_add(inventory.metadata_bytes())
                        .filter(|n| *n <= 512 * 1024 * 1024)
                        .ok_or_else(limit)?;
                    let index = Arc::new(self.frames.pop().expect("root checked"));
                    let complete = Arc::new(CompletedDeclarationFragment {
                        source: self.parent.scan_handle().clone(),
                        retained_bytes,
                        index,
                        completed_indexes: std::mem::take(&mut self.completed_indexes),
                        inventory,
                        records: std::mem::take(&mut self.records),
                        bound_procedural: std::mem::take(&mut self.bound_procedural),
                        bound_attributes: std::mem::take(&mut self.bound_attributes),
                        inert_attributes: std::mem::take(&mut self.inert_attributes),
                        item_attributes: std::mem::take(&mut self.item_attributes),
                        item_derives: std::mem::take(&mut self.item_derives),
                        item_replacements: std::mem::take(&mut self.item_replacements),
                        item_owner: self.item_owner.take(),
                        intrinsic_arguments: std::mem::take(&mut self.intrinsic_arguments),
                        completed_arguments: std::mem::take(&mut self.completed_arguments),
                    });
                    self.complete = Some(Arc::clone(&complete));
                    return Ok(DeclarationFragmentStep::Complete(complete));
                }
                // Preflight retention before adoption mutates either frame. The
                // index's owned heaps were already charged when it was admitted;
                // moving it into an Arc adds only its container and pointer.
                if self.completed_indexes.len() >= 4096 {
                    return Err(limit());
                }
                let retention = std::mem::size_of::<GeneratedDeclarationMacroIndex>()
                    .checked_add(std::mem::size_of::<Arc<GeneratedDeclarationMacroIndex>>())
                    .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
                    .ok_or_else(limit)?;
                self.retained_bytes
                    .checked_add(retention)
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                self.completed_indexes
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                let child = self.frames.last().expect("child checked");
                let mut parent = self.frames[self.frames.len() - 2].clone();
                let deriving = self.active_derives.last().is_some_and(|a| {
                    a.proof.parent.fingerprint() == parent.output().fingerprint()
                        && a.proof
                            .outputs
                            .get(a.next - 1)
                            .is_some_and(|o| o.fingerprint() == child.output().fingerprint())
                });
                if deriving {
                    let active = self.active_derives.last().ok_or_else(limit)?;
                    let complete = active.next == active.proof.outputs.len();
                    parent.adopt_completed_child_prefix(
                        child.output().record().key(),
                        child,
                        complete,
                        Some(u32::try_from(active.next - 1).map_err(|_| limit())?),
                    )?;
                    if complete {
                        self.item_derives
                            .try_reserve_exact(1)
                            .map_err(|_| limit())?;
                        self.completed_indexes.push(Arc::new(
                            self.frames.pop().expect("completed child checked"),
                        ));
                        *self.frames.last_mut().ok_or_else(limit)? = parent;
                        let active = self.active_derives.pop().ok_or_else(limit)?;
                        self.item_derives.push(active.proof);
                    } else {
                        let output = active.proof.outputs[active.next].clone();
                        let index = GeneratedDeclarationMacroIndex::new(
                            output,
                            DeclarationMacroParent::Generated(Arc::new(
                                parent.derive_prefix_view(active.proof.invocation)?,
                            )),
                        )?;
                        let bytes = self
                            .retained_bytes
                            .checked_add(index.retained_metadata_bytes())
                            .and_then(|n| n.checked_add(retention))
                            .filter(|n| *n <= 512 * 1024 * 1024)
                            .ok_or_else(limit)?;
                        self.completed_indexes.push(Arc::new(
                            self.frames.pop().expect("completed child checked"),
                        ));
                        *self.frames.last_mut().ok_or_else(limit)? = parent;
                        self.frames.push(index);
                        self.retained_bytes = bytes.checked_sub(retention).ok_or_else(limit)?;
                        self.active_derives.last_mut().ok_or_else(limit)?.next += 1;
                    }
                } else {
                    parent.adopt_completed_child(child.output().record().key(), child)?;
                    self.completed_indexes.push(Arc::new(
                        self.frames.pop().expect("completed child checked"),
                    ));
                    *self.frames.last_mut().expect("parent checked") = parent;
                }
                self.retained_bytes = self
                    .retained_bytes
                    .checked_add(retention)
                    .ok_or_else(limit)?;
                continue;
            }
            // Ordinary item discovery has already executed these attributes under
            // authority. Retire replaced bodies before considering their cold calls.
            if let Some(owner) = &self.item_owner {
                let mut replacement = None;
                for p in frame.output().scan().producers().iter().filter(|p| {
                    p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                }) {
                    if self.item_replacements.iter().any(|r| {
                        r.parent.fingerprint() == frame.output().fingerprint()
                            && r.invocation == p.identity
                    }) {
                        continue;
                    }
                    if let Some((child, range)) =
                        owner.completed_declaration_attribute(frame.output(), p.identity)?
                    {
                        replacement = Some((p.identity, child, range));
                        break;
                    }
                }
                if let Some((invocation, output, range)) = replacement {
                    let mut parent = frame.clone();
                    parent.retire_replaced_generated_body(invocation, range)?;
                    let index = GeneratedDeclarationMacroIndex::new(
                        output.clone(),
                        DeclarationMacroParent::Generated(Arc::new(parent.clone())),
                    )?;
                    let record = output.record_handle();
                    let bytes = self
                        .retained_bytes
                        .checked_add(output.metadata_bytes()?)
                        .and_then(|n| n.checked_add(index.retained_metadata_bytes()))
                        .and_then(|n| n.checked_add(record.wire().len().checked_mul(2)?))
                        .and_then(|n| {
                            n.checked_add(std::mem::size_of::<GeneratedItemReplacement>())
                        })
                        .filter(|n| *n <= 512 * 1024 * 1024)
                        .ok_or_else(limit)?;
                    if self.frames.len() >= 256 || self.records.len() >= 4096 {
                        return Err(limit());
                    }
                    let parent_output = frame.output().clone();
                    self.frames.try_reserve_exact(1).map_err(|_| limit())?;
                    self.scans.try_reserve_exact(1).map_err(|_| limit())?;
                    self.records.try_reserve_exact(1).map_err(|_| limit())?;
                    self.item_replacements
                        .try_reserve_exact(1)
                        .map_err(|_| limit())?;
                    self.budget.transaction(
                        &BTreeMap::from([(record.key().fingerprint(), record.usage())]),
                        || Ok(()),
                    )?;
                    self.item_replacements.push(GeneratedItemReplacement {
                        parent: parent_output,
                        invocation,
                        child: output.clone(),
                        range,
                    });
                    *self.frames.last_mut().ok_or_else(limit)? = parent;
                    self.frames.push(index);
                    self.scans.push(output.scan().clone());
                    self.records.push(record);
                    self.retained_bytes = bytes;
                    continue;
                }
            }
            let invocation = frame.next_pending_producer().ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "fragment pending set and inventory disagree",
                )
            })?;
            if let Some(owner) = &self.item_owner
                && let Some((receipt, outputs)) =
                    owner.completed_declaration_derives(frame.output(), invocation)?
            {
                if self.active_derives.len() >= 256
                    || self.frames.len() >= 256
                    || self.item_derives.len() >= 4096
                    || self
                        .records
                        .len()
                        .checked_add(outputs.len())
                        .is_none_or(|n| n > 4096)
                {
                    return Err(limit());
                }
                let mut bytes = self
                    .retained_bytes
                    .checked_add(std::mem::size_of::<ActiveItemDerives>())
                    .and_then(|n| {
                        n.checked_add(
                            outputs
                                .capacity()
                                .checked_mul(std::mem::size_of::<Arc<GeneratedDeclarations>>())?,
                        )
                    })
                    .ok_or_else(limit)?;
                let index = GeneratedDeclarationMacroIndex::new(
                    outputs.first().ok_or_else(limit)?.clone(),
                    DeclarationMacroParent::Generated(Arc::new(
                        frame.derive_prefix_view(invocation)?,
                    )),
                )?;
                bytes = bytes
                    .checked_add(index.retained_metadata_bytes())
                    .ok_or_else(limit)?;
                for output in &outputs {
                    bytes = bytes
                        .checked_add(output.metadata_bytes()?)
                        .and_then(|n| n.checked_add(output.record().wire().len().checked_mul(2)?))
                        .filter(|n| *n <= 512 * 1024 * 1024)
                        .ok_or_else(limit)?;
                }
                let parent = frame.output().clone();
                self.frames.try_reserve_exact(1).map_err(|_| limit())?;
                self.active_derives
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                self.scans
                    .try_reserve_exact(outputs.len())
                    .map_err(|_| limit())?;
                self.records
                    .try_reserve_exact(outputs.len())
                    .map_err(|_| limit())?;
                let usage = outputs
                    .iter()
                    .map(|o| (o.record().key().fingerprint(), o.record().usage()))
                    .collect();
                self.budget.transaction(&usage, || Ok(()))?;
                self.scans.extend(outputs.iter().map(|o| o.scan().clone()));
                self.records
                    .extend(outputs.iter().map(|o| o.record_handle()));
                self.active_derives.push(ActiveItemDerives {
                    proof: CompletedItemDerives {
                        parent,
                        invocation,
                        receipt,
                        outputs,
                    },
                    next: 1,
                });
                self.frames.push(index);
                self.retained_bytes = bytes;
                continue;
            }
            if let Some(owner) = &self.item_owner
                && let Some(definition) =
                    owner.completed_helper_declaration(frame.output(), invocation)?
            {
                if self.item_attributes.len() >= 100_000 {
                    return Err(limit());
                }
                let bytes = self
                    .retained_bytes
                    .checked_add(std::mem::size_of::<CompletedItemAttribute>())
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                self.item_attributes
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                let mut updated = frame.clone();
                updated.retire_completed_item_attribute(invocation, owner)?;
                self.item_attributes.push(CompletedItemAttribute {
                    output: frame.output().clone(),
                    invocation,
                    definition,
                    catalog: owner.clone(),
                });
                *self.frames.last_mut().ok_or_else(limit)? = updated;
                self.retained_bytes = bytes;
                continue;
            }
            if let Some(owner) = &self.item_owner
                && let Some(output) =
                    owner.completed_declaration_call(frame.output(), invocation)?
            {
                let index = GeneratedDeclarationMacroIndex::new(
                    output.clone(),
                    DeclarationMacroParent::Generated(Arc::new(frame.clone())),
                )?;
                let record = output.record_handle();
                let bytes = self
                    .retained_bytes
                    .checked_add(output.metadata_bytes()?)
                    .and_then(|n| n.checked_add(index.retained_metadata_bytes()))
                    .and_then(|n| n.checked_add(record.wire().len().checked_mul(2)?))
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                if self.frames.len() >= 256 || self.records.len() >= 4096 {
                    return Err(limit());
                }
                self.frames.try_reserve_exact(1).map_err(|_| limit())?;
                self.scans.try_reserve_exact(1).map_err(|_| limit())?;
                self.records.try_reserve_exact(1).map_err(|_| limit())?;
                self.budget.transaction(
                    &BTreeMap::from([(record.key().fingerprint(), record.usage())]),
                    || Ok(()),
                )?;
                self.frames.push(index);
                self.scans.push(output.scan().clone());
                self.records.push(record);
                self.retained_bytes = bytes;
                continue;
            }
            if frame.output().scan().producers().iter().any(|p| p.identity==invocation
                && p.kind==crate::syntax::declaration_scan::DeclarationProducerKind::Attribute)
                && let Some(namespaces)=namespaces
                && let Some(selection)=crate::service::source_attribute_requests::SourceAttributeSelection::select_generated(
                    provider.module_source(),provider,Arc::new(frame.clone()),invocation,namespaces,std::slice::from_ref(provider))?
            {
                let selection=Arc::new(selection);self.pending_attribute=Some(selection.clone());
                return Ok(DeclarationFragmentStep::NeedAttributeSelection(selection));
            }
            let mut compiler_binding = None;
            if frame.output().scan().producers().iter().any(|p| {
                p.identity == invocation && p.unqualified_attribute.as_deref() == Some("test")
            }) {
                let Some(namespaces) = namespaces else {
                    return Ok(DeclarationFragmentStep::NeedInput(invocation));
                };
                let binding=match crate::service::source_attribute_requests::SourceAttributeSelection::generated_lookup(frame,invocation,namespaces,&self.cfg)? {
                    crate::frontend::macro_names::PathLookup::Absent=>None,
                    crate::frontend::macro_names::PathLookup::Defined(definition)=> {
                        if !super::builtin_attributes::permits_builtin_test_marker(
                            &definition, provider.procedural.as_deref(),
                            provider.compiler.as_ref().and_then(|c| c.attribute_catalog()),
                        )? {
                            return Ok(DeclarationFragmentStep::NeedInput(invocation));
                        }
                        Some(definition)
                    }
                    crate::frontend::macro_names::PathLookup::Ambiguous(_)=>return Err(fault("LRR-E-MACRO-AMBIGUOUS","generated compiler test attribute is ambiguous")),
                    crate::frontend::macro_names::PathLookup::Inaccessible(_)=>return Err(fault("LRR-E-MACRO-INACCESSIBLE","generated compiler test attribute is inaccessible")),
                    _=>return Ok(DeclarationFragmentStep::NeedInput(invocation)),
                };
                frame
                    .output()
                    .validate_compiler_test_target(invocation, &self.cfg)?;
                compiler_binding = Some(GeneratedCompilerTestBinding {
                    parent: Arc::new(frame.clone()),
                    definition: binding,
                });
            }
            if let Some(definition) =
                super::nested_inert::selected_definition(frame.output().scan(), invocation)?
            {
                let name = frame
                    .output()
                    .scan()
                    .producers()
                    .iter()
                    .find(|row| row.identity == invocation)
                    .and_then(|row| row.unqualified_attribute.as_deref())
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated inert annotation absent",
                        )
                    })?;
                let builtin_inert = super::builtin_attributes::inert_descriptor(name)?.is_some();
                let attribute_binding = if builtin_inert && let Some(namespaces) = namespaces {
                    let lookup = crate::service::source_attribute_requests::SourceAttributeSelection::generated_lookup(
                        frame, invocation, namespaces, &self.cfg,
                    )?;
                    let Some(binding) = super::builtin_attributes::certify_inert_binding(
                        name,
                        &lookup,
                        provider.procedural.as_deref(),
                        provider
                            .compiler
                            .as_ref()
                            .and_then(|c| c.attribute_catalog()),
                    )?
                    else {
                        let input = match lookup {
                            crate::frontend::macro_names::PathLookup::NeedInput(input) => input,
                            _ => invocation,
                        };
                        return Ok(DeclarationFragmentStep::NeedInput(input));
                    };
                    Some((Arc::new(frame.clone()), binding))
                } else {
                    if builtin_inert && provider.procedural.is_some() {
                        return Ok(DeclarationFragmentStep::NeedInput(invocation));
                    }
                    // Compiler-only scheduling can finish without a procedural
                    // namespace. Its module annotation frontier remains pending;
                    // this is not a final namespace or catalog certificate.
                    None
                };
                if self.inert_attributes.len() >= 100_000 {
                    return Err(limit());
                }
                let bytes = self
                    .retained_bytes
                    .checked_add(std::mem::size_of::<GeneratedInertAttribute>() + 128)
                    .and_then(|n| {
                        n.checked_add(
                            compiler_binding
                                .as_ref()
                                .map_or(0, |b| b.parent.retained_metadata_bytes()),
                        )
                    })
                    .and_then(|n| {
                        n.checked_add(
                            attribute_binding
                                .as_ref()
                                .map_or(0, |(parent, _)| parent.retained_metadata_bytes()),
                        )
                    })
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                self.inert_attributes
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                let output = frame.output().clone();
                let mut updated = frame.clone();
                updated.retire_inert_attribute(invocation)?;
                *self.frames.last_mut().ok_or_else(limit)? = updated;
                self.inert_attributes.push(GeneratedInertAttribute {
                    output,
                    invocation,
                    definition,
                    compiler_binding,
                    attribute_binding,
                });
                self.retained_bytes = bytes;
                continue;
            }
            if let Some(namespaces) = namespaces
                && let Some(selection) = provider.generated_procedural_selection(
                    Arc::new(frame.clone()),
                    invocation,
                    namespaces,
                    std::slice::from_ref(provider),
                )?
            {
                let bytes = self
                    .retained_bytes
                    .checked_add(selection.metadata_bytes()?)
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(limit)?;
                let selection = Arc::new(selection);
                self.pending_procedural = Some(selection.clone());
                self.retained_bytes = bytes;
                return Ok(DeclarationFragmentStep::NeedProceduralSelection(selection));
            }
            if let Some(namespaces) = namespaces
                && let Some((definition, kind)) =
                    provider.generated_runtime_definition(frame, invocation, namespaces)?
            {
                let scans = frame
                    .output()
                    .intrinsic_argument_scans(invocation, &self.cfg, kind)?;
                if scans.iter().any(|scan| !scan.producers().is_empty()) {
                    let arguments = frame.output().intrinsic_arguments(
                        invocation,
                        definition.clone(),
                        &self.cfg,
                    )?;
                    self.active_arguments = Some(ActiveGeneratedArguments {
                        output: frame.output().clone(),
                        invocation,
                        definition: definition.clone(),
                        assembly: Box::new(
                            super::argument_discovery::ArgumentDiscovery::with_parent(
                                arguments,
                                DeclarationMacroParent::Generated(Arc::new(frame.clone())),
                            )?,
                        ),
                    });
                    return self.poll_using(provider, Some(namespaces));
                }
                if scans.iter().all(|scan| scan.producers().is_empty()) {
                    let bytes = scans.iter().try_fold(self.retained_bytes, |n, scan| {
                        n.checked_add(scan.retained_bytes())
                            .filter(|n| *n <= 512 * 1024 * 1024)
                            .ok_or_else(limit)
                    })?;
                    let output = frame.output().clone();
                    let mut frame = frame.clone();
                    frame.complete_intrinsic_arguments(invocation, &scans)?;
                    self.scans.try_reserve(scans.len()).map_err(|_| limit())?;
                    self.intrinsic_arguments
                        .try_reserve(1)
                        .map_err(|_| limit())?;
                    let bytes = bytes
                        .checked_add(
                            scans
                                .capacity()
                                .checked_mul(std::mem::size_of::<Arc<DeclarationScan>>())
                                .ok_or_else(limit)?,
                        )
                        .and_then(|n| {
                            n.checked_add(std::mem::size_of::<GeneratedIntrinsicArguments>())
                        })
                        .filter(|n| *n <= 512 * 1024 * 1024)
                        .ok_or_else(limit)?;
                    self.scans.extend(scans.iter().cloned());
                    self.intrinsic_arguments.push(GeneratedIntrinsicArguments {
                        output,
                        invocation,
                        definition,
                        scans,
                    });
                    *self.frames.last_mut().ok_or_else(limit)? = frame;
                    self.retained_bytes = bytes;
                    continue;
                }
            }
            match provider.generated_declaration_demand(frame, invocation)? {
                SourceDeclarativeDemand::NeedInput(input) => {
                    return Ok(DeclarationFragmentStep::NeedInput(input));
                }
                SourceDeclarativeDemand::Ready(request) => {
                    let request = Arc::from(request);
                    self.pending = Some(Arc::clone(&request));
                    return Ok(DeclarationFragmentStep::NeedExpansion(request));
                }
            }
        }
    }
    /// Exact reply admission and accounting; no native or compiler execution occurs here.
    pub fn admit(&mut self, record: Arc<ExpansionRecord>) -> Result<(), Diagnostic> {
        if let Some(arguments) = &mut self.active_arguments {
            return arguments.assembly.admit(record);
        }
        let request = self.pending.as_ref().ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "declaration fragment has no pending request",
            )
        })?;
        if request.key() != record.key() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "declaration fragment reply key differs",
            ));
        }
        if self.frames.len() >= 256 || self.records.len() >= 4096 {
            return Err(limit());
        }
        let replacement_parent = if let Some((selection, _)) = self
            .bound_attributes
            .iter()
            .find(|(_, key)| key == request.key())
        {
            let mut parent = self.frames.last().ok_or_else(limit)?.clone();
            parent.retire_replaced_generated_body(selection.producer(), selection.item_range())?;
            Some(parent)
        } else {
            None
        };
        let parent = match self.frames.last() {
            Some(frame) => DeclarationMacroParent::Generated(Arc::new(frame.clone())),
            None => self.root_parent.clone(),
        };
        let parent_scan = match &parent {
            DeclarationMacroParent::Source(index) => index.scan(),
            DeclarationMacroParent::Argument(index) => index.argument().scan(),
            DeclarationMacroParent::Generated(index) => index.output().scan(),
        };
        let output = Arc::new(GeneratedDeclarations::admit(
            request.key(),
            Arc::clone(&record),
            self.epoch,
            parent_scan,
            &self.cfg,
            DeclarationScanLimits::default(),
        )?);
        let index = GeneratedDeclarationMacroIndex::new(Arc::clone(&output), parent)?;
        let bytes = self
            .retained_bytes
            .checked_add(output.metadata_bytes()?)
            .and_then(|n| n.checked_add(record.wire().len().checked_mul(2)?))
            .and_then(|n| n.checked_add(index.retained_metadata_bytes()))
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(limit)?;
        self.frames.try_reserve(1).map_err(|_| limit())?;
        self.scans.try_reserve(1).map_err(|_| limit())?;
        self.records.try_reserve(1).map_err(|_| limit())?;
        let budget = self.budget.clone();
        let (index, record) = budget.transaction(
            &BTreeMap::from([(record.key().fingerprint(), record.usage())]),
            || Ok((index, record)),
        )?;
        if let Some(parent) = replacement_parent {
            *self.frames.last_mut().ok_or_else(limit)? = parent;
        }
        self.frames.push(index);
        self.scans.push(Arc::clone(output.scan()));
        self.records.push(record);
        self.retained_bytes = bytes;
        self.pending = None;
        Ok(())
    }
}

#[cfg(test)]
mod module_fact_masks {
    use super::ModuleReplacementRanges;
    use crate::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        source::snapshot::{CfgSet, SourceBlob, SourcePath},
        syntax::{outline::ScopeKey, parse::ParsedFile, spans::SourceRange},
    };
    // Unchanged rustc issue53675 source; fixed2024 cold metadata adaptation only.
    #[test]
    fn rustc_module_ranges_mask_only_the_exact_owned_capture_and_complete_interval() {
        let text = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-53675-a-test-called-panic.rs"
        );
        let parsed = ParsedFile::parse_declarations(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let scan = |unit| {
            parsed
                .scan_declarations(
                    SourcePath::new("upstream.rs").unwrap(),
                    ScopeKey {
                        crate_key: Digest([unit; 32]),
                        module_path: vec![],
                        owner: None,
                    },
                    &CfgSet::guest_default(),
                    Default::default(),
                )
                .unwrap()
        };
        let original = scan(1);
        let foreign = scan(2);
        assert_eq!(original.modules().len(), 4);
        assert_eq!(original.modules()[0].range, foreign.modules()[0].range);
        assert_ne!(original.fingerprint(), foreign.fingerprint());
        let first = original.modules()[0].range;
        let second = original.modules()[1].range;
        let ranges = ModuleReplacementRanges::new(&[
            (original.fingerprint(), first),
            (original.fingerprint(), first),
        ])
        .unwrap();
        assert!(ranges.covers(original.fingerprint(), first));
        assert!(!ranges.covers(foreign.fingerprint(), first));
        assert!(!ranges.covers(original.fingerprint(), second));
        let partial = ModuleReplacementRanges::new(&[(
            original.fingerprint(),
            SourceRange {
                start: first.start + 1,
                end: first.end,
            },
        )])
        .unwrap();
        assert!(!partial.covers(original.fingerprint(), first));
        assert!(
            ModuleReplacementRanges::new(&[(
                original.fingerprint(),
                SourceRange {
                    start: first.start,
                    end: first.start
                }
            )])
            .is_err()
        );
    }
}

#[path = "declaration_fragment/empty_original.rs"]
mod empty_original;
