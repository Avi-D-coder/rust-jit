//! Pinned grammar-specific syntax transport for nested declaration discovery.
use crate::{
    AdapterError,
    item_output::{Origin, SyntaxEvent, owned_syntax},
    span_store::SpanStore,
    token_wire::{Delimiter, Key, TokenForest, TokenTree},
};
use syntax::{
    AstNode,
    ast::{self, HasName},
};

#[derive(Clone, Copy, Debug)]
pub enum DeclarationGrammar {
    Items,
    Expression,
    Statements,
    Type,
    Pattern,
    TraitItems,
    ImplItems,
}
impl DeclarationGrammar {
    pub(crate) fn entry_point(self) -> parser::TopEntryPoint {
        match self {
            DeclarationGrammar::Items
            | DeclarationGrammar::TraitItems
            | DeclarationGrammar::ImplItems => parser::TopEntryPoint::MacroItems,
            DeclarationGrammar::Expression => parser::TopEntryPoint::Expr,
            DeclarationGrammar::Statements => parser::TopEntryPoint::MacroStmts,
            DeclarationGrammar::Type => parser::TopEntryPoint::Type,
            DeclarationGrammar::Pattern => parser::TopEntryPoint::Pattern,
        }
    }
}
pub struct DeclarationSyntax {
    pub text: String,
    pub origins: Vec<Origin>,
    pub syntax: Vec<SyntaxEvent>,
    pub definitions: Vec<DefinitionCapture>,
    pub calls: Vec<CallCapture>,
}
pub struct DefinitionCapture {
    pub start: u32,
    pub end: u32,
    pub name_span: Key,
    pub body: TokenForest,
    pub edition: u16,
}
pub struct CallCapture {
    pub start: u32,
    pub end: u32,
    /// Exact pinned span-map key of the final path identifier, distinct from
    /// the input group's opening delimiter used for expansion admission.
    pub name_span: Key,
    pub path: TokenForest,
    pub input: TokenForest,
    pub site: Key,
    pub edition: u16,
}
fn group(
    node: &syntax::SyntaxNode,
    map: &span::SpanMap,
    source: span::Span,
    store: &SpanStore,
) -> Result<TokenForest, AdapterError> {
    let forest = store.export(&syntax_bridge::syntax_node_to_token_tree(
        node,
        map,
        source,
        syntax_bridge::DocCommentDesugarMode::Mbe,
    ))?;
    match forest.roots() {
        [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] if children.len() == 1 => forest.with_roots(vec![children[0].clone()]),
        _ => Ok(forest),
    }
}
fn error(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
pub fn materialize(
    input: &TokenForest,
    grammar: DeclarationGrammar,
) -> Result<DeclarationSyntax, AdapterError> {
    let mut store = SpanStore::default();
    let (root, map) = parse_root(input, grammar, &mut store)?;
    materialize_root(&root, &map, &store)
}

/// Retain header facts from the same admitted pinned tree, without parsing a
/// second time or altering the ordinary declaration/snapshot representation.
pub fn materialize_with_headers(
    input: &TokenForest,
    grammar: DeclarationGrammar,
) -> Result<(DeclarationSyntax, Vec<crate::item_output::Item>), AdapterError> {
    let mut store = SpanStore::default();
    let (root, map) = parse_root(input, grammar, &mut store)?;
    let headers = crate::item_output::capture_headers(&root, &map, &store)?;
    Ok((materialize_root(&root, &map, &store)?, headers))
}

pub(crate) fn parse_root(
    input: &TokenForest,
    grammar: DeclarationGrammar,
    store: &mut SpanStore,
) -> Result<(syntax::SyntaxNode, span::SpanMap), AdapterError> {
    let entry = grammar.entry_point();
    let tree = store.import(input)?;
    let (parsed, map) = syntax_bridge::token_tree_to_syntax_node(&tree, entry, &mut |context| {
        context.edition(store.database())
    });
    if let Some(problem) = parsed.errors().first() {
        return Err(error(format!("invalid {grammar:?} expansion: {problem}")));
    }
    let root = parsed.syntax_node();
    crate::grammar::validate_complete_syntax(&root)?;
    // The pinned parser shares MacroItems for associated fragments. Preserve
    // that parser and require its actual associated-item node classes here.
    if matches!(
        grammar,
        DeclarationGrammar::TraitItems | DeclarationGrammar::ImplItems
    ) && root.children().any(|n| !ast::AssocItem::can_cast(n.kind()))
    {
        return Err(error(
            "non-associated declaration in associated expansion slot",
        ));
    }
    Ok((root, map))
}

/// Shared transport for a pinned parsed tree, including structurally attached
/// expansions. Do not round-trip an attached tree through text: doing so loses
/// fragment boundaries (and therefore expression precedence).
pub(crate) fn materialize_root(
    root: &syntax::SyntaxNode,
    map: &span::SpanMap,
    store: &SpanStore,
) -> Result<DeclarationSyntax, AdapterError> {
    let length = u32::from(root.text_range().len());
    let count = map.iter().count();
    let mut origins = Vec::new();
    origins
        .try_reserve_exact(count)
        .map_err(|_| error("declaration origin allocation"))?;
    let mut start = 0;
    for (end, span) in map.iter() {
        let end = u32::from(end);
        if start >= end || end > length {
            return Err(error("declaration origin range ordering"));
        }
        origins.push(Origin {
            start,
            end,
            span: store.key(span)?,
        });
        start = end;
    }
    if start != length {
        return Err(error("declaration origin coverage is incomplete"));
    }
    let syntax = owned_syntax(root)?;
    let Captures {
        definitions,
        calls,
    } = capture_nodes(root, map, store, None)?;
    Ok(DeclarationSyntax {
        text: root.text().to_string(),
        origins,
        syntax,
        definitions,
        calls,
    })
}

/// Definitions and calls captured by one structural walk.
pub(crate) struct Captures {
    pub(crate) definitions: Vec<DefinitionCapture>,
    pub(crate) calls: Vec<CallCapture>,
}

/// The one capture walk shared by whole-tree materialization and by the
/// incremental edit path. With `within`, subtrees disjoint from the range are
/// pruned at their top, so the work is proportional to the selected range plus
/// the width of the ancestors along its path; a capture must lie wholly inside
/// the range. Preorder order and skip rules are identical in both modes.
pub(crate) fn capture_nodes(
    root: &syntax::SyntaxNode,
    map: &span::SpanMap,
    store: &SpanStore,
    within: Option<span::TextRange>,
) -> Result<Captures, AdapterError> {
    let mut definitions = Vec::new();
    let mut calls = Vec::new();
    let mut bytes = 0usize;
    if within.is_some_and(|range| range.is_empty()) {
        return Ok(Captures { definitions, calls });
    }
    let mut walk = root.preorder();
    while let Some(event) = walk.next() {
        let syntax::WalkEvent::Enter(node) = event else {
            continue;
        };
        if let Some(within) = within {
            let range = node.text_range();
            if range.end() <= within.start() || within.end() <= range.start() {
                walk.skip_subtree();
                continue;
            }
            if !within.contains_range(range) {
                if ast::MacroRules::can_cast(node.kind()) || ast::MacroCall::can_cast(node.kind())
                {
                    return Err(error("captured declaration crosses the selected range"));
                }
                continue;
            }
        }
        if let Some(definition) = ast::MacroRules::cast(node.clone()) {
            let name = definition
                .name()
                .ok_or_else(|| error("generated definition name absent"))?;
            let keyword = definition
                .macro_rules_token()
                .ok_or_else(|| error("generated definition keyword absent"))?;
            let body = definition
                .token_tree()
                .ok_or_else(|| error("generated definition body absent"))?;
            let source = map.span_at(keyword.text_range().start());
            let body = group(body.syntax(), map, source, store)?;
            bytes = bytes
                .checked_add(body.allocated_bytes()?)
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| error("generated macro capture retention budget"))?;
            definitions
                .try_reserve_exact(1)
                .map_err(|_| error("generated definition capture allocation"))?;
            definitions.push(DefinitionCapture {
                start: u32::from(node.text_range().start()),
                end: u32::from(node.text_range().end()),
                name_span: store.key(map.span_at(name.syntax().text_range().start()))?,
                body,
                edition: source.ctx.edition(store.database()).number() as u16,
            });
            walk.skip_subtree();
            continue;
        }
        if let Some(call) = ast::MacroCall::cast(node.clone()) {
            let path = call
                .path()
                .ok_or_else(|| error("generated call path absent"))?;
            let name = path
                .segment()
                .and_then(|segment| segment.name_ref())
                .ok_or_else(|| error("generated call member name absent"))?;
            let name_span = store.key(map.span_at(name.syntax().text_range().start()))?;
            let input = call
                .token_tree()
                .ok_or_else(|| error("generated call input absent"))?;
            let source = map.span_at(path.syntax().text_range().start());
            let path = store.export(&syntax_bridge::syntax_node_to_token_tree(
                path.syntax(),
                map,
                source,
                syntax_bridge::DocCommentDesugarMode::Mbe,
            ))?;
            let input = group(input.syntax(), map, source, store)?;
            let [TokenTree::Group { open, .. }] = input.roots() else {
                return Err(error("generated call capture root absent"));
            };
            let site = *open;
            let input_bytes = input.allocated_bytes()?;
            bytes = bytes
                .checked_add(path.allocated_bytes()?)
                .and_then(|n| n.checked_add(input_bytes))
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| error("generated macro capture retention budget"))?;
            calls
                .try_reserve_exact(1)
                .map_err(|_| error("generated call capture allocation"))?;
            calls.push(CallCapture {
                start: u32::from(node.text_range().start()),
                end: u32::from(node.text_range().end()),
                name_span,
                path,
                input,
                site,
                edition: source.ctx.edition(store.database()).number() as u16,
            });
            walk.skip_subtree();
            continue;
        }
        if ast::TokenTree::can_cast(node.kind()) || ast::Attr::can_cast(node.kind()) {
            walk.skip_subtree();
        }
    }
    Ok(Captures { definitions, calls })
}
