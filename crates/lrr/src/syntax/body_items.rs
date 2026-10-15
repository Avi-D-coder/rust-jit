//! Owned original item headers in an explicitly entered lexical body scope.
use super::{
    ast_adapter::{TokenRecipe, range},
    outline::{AttributeRecipe, BodyHandle, HeaderRecipe, ItemOutline, ScopeKey},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::manifest::{DefKey, ItemKind, Namespace},
};
use ra_ap_syntax::{
    AstNode, SyntaxElement, SyntaxNode,
    ast::{self, HasName},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerKey {
    pub definition: Digest,
    pub content: Digest,
    pub kind: ItemKind,
}
impl StableEncode for ContainerKey {
    const DOMAIN: &'static str = "lrr/v2/item-container";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.content);
        e.u32(match self.kind {
            ItemKind::Module => 4,
            ItemKind::Trait => 8,
            ItemKind::Impl => 7,
            ItemKind::ExternBlock => 13,
            ItemKind::Macro => 9,
            ItemKind::Function => 1,
            ItemKind::Struct => 2,
            ItemKind::Enum => 3,
            ItemKind::Constant => 5,
            ItemKind::Static => 6,
            ItemKind::TypeAlias => 10,
            ItemKind::Import => 11,
            ItemKind::Union => 12,
        });
    }
}
#[derive(Debug, Clone)]
pub struct ContainerHandle {
    pub key: ContainerKey,
    pub range: SourceRange,
    pub entry_blocked: bool,
}
#[derive(Debug, Clone)]
pub struct BodyItem {
    pub scope: u32,
    pub item: ItemOutline,
    pub namespaces: Vec<Namespace>,
    /// Contents of modules/traits/impls/extern blocks/templates are separate entry demands.
    pub container_range: Option<SourceRange>,
    pub container: Option<ContainerHandle>,
    pub attribute_input: Option<Digest>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
pub(crate) struct BodyItemContext<'a> {
    pub parsed: &'a super::parse::ParsedFile,
    pub parent: &'a BodyHandle,
    pub containing: &'a ScopeKey,
    pub scope: u32,
    pub profile: &'a crate::core::profile::ProfileIdentity,
    pub cfg: &'a crate::source::snapshot::CfgSet,
    pub header_context: &'a [TokenRecipe],
}
#[derive(Default)]
pub(crate) struct BodyItemBudget {
    bytes: usize,
    ordinals: BTreeMap<(u32, Namespace, String), u32>,
}
impl BodyItemBudget {
    fn charge(&mut self, n: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(n)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("body item header metadata budget exceeded"))?;
        Ok(())
    }
    pub fn collect(
        &mut self,
        item: &ast::Item,
        context: BodyItemContext<'_>,
        attrs: &[AttributeRecipe],
        attribute_input: Option<Digest>,
    ) -> Result<Option<BodyItem>, Diagnostic> {
        let BodyItemContext {
            parsed,
            parent,
            containing,
            scope,
            profile,
            cfg,
            header_context,
        } = context;
        let node = item.syntax();
        let (kind, namespace) = match item {
            ast::Item::Fn(_) => (ItemKind::Function, Namespace::Value),
            ast::Item::Const(_) => (ItemKind::Constant, Namespace::Value),
            ast::Item::Static(_) => (ItemKind::Static, Namespace::Value),
            ast::Item::Struct(_) => (ItemKind::Struct, Namespace::Type),
            ast::Item::Union(_) => (ItemKind::Union, Namespace::Type),
            ast::Item::Enum(_) => (ItemKind::Enum, Namespace::Type),
            ast::Item::TypeAlias(_) => (ItemKind::TypeAlias, Namespace::Type),
            ast::Item::Module(_) | ast::Item::ExternCrate(_) => (ItemKind::Module, Namespace::Type),
            ast::Item::Trait(_) => (ItemKind::Trait, Namespace::Type),
            ast::Item::Impl(_) => (ItemKind::Impl, Namespace::Type),
            ast::Item::Use(_) => (ItemKind::Import, Namespace::Type),
            ast::Item::ExternBlock(_) => (ItemKind::ExternBlock, Namespace::Type),
            ast::Item::MacroRules(_) | ast::Item::MacroDef(_) => {
                (ItemKind::Macro, Namespace::Macro)
            }
            ast::Item::MacroCall(_) | ast::Item::AsmExpr(_) => return Ok(None),
        };
        let executable = match item {
            ast::Item::Fn(_) => node
                .children()
                .find_map(ast::BlockExpr::cast)
                .map(|n| n.syntax().clone()),
            ast::Item::Const(_) | ast::Item::Static(_) => node
                .children()
                .find_map(ast::Expr::cast)
                .map(|n| n.syntax().clone()),
            _ => None,
        };
        let container = match item {
            ast::Item::Module(n) => n.item_list().map(|n| n.syntax().clone()),
            ast::Item::Trait(n) => n.assoc_item_list().map(|n| n.syntax().clone()),
            ast::Item::Impl(n) => n.assoc_item_list().map(|n| n.syntax().clone()),
            ast::Item::ExternBlock(n) => n.extern_item_list().map(|n| n.syntax().clone()),
            ast::Item::MacroRules(n) => n.token_tree().map(|n| n.syntax().clone()),
            ast::Item::MacroDef(_) => node
                .children()
                .filter_map(ast::TokenTree::cast)
                .last()
                .map(|n| n.syntax().clone()),
            _ => None,
        };
        let raw_name = match item {
            ast::Item::ExternCrate(n) => n
                .rename()
                .and_then(|r| r.name())
                .map(|n| n.syntax().text().to_string())
                .or_else(|| n.name_ref().map(|n| n.syntax().text().to_string())),
            _ => node
                .children()
                .find_map(ast::Name::cast)
                .map(|n| n.syntax().text().to_string()),
        };
        let named = !matches!(
            kind,
            ItemKind::Impl | ItemKind::Import | ItemKind::ExternBlock
        );
        let name = match raw_name {
            Some(name) => super::ast_adapter::semantic_name(&name)
                .ok_or_else(|| error("body item declaration name limit exceeded"))?,
            None if named => return Err(error("body item declaration name boundary missing")),
            None => String::new(),
        };
        self.charge(std::mem::size_of::<BodyItem>())?;
        self.charge(name.len())?;
        for part in &containing.module_path {
            self.charge(
                part.len()
                    .checked_add(std::mem::size_of::<String>())
                    .ok_or_else(|| error("body item module size overflow"))?,
            )?;
        }
        for attr in attrs {
            self.charge(
                attr.name
                    .len()
                    .checked_add(std::mem::size_of::<AttributeRecipe>())
                    .ok_or_else(|| error("body item attribute size overflow"))?,
            )?;
            for t in &attr.tokens {
                self.charge(
                    t.text
                        .len()
                        .checked_add(std::mem::size_of::<TokenRecipe>())
                        .ok_or_else(|| error("body item attribute token size overflow"))?,
                )?;
            }
        }
        let excluded = executable
            .as_ref()
            .or(container.as_ref())
            .map(|n| n.text_range());
        let tokens = self.header_tokens(node, excluded)?;
        let header_span = SourceRange {
            start: u32::from(node.text_range().start()),
            end: excluded.map_or(u32::from(node.text_range().end()), |r| u32::from(r.start())),
        };
        for token in header_context {
            self.charge(
                token
                    .text
                    .len()
                    .checked_add(std::mem::size_of::<TokenRecipe>())
                    .ok_or_else(|| error("container header context size overflow"))?,
            )?;
        }
        let header = HeaderRecipe {
            profile: profile.clone(),
            tokens,
            context: header_context.to_vec(),
            attributes: attrs.to_vec(),
            visibility: super::outline::visibility(node),
        };
        let mut owner = StableEncoder::new("lrr/v2/body-item-scope");
        owner.digest(parent.def_key);
        owner.u32(scope);
        let owner = owner.finish();
        let enclosing = if kind == ItemKind::Impl {
            let mut e = StableEncoder::new("lrr/v2/body-impl-identity");
            e.digest(owner);
            e.record(&header);
            e.finish()
        } else {
            owner
        };
        let ordinal = self
            .ordinals
            .entry((scope, namespace, name.clone()))
            .or_default();
        let duplicate_ordinal = *ordinal;
        *ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| error("body item duplicate ordinal overflow"))?;
        let def_key = DefKey {
            crate_key: containing.crate_key,
            module_path: containing.module_path.clone(),
            namespace,
            item_kind: kind,
            declared_name: name,
            enclosing_impl: Some(enclosing),
            duplicate_ordinal,
        };
        let definition = def_key.fingerprint();
        let body = if let Some(n) = executable {
            Some(BodyHandle {
                def_key: definition,
                body_hash: parsed.body_content_identity(&n)?,
                range: range(n.text_range()),
                entry_blocked: attribute_input.is_some(),
            })
        } else {
            None
        };
        let mut namespaces = if named { vec![namespace] } else { vec![] };
        if let ast::Item::Struct(n) = item
            && (node
                .children()
                .any(|n| ast::TupleFieldList::can_cast(n.kind()))
                || n.semicolon_token().is_some()
                    && !node
                        .children()
                        .any(|n| ast::RecordFieldList::can_cast(n.kind())))
        {
            namespaces.push(Namespace::Value);
        }
        let container_handle = if let Some(n) = container.as_ref() {
            Some(ContainerHandle {
                key: ContainerKey {
                    definition,
                    content: parsed.body_content_identity(n)?,
                    kind,
                },
                range: range(n.text_range()),
                entry_blocked: attribute_input.is_some(),
            })
        } else {
            None
        };
        Ok(Some(BodyItem {
            scope,
            item: ItemOutline {
                def_key,
                scope: ScopeKey {
                    crate_key: containing.crate_key,
                    module_path: containing.module_path.clone(),
                    owner: Some(owner),
                },
                header,
                constructor_fields: super::outline::constructor_fields(item, cfg)?,
                body,
                declaration_span: range(node.text_range()),
                header_span,
            },
            namespaces,
            container_range: container.map(|n| range(n.text_range())),
            container: container_handle,
            attribute_input,
        }))
    }
    fn header_tokens(
        &mut self,
        node: &SyntaxNode,
        excluded: Option<ra_ap_syntax::TextRange>,
    ) -> Result<Vec<TokenRecipe>, Diagnostic> {
        let mut result = Vec::new();
        let mut stack = vec![SyntaxElement::Node(node.clone())];
        let mut visited = 0usize;
        while let Some(element) = stack.pop() {
            if excluded.is_some_and(|r| r.contains_range(element.text_range())) {
                continue;
            }
            visited = visited
                .checked_add(1)
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("body item header syntax limit exceeded"))?;
            match element {
                SyntaxElement::Node(n) => {
                    let count = n.children_with_tokens().count();
                    stack
                        .len()
                        .checked_add(count)
                        .filter(|n| *n <= 1_000_000)
                        .ok_or_else(|| error("body item header stack limit exceeded"))?;
                    let mut children = Vec::new();
                    children
                        .try_reserve(count)
                        .map_err(|_| error("body item child allocation failed"))?;
                    children.extend(n.children_with_tokens());
                    stack
                        .try_reserve(children.len())
                        .map_err(|_| error("body item header stack allocation failed"))?;
                    stack.extend(children.into_iter().rev());
                }
                SyntaxElement::Token(t) if !t.kind().is_trivia() => {
                    self.charge(
                        t.text()
                            .len()
                            .checked_add(std::mem::size_of::<TokenRecipe>())
                            .ok_or_else(|| error("body item token size overflow"))?,
                    )?;
                    result
                        .try_reserve(1)
                        .map_err(|_| error("body item token allocation failed"))?;
                    result.push(TokenRecipe {
                        kind: t.kind() as u16,
                        text: t.text().to_owned(),
                    });
                }
                _ => {}
            }
        }
        Ok(result)
    }
}
