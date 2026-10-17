//! A child impl frontier waits for its real ancestor's declaration traversal.
//! Output membership alone never supplies parent attachment or completion.
use super::item_impl_completion::AncestorImplCompletion;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::{
        generated_declarations::GeneratedDeclarations, generated_items::GeneratedItems,
        identity::GrammarSlot,
    },
    frontend::generated_headers::ActiveImplProjection,
    source::snapshot::CfgSet,
    syntax::outline::ScopeKey,
};
use std::sync::Arc;


fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}

pub(super) struct ChildImplFrontier {
    ancestor_job: Digest,
    items: Arc<GeneratedItems>,
    parent_items: Arc<GeneratedItems>,
    projection: Arc<ActiveImplProjection>,
    scope: ScopeKey,
    cfg: CfgSet,
    cfg_fingerprint: Digest,
    epoch: u64,
    bytes: usize,
}

impl ChildImplFrontier {
    pub(super) fn prepare(
        ancestor_job: Digest,
        items: Arc<GeneratedItems>,
        parent_items: Arc<GeneratedItems>,
        projection: Arc<ActiveImplProjection>,
        scope: ScopeKey,
        cfg: CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        if epoch == 0
            || items.epoch() != epoch
            || parent_items.epoch() != epoch
            || items.record().key().grammar != GrammarSlot::Items
            || projection
                .active_impl_ranges(&items, &cfg, &scope, epoch)?
                .is_empty()
        {
            return Err(fault("child impl frontier actual output/epoch is absent"));
        }
        let cfg_fingerprint = cfg.fingerprint();
        let bytes = std::mem::size_of::<Self>()
            .checked_add(scope.heap_bytes())
            .and_then(|n| n.checked_add(projection.metadata_bytes()))
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| fault("child impl frontier metadata overflow"))?
            .checked_add(cfg.metadata_bytes()?)
            .ok_or_else(|| fault("child impl frontier metadata limit"))?;
        Ok(Self {
            ancestor_job,
            items,
            parent_items,
            projection,
            scope,
            cfg,
            cfg_fingerprint,
            epoch,
            bytes,
        })
    }

    pub(super) fn ancestor_job(&self) -> Digest {
        self.ancestor_job
    }

    pub(super) fn scope(&self) -> &ScopeKey {
        &self.scope
    }

    pub(super) fn cfg_fingerprint(&self) -> Digest {
        self.cfg_fingerprint
    }

    /// Includes the retained projection conservatively. GeneratedItems and its
    /// record payloads retain the actual child/parent catalog/record owners.
    pub(super) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        Ok(self.bytes)
    }

    pub(super) fn resolve(
        &self,
        completed: &AncestorImplCompletion,
        epoch: u64,
    ) -> Result<Vec<Digest>, Diagnostic> {
        if self.epoch != epoch || self.items.epoch() != epoch || self.parent_items.epoch() != epoch
        {
            return Err(fault("child impl frontier epoch changed"));
        }
        let cfg = self.cfg_fingerprint;
        completed.require_owner(self.ancestor_job, epoch, cfg)?;
        let outputs = completed.outputs();
        let child = completed.unique_output(self.items.record())?;
        let root_index = completed.root_index();
        if child == root_index {
            return Err(fault("child impl output is the ancestor root"));
        }
        let output = &outputs[child];
        if output.epoch() != epoch
            || output.scan().scope() != &self.scope
            || output.scan().cfg() != cfg
        {
            return Err(fault("child impl actual scope/cfg/epoch differs"));
        }
        Self::require_parent_chain(
            outputs,
            completed.scans(),
            child,
            root_index,
            &self.parent_items,
            epoch,
            cfg,
        )?;

        // One shared active raw-header -> admitted canonical-header join. No
        // parser, scope reconstruction, header re-encoding or body query here.
        let pairs =
            output.item_impl_headers_from_projection(&self.items, &self.cfg, &self.projection)?;
        if pairs.is_empty() {
            return Err(fault("child impl completed active headers absent"));
        }
        pairs
            .len()
            .checked_mul(std::mem::size_of::<Digest>())
            .ok_or_else(|| fault("child impl resolved header limit"))?;
        let mut headers = Vec::new();
        headers
            .try_reserve_exact(pairs.len())
            .map_err(|_| fault("child impl resolved header allocation"))?;
        for (_, header) in pairs {
            if !completed.contains_header(header) {
                return Err(fault("child impl header absent from completed inventory"));
            }
            headers.push(header);
        }
        Ok(headers)
    }

    fn require_parent_chain(
        outputs: &[Arc<GeneratedDeclarations>],
        scans: &[(Digest, usize)],
        mut current: usize,
        root: usize,
        expected_parent: &GeneratedItems,
        epoch: u64,
        cfg: Digest,
    ) -> Result<(), Diagnostic> {
        let mut visited = Vec::new();
        for depth in 0usize.. {
            if current == root {
                return Ok(());
            }
            if visited.contains(&current) {
                return Err(fault("child impl ancestry cycle"));
            }
            visited.push(current);
            let child = &outputs[current];
            if child.epoch() != epoch || child.scan().cfg() != cfg {
                return Err(fault("child impl parent output cfg/epoch differs"));
            }
            let parent_scan = child.parent_inventory();
            let at = scans.partition_point(|row| row.0 < parent_scan);
            let (_, parent) = scans
                .get(at)
                .filter(|row| row.0 == parent_scan)
                .ok_or_else(|| fault("child impl actual generated parent absent"))?;
            if scans.get(at + 1).is_some_and(|row| row.0 == parent_scan) {
                return Err(fault("child impl actual generated parent is ambiguous"));
            }
            if depth == 0
                && (!outputs[*parent]
                    .record()
                    .same_record(expected_parent.record())
                    || outputs[*parent].epoch() != expected_parent.epoch())
            {
                return Err(fault("child impl selected parent catalog differs"));
            }
            if !outputs[*parent].scan().producers().iter().any(|producer| {
                producer.identity == child.parent_invocation()
                    && producer.grammar == child.record().key().grammar
                    && &producer.scope == child.scan().scope()
            }) {
                return Err(fault("child impl actual parent producer absent"));
            }
            current = *parent;
        }
        Err(fault("child impl ancestry depth limit"))
    }
}
