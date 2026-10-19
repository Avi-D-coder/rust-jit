//! Actual original annotation closure transported to ordinary source publication.
//! This receipt grants neither executable entry nor lexical namespace completion.
use super::*;
use crate::{
    core::digest::Digest,
    expand::source_derives::SourceDeriveCompletionProof,
    source::{changes::PreparedSourceView, original_items::OriginalAnnotationGroup},
    syntax::{
        body_items::BodyItem,
        outline::{BodyHandle, ScopeKey},
        parse::ParsedFile,
    },
};

/// Private construction requires the same current group and exact output custody
/// as the ordinary declaration index's real completion. A caller flag cannot
/// stand in for a completed annotation.
pub(crate) struct CompletedOriginalAnnotation {
    group: Arc<OriginalAnnotationGroup>,
    proof: SourceDeriveCompletionProof,
    local: Option<(BodyHandle, ScopeKey)>,
}
impl CompletedOriginalAnnotation {
    pub(crate) fn item(&self) -> Digest {
        self.group.item()
    }
    pub(crate) fn require_source(&self, source: &PreparedSourceView) -> Result<(), Diagnostic> {
        self.proof.require_group(&self.group, source)
    }
    pub(crate) fn require_local_declaration(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        scope: &ScopeKey,
        row: &BodyItem,
    ) -> Result<(), Diagnostic> {
        let producer = row.attribute_input.ok_or_else(fault)?;
        self.group.require_item(parsed, &row.item, producer)?;
        if self
            .local
            .as_ref()
            .is_none_or(|(actual_body, actual_scope)| actual_body != body || actual_scope != scope)
            || !body.range.contains(row.item.declaration_span)
            || scope.crate_key != row.item.scope.crate_key
            || scope.module_path != row.item.scope.module_path
        {
            return Err(fault());
        }
        Ok(())
    }
}
impl DeclarationMacroIndex {
    pub(crate) fn completed_original_annotation(
        &self,
        source: &PreparedSourceView,
        group: &Arc<OriginalAnnotationGroup>,
        proof: &SourceDeriveCompletionProof,
    ) -> Result<CompletedOriginalAnnotation, Diagnostic> {
        proof.require_group(group, source)?;
        if group.scan_identity() != self.scan.fingerprint() || group.annotations().is_empty() {
            return Err(fault());
        }
        for annotation in group.annotations() {
            let producer = self.producer(*annotation)?;
            if self.pending.contains(annotation)
                || producer.kind
                    != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                || !group.call().range.contains(producer.range)
            {
                return Err(fault());
            }
        }
        let occurrences = group
            .annotations()
            .iter()
            .filter_map(|annotation| self.appended_outputs.get(annotation))
            .flat_map(|outputs| outputs.keys().copied())
            .collect();
        proof.require_occurrences(&occurrences)?;
        let target = source
            .original_annotation_producer(group.call().item_input)?
            .ok_or_else(fault)?;
        if target.item().def_key.fingerprint() != group.item() {
            return Err(fault());
        }
        let local = target
            .local_plan()
            .map(|plan| (plan.body().clone(), plan.scope().clone()));
        Ok(CompletedOriginalAnnotation {
            group: Arc::clone(group),
            proof: proof.clone(),
            local,
        })
    }
}
