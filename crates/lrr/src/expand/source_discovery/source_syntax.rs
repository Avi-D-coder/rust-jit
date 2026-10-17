//! Coordinator preparation of actual sealed macro syntax in its caller tree.
//! No expansion, body execution, source publication or name authority occurs here.
use super::*;
use crate::{
    expand::{
        generated_declarations::GeneratedDeclarations,
        source_declarative::assembly::{PreparedSourceSyntax, SyntaxRequestReceipt},
    },
    source::changes::SourceSelection,
};
use std::sync::Arc;
#[path = "completed_source_syntax.rs"]
mod completed;
pub(crate) use completed::CompletedSourceSyntax;

impl SourceDiscovery {
    pub(super) fn remember_syntax_request(
        &mut self,
        request: &super::super::source_declarative::SourceDeclarativeRequest,
    ) -> Result<(), Diagnostic> {
        let receipt = SyntaxRequestReceipt::capture(request)?;
        self.remember_syntax_receipt(receipt)
    }
    pub(super) fn remember_generated_syntax_request(
        &mut self,
        request: &super::super::generated_call_request::GeneratedCallRequest,
    ) -> Result<(), Diagnostic> {
        let receipt = SyntaxRequestReceipt::capture_generated(request)?;
        self.remember_syntax_receipt(receipt)
    }
    fn remember_syntax_receipt(&mut self, receipt: SyntaxRequestReceipt) -> Result<(), Diagnostic> {
        let key = receipt.key().fingerprint();
        if let Some(actual) = self.syntax_requests.get(&key) {
            if actual != &receipt {
                return Err(fault("issued syntax attachment receipt changed"));
            }
            return Ok(());
        }
        let bytes = self
            .syntax_request_bytes
            .checked_add(receipt.metadata_bytes()?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| fault("issued syntax attachment metadata budget"))?;
        self.syntax_requests.insert(key, receipt);
        self.syntax_request_bytes = bytes;
        Ok(())
    }

    /// Replay only already admitted outputs bound by this actual completed
    /// discovery. Original/generated calls consume their actual issued receipts
    /// in AST occurrence order. Missing structural reconciliation is an error,
    /// never a fallback to macro-specific runtime semantics.
    pub fn prepare_source_syntax(
        &self,
        source: &SourceSelection,
    ) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        self.prepare_syntax(source, None)
    }

    /// Prepare one exact original function before entry. Calls owned by cold
    /// functions remain structurally pending and cannot authorize their entry.
    pub fn prepare_body_syntax(
        &self,
        source: &SourceSelection,
        definition: Digest,
    ) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        self.prepare_syntax(source, Some(definition))
    }

    fn prepare_syntax(
        &self,
        source: &SourceSelection,
        definition: Option<Digest>,
    ) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let seal = self
            .seal
            .as_ref()
            .ok_or_else(|| fault("source syntax preparation requires completed discovery"))?;
        if seal.unresolved_frontiers() != 0 || seal.key() != self.key {
            return Err(fault("source syntax preparation discovery context differs"));
        }
        let environment = self
            .completed_source_environment
            .as_ref()
            .ok_or_else(|| fault("completed source syntax environment absent"))?;
        let plan = environment.source_syntax(source)?;
        if plan.seal() != &self.key
            || Some(plan.seal_value()) != self.body_boundary_seal_fingerprint
        {
            return Err(fault("source syntax retained seal differs"));
        }
        match definition {
            Some(definition) => plan.prepare_body(definition),
            None => plan.prepare_source(),
        }
    }
    /// Capture actual immutable assembly source/issued reply inputs once. No
    /// executable function body plan is prepared by discovery completion.
    pub(super) fn prepare_completed_source_syntax(
        &self,
        seal: &crate::db::seals::Seal,
    ) -> Result<BTreeMap<Digest, Arc<CompletedSourceSyntax>>, Diagnostic> {
        use crate::expand::source_declarative::assembly::LazySourceAssemblySeed;
        let Some((_, _, providers)) = &self.declarative else {
            return Ok(BTreeMap::new());
        };
        if seal.key() != self.key || seal.unresolved_frontiers() != 0 {
            return Err(fault("completed syntax inputs seal differs"));
        }
        let seal_value = self
            .body_boundary_seal_fingerprint
            .ok_or_else(|| fault("completed syntax seal identity absent"))?;
        let mut by_source =
            BTreeMap::<Digest, BTreeMap<(Digest, Digest), completed::SyntaxReply>>::new();
        let mut receipts = BTreeMap::<Digest, Arc<SyntaxRequestReceipt>>::new();
        let mut count = 0usize;
        let mut bytes = 0usize;
        let mut collect = |source_scan: Digest,
                           output: &Arc<GeneratedDeclarations>|
         -> Result<(), Diagnostic> {
            let key = output.record().key();
            // A derive output is appended after its annotated item, never
            // attached at a function-like call site; several derives of one
            // item share that item's producer. Its record stays in the seal.
            if key.macro_identity.is_derive() {
                return Ok(());
            }
            if output.epoch() != self.epoch
                || seal
                    .input_dependencies()
                    .binary_search(&(
                        key.fingerprint(),
                        output.record().descriptor().fingerprint(),
                    ))
                    .is_err()
            {
                return Err(fault(
                    "source syntax output lacks its actual sealed record dependency",
                ));
            }
            let parent = (output.parent_inventory(), output.parent_invocation());
            if let Some(prior) = by_source
                .get(&source_scan)
                .and_then(|rows| rows.get(&parent))
            {
                if prior.output.fingerprint() != output.fingerprint() {
                    return Err(fault(
                        "source syntax producer has conflicting admitted outputs",
                    ));
                }
                return Ok(());
            }
            count = count
                .checked_add(1)
                .ok_or_else(crate::runtime::arena::resource)?;
            bytes = bytes
                .checked_add(
                    512 + 16 * std::mem::size_of::<((Digest, Digest), completed::SyntaxReply)>(),
                )
                .ok_or_else(crate::runtime::arena::resource)?;
            let receipt_key = key.fingerprint();
            let receipt = match receipts.get(&receipt_key) {
                Some(receipt) => Some(Arc::clone(receipt)),
                None => match self.syntax_requests.get(&receipt_key) {
                    None => None, // Explicit missing attachment evidence, refused only if demanded.
                    Some(actual) => {
                        bytes = bytes
                            .checked_add(actual.metadata_bytes()?)
                            .and_then(|n| n.checked_add(256))
                            .ok_or_else(crate::runtime::arena::resource)?;
                        let receipt = Arc::new(actual.clone());
                        receipts.insert(receipt_key, Arc::clone(&receipt));
                        Some(receipt)
                    }
                },
            };
            if receipt.as_ref().is_some_and(|receipt| receipt.key() != key) {
                return Err(fault("source syntax issued key differs"));
            }
            by_source.entry(source_scan).or_default().insert(
                parent,
                completed::SyntaxReply {
                    output: Arc::clone(output),
                    receipt,
                },
            );
            Ok(())
        };
        for output in self.item_declaration_inputs.values() {
            collect(output.parent_inventory(), output)?;
        }
        for fragment in self
            .nested_completed
            .values()
            .chain(&self.completed_argument_fragments)
        {
            let source_scan = fragment.source_scan().fingerprint();
            for output in fragment.syntax_outputs()? {
                collect(source_scan, &output)?;
            }
        }
        drop(collect);
        drop(receipts);
        let mut plans = BTreeMap::new();
        let mut retained = 0usize;
        for (ordinal, provider) in providers
            .iter()
            .enumerate()
            .filter(|(_, provider)| provider.macro_export_source_role().is_none())
        {
            let index = self
                .nested_sources
                .get(&ordinal)
                .ok_or_else(|| fault("source syntax declaration inventory absent"))?;
            let seed = LazySourceAssemblySeed::new(provider, index.scan_handle(), self.epoch);
            let replies = by_source
                .remove(&index.scan().fingerprint())
                .unwrap_or_default();
            let plan =
                CompletedSourceSyntax::capture_lazy(seed, replies, self.key.clone(), seal_value)?;
            insert_plan(&mut plans, &mut retained, plan)?;
        }
        for module in self.staged_modules.values().filter(|module| {
            module.namespace_complete && module.provider.macro_export_source_role().is_none()
        }) {
            let scan = match &module.annotation_plan {
                Some(plan) => Arc::clone(plan.scan()),
                None => module
                    .syntax
                    .prepared_source()
                    .declaration_inventory(Default::default())?,
            };
            let seed = LazySourceAssemblySeed::new(&module.provider, &scan, self.epoch);
            let replies = by_source.remove(&scan.fingerprint()).unwrap_or_default();
            let plan =
                CompletedSourceSyntax::capture_lazy(seed, replies, self.key.clone(), seal_value)?;
            insert_plan(&mut plans, &mut retained, plan)?;
        }
        Ok(plans)
    }
}
fn insert_plan(
    plans: &mut BTreeMap<Digest, Arc<CompletedSourceSyntax>>,
    retained: &mut usize,
    plan: Arc<CompletedSourceSyntax>,
) -> Result<(), Diagnostic> {
    *retained = retained
        .checked_add(plan.metadata_bytes())
        .and_then(|n| {
            n.checked_add(256 + 16 * std::mem::size_of::<(Digest, Arc<CompletedSourceSyntax>)>())
        })
        .ok_or_else(crate::runtime::arena::resource)?;
    let key = crate::db::key::QueryKey::Outline(plan.source().outline.clone()).fingerprint();
    if plans.insert(key, plan).is_some() {
        return Err(fault("completed source syntax owner duplicated"));
    }
    Ok(())
}
