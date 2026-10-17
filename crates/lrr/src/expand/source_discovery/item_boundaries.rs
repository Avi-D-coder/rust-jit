//! Deferred publication of an actual ordinary item's no-catalog boundary.
//! Prerequisites are coordinator-issued typed jobs, not guessed macro absence.
use super::*;
use crate::expand::body_macro_boundary::CompletedEmptyBoundary;


pub(super) struct PendingItemBoundary {
    boundary: CompletedEmptyBoundary,
    source: PreparedSourceView,
    epoch: u64,
    cfg: Digest,
    prerequisites: Vec<DiscoveryJob>,
    bytes: usize,
}

impl PendingItemBoundary {
    /// The caller must supply the actual no-catalog branch's boundary and all
    /// jobs from its admission update, including nested declaration closure.
    /// This method never derives macro absence from an empty header count.
    pub(super) fn prepare(
        boundary: CompletedEmptyBoundary,
        source: &PreparedSourceView,
        epoch: u64,
        prerequisites: &[DiscoveryJob],
    ) -> Result<Self, Diagnostic> {
        let key = boundary.record.key();
        let cfg = source.captured_macro_cfg()?.fingerprint();
        let actual_call = source
            .outline()
            .item_invocations
            .iter()
            .any(|call| call == &boundary.call)
            || source.outline().items.iter().any(|item| {
                crate::syntax::outline::potential_attribute_producer(item)
                    .is_some_and(|call| call == boundary.call)
            });
        if epoch == 0
            || key.grammar != super::super::identity::GrammarSlot::Items
            || key.invocation != boundary.call.item_input
            || key.procedural_epoch.is_some_and(|actual| actual != epoch)
            || source.selection().outline.cfg != cfg
            || prerequisites.is_empty()
            || !actual_call
        {
            return Err(fault("pending item boundary actual owner differs"));
        }
        let job_bytes = prerequisites
            .len()
            .checked_mul(std::mem::size_of::<DiscoveryJob>())
            .ok_or_else(|| fault("pending item boundary metadata overflow"))?;
        let source_bytes = source
            .exclusive_metadata_bytes()?
            .checked_sub(std::mem::size_of::<PreparedSourceView>())
            .ok_or_else(|| fault("pending item boundary source metadata overflow"))?;
        let bytes = std::mem::size_of::<Self>()
            .checked_add(boundary.call.scope.heap_bytes())
            .and_then(|n| n.checked_add(job_bytes))
            .and_then(|n| n.checked_add(128))
            .and_then(|n| n.checked_add(source_bytes))
            .ok_or_else(|| fault("pending item boundary metadata limit"))?;
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(prerequisites.len())
            .map_err(|_| fault("pending item boundary prerequisite allocation"))?;
        jobs.extend_from_slice(prerequisites);
        jobs.sort_by_key(|job| (job.kind, job.identity, job.module_order, job.source_offset));
        jobs.dedup();
        // Same kind/identity with conflicting source order cannot be silently
        // collapsed. The worklist has the same uniqueness requirement.
        for pair in jobs.windows(2) {
            if pair[0].kind == pair[1].kind && pair[0].identity == pair[1].identity {
                return Err(fault("pending item boundary prerequisite owner conflict"));
            }
        }
        let source = source.share();
        let actual_source_bytes = source
            .exclusive_metadata_bytes()?
            .checked_sub(std::mem::size_of::<PreparedSourceView>())
            .ok_or_else(|| fault("pending item boundary retained source overflow"))?;
        let actual_bytes = bytes
            .checked_sub(job_bytes)
            .and_then(|n| n.checked_sub(source_bytes))
            .and_then(|n| n.checked_add(actual_source_bytes))
            .and_then(|n| {
                jobs.capacity()
                    .checked_mul(std::mem::size_of::<DiscoveryJob>())
                    .and_then(|jobs| n.checked_add(jobs))
            })
            .ok_or_else(|| fault("pending item boundary retained limit"))?;
        Ok(Self {
            boundary,
            source,
            epoch,
            cfg,
            prerequisites: jobs,
            bytes: actual_bytes,
        })
    }

    /// Exclusive receipt/key/job storage; record and source publication payloads
    /// remain shared with the enclosing discovery's existing retained owners.
    pub(super) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        Ok(self.bytes)
    }

    fn ready(
        &self,
        work: &Discovery,
        source: &PreparedSourceView,
        epoch: u64,
    ) -> Result<bool, Diagnostic> {
        if self.epoch != epoch
            || !self.source.same_publication(source)
            || self.source.selection() != source.selection()
            || self.cfg != source.captured_macro_cfg()?.fingerprint()
        {
            return Err(fault("pending item boundary source/cfg/epoch changed"));
        }
        Ok(self
            .prerequisites
            .iter()
            .all(|job| work.completed_job(job.kind, job.identity)))
    }
}

impl SourceDiscovery {
    /// Cold fixed-point hook, before work.poll can seal. It does not query a
    /// provider, parse output or inspect the source world for membership.
    pub(super) fn complete_ready_item_boundaries(&mut self) -> Result<bool, Diagnostic> {
        let result = self.complete_ready_item_boundaries_inner();
        if let Err(error) = &result {
            // A provider or route update may already have mutated. Preserve
            // admission's fail-stop contract; a partial request cannot seal.
            self.failed = Some(error.clone());
        }
        result
    }

    fn complete_ready_item_boundaries_inner(&mut self) -> Result<bool, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(self.pending_item_boundaries.len())
            .map_err(|_| fault("ready item boundary allocation"))?;
        for (identity, pending) in &self.pending_item_boundaries {
            let boundary = &pending.boundary;
            let source = self
                .sources
                .get(boundary.source_ordinal)
                .ok_or_else(|| fault("pending item boundary source absent"))?;
            if *identity != boundary.record.key().fingerprint()
                || self
                    .records
                    .get(identity)
                    .is_none_or(|record| !record.same_record(&boundary.record))
                || self
                    .generated_owners
                    .get(identity)
                    .is_none_or(|(ordinal, scope)| {
                        *ordinal != boundary.source_ordinal || scope != &boundary.call.scope
                    })
            {
                return Err(fault("pending item boundary record/scope owner differs"));
            }
            if pending.ready(&self.work, source, self.epoch)? {
                if self.completed_empty_boundaries.contains_key(identity) {
                    return Err(fault("completed item boundary already present"));
                }
                if self
                    .declarative
                    .as_ref()
                    .and_then(|(_, _, providers)| providers.get(boundary.source_ordinal))
                    .is_none()
                {
                    return Err(fault("ready item boundary provider absent"));
                }
                ready.push(*identity);
            }
        }
        if ready.is_empty() {
            return Ok(false);
        }
        self.completed_empty_boundaries
            .len()
            .checked_add(ready.len())
            .ok_or_else(|| fault("completed item boundary count limit"))?;
        for identity in ready {
            let pending = self
                .pending_item_boundaries
                .get(&identity)
                .ok_or_else(|| fault("ready item boundary absent"))?;
            self.declarative
                .as_mut()
                .and_then(|(_, _, providers)| providers.get_mut(pending.boundary.source_ordinal))
                .ok_or_else(|| fault("ready item boundary provider absent"))?
                .complete_without_macro_bindings(pending.boundary.call.item_input)?;
            let pending = self
                .pending_item_boundaries
                .remove(&identity)
                .ok_or_else(|| fault("completed item boundary absent"))?;
            self.completed_empty_boundaries
                .insert(identity, pending.boundary);
        }
        Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            &self
                .declarative
                .as_ref()
                .ok_or_else(|| fault("completed item boundary providers absent"))?
                .2,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        self.work.namespace_changed()?;
        Ok(true)
    }
}
