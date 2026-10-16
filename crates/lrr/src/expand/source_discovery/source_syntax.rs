//! Coordinator preparation of actual sealed macro syntax in its caller tree.
//! No expansion, body execution, source publication or name authority occurs here.
use super::*;
use crate::{
    expand::{
        generated_declarations::GeneratedDeclarations,
        source_declarative::assembly::{
            PreparedSourceSyntax, SourceSyntaxAssembly, SyntaxRequestReceipt,
        },
    },
    source::changes::SourceSelection,
};
use std::sync::Arc;
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
        if self.syntax_requests.len() >= 4096 {
            return Err(fault("issued syntax attachment count budget"));
        }
        let bytes = self
            .syntax_request_bytes
            .checked_add(receipt.metadata_bytes()?)
            .and_then(|n| n.checked_add(128))
            .filter(|n| *n <= 32 * 1024 * 1024)
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
        let (_, _, providers) = self
            .declarative
            .as_ref()
            .ok_or_else(|| fault("source syntax preparation requires captured macro providers"))?;
        let mut owners = self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, selected)| selected.selection() == source);
        let (ordinal, _) = owners
            .next()
            .ok_or_else(|| fault("source syntax captured owner absent"))?;
        if owners.next().is_some() {
            return Err(fault("source syntax captured owner ambiguous"));
        }
        let provider = providers
            .get(ordinal)
            .ok_or_else(|| fault("source syntax captured provider absent"))?;
        let index = self
            .nested_sources
            .get(&ordinal)
            .ok_or_else(|| fault("source syntax declaration inventory absent"))?;
        // The issued expansion key and the declaration producer are distinct
        // identities. The admitted output binds the exact edge between them.
        let mut outputs = BTreeMap::<(Digest, Digest), Arc<GeneratedDeclarations>>::new();
        let mut collect = |output: &Arc<GeneratedDeclarations>| -> Result<(), Diagnostic> {
            let key = output.record().key();
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
            if let Some(prior) = outputs.get(&parent) {
                if prior.fingerprint() != output.fingerprint() {
                    return Err(fault(
                        "source syntax producer has conflicting admitted outputs",
                    ));
                }
                return Ok(());
            }
            if outputs.len() >= 100_000 {
                return Err(fault("source syntax output handle count budget"));
            }
            outputs
                .len()
                .checked_add(1)
                .and_then(|count| {
                    count.checked_mul(
                        128 + std::mem::size_of::<((Digest, Digest), Arc<GeneratedDeclarations>)>(),
                    )
                })
                .filter(|bytes| *bytes <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("source syntax output index metadata budget"))?;
            outputs.insert(parent, output.clone());
            Ok(())
        };
        for output in self
            .item_declaration_inputs
            .values()
            .filter(|output| output.parent_inventory() == index.scan().fingerprint())
        {
            collect(output)?;
        }
        for fragment in self
            .nested_completed
            .values()
            .chain(&self.completed_argument_fragments)
        {
            if fragment.source_scan().fingerprint() != index.scan().fingerprint() {
                continue;
            }
            for output in fragment.syntax_outputs()? {
                collect(&output)?;
            }
        }
        let mut assembly = SourceSyntaxAssembly::new(provider, index, self.epoch)?;
        loop {
            let pending = match definition {
                Some(definition) => assembly.next_body_invocation(definition)?,
                None => assembly.next_pending_invocation()?,
            };
            let Some((parent, invocation)) = pending else {
                break;
            };
            let output = match outputs.remove(&(parent, invocation)) {
                Some(output) => output,
                None => {
                    return Err(fault(&format!(
                        "source syntax call requires an actual expansion output: producer={invocation}, parent={parent}, remaining_outputs={}, inventory={}",
                        outputs.len(),
                        index.scan().fingerprint(),
                    )));
                }
            };
            let receipt = self
                .syntax_requests
                .get(&output.record().key().fingerprint())
                .ok_or_else(|| {
                    fault("source syntax output requires its actual issued request receipt")
                })?;
            assembly = assembly.attach_receipt(receipt, output)?;
        }
        if definition.is_none() && !outputs.is_empty() {
            return Err(fault(
                "source syntax output requires further structural reconciliation",
            ));
        }
        let prepared = match definition {
            Some(definition) => assembly.finish_for_body(definition)?,
            None => assembly.finish()?,
        };
        if !prepared.ready_for_admission() {
            return Err(fault("source syntax preparation left pending calls"));
        }
        Ok(Arc::new(prepared))
    }
}
