//! One immutable declaration-completion index shared by sibling impl waiters.
//! Output lookup supplies actual handles, never parent or namespace authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        declaration_fragment::CompletedDeclarationFragment,
        generated_declarations::GeneratedDeclarations, identity::GrammarSlot,
        record::ExpansionRecord,
    },
};
use std::sync::Arc;


fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}

pub(super) struct AncestorImplCompletion {
    completed: Arc<CompletedDeclarationFragment>,
    ancestor_job: Digest,
    epoch: u64,
    cfg: Digest,
    outputs: Vec<Arc<GeneratedDeclarations>>,
    scans: Vec<(Digest, usize)>,
    records: Vec<(Digest, Digest, usize)>,
    headers: Vec<Digest>,
    root_index: usize,
    bytes: usize,
}

impl AncestorImplCompletion {
    pub(super) fn prepare(
        completed: Arc<CompletedDeclarationFragment>,
        ancestor_job: Digest,
        epoch: u64,
        cfg: Digest,
    ) -> Result<Self, Diagnostic> {
        let root = completed.index().output();
        let source = completed.source_scan();
        let mut job = StableEncoder::new("lrr/v2/item-declaration-job");
        job.digest(root.fingerprint());
        if epoch == 0
            || ancestor_job != job.finish()
            || root.epoch() != epoch
            || root.record().key().grammar != GrammarSlot::Items
            || root.scan().cfg() != cfg
            || source.cfg() != cfg
            || root.parent_inventory() != source.fingerprint()
            || !source.producers().iter().any(|producer| {
                producer.identity == root.parent_invocation()
                    && producer.grammar == root.record().key().grammar
                    && &producer.scope == root.scan().scope()
            })
        {
            return Err(fault("child impl ancestor job/source attachment differs"));
        }
        // This is the only completed-subtree listing for the cached ancestor.
        // Its existing traversal/count limits remain in syntax_outputs itself.
        let outputs = completed.syntax_outputs()?;
        let header_count = completed.inventory().headers().len();
        if outputs.is_empty() {
            return Err(fault("child impl completion row limit"));
        }
        outputs
            .len()
            .checked_mul(
                std::mem::size_of::<Arc<GeneratedDeclarations>>()
                    + std::mem::size_of::<(Digest, usize)>()
                    + std::mem::size_of::<(Digest, Digest, usize)>(),
            )
            .and_then(|n| n.checked_add(header_count.checked_mul(std::mem::size_of::<Digest>())?))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(|| fault("child impl completion index budget"))?;
        let mut scans = Vec::new();
        let mut records = Vec::new();
        let mut headers = Vec::new();
        scans
            .try_reserve_exact(outputs.len())
            .map_err(|_| fault("child impl scan index allocation"))?;
        records
            .try_reserve_exact(outputs.len())
            .map_err(|_| fault("child impl record index allocation"))?;
        headers
            .try_reserve_exact(header_count)
            .map_err(|_| fault("child impl header index allocation"))?;
        let mut root_index = None;
        for (index, output) in outputs.iter().enumerate() {
            scans.push((output.scan().fingerprint(), index));
            records.push((
                output.record().key().fingerprint(),
                output.record().descriptor().fingerprint(),
                index,
            ));
            if Arc::ptr_eq(output, root) && root_index.replace(index).is_some() {
                return Err(fault("child impl ancestor output attachment is ambiguous"));
            }
        }
        let root_index = root_index.ok_or_else(|| fault("child impl ancestor output absent"))?;
        scans.sort_unstable();
        records.sort_unstable();
        headers.extend(
            completed
                .inventory()
                .headers()
                .iter()
                .map(|header| header.header_identity),
        );
        headers.sort_unstable();
        headers.dedup();
        // Charge actual retained capacities, including shared-handle storage.
        // The already-owned completed/source/output payload is not duplicated.
        let bytes = outputs
            .capacity()
            .checked_mul(std::mem::size_of::<Arc<GeneratedDeclarations>>())
            .and_then(|n| {
                n.checked_add(
                    scans
                        .capacity()
                        .checked_mul(std::mem::size_of::<(Digest, usize)>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(records.capacity().checked_mul(std::mem::size_of::<(
                    Digest,
                    Digest,
                    usize,
                )>())?)
            })
            .and_then(|n| {
                n.checked_add(
                    headers
                        .capacity()
                        .checked_mul(std::mem::size_of::<Digest>())?,
                )
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(|| fault("child impl retained completion budget"))?;
        Ok(Self {
            completed,
            ancestor_job,
            epoch,
            cfg,
            outputs,
            scans,
            records,
            headers,
            root_index,
            bytes,
        })
    }

    pub(super) fn require_owner(
        &self,
        ancestor_job: Digest,
        epoch: u64,
        cfg: Digest,
    ) -> Result<(), Diagnostic> {
        if self.ancestor_job != ancestor_job || self.epoch != epoch || self.cfg != cfg {
            return Err(fault("child impl retained ancestor owner differs"));
        }
        Ok(())
    }

    pub(super) fn completed(&self) -> &Arc<CompletedDeclarationFragment> {
        &self.completed
    }
    pub(super) fn outputs(&self) -> &[Arc<GeneratedDeclarations>] {
        &self.outputs
    }
    pub(super) fn scans(&self) -> &[(Digest, usize)] {
        &self.scans
    }
    pub(super) fn root_index(&self) -> usize {
        self.root_index
    }
    pub(super) fn contains_header(&self, header: Digest) -> bool {
        self.headers.binary_search(&header).is_ok()
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn unique_output(&self, record: &ExpansionRecord) -> Result<usize, Diagnostic> {
        let key = record.key().fingerprint();
        let descriptor = record.descriptor().fingerprint();
        let at = self
            .records
            .partition_point(|row| (row.0, row.1) < (key, descriptor));
        let mut found = None;
        for &(_, _, index) in self.records[at..]
            .iter()
            .take_while(|row| (row.0, row.1) == (key, descriptor))
        {
            // The digest index is only a candidate locator. Actual record
            // equality and uniqueness remain the selection authority.
            if self.outputs[index].record().same_record(record) && found.replace(index).is_some() {
                return Err(fault("child impl completed output attachment is ambiguous"));
            }
        }
        found.ok_or_else(|| fault("child impl actual completed output absent"))
    }
}
