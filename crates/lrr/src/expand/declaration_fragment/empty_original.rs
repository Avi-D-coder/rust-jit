//! Closed completed original empty-output facts; no parser/expander effects.
use super::*;
impl CompletedDeclarationFragment {
    pub(crate) fn require_original_empty_record(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        epoch: u64,
    ) -> Result<&ExpansionRecord, Diagnostic> {
        self.require_original_record(source, epoch, false)
    }
    /// A completed original body-position (statement/expression) expansion
    /// whose actual output declares nothing: no item headers, modules,
    /// imports, macro definitions, impls, traits, associated fragments or
    /// nested producers. Its tokens execute as ordinary body syntax and are
    /// never namespace evidence.
    pub(crate) fn require_original_declaration_free_record(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        epoch: u64,
    ) -> Result<&ExpansionRecord, Diagnostic> {
        self.require_original_record(source, epoch, true)
    }
    /// A completed original body-position (statement/expression) expansion
    /// together with every nested expansion its actual output produced (for
    /// example `panic!` emitting `panic_2015!`). Each record in the closed
    /// tree must be a completed declaration-free body output attached to
    /// exactly one producer of its actual parent output. Returns the tree's
    /// records, the original record first. The tokens execute as ordinary
    /// body syntax and are never namespace evidence.
    pub(crate) fn require_original_declaration_free_tree(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        epoch: u64,
    ) -> Result<Vec<&ExpansionRecord>, Diagnostic> {
        use crate::expand::identity::{GrammarSlot, MacroIdentity};
        let deny = || {
            fault(
                "LRR-E-CONST-SOURCE",
                "original declaration-free expansion tree differs or has generated contributions",
            )
        };
        if self.completed_indexes.is_empty() {
            return Ok(vec![self.require_original_declaration_free_record(source, epoch)?]);
        }
        let output = self.index.output();
        let record = output.record();
        if epoch == 0
            || self.source.parse() != &source.selection().outline.parse
            || self.source.path() != &source.selection().path
            || self.source.scope() != &source.outline().root_scope
            || self.source.cfg() != source.selection().outline.cfg
            || output.parent_inventory() != self.source.fingerprint()
            || record.key().call_edition != self.source.parse().profile.edition
            || !matches!(
                record.key().grammar,
                GrammarSlot::Statements | GrammarSlot::Expression
            )
            || self.records.len() != self.completed_indexes.len().checked_add(1).ok_or_else(deny)?
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
            || !self.source.producers().iter().any(|producer| {
                producer.identity == record.key().invocation
                    && producer.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    && producer.grammar == record.key().grammar
            })
        {
            return Err(deny());
        }
        let tree = std::iter::once(&self.index)
            .chain(&self.completed_indexes)
            .collect::<Vec<_>>();
        let mut attached = vec![false; self.completed_indexes.len()];
        let mut records = Vec::new();
        records.try_reserve_exact(tree.len()).map_err(|_| deny())?;
        for index in &tree {
            let output = index.output();
            let record = output.record();
            let scan = output.scan();
            if !index.is_complete()
                || output.epoch() != epoch
                || scan.cfg() != self.source.cfg()
                || record.key().procedural_epoch.is_some()
                || !matches!(
                    record.key().macro_identity,
                    MacroIdentity::Declaration { .. } | MacroIdentity::Builtin { .. }
                )
                || !matches!(
                    record.key().grammar,
                    GrammarSlot::Statements
                        | GrammarSlot::Expression
                        | GrammarSlot::Pattern
                        | GrammarSlot::Type
                )
                || !scan.impls().is_empty()
                || !scan.macro_definitions().is_empty()
                || !scan.modules().is_empty()
                || !scan.imports().is_empty()
                || !scan.traits().is_empty()
                || !scan.associated_fragments().is_empty()
            {
                return Err(deny());
            }
            // The only admitted header rows are the actual nested macro
            // calls themselves (unnamed `Other` rows at a producer range).
            for header in output.headers() {
                if header.kind != crate::expand::generated_items::GeneratedHeaderKind::Other
                    || header.name.is_some()
                    || !scan.producers().iter().any(|producer| {
                        producer.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                            && producer.range.start == header.start
                            && producer.range.end == header.end
                    })
                {
                    return Err(deny());
                }
            }
            // Every actual producer of this output completed as exactly one
            // retained child of this tree.
            for producer in scan.producers() {
                let mut children = self.completed_indexes.iter().enumerate().filter(|(_, child)| {
                    let child = child.output();
                    child.parent_inventory() == scan.fingerprint()
                        && child.parent_invocation() == producer.identity
                        && child.record().key().invocation == producer.identity
                        && child.record().key().grammar == producer.grammar
                });
                let (Some((ordinal, _)), None) = (children.next(), children.next()) else {
                    return Err(deny());
                };
                if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                    || std::mem::replace(&mut attached[ordinal], true)
                {
                    return Err(deny());
                }
            }
            let mut actual = self.records.iter().filter(|actual| actual.same_record(record));
            let (Some(actual), None) = (actual.next(), actual.next()) else {
                return Err(deny());
            };
            records.push(actual.as_ref());
        }
        if attached.iter().any(|attached| !attached) {
            return Err(deny());
        }
        Ok(records)
    }
    fn require_original_record(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        epoch: u64,
        body_output: bool,
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
        if body_output {
            return if matches!(
                record.key().grammar,
                crate::expand::identity::GrammarSlot::Statements
                    | crate::expand::identity::GrammarSlot::Expression
            ) && output.headers().is_empty()
                && scan.traits().is_empty()
                && scan.associated_fragments().is_empty()
            {
                Ok(record)
            } else {
                Err(deny())
            };
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
