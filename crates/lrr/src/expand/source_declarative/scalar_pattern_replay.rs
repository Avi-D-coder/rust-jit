//! Original-only replay for reached pattern namespace admission. Never expand.
use super::*;
impl SourceDeclarative {
    pub(crate) fn replay_scalar_pattern_source(
        source: &PreparedSourceView,
        input: &crate::db::seals::DiscoveryInput,
    ) -> Result<crate::frontend::source_macros::PublishedMacroIndex, Diagnostic> {
        source.require_original_hygiene()?;
        if !input.sources.contains(source.selection()) || input.seal.unresolved_frontiers() != 0 {
            return Err(fault(
                "LRR-E-PATTERN-SOURCE",
                "original pattern source lacks completed publication",
            ));
        }
        let mut provider = Self::new(source.share())?;
        // Exact private inert proofs are retained by the completed current seal.
        // Apply only matching original Attribute item producers, by real Meta
        // range and source parse; bare names never retire a producer.
        for call in &source.outline().item_invocations {
            if call.attribute_input.is_none()
                || provider.index.producer_kind(call.item_input).is_none()
            {
                continue;
            }
            let mut proofs = input.nested_inert_proofs.iter().filter(|proof| {
                proof.scan().parse() == &source.selection().outline.parse
                    && proof
                        .scan()
                        .producers()
                        .iter()
                        .any(|row| row.identity == proof.producer() && row.range == call.range)
            });
            let Some(proof) = proofs.next() else {
                continue;
            };
            if proofs.next().is_some() || proof.attribute_facet().is_none() {
                return Err(fault(
                    "LRR-E-PATTERN-SOURCE",
                    "original pattern Attribute proof is ambiguous or lacks its facet",
                ));
            }
            proof.revalidate(source.captured_macro_blob()?, source.captured_macro_cfg()?)?;
            provider.index.complete_empty_item_producer(
                &call.scope,
                u64::from(call.range.start),
                call.item_input,
            )?;
        }
        // A remaining producer is a real prerequisite. Do not reconstruct its
        // generated declarations, foreign scope or replacements from spelling.
        if provider.index.pending_producers().next().is_some() {
            return Err(fault(
                "LRR-E-PATTERN-SOURCE",
                "original pattern macro namespace has an unreplayed producer",
            ));
        }
        provider.completed_published_index()
    }
}
