//! Closed Section17 Value declaration facet, distinct from Macro/Attribute facets.
use super::*;
use crate::core::profile::ProfileIdentity;
#[derive(Debug, Clone)]
pub(super) struct CompilerValueBindingFacet {
    profile: ProfileIdentity,
}
impl CompilerValueBindingFacet {
    pub(super) fn for_profile(profile: &ProfileIdentity) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        Ok(Self {
            profile: profile.clone(),
        })
    }
    pub(super) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/compiler-value-binding-facet");
        e.record(&self.profile);
        crate::facade::value_binding_prelude::encode_inventory(&mut e);
        e.finish()
    }
    pub(super) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.profile.semantics.capacity())
            .ok_or_else(|| fault("Value declaration facet metadata overflow"))
    }
}
impl CompileInputCatalog {
    /// Only a genuine privately minted matched compiler facet can select the
    /// closed synthetic prelude. Matching Macro/Attribute exports is insufficient.
    pub(crate) fn value_binding_facet(
        &self,
        profile: &ProfileIdentity,
    ) -> Result<Digest, Diagnostic> {
        let facet = self
            .value_binding
            .as_ref()
            .ok_or_else(|| fault("compiler Value declaration facet is absent"))?;
        if &facet.profile != profile || self.canonical_standalone_member(profile)?.is_none() {
            return Err(fault(
                "compiler Value declaration facet/profile membership differs",
            ));
        }
        Ok(facet.identity())
    }
}
