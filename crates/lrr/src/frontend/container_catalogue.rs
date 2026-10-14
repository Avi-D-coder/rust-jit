//! Explicit original container entry. This does not publish a local module world.
use super::source_macros::EnteredMacroIndex;
use crate::{
    core::diagnostic::Diagnostic,
    source::{
        changes::SourceSelection,
        manifest::{BodyKey, ItemKind},
    },
    syntax::{
        body_items::{BodyItem, ContainerKey},
        entered_macros::EnteredMacroFacts,
    },
};
pub struct ContainerCatalogue<'a> {
    entered: EnteredMacroIndex<'a>,
    key: ContainerKey,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl EnteredMacroIndex<'_> {
    pub fn enter_container(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        key: &ContainerKey,
    ) -> Result<ContainerCatalogue<'_>, Diagnostic> {
        if expected.canonical()? != *self.parent.source() || *body != self.body {
            return Err(error("container parent source/body mismatch"));
        }
        let item = self
            .facts()
            .items
            .iter()
            .find(|i| i.container.as_ref().is_some_and(|c| c.key == *key))
            .ok_or_else(|| error("container absent or changed"))?;
        ContainerCatalogue::from_item(self, item)
    }
}
impl<'a> ContainerCatalogue<'a> {
    fn from_item(parent: &'a EnteredMacroIndex<'a>, item: &BodyItem) -> Result<Self, Diagnostic> {
        let handle = item
            .container
            .as_ref()
            .ok_or_else(|| error("item has no container"))?;
        let mut scope = item.item.scope.clone();
        scope.owner = Some(handle.key.definition);
        if handle.key.kind == ItemKind::Module {
            scope
                .module_path
                .try_reserve(1)
                .map_err(|_| error("container module path allocation failed"))?;
            scope
                .module_path
                .push(item.item.def_key.declared_name.clone());
        }
        let mut context = Vec::new();
        if matches!(
            handle.key.kind,
            ItemKind::Trait | ItemKind::Impl | ItemKind::ExternBlock
        ) {
            let tokens = item
                .item
                .header
                .context
                .iter()
                .chain(&item.item.header.tokens);
            let mut bytes = 0usize;
            for token in tokens {
                bytes = bytes
                    .checked_add(token.text.len())
                    .and_then(|n| n.checked_add(std::mem::size_of_val(token)))
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("container context metadata exceeded"))?;
                context
                    .try_reserve(1)
                    .map_err(|_| error("container context allocation failed"))?;
                context.push(token.clone());
            }
        }
        let facts = parent
            .parent
            .view
            .retained_container_facts(handle, &scope, &context)?;
        let mut module_scope = parent.module_scope.clone();
        if handle.key.kind == ItemKind::Module {
            module_scope.module_path = scope.module_path.clone();
            module_scope.owner = None;
        }
        // Private owner key for shared lexical machinery; never a public executable handle.
        let owner = BodyKey {
            definition: handle.key.definition,
            exact_body_bytes: handle.key.content,
        };
        let entered = parent.child_from_facts(
            facts,
            item,
            &owner,
            module_scope,
            handle.key.kind != ItemKind::Module,
        )?;
        Ok(Self {
            entered,
            key: handle.key.clone(),
        })
    }
    pub fn key(&self) -> &ContainerKey {
        &self.key
    }
    pub fn facts(&self) -> &EnteredMacroFacts {
        self.entered.facts()
    }
    pub fn enter_container(
        &self,
        expected: &SourceSelection,
        parent_key: &ContainerKey,
        key: &ContainerKey,
    ) -> Result<ContainerCatalogue<'_>, Diagnostic> {
        if expected.canonical()? != *self.entered.parent.source() || *parent_key != self.key {
            return Err(error("nested container source/parent mismatch"));
        }
        let item = self
            .entered
            .facts()
            .items
            .iter()
            .find(|i| i.container.as_ref().is_some_and(|c| c.key == *key))
            .ok_or_else(|| error("nested container absent or changed"))?;
        ContainerCatalogue::from_item(&self.entered, item)
    }
    pub fn enter_body(
        &self,
        expected: &SourceSelection,
        parent_key: &ContainerKey,
        body: &BodyKey,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        if *parent_key != self.key {
            return Err(error("container executable parent mismatch"));
        }
        self.entered
            .enter_nested_body(expected, &self.entered.body, body)
    }
}
