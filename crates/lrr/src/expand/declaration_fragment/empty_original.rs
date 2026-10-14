//! Closed completed original empty-output facts; no parser/expander effects.
use super::*;
impl CompletedDeclarationFragment {
    pub(crate) fn require_original_empty_record(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        epoch: u64,
    ) -> Result<&ExpansionRecord, Diagnostic> {
        let deny = || {
            fault(
                "LRR-E-CONST-SOURCE",
                "original empty declaration fragment differs or has generated contributions",
            )
        };
        let output = self.index.output();
        let record = output.record();
        let scan = output.scan();
        if epoch == 0
            || output.epoch() != epoch
            || self.source.parse() != &source.selection().outline.parse
            || self.source.path() != &source.selection().path
            || self.source.scope() != &source.outline().root_scope
            || self.source.cfg() != source.selection().outline.cfg
            || output.parent_inventory() != self.source.fingerprint()
            || scan.cfg() != self.source.cfg()
            || record.key().call_edition != self.source.parse().profile.edition
            || !matches!(
                record.key().macro_identity,
                crate::expand::identity::MacroIdentity::Declaration { .. }
            )
            || record.key().procedural_epoch.is_some()
            || self.records.len() != 1
            || !self.records[0].same_record(record)
            || !self.completed_indexes.is_empty()
            || !self.inventory.headers().is_empty()
            || !self.bound_procedural.is_empty()
            || !self.bound_attributes.is_empty()
            || !self.inert_attributes.is_empty()
            || !self.item_attributes.is_empty()
            || !self.item_derives.is_empty()
            || !self.item_replacements.is_empty()
            || self.item_owner.is_some()
            || !self.intrinsic_arguments.is_empty()
            || !self.completed_arguments.is_empty()
            || !scan.impls().is_empty()
            || !scan.producers().is_empty()
            || !scan.macro_definitions().is_empty()
            || !scan.modules().is_empty()
            || !scan.imports().is_empty()
            || !self.source.producers().iter().any(|producer| {
                producer.identity == record.key().invocation
                    && producer.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && producer.grammar == record.key().grammar
            })
        {
            return Err(deny());
        }
        let roots = output.tokens().roots();
        if !roots.is_empty()
            && !matches!(roots,
            [crate::syntax::token_wire::TokenTree::Group { children, .. }] if children.is_empty())
        {
            return Err(deny());
        }
        Ok(record)
    }
}
