//! T054 typed query-key fields. Content identity does not grant source authority.
use super::{memo::ConstMemoPayload, wire::ConstantWireType};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    syntax::pure_constants::ConstantPlan,
};
/// Exact identity for the selected pure plan and its expected concrete type.
/// A caller-supplied admission digest is only identity data. Current source/seal
/// evidence must be privately minted and replayed by the coordinator/provider.
/// This record never grants macro, build, native or guest execution authority.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstQueryIdentity {
    definition: Digest,
    selection: Digest,
    source: Digest,
    cfg: Digest,
    profile: ProfileIdentity,
    result_type: Digest,
    admission: Digest,
    universe: Option<Digest>,
}
impl ConstQueryIdentity {
    /// Checked content constructor. Concrete arguments and the exact selected
    /// expression are already encoded in the captured plan identity.
    pub fn capture(
        plan: &ConstantPlan,
        expected: &ConstantWireType,
        admission: Digest,
    ) -> Result<Self, Diagnostic> {
        plan.profile().validate_source()?;
        expected.fingerprint_checked()?;
        // Admit sizes before copying the profile into an owned query identity.
        expected.metadata_bytes()?;
        std::mem::size_of::<Self>()
            .checked_add(plan.profile().semantics.len())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "constant query identity metadata limit".into(),
                )
            })?;
        Self::capture_content(
            plan.owner_definition(),
            plan.identity(),
            plan.source().source(),
            plan.source().cfg(),
            plan.profile().clone(),
            expected,
            admission,
        )
    }
    /// Content data only; this constructor never mints current source authority.
    /// Serialized keys are equally untrusted and are checked again at fetch.
    pub fn capture_content(
        definition: Digest,
        selection: Digest,
        source: Digest,
        cfg: Digest,
        profile: ProfileIdentity,
        expected: &ConstantWireType,
        admission: Digest,
    ) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        let result_type = expected.fingerprint_checked()?;
        let key = Self {
            definition,
            selection,
            source,
            cfg,
            profile,
            result_type,
            admission,
            universe: None,
        };
        key.validate()?;
        Ok(key)
    }
    pub(crate) fn bind_universe(mut self, universe: Digest) -> Self {
        self.universe = Some(universe);
        self
    }
    pub(crate) fn universe(&self) -> Option<Digest> {
        self.universe
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn source(&self) -> Digest {
        self.source
    }
    pub fn cfg(&self) -> Digest {
        self.cfg
    }
    pub fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub fn result_type(&self) -> Digest {
        self.result_type
    }
    pub fn validate(&self) -> Result<(), Diagnostic> {
        self.profile.validate_source()?;
        self.metadata_bytes()?;
        Ok(())
    }
    pub fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        self.validate()?;
        Ok(self.profile.semantics.capacity())
    }
    pub fn selection(&self) -> Digest {
        self.selection
    }
    pub fn admission(&self) -> Digest {
        self.admission
    }
    /// Compare all content/type fields before accepting or decoding the result.
    /// The payload itself carries no admission proof; replay remains mandatory.
    pub fn matches_payload(&self, payload: &ConstMemoPayload) -> bool {
        self.definition == payload.definition()
            && self.selection == payload.selection()
            && self.source == payload.source()
            && self.cfg == payload.cfg()
            && &self.profile == payload.profile()
            && self.result_type == payload.type_identity()
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.profile.semantics.capacity())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "constant query identity metadata limit".into(),
                )
            })
    }
}
impl StableEncode for ConstQueryIdentity {
    // A new domain forces old definition/argument-digest-only cache entries to miss.
    const DOMAIN: &'static str = "lrr/v3/typed-constant-query-identity";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.selection);
        e.digest(self.source);
        e.digest(self.cfg);
        e.record(&self.profile);
        e.digest(self.result_type);
        e.digest(self.admission);
        e.option(&self.universe, |e, universe| e.digest(*universe));
    }
}
