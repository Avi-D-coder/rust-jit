//! Final textual selection joins source order, pinned owned name classes and
//! explicit declaration origins. It never infers an origin from a token mark.
use super::{
    hygiene::HygieneFacts,
    macro_origins::{ExpansionAncestry, MacroCandidate, MacroSelection},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::{MacroDefinition, MacroName, ScopeCandidates, ScopeKey, TextualScopes},
    syntax::token_wire::Key,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq)]
struct Declaration {
    definition: MacroDefinition,
    context: Key,
    origin: Option<Key>,
}
#[derive(Default, Clone)]
pub struct MacroDeclarations {
    declarations: BTreeMap<Digest, Declaration>,
}
pub struct MacroReference<'a> {
    pub scope: ScopeKey,
    pub before: u64,
    pub name: &'a MacroName,
    pub context: Key,
    pub origin: Option<Key>,
}
#[derive(Clone, Copy)]
pub struct OuterMacroScope<'a> {
    pub scopes: &'a TextualScopes,
    pub scope: ScopeKey,
    pub before: u64,
    pub declarations: &'a MacroDeclarations,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HygienicLookup {
    Selection(MacroSelection),
    NeedInput(Digest),
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl MacroDeclarations {
    /// Encode retained declaration facts without recomputing hygiene normal forms.
    pub(crate) fn semantic_fingerprint(&self) -> Result<Digest, Diagnostic> {
        let mut e = crate::core::digest::StableEncoder::new("lrr/v1/macro-declaration-environment");
        e.u64(self.declarations.len() as u64);
        for (identity, row) in &self.declarations {
            e.digest(*identity);
            e.digest(row.definition.identity);
            e.digest(row.definition.defining_unit);
            e.u16(row.definition.definition_edition);
            e.digest(Digest(row.context.0));
            e.option(&row.origin, |e, origin| e.digest(Digest(origin.0)));
        }
        Ok(e.finish())
    }

    pub(crate) fn definition_by_identity(&self, identity: Digest) -> Option<&MacroDefinition> {
        self.declarations.get(&identity).map(|row| &row.definition)
    }
    pub(crate) fn origin_of(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<Option<Key>>, Diagnostic> {
        let Some(declaration) = self.declarations.get(&definition.identity) else {
            return Ok(None);
        };
        if declaration.definition != *definition {
            return Err(error("macro origin definition metadata differs"));
        }
        Ok(Some(declaration.origin))
    }
    pub(crate) fn metadata_of(
        &self,
        definition: &MacroDefinition,
    ) -> Result<(Key, Option<Key>), Diagnostic> {
        let declaration = self
            .declarations
            .get(&definition.identity)
            .ok_or_else(|| error("module macro declaration absent"))?;
        if declaration.definition != *definition {
            return Err(error("module macro declaration identity differs"));
        }
        Ok((declaration.context, declaration.origin))
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.declarations
            .len()
            .checked_mul(256)
            .ok_or_else(|| error("macro declaration metadata overflow"))
    }

    pub fn register(
        &mut self,
        definition: MacroDefinition,
        context: Key,
        origin: Option<Key>,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<(), Diagnostic> {
        self.preflight_register(&definition, context, origin, facts, ancestry)?;
        self.declarations
            .entry(definition.identity)
            .or_insert(Declaration {
                definition,
                context,
                origin,
            });
        Ok(())
    }
    pub(crate) fn preflight_register(
        &self,
        definition: &MacroDefinition,
        context: Key,
        origin: Option<Key>,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<(), Diagnostic> {
        definition.validate()?;
        facts.name_item(context)?;
        ancestry.descendant_of(origin, None)?;
        let id = definition.identity;
        let declaration = Declaration {
            definition: definition.clone(),
            context,
            origin,
        };
        if let Some(old) = self.declarations.get(&id) {
            return if old == &declaration {
                Ok(())
            } else {
                Err(error("conflicting macro declaration metadata"))
            };
        }
        Ok(())
    }
    pub(crate) fn copy_selected(
        &mut self,
        other: &Self,
        identities: &std::collections::BTreeSet<Digest>,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<(), Diagnostic> {
        self.declarations
            .len()
            .checked_add(identities.len())
            .ok_or_else(|| error("imported declaration count limit"))?;
        for identity in identities {
            let row = other
                .declarations
                .get(identity)
                .ok_or_else(|| error("imported declaration metadata absent"))?;
            self.register(
                row.definition.clone(),
                row.context,
                row.origin,
                facts,
                ancestry,
            )?;
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        scopes: &TextualScopes,
        reference: MacroReference<'_>,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_with_outer(scopes, reference, None, facts, ancestry)
    }
    pub fn resolve_with_outer(
        &self,
        scopes: &TextualScopes,
        reference: MacroReference<'_>,
        outer: Option<OuterMacroScope<'_>>,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_with_outers(scopes, reference, outer.as_slice(), facts, ancestry)
    }
    pub fn resolve_with_outers(
        &self,
        scopes: &TextualScopes,
        reference: MacroReference<'_>,
        outers: &[OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_with_outers_using(scopes, reference, outers, facts, ancestry, true)
    }
    /// Available candidates for pure discovery; this result cannot certify a scope.
    pub(crate) fn resolve_for_discovery(
        &self,
        scopes: &TextualScopes,
        reference: MacroReference<'_>,
        outers: &[OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_with_outers_using(scopes, reference, outers, facts, ancestry, false)
    }
    fn resolve_with_outers_using(
        &self,
        scopes: &TextualScopes,
        reference: MacroReference<'_>,
        outers: &[OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
        require_frontiers: bool,
    ) -> Result<HygienicLookup, Diagnostic> {
        let comparison = facts.name_item(reference.context)?;
        ancestry.descendant_of(reference.origin, None)?;
        let candidates = |scopes: &TextualScopes, scope, before| {
            if require_frontiers {
                scopes.candidates(scope, before, reference.name)
            } else {
                scopes.candidates_for_discovery(scope, before, reference.name)
            }
        };
        let mut definitions = match candidates(scopes, reference.scope, reference.before)? {
            ScopeCandidates::NeedInput(invocation) => {
                return Ok(HygienicLookup::NeedInput(invocation));
            }
            ScopeCandidates::Definitions(definitions) => definitions,
        };
        for outer in outers {
            match candidates(outer.scopes, outer.scope, outer.before)? {
                ScopeCandidates::NeedInput(invocation) => {
                    return Ok(HygienicLookup::NeedInput(invocation));
                }
                ScopeCandidates::Definitions(extra) => {
                    definitions
                        .len()
                        .checked_add(extra.len())
                        .ok_or_else(|| error("joined macro candidate limit exceeded"))?;
                    definitions
                        .try_reserve(extra.len())
                        .map_err(|_| error("joined macro candidate allocation failed"))?;
                    definitions.extend(extra);
                }
            }
        }
        let mut compatible = Vec::new();
        for definition in definitions {
            let mut declaration = self.declarations.get(&definition.identity);
            for inherited in outers
                .iter()
                .filter_map(|outer| outer.declarations.declarations.get(&definition.identity))
            {
                if declaration.is_some_and(|known| known != inherited) {
                    return Err(error("conflicting joined macro declaration metadata"));
                }
                declaration = Some(inherited);
            }
            let declaration =
                declaration.ok_or_else(|| error("macro declaration metadata input is missing"))?;
            if declaration.definition != definition {
                return Err(error("macro scope and declaration metadata disagree"));
            }
            // Validate origins even for incompatible names; malformed source
            // metadata must not become silently accepted absence.
            ancestry.descendant_of(declaration.origin, None)?;
            if facts.name_item(declaration.context)? == comparison {
                compatible
                    .try_reserve(1)
                    .map_err(|_| error("compatible macro candidate allocation failed"))?;
                compatible.push(MacroCandidate {
                    definition,
                    origin: declaration.origin,
                });
            }
        }
        Ok(HygienicLookup::Selection(
            ancestry.select_compatible(reference.origin, &compatible)?,
        ))
    }
}
