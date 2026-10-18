//! Finite compiler-owned Value-prelude declarations for pattern disambiguation.
//! This is the selected synthetic facade, not a host/sysroot export catalogue.
#[path = "retained_value_binding_prelude.rs"]
mod retained;
pub(crate) use retained::{RetainedPreludeName, RetainedValueBindingPrelude};

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
    policy_source: SourceSelection,
    scope: ScopeKey,
    compiler: Digest,
    facet: Digest,
    standard: bool,
    disabled: bool,
}
impl ValueBindingPrelude {
    pub(crate) fn prepare(
        root: &PreparedSourceView,
        source: &PreparedSourceView,
        catalog: &CompileInputCatalog,
        scope: &ScopeKey,
        policies: (
            &crate::syntax::BindingPreludePolicies,
            &crate::syntax::BindingPreludePolicies,
        ),
    ) -> Result<Self, Diagnostic> {
        root.require_original_hygiene()?;
        source.require_original_hygiene()?;
        let owner = &root.selection().outline;
        let target = &source.selection().outline;
        if !owner.module_path.is_empty()
            || owner.crate_key != target.crate_key
            || owner.parse.profile != target.parse.profile
            || owner.cfg != target.cfg
            || scope.crate_key != target.crate_key
            || scope.owner.is_some()
            || !scope.module_path.starts_with(&target.module_path)
        {
            return Err(fault("Value prelude owning crate/module differs"));
        }
        let profile = &owner.parse.profile;
        let facet = catalog.value_binding_facet(profile)?;
        let compiler = catalog.owning_profile_identity()?;
        let policy = policies.0.for_scope(&owner.parse, owner.cfg, scope)?;
        let mut relative = scope.clone();
        relative.module_path = scope.module_path[target.module_path.len()..].to_vec();
        let local = policies.1.for_scope(&target.parse, target.cfg, &relative)?;
        Ok(Self {
            source: source.selection().clone(),
            policy_source: root.selection().clone(),
            scope: scope.clone(),
            compiler,
            facet,
            standard: policy.standard(),
            disabled: policy.disabled() || local.disabled(),
        })
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let policy_bytes = self.policy_source.metadata_bytes()?;
        std::mem::size_of::<Self>()
            .checked_add(self.source.metadata_bytes()?)
            .and_then(|n| n.checked_add(policy_bytes))
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .ok_or_else(|| fault("Value prelude receipt metadata overflow"))
    }
}
impl StableEncode for ValueBindingPrelude {
    const DOMAIN: &'static str = "lrr/v2/source-selected-value-binding-prelude";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.policy_source.path.as_str());
        e.record(&self.policy_source.outline);
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
