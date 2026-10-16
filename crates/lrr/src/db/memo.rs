use super::{
    dependency::{DependencyStamp, InputKey, InputRecord, InputValue, missing_fingerprint},
    key::QueryKey,
    poll::{InputRequest, QueryPoll},
    provider::{Provided, Provider},
    trace::{Action, Trace},
    value::QueryValue,
};
use crate::core::{diagnostic::Diagnostic, digest::Digest};
use std::{collections::BTreeMap, sync::Arc};
pub const MAX_QUERY_DEPTH: usize = 256;

#[path = "lexical_inputs.rs"]
mod lexical_inputs;

#[path = "scope_certificate_index.rs"]
mod scope_certificate_index;

#[path = "namespace_witness.rs"]
mod namespace_witness;
pub(crate) use namespace_witness::NamespaceWitness;

#[path = "constant_inputs.rs"]
mod constant_inputs;

#[path = "source_view_inputs.rs"]
mod source_view_inputs;

#[path = "body_leases.rs"]
mod body_leases;
#[path = "prepared_modules.rs"]
mod prepared_modules;
#[path = "retained_body_inputs.rs"]
mod retained_body_inputs;
pub(crate) use retained_body_inputs::{RetainedOriginalDeclaration, RetainedSourceLease};
#[path = "retained_syntax_inputs.rs"]
mod retained_syntax_inputs;
pub(crate) use body_leases::BodyExecutionLease;

#[derive(Debug, Clone)]
pub struct MemoEntry {
    pub value: Arc<QueryValue>,
    pub fingerprint: Digest,
    pub dependencies: Vec<DependencyStamp>,
    pub verified_revision: u64,
    pub last_used_counter: u64,
    pub estimated_bytes: usize,
}
#[derive(Debug, Clone)]
pub struct Outcome {
    pub value: Arc<QueryValue>,
    pub fingerprint: Digest,
    pub cacheable: bool,
}
#[derive(Default)]
struct Collector {
    dependencies: Vec<DependencyStamp>,
    non_cacheable: bool,
    suspended: Option<InputRequest>,
}
#[derive(Default)]
struct ExpansionReadObservation {
    stamps: BTreeMap<Digest, Digest>,
    error: Option<Diagnostic>,
}
struct SourcePin {
    snapshot: std::sync::Weak<crate::source::snapshot::Snapshot>,
    parsed: std::sync::Weak<crate::syntax::parse::ParsedFile>,
    attachments: std::sync::Weak<crate::syntax::spans::SpanAttachments>,
}
struct QueryPin {
    body: std::sync::Weak<crate::syntax::body_index::BodyIndex>,
    value: std::sync::Weak<QueryValue>,
    parsed: std::sync::Weak<crate::syntax::parse::ParsedFile>,
    outline: std::sync::Weak<crate::syntax::outline::Outline>,
    signature: std::sync::Weak<crate::syntax::outline::HeaderRecipe>,
    inventory: std::sync::Weak<crate::frontend::impl_index::ImplHeaderIndex>,
    reference_header:
        std::sync::Weak<crate::syntax::closed_reference_signatures::ReferenceHeaderPlan>,
    pattern_plan: std::sync::Weak<crate::syntax::pattern_plan::PatternSourcePlan>,
}
impl QueryPin {
    fn new(value: &Arc<QueryValue>) -> Self {
        let mut pin = Self {
            value: Arc::downgrade(value),
            body: std::sync::Weak::new(),
            parsed: std::sync::Weak::new(),
            outline: std::sync::Weak::new(),
            signature: std::sync::Weak::new(),
            inventory: std::sync::Weak::new(),
            reference_header: std::sync::Weak::new(),
            pattern_plan: std::sync::Weak::new(),
        };
        match value.as_ref() {
            QueryValue::ReferenceHeader(v) => pin.reference_header = Arc::downgrade(v),
            QueryValue::PatternPlan(v) => {
                pin.pattern_plan = Arc::downgrade(v);
                pin.body = Arc::downgrade(v.index());
                pin.parsed = Arc::downgrade(v.parsed());
            }
            QueryValue::BodyStructure(v) => pin.body = Arc::downgrade(v),
            QueryValue::ParseFile(v) => pin.parsed = Arc::downgrade(v),
            QueryValue::Outline(v) => pin.outline = Arc::downgrade(v),
            QueryValue::Signature(v) => pin.signature = Arc::downgrade(v),
            QueryValue::ScopeDeclarationInventory { inventory, .. } => {
                pin.inventory = Arc::downgrade(inventory)
            }
            _ => {}
        }
        pin
    }
    fn external(&self, outer: &BTreeMap<usize, usize>, inner: &BTreeMap<usize, usize>) -> bool {
        self.value.strong_count()
            > outer
                .get(&(self.value.as_ptr() as usize))
                .copied()
                .unwrap_or(0)
            || self.body.strong_count()
                > inner
                    .get(&(self.body.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.parsed.strong_count()
                > inner
                    .get(&(self.parsed.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.outline.strong_count()
                > inner
                    .get(&(self.outline.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.signature.strong_count()
                > inner
                    .get(&(self.signature.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.reference_header.strong_count()
                > inner
                    .get(&(self.reference_header.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.pattern_plan.strong_count()
                > inner
                    .get(&(self.pattern_plan.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
            || self.inventory.strong_count()
                > inner
                    .get(&(self.inventory.as_ptr() as usize))
                    .copied()
                    .unwrap_or(0)
    }
}
pub struct Database {
    inputs: BTreeMap<InputKey, InputRecord>,
    scope_certificates: scope_certificate_index::ScopeCertificateIndex,
    memos: BTreeMap<QueryKey, MemoEntry>,
    revision: u64,
    source_revision: u64,
    active_queries: Vec<QueryKey>,
    collectors: Vec<Collector>,
    expansion_reads: Option<ExpansionReadObservation>,
    use_counter: u64,
    run_active: bool,
    run_budgets: Option<crate::engine::telemetry::CacheBudgets>,
    source_pins: Vec<SourcePin>,
    body_leases: Vec<std::sync::Weak<body_leases::BodyExecutionOwner>>,
    prepared_sources: Vec<std::sync::Weak<crate::source::changes::PreparedSourceState>>,
    // Allocation addresses are private accounting identities, never semantic keys,
    // serialized output, or eviction-order inputs. Weak owners prevent address reuse.
    query_pins: BTreeMap<usize, QueryPin>,
    input_pins: BTreeMap<usize, std::sync::Weak<InputValue>>,
    pub trace: Trace,
}
impl Default for Database {
    fn default() -> Self {
        Self {
            inputs: BTreeMap::new(),
            scope_certificates: Default::default(),
            memos: BTreeMap::new(),
            revision: 1,
            source_revision: 1,
            active_queries: vec![],
            collectors: vec![],
            expansion_reads: None,
            use_counter: 0,
            run_active: false,
            run_budgets: None,
            source_pins: vec![],
            body_leases: vec![],
            prepared_sources: vec![],
            query_pins: BTreeMap::new(),
            input_pins: BTreeMap::new(),
            trace: Trace::default(),
        }
    }
}
impl Database {
    pub(crate) fn require_coordinator(&self) -> Result<(), Diagnostic> {
        if !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "coordinator work during pure query",
            ));
        }
        Ok(())
    }
    fn memo_owners(&self) -> (BTreeMap<usize, usize>, BTreeMap<usize, usize>) {
        let mut outer = BTreeMap::new();
        let mut inner = BTreeMap::new();
        let mut plans = std::collections::BTreeSet::new();
        for memo in self.memos.values() {
            let count = outer.entry(Arc::as_ptr(&memo.value) as usize).or_insert(0);
            *count += 1;
            if *count == 1 {
                let allocation = match memo.value.as_ref() {
                    QueryValue::ReferenceHeader(v) => Some(Arc::as_ptr(v) as usize),
                    QueryValue::PatternPlan(v) => {
                        if plans.insert(Arc::as_ptr(v)) {
                            *inner.entry(Arc::as_ptr(v.parsed()) as usize).or_insert(0) += 1;
                            *inner.entry(Arc::as_ptr(v.index()) as usize).or_insert(0) += 1;
                        }
                        Some(Arc::as_ptr(v) as usize)
                    }
                    QueryValue::BodyStructure(v) => Some(Arc::as_ptr(v) as usize),
                    QueryValue::ParseFile(v) => Some(Arc::as_ptr(v) as usize),
                    QueryValue::Outline(v) => Some(Arc::as_ptr(v) as usize),
                    QueryValue::Signature(v) => Some(Arc::as_ptr(v) as usize),
                    QueryValue::ScopeDeclarationInventory { inventory, .. } => {
                        Some(Arc::as_ptr(inventory) as usize)
                    }
                    _ => None,
                };
                if let Some(allocation) = allocation {
                    *inner.entry(allocation).or_insert(0) += 1;
                }
            }
        }
        // A live input box owns one inner inventory reference, regardless of
        // how many outer Arc handles point to that same box.
        let mut owners = std::collections::BTreeSet::new();
        for value in self.inputs.values().map(|r| r.value.clone()).chain(
            self.input_pins
                .values()
                .filter_map(std::sync::Weak::upgrade),
        ) {
            if owners.insert(Arc::as_ptr(&value) as usize)
                && let InputValue::DiscoverySeal(input) = value.as_ref()
            {
                *inner
                    .entry(Arc::as_ptr(&input.declaration_inventory) as usize)
                    .or_insert(0) += 1;
            }
        }
        (outer, inner)
    }
    pub(crate) fn publish_prepared(
        &mut self,
        state: Arc<crate::source::changes::PreparedSourceState>,
    ) -> Result<(), Diagnostic> {
        self.publish_inputs(state.inputs.clone())?;
        let weak = Arc::downgrade(&state);
        if !self.prepared_sources.iter().any(|old| old.ptr_eq(&weak)) {
            self.prepared_sources.push(weak);
        }
        Ok(())
    }
    /// Query-safe exact immutable lease; current input must select this source.
    pub(crate) fn prepared_source(
        &self,
        selection: &crate::source::changes::SourceSelection,
    ) -> Result<Option<crate::source::changes::PreparedSourceView>, Diagnostic> {
        use crate::source::changes::PreparedSourceView;
        if !matches!(self.published_input(&InputKey::CapturedSource(selection.location_identity())), Some((InputValue::CapturedSource(actual), _)) if actual == selection)
        {
            return Ok(None);
        }
        if self.prepared_sources.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "retained source selection count limit",
            ));
        }
        for state in self
            .prepared_sources
            .iter()
            .rev()
            .filter_map(std::sync::Weak::upgrade)
        {
            if let Some(view) = PreparedSourceView::from_retained_state(state, selection)? {
                return Ok(Some(view));
            }
        }
        Ok(None)
    }
    pub(crate) fn prepared_parse(
        &self,
        key: &crate::syntax::parse::ParseKey,
    ) -> Option<Arc<crate::syntax::parse::ParsedFile>> {
        for state in self
            .prepared_sources
            .iter()
            .rev()
            .filter_map(std::sync::Weak::upgrade)
        {
            if let Some(parsed) = state.parses.get(key) {
                return Some(parsed.clone());
            }
        }
        None
    }
    pub(crate) fn prepared_outline(
        &self,
        key: &super::key::OutlineKey,
    ) -> Option<Arc<crate::syntax::outline::Outline>> {
        for state in self
            .prepared_sources
            .iter()
            .rev()
            .filter_map(std::sync::Weak::upgrade)
        {
            if let Some(outline) = state.outlines.get(key) {
                return Some(outline.clone());
            }
        }
        None
    }
    pub fn pin_source_lease(
        &mut self,
        parsed: Arc<crate::syntax::parse::ParsedFile>,
        snapshot: Arc<crate::source::snapshot::Snapshot>,
        attachments: Arc<crate::syntax::spans::SpanAttachments>,
        file: &crate::source::snapshot::SourcePath,
    ) -> Result<crate::syntax::spans::SourceLease, Diagnostic> {
        let pin = SourcePin {
            snapshot: Arc::downgrade(&snapshot),
            parsed: Arc::downgrade(&parsed),
            attachments: Arc::downgrade(&attachments),
        };
        let lease = crate::syntax::spans::SourceLease::new(parsed, snapshot, attachments, file)?;
        if !self.source_pins.iter().any(|entry| {
            entry.snapshot.ptr_eq(&pin.snapshot)
                && entry.parsed.ptr_eq(&pin.parsed)
                && entry.attachments.ptr_eq(&pin.attachments)
        }) {
            self.source_pins.push(pin);
        }
        Ok(lease)
    }
    pub fn begin_run(&mut self) -> Result<(), Diagnostic> {
        self.begin_run_with_budgets(crate::engine::telemetry::CacheBudgets::default())
    }
    pub fn begin_run_with_budgets(
        &mut self,
        budgets: crate::engine::telemetry::CacheBudgets,
    ) -> Result<(), Diagnostic> {
        if self.run_active || !self.active_queries.is_empty() {
            return Err(fault("LRR-E-INTERNAL-SNAPSHOT", "run already active"));
        }
        self.evict_between_runs(budgets)?;
        self.run_active = true;
        self.run_budgets = Some(budgets);
        Ok(())
    }
    pub fn finish_run(&mut self) -> Result<(), Diagnostic> {
        if !self.run_active || !self.active_queries.is_empty() {
            return Err(fault("LRR-E-INTERNAL-SNAPSHOT", "invalid run completion"));
        }
        self.run_active = false;
        self.run_budgets = None;
        Ok(())
    }
    pub fn cache_usage(&self) -> Result<crate::engine::telemetry::CacheUsage, Diagnostic> {
        self.cache_usage_with_prepared(&self.prepared_sources, self.prepared_sources.capacity())
    }
    fn cache_usage_with_prepared(
        &self,
        prepared_sources: &[std::sync::Weak<crate::source::changes::PreparedSourceState>],
        prepared_capacity: usize,
    ) -> Result<crate::engine::telemetry::CacheUsage, Diagnostic> {
        let mut usage = crate::engine::telemetry::CacheUsage::default();
        let body_leases =
            body_leases::metadata(self.body_leases.capacity(), self.body_leases.len())?;
        usage.memo_bytes = account_add(usage.memo_bytes, body_leases)?;
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, body_leases)?;
        let certificates = self.scope_certificates.metadata_bytes();
        usage.memo_bytes = account_add(usage.memo_bytes, certificates)?;
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, certificates)?;
        let mut source_allocations = std::collections::BTreeSet::new();
        let mut map_allocations = std::collections::BTreeSet::new();
        let mut inventory_allocations = std::collections::BTreeSet::new();
        for value in self.inputs.values() {
            for blob in value.value.source_blobs() {
                if source_allocations.insert(blob.storage_id()) {
                    usage.source_input_bytes =
                        account_add(usage.source_input_bytes, blob.bytes().len())?;
                }
            }
        }
        for (key, value) in &self.inputs {
            if let InputValue::DiscoverySeal(input) = value.value.as_ref() {
                inventory_allocations.insert(Arc::as_ptr(&input.declaration_inventory));
            }
            let bytes = input_metadata_bytes(key, value.value.as_ref())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let input_owners: std::collections::BTreeSet<_> = self
            .inputs
            .values()
            .map(|record| Arc::as_ptr(&record.value) as usize)
            .collect();
        let retired_inputs: Vec<_> = self
            .input_pins
            .iter()
            .filter(|(allocation, _)| !input_owners.contains(allocation))
            .filter_map(|(_, pin)| pin.upgrade())
            .collect();
        for value in &retired_inputs {
            if let InputValue::DiscoverySeal(input) = value.as_ref() {
                inventory_allocations.insert(Arc::as_ptr(&input.declaration_inventory));
            }
            let bytes = account_add(value.estimated_bytes()?, 16)?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            for blob in value.source_blobs() {
                if source_allocations.insert(blob.storage_id()) {
                    usage.source_input_bytes =
                        account_add(usage.source_input_bytes, blob.bytes().len())?;
                }
            }
        }
        for (allocation, pin) in &self.input_pins {
            let bytes = 64
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<std::sync::Weak<InputValue>>();
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            if pin.strong_count() > usize::from(input_owners.contains(allocation)) {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            }
        }
        usage.memo_entries = self.memos.len();
        let snapshots: Vec<_> = self
            .source_pins
            .iter()
            .filter_map(|pin| pin.snapshot.upgrade())
            .collect();
        let mut snapshot_allocations = std::collections::BTreeSet::new();
        for snapshot in &snapshots {
            if !snapshot_allocations.insert(Arc::as_ptr(snapshot)) {
                continue;
            }
            for blob in snapshot.files().values() {
                if source_allocations.insert(blob.storage_id()) {
                    usage.source_input_bytes =
                        account_add(usage.source_input_bytes, blob.bytes().len())?;
                }
            }
            let bytes = snapshot.source_map_bytes(&mut map_allocations);
            usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
            usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
            let metadata = snapshot.metadata_bytes()?;
            usage.memo_bytes = account_add(usage.memo_bytes, metadata)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, metadata)?;
        }
        let mut parse_allocations = std::collections::BTreeSet::new();
        let mut outline_allocations = std::collections::BTreeSet::new();
        let mut signature_allocations = std::collections::BTreeSet::new();
        let mut body_allocations = std::collections::BTreeSet::new();
        // Account each immutable body once, independently of its memo wrappers.
        // In particular, a retained outer Outcome also pins the inner body.
        let mut cached_bodies = BTreeMap::new();
        let mut reference_header_allocations = std::collections::BTreeSet::new();
        let mut pattern_plan_allocations = std::collections::BTreeSet::new();
        let mut cached_reference_headers = BTreeMap::new();
        let mut cached_pattern_plans = BTreeMap::new();
        let mut cached_plan_parses = BTreeMap::new();
        let (memo_outer, memo_inner) = self.memo_owners();
        for memo in self.memos.values() {
            if let QueryValue::ParseFile(parsed) = memo.value.as_ref() {
                parse_allocations.insert(Arc::as_ptr(parsed));
            }
            if let QueryValue::Outline(outline) = memo.value.as_ref() {
                outline_allocations.insert(Arc::as_ptr(outline));
            }
            if let QueryValue::Signature(signature) = memo.value.as_ref() {
                signature_allocations.insert(Arc::as_ptr(signature));
            }
            if let QueryValue::ScopeDeclarationInventory { inventory, .. } = memo.value.as_ref() {
                inventory_allocations.insert(Arc::as_ptr(inventory));
            }
            let parse = memo.value.parse_bytes();
            let body_bytes = if let QueryValue::BodyStructure(body) = memo.value.as_ref() {
                let pinned = Arc::strong_count(&memo.value)
                    > memo_outer[&(Arc::as_ptr(&memo.value) as usize)]
                    || Arc::strong_count(body) > memo_inner[&(Arc::as_ptr(body) as usize)];
                cached_bodies
                    .entry(Arc::as_ptr(body))
                    .and_modify(|(_, previous): &mut (_, bool)| *previous |= pinned)
                    .or_insert((body, pinned));
                account_add(body.estimated_bytes(), 2 * std::mem::size_of::<usize>())?
            } else {
                0
            };
            let plan_bytes = match memo.value.as_ref() {
                QueryValue::ReferenceHeader(plan) => {
                    let pinned = Arc::strong_count(&memo.value)
                        > memo_outer[&(Arc::as_ptr(&memo.value) as usize)]
                        || Arc::strong_count(plan) > memo_inner[&(Arc::as_ptr(plan) as usize)];
                    cached_reference_headers
                        .entry(Arc::as_ptr(plan))
                        .and_modify(|(_, previous): &mut (_, bool)| *previous |= pinned)
                        .or_insert((plan, pinned));
                    account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                QueryValue::PatternPlan(plan) => {
                    let pinned = Arc::strong_count(&memo.value)
                        > memo_outer[&(Arc::as_ptr(&memo.value) as usize)]
                        || Arc::strong_count(plan) > memo_inner[&(Arc::as_ptr(plan) as usize)];
                    // A plan owns these allocations even when their original
                    // query wrappers were evicted and weak rows pruned.
                    let body = plan.index();
                    let body_pinned = pinned
                        || Arc::strong_count(body) > memo_inner[&(Arc::as_ptr(body) as usize)];
                    cached_bodies
                        .entry(Arc::as_ptr(body))
                        .and_modify(|(_, previous): &mut (_, bool)| *previous |= body_pinned)
                        .or_insert((body, body_pinned));
                    let parsed = plan.parsed();
                    let parse_pinned = pinned
                        || Arc::strong_count(parsed) > memo_inner[&(Arc::as_ptr(parsed) as usize)];
                    cached_plan_parses
                        .entry(Arc::as_ptr(parsed))
                        .and_modify(|(_, previous): &mut (_, bool)| *previous |= parse_pinned)
                        .or_insert((parsed, parse_pinned));
                    cached_pattern_plans
                        .entry(Arc::as_ptr(plan))
                        .and_modify(|(_, previous): &mut (_, bool)| *previous |= pinned)
                        .or_insert((plan, pinned));
                    account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                _ => 0,
            };
            let bytes = memo
                .estimated_bytes
                .saturating_sub(parse)
                .checked_sub(body_bytes)
                .and_then(|bytes| bytes.checked_sub(plan_bytes))
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "shared body accounting underflow"))?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.parse_bytes = account_add(usage.parse_bytes, parse)?;
            if QueryPin::new(&memo.value).external(&memo_outer, &memo_inner) {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
                usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, parse)?;
            }
        }
        for (allocation, (body, pinned)) in cached_bodies {
            body_allocations.insert(allocation);
            let bytes = account_add(body.estimated_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            if pinned {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            }
        }
        for (allocation, (plan, pinned)) in cached_reference_headers {
            reference_header_allocations.insert(allocation);
            let bytes = account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            if pinned {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            }
        }
        for (allocation, (plan, pinned)) in cached_pattern_plans {
            pattern_plan_allocations.insert(allocation);
            let bytes = account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            if pinned {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            }
        }
        for (allocation, (parsed, pinned)) in cached_plan_parses {
            if parse_allocations.insert(allocation) {
                let bytes = parsed.estimated_bytes();
                usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
                if pinned {
                    usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
                }
            }
        }
        let prepared: Vec<_> = prepared_sources
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .collect();
        for state in &prepared {
            let mut metadata = state.file_metadata_bytes()?;
            for (key, value) in &state.inputs {
                metadata = account_add(metadata, input_metadata_bytes(key, value)?)?;
                for blob in value.source_blobs() {
                    if source_allocations.insert(blob.storage_id()) {
                        usage.source_input_bytes =
                            account_add(usage.source_input_bytes, blob.bytes().len())?;
                    }
                }
            }
            for file in state.files.values() {
                if parse_allocations.insert(Arc::as_ptr(&file.parsed)) {
                    let bytes = file.parsed.estimated_bytes();
                    usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
                    usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
                }
                if outline_allocations.insert(Arc::as_ptr(&file.outline)) {
                    metadata = account_add(metadata, file.outline.estimated_bytes())?;
                }
            }
            usage.memo_bytes = account_add(usage.memo_bytes, metadata)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, metadata)?;
        }
        // Account cached parses before upgrading weak pins: upgrading a cached
        // allocation would itself look like an external user and falsely pin it.
        let retired_parses: Vec<_> = self
            .source_pins
            .iter()
            .filter(|pin| parse_allocations.insert(pin.parsed.as_ptr()))
            .filter_map(|pin| pin.parsed.upgrade())
            .collect();
        for parsed in &retired_parses {
            let bytes = parsed.estimated_bytes();
            usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
            usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
        }
        let retired_values: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| !memo_outer.contains_key(&(pin.value.as_ptr() as usize)))
            .filter_map(|pin| pin.value.upgrade())
            .collect();
        for value in &retired_values {
            let inner_bytes = match value.as_ref() {
                QueryValue::BodyStructure(v) => {
                    account_add(v.estimated_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                QueryValue::ReferenceHeader(v) => {
                    account_add(v.metadata_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                QueryValue::PatternPlan(v) => {
                    account_add(v.metadata_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                QueryValue::ParseFile(v) => v.estimated_bytes(),
                QueryValue::Outline(v) => v.estimated_bytes(),
                QueryValue::Signature(v) => v.estimated_bytes(),
                QueryValue::ScopeDeclarationInventory { inventory, .. } => {
                    inventory.metadata_bytes()
                }
                _ => 0,
            };
            let bytes = account_add(
                value
                    .estimated_bytes()
                    .checked_sub(inner_bytes)
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "retired value accounting underflow")
                    })?,
                16,
            )?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        // Frames retain inner body handles after their outer query Outcome is
        // gone. Weak pins preserve accounting even after memo invalidation.
        let query_bodies: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| body_allocations.insert(pin.body.as_ptr()))
            .filter_map(|pin| pin.body.upgrade())
            .collect();
        for body in &query_bodies {
            let bytes = account_add(body.estimated_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let query_parses: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| parse_allocations.insert(pin.parsed.as_ptr()))
            .filter_map(|pin| pin.parsed.upgrade())
            .collect();
        for parsed in &query_parses {
            let bytes = parsed.estimated_bytes();
            usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
            usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
        }
        let query_outlines: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| outline_allocations.insert(pin.outline.as_ptr()))
            .filter_map(|pin| pin.outline.upgrade())
            .collect();
        for outline in &query_outlines {
            let bytes = outline.estimated_bytes();
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let query_signatures: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| signature_allocations.insert(pin.signature.as_ptr()))
            .filter_map(|pin| pin.signature.upgrade())
            .collect();
        for signature in &query_signatures {
            let bytes = signature.estimated_bytes();
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let query_inventories: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| inventory_allocations.insert(pin.inventory.as_ptr()))
            .filter_map(|pin| pin.inventory.upgrade())
            .collect();
        for inventory in &query_inventories {
            let bytes = inventory.metadata_bytes();
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let query_reference_headers: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| reference_header_allocations.insert(pin.reference_header.as_ptr()))
            .filter_map(|pin| pin.reference_header.upgrade())
            .collect();
        for plan in &query_reference_headers {
            let bytes = account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        let query_pattern_plans: Vec<_> = self
            .query_pins
            .values()
            .filter(|pin| pattern_plan_allocations.insert(pin.pattern_plan.as_ptr()))
            .filter_map(|pin| pin.pattern_plan.upgrade())
            .collect();
        for plan in &query_pattern_plans {
            let bytes = account_add(plan.metadata_bytes(), 2 * std::mem::size_of::<usize>())?;
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
        for pin in self.query_pins.values() {
            let bytes = 64 + std::mem::size_of::<usize>() + std::mem::size_of::<QueryPin>();
            usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
            if pin.external(&memo_outer, &memo_inner) {
                usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
            }
        }
        let attachments: Vec<_> = self
            .source_pins
            .iter()
            .filter_map(|pin| pin.attachments.upgrade())
            .collect();
        let mut attachment_allocations = std::collections::BTreeSet::new();
        for attachment in &attachments {
            if attachment_allocations.insert(Arc::as_ptr(attachment)) {
                let bytes = account_add(
                    std::mem::size_of_val(attachment.as_ref()),
                    attachment.estimated_bytes(),
                )?;
                usage.parse_bytes = account_add(usage.parse_bytes, bytes)?;
                usage.pinned_parse_bytes = account_add(usage.pinned_parse_bytes, bytes)?;
            }
        }
        let registry_bytes = self
            .source_pins
            .capacity()
            .checked_mul(std::mem::size_of::<SourcePin>())
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "source pin accounting overflow"))?;
        usage.memo_bytes = account_add(usage.memo_bytes, registry_bytes)?;
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, registry_bytes)?;
        let prepared_registry_bytes = prepared_capacity
            .checked_mul(std::mem::size_of::<
                std::sync::Weak<crate::source::changes::PreparedSourceState>,
            >())
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "prepared registry accounting overflow",
                )
            })?;
        usage.memo_bytes = account_add(usage.memo_bytes, prepared_registry_bytes)?;
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, prepared_registry_bytes)?;
        Ok(usage)
    }
    /// Reclaim only between runs. Retained Arc handles pin their cache entries.
    pub fn evict_between_runs(
        &mut self,
        budgets: crate::engine::telemetry::CacheBudgets,
    ) -> Result<crate::engine::telemetry::CacheEviction, Diagnostic> {
        if self.run_active || !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "cache eviction during active run/query",
            ));
        }
        self.prepared_sources
            .retain(|state| state.strong_count() > 0);
        self.prepared_sources.shrink_to_fit();
        let input_owners: std::collections::BTreeSet<_> = self
            .inputs
            .values()
            .map(|record| Arc::as_ptr(&record.value) as usize)
            .collect();
        self.input_pins.retain(|allocation, pin| {
            pin.strong_count() > usize::from(input_owners.contains(allocation))
        });
        let (memo_outer, memo_inner) = self.memo_owners();
        self.query_pins
            .retain(|_, pin| pin.external(&memo_outer, &memo_inner));
        self.source_pins.retain(|pin| {
            pin.snapshot.strong_count() > 0
                || pin.parsed.strong_count()
                    > memo_inner
                        .get(&(pin.parsed.as_ptr() as usize))
                        .copied()
                        .unwrap_or(0)
                || pin.attachments.strong_count() > 0
        });
        self.source_pins.shrink_to_fit();
        let before = self.cache_usage()?;
        if before.source_input_bytes > budgets.source_input_bytes
            || before.pinned_memo_bytes > budgets.query_bytes
            || before.pinned_parse_bytes > budgets.parse_bytes
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "pinned cache/source inputs exceed configured cap",
            ));
        }
        let mut candidates: Vec<_> = self
            .memos
            .iter()
            .filter(|(_, m)| {
                Arc::strong_count(&m.value) == 1
                    && !QueryPin::new(&m.value).external(&memo_outer, &memo_inner)
            })
            .map(|(key, m)| (m.last_used_counter, key.clone()))
            .collect();
        candidates.sort();
        let mut after = before.clone();
        let mut victims = vec![];
        for (_, key) in candidates {
            if after.memo_bytes <= budgets.query_bytes && after.parse_bytes <= budgets.parse_bytes {
                break;
            }
            if after.memo_bytes <= budgets.query_bytes
                && self.memos.get(&key).is_some_and(|m| {
                    m.value.parse_bytes() == 0
                        && !matches!(m.value.as_ref(), QueryValue::PatternPlan(_))
                })
            {
                continue;
            }
            let memo = self.memos.remove(&key).expect("selected retained memo");
            let parse = memo.value.parse_bytes();
            let shared_body = match memo.value.as_ref() {
                QueryValue::BodyStructure(body) if Arc::strong_count(body) > 1 => {
                    account_add(body.estimated_bytes(), 2 * std::mem::size_of::<usize>())?
                }
                _ => 0,
            };
            let shared_parse = match memo.value.as_ref() {
                QueryValue::ParseFile(parsed) if Arc::strong_count(parsed) > 1 => parse,
                _ => 0,
            };
            // Last owning cached plan releases its parser/body too. Shared
            // allocations remain charged until their actual final owner drops.
            if let QueryValue::PatternPlan(plan) = memo.value.as_ref() {
                if Arc::strong_count(plan.index()) == 1 {
                    after.memo_bytes = after.memo_bytes.saturating_sub(account_add(
                        plan.index().estimated_bytes(),
                        2 * std::mem::size_of::<usize>(),
                    )?);
                }
                if Arc::strong_count(plan.parsed()) == 1 {
                    after.parse_bytes = after
                        .parse_bytes
                        .saturating_sub(plan.parsed().estimated_bytes());
                }
            }
            after.memo_bytes = after.memo_bytes.saturating_sub(
                memo.estimated_bytes
                    .saturating_sub(parse)
                    .saturating_sub(shared_body),
            );
            after.parse_bytes = after
                .parse_bytes
                .saturating_sub(parse.saturating_sub(shared_parse));
            after.memo_entries -= 1;
            self.trace.record(
                self.revision,
                key.clone(),
                Action::Evicted,
                "between-run budget",
                vec![],
            );
            victims.push(key);
        }
        Ok(crate::engine::telemetry::CacheEviction {
            before,
            after,
            victims,
        })
    }
    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn memo(&self, key: &QueryKey) -> Option<&MemoEntry> {
        self.memos.get(key)
    }
    /// Replace the complete immutable input universe atomically, including deletions.
    pub fn publish_inputs(
        &mut self,
        mut inputs: BTreeMap<InputKey, InputValue>,
    ) -> Result<(), Diagnostic> {
        if self.run_active || !self.active_queries.is_empty() {
            return Err(fault("LRR-E-INTERNAL-SNAPSHOT", "publication during query"));
        }
        if inputs.values().any(|value| {
            matches!(
                value,
                InputValue::ExpansionRecord(_)
                    | InputValue::ModuleRecord { .. }
                    | InputValue::IncludeRecord { .. }
                    | InputValue::DiscoverySeal(_)
                    | InputValue::GeneratedSyntax(_)
                    | InputValue::RetainedSyntax(_)
            )
        }) {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "expansion/include records require derived input publication",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        let mut staged = BTreeMap::new();
        let mut granular = BTreeMap::new();
        for (key, value) in &mut inputs {
            if let (InputKey::ScopeMembers(scope), InputValue::ScopeMembers(members)) = (key, value)
            {
                members.normalize();
                granular.insert(
                    InputKey::ScopeBlockers(scope.clone()),
                    InputValue::ScopeBlockers(members.blockers.clone()),
                );
                for ((namespace, name), candidates) in &members.candidates {
                    if !candidates.is_empty() {
                        granular.insert(
                            InputKey::NameCandidates(super::key::BucketKey {
                                scope: scope.clone(),
                                namespace: *namespace,
                                name: name.clone(),
                            }),
                            InputValue::NameCandidates(candidates.clone()),
                        );
                    }
                }
            }
        }
        for (key, value) in granular {
            if inputs.insert(key, value).is_some() {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "conflicting granular scope publication",
                ));
            }
        }
        for (key, value) in inputs {
            if !value.matches(&key) {
                return Err(fault("LRR-E-INTERNAL-SNAPSHOT", "input variant mismatch"));
            }
            if let (InputKey::SourceBlob(d), InputValue::SourceBlob(b)) = (&key, &value)
                && *d != b.content()
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "source content identity mismatch",
                ));
            }
            staged.insert(
                key,
                InputRecord {
                    fingerprint: value.fingerprint(),
                    value: Arc::new(value),
                },
            );
        }
        self.inputs = staged;
        self.scope_certificates = Default::default();
        self.revision = revision;
        self.source_revision = revision;
        Ok(())
    }
    /// Coordinator-only atomic publication. Complete certificates never arise inside queries.
    pub(crate) fn publish_discovery(
        &mut self,
        input: super::seals::DiscoveryInput,
        records: &[Arc<crate::expand::record::ExpansionRecord>],
        modules: &[Arc<crate::source::module_syntax::ModuleSyntaxInput>],
        ordinary_modules: &[Arc<crate::source::ordinary_module::OrdinaryModuleSyntaxInput>],
        derived: BTreeMap<InputKey, InputValue>,
    ) -> Result<(), Diagnostic> {
        use crate::core::digest::StableEncode;
        if !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "discovery publication during pure query",
            ));
        }
        input.seal.key().validate()?;
        if input.records.len() != records.len()
            || records.len() > 100_000
            || input.sources.is_empty()
            || input.sources.len() > 100_000
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "discovery publication record/source count differs",
            ));
        }
        if !matches!(self.inputs.get(&InputKey::SourceDependencies).map(|value| value.value.as_ref()), Some(InputValue::SourceDependencies(graph)) if graph.fingerprint() == input.source_dependencies)
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "discovery source dependency graph is stale or absent",
            ));
        }
        // Each immutable inventory source must be one of this publication's
        // captured sources or an actually completed declaration/argument facet.
        for scan in input.declaration_inventory.scan_handles() {
            let source_owned = scan.generated_origin().is_none()
                && input.sources.iter().any(|source| {
                    scan.parse() == &source.outline.parse
                        && scan.path() == &source.path
                        && scan.cfg() == source.outline.cfg
                        && scan.scope().crate_key == source.outline.crate_key
                        && scan.scope().module_path == source.outline.module_path
                        && scan.scope().owner.is_none()
                });
            let fragment_owned = input
                .declaration_fragments
                .iter()
                .chain(&input.argument_fragments)
                .any(|fragment| {
                    fragment
                        .inventory()
                        .scan_handles()
                        .iter()
                        .any(|owned| owned.fingerprint() == scan.fingerprint())
                });
            let argument_owned = input
                .argument_indexes
                .iter()
                .any(|index| index.argument().scan().fingerprint() == scan.fingerprint());
            if !source_owned && !fragment_owned && !argument_owned {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "declaration inventory source has no publication owner",
                ));
            }
        }
        for (key, fingerprint) in &input.compiler_inputs {
            if !matches!(
                key,
                InputKey::ModuleRecord(_)
                    | InputKey::IncludeRecord(_)
                    | InputKey::UnitCompileEnv(_)
                    | InputKey::ExpansionRecord(_)
            ) || self
                .inputs
                .get(key)
                .is_none_or(|row| row.fingerprint != *fingerprint)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery compiler input is stale or absent",
                ));
            }
            if input
                .seal
                .input_dependencies()
                .binary_search(&(key.fingerprint(), *fingerprint))
                .is_err()
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "compiler input is absent from discovery seal",
                ));
            }
        }
        if let Some(scope) = &input.scope
            && input.seal.domain()
                != &(super::seals::SealDomain::Scope {
                    id: scope.scope.fingerprint(),
                    namespace: scope.namespace,
                })
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "certified namespace owner differs from seal",
            ));
        }
        let epoch = self
            .inputs
            .get(&InputKey::ExpansionEpoch)
            .map(|v| v.value.as_ref());
        if epoch.is_some_and(
            |v| !matches!(v,InputValue::ExpansionEpoch(e) if *e==input.seal.key().epoch),
        ) {
            return Err(fault(
                "LRR-E-QUERY-REQUEST",
                "discovery publication has stale epoch",
            ));
        }
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(
                records
                    .len()
                    .checked_add(input.sources.len())
                    .and_then(|n| n.checked_add(input.compiler_inputs.len()))
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "discovery dependency count overflow",
                        )
                    })?,
            )
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "discovery dependency allocation"))?;
        if modules
            .len()
            .checked_add(ordinary_modules.len())
            .is_none_or(|n| n > 100_000)
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "module publication count limit",
            ));
        }
        let mut module_rows = BTreeMap::new();
        let mut module_bytes = 0usize;
        for module in modules {
            let origin = module.origin();
            let record_key = InputKey::ModuleRecord(origin.capture().input_identity());
            let record = self.inputs.get(&record_key).ok_or_else(|| {
                fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "module publication adopted record absent",
                )
            })?;
            if !matches!(record.value.as_ref(), InputValue::ModuleRecord { captured, .. }
                if captured.identity() == origin.capture().record().identity())
                || input.compiler_inputs.get(&record_key) != Some(&record.fingerprint)
                || !input.sources.contains(module.prepared_source().selection())
                || input
                    .seal
                    .input_dependencies()
                    .binary_search(&(
                        origin.parent().record().key().fingerprint(),
                        origin.parent().record().descriptor().fingerprint(),
                    ))
                    .is_err()
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "module publication provenance differs from seal",
                ));
            }
            module_bytes = module_bytes
                .checked_add(module.metadata_bytes())
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "module publication projection metadata limit",
                    )
                })?;
            for (key, expected) in module.inputs() {
                if !derived
                    .get(key)
                    .is_some_and(|actual| actual.fingerprint() == expected.fingerprint())
                {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "module publication source projection is incomplete or substituted",
                    ));
                }
                let fingerprint = expected.fingerprint();
                if module_rows
                    .insert(key.clone(), fingerprint)
                    .is_some_and(|old| old != fingerprint)
                {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "module publication row identity conflict",
                    ));
                }
            }
        }
        for module in ordinary_modules {
            let origin = module.origin();
            let record_key = InputKey::ModuleRecord(origin.capture().input_identity());
            let record = self.inputs.get(&record_key).ok_or_else(|| {
                fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "ordinary publication adopted record absent",
                )
            })?;
            let parent = origin.declaration().source().selection();
            if !matches!(record.value.as_ref(), InputValue::ModuleRecord { captured, .. }
                if captured.identity() == origin.capture().record().identity())
                || input.compiler_inputs.get(&record_key) != Some(&record.fingerprint)
                || !input.sources.contains(module.prepared_source().selection())
                || !input.sources.contains(parent)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "ordinary publication provenance differs from seal",
                ));
            }
            module_bytes = module_bytes
                .checked_add(module.metadata_bytes())
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "ordinary publication projection metadata limit",
                    )
                })?;
            for (key, expected) in module.inputs() {
                if !derived
                    .get(key)
                    .is_some_and(|actual| actual.fingerprint() == expected.fingerprint())
                {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "ordinary publication source projection incomplete or substituted",
                    ));
                }
                let fingerprint = expected.fingerprint();
                if module_rows
                    .insert(key.clone(), fingerprint)
                    .is_some_and(|old| old != fingerprint)
                {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "ordinary publication row identity conflict",
                    ));
                }
            }
        }
        let current = |key: &InputKey| -> Option<&InputValue> {
            self.inputs
                .get(key)
                .map(|row| row.value.as_ref())
                .or_else(|| {
                    module_rows
                        .contains_key(key)
                        .then(|| derived.get(key))
                        .flatten()
                })
        };
        for source in &input.sources {
            source.canonical()?;
            let key = InputKey::CapturedSource(source.location_identity());
            match current(&key) {
                Some(InputValue::CapturedSource(actual)) if actual == source => {}
                _ => {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "discovery source is absent or changed in current publication",
                    ));
                }
            }
            if !matches!(current(&InputKey::SourceBlob(source.outline.parse.content)),Some(InputValue::SourceBlob(blob)) if blob.content()==source.outline.parse.content)
                || !matches!(current(&InputKey::CrateCfg(source.outline.crate_key)),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==source.outline.cfg)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery source content/cfg differs in current publication",
                ));
            }
            let outline = QueryKey::Outline(source.outline.clone()).fingerprint();
            let index = input
                .seal
                .input_dependencies()
                .binary_search_by_key(&outline, |(id, _)| *id)
                .map_err(|_| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "discovery certificate lacks scanned source",
                    )
                })?;
            let fingerprint = input.seal.input_dependencies()[index].1;
            dependencies.push((outline, fingerprint));
        }
        let mut extra_bytes = 0usize;
        for (record, descriptor) in records.iter().zip(&input.records) {
            record.key().validate()?;
            if record.descriptor().fingerprint() != descriptor.fingerprint()
                || record.key() != &descriptor.key
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery descriptor/record mismatch",
                ));
            }
            if let Some(expected) = record.key().procedural_epoch
                && !matches!(epoch,Some(InputValue::ExpansionEpoch(actual)) if *actual==expected && expected==input.seal.key().epoch)
            {
                return Err(fault(
                    "LRR-E-QUERY-REQUEST",
                    "discovery procedural record lacks matching current epoch",
                ));
            }
            let key = InputKey::ExpansionRecord(record.key().fingerprint());
            dependencies.push((record.key().fingerprint(), descriptor.fingerprint()));
            if let Some(old) = self.inputs.get(&key) {
                if !matches!(old.value.as_ref(),InputValue::ExpansionRecord(actual) if actual.same_record(record))
                {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "immutable discovery expansion conflict",
                    ));
                }
            } else {
                let bytes = record
                    .estimated_bytes()?
                    .checked_add(std::mem::size_of::<InputValue>())
                    .and_then(|n| {
                        n.checked_add(
                            64 + std::mem::size_of::<InputKey>()
                                + std::mem::size_of::<InputRecord>()
                                + 16,
                        )
                    })
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "discovery record size overflow")
                    })?;
                extra_bytes = extra_bytes.checked_add(bytes).ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "discovery publication size overflow",
                    )
                })?;
            }
        }
        dependencies.extend(
            input
                .compiler_inputs
                .iter()
                .map(|(key, fingerprint)| (key.fingerprint(), *fingerprint)),
        );
        if input.procedural_catalogs.len() > 4096 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "discovery procedural catalog count limit",
            ));
        }
        let mut previous = None;
        let mut catalog_bytes = 0usize;
        for catalog in &input.procedural_catalogs {
            let fingerprint = catalog.fingerprint();
            if previous.is_some_and(|old| old >= fingerprint)
                || catalog.graph() != input.seal.key().unit_graph
                || catalog.epoch() != input.seal.key().epoch
                || !input
                    .sources
                    .iter()
                    .any(|source| source.outline.crate_key == catalog.consumer())
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery procedural catalog provenance/order differs",
                ));
            }
            catalog_bytes = catalog_bytes
                .checked_add(catalog.metadata_bytes()?)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "discovery procedural catalog metadata limit",
                    )
                })?;
            previous = Some(fingerprint);
            dependencies.push((fingerprint, fingerprint));
        }
        let mut previous_helper = None;
        let mut helper_bytes = 0usize;
        for (request, receipt) in &input.helper_declarations {
            receipt.validate_request(request)?;
            if previous_helper.is_some_and(|old| old >= request.demand())
                || request.epoch != input.seal.key().epoch
                || request.graph != input.seal.key().unit_graph
                || !request.origin.published_in(&input.sources, &input.records)
                || !input.procedural_catalogs.iter().any(|catalog| {
                    catalog.fingerprint() == request.catalog
                        && catalog.consumer() == request.consumer
                        && catalog.environment() == request.environment
                        && catalog
                            .select(
                                &request.definition,
                                crate::expand::identity::ProceduralKind::Derive,
                            )
                            .is_ok_and(|selected| {
                                selected
                                    .is_some_and(|export| export.identity() == &request.identity)
                            })
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery helper declaration provenance or order differs",
                ));
            }
            helper_bytes = helper_bytes
                .checked_add(request.metadata_bytes()?)
                .and_then(|n| n.checked_add(receipt.estimated_bytes().ok()?))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "discovery helper declaration metadata limit",
                    )
                })?;
            dependencies.push((request.demand(), receipt.fingerprint()));
            previous_helper = Some(request.demand());
        }
        let published_descriptors = input
            .records
            .iter()
            .map(|descriptor| descriptor.fingerprint())
            .collect::<std::collections::BTreeSet<_>>();
        let mut previous_fragment = None;
        let mut fragment_bytes = 0usize;
        if input.declaration_fragments.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "declaration fragment publication count limit",
            ));
        }
        for fragment in &input.declaration_fragments {
            let output = fragment.index().output();
            let identity = output.record().key().invocation;
            let source = fragment.source_scan();
            if previous_fragment.is_some_and(|old| old >= identity)
                || !fragment.index().is_complete()
                || output.epoch() != input.seal.key().epoch
                || output.parent_inventory() != source.fingerprint()
                || !input.sources.iter().any(|selected| {
                    selected.outline.parse == *source.parse()
                        && selected.path == *source.path()
                        && selected.outline.cfg == source.cfg()
                        && selected.outline.crate_key == source.scope().crate_key
                        && selected.outline.module_path == source.scope().module_path
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "declaration fragment source/epoch/order differs",
                ));
            }
            let Some(InputValue::CrateCfg(cfg)) =
                current(&InputKey::CrateCfg(source.scope().crate_key))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated intrinsic cfg absent",
                ));
            };
            fragment.validate_intrinsic_arguments(cfg)?;
            for record in fragment.records() {
                if !published_descriptors.contains(&record.descriptor().fingerprint()) {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "declaration fragment record absent from complete publication",
                    ));
                }
            }
            fragment_bytes = fragment_bytes
                .checked_add(fragment.metadata_bytes())
                .filter(|n| *n <= 512 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "declaration fragment publication metadata limit",
                    )
                })?;
            previous_fragment = Some(identity);
            dependencies.push((identity, output.fingerprint()));
        }
        if input.source_replacements.len() > 100_000
            || input
                .source_replacements
                .windows(2)
                .any(|pair| pair[0].fingerprint() >= pair[1].fingerprint())
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "replacement proofs unordered or excessive",
            ));
        }
        let mut replacement_outlines = std::collections::BTreeMap::new();
        let mut replacement_bytes = 0usize;
        for proof in &input.source_replacements {
            let selected = proof.source();
            if !input.sources.contains(selected)
                || proof.descriptor().key.procedural_epoch != Some(input.seal.key().epoch)
                || !published_descriptors.contains(&proof.descriptor().fingerprint())
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "replacement record/source absent from complete publication",
                ));
            }
            if !replacement_outlines.contains_key(&selected.outline) {
                let Some(InputValue::SourceBlob(blob)) =
                    current(&InputKey::SourceBlob(selected.outline.parse.content))
                else {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "replacement source blob absent",
                    ));
                };
                let Some(InputValue::CrateCfg(cfg)) =
                    current(&InputKey::CrateCfg(selected.outline.crate_key))
                else {
                    return Err(fault("LRR-E-INTERNAL-SNAPSHOT", "replacement cfg absent"));
                };
                let parsed = crate::syntax::parse::ParsedFile::parse_declarations(
                    blob,
                    selected.outline.parse.profile.clone(),
                )?;
                replacement_bytes = replacement_bytes
                    .checked_add(parsed.estimated_bytes())
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "replacement parser metadata limit")
                    })?;
                let outline = parsed.outline(
                    selected.outline.crate_key,
                    selected.outline.module_path.clone(),
                    cfg,
                )?;
                replacement_bytes = replacement_bytes
                    .checked_add(outline.estimated_bytes())
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "replacement outline metadata limit")
                    })?;
                replacement_outlines.insert(selected.outline.clone(), outline);
            }
            proof.validate_outline(
                replacement_outlines.get(&selected.outline).ok_or_else(|| {
                    fault("LRR-E-INTERNAL-SNAPSHOT", "replacement outline absent")
                })?,
            )?;
            dependencies.push((proof.fingerprint(), proof.fingerprint()));
        }
        let mut runtime_parses = std::collections::BTreeMap::new();
        let mut runtime_source_bytes = 0usize;
        if input.argument_indexes.len() > 100_000
            || input
                .argument_indexes
                .windows(2)
                .any(|w| w[0].argument().fingerprint() >= w[1].argument().fingerprint())
            || input.argument_fragments.windows(2).any(|w| {
                w[0].index().output().record().key().fingerprint()
                    >= w[1].index().output().record().key().fingerprint()
            })
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "argument certificates unordered or excessive",
            ));
        }
        for index in &input.argument_indexes {
            if !input
                .argument_summaries
                .iter()
                .flat_map(|(_, _, args)| args)
                .any(|argument| index.completed_for(argument))
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "argument completion certificate absent from summaries",
                ));
            }
        }
        let mut previous_argument_set = None;
        let mut argument_bytes = 0usize;
        if input.argument_summaries.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "intrinsic argument summary count limit",
            ));
        }
        for (identity, source, arguments) in &input.argument_summaries {
            if previous_argument_set.is_some_and(|old| old >= *identity)
                || arguments.is_empty()
                || arguments.len() > 10_000
                || !input.sources.iter().any(|selected| {
                    selected.outline.parse == *source.parse()
                        && selected.path == *source.path()
                        && selected.outline.cfg == source.cfg()
                        && selected.outline.crate_key == source.scope().crate_key
                        && selected.outline.module_path == source.scope().module_path
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "intrinsic argument summary source/order differs",
                ));
            }
            let Some(InputValue::CrateCfg(cfg)) =
                current(&InputKey::CrateCfg(source.scope().crate_key))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "intrinsic argument summary cfg absent",
                ));
            };
            let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(
                &source.parse().profile,
                arguments[0].definition(),
            )?;
            let Some(InputValue::SourceBlob(blob)) =
                current(&InputKey::SourceBlob(source.parse().content))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "intrinsic argument source blob absent",
                ));
            };
            let selected = input
                .sources
                .iter()
                .find(|selected| {
                    selected.outline.parse == *source.parse() && selected.path == *source.path()
                })
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "intrinsic argument source selection absent",
                    )
                })?;
            let forest = crate::expand::argument_declarations::ArgumentDeclarations::source_tokens(
                selected, blob,
            )?;
            runtime_source_bytes = runtime_source_bytes
                .checked_add(blob.bytes().len())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "intrinsic argument source parse budget",
                    )
                })?;
            // Current captured source/content/cfg were checked above. Reuse
            // the explicitly prepared parser so dependency editions remain
            // attached to their syntax; fallback retains validated live admission.
            let parsed = match self.prepared_parse(source.parse()) {
                Some(parsed) => parsed,
                None => Arc::new(crate::syntax::parse::ParsedFile::parse(
                    blob,
                    source.parse().profile.clone(),
                )?),
            };
            let producer = source
                .producers()
                .iter()
                .find(|producer| producer.identity == *identity)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "intrinsic argument producer absent",
                    )
                })?;
            let call = parsed.macro_call_at(producer.range)?;
            let prepared = crate::frontend::runtime_macro_plan::prepare_runtime_macro(
                &parsed,
                call,
                crate::frontend::macro_names::PathLookup::Defined(
                    arguments[0].definition().clone(),
                ),
                &catalog,
            )?;
            if !matches!(prepared, crate::frontend::runtime_macro_plan::RuntimeMacroPreparation::Intrinsic(plan)
                if plan.arguments().declaration_arguments().iter().copied().eq(arguments.iter().map(|argument| argument.source_range())))
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "intrinsic argument sequence differs from actual syntax",
                ));
            }
            for argument in arguments {
                if argument.invocation() != *identity
                    || !input
                        .argument_indexes
                        .iter()
                        .any(|index| index.completed_for(argument))
                    || catalog.select(argument.definition())?.is_none()
                {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "intrinsic argument definition/completion differs",
                    ));
                }
                for producer in argument.scan().producers() {
                    let index = input
                        .argument_indexes
                        .iter()
                        .find(|index| index.completed_for(argument))
                        .ok_or_else(|| {
                            fault(
                                "LRR-E-INTERNAL-SNAPSHOT",
                                "argument completion index absent",
                            )
                        })?;
                    if index.validates_inert(argument.scan(), producer.identity)? {
                        continue;
                    }
                    if !input.argument_fragments.iter().any(|fragment| {
                        fragment.index().output().parent_inventory()
                            == argument.scan().fingerprint()
                            && fragment.index().output().record().key().invocation
                                == producer.identity
                            && fragment.index().is_complete()
                    }) {
                        return Err(fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "argument producer has no complete exact fragment",
                        ));
                    }
                }
                argument.validate_source(source, cfg, &forest)?;
                argument_bytes = argument_bytes
                    .checked_add(argument.metadata_bytes())
                    .filter(|n| *n <= 512 * 1024 * 1024)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "intrinsic argument publication metadata limit",
                        )
                    })?;
            }
            // The worklist uses an explicit length-prefixed argument sequence.
            let mut sequence =
                crate::core::digest::StableEncoder::new("lrr/v2/intrinsic-argument-set");
            sequence.sequence(arguments, |e, argument| e.record(argument.as_ref()));
            dependencies.push((*identity, sequence.finish()));
            previous_argument_set = Some(*identity);
        }
        if input.argument_fragments.len() > 4096 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "intrinsic argument fragment count limit",
            ));
        }
        for fragment in &input.argument_fragments {
            let output = fragment.index().output();
            let Some(argument) = input
                .argument_summaries
                .iter()
                .flat_map(|(_, _, args)| args)
                .find(|argument| argument.scan().fingerprint() == output.parent_inventory())
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "argument fragment parent absent",
                ));
            };
            if !fragment.index().is_complete()
                || output.epoch() != input.seal.key().epoch
                || !argument
                    .scan()
                    .producers()
                    .iter()
                    .any(|producer| producer.identity == output.record().key().invocation)
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "argument fragment completion differs",
                ));
            }
            let Some(InputValue::CrateCfg(cfg)) =
                current(&InputKey::CrateCfg(argument.scan().scope().crate_key))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "argument generated intrinsic cfg absent",
                ));
            };
            fragment.validate_intrinsic_arguments(cfg)?;
            for record in fragment.records() {
                if !published_descriptors.contains(&record.descriptor().fingerprint()) {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "argument fragment record absent",
                    ));
                }
            }
        }
        if input.nested_inert_proofs.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "nested inert proof count limit",
            ));
        }
        let mut previous_inert = None;
        let mut inert_bytes = 0usize;
        for proof in &input.nested_inert_proofs {
            let scan = proof.scan();
            if previous_inert.is_some_and(|old| old >= proof.producer())
                || !input.sources.iter().any(|selected| {
                    selected.outline.parse == *scan.parse()
                        && selected.path == *scan.path()
                        && selected.outline.cfg == scan.cfg()
                        && selected.outline.crate_key == scan.scope().crate_key
                        && selected.outline.module_path == scan.scope().module_path
                })
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "nested inert proof source/order differs",
                ));
            }
            let Some(InputValue::SourceBlob(blob)) =
                current(&InputKey::SourceBlob(scan.parse().content))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "nested inert proof source blob absent",
                ));
            };
            let Some(InputValue::CrateCfg(cfg)) =
                current(&InputKey::CrateCfg(scan.scope().crate_key))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "nested inert proof cfg absent",
                ));
            };
            inert_bytes = inert_bytes
                .checked_add(blob.bytes().len())
                .and_then(|n| n.checked_add(proof.metadata_bytes().ok()?))
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "nested inert proof publication budget",
                    )
                })?;
            proof.revalidate(blob, cfg)?;
            dependencies.push((proof.producer(), proof.fingerprint()));
            previous_inert = Some(proof.producer());
        }
        let mut previous_runtime = None;
        let mut runtime_bytes = 0usize;
        if input.runtime_no_item_proofs.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "discovery runtime proof count limit",
            ));
        }
        for (identity, proof) in &input.runtime_no_item_proofs {
            if previous_runtime.is_some_and(|old| old >= *identity)
                || !input
                    .sources
                    .iter()
                    .any(|source| &source.outline.parse == proof.source())
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery runtime proof source/order differs",
                ));
            }
            let Some(InputValue::DefinitionBody(body)) =
                current(&InputKey::DefinitionBody(proof.body().definition))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery runtime proof body absent",
                ));
            };
            if &body.key() != proof.body()
                || proof.invocation().start < body.range.start
                || proof.invocation().end > body.range.end
                || !matches!(current(&InputKey::DefinitionSource(proof.body().definition)), Some(InputValue::DefinitionSource(source)) if source == proof.source())
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery runtime proof body/source differs",
                ));
            }
            let Some(InputValue::SourceBlob(blob)) =
                current(&InputKey::SourceBlob(proof.source().content))
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery runtime proof source blob absent",
                ));
            };
            if !runtime_parses.contains_key(proof.source()) {
                runtime_source_bytes = runtime_source_bytes
                    .checked_add(blob.bytes().len())
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "runtime proof source parse budget")
                    })?;
                runtime_parses.insert(
                    proof.source().clone(),
                    match self.prepared_parse(proof.source()) {
                        Some(parsed) => parsed,
                        None => Arc::new(crate::syntax::parse::ParsedFile::parse(
                            blob,
                            proof.source().profile.clone(),
                        )?),
                    },
                );
            }
            let parsed = runtime_parses.get(proof.source()).ok_or_else(|| {
                fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "runtime proof parser input absent",
                )
            })?;
            let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(
                &proof.source().profile,
                proof.definition(),
            )?;
            let call = parsed.macro_call_at(proof.invocation())?;
            let prepared = crate::frontend::runtime_macro_plan::prepare_runtime_macro(
                parsed,
                call,
                crate::frontend::macro_names::PathLookup::Defined(proof.definition().clone()),
                &catalog,
            )?;
            if !matches!(prepared, crate::frontend::runtime_macro_plan::RuntimeMacroPreparation::Intrinsic(plan) if plan.no_item_proof(proof.body()).as_ref() == Some(proof))
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "discovery runtime proof does not revalidate",
                ));
            }
            runtime_bytes =
                runtime_bytes
                    .checked_add(proof.allocated_bytes().ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "runtime proof size overflow")
                    })?)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "runtime proof metadata limit"))?;
            previous_runtime = Some(*identity);
            dependencies.push((*identity, proof.fingerprint()));
        }
        dependencies.sort();
        if dependencies.as_slice() != input.seal.input_dependencies() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "discovery certificate dependencies differ from complete publication",
            ));
        }
        let mut staged = BTreeMap::new();
        for (key, value) in derived {
            if !value.matches(&key) {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated query input variant differs",
                ));
            }
            if let Some(expected) = module_rows.get(&key) {
                if value.fingerprint() != *expected {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "module source row differs",
                    ));
                }
            } else {
                match (&key, &value) {
                    (InputKey::GeneratedSyntax(_), InputValue::GeneratedSyntax(unit)) => {
                        if unit.seal != input.seal.key()
                            || input
                                .seal
                                .input_dependencies()
                                .binary_search(&(
                                    unit.items.record().key().fingerprint(),
                                    unit.items.record().descriptor().fingerprint(),
                                ))
                                .is_err()
                        {
                            return Err(fault(
                                "LRR-E-INTERNAL-SNAPSHOT",
                                "generated syntax lacks matching discovery dependency",
                            ));
                        }
                    }
                    (InputKey::DefinitionHeader(_), InputValue::DefinitionHeader(_))
                    | (InputKey::DefinitionSource(_), InputValue::DefinitionSource(_))
                    | (InputKey::DeclarationContext(_), InputValue::DeclarationContext(_))
                    | (InputKey::DefinitionBody(_), InputValue::DefinitionBody(_)) => {}
                    _ => {
                        return Err(fault(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "unexpected generated query input kind",
                        ));
                    }
                }
            }
            if let Some(old) = self.inputs.get(&key) {
                let same = match (old.value.as_ref(), &value) {
                    (InputValue::GeneratedSyntax(old), InputValue::GeneratedSyntax(new)) => {
                        old.same_unit(new)
                    }
                    _ => old.fingerprint == value.fingerprint(),
                };
                if !same {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "immutable generated query input conflict",
                    ));
                }
            } else {
                extra_bytes = extra_bytes
                    .checked_add(input_metadata_bytes(&key, &value)?)
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "generated query metadata overflow")
                    })?;
                staged.insert(
                    key,
                    InputRecord {
                        fingerprint: value.fingerprint(),
                        value: Arc::new(value),
                    },
                );
            }
        }
        let prepared_sources = self.module_prepared_registry(modules, ordinary_modules)?;
        let scope_certificates = self.scope_certificates.with_input(&input)?;
        let usage =
            self.cache_usage_with_prepared(&prepared_sources, prepared_sources.capacity())?;
        let budgets = self.run_budgets.unwrap_or_default();
        if usage.parse_bytes > budgets.parse_bytes
            || usage.source_input_bytes > budgets.source_input_bytes
            || usage.memo_bytes > budgets.query_bytes
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "discovery prepared source registry exceeds cache budgets",
            ));
        }
        let seal_key = InputKey::DiscoverySeal(input.seal.key());
        let value = InputValue::DiscoverySeal(Box::new(input));
        if let Some(old) = self.inputs.get(&seal_key) {
            if old.fingerprint != value.fingerprint() {
                return Err(fault(
                    "LRR-E-EXPANSION-SEAL-VIOLATION",
                    "immutable discovery certificate conflict",
                ));
            }
            if extra_bytes == 0 {
                // The identical publication can still supply a live immutable
                // module lease after its earlier weak owner was dropped.
                self.prepared_sources = prepared_sources;
                return Ok(());
            }
        } else {
            extra_bytes = extra_bytes
                .checked_add(input_metadata_bytes(&seal_key, &value)?)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "discovery publication size overflow",
                    )
                })?;
        }
        let index_extra = scope_certificates
            .as_ref()
            .map(|index| {
                index
                    .metadata_bytes()
                    .checked_sub(self.scope_certificates.metadata_bytes())
            })
            .unwrap_or(Some(0))
            .ok_or_else(|| {
                fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "scope certificate index accounting decreased",
                )
            })?;
        let total = usage
            .memo_bytes
            .checked_add(extra_bytes)
            .and_then(|bytes| bytes.checked_add(index_extra))
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "discovery publication metadata overflow",
                )
            })?;
        if total > self.run_budgets.unwrap_or_default().query_bytes {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "discovery publication exceeds query metadata cap",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        for record in records {
            let key = InputKey::ExpansionRecord(record.key().fingerprint());
            if !self.inputs.contains_key(&key) {
                let value = InputValue::ExpansionRecord(record.try_copy()?);
                staged.insert(
                    key,
                    InputRecord {
                        fingerprint: value.fingerprint(),
                        value: Arc::new(value),
                    },
                );
            }
        }
        if !self.inputs.contains_key(&seal_key) {
            staged.insert(
                seal_key,
                InputRecord {
                    fingerprint: value.fingerprint(),
                    value: Arc::new(value),
                },
            );
        }
        self.inputs.extend(staged);
        if let Some(index) = scope_certificates {
            self.scope_certificates = index;
        }
        self.prepared_sources = prepared_sources;
        self.revision = revision;
        Ok(())
    }
    /// Begin a new coordinator epoch without discarding captured source inputs.
    pub fn begin_expansion_epoch(&mut self, epoch: u64) -> Result<(), Diagnostic> {
        if self.run_active || !self.active_queries.is_empty() || epoch == 0 {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "invalid expansion epoch transition during work",
            ));
        }
        let key = InputKey::ExpansionEpoch;
        if self.inputs.get(&key).is_some_and(|old| !matches!(old.value.as_ref(), InputValue::ExpansionEpoch(previous) if epoch > *previous)) {
            return Err(fault("LRR-E-EXPANSION-INPUT", "expansion epoch must advance"));
        }
        let value = InputValue::ExpansionEpoch(epoch);
        let bytes = input_metadata_bytes(&key, &value)?;
        let total = self
            .cache_usage()?
            .memo_bytes
            .checked_add(bytes)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "epoch input metadata overflow"))?;
        if total > self.run_budgets.unwrap_or_default().query_bytes {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "epoch input exceeds query metadata cap",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        self.inputs.insert(
            key,
            InputRecord {
                fingerprint: value.fingerprint(),
                value: Arc::new(value),
            },
        );
        self.revision = revision;
        Ok(())
    }
    /// Append a derived expansion input without replacing frozen base inputs.
    /// Providers cannot publish while a query dependency stack is active.
    pub(crate) fn publish_expansion(
        &mut self,
        record: crate::expand::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        if !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "expansion publication during pure query",
            ));
        }
        use crate::core::digest::StableEncode;
        if let Some(epoch) = record.key().procedural_epoch
            && let Some(active) = self.inputs.get(&InputKey::ExpansionEpoch)
            && !matches!(active.value.as_ref(), InputValue::ExpansionEpoch(current) if *current == epoch)
        {
            return Err(fault(
                "LRR-E-QUERY-REQUEST",
                "procedural record publication has stale epoch",
            ));
        }
        let key = InputKey::ExpansionRecord(record.key().fingerprint());
        if let Some(old) = self.inputs.get(&key) {
            return match old.value.as_ref() {
                InputValue::ExpansionRecord(old) if old.same_record(&record) => Ok(()),
                _ => Err(fault(
                    "LRR-E-EXPANSION-CONFLICT",
                    "immutable expansion record conflict",
                )),
            };
        }
        let value = InputValue::ExpansionRecord(record);
        let bytes = input_metadata_bytes(&key, &value)?;
        let total = self
            .cache_usage()?
            .memo_bytes
            .checked_add(bytes)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "expansion input metadata overflow"))?;
        if total > self.run_budgets.unwrap_or_default().query_bytes {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "expansion input exceeds query metadata cap",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        let fingerprint = value.fingerprint();
        self.inputs.insert(
            key,
            InputRecord {
                value: Arc::new(value),
                fingerprint,
            },
        );
        self.revision = revision;
        Ok(())
    }
    /// Coordinator-only immutable capture publication; never an effect in a provider.
    pub(crate) fn publish_include(
        &mut self,
        input: Digest,
        captured: crate::source::included::CapturedInclude,
    ) -> Result<(), Diagnostic> {
        if !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "include publication during pure query",
            ));
        }
        let key = InputKey::IncludeRecord(input);
        if let Some(old) = self.inputs.get(&key) {
            return match old.value.as_ref() {
                InputValue::IncludeRecord { captured: old, .. }
                    if old.identity() == captured.identity() =>
                {
                    Ok(())
                }
                _ => Err(fault(
                    "LRR-E-INCLUDE-CONFLICT",
                    "immutable include record conflict",
                )),
            };
        }
        let usage = self.cache_usage()?;
        let budgets = self.run_budgets.unwrap_or_default();
        let value = InputValue::IncludeRecord { input, captured };
        let metadata = account_add(usage.memo_bytes, input_metadata_bytes(&key, &value)?)?;
        let blob = value
            .source_blob()
            .ok_or_else(|| fault("LRR-E-INTERNAL-SNAPSHOT", "include record storage missing"))?;
        // Conservatively count a full blob before insertion; accounting afterward deduplicates storage.
        let source = account_add(usage.source_input_bytes, blob.bytes().len())?;
        if metadata > budgets.query_bytes || source > budgets.source_input_bytes {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "include input exceeds cache cap",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        let fingerprint = value.fingerprint();
        self.inputs.insert(
            key,
            InputRecord {
                value: Arc::new(value),
                fingerprint,
            },
        );
        self.revision = revision;
        Ok(())
    }
    /// Only exact coordinator admission may publish a module candidate universe.
    pub(crate) fn publish_module(
        &mut self,
        input: Digest,
        captured: crate::source::module_capture::CapturedModule,
    ) -> Result<(), Diagnostic> {
        if !self.active_queries.is_empty() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "module publication during pure query",
            ));
        }
        let key = InputKey::ModuleRecord(input);
        if let Some(old) = self.inputs.get(&key) {
            return match old.value.as_ref() {
                InputValue::ModuleRecord { captured: old, .. }
                    if old.identity() == captured.identity() =>
                {
                    Ok(())
                }
                _ => Err(fault(
                    "LRR-E-MODULE-CONFLICT",
                    "immutable module record conflict",
                )),
            };
        }
        let usage = self.cache_usage()?;
        let budgets = self.run_budgets.unwrap_or_default();
        let value = InputValue::ModuleRecord {
            input,
            captured: Arc::new(captured),
        };
        let metadata = account_add(usage.memo_bytes, input_metadata_bytes(&key, &value)?)?;
        let mut source = usage.source_input_bytes;
        for blob in value.source_blobs() {
            source = account_add(source, blob.bytes().len())?;
        }
        if metadata > budgets.query_bytes || source > budgets.source_input_bytes {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "module input exceeds cache cap",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        let fingerprint = value.fingerprint();
        self.inputs.insert(
            key,
            InputRecord {
                value: Arc::new(value),
                fingerprint,
            },
        );
        self.revision = revision;
        Ok(())
    }
    pub(crate) fn compiler_input_stamps(
        &self,
    ) -> Result<BTreeMap<InputKey, crate::core::digest::Digest>, Diagnostic> {
        let mut result = BTreeMap::new();
        for (key, row) in &self.inputs {
            if matches!(
                key,
                InputKey::ModuleRecord(_)
                    | InputKey::IncludeRecord(_)
                    | InputKey::UnitCompileEnv(_)
                    | InputKey::ExpansionRecord(_)
            ) {
                if result.len() >= 100_000 {
                    return Err(fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "compiler input universe count limit",
                    ));
                }
                result.insert(key.clone(), row.fingerprint);
            }
        }
        Ok(result)
    }
    /// Coordinator-side observation of actual reads, never an input-universe scan.
    pub(crate) fn observe_expansion_inputs<T>(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<(T, BTreeMap<Digest, crate::expand::record::ExpansionUsage>), Diagnostic> {
        use crate::core::digest::StableEncode;
        if !self.active_queries.is_empty() || self.expansion_reads.is_some() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "expansion observation requires coordinator boundary",
            ));
        }
        let revision = self.revision;
        self.expansion_reads = Some(ExpansionReadObservation::default());
        let result = work(self);
        let observed = self
            .expansion_reads
            .take()
            .expect("installed expansion read observation");
        let value = result?;
        if let Some(error) = observed.error {
            return Err(error);
        }
        if self.revision != revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "expansion observation input revision changed",
            ));
        }
        let mut usage = BTreeMap::new();
        for (identity, fingerprint) in observed.stamps {
            let row = self
                .inputs
                .get(&InputKey::ExpansionRecord(identity))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "observed expansion record disappeared",
                    )
                })?;
            let InputValue::ExpansionRecord(record) = row.value.as_ref() else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "observed expansion variant differs",
                ));
            };
            if row.fingerprint != fingerprint || record.key().fingerprint() != identity {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "observed expansion identity changed",
                ));
            }
            usage.insert(identity, record.usage());
        }
        Ok((value, usage))
    }
    /// Inspect immutable published input without recording a query dependency,
    /// retaining a pin, mutating trace state or servicing any provider demand.
    pub(crate) fn published_input(&self, key: &InputKey) -> Option<(&InputValue, Digest)> {
        self.inputs
            .get(key)
            .map(|record| (record.value.as_ref(), record.fingerprint))
    }
    pub fn input(&mut self, key: &InputKey) -> Option<Arc<InputValue>> {
        let (value, fingerprint) = match self.inputs.get(key) {
            Some(r) => (Some(r.value.clone()), r.fingerprint),
            None => (None, missing_fingerprint()),
        };
        if let Some(observation) = &mut self.expansion_reads
            && let InputKey::ExpansionRecord(identity) = key
            && value.is_some()
        {
            if observation
                .stamps
                .get(identity)
                .is_some_and(|old| *old != fingerprint)
            {
                observation.error = Some(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "expansion observation stamp conflict",
                ));
            } else if observation.stamps.len() >= 100_000
                && !observation.stamps.contains_key(identity)
            {
                observation.error = Some(fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "expansion observation count limit",
                ));
            } else {
                observation.stamps.insert(*identity, fingerprint);
            }
        }
        if let Some(value) = &value {
            self.input_pins
                .entry(Arc::as_ptr(value) as usize)
                .or_insert_with(|| Arc::downgrade(value));
        }
        self.record(DependencyStamp::Input {
            key: key.clone(),
            fingerprint,
        });
        value
    }
    fn record(&mut self, dependency: DependencyStamp) {
        if let Some(c) = self.collectors.last_mut() {
            c.dependencies.push(dependency);
        }
    }
    fn uncacheable(&mut self) {
        for c in &mut self.collectors {
            c.non_cacheable = true;
        }
    }
    fn returned(&mut self, key: &QueryKey, outcome: Outcome) -> QueryPoll<Outcome> {
        self.query_pins
            .entry(Arc::as_ptr(&outcome.value) as usize)
            .or_insert_with(|| QueryPin::new(&outcome.value));
        if outcome.cacheable {
            self.record(DependencyStamp::Query {
                key: key.clone(),
                fingerprint: outcome.fingerprint,
            });
        } else {
            self.uncacheable();
        }
        match outcome.value.as_ref() {
            QueryValue::Error(error) => QueryPoll::Ready(Err(error.clone())),
            QueryValue::ConstantFailure(failure) => {
                QueryPoll::Ready(Err(failure.diagnostic().clone()))
            }
            _ => QueryPoll::Ready(Ok(outcome)),
        }
    }
    fn failed(
        &mut self,
        key: &QueryKey,
        code: &'static str,
        message: impl Into<String>,
    ) -> QueryPoll<Outcome> {
        let value = Arc::new(QueryValue::Error(fault(code, message)));
        let fingerprint = value.fingerprint();
        self.returned(
            key,
            Outcome {
                value,
                fingerprint,
                cacheable: false,
            },
        )
    }
    fn suspended(
        &mut self,
        key: &QueryKey,
        request: InputRequest,
        dependencies: Vec<DependencyStamp>,
    ) -> QueryPoll<Outcome> {
        self.memos.remove(key);
        if let Some(parent) = self.collectors.last_mut() {
            if parent.suspended.is_none() {
                parent.suspended = Some(request.clone());
            }
            parent.non_cacheable = true;
        }
        self.trace.record(
            self.revision,
            key.clone(),
            Action::Suspended,
            "required input is unavailable",
            dependencies,
        );
        QueryPoll::NeedInput(request)
    }
    pub fn fetch(&mut self, provider: &impl Provider, key: QueryKey) -> QueryPoll<Outcome> {
        if let QueryKey::Constant(selected) = &key
            && let Err(error) = self.verify_constant_key(selected)
        {
            self.memos.remove(&key);
            return self.failed(&key, error.code, error.message);
        }
        self.use_counter = match self.use_counter.checked_add(1) {
            Some(n) => n,
            None => return self.failed(&key, "LRR-E-ID-EXHAUSTED", "query use counter exhausted"),
        };
        let counter = self.use_counter;
        if let Some(m) = self
            .memos
            .get_mut(&key)
            .filter(|m| m.verified_revision == self.revision)
        {
            m.last_used_counter = counter;
            let outcome = Outcome {
                value: m.value.clone(),
                fingerprint: m.fingerprint,
                cacheable: true,
            };
            self.trace.record(
                self.revision,
                key.clone(),
                Action::MemoHit,
                "already verified",
                vec![],
            );
            return self.returned(&key, outcome);
        }
        if self.active_queries.contains(&key) || self.active_queries.len() >= MAX_QUERY_DEPTH {
            let code = if self.active_queries.len() >= MAX_QUERY_DEPTH {
                "LRR-E-QUERY-DEPTH"
            } else {
                match key {
                    QueryKey::Constant(_) => "LRR-E-CONST-CYCLE",
                    _ => "LRR-E-QUERY-CYCLE",
                }
            };
            let path = self
                .active_queries
                .iter()
                .chain(std::iter::once(&key))
                .map(|k| {
                    format!(
                        "{}:{}",
                        k.kind(),
                        crate::core::digest::StableEncode::fingerprint(k)
                    )
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            self.trace
                .record(self.revision, key.clone(), Action::Cycle, code, vec![]);
            return self.failed(&key, code, path);
        }
        self.active_queries.push(key.clone());
        if let Some(old) = self.memos.get(&key).cloned() {
            self.collectors.push(Collector::default());
            let mut matches = true;
            for stamp in &old.dependencies {
                let current = match stamp {
                    DependencyStamp::Input { key, .. } => {
                        let _ = self.input(key);
                        self.inputs
                            .get(key)
                            .map_or_else(missing_fingerprint, |r| r.fingerprint)
                    }
                    DependencyStamp::Query { key, .. } => match self.fetch(provider, key.clone()) {
                        QueryPoll::Ready(Ok(out)) => {
                            if !out.cacheable {
                                matches = false;
                            }
                            out.fingerprint
                        }
                        QueryPoll::Ready(Err(_)) => match self
                            .memos
                            .get(key)
                            .filter(|m| m.verified_revision == self.revision)
                        {
                            Some(m) => m.fingerprint,
                            None => {
                                matches = false;
                                Digest::default()
                            }
                        },
                        QueryPoll::NeedInput(_) => {
                            matches = false;
                            break;
                        }
                    },
                };
                let expected = match stamp {
                    DependencyStamp::Input { fingerprint, .. }
                    | DependencyStamp::Query { fingerprint, .. } => *fingerprint,
                };
                if current != expected {
                    matches = false;
                }
            }
            let validation = self.collectors.pop().expect("validation collector");
            if let Some(request) = validation.suspended {
                self.active_queries.pop();
                return self.suspended(&key, request, validation.dependencies);
            }

            if matches && !validation.non_cacheable {
                let m = self.memos.get_mut(&key).expect("retained memo");
                m.verified_revision = self.revision;
                m.last_used_counter = counter;
                self.active_queries.pop();
                self.trace.record(
                    self.revision,
                    key.clone(),
                    Action::ValidatedReuse,
                    "dependencies unchanged",
                    old.dependencies,
                );
                return self.returned(
                    &key,
                    Outcome {
                        value: old.value,
                        fingerprint: old.fingerprint,
                        cacheable: true,
                    },
                );
            }
            self.trace.record(
                self.revision,
                key.clone(),
                Action::Invalidated,
                "dependency changed or non-cacheable",
                old.dependencies,
            );
        }
        self.collectors.push(Collector::default());
        let provided = provider.execute(self, &key);
        let mut collected = self.collectors.pop().expect("provider collector");
        self.active_queries.pop();
        let provided = match provided {
            QueryPoll::NeedInput(request) => {
                self.trace.record(
                    self.revision,
                    key.clone(),
                    Action::ProviderRun,
                    "provider suspended",
                    collected.dependencies.clone(),
                );
                if let Err(error) = request.validate() {
                    self.memos.remove(&key);
                    return self.failed(&key, error.code, error.message);
                }
                return self.suspended(&key, request, collected.dependencies);
            }
            QueryPoll::Ready(result) => {
                if let Some(request) = collected.suspended {
                    self.trace.record(
                        self.revision,
                        key.clone(),
                        Action::ProviderRun,
                        "provider returned with suspended dependency",
                        collected.dependencies.clone(),
                    );
                    return self.suspended(&key, request, collected.dependencies);
                }
                match result {
                    Ok(value) => value,
                    Err(error) => Provided {
                        value: QueryValue::Error(error),
                        cacheable: true,
                    },
                }
            }
        };
        collected.dependencies.sort();
        let inconsistent = collected
            .dependencies
            .windows(2)
            .any(|w| w[0].same_key(&w[1]) && w[0] != w[1]);
        collected.dependencies.dedup();
        let (value, cacheable) = if inconsistent {
            (
                QueryValue::Error(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "dependency changed inside revision",
                )),
                false,
            )
        } else if !provided.value.matches(&key) {
            (
                QueryValue::Error(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "query result variant mismatch",
                )),
                false,
            )
        } else {
            let cacheable = provided.cacheable
                && !collected.non_cacheable
                && !matches!(provided.value, QueryValue::ConstantMacroDemand(_));
            (provided.value, cacheable)
        };
        let value = Arc::new(value);
        let fingerprint = value.fingerprint();
        self.trace.record(
            self.revision,
            key.clone(),
            Action::ProviderRun,
            "provider executed",
            collected.dependencies.clone(),
        );
        if cacheable {
            let estimated_bytes = value
                .estimated_bytes()
                .saturating_add(
                    collected.dependencies.capacity() * std::mem::size_of::<DependencyStamp>(),
                )
                .saturating_add(
                    std::mem::size_of::<MemoEntry>() + std::mem::size_of::<QueryKey>() + 64,
                )
                .saturating_add(key.heap_bytes())
                .saturating_add(
                    collected
                        .dependencies
                        .iter()
                        .map(DependencyStamp::heap_bytes)
                        .sum::<usize>(),
                );
            self.memos.insert(
                key.clone(),
                MemoEntry {
                    value: value.clone(),
                    fingerprint,
                    dependencies: collected.dependencies,
                    verified_revision: self.revision,
                    last_used_counter: counter,
                    estimated_bytes,
                },
            );
        } else {
            self.memos.remove(&key);
        }
        self.returned(
            &key,
            Outcome {
                value,
                fingerprint,
                cacheable,
            },
        )
    }
}
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}

fn account_add(left: usize, right: usize) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "cache accounting overflow"))
}
fn input_metadata_bytes(key: &InputKey, value: &InputValue) -> Result<usize, Diagnostic> {
    let base = 64 + std::mem::size_of::<InputKey>() + std::mem::size_of::<InputRecord>() + 16;
    account_add(
        account_add(base, key.heap_bytes())?,
        value.estimated_bytes()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as lrr;
    include!("../../../../xtask/tests/support/constant_key.rs");
    include!("../../../../xtask/tests/support/constant_value.rs");
    use crate::{
        core::profile::ProfileIdentity, db::provider::Provided, engine::telemetry::CacheBudgets,
    };
    struct ConstantFixture;
    impl Provider for ConstantFixture {
        fn execute(&self, _: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
            Provided::pure(fixture_value(requested, 1))
        }
    }
    fn key(n: u8) -> QueryKey {
        QueryKey::Constant(fixture_key(
            Digest::of_bytes(&[n]),
            vec![],
            Digest::default(),
            ProfileIdentity::current(),
        ))
    }
    #[test]
    fn equal_use_counters_evict_in_stable_key_order() {
        let mut db = Database::default();
        for n in 0..3 {
            drop(db.fetch(&ConstantFixture, key(n)));
        }
        for entry in db.memos.values_mut() {
            entry.last_used_counter = 7;
        }
        let expected: Vec<_> = db.memos.keys().cloned().collect();
        let report = db
            .evict_between_runs(CacheBudgets {
                query_bytes: 0,
                ..CacheBudgets::default()
            })
            .unwrap();
        assert_eq!(report.victims, expected);
        assert_eq!(report.after.memo_bytes, 0);
    }
    #[test]
    fn accounting_overflow_refuses_run_without_eviction() {
        let mut db = Database::default();
        db.trace.detailed = true;
        for n in 0..2 {
            drop(db.fetch(&ConstantFixture, key(n)));
        }
        db.memos.get_mut(&key(0)).unwrap().estimated_bytes = usize::MAX;
        assert_eq!(db.begin_run().unwrap_err().code, "LRR-E-METADATA-LIMIT");
        assert_eq!(db.memos.len(), 2);
        assert!(!db.run_active);
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| event.action != Action::Evicted)
        );
    }
}

#[cfg(test)]
#[path = "prepared_module_registry_snapshot.rs"]
mod prepared_module_registry_snapshot;
#[cfg(test)]
pub(crate) use prepared_module_registry_snapshot::PreparedModuleRegistrySnapshot;

#[cfg(test)]
#[path = "body_structure_accounting_tests.rs"]
mod body_structure_accounting_tests;

#[cfg(test)]
#[path = "source_plan_tests.rs"]
mod source_plan_tests;
