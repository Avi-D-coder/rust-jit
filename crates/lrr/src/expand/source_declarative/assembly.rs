//! Checked request/record attachment into the ordinary retained source tree.
//! Preparation only: no expansion executes and no source or seal is published.
use super::{SourceDeclarative, SourceDeclarativeRequest, adapter};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        generated_declarations::{GeneratedDeclarations, declaration_grammar},
        identity::GrammarSlot,
    },
    frontend::impl_index::DeclarationMacroIndex,
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{
        declaration_scan::{DeclarationProducerKind, DeclarationScan},
        spans::SourceRange,
    },
};
use lrr_expand_adapter::{
    declaration_output::{self, CallCapture, DeclarationGrammar},
    syntax_attachment::{FragmentSlot, ListSlot, SyntaxAttachment, SyntaxSnapshot},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[path = "syntax_receipt.rs"]
mod receipt;
pub(crate) use receipt::SyntaxRequestReceipt;

#[path = "assembly_anchors.rs"]
mod anchors;
pub use anchors::OriginalDeclarationAnchor;
#[path = "body_selection.rs"]
mod body_selection;

const MAX_CALLS: usize = 100_000;
const MAX_METADATA: usize = 128 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn charge(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .filter(|n| *n <= MAX_METADATA)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "source syntax assembly budget".into(),
            )
        })
}
fn anchor(call: &CallCapture) -> Result<Digest, Diagnostic> {
    let mut e = StableEncoder::new("lrr/v1/syntax-call-attachment");
    e.digest(Digest(call.name_span.0));
    e.digest(Digest(call.site.0));
    e.u16(call.edition);
    e.digest(Digest::of_bytes(&call.path.encode().map_err(adapter)?));
    e.digest(Digest::of_bytes(&call.input.encode().map_err(adapter)?));
    Ok(e.finish())
}
struct CallBinding {
    // Positions in this vector follow actual AST preorder. Origin keys alone
    // cannot distinguish repeated copies of an invocation in one expansion.
    invocation: Option<Digest>,
    // Ordinary item discovery and declaration scans retain distinct issued
    // request IDs for the same exact original syntax occurrence.
    original_item_invocation: Option<Digest>,
    grammar: Option<GrammarSlot>,
    parent: Arc<DeclarationScan>,
    anchor: Digest,
}

pub struct SourceSyntaxAssembly {
    source: PreparedSourceView,
    epoch: u64,
    tree: SyntaxAttachment,
    calls: Vec<CallBinding>,
    outputs: Vec<Arc<GeneratedDeclarations>>,
    originals: Vec<OriginalDeclarationAnchor>,
    retained_bytes: usize,
}
/// A checked immutable preparation result. Even an empty pending-call set is
/// not namespace completeness, a current source lease, or VM entry authority.
pub struct PreparedSourceSyntax {
    source: PreparedSourceView,
    epoch: u64,
    syntax: Arc<SyntaxSnapshot>,
    outputs: Vec<Arc<GeneratedDeclarations>>,
    originals: Vec<OriginalDeclarationAnchor>,
    pending: Vec<Digest>,
    // Only finish_for_body may mint a body-scoped readiness certificate.
    selected_body: Option<Digest>,
    identity: Digest,
}
impl PreparedSourceSyntax {
    pub fn source(&self) -> &SourceSelection {
        self.source.selection()
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn syntax(&self) -> &Arc<SyntaxSnapshot> {
        &self.syntax
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    /// Every remaining structural call, including calls without a certified
    /// declaration producer (represented by their structural attachment anchor).
    /// These are pending markers, never authority to issue an expansion.
    pub fn pending_calls(&self) -> &[Digest] {
        &self.pending
    }
    pub(crate) fn ready_for_admission(&self) -> bool {
        self.selected_body.is_some() || self.pending.is_empty()
    }
    pub(crate) fn permits_body(&self, definition: Digest) -> bool {
        self.selected_body.map_or_else(
            || self.pending.is_empty(),
            |selected| selected == definition,
        )
    }
    pub fn outputs(&self) -> &[Arc<GeneratedDeclarations>] {
        &self.outputs
    }
    /// Original declaration occurrences tracked through exact tree edits. These
    /// identify source anchors, not a published generated-member namespace.
    pub fn original_declarations(&self) -> &[OriginalDeclarationAnchor] {
        &self.originals
    }
    pub(crate) fn metadata_excluding_snapshot(&self) -> Result<usize, Diagnostic> {
        let mut bytes = charge(
            std::mem::size_of::<Self>(),
            self.source.selection().metadata_bytes()?,
        )?;
        for (capacity, size) in [
            (
                self.outputs.capacity(),
                std::mem::size_of::<Arc<GeneratedDeclarations>>(),
            ),
            (self.pending.capacity(), std::mem::size_of::<Digest>()),
        ] {
            bytes = charge(
                bytes,
                capacity
                    .checked_mul(size)
                    .ok_or_else(|| fault("retained syntax metadata overflow"))?,
            )?;
        }
        for output in &self.outputs {
            bytes = charge(bytes, output.metadata_bytes()?)?;
        }
        bytes = charge(bytes, anchors::metadata(&self.originals)?)?;
        Ok(bytes)
    }
}

impl SourceSyntaxAssembly {
    pub fn new(
        provider: &SourceDeclarative,
        index: &DeclarationMacroIndex,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        let source = &provider.source;
        let selection = source.selection();
        let scan = index.scan_handle();
        if epoch == 0
            || scan.parse() != &selection.outline.parse
            || scan.path() != &selection.path
            || scan.cfg() != selection.outline.cfg
            || scan.scope() != &source.outline().root_scope
        {
            return Err(fault("source syntax assembly caller/scan/epoch differs"));
        }
        let tree = SyntaxAttachment::parse(&provider.owned_forest()?, DeclarationGrammar::Items)
            .map_err(adapter)?;
        let captured = tree.materialize().map_err(adapter)?;
        let producers = scan
            .producers()
            .iter()
            .filter(|p| p.kind == DeclarationProducerKind::Macro)
            .map(|p| ((p.range.start, p.range.end), p))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut calls = Vec::new();
        reserve_calls(&mut calls, captured.calls.len())?;
        for call in &captured.calls {
            let site = call
                .input
                .spans()
                .iter()
                .find(|s| s.key() == call.site)
                .ok_or_else(|| fault("source syntax call site absent"))?;
            if site.source.unit.0 != selection.outline.crate_key.0
                || site.source.content.0 != selection.outline.parse.content.0
                || site.source.path != selection.path.as_str()
            {
                return Err(fault("source syntax call belongs to another source"));
            }
            let original = source
                .macro_call_for_opening_at(SourceRange::checked(site.start, site.end)?)?
                .ok_or_else(|| fault("source syntax call has no original invocation"))?;
            let range = original.range();
            let producer = producers.get(&(range.start, range.end));
            if let Some(p) = producer
                && !seen.insert(p.identity)
            {
                return Err(fault("source syntax invocation duplicated"));
            }
            calls.push(CallBinding {
                invocation: producer.map(|p| p.identity),
                original_item_invocation: source
                    .outline()
                    .item_invocations
                    .iter()
                    .find(|item| item.range == range && item.attribute_input.is_none())
                    .map(|item| item.item_input),
                grammar: producer.map(|p| p.grammar),
                parent: scan.clone(),
                anchor: anchor(call)?,
            });
        }
        if seen.len() != producers.len() {
            return Err(fault("source syntax invocation capture incomplete"));
        }
        let originals = anchors::capture(source, &tree)?;
        let retained_bytes = charge(
            charge(scan.retained_bytes(), source.prepared_metadata_bytes()?)?,
            anchors::metadata(&originals)?,
        )?;
        charge_calls(retained_bytes, calls.len())?;
        Ok(Self {
            source: source.share(),
            epoch,
            tree,
            calls,
            outputs: vec![],
            originals,
            retained_bytes,
        })
    }

    /// Consume the builder so any failure discards the entire candidate. No
    /// partially attached tree or publication can escape a failed transaction.
    pub fn attach(
        self,
        request: &SourceDeclarativeRequest,
        output: Arc<GeneratedDeclarations>,
    ) -> Result<Self, Diagnostic> {
        let receipt = SyntaxRequestReceipt::capture(request)?;
        self.attach_receipt(&receipt, output)
    }

    pub(crate) fn next_pending_invocation(
        &self,
    ) -> Result<Option<(Digest, Option<Digest>)>, Diagnostic> {
        self.calls
            .first()
            .map(|call| {
                call.invocation
                    .map(|id| (id, call.original_item_invocation))
                    .ok_or_else(|| {
                        fault("retained source call lacks an actual declaration producer")
                    })
            })
            .transpose()
    }

    /// Coordinator replay consumes the receipt of the genuine issued request,
    /// never a caller-supplied key, generated text or runtime macro recipe.
    pub(crate) fn attach_receipt(
        mut self,
        receipt: &SyntaxRequestReceipt,
        output: Arc<GeneratedDeclarations>,
    ) -> Result<Self, Diagnostic> {
        let key = receipt.key();
        if output.record().key() != key
            || output.epoch() != self.epoch
            || key
                .procedural_epoch
                .is_some_and(|epoch| epoch != self.epoch)
        {
            return Err(fault("source syntax issued request/record/epoch differs"));
        }
        let position = self
            .calls
            .iter()
            .position(|c| c.matches_invocation(key.invocation))
            .ok_or_else(|| fault("source syntax issued invocation is not pending"))?;
        if self
            .calls
            .iter()
            .skip(position + 1)
            .any(|c| c.matches_invocation(key.invocation))
        {
            return Err(fault("source syntax invocation identity is ambiguous"));
        }
        let selected = &self.calls[position];
        if selected.invocation != Some(output.parent_invocation())
            || output.parent_inventory() != selected.parent.fingerprint()
            || selected.grammar != Some(key.grammar)
        {
            return Err(fault("source syntax output parent/grammar differs"));
        }
        let before = self.tree.materialize().map_err(adapter)?;
        verify_calls(&self.calls, &before.calls)?;
        let call = &before.calls[position];
        if call.site != receipt.site || call.edition != key.call_edition {
            return Err(fault("source syntax issued call tokens/site differ"));
        }
        receipt.require_call_roots(&call.input)?;
        let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(output.record().wire())
            .map_err(adapter)?;
        let generated = declaration_output::materialize(&tokens, declaration_grammar(key.grammar))
            .map_err(adapter)?;
        let replacement = generated_calls(&output, &generated.calls)?;
        let count = self
            .calls
            .len()
            .checked_sub(1)
            .and_then(|n| n.checked_add(replacement.len()))
            .ok_or_else(|| fault("source syntax call count overflow"))?;
        if self.outputs.len() >= MAX_CALLS {
            return Err(fault("source syntax output count limit"));
        }
        let retained_bytes = charge(
            charge(self.retained_bytes, output.metadata_bytes()?)?,
            std::mem::size_of::<Arc<GeneratedDeclarations>>(),
        )?;
        charge_calls(retained_bytes, count)?;
        let mut calls = Vec::new();
        reserve_calls(&mut calls, count)?;
        self.outputs
            .try_reserve(1)
            .map_err(|_| fault("source syntax output retention allocation"))?;
        let edit = match key.grammar {
            GrammarSlot::Expression => {
                self.tree
                    .attach_with_edit(call.start, call.end, &tokens, FragmentSlot::Expression)
            }
            GrammarSlot::Pattern => {
                self.tree
                    .attach_with_edit(call.start, call.end, &tokens, FragmentSlot::Pattern)
            }
            GrammarSlot::Type => {
                self.tree
                    .attach_with_edit(call.start, call.end, &tokens, FragmentSlot::Type)
            }
            GrammarSlot::Statements => {
                self.tree
                    .attach_list_with_edit(call.start, call.end, &tokens, ListSlot::Statements)
            }
            GrammarSlot::Items => {
                self.tree
                    .attach_list_with_edit(call.start, call.end, &tokens, ListSlot::Items)
            }
            GrammarSlot::TraitItems => {
                self.tree
                    .attach_list_with_edit(call.start, call.end, &tokens, ListSlot::TraitItems)
            }
            GrammarSlot::ImplItems => {
                self.tree
                    .attach_list_with_edit(call.start, call.end, &tokens, ListSlot::ImplItems)
            }
        }
        .map_err(adapter)?;
        self.originals = anchors::translate(self.originals, edit)?;
        let mut prior = self.calls.into_iter();
        calls.extend(prior.by_ref().take(position));
        prior
            .next()
            .ok_or_else(|| fault("source syntax pending call disappeared"))?;
        calls.extend(replacement);
        calls.extend(prior);
        verify_calls(&calls, &self.tree.materialize().map_err(adapter)?.calls)?;
        self.calls = calls;
        self.retained_bytes = retained_bytes;
        self.outputs.push(output);
        Ok(self)
    }

    pub fn finish(self) -> Result<PreparedSourceSyntax, Diagnostic> {
        self.finish_scope(None)
    }
    fn finish_scope(
        mut self,
        selected_body: Option<Digest>,
    ) -> Result<PreparedSourceSyntax, Diagnostic> {
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(self.calls.len())
            .map_err(|_| fault("source syntax pending allocation"))?;
        // An unowned structural call must not vanish from the completeness
        // result. Its anchor records pending work; it grants no producer ID.
        pending.extend(self.calls.iter().map(|c| c.invocation.unwrap_or(c.anchor)));
        let syntax = Arc::new(self.tree.snapshot().map_err(adapter)?);
        anchors::validate(&self.source, syntax.clone(), &self.originals)?;
        self.outputs
            .sort_by_key(|output| output.record().key().fingerprint());
        let mut identity = StableEncoder::new(if selected_body.is_some() {
            "lrr/v3/prepared-source-syntax/selected-original-body"
        } else {
            "lrr/v2/prepared-source-syntax/original-anchors"
        });
        if let Some(definition) = selected_body {
            identity.digest(definition);
        }
        identity.string(self.source.selection().path.as_str());
        identity.record(&self.source.selection().outline);
        identity.u64(self.epoch);
        identity.digest(Digest(syntax.identity().0));
        identity.u64(self.outputs.len() as u64); // Bounded by the capture/output ledger.
        for output in &self.outputs {
            identity.record(&output.record().descriptor());
        }
        identity.u64(pending.len() as u64);
        for invocation in &pending {
            identity.digest(*invocation);
        }
        identity.u64(self.originals.len() as u64);
        for anchor in &self.originals {
            anchor.encode(&mut identity);
        }
        Ok(PreparedSourceSyntax {
            source: self.source,
            epoch: self.epoch,
            syntax,
            outputs: self.outputs,
            originals: self.originals,
            pending,
            selected_body,
            identity: identity.finish(),
        })
    }
}
impl CallBinding {
    fn matches_invocation(&self, invocation: Digest) -> bool {
        self.invocation == Some(invocation) || self.original_item_invocation == Some(invocation)
    }
}
fn reserve_calls(calls: &mut Vec<CallBinding>, count: usize) -> Result<(), Diagnostic> {
    charge_calls(0, count)?;
    calls
        .try_reserve_exact(count)
        .map_err(|_| fault("source syntax call allocation"))
}
fn charge_calls(bytes: usize, count: usize) -> Result<usize, Diagnostic> {
    if count > MAX_CALLS {
        return Err(fault("source syntax call count limit"));
    }
    charge(
        bytes,
        count
            .checked_mul(std::mem::size_of::<CallBinding>())
            .ok_or_else(|| fault("source syntax call metadata overflow"))?,
    )
}
fn verify_calls(bindings: &[CallBinding], captures: &[CallCapture]) -> Result<(), Diagnostic> {
    if bindings.len() != captures.len() {
        return Err(fault("source syntax call topology differs"));
    }
    for (binding, capture) in bindings.iter().zip(captures) {
        if binding.anchor != anchor(capture)? {
            return Err(fault("source syntax call occurrence differs"));
        }
    }
    Ok(())
}
fn generated_calls(
    output: &Arc<GeneratedDeclarations>,
    calls: &[CallCapture],
) -> Result<Vec<CallBinding>, Diagnostic> {
    let scan = output.scan();
    let producers = scan
        .producers()
        .iter()
        .filter(|p| p.kind == DeclarationProducerKind::Macro)
        .map(|p| ((p.range.start, p.range.end), p))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    reserve_calls(&mut result, calls.len())?;
    for call in calls {
        let producer = producers.get(&(call.start, call.end));
        if let Some(p) = producer
            && (!seen.insert(p.identity)
                || anchor(output.macro_call(p.identity)?)? != anchor(call)?)
        {
            return Err(fault("generated syntax invocation owner differs"));
        }
        result.push(CallBinding {
            invocation: producer.map(|p| p.identity),
            original_item_invocation: None,
            grammar: producer.map(|p| p.grammar),
            parent: scan.clone(),
            anchor: anchor(call)?,
        });
    }
    if seen.len() != producers.len() {
        return Err(fault("generated syntax invocation capture incomplete"));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "assembly_tests.rs"]
pub(crate) mod tests;
