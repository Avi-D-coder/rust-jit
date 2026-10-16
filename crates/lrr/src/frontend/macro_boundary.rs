//! Immutable roots of an actual selected textual macro boundary. This is not a
//! second resolver: consumers pass these roots to MacroDeclarations as outers.
use super::{
    hygiene::HygieneFacts,
    macro_origins::ExpansionAncestry,
    macro_scopes::{MacroDeclarations, OuterMacroScope},
};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::{ScopeKey, TextualScopes},
};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RetainedMacroScope {
    scopes: Arc<TextualScopes>,
    declarations: Arc<MacroDeclarations>,
    facts: Arc<HygieneFacts>,
    ancestry: Arc<ExpansionAncestry>,
    scope: ScopeKey,
    before: u64,
}
impl RetainedMacroScope {
    pub(super) fn new(
        scopes: Arc<TextualScopes>,
        declarations: Arc<MacroDeclarations>,
        facts: Arc<HygieneFacts>,
        ancestry: Arc<ExpansionAncestry>,
        scope: ScopeKey,
        before: u64,
    ) -> Result<Self, Diagnostic> {
        // This checks that the exact retained root actually owns the selected
        // rib. It grants no name, source origin, or producer completion.
        scopes.parent(scope)?;
        Ok(Self {
            scopes,
            declarations,
            facts,
            ancestry,
            scope,
            before,
        })
    }
    pub(crate) fn outer(&self) -> OuterMacroScope<'_> {
        OuterMacroScope {
            scopes: &self.scopes,
            declarations: &self.declarations,
            scope: self.scope,
            before: self.before,
        }
    }
    pub(crate) fn at(&self, scope: ScopeKey, before: u64) -> Result<Self, Diagnostic> {
        Self::new(
            Arc::clone(&self.scopes),
            Arc::clone(&self.declarations),
            Arc::clone(&self.facts),
            Arc::clone(&self.ancestry),
            scope,
            before,
        )
    }
    pub(crate) fn scopes(&self) -> &Arc<TextualScopes> {
        &self.scopes
    }
    pub(crate) fn declarations(&self) -> &Arc<MacroDeclarations> {
        &self.declarations
    }
    pub(crate) fn facts(&self) -> &Arc<HygieneFacts> {
        &self.facts
    }
    pub(crate) fn ancestry(&self) -> &Arc<ExpansionAncestry> {
        &self.ancestry
    }
    pub(crate) fn shared_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.scopes
            .metadata_bytes()?
            .checked_add(self.declarations.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.facts.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(self.ancestry.metadata_bytes().ok()?))
            .ok_or_else(crate::runtime::arena::resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::digest::Digest,
        expand::scopes::{MacroDefinition, MacroName, ScopeLookup},
    };
    #[test]
    fn immutable_scope_roots_survive_owner_copy_on_write() {
        let unit = Digest([234; 32]);
        let scope = ScopeKey::root(unit);
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope).unwrap();
        let mut scopes = Arc::new(scopes);
        let (context, facts) = crate::syntax::floating_fixture_hygiene::root(unit, 2024);
        let retained = RetainedMacroScope::new(
            Arc::clone(&scopes),
            Arc::new(MacroDeclarations::default()),
            Arc::new(facts),
            Arc::new(ExpansionAncestry::default()),
            scope,
            10,
        )
        .unwrap();
        assert!(Arc::ptr_eq(retained.scopes(), &scopes));
        let name = MacroName::parse("after", 2024).unwrap();
        Arc::make_mut(&mut scopes)
            .define(
                scope,
                1,
                name.clone(),
                MacroDefinition {
                    identity: Digest([235; 32]),
                    defining_unit: unit,
                    definition_edition: 2024,
                },
            )
            .unwrap();
        assert!(!Arc::ptr_eq(retained.scopes(), &scopes));
        assert_eq!(
            retained.outer().scopes.lookup(scope, 10, &name).unwrap(),
            ScopeLookup::Absent
        );
        assert!(matches!(
            scopes.lookup(scope, 10, &name).unwrap(),
            ScopeLookup::Defined(_)
        ));
        retained.facts().context(context).unwrap();
        assert!(retained.shared_metadata_bytes().unwrap() > 0);
    }
}
