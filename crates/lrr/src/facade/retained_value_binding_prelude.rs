//! Read-only selection from an already privately minted compiler Value facet.
//! Absence concerns this selected finite facade, not locals/module/imports or a
//! different compiler/prelude world. Positive names still need ordinary source
//! Value/variant selection; this receipt supplies no invented resolved identity.
use super::{VALUES, ValueBindingPrelude, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::changes::PreparedSourceView,
    syntax::outline::ScopeKey,
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RetainedPreludeName {
    AbsentInSelectedFacet,
    RequiresCommonValueResolution(&'static str),
}
/// Opaque cold source/receipt join. Current published source/compiler/seal
/// dependencies belong to the enclosing common query; this grants no binding.
#[derive(Debug, Clone)]
pub(crate) struct RetainedValueBindingPrelude {
    receipt: ValueBindingPrelude,
    source_identity: Digest,
    identity: Digest,
    bytes: usize,
}
impl ValueBindingPrelude {
    /// This performs no parse or catalogue replay. The existing opaque receipt
    /// was minted from actual source policy and a complete matched compiler
    /// Value facet during source discovery, not from caller lookup assertions.
    pub(crate) fn retain(
        &self,
        source: &PreparedSourceView,
        scope: &ScopeKey,
    ) -> Result<RetainedValueBindingPrelude, Diagnostic> {
        source.require_original_hygiene()?;
        if self.source != *source.selection()
            || self.scope != *scope
            || scope.owner.is_some()
            || scope.crate_key != self.source.outline.crate_key
            || !scope
                .module_path
                .starts_with(&self.source.outline.module_path)
        {
            return Err(fault("retained Value prelude actual source/scope differs"));
        }
        self.source.outline.parse.profile.validate_source()?;
        let bytes = self
            .metadata_bytes()?
            .checked_sub(std::mem::size_of::<Self>())
            .and_then(|heap| heap.checked_add(std::mem::size_of::<RetainedValueBindingPrelude>()))
            .ok_or_else(|| fault("retained Value prelude metadata limit"))?;
        let mut source_identity = StableEncoder::new("lrr/v1/retained-value-prelude-source");
        source_identity.string(self.source.path.as_str());
        source_identity.record(&self.source.outline);
        source_identity.record(&self.scope);
        Ok(RetainedValueBindingPrelude {
            receipt: self.clone(),
            source_identity: source_identity.finish(),
            identity: self.fingerprint(),
            bytes,
        })
    }
}
impl RetainedValueBindingPrelude {
    pub(crate) fn source_identity(&self) -> Digest {
        self.source_identity
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn owning_edition(&self) -> u16 {
        self.receipt.source.outline.parse.profile.edition
    }
    /// Exact minted source/compilation facet, available for explicit query keys
    /// and semantic dependency encoding; neither digest alone grants authority.
    pub(crate) fn compiler(&self) -> Digest {
        self.receipt.compiler
    }
    pub(crate) fn facet(&self) -> Digest {
        self.receipt.facet
    }
    /// Constant-size finite namespace selection; no parsing, source traversal,
    /// catalogue construction, namespace census or allocation during retry.
    pub(crate) fn lookup(&self, name: &str) -> RetainedPreludeName {
        if !self.receipt.disabled {
            for declared in VALUES {
                if name == declared {
                    return RetainedPreludeName::RequiresCommonValueResolution(declared);
                }
            }
        }
        RetainedPreludeName::AbsentInSelectedFacet
    }
}
impl StableEncode for RetainedValueBindingPrelude {
    const DOMAIN: &'static str = "lrr/v1/retained-value-binding-prelude";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.source_identity);
        e.digest(self.identity);
        e.digest(self.receipt.compiler);
        e.digest(self.receipt.facet);
        e.u16(self.owning_edition());
        e.bool(self.receipt.standard);
        e.bool(self.receipt.disabled);
    }
}
