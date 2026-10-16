//! Owned scope/import view for a nested body; declaration bodies remain separate.
use super::source_macros::EnteredMacroIndex;
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::ScopeKey,
    syntax::{
        entered_macros::{BodyImport, EnteredMacroFacts},
        imports::{ImportKind, ImportSegment},
        outline::Visibility,
    },
};
use std::collections::BTreeMap;
pub(crate) struct Namespace {
    pub facts: EnteredMacroFacts,
    pub contexts: BTreeMap<u32, (usize, ScopeKey)>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn charge(bytes: &mut usize, size: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(size)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(|| error("nested namespace metadata budget exceeded"))?;
    Ok(())
}
fn import(bytes: &mut usize, row: &BodyImport) -> Result<(), Diagnostic> {
    charge(bytes, std::mem::size_of::<BodyImport>())?;
    for s in &row.source.scope.module_path {
        charge(
            bytes,
            s.len()
                .checked_add(std::mem::size_of::<String>())
                .ok_or_else(|| error("nested namespace path size overflow"))?,
        )?;
    }
    for s in &row.source.path {
        charge(bytes, std::mem::size_of::<ImportSegment>())?;
        if let ImportSegment::Name(n) = s {
            charge(bytes, n.len())?;
        }
    }
    if let ImportKind::Named { alias: Some(name) } = &row.source.kind {
        charge(bytes, name.len())?;
    }
    if let Visibility::Restricted(tokens) = &row.source.visibility {
        for t in tokens {
            charge(
                bytes,
                t.text
                    .len()
                    .checked_add(std::mem::size_of_val(t))
                    .ok_or_else(|| error("nested visibility size overflow"))?,
            )?;
        }
    }
    Ok(())
}
fn item(bytes: &mut usize, row: &crate::syntax::body_items::BodyItem) -> Result<(), Diagnostic> {
    charge(bytes, std::mem::size_of_val(row))?;
    charge(bytes, row.item.header.estimated_bytes())?;
    charge(bytes, row.item.constructor_field_bytes()?)?;
    charge(bytes, row.item.def_key.declared_name.len())?;
    charge(
        bytes,
        row.namespaces
            .len()
            .checked_mul(std::mem::size_of::<crate::source::manifest::Namespace>())
            .ok_or_else(|| error("nested item namespace size overflow"))?,
    )?;
    for name in row
        .item
        .scope
        .module_path
        .iter()
        .chain(&row.item.def_key.module_path)
    {
        charge(bytes, name.len() + std::mem::size_of::<String>())?;
    }
    Ok(())
}
impl Namespace {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.facts
            .metadata_bytes()?
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| n.checked_sub(std::mem::size_of::<EnteredMacroFacts>()))
            .and_then(|n| {
                n.checked_add(
                    self.contexts
                        .len()
                        .checked_mul(16 * std::mem::size_of::<(u32, (usize, ScopeKey))>() + 256)?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)
    }
    pub fn capture(
        parent: &EnteredMacroIndex<'_>,
        child: &EnteredMacroFacts,
        selected: u32,
    ) -> Result<Self, Diagnostic> {
        let source = parent
            .namespace
            .as_ref()
            .map(|n| &n.facts)
            .unwrap_or(&parent.facts);
        let mut facts = EnteredMacroFacts {
            bindings: vec![],
            items: vec![],
            imports: vec![],
            invocations: vec![],
            scopes: vec![],
            events: vec![],
            opaque: vec![],
        };
        let mut bytes = 0usize;
        charge(
            &mut bytes,
            child
                .scopes
                .len()
                .checked_mul(std::mem::size_of::<
                    crate::syntax::entered_macros::BodyMacroScope,
                >())
                .ok_or_else(|| error("nested scope size overflow"))?,
        )?;
        facts
            .scopes
            .try_reserve(child.scopes.len())
            .map_err(|_| error("nested scope allocation failed"))?;
        facts.scopes.extend_from_slice(&child.scopes);
        for row in &child.imports {
            import(&mut bytes, row)?;
            facts
                .imports
                .try_reserve(1)
                .map_err(|_| error("nested import allocation failed"))?;
            facts.imports.push(row.clone());
        }
        for row in &child.items {
            item(&mut bytes, row)?;
            facts
                .items
                .try_reserve(1)
                .map_err(|_| error("nested item allocation failed"))?;
            facts.items.push(row.clone());
        }
        for event in &child.events {
            if !matches!(
                event.kind,
                crate::syntax::entered_macros::BodyMacroEventKind::Producer { .. }
            ) {
                continue;
            }
            charge(&mut bytes, std::mem::size_of_val(event))?;
            facts
                .events
                .try_reserve(1)
                .map_err(|_| error("nested event allocation failed"))?;
            facts.events.push(event.clone());
        }
        let mut mapping = BTreeMap::new();
        let mut contexts = BTreeMap::new();
        let mut current = Some(selected);
        while let Some(old) = current {
            if mapping.contains_key(&old) || mapping.len() >= 256 {
                return Err(error("nested namespace scope ancestry limit exceeded"));
            }
            let source_scope = source
                .scopes
                .iter()
                .find(|s| s.ordinal == old)
                .ok_or_else(|| error("nested namespace parent scope missing"))?;
            let ordinal = u32::try_from(facts.scopes.len())
                .map_err(|_| error("nested scope ordinal overflow"))?;
            charge(
                &mut bytes,
                std::mem::size_of_val(source_scope) + std::mem::size_of::<(u32, usize, ScopeKey)>(),
            )?;
            let context = match parent.namespace.as_ref().and_then(|n| n.contexts.get(&old)) {
                Some((frame, key)) => (
                    frame
                        .checked_add(1)
                        .ok_or_else(|| error("nested namespace frame overflow"))?,
                    *key,
                ),
                None => (
                    0,
                    *parent
                        .keys
                        .get(&old)
                        .ok_or_else(|| error("nested namespace original scope missing"))?,
                ),
            };
            contexts.insert(ordinal, context);
            mapping.insert(old, ordinal);
            let mut scope = source_scope.clone();
            scope.ordinal = ordinal;
            scope.parent = None;
            facts
                .scopes
                .try_reserve(1)
                .map_err(|_| error("nested scope ancestry allocation failed"))?;
            facts.scopes.push(scope);
            current = source_scope.parent;
        }
        facts
            .scopes
            .first_mut()
            .ok_or_else(|| error("nested namespace root missing"))?
            .parent = mapping.get(&selected).copied();
        for (old, new) in &mapping {
            let source_scope = source
                .scopes
                .iter()
                .find(|s| s.ordinal == *old)
                .expect("captured source scope");
            facts.scopes[*new as usize].parent =
                source_scope.parent.and_then(|p| mapping.get(&p).copied());
        }
        for row in &source.items {
            if let Some(scope) = mapping.get(&row.scope) {
                if facts.items.len() >= 1_000_000 {
                    return Err(error("nested item count exceeded"));
                }
                item(&mut bytes, row)?;
                facts
                    .items
                    .try_reserve(1)
                    .map_err(|_| error("nested ancestor item allocation failed"))?;
                let mut row = row.clone();
                row.scope = *scope;
                facts.items.push(row);
            }
        }
        for event in &source.events {
            if !matches!(
                event.kind,
                crate::syntax::entered_macros::BodyMacroEventKind::Producer { .. }
            ) {
                continue;
            }
            if let Some(scope) = mapping.get(&event.scope) {
                if facts.events.len() >= 1_000_000 {
                    return Err(error("nested event count exceeded"));
                }
                charge(&mut bytes, std::mem::size_of_val(event))?;
                facts
                    .events
                    .try_reserve(1)
                    .map_err(|_| error("nested ancestor event allocation failed"))?;
                let mut event = event.clone();
                event.scope = *scope;
                facts.events.push(event);
            }
        }
        for row in &source.imports {
            if let Some(scope) = mapping.get(&row.scope) {
                if facts.imports.len() >= 100_000 {
                    return Err(error("nested namespace import count exceeded"));
                }
                import(&mut bytes, row)?;
                facts
                    .imports
                    .try_reserve(1)
                    .map_err(|_| error("nested ancestor import allocation failed"))?;
                let mut row = row.clone();
                row.scope = *scope;
                facts.imports.push(row);
            }
        }
        Ok(Self { facts, contexts })
    }
}
