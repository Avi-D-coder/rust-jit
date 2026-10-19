//! Immutable actual source assembly inputs. Completion captures them once;
//! selected preparation consumes only the demanded sealed producer chain.
use super::*;
use crate::{
    db::seals::SealKey,
    expand::source_declarative::assembly::{LazySourceAssemblySeed, SourceAssemblySeed},
};
const ARC: usize = 2 * std::mem::size_of::<usize>();
/// One macro-generated associated function of an original impl/trait.
#[derive(Clone, Debug)]
pub(crate) struct GeneratedAssociatedMember {
    pub(crate) identity: Digest,
    pub(crate) owner: Digest,
    pub(crate) owner_trait: bool,
    pub(crate) name: String,
    /// An associated constant (otherwise a function).
    pub(crate) constant: bool,
    pub(crate) provided: bool,
}
pub(super) struct SyntaxReply {
    pub(super) output: Arc<GeneratedDeclarations>,
    pub(super) receipt: Option<Arc<SyntaxRequestReceipt>>,
}
pub(crate) struct CompletedSourceSyntax {
    seed: LazySourceAssemblySeed,
    replies: BTreeMap<(Digest, Digest), SyntaxReply>,
    seal: SealKey,
    seal_value: Digest,
    identity: Digest,
    bytes: usize,
}
impl CompletedSourceSyntax {
    pub(crate) fn original_local_declaration(&self,
        completion: &crate::frontend::impl_index::CompletedOriginalAnnotation,
    ) -> Result<Arc<crate::expand::source_declarative::assembly::InitialLocalDeclaration>, Diagnostic> {
        self.seed.get()?.original_local_declaration(completion)
    }
    pub(super) fn capture(
        seed: Arc<SourceAssemblySeed>,
        replies: BTreeMap<(Digest, Digest), SyntaxReply>,
        seal: SealKey,
        seal_value: Digest,
    ) -> Result<Arc<Self>, Diagnostic> {
        Self::capture_lazy(LazySourceAssemblySeed::ready(seed)?, replies, seal, seal_value)
    }
    /// Capture the issued replies now; the initial source assembly itself is
    /// built only when this source is first prepared for entry.
    pub(super) fn capture_lazy(
        seed: LazySourceAssemblySeed,
        replies: BTreeMap<(Digest, Digest), SyntaxReply>,
        seal: SealKey,
        seal_value: Digest,
    ) -> Result<Arc<Self>, Diagnostic> {
        if seed.epoch() != seal.epoch {
            return Err(fault("completed source syntax epoch/count differs"));
        }
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(ARC)
            .ok_or_else(crate::runtime::arena::resource)?;
        // The assembly is a pure function of these inputs.
        let mut identity =
            crate::core::digest::StableEncoder::new("lrr/v2/completed-source-syntax-inputs");
        identity.string(seed.source().path.as_str());
        identity.record(&seed.source().outline);
        identity.u64(seed.epoch());
        identity.digest(seed.scan().fingerprint());
        identity.record(&seal);
        identity.digest(seal_value);
        identity.u64(replies.len() as u64);
        for ((parent, invocation), reply) in &replies {
            let output = &reply.output;
            if output.parent_inventory() != *parent
                || output.parent_invocation() != *invocation
                || reply
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| output.record().key() != receipt.key())
                || output.epoch() != seal.epoch
            {
                return Err(fault("completed source syntax issued reply differs"));
            }
            bytes = bytes
                .checked_add(256 + 16 * std::mem::size_of::<((Digest, Digest), SyntaxReply)>())
                .and_then(|n| n.checked_add(output.metadata_bytes().ok()?))
                .and_then(|n| {
                    n.checked_add(
                        reply
                            .receipt
                            .as_ref()
                            .map(|receipt| receipt.metadata_bytes())
                            .transpose()
                            .ok()?
                            .unwrap_or(0),
                    )
                })
                .and_then(|n| n.checked_add(ARC))
                .ok_or_else(crate::runtime::arena::resource)?;
            identity.digest(*parent);
            identity.digest(*invocation);
            identity.digest(output.fingerprint());
            identity.option(&reply.receipt, |e, receipt| e.record(receipt.as_ref()));
        }
        Ok(Arc::new(Self {
            seed,
            replies,
            seal,
            seal_value,
            identity: identity.finish(),
            bytes,
        }))
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        self.seed.source()
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.seed.epoch()
    }
    pub(crate) fn seal(&self) -> &SealKey {
        &self.seal
    }
    pub(crate) fn seal_value(&self) -> Digest {
        self.seal_value
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn prepare_body(
        &self,
        definition: Digest,
    ) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        self.prepare(Some(definition))
    }
    pub(crate) fn prepare_source(&self) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        self.prepare(None)
    }
    /// Macro-generated functions written by this source's admitted outputs
    /// whose own text contains a macro producer. Structural only: no body is
    /// assembled, lowered or published by this enumeration.
    pub(crate) fn generated_macro_bearing_functions(&self) -> Vec<Digest> {
        let mut found = Vec::new();
        for reply in self.replies.values() {
            let output = &reply.output;
            let calls = output
                .scan()
                .producers()
                .iter()
                .filter(|producer| {
                    producer.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                })
                .map(|producer| producer.range)
                .collect::<Vec<_>>();
            if calls.is_empty() {
                continue;
            }
            for header in output.headers() {
                if header.kind == crate::expand::generated_items::GeneratedHeaderKind::Function
                    && calls
                        .iter()
                        .any(|call| header.header_end <= call.start && call.end <= header.end)
                {
                    found.push(header.key.fingerprint());
                }
            }
        }
        found
    }
    /// Macro-generated associated functions/constants written into an original impl or
    /// trait of this source through its actual issued impl/trait-item replies
    /// (nested associated calls included). Structural only: nothing is
    /// assembled, published or lowered by this enumeration.
    pub(crate) fn generated_associated_members(
        &self,
    ) -> Result<Vec<GeneratedAssociatedMember>, Diagnostic> {
        use crate::{expand::identity::GrammarSlot, source::manifest::ItemKind};
        let associated = |output: &GeneratedDeclarations| {
            matches!(
                output.record().key().grammar,
                GrammarSlot::ImplItems | GrammarSlot::TraitItems
            )
        };
        let root = self.seed.scan().fingerprint();
        let parents = self
            .replies
            .iter()
            .map(|(key, reply)| (reply.output.scan().fingerprint(), (*key, &reply.output)))
            .collect::<BTreeMap<_, _>>();
        let source = self.seed.source_view();
        let mut members = Vec::new();
        'outputs: for (key, reply) in &self.replies {
            if !associated(&reply.output) {
                continue;
            }
            // The root call of this associated chain, in the original source.
            let mut root_key = *key;
            let mut steps = 0usize;
            while root_key.0 != root {
                let Some((parent_key, parent)) = parents.get(&root_key.0) else {
                    continue 'outputs;
                };
                steps += 1;
                if !associated(parent) || steps > self.replies.len() {
                    continue 'outputs;
                }
                root_key = *parent_key;
            }
            let Some(producer) = self
                .seed
                .scan()
                .producers()
                .iter()
                .find(|producer| producer.identity == root_key.1)
            else {
                continue;
            };
            let Some(owner) = source
                .outline()
                .items
                .iter()
                .filter(|item| {
                    matches!(item.def_key.item_kind, ItemKind::Impl | ItemKind::Trait)
                        && item.declaration_span.contains(producer.range)
                })
                .min_by_key(|item| item.declaration_span.len())
            else {
                continue;
            };
            let (owner_identity, owner_trait) = match owner.def_key.item_kind {
                ItemKind::Impl => match owner.def_key.enclosing_impl {
                    Some(identity) => (identity, false),
                    None => continue,
                },
                _ => (owner.def_key.fingerprint(), true),
            };
            let parsed = reply.output.admitted_parsed();
            for header in reply.output.headers() {
                use crate::expand::generated_items::GeneratedHeaderKind;
                let constant = match header.kind {
                    GeneratedHeaderKind::Function => false,
                    GeneratedHeaderKind::Const => true,
                    _ => continue,
                };
                if header.key.structural_path.len() != 1 {
                    continue;
                }
                let Some(name) = &header.name else {
                    continue;
                };
                use ra_ap_syntax::{AstNode, ast};
                let provided = parsed
                    .syntax_at(
                        crate::syntax::spans::SourceRange::checked(header.start, header.end)?,
                        |node| ast::Fn::can_cast(node.kind()) || ast::Const::can_cast(node.kind()),
                    )?
                    .is_some_and(|node| match ast::Fn::cast(node.clone()) {
                        Some(function) => function.body().is_some(),
                        None => ast::Const::cast(node).is_some_and(|value| value.body().is_some()),
                    });
                members.push(GeneratedAssociatedMember {
                    identity: header.key.fingerprint(),
                    owner: owner_identity,
                    owner_trait,
                    name: name.text.clone(),
                    constant,
                    provided,
                });
            }
        }
        Ok(members)
    }
    /// The admitted output that wrote a macro-generated declaration, and the
    /// outer-to-inner chain of issued replies that attaches it to this source.
    /// Pure lookup over this plan's own sealed replies; nothing is expanded.
    fn generated_target(
        &self,
        definition: Digest,
    ) -> Result<
        Option<(
            crate::expand::source_declarative::assembly::GeneratedBodyTarget,
            Vec<(Digest, Digest)>,
        )>,
        Diagnostic,
    > {
        let mut found = None;
        for (key, reply) in &self.replies {
            for header in reply.output.headers() {
                if header.key.fingerprint() != definition {
                    continue;
                }
                if found.is_some() {
                    return Err(fault("generated declaration has several admitted outputs"));
                }
                found = Some((*key, &reply.output, header));
            }
        }
        let Some((key, output, header)) = found else {
            return Ok(None);
        };
        if header.key.expansion != output.record().key().fingerprint() {
            return Err(fault("generated declaration record identity differs"));
        }
        let target = crate::expand::source_declarative::assembly::GeneratedBodyTarget {
            definition,
            record: output.record().key().fingerprint(),
            output: crate::syntax::spans::SourceRange::checked(header.start, header.end)?,
        };
        // Walk parents through the outputs' own scans to the original source.
        let root = self.seed.scan().fingerprint();
        let parents = self
            .replies
            .iter()
            .map(|(key, reply)| (reply.output.scan().fingerprint(), *key))
            .collect::<BTreeMap<_, _>>();
        let mut chain = vec![key];
        let mut parent = key.0;
        while parent != root {
            let next = *parents
                .get(&parent)
                .ok_or_else(|| fault("generated declaration producer chain absent"))?;
            if chain.len() > self.replies.len() || chain.contains(&next) {
                return Err(fault("generated declaration producer chain differs"));
            }
            chain.push(next);
            parent = next.0;
        }
        chain.reverse();
        Ok(Some((target, chain)))
    }
    fn prepare(&self, definition: Option<Digest>) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        if let Some(definition) = definition
            && !self
                .seed
                .source_view()
                .outline()
                .items
                .iter()
                .any(|item| item.def_key.fingerprint() == definition)
        {
            return self.prepare_generated(definition);
        }
        let mut assembly = self.seed.get()?.fork()?;
        let mut used = BTreeSet::new();
        loop {
            let pending = match definition {
                Some(definition) => assembly.next_body_invocation(definition)?,
                None => assembly.next_pending_invocation()?,
            };
            let Some(key) = pending else {
                break;
            };
            let reply = self.replies.get(&key).ok_or_else(|| fault(&format!("source syntax call requires an actual expansion output: producer={}, parent={}", key.1, key.0)))?;
            if used.contains(&key) {
                return Err(fault("source syntax producer output was already consumed"));
            }
            used.len()
                .checked_add(1)
                .and_then(|len| {
                    len.checked_mul(256 + 16 * std::mem::size_of::<(Digest, Digest)>())
                })
                .ok_or_else(crate::runtime::arena::resource)?;
            used.insert(key);
            let receipt = reply.receipt.as_ref().ok_or_else(|| {
                fault("source syntax output requires its actual issued request receipt")
            })?;
            assembly = assembly.attach_receipt(receipt, Arc::clone(&reply.output))?;
        }
        if definition.is_none() && used.len() != self.replies.len() {
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
    /// The same scoped assembly for a macro-generated function: attach only
    /// the actual issued producer chain that wrote its declaration, then the
    /// calls its own body owns. Other bodies keep their pending markers.
    fn prepare_generated(
        &self,
        definition: Digest,
    ) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        let (target, chain) = self.generated_target(definition)?.ok_or_else(|| {
            fault("selected generated body has no admitted output in this source")
        })?;
        let mut assembly = self.seed.get()?.fork()?;
        let mut used = BTreeSet::new();
        for key in chain {
            assembly = self.attach_reply(assembly, key, &mut used)?;
        }
        while let Some(key) = assembly.next_generated_body_invocation(&target)? {
            assembly = self.attach_reply(assembly, key, &mut used)?;
        }
        let prepared = assembly.finish_for_generated_body(&target)?;
        if !prepared.ready_for_admission() || !prepared.permits_body(definition) {
            return Err(fault("source syntax preparation left pending calls"));
        }
        Ok(Arc::new(prepared))
    }
    /// Attach one issued reply exactly once, through its actual receipt.
    fn attach_reply(
        &self,
        assembly: crate::expand::source_declarative::assembly::SourceSyntaxAssembly,
        key: (Digest, Digest),
        used: &mut BTreeSet<(Digest, Digest)>,
    ) -> Result<crate::expand::source_declarative::assembly::SourceSyntaxAssembly, Diagnostic> {
        let reply = self.replies.get(&key).ok_or_else(|| {
            fault(&format!(
                "source syntax call requires an actual expansion output: producer={}, parent={}",
                key.1, key.0
            ))
        })?;
        if !used.insert(key) {
            return Err(fault("source syntax producer output was already consumed"));
        }
        let receipt = reply.receipt.as_ref().ok_or_else(|| {
            fault("source syntax output requires its actual issued request receipt")
        })?;
        assembly.attach_receipt(receipt, Arc::clone(&reply.output))
    }
}

#[cfg(test)]
#[path = "completed_source_syntax_tests.rs"]
mod tests;
