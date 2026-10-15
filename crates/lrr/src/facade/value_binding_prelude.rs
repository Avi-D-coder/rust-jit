//! Finite compiler-owned Value-prelude declarations for pattern disambiguation.
//! This is the selected synthetic facade, not a host/sysroot export catalogue.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::outline::ScopeKey,
};
// Pinned core prelude v1, shared by the four supported owning editions. 2021
// iterator/conversion and 2024 async additions are Type-only declarations.
// The finite selected facade deliberately makes no real-library completeness claim.
const VALUES: [&str; 9] = [
    "Some",
    "None",
    "Ok",
    "Err",
    "drop",
    "align_of",
    "align_of_val",
    "size_of",
    "size_of_val",
];
pub(crate) fn encode_inventory(e: &mut StableEncoder) {
    e.sequence(&VALUES, |e, name| e.string(name));
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-PRELUDE", message.into())
}
/// Minted only during actual SourceDiscovery publication from its retained
/// owning compiler catalogue and original source. Callers cannot issue a Boolean.
#[derive(Debug, Clone)]
pub(crate) struct ValueBindingPrelude {
    source: SourceSelection,
    scope: ScopeKey,
    compiler: Digest,
    facet: Digest,
    standard: bool,
    disabled: bool,
}
impl ValueBindingPrelude {
    pub(crate) fn prepare(
        source: &PreparedSourceView,
        catalog: &CompileInputCatalog,
        scope: &ScopeKey,
    ) -> Result<Option<Self>, Diagnostic> {
        source.require_original_hygiene()?;
        if !source.selection().outline.module_path.is_empty() {
            return Ok(None); // Owning-crate policy join remains a typed consumer prerequisite.
        }
        // Conservative bounded structural work before parse/header selection.
        if source
            .captured_macro_blob()?
            .bytes()
            .len()
            .checked_mul(128)
            .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            return Err(fault("Value prelude source/header metadata limit"));
        }
        let profile = &source.selection().outline.parse.profile;
        let facet = catalog.value_binding_facet(profile)?;
        let compiler = catalog.owning_profile_identity()?;
        if scope.crate_key != source.selection().outline.crate_key
            || scope.owner.is_some()
            || !scope
                .module_path
                .starts_with(&source.selection().outline.module_path)
        {
            return Err(fault("Value prelude owning source/module differs"));
        }
        let parsed = crate::syntax::parse::ParsedFile::parse(
            source.captured_macro_blob()?,
            profile.clone(),
        )?;
        let Some(policy) = parsed.binding_prelude_policy(source.captured_macro_cfg()?, scope)?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            source: source.selection().clone(),
            scope: scope.clone(),
            compiler,
            facet,
            standard: policy.standard(),
            disabled: policy.disabled(),
        }))
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub(crate) fn require_absent(
        &self,
        source: &PreparedSourceView,
        scope: &ScopeKey,
        name: &str,
    ) -> Result<(), Diagnostic> {
        if self.source != *source.selection() || &self.scope != scope {
            return Err(fault("Value prelude source or owning scope differs"));
        }
        // Replay the real source policy and the exact privately minted compiler
        // generation. It cannot be manufactured by a partial custom catalogue.
        let catalog = CompileInputCatalog::standalone(&self.source.outline.parse.profile)?;
        let actual = Self::prepare(source, &catalog, scope)?
            .ok_or_else(|| fault("Value prelude selected policy remains unestablished"))?;
        if actual.fingerprint() != self.fingerprint() {
            return Err(fault("Value prelude source/compiler policy changed"));
        }
        if !self.disabled && VALUES.contains(&name) {
            return Err(fault(
                "plain pattern has a selected Value prelude declaration",
            ));
        }
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.source.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .ok_or_else(|| fault("Value prelude receipt metadata overflow"))
    }
}
impl StableEncode for ValueBindingPrelude {
    const DOMAIN: &'static str = "lrr/v1/source-selected-value-binding-prelude";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.source.path.as_str());
        e.record(&self.source.outline.parse);
        e.digest(self.source.outline.crate_key);
        e.sequence(&self.source.outline.module_path, |e, n| e.string(n));
        e.digest(self.source.outline.cfg);
        e.record(&self.scope);
        e.digest(self.compiler);
        e.digest(self.facet);
        e.bool(self.standard);
        e.bool(self.disabled);
        encode_inventory(e);
    }
}
