//! Candidate syntax scheduling for compiler no-transform attributes. This is
//! deliberately not a namespace selection or an attribute completion proof.
use super::{SourceDeclarative, original_annotations::OriginalAnnotationPlan};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::changes::SourceSelection,
    syntax::{imports::ImportSegment, outline::ItemOutline},
};
use std::collections::BTreeMap;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ORIGINAL-ANNOTATION", message.into())
}
struct Candidate {
    producer: Digest,
    header: Digest,
    descriptors: Digest,
}
pub(crate) struct CompilerAttributePreparation {
    source: SourceSelection,
    compiler: Digest,
    candidates: BTreeMap<Digest, Candidate>,
    bytes: usize,
}
impl CompilerAttributePreparation {
    pub(super) fn capture(
        provider: &SourceDeclarative,
        plan: &OriginalAnnotationPlan,
    ) -> Result<Option<Self>, Diagnostic> {
        if provider.matched_attribute_facet()?.is_none() {
            return Ok(None);
        }
        let source = provider.module_source();
        plan.require_source(source)?;
        let compiler = provider
            .compiler
            .as_ref()
            .ok_or_else(|| fault("compiler preparation catalog absent"))?
            .owning_profile_identity()?;
        let mut result = Self {
            source: source.selection().clone(),
            compiler,
            candidates: BTreeMap::new(),
            bytes: std::mem::size_of::<Self>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(source.selection().metadata_bytes().ok()?))
                .ok_or_else(|| fault("compiler preparation source accounting overflow"))?,
        };
        for (producer, ordinal) in plan.items() {
            let item = source
                .outline()
                .items
                .get(ordinal)
                .ok_or_else(|| fault("compiler preparation item absent"))?;
            let Some(rows) = plan.rows(source, producer, item.def_key.fingerprint())? else {
                return Err(fault("compiler preparation annotation rows absent"));
            };
            let mut descriptors = StableEncoder::new("lrr/v1/compiler-attribute-preparation-rows");
            let mut count = 0usize;
            let mut eligible = true;
            for (row, attribute) in rows.zip(&item.header.attributes) {
                // Predefined tool attributes (`rustfmt::skip`) are inert by the
                // outline's own classification; they are not compiler candidates
                // but do not disqualify the item's builtin rows.
                if attribute.class == crate::syntax::outline::AttributeClass::ToolLint {
                    descriptors.u8(0);
                    descriptors.string(&attribute.name);
                    count += 1;
                    continue;
                }
                descriptors.u8(1);
                // Qualified attributes never become compiler candidates by their
                // last spelling. The retained AST path is authoritative syntax.
                let [ImportSegment::Name(name)] = row.segments.as_slice() else {
                    eligible = false;
                    break;
                };
                if row.absolute {
                    eligible = false;
                    break;
                }
                let Some(descriptor) =
                    crate::expand::builtin_attributes::preparation_inert_descriptor(name)?
                else {
                    eligible = false;
                    break;
                };
                let Some(proof) =
                    crate::expand::nested_inert::NestedInertProof::select_original_plan(
                        plan,
                        row.scan_row,
                    )?
                else {
                    eligible = false;
                    break;
                };
                if proof.name()? != name {
                    return Err(fault("compiler preparation Meta/name differs"));
                }
                descriptors.digest(proof.producer());
                descriptors.digest(descriptor.definition_identity());
                count += 1;
            }
            if eligible
                && count != 0
                && count
                    == item
                        .header
                        .attributes
                        .iter()
                        .filter(|attribute| {
                            crate::syntax::outline::attribute_may_name_macro(attribute)
                        })
                        .count()
            {
                result.bytes = result
                    .bytes
                    .checked_add(std::mem::size_of::<(Digest, Candidate)>() + 128)
                    .ok_or_else(|| fault("compiler preparation metadata limit"))?;
                result.candidates.insert(
                    item.def_key.fingerprint(),
                    Candidate {
                        producer,
                        header: item.header.fingerprint(),
                        descriptors: descriptors.finish(),
                    },
                );
            }
        }
        Ok(Some(result))
    }
    pub(crate) fn require_outline(
        &self,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        if outline.content != self.source.outline.parse.content
            || outline.profile != self.source.outline.parse.profile
            || outline.cfg != self.source.outline.cfg
            || outline.root_scope.crate_key != self.source.outline.crate_key
            || outline.root_scope.module_path != self.source.outline.module_path
            || outline.root_scope.owner.is_some()
        {
            return Err(fault(
                "compiler preparation route source/profile/cfg/scope differs",
            ));
        }
        Ok(())
    }
    /// Candidate syntax only. No producer, frontier or namespace is completed.
    pub(crate) fn permits(&self, item: &ItemOutline) -> bool {
        self.candidates
            .get(&item.def_key.fingerprint())
            .is_some_and(|row| row.header == item.header.fingerprint())
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn require_final(
        &self,
        provider: &SourceDeclarative,
        completed: &BTreeMap<Digest, crate::expand::source_derives::SourceEmptyDeriveProof>,
    ) -> Result<(), Diagnostic> {
        if provider.module_source().selection() != &self.source
            || provider.matched_attribute_facet()?.is_none()
            || provider
                .compiler
                .as_ref()
                .ok_or_else(|| fault("compiler preparation catalog absent at seal"))?
                .owning_profile_identity()?
                != self.compiler
        {
            return Err(fault(
                "compiler preparation source/catalog changed before seal",
            ));
        }
        provider
            .macro_index()
            .pending_producers()
            .count()
            .checked_mul(160)
            .ok_or_else(|| fault("compiler attribute final pending-set temporary limit"))?;
        let pending: std::collections::BTreeSet<_> =
            provider.macro_index().pending_producers().collect();
        for (item, row) in &self.candidates {
            if pending.contains(&row.producer) {
                return Err(fault(
                    "compiler attribute candidate producer remains pending at seal",
                ));
            }
            if completed
                .get(&row.producer)
                .is_none_or(|proof| proof.item != *item)
            {
                return Err(fault(
                    "compiler attribute candidate has no final selected completion",
                ));
            }
        }
        Ok(())
    }
}
impl StableEncode for CompilerAttributePreparation {
    const DOMAIN: &'static str = "lrr/v1/compiler-attribute-candidate-preparation";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source.outline);
        e.string(self.source.path.as_str());
        e.digest(self.compiler);
        e.u64(self.candidates.len() as u64);
        for (item, row) in &self.candidates {
            e.digest(*item);
            e.digest(row.producer);
            e.digest(row.header);
            e.digest(row.descriptors);
        }
    }
}

impl SourceDeclarative {
    pub(crate) fn matched_attribute_facet(
        &self,
    ) -> Result<Option<&crate::frontend::builtin_attributes::BuiltinAttributeCatalog>, Diagnostic>
    {
        let Some(compiler) = self.compiler.as_ref() else {
            return Ok(None);
        };
        let Some(attributes) = compiler.attribute_catalog() else {
            return Ok(None);
        };
        let source = self.module_source();
        let matched = if self.macro_export_source_role().is_some() {
            compiler
                .canonical_standalone_member(&source.selection().outline.parse.profile)?
                .is_some()
        } else {
            compiler.runtime_prelude_unit(false)
                == crate::facade::unit(&source.selection().outline.parse.profile, "core")
                && compiler.runtime_catalog()?.is_some()
        };
        if !matched {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "original inert prefix compiler Attribute facet/profile differs".into(),
            ));
        }
        Ok(Some(attributes))
    }
    pub(crate) fn prepare_compiler_attributes(
        &mut self,
        plan: &OriginalAnnotationPlan,
    ) -> Result<usize, Diagnostic> {
        let prepared = CompilerAttributePreparation::capture(self, plan)?;
        let bytes = prepared
            .as_ref()
            .map_or(0, CompilerAttributePreparation::metadata_bytes);
        self.compiler_attribute_preparation = prepared.map(std::sync::Arc::new);
        Ok(bytes)
    }
    pub(crate) fn require_final_compiler_attributes(
        &self,
        completed: &BTreeMap<Digest, crate::expand::source_derives::SourceEmptyDeriveProof>,
    ) -> Result<(), Diagnostic> {
        if let Some(prepared) = &self.compiler_attribute_preparation {
            prepared.require_final(self, completed)?;
        }
        Ok(())
    }
}
