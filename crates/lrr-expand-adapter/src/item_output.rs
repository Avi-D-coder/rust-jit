//! Owned item syntax from the pinned token-tree parser, retaining every origin.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
use syntax::{
    AstNode,
    ast::{self, HasName},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ItemKind {
    Function,
    Import,
    MacroRules,
    Module,
    Struct,
    Enum,
    Trait,
    Impl,
    Type,
    Const,
    Static,
    Union,
    Other,
    ExternCrate,
}
impl ItemKind {
    pub fn tag(self) -> u32 {
        match self {
            Self::Function => 0,
            Self::Import => 1,
            Self::MacroRules => 2,
            Self::Module => 3,
            Self::Struct => 4,
            Self::Enum => 5,
            Self::Trait => 6,
            Self::Impl => 7,
            Self::Type => 8,
            Self::Const => 9,
            Self::Static => 10,
            Self::Union => 11,
            Self::Other => 12,
            Self::ExternCrate => 13,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Origin {
    pub start: u32,
    pub end: u32,
    pub span: Key,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Name {
    pub text: String,
    pub start: u32,
    pub end: u32,
    pub span: Key,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Item {
    pub ordinal: u32,
    pub structural_path: Vec<u32>,
    pub kind: ItemKind,
    pub name: Option<Name>,
    pub start: u32,
    pub end: u32,
    pub header_start: u32,
    pub header_end: u32,
}
#[derive(Debug, Clone)]
pub enum SyntaxEvent {
    Start(u16),
    Token { kind: u16, text: String },
    Finish,
}
#[derive(Debug, Clone)]
pub struct MacroBody {
    pub ordinal: u32,
    pub edition: u16,
    pub tokens: TokenForest,
}
#[derive(Debug, Clone)]
pub struct MacroCall {
    pub ordinal: u32,
    pub path: TokenForest,
    pub input: TokenForest,
    pub site: Key,
    pub edition: u16,
}
#[derive(Debug)]
pub struct Items {
    pub text: String,
    pub origins: Vec<Origin>,
    pub items: Vec<Item>,
    pub syntax: Vec<SyntaxEvent>,
    pub macros: Vec<MacroBody>,
    pub calls: Vec<MacroCall>,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
// Traverse only declaration module containers here. Body/closure/const producer
// discovery belongs to the separate declaration scanner, never eager lowering.
fn queue_items(
    container: &syntax::SyntaxNode,
    parent: &[u32],
    pending: &mut Vec<(ast::Item, Vec<u32>)>,
    pending_bytes: &mut usize,
) -> Result<(), AdapterError> {
    if parent.len() >= 256 {
        return Err(fault("generated module depth limit"));
    }
    let begin = pending.len();
    for (ordinal, item) in container.children().filter_map(ast::Item::cast).enumerate() {
        if pending.len() >= 1_000_000 {
            return Err(fault("generated pending declaration count limit"));
        }
        let length = parent
            .len()
            .checked_add(1)
            .ok_or_else(|| fault("generated item path overflow"))?;
        let charge = length
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|n| n.checked_add(96))
            .ok_or_else(|| fault("generated pending metadata overflow"))?;
        let total = pending_bytes
            .checked_add(charge)
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("generated pending declaration metadata limit"))?;
        let mut path = Vec::new();
        path.try_reserve_exact(length)
            .map_err(|_| fault("generated item path allocation"))?;
        path.extend_from_slice(parent);
        path.push(u32::try_from(ordinal).map_err(|_| fault("generated item ordinal overflow"))?);
        pending
            .try_reserve(1)
            .map_err(|_| fault("generated pending declaration allocation"))?;
        pending.push((item, path));
        *pending_bytes = total;
    }
    pending[begin..].reverse();
    Ok(())
}
pub fn materialize(input: &TokenForest) -> Result<Items, AdapterError> {
    if input.encoded_len()? > 32 * 1024 * 1024 || input.allocated_bytes()? > 64 * 1024 * 1024 {
        return Err(fault("item output input budget"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) = syntax_bridge::token_tree_to_syntax_node(
        &tree,
        parser::TopEntryPoint::MacroItems,
        &mut |context| context.edition(store.database()),
    );
    if let Some(error) = parsed.errors().first() {
        return Err(fault(&format!("invalid Items expansion: {error}")));
    }
    let root = parsed.syntax_node();
    let length = u32::from(root.text_range().len());
    if length > 32 * 1024 * 1024 {
        return Err(fault("item syntax text limit"));
    }
    let count = map.iter().count();
    if count > 2_000_000 {
        return Err(fault("item origin count limit"));
    }
    let mut origins = Vec::new();
    origins
        .try_reserve_exact(count)
        .map_err(|_| fault("item origin allocation"))?;
    let mut start = 0;
    for (end, span) in map.iter() {
        let end = u32::from(end);
        if start >= end || end > length {
            return Err(fault("item origin range ordering"));
        }
        origins.push(Origin {
            start,
            end,
            span: store.key(span)?,
        });
        start = end;
    }
    let text = root.text().to_string();
    let mut items = Vec::new();
    let mut macros = Vec::new();
    let mut calls = Vec::new();
    let mut macro_bytes = 0usize;
    let mut pending = Vec::new();
    let mut pending_bytes = 0usize;
    let mut path_bytes = 0usize;
    queue_items(&root, &[], &mut pending, &mut pending_bytes)?;
    while let Some((item, structural_path)) = pending.pop() {
        let charge = structural_path
            .len()
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|n| n.checked_add(96))
            .ok_or_else(|| fault("generated pending metadata overflow"))?;
        pending_bytes = pending_bytes
            .checked_sub(charge)
            .ok_or_else(|| fault("generated pending metadata underflow"))?;
        path_bytes = path_bytes
            .checked_add(charge)
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("generated item path metadata limit"))?;
        if let ast::Item::Module(module) = &item
            && let Some(body) = module.item_list()
        {
            queue_items(
                body.syntax(),
                &structural_path,
                &mut pending,
                &mut pending_bytes,
            )?;
        }
        let associated = match &item {
            ast::Item::Impl(value) => value.assoc_item_list(),
            ast::Item::Trait(value) => value.assoc_item_list(),
            _ => None,
        };
        if let Some(body) = associated {
            queue_items(
                body.syntax(),
                &structural_path,
                &mut pending,
                &mut pending_bytes,
            )?;
        }
        if items.len() >= 1_000_000 {
            return Err(fault("generated header count limit"));
        }
        let r = item.syntax().text_range();
        let mut header_end = u32::from(r.end());
        let (kind, name) = match &item {
            ast::Item::Fn(i) => {
                if let Some(body) = i.body() {
                    header_end = u32::from(body.syntax().text_range().start());
                }
                (ItemKind::Function, i.name())
            }
            ast::Item::Use(_) => (ItemKind::Import, None),
            ast::Item::ExternCrate(i) => (ItemKind::ExternCrate, i.rename().and_then(|r| r.name())),
            ast::Item::MacroRules(i) => (ItemKind::MacroRules, i.name()),
            ast::Item::Module(i) => {
                if let Some(body) = i.item_list() {
                    header_end = u32::from(body.syntax().text_range().start());
                }
                (ItemKind::Module, i.name())
            }
            ast::Item::Struct(i) => (ItemKind::Struct, i.name()),
            ast::Item::Enum(i) => (ItemKind::Enum, i.name()),
            ast::Item::Trait(i) => {
                if let Some(body) = i.assoc_item_list() {
                    header_end = u32::from(body.syntax().text_range().start());
                }
                (ItemKind::Trait, i.name())
            }
            ast::Item::Impl(i) => {
                if let Some(body) = i.assoc_item_list() {
                    header_end = u32::from(body.syntax().text_range().start());
                }
                (ItemKind::Impl, None)
            }
            ast::Item::TypeAlias(i) => (ItemKind::Type, i.name()),
            ast::Item::Const(i) => (ItemKind::Const, i.name()),
            ast::Item::Static(i) => (ItemKind::Static, i.name()),
            ast::Item::Union(i) => (ItemKind::Union, i.name()),
            _ => (ItemKind::Other, None),
        };
        let name = name
            .map(|name| {
                let r = name.syntax().text_range();
                Ok(Name {
                    text: name.text().to_string(),
                    start: u32::from(r.start()),
                    end: u32::from(r.end()),
                    span: store.key(map.span_at(r.start()))?,
                })
            })
            .transpose()?;
        let name = match (&item, name) {
            (ast::Item::ExternCrate(i), None) => {
                let name = i
                    .name_ref()
                    .ok_or_else(|| fault("extern crate name absent"))?;
                let r = name.syntax().text_range();
                Some(Name {
                    text: name.text().to_string(),
                    start: u32::from(r.start()),
                    end: u32::from(r.end()),
                    span: store.key(map.span_at(r.start()))?,
                })
            }
            (_, name) => name,
        };
        let ordinal =
            u32::try_from(items.len()).map_err(|_| fault("generated header ordinal overflow"))?;
        if let ast::Item::MacroRules(declaration) = &item {
            let body = declaration
                .token_tree()
                .ok_or_else(|| fault("macro body absent"))?;
            let keyword = declaration
                .macro_rules_token()
                .ok_or_else(|| fault("macro keyword absent"))?;
            let source = map.span_at(keyword.text_range().start());
            let edition = source.ctx.edition(store.database()).number() as u16;
            let converted = syntax_bridge::syntax_node_to_token_tree(
                body.syntax(),
                &map,
                source,
                syntax_bridge::DocCommentDesugarMode::Mbe,
            );
            let exported = store.export(&converted)?;
            let tokens = match exported.roots() {
                [
                    crate::token_wire::TokenTree::Group {
                        delimiter: crate::token_wire::Delimiter::Invisible,
                        children,
                        ..
                    },
                ] if children.len() == 1 => TokenForest::new(
                    vec![children[0].clone()],
                    exported.contexts().to_vec(),
                    exported.spans().to_vec(),
                )?,
                _ => exported,
            };
            macro_bytes = macro_bytes
                .checked_add(tokens.encoded_len()?)
                .filter(|n| *n <= 256 * 1024 * 1024)
                .ok_or_else(|| fault("generated macro body budget"))?;
            macros
                .try_reserve(1)
                .map_err(|_| fault("generated macro body allocation"))?;
            macros.push(MacroBody {
                ordinal,
                edition,
                tokens,
            });
        }
        if let ast::Item::MacroCall(call) = &item {
            if calls.len() >= 100_000 {
                return Err(fault("generated call count limit"));
            }
            let path = call
                .path()
                .ok_or_else(|| fault("generated call path absent"))?;
            let arguments = call
                .token_tree()
                .ok_or_else(|| fault("generated call arguments absent"))?;
            let source = map.span_at(path.syntax().text_range().start());
            let path = store.export(&syntax_bridge::syntax_node_to_token_tree(
                path.syntax(),
                &map,
                source,
                syntax_bridge::DocCommentDesugarMode::Mbe,
            ))?;
            let arguments = store.export(&syntax_bridge::syntax_node_to_token_tree(
                arguments.syntax(),
                &map,
                source,
                syntax_bridge::DocCommentDesugarMode::Mbe,
            ))?;
            let input = match arguments.roots() {
                [
                    crate::token_wire::TokenTree::Group {
                        delimiter: crate::token_wire::Delimiter::Invisible,
                        children,
                        ..
                    },
                ] if children.len() == 1 => TokenForest::new(
                    vec![children[0].clone()],
                    arguments.contexts().to_vec(),
                    arguments.spans().to_vec(),
                )?,
                _ => arguments,
            };
            let [crate::token_wire::TokenTree::Group { open, .. }] = input.roots() else {
                return Err(fault("generated call argument group absent"));
            };
            let site = *open;
            let input_bytes = input.encoded_len()?;
            macro_bytes = macro_bytes
                .checked_add(path.encoded_len()?)
                .and_then(|n| n.checked_add(input_bytes))
                .filter(|n| *n <= 256 * 1024 * 1024)
                .ok_or_else(|| fault("generated call/body token retention limit"))?;
            calls
                .try_reserve(1)
                .map_err(|_| fault("generated call allocation"))?;
            calls.push(MacroCall {
                ordinal,
                path,
                input,
                site,
                edition: source.ctx.edition(store.database()).number() as u16,
            });
        }
        items
            .try_reserve(1)
            .map_err(|_| fault("generated header allocation"))?;
        items.push(Item {
            ordinal,
            structural_path,
            kind,
            name,
            start: u32::from(r.start()),
            end: u32::from(r.end()),
            header_start: u32::from(r.start()),
            header_end,
        });
    }
    let syntax = owned_syntax(&root)?;
    Ok(Items {
        text,
        origins,
        items,
        syntax,
        macros,
        calls,
    })
}

pub(crate) fn owned_syntax(root: &syntax::SyntaxNode) -> Result<Vec<SyntaxEvent>, AdapterError> {
    let length = u32::from(root.text_range().len());
    let event_count = root.preorder_with_tokens().count();
    if event_count > 6_000_000
        || event_count
            .checked_mul(48)
            .and_then(|n| n.checked_add(length as usize))
            .is_none_or(|n| n > 256 * 1024 * 1024)
    {
        return Err(fault("owned generated syntax metadata limit"));
    }
    let mut syntax = Vec::new();
    syntax
        .try_reserve_exact(event_count)
        .map_err(|_| fault("owned syntax allocation"))?;
    for event in root.preorder_with_tokens() {
        match event {
            syntax::WalkEvent::Enter(syntax::NodeOrToken::Node(n)) => {
                syntax.push(SyntaxEvent::Start(u16::from(n.kind())))
            }
            syntax::WalkEvent::Enter(syntax::NodeOrToken::Token(t)) => {
                syntax.push(SyntaxEvent::Token {
                    kind: u16::from(t.kind()),
                    text: t.text().to_string(),
                })
            }
            syntax::WalkEvent::Leave(syntax::NodeOrToken::Node(_)) => {
                syntax.push(SyntaxEvent::Finish)
            }
            syntax::WalkEvent::Leave(syntax::NodeOrToken::Token(_)) => {}
        }
    }
    Ok(syntax)
}
