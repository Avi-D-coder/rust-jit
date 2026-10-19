//! Derived declaration-only name preparation; no entered-body or runtime grants.
use super::*;
use crate::{
    db::seals::{Seal, SealDomain},
    frontend::{
        associated_declarations::CapturedDeclarationScope, impl_index::DeclarationMacroIndex,
        source_locals::cold_declarations::ColdDeclarationNames,
    },
};
impl Database {
    pub(crate) fn publish_cold_declaration_names(
        &mut self,
        scope: &CapturedDeclarationScope<'_>,
        parent: &DeclarationMacroIndex,
        seal: &Seal,
        completions: &BTreeMap<Digest, crate::frontend::impl_index::CompletedOriginalAnnotation>,
    ) -> Result<(), Diagnostic> {
        self.require_coordinator()?;
        if self.run_active || !matches!(seal.key().domain, SealDomain::ImplCatalog { .. }) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold declaration names require a completed source catalog",
            ));
        }
        scope.require_current(self)?;
        if !matches!(self.input(&InputKey::ExpansionEpoch).as_deref(),
            Some(InputValue::ExpansionEpoch(epoch)) if *epoch == seal.key().epoch)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold declaration names owning epoch differs",
            ));
        }
        let reusable = scope.facts().items.iter().find_map(|row| {
            match self
                .inputs
                .get(&InputKey::DeclarationNames(row.item.scope.clone()))
                .map(|input| input.value.as_ref())
            {
                Some(InputValue::DeclarationNames(input)) => Some(Arc::clone(input)),
                _ => None,
            }
        });
        let reusable = match reusable {
            Some(input) if input.reusable(scope, parent, seal, completions)? => Some(input),
            _ => None,
        };
        let prepared = if let Some(input) = reusable {
            input
        } else {
            let publication = self.revision.checked_add(1).ok_or_else(|| {
                fault(
                    "LRR-E-ID-EXHAUSTED",
                    "cold declaration name revision exhausted",
                )
            })?;
            ColdDeclarationNames::prepare(scope, parent, seal, publication, completions)?
        };
        let mut staged = BTreeMap::new();
        for scope in prepared.scopes() {
            let key = InputKey::DeclarationNames(scope.clone());
            let value = InputValue::DeclarationNames(Arc::clone(&prepared));
            let fingerprint = value.fingerprint();
            if self
                .inputs
                .get(&key)
                .is_some_and(|old| old.fingerprint == fingerprint)
            {
                continue;
            }
            staged.insert(
                key,
                InputRecord {
                    value: Arc::new(value),
                    fingerprint,
                },
            );
        }
        if staged.is_empty() {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or_else(|| {
            fault(
                "LRR-E-ID-EXHAUSTED",
                "cold declaration name revision exhausted",
            )
        })?;
        self.inputs.extend(staged);
        self.revision = revision;
        Ok(())
    }
}

#[cfg(test)]
impl Database {
    /// Revoke only this derived input to exercise observed absence. Restoration
    /// in the control uses actual SourceDiscovery publication, not raw injection.
    pub(crate) fn take_cold_declaration_names_for_control(
        &mut self,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Option<Arc<InputValue>> {
        let row = self
            .inputs
            .remove(&InputKey::DeclarationNames(scope.clone()))?;
        self.revision = self.revision.checked_add(1).expect("control revision");
        Some(row.value)
    }
}
