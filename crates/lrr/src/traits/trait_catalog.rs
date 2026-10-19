//! Complete raw trait membership before demand-specific typed fact preparation.
//! Membership is not applicability, inference, a solver answer or executable code.
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
        seals::{DiscoveryInput, SealDomain, SealKey}, value::QueryValue},
    frontend::impl_trait_identity::{ImplTraitIdentityResult, PreparedImplTraitIdentity},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug)]
pub struct PreparedImplTraitCatalog {
    key: SealKey,
    inventory: Arc<QueryValue>,
    identities: BTreeMap<Digest, Arc<QueryValue>>,
    buckets: BTreeMap<Digest, Vec<(Digest, Digest)>>,
    pending: Vec<Digest>,
}
impl PreparedImplTraitCatalog {
    pub fn key(&self) -> &SealKey { &self.key }
    pub fn pending(&self) -> &[Digest] { &self.pending }
    pub(crate) fn candidates(&self, trait_id: Digest) -> Option<&[(Digest, Digest)]> {
        self.pending.is_empty().then(|| self.buckets.get(&trait_id).map(Vec::as_slice).unwrap_or(&[]))
    }
    /// Source-ID half of typed provider publication. Extra translated impls may
    /// serve other demanded traits/inherent items; this exact bucket may omit none.
    pub(crate) fn require_translation(&self, publication: &DiscoveryInput, trait_id: Digest,
        translated: &BTreeMap<Digest, Digest>,
    ) -> Result<&[(Digest, Digest)], Diagnostic> {
        if !self.matches_catalog(publication) {
            return Err(fault("translated candidates require the same complete source catalog"));
        }
        let candidates = self.candidates(trait_id).ok_or_else(|| fault("candidate source membership remains pending"))?;
        for (raw, canonical) in candidates {
            if translated.get(raw) != Some(canonical) {
                return Err(fault("demanded typed bucket omits or changes an actual candidate"));
            }
        }
        Ok(candidates)
    }
    pub(crate) fn classification(&self, raw: Digest) -> Option<&PreparedImplTraitIdentity> {
        match self.identities.get(&raw)?.as_ref() {
            QueryValue::ImplTraitIdentity(identity) => Some(identity), _ => None,
        }
    }
    /// The typed provider may consume only this same complete immutable world.
    /// A filtered/scope inventory or pending classification never proves absence.
    pub(crate) fn matches_catalog(&self, publication: &DiscoveryInput) -> bool {
        let QueryValue::PreparedScopeImplHeaders(headers) = self.inventory.as_ref() else { return false; };
        self.pending.is_empty() && self.key == publication.seal.key()
            && matches!(self.key.domain, SealDomain::ImplCatalog { .. })
            && Arc::ptr_eq(headers.inventory_root(), &publication.declaration_inventory)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SealKey) -> QueryPoll<Self> {
        let inventory = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = inventory.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait membership inventory variant differs")));
        };
        let mut identities = BTreeMap::new(); let mut buckets = BTreeMap::<_, Vec<_>>::new(); let mut pending = Vec::new();
        for raw in headers.inventory_root().headers() {
            let identity = match db.fetch(&SyntaxProvider, QueryKey::ImplTraitIdentity(Box::new(
                super::header_syntax::ImplHeaderSyntaxKey { inventory: key.clone(), implementation: raw.identity }))) {
                QueryPoll::Ready(Ok(value)) => value.value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::ImplTraitIdentity(classified) = identity.as_ref() else {
                return QueryPoll::Ready(Err(fault("trait membership classification variant differs")));
            };
            match classified.result() {
                ImplTraitIdentityResult::Pending => pending.push(raw.identity),
                ImplTraitIdentityResult::Inherent => {},
                ImplTraitIdentityResult::Trait(trait_id) => {
                    let Some(owner) = classified.syntax().canonical_owner() else {
                        return QueryPoll::Ready(Err(fault("classified trait impl canonical owner absent")));
                    };
                    buckets.entry(trait_id).or_default().push((raw.identity, owner.identity()));
                }
            }
            identities.insert(raw.identity, identity);
        }
        QueryPoll::Ready(Ok(Self { key: key.clone(), inventory, identities, buckets, pending }))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-IMPL-HEADER", message.into()) }
impl StableEncode for PreparedImplTraitCatalog {
    const DOMAIN: &'static str = "lrr/v1/prepared-impl-trait-catalog";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        let QueryValue::PreparedScopeImplHeaders(headers) = self.inventory.as_ref() else { unreachable!("checked source inventory") };
        e.digest(headers.fingerprint());
        e.map(&self.identities, |e, raw, identity| {
            e.digest(*raw);
            let QueryValue::ImplTraitIdentity(identity) = identity.as_ref() else { unreachable!("checked source identity") };
            e.record(identity.as_ref());
        });
    }
}
