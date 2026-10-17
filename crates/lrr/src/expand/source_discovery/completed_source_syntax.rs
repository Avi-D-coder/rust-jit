//! Immutable actual source assembly inputs. Completion captures them once;
//! selected preparation consumes only the demanded sealed producer chain.
use super::*;
use crate::{db::seals::SealKey, expand::source_declarative::assembly::SourceAssemblySeed};
const LIMIT: usize = 128 * 1024 * 1024;
const MAX_REPLIES: usize = 100_000;
const ARC: usize = 2 * std::mem::size_of::<usize>();
pub(super) struct SyntaxReply {
    pub(super) output: Arc<GeneratedDeclarations>,
    pub(super) receipt: Option<Arc<SyntaxRequestReceipt>>,
}
pub(crate) struct CompletedSourceSyntax {
    seed: Arc<SourceAssemblySeed>,
    replies: BTreeMap<(Digest, Digest), SyntaxReply>,
    seal: SealKey,
    seal_value: Digest,
    identity: Digest,
    bytes: usize,
}
impl CompletedSourceSyntax {
    pub(super) fn capture(
        seed: Arc<SourceAssemblySeed>,
        replies: BTreeMap<(Digest, Digest), SyntaxReply>,
        seal: SealKey,
        seal_value: Digest,
    ) -> Result<Arc<Self>, Diagnostic> {
        if seed.epoch() != seal.epoch || replies.len() > MAX_REPLIES {
            return Err(fault("completed source syntax epoch/count differs"));
        }
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(ARC)
            .and_then(|n| n.checked_add(seed.metadata_bytes()))
            .filter(|n| *n <= LIMIT)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut identity =
            crate::core::digest::StableEncoder::new("lrr/v1/completed-source-syntax-inputs");
        identity.digest(seed.fingerprint());
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
                .filter(|n| *n <= LIMIT)
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
    fn prepare(&self, definition: Option<Digest>) -> Result<Arc<PreparedSourceSyntax>, Diagnostic> {
        let mut assembly = self.seed.fork()?;
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
                        .filter(|n| *n <= 32 * 1024 * 1024)
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
}

#[cfg(test)]
#[path = "completed_source_syntax_tests.rs"]
mod tests;
