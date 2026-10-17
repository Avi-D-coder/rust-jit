//! Item-local helper admission from resolved, identity-bound derive declarations.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
};
use std::collections::BTreeSet;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
#[derive(Debug, Clone)]
pub struct DeriveHelperBinding {
    invocation: ExpansionKey,
    declaration: Digest,
    helpers: BTreeSet<String>,
}
impl DeriveHelperBinding {
    /// Metadata must come from the exact prepared export declaration, not its spelling.
    pub fn new(
        invocation: ExpansionKey,
        declaration: Digest,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, Diagnostic> {
        invocation.validate()?;
        if invocation.grammar != GrammarSlot::Items
            || !matches!(
                invocation.macro_identity,
                MacroIdentity::Procedural {
                    kind: ProceduralKind::Derive,
                    ..
                }
            )
        {
            return Err(fault("helper declaration requires an item derive identity"));
        }
        Self::from_names(invocation, declaration, names)
    }
    /// Only an exact compiler-owned descriptor can grant builtin helper metadata.
    pub fn builtin(
        invocation: ExpansionKey,
        descriptor: &lrr_expand_adapter::builtin_derives::BuiltinDeriveDescriptor,
    ) -> Result<Self, Diagnostic> {
        invocation.validate()?;
        if invocation.grammar != GrammarSlot::Items
            || invocation.macro_identity
                != (MacroIdentity::Builtin {
                    definition: Digest(descriptor.definition_identity().0),
                })
            || invocation.engine_version != Digest(descriptor.engine_identity().0)
        {
            return Err(fault(
                "builtin derive helper metadata identity/engine mismatch",
            ));
        }
        Self::from_names(
            invocation,
            Digest(descriptor.definition_identity().0),
            descriptor.helpers().iter().map(|name| (*name).to_owned()),
        )
    }
    fn from_names(
        invocation: ExpansionKey,
        declaration: Digest,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, Diagnostic> {
        let mut helpers = BTreeSet::new();
        let mut bytes = 0usize;
        let mut count = 0usize;
        for name in names {
            count = count
                .checked_add(1)
                .ok_or_else(|| fault("derive helper count overflow"))?;
            let name = crate::syntax::ast_adapter::macro_name(&name, invocation.definition_edition)
                .ok_or_else(|| fault("invalid derive helper identifier"))?;
            bytes = bytes
                .checked_add(name.len() + 64)
                .ok_or_else(|| fault("derive helper metadata limit"))?;
            helpers.insert(name);
        }
        Ok(Self {
            invocation,
            declaration,
            helpers,
        })
    }
    pub fn invocation(&self) -> &ExpansionKey {
        &self.invocation
    }
    pub fn helpers(&self) -> &BTreeSet<String> {
        &self.helpers
    }
}
impl StableEncode for DeriveHelperBinding {
    const DOMAIN: &'static str = "lrr/v2/derive-helper-binding";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.invocation);
        e.digest(self.declaration);
        e.u64(self.helpers.len() as u64);
        for name in &self.helpers {
            e.string(name);
        }
    }
}
#[derive(Debug, Clone)]
pub struct ItemHelperScope {
    owner: Digest,
    cfg: Digest,
    bindings: Vec<DeriveHelperBinding>,
}
#[derive(Debug, Clone)]
pub enum HelperLookup {
    Ready(Vec<ExpansionKey>),
    NeedDeclarations(Vec<ExpansionKey>),
}
impl ItemHelperScope {
    pub(crate) fn context_matches(&self, owner: Digest, cfg: Digest) -> bool {
        self.owner == owner && self.cfg == cfg
    }
    pub fn new(
        owner: Digest,
        cfg: Digest,
        bindings: Vec<DeriveHelperBinding>,
    ) -> Result<Self, Diagnostic> {
        let mut seen = BTreeSet::new();
        let mut bytes = 0usize;
        for binding in &bindings {
            bytes = bytes
                .checked_add(std::mem::size_of::<DeriveHelperBinding>())
                .and_then(|n| {
                    n.checked_add(crate::expand::record::key_heap_bytes(&binding.invocation))
                })
                .ok_or_else(|| fault("item helper metadata limit"))?;
            if !seen.insert(binding.invocation.invocation) {
                return Err(fault("duplicate derive invocation in item scope"));
            }
            for name in &binding.helpers {
                bytes = bytes
                    .checked_add(name.capacity() + 64)
                    .ok_or_else(|| fault("item helper metadata limit"))?;
            }
        }
        Ok(Self {
            owner,
            cfg,
            bindings,
        })
    }
    /// The owning ADT also governs its fields/variants; never use a sibling owner.
    /// Only currently active exact derives grant helpers. No macro is invoked here.
    pub fn helper_owners(
        &self,
        owner: Digest,
        cfg: Digest,
        active: &[ExpansionKey],
        name: &str,
    ) -> Result<Vec<ExpansionKey>, Diagnostic> {
        match self.lookup_helpers(owner, cfg, active, name)? {
            HelperLookup::Ready(owners) => Ok(owners),
            HelperLookup::NeedDeclarations(_) => Err(fault(
                "helper declarations are incomplete for active derives",
            )),
        }
    }
    pub fn lookup_helpers(
        &self,
        owner: Digest,
        cfg: Digest,
        active: &[ExpansionKey],
        name: &str,
    ) -> Result<HelperLookup, Diagnostic> {
        if owner != self.owner || cfg != self.cfg {
            return Err(fault("derive helper scope owner/cfg mismatch"));
        }
        let mut missing = Vec::new();
        let mut missing_bytes = 0usize;
        for invocation in active {
            invocation.validate()?;
            if invocation.grammar != GrammarSlot::Items
                || !matches!(
                    invocation.macro_identity,
                    MacroIdentity::Procedural {
                        kind: ProceduralKind::Derive,
                        ..
                    } | MacroIdentity::Builtin { .. }
                        | MacroIdentity::BuiltinDerive { .. }
                )
            {
                return Err(fault(
                    "active helper scope contains a non-derive invocation",
                ));
            }
            if !self
                .bindings
                .iter()
                .any(|binding| &binding.invocation == invocation)
                && !missing.contains(invocation)
            {
                missing_bytes = missing_bytes
                    .checked_add(std::mem::size_of::<ExpansionKey>())
                    .and_then(|n| n.checked_add(crate::expand::record::key_heap_bytes(invocation)))
                    .ok_or_else(|| fault("missing helper declaration metadata limit"))?;
                missing.push(invocation.clone());
            }
        }
        if !missing.is_empty() {
            return Ok(HelperLookup::NeedDeclarations(missing));
        }
        let mut owners = Vec::new();
        for binding in &self.bindings {
            if active.contains(&binding.invocation) {
                let canonical =
                    crate::syntax::ast_adapter::macro_name(name, binding.invocation.call_edition)
                        .ok_or_else(|| fault("invalid helper attribute identifier"))?;
                if binding.helpers.contains(&canonical) {
                    owners.push(binding.invocation.clone());
                }
            }
        }
        Ok(HelperLookup::Ready(owners))
    }
}
impl StableEncode for ItemHelperScope {
    const DOMAIN: &'static str = "lrr/v2/item-helper-scope";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.owner);
        e.digest(self.cfg);
        e.sequence(&self.bindings, |e, b| e.record(b));
    }
}
