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
    names: Option<Vec<Arc<crate::expand::declaration_fragment::CompletedDeclarationNameEffects>>>,
}
impl CompletedOriginalAnnotation {
    pub(crate) fn declared_names_at<'a>(&'a self, scope: &'a ScopeKey)
        -> impl Iterator<Item = Arc<crate::frontend::generated_headers::declared_names::DeclaredItemName>> + 'a {
        self.name_effects().into_iter().flatten().flat_map(|effect| effect.names.iter())
            .filter(move |declaration| declaration.scope() == scope).cloned()
    }
    pub(crate) fn original_target<'a>(&self, source: &'a PreparedSourceView)
        -> Result<crate::source::original_items::OriginalItemTarget<'a>, Diagnostic> {
        self.require_source(source)?;
        source.original_annotation_producer(self.group.call().item_input)?.ok_or_else(fault)
    }
    pub(crate) fn with_name_effects(
        mut self,
        parent: &DeclarationMacroIndex,
        source: &PreparedSourceView,
        fragments: &std::collections::BTreeMap<Digest, Arc<crate::expand::declaration_fragment::CompletedDeclarationFragment>>,
    ) -> Result<Self, Diagnostic> {
        self.require_source(source)?;
        // Revalidate this exact parent, including all actual adopted outputs.
        // An otherwise matching fresh index cannot certify an empty effect set.
        parent.completed_original_annotation(source, &self.group, &self.proof)?;
        if self.group.scan_identity() != parent.scan.fingerprint() { return Err(fault()); }
        let root = parent.facts.context(parent.context)?;
        let mut names = Vec::new();
        for annotation in self.group.annotations() {
            for (occurrence, descriptor) in parent.appended_outputs.get(annotation).into_iter().flatten() {
                let fragment = fragments.get(occurrence).ok_or_else(fault)?;
                let output = fragment.index().output();
                if !fragment.index().is_complete()
                    || fragment.source_scan().fingerprint() != self.group.scan_identity()
                    || output.parent_invocation() != *annotation
                    || output.record().key().invocation != *occurrence
                    || output.record().descriptor().fingerprint() != *descriptor
                { return Err(fault()); }
                names.push(fragment.declaration_name_effects(root, source.captured_macro_cfg()?)?);
            }
        }
        self.names = Some(names);
        Ok(self)
    }
    pub(crate) fn name_effects(&self) -> Option<&[Arc<crate::expand::declaration_fragment::CompletedDeclarationNameEffects>]> {
        self.names.as_deref()
    }
    pub(crate) fn name_fingerprint(&self) -> Digest {
        use crate::core::digest::StableEncoder;
        let mut e = StableEncoder::new("lrr/v1/original-annotation-name-effects");
        e.digest(self.group.fingerprint()); e.digest(self.proof.fingerprint);
        e.bool(self.names.is_some());
        for names in self.names.iter().flatten() { e.digest(names.fingerprint()); }
        e.finish()
    }
    pub(crate) fn retired_names(&self) -> impl Iterator<Item = Digest> + '_ {
        std::iter::once(self.group.call().item_input).chain(self.group.annotations().iter().copied())
    }
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
            names: None,
        })
    }
}
