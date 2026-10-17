//! Associated output facts through a real issued producer chain and original
//! canonical owner. No namespace closure, member selection or call authority.
use super::*;
use crate::{
    expand::{
        generated_declarations::GeneratedDeclarations,
        generated_items::GeneratedHeaderKind,
        identity::GeneratedItemKey,
        source_declarative::{SourceDeclarativeRequest, assembly::SyntaxRequestReceipt},
    },
    syntax::{
        declaration_scan::DeclarationNameContext,
        outline::{BodyHandle, Outline},
    },
};

/// These are actual private-constructed issued requests and admitted outputs,
/// never a caller-supplied digest/grammar assertion.
pub(crate) struct IssuedAssociatedOutput<'a> {
    pub(crate) request: &'a SourceDeclarativeRequest,
    pub(crate) output: Arc<GeneratedDeclarations>,
}
struct GeneratedSlot {
    source: u32,
    header: u32,
    declaration: Option<u32>,
    body: Option<BodyHandle>,
}
pub(crate) struct PreparedGeneratedAssociatedDeclarations<'a> {
    source: &'a PreparedSourceView,
    owner: &'a ItemOutline,
    owner_identity: Digest,
    outputs: Vec<Arc<GeneratedDeclarations>>,
    receipts: Vec<SyntaxRequestReceipt>,
    outline: Outline,
    slots: Vec<GeneratedSlot>,
    fingerprint: Digest,
    metadata_bytes: usize,
}
pub(crate) struct GeneratedAssociatedHeader<'a> {
    pub(crate) source: &'a AssociatedDeclarationHeader,
    /// Existing generated-item identity from the real expansion/header/path.
    pub(crate) key: &'a GeneratedItemKey,
    pub(crate) owner: &'a ItemOutline,
    /// Auxiliary actual Outline DefKey/header metadata. Generated identity is
    /// `key`; fragment-local duplicate ordinals alone are not globally unique.
    pub(crate) declaration: Option<&'a ItemOutline>,
    pub(crate) body: Option<&'a BodyHandle>,
}
fn generated_fault(message: &str) -> Diagnostic {
    fault(message)
}
impl<'a> PreparedGeneratedAssociatedDeclarations<'a> {
    pub(crate) fn prepare(
        original: &'a PreparedOriginalAssociatedDeclarations<'_>,
        invocation: Digest,
        chain: &[IssuedAssociatedOutput<'_>],
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        if epoch == 0 || chain.is_empty() {
            return Err(generated_fault(
                "associated issued chain count/epoch differs",
            ));
        }
        let root_slot = original
            .associated_producer(invocation)
            .ok_or_else(|| generated_fault("associated original producer slot absent"))?;
        let OriginalAssociatedTarget::Producer(root_producer) = root_slot.target else {
            return Err(generated_fault(
                "associated root is not an issued producer slot",
            ));
        };
        if root_producer.identity != invocation {
            return Err(generated_fault(
                "associated original producer identity differs",
            ));
        }
        let owner = root_slot
            .owner
            .canonical
            .ok_or_else(|| generated_fault("associated enclosing canonical owner is unfinished"))?;
        let owner_identity = root_slot
            .owner
            .member_owner
            .ok_or_else(|| generated_fault("associated enclosing child owner absent"))?;
        let source = original.source();
        let cfg = source.captured_macro_cfg()?;
        let grammar = root_producer.grammar;
        if !matches!(grammar, GrammarSlot::TraitItems | GrammarSlot::ImplItems) {
            return Err(generated_fault("associated root grammar differs"));
        }
        let mut budget = std::mem::size_of::<Self>();
        let mut outputs = reserve::<Arc<GeneratedDeclarations>>(chain.len(), &mut budget)?;
        let mut receipts = reserve::<SyntaxRequestReceipt>(chain.len(), &mut budget)?;
        let mut receipt_heap = 0usize;
        let mut parent = original.scan().as_ref();
        for (ordinal, issued) in chain.iter().enumerate() {
            let receipt = SyntaxRequestReceipt::capture(issued.request)?;
            let heap = receipt
                .metadata_bytes()?
                .checked_sub(std::mem::size_of::<SyntaxRequestReceipt>())
                .ok_or_else(resource)?;
            budget = checked_add(budget, heap)?;
            receipt_heap = checked_add(receipt_heap, heap)?;
            let output = issued.output.as_ref();
            let key = receipt.key();
            let producer = parent
                .producers()
                .iter()
                .find(|row| row.identity == key.invocation)
                .ok_or_else(|| generated_fault("associated issued parent producer absent"))?;
            if (ordinal == 0 && producer.identity != invocation)
                || producer.kind != DeclarationProducerKind::Macro
                || producer.grammar != grammar
                || output.parent_inventory() != parent.fingerprint()
                || output.parent_invocation() != producer.identity
                || output.record().key() != key
                || key.grammar != grammar
                || output.epoch() != epoch
                || output.scan().cfg() != cfg.fingerprint()
                || output.scan().path() != &source.selection().path
                || output.scan().scope() != &producer.scope
                || output.admitted_parsed().key() != *output.scan().parse()
                || output.admitted_parsed().generated_origin != Some(output.fingerprint())
            {
                return Err(generated_fault(
                    "associated issued output source/record/parent/epoch differs",
                ));
            }
            if ordinal > 0 {
                let mut slots = parent
                    .associated_fragments()
                    .iter()
                    .filter(|row| row.producer == Some(producer.identity));
                let row = slots.next().ok_or_else(|| {
                    generated_fault(
                        "associated child producer is not a direct parent fragment slot",
                    )
                })?;
                if slots.next().is_some()
                    || row.kind != AssociatedDeclarationKind::MacroProducer
                    || row.range != producer.range
                    || row.lexical_scope != producer.lexical_scope
                    || !matches!(row.owner,AssociatedDeclarationOwner::Fragment{grammar:g,..} if g==grammar)
                {
                    return Err(generated_fault(
                        "associated child direct fragment attachment differs",
                    ));
                }
            }
            // The owned output is already bounded/accounted by its admission.
            // Only shared handles are new here; callers must retain those roots.
            outputs.push(Arc::clone(&issued.output));
            receipts.push(receipt);
            parent = output.scan().as_ref();
        }
        let output = outputs.last().expect("nonempty checked associated chain");
        let rows = output.scan().associated_fragments();
        // Ordinary Outline duplicates the genuine parent header context in
        // direct members; bound this retained metadata before asking it to walk.
        let context = owner
            .header
            .estimated_bytes()
            .checked_add(owner.scope.heap_bytes())
            .ok_or_else(resource)?;
        let preflight = context
            .checked_mul(rows.len().checked_add(1).ok_or_else(resource)?)
            .and_then(|n| n.checked_add(output.scan().retained_bytes().checked_mul(4)?))
            .ok_or_else(resource)?;
        checked_add(budget, preflight)?;
        let outline = output
            .admitted_parsed()
            .outline_associated_fragment(owner, cfg)?;
        budget = checked_add(budget, outline.estimated_bytes())?;
        let mut headers = reserve::<((u32, u32), u32)>(output.headers().len(), &mut budget)?;
        for (i, row) in output.headers().iter().enumerate() {
            headers.push((
                (row.start, row.end),
                u32::try_from(i).map_err(|_| resource())?,
            ));
        }
        headers.sort_unstable_by_key(|row| row.0);
        let mut declarations = reserve::<((u32, u32), u32)>(outline.items.len(), &mut budget)?;
        for (i, item) in outline.items.iter().enumerate() {
            declarations.push((
                key(item.declaration_span),
                u32::try_from(i).map_err(|_| resource())?,
            ));
        }
        declarations.sort_unstable_by_key(|row| row.0);
        let mut spans = reserve::<(Digest, u32)>(output.tokens().spans().len(), &mut budget)?;
        for (i, span) in output.tokens().spans().iter().enumerate() {
            spans.push((
                Digest(span.key().0),
                u32::try_from(i).map_err(|_| resource())?,
            ));
        }
        spans.sort_unstable_by_key(|row| row.0);
        if spans.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(generated_fault(
                "associated actual span identity duplicated",
            ));
        }
        let mut slots = reserve::<GeneratedSlot>(rows.len(), &mut budget)?;
        for (ordinal, row) in rows.iter().enumerate() {
            if !matches!(row.owner,AssociatedDeclarationOwner::Fragment{grammar:g,..} if g==grammar)
            {
                return Err(generated_fault("associated output fragment owner differs"));
            }
            let header_index = at_span(&headers, row.range)?
                .ok_or_else(|| generated_fault("associated actual pinned header absent"))?;
            let header = &output.headers()[header_index as usize];
            let expected = match row.kind {
                AssociatedDeclarationKind::Function => GeneratedHeaderKind::Function,
                AssociatedDeclarationKind::Const => GeneratedHeaderKind::Const,
                AssociatedDeclarationKind::TypeAlias => GeneratedHeaderKind::Type,
                AssociatedDeclarationKind::MacroProducer => GeneratedHeaderKind::Other,
            };
            if header.kind != expected
                || header.key.expansion != output.record().key().fingerprint()
            {
                return Err(generated_fault("associated pinned header kind/key differs"));
            }
            match (&row.name, &header.name) {
                (Some(name), Some(actual)) => {
                    let span_index = spans
                        .binary_search_by_key(&Digest(actual.span.0), |row| row.0)
                        .map_err(|_| generated_fault("associated name actual span absent"))?;
                    let span = &output.tokens().spans()[spans[span_index].1 as usize];
                    let edition = match span.edition {
                        crate::syntax::token_wire::Edition::E2015 => 2015,
                        crate::syntax::token_wire::Edition::E2018 => 2018,
                        crate::syntax::token_wire::Edition::E2021 => 2021,
                        crate::syntax::token_wire::Edition::E2024 => 2024,
                    };
                    if actual.start != name.range.start
                        || actual.end != name.range.end
                        || actual.text != name.spelling
                        || name.edition != edition
                        || !matches!(name.context,DeclarationNameContext::Captured(context) if context==span.context)
                    {
                        return Err(generated_fault(
                            "associated name physical context/edition attachment differs",
                        ));
                    }
                }
                (None, None) => {}
                _ => return Err(generated_fault("associated name capture differs")),
            }
            let declaration = at_span(&declarations, row.range)?;
            let body = if row.kind == AssociatedDeclarationKind::MacroProducer {
                if declaration.is_some() || row.producer.is_none() {
                    return Err(generated_fault(
                        "unexpanded associated producer became a declaration",
                    ));
                }
                None
            } else {
                let item = &outline.items[declaration
                    .ok_or_else(|| generated_fault("associated ordinary Outline row absent"))?
                    as usize];
                let (kind, namespace) = match row.kind {
                    AssociatedDeclarationKind::Function => (ItemKind::Function, Namespace::Value),
                    AssociatedDeclarationKind::Const => (ItemKind::Constant, Namespace::Value),
                    AssociatedDeclarationKind::TypeAlias => (ItemKind::TypeAlias, Namespace::Type),
                    AssociatedDeclarationKind::MacroProducer => unreachable!(),
                };
                if item.def_key.item_kind != kind
                    || item.def_key.namespace != namespace
                    || item.scope.owner != Some(owner_identity)
                    || item.def_key.enclosing_impl != Some(owner_identity)
                    || item.scope.crate_key != owner.scope.crate_key
                    || item.scope.module_path != owner.scope.module_path
                    || item.header.context != owner.header.tokens
                    || item.header.tokens != row.header
                    || item.body.as_ref().map(|body| body.range) != row.body
                {
                    return Err(generated_fault(
                        "associated ordinary Outline parent/header/body differs",
                    ));
                }
                item.body.clone().map(|mut body| {
                    body.def_key = header.key.fingerprint();
                    body
                })
            };
            slots.push(GeneratedSlot {
                source: u32::try_from(ordinal).map_err(|_| resource())?,
                header: header_index,
                declaration,
                body,
            });
        }
        // Slots already follow actual retained AST order, which is an attachment
        // locator, not a member lookup order or a negative namespace answer.
        let metadata_bytes = checked_add(
            checked_add(std::mem::size_of::<Self>(), outline.estimated_bytes())?,
            receipt_heap,
        )?
        .checked_add(
            outputs
                .capacity()
                .checked_mul(std::mem::size_of::<Arc<GeneratedDeclarations>>())
                .ok_or_else(resource)?,
        )
        .and_then(|n| {
            n.checked_add(
                receipts
                    .capacity()
                    .checked_mul(std::mem::size_of::<SyntaxRequestReceipt>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                slots
                    .capacity()
                    .checked_mul(std::mem::size_of::<GeneratedSlot>())?,
            )
        })
        .ok_or_else(resource)?;
        let mut encoded = StableEncoder::new("lrr/v1/generated-associated-declaration-source-join");
        encoded.digest(original.fingerprint());
        encoded.digest(owner.def_key.fingerprint());
        encoded.digest(owner_identity);
        encoded.u64(epoch);
        encoded.sequence(&outputs, |e, output| {
            e.digest(output.fingerprint());
            e.record(&output.record().descriptor());
        });
        encoded.digest(outline.fingerprint());
        encoded.sequence(&slots, |e, row| {
            e.u32(row.source);
            e.u32(row.header);
            e.record(
                &outputs.last().expect("checked associated chain").headers()[row.header as usize]
                    .key,
            );
            e.option(&row.declaration, |e, n| e.u32(*n));
            e.option(&row.body, |e, b| e.record(b));
        });
        Ok(Self {
            source,
            owner,
            owner_identity,
            outputs,
            receipts,
            outline,
            slots,
            fingerprint: encoded.finish(),
            metadata_bytes,
        })
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    pub(crate) fn owner_identity(&self) -> Digest {
        self.owner_identity
    }
    pub(crate) fn outputs(&self) -> &[Arc<GeneratedDeclarations>] {
        &self.outputs
    }
    pub(crate) fn receipts(&self) -> &[SyntaxRequestReceipt] {
        &self.receipts
    }
    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }
    pub(crate) fn header(&self, ordinal: usize) -> Option<GeneratedAssociatedHeader<'_>> {
        let row = self.slots.get(ordinal)?;
        let output = self.outputs.last()?;
        Some(GeneratedAssociatedHeader {
            source: &output.scan().associated_fragments()[row.source as usize],
            key: &output.headers()[row.header as usize].key,
            owner: self.owner,
            declaration: row.declaration.map(|n| &self.outline.items[n as usize]),
            body: row.body.as_ref(),
        })
    }
}
#[cfg(test)]
#[path = "generated_associated_declaration_tests.rs"]
mod tests;
