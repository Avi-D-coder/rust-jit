//! Select effective generated attributes through the pinned token-tree syntax map.
use crate::{
    AdapterError,
    declaration_output::DeclarationGrammar,
    span_store::SpanStore,
    token_wire::{Delimiter, Key, TokenForest, TokenTree},
};
use syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};
#[derive(Debug, Clone, Copy)]
pub struct AttributeRange {
    pub start: u32,
    pub end: u32,
    pub wrapper_start: u32,
    pub wrapper_end: u32,
}
pub struct AttributeInput {
    pub item: TokenForest,
    pub arguments: Option<TokenForest>,
    pub path: TokenForest,
    pub site: Key,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
fn charge(current: usize, amount: usize) -> Result<usize, AdapterError> {
    current
        .checked_add(amount)
        .ok_or_else(|| fault("generated attribute token retention limit"))
}
fn roots(forest: &TokenForest) -> &[TokenTree] {
    match forest.roots() {
        [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] => children,
        roots => roots,
    }
}
/// Select a generated meta's original parenthesized arguments by syntax position.
/// The caller resolves its annotation identity; this function grants no semantics.
pub fn argument_forest(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    meta_start: u32,
    meta_end: u32,
) -> Result<TokenForest, AdapterError> {
    argument_forest_for_grammar(
        input,
        item_start,
        item_end,
        meta_start,
        meta_end,
        DeclarationGrammar::Items,
    )
}
pub fn argument_forest_for_grammar(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    meta_start: u32,
    meta_end: u32,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    meta_forest(
        input, item_start, item_end, meta_start, meta_end, false, grammar,
    )
}
/// Original generated annotation path, including its context/edition records.
pub fn path_forest(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    meta_start: u32,
    meta_end: u32,
) -> Result<TokenForest, AdapterError> {
    path_forest_for_grammar(
        input,
        item_start,
        item_end,
        meta_start,
        meta_end,
        DeclarationGrammar::Items,
    )
}
pub fn path_forest_for_grammar(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    meta_start: u32,
    meta_end: u32,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    meta_forest(
        input, item_start, item_end, meta_start, meta_end, true, grammar,
    )
}
/// Resolve only the path of an exact indexed attribute meta. Inert names can
/// belong to fields or a fragment root, so path transport has no item-target
/// requirement. Target/cfg/producer authority remains with the owned caller.
pub fn path_forest_at_meta_for_grammar(
    input: &TokenForest,
    meta_start: u32,
    meta_end: u32,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    if meta_start >= meta_end {
        return Err(fault("generated attribute path range or byte limit"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated attribute path syntax invalid"));
    }
    let mut selected = None;
    for node in parsed.syntax_node().descendants() {
        let Some(meta) = ast::Meta::cast(node) else {
            continue;
        };
        let range = meta.syntax().text_range();
        if u32::from(range.start()) == meta_start && u32::from(range.end()) == meta_end {
            if selected.is_some() {
                return Err(fault("generated attribute path meta is ambiguous"));
            }
            selected = Some(meta);
        }
    }
    let node = selected
        .ok_or_else(|| fault("generated attribute path exact meta absent"))?
        .path()
        .ok_or_else(|| fault("generated attribute path absent"))?
        .syntax()
        .clone();
    let result = store.export(&syntax_bridge::syntax_node_to_token_tree(
        &node,
        &map,
        map.span_at(node.text_range().start()),
        syntax_bridge::DocCommentDesugarMode::Mbe,
    ))?;
    Ok(result)
}

/// Export the original binding name of an exact generated use-tree leaf. For a
/// glob, export its module path. This supplies spans only, never alias semantics.
pub fn import_binding_forest(
    input: &TokenForest,
    start: u32,
    end: u32,
) -> Result<TokenForest, AdapterError> {
    import_binding_forest_for_grammar(input, start, end, DeclarationGrammar::Items)
}
pub fn import_binding_forest_for_grammar(
    input: &TokenForest,
    start: u32,
    end: u32,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    if start >= end {
        return Err(fault("generated import input range or byte limit"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated import syntax invalid"));
    }
    let mut selected = None;
    for node in parsed.syntax_node().descendants() {
        if let Some(leaf) = ast::UseTree::cast(node) {
            let range = leaf.syntax().text_range();
            if u32::from(range.start()) == start && u32::from(range.end()) == end {
                selected = Some(leaf);
                break;
            }
        }
    }
    let leaf = selected.ok_or_else(|| fault("generated import leaf absent"))?;
    let name = if let Some(rename) = leaf.rename() {
        rename.name().map(|n| n.syntax().clone())
    } else if leaf.star_token().is_some() {
        leaf.path().map(|p| p.syntax().clone())
    } else {
        leaf.path()
            .and_then(|p| p.segment())
            .and_then(|s| s.name_ref())
            .map(|n| n.syntax().clone())
    }
    .ok_or_else(|| fault("generated import binding name absent"))?;
    let result = store.export(&syntax_bridge::syntax_node_to_token_tree(
        &name,
        &map,
        map.span_at(name.text_range().start()),
        syntax_bridge::DocCommentDesugarMode::Mbe,
    ))?;
    Ok(result)
}
/// Original target paths, including grouped-use prefixes. No resolution or execution.
pub fn import_path_forests(
    input: &TokenForest,
    start: u32,
    end: u32,
) -> Result<Vec<TokenForest>, AdapterError> {
    import_path_forests_for_grammar(input, start, end, DeclarationGrammar::Items)
}
pub fn import_path_forests_for_grammar(
    input: &TokenForest,
    start: u32,
    end: u32,
    grammar: DeclarationGrammar,
) -> Result<Vec<TokenForest>, AdapterError> {
    if start >= end {
        return Err(fault("generated import input range or byte limit"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated import syntax invalid"));
    }
    let mut selected = None;
    for node in parsed.syntax_node().descendants() {
        if let Some(leaf) = ast::UseTree::cast(node) {
            let range = leaf.syntax().text_range();
            if u32::from(range.start()) == start && u32::from(range.end()) == end {
                selected = Some(leaf);
                break;
            }
        }
    }
    let leaf = selected.ok_or_else(|| fault("generated import leaf absent"))?;
    let mut output = Vec::new();
    let mut retained = 0usize;
    for node in leaf.syntax().ancestors() {
        let Some(tree) = ast::UseTree::cast(node) else {
            continue;
        };
        let Some(path) = tree.path() else {
            continue;
        };
        let original = store.export(&syntax_bridge::syntax_node_to_token_tree(
            path.syntax(),
            &map,
            map.span_at(path.syntax().text_range().start()),
            syntax_bridge::DocCommentDesugarMode::Mbe,
        ))?;
        retained = charge(retained, original.allocated_bytes()?)?;
        output
            .try_reserve_exact(1)
            .map_err(|_| fault("generated import path allocation"))?;
        output.push(original);
    }
    if output.is_empty() {
        return Err(fault("generated import target path absent"));
    }
    Ok(output)
}

fn meta_forest(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    meta_start: u32,
    meta_end: u32,
    path: bool,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    if item_start >= item_end
        || meta_start >= meta_end
        || meta_start < item_start
        || meta_end > item_end
    {
        return Err(fault("generated meta argument input limit or range"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated meta argument syntax invalid"));
    }
    let item = parsed
        .syntax_node()
        .descendants()
        .filter_map(ast::Item::cast)
        .find(|item| {
            let range = item.syntax().text_range();
            u32::from(range.start()) == item_start && u32::from(range.end()) == item_end
        })
        .ok_or_else(|| fault("generated meta argument item absent"))?;
    let selected = |meta: &ast::Meta| {
        let range = meta.syntax().text_range();
        u32::from(range.start()) == meta_start && u32::from(range.end()) == meta_end
    };
    // Paths also belong to fields/variants. Select the exact pinned-AST meta;
    // the caller's owning-item/cfg facts supply semantic eligibility.
    let meta = if path {
        item.syntax()
            .descendants()
            .filter_map(ast::Meta::cast)
            .find(selected)
    } else {
        item.attrs()
            .flat_map(|attribute| attribute.syntax().descendants().filter_map(ast::Meta::cast))
            .find(selected)
    }
    .ok_or_else(|| fault("generated meta argument annotation absent"))?;
    let node = if path {
        meta.path()
            .ok_or_else(|| fault("generated meta path absent"))?
            .syntax()
            .clone()
    } else {
        meta.syntax()
            .children()
            .find_map(ast::TokenTree::cast)
            .ok_or_else(|| fault("generated meta arguments absent"))?
            .syntax()
            .clone()
    };
    let tokens = store.export(&syntax_bridge::syntax_node_to_token_tree(
        &node,
        &map,
        map.span_at(node.text_range().start()),
        syntax_bridge::DocCommentDesugarMode::Mbe,
    ))?;
    if path {
        return Ok(tokens);
    }
    let [
        root @ TokenTree::Group {
            delimiter: Delimiter::Parenthesis,
            ..
        },
    ] = roots(&tokens)
    else {
        return Err(fault("generated meta arguments require parentheses"));
    };
    let result = TokenForest::new(
        vec![root.clone()],
        tokens.contexts().to_vec(),
        tokens.spans().to_vec(),
    )?;
    Ok(result)
}
pub fn materialize(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    applied: usize,
    effective: &[AttributeRange],
) -> Result<AttributeInput, AdapterError> {
    materialize_for_grammar(
        input,
        item_start,
        item_end,
        applied,
        effective,
        DeclarationGrammar::Items,
    )
}
pub fn materialize_for_grammar(
    input: &TokenForest,
    item_start: u32,
    item_end: u32,
    applied: usize,
    effective: &[AttributeRange],
    grammar: DeclarationGrammar,
) -> Result<AttributeInput, AdapterError> {
    if applied >= effective.len() || item_start >= item_end {
        return Err(fault("generated attribute input limit or selection"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated attribute syntax invalid"));
    }
    let root = parsed.syntax_node();
    let item = root
        .descendants()
        .filter_map(ast::Item::cast)
        .find(|item| {
            let range = item.syntax().text_range();
            u32::from(range.start()) == item_start && u32::from(range.end()) == item_end
        })
        .ok_or_else(|| fault("generated attribute item absent"))?;
    let source = map.span_at(item.syntax().text_range().start());
    let export = |node: &syntax::SyntaxNode| {
        store.export(&syntax_bridge::syntax_node_to_token_tree(
            node,
            &map,
            source,
            syntax_bridge::DocCommentDesugarMode::Mbe,
        ))
    };
    let whole = export(item.syntax())?;
    let [TokenTree::Group { open, close, .. }] = whole.roots() else {
        return Err(fault("generated attribute item root absent"));
    };
    let body = roots(&whole);
    let mut prefix = 0usize;
    let mut parts = vec![];
    parts
        .try_reserve_exact(
            effective
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| fault("generated attribute part count overflow"))?,
        )
        .map_err(|_| fault("generated attribute part allocation"))?;
    let count = item.attrs().count();
    let mut original = Vec::new();
    original
        .try_reserve_exact(count)
        .map_err(|_| fault("generated original attribute allocation"))?;
    original.extend(item.attrs());
    for attribute in &original {
        let tokens = export(attribute.syntax())?;
        let end = prefix
            .checked_add(roots(&tokens).len())
            .ok_or_else(|| fault("generated attribute prefix overflow"))?;
        if body.get(prefix..end) != Some(roots(&tokens)) {
            return Err(fault(
                "generated attribute wrapper differs from item prefix",
            ));
        }
        prefix = end;
    }
    let mut output = Vec::new();
    let mut arguments = None;
    let mut selected_path = None;
    let mut retained = whole.allocated_bytes()?;
    for (index, range) in effective.iter().enumerate() {
        let wrapper = original
            .iter()
            .find(|attribute| {
                let r = attribute.syntax().text_range();
                u32::from(r.start()) == range.wrapper_start
                    && u32::from(r.end()) == range.wrapper_end
            })
            .ok_or_else(|| fault("effective generated attribute wrapper absent"))?;
        let meta = wrapper
            .syntax()
            .descendants()
            .filter_map(ast::Meta::cast)
            .find(|meta| {
                let r = meta.syntax().text_range();
                u32::from(r.start()) == range.start && u32::from(r.end()) == range.end
            })
            .ok_or_else(|| fault("effective generated attribute meta absent"))?;
        let content = export(meta.syntax())?;
        retained = charge(retained, content.allocated_bytes()?)?;
        if index == applied {
            let path = meta
                .path()
                .ok_or_else(|| fault("generated attribute path absent"))?;
            let path = export(path.syntax())?;
            retained = charge(retained, path.allocated_bytes()?)?;
            selected_path = Some(path);
            if let Some(group) = meta.syntax().children().find_map(ast::TokenTree::cast) {
                let tokens = export(group.syntax())?;
                let [
                    TokenTree::Group {
                        open,
                        close,
                        children,
                        ..
                    },
                ] = roots(&tokens)
                else {
                    return Err(fault("generated attribute arguments absent"));
                };
                let arguments_input = TokenForest::new(
                    vec![TokenTree::Group {
                        delimiter: Delimiter::Invisible,
                        open: *open,
                        close: *close,
                        children: children.clone(),
                    }],
                    tokens.contexts().to_vec(),
                    tokens.spans().to_vec(),
                )?;
                retained = charge(retained, arguments_input.allocated_bytes()?)?;
                arguments = Some(arguments_input);
            }
        } else {
            let wrapper = export(wrapper.syntax())?;
            retained = charge(retained, wrapper.allocated_bytes()?)?;
            let [hash, TokenTree::Group { open, close, .. }] = roots(&wrapper) else {
                return Err(fault("generated attribute wrapper tokens absent"));
            };
            output
                .try_reserve(2)
                .map_err(|_| fault("generated effective attribute allocation"))?;
            output.push(hash.clone());
            output.push(TokenTree::Group {
                delimiter: Delimiter::Bracket,
                open: *open,
                close: *close,
                children: roots(&content).to_vec(),
            });
            parts.push(wrapper);
        }
        parts.push(content);
    }
    output
        .try_reserve(body.len().saturating_sub(prefix))
        .map_err(|_| fault("generated attribute body allocation"))?;
    output.extend_from_slice(&body[prefix..]);
    let path = selected_path.ok_or_else(|| fault("generated selected attribute path absent"))?;
    let site = match roots(&path).first() {
        Some(TokenTree::Ident { span, .. }) | Some(TokenTree::Punct { span, .. }) => *span,
        _ => return Err(fault("generated selected attribute site absent")),
    };
    let mut contexts = std::collections::BTreeMap::new();
    let mut spans = std::collections::BTreeMap::new();
    for forest in std::iter::once(&whole)
        .chain(parts.iter())
        .chain(std::iter::once(&path))
        .chain(arguments.iter())
    {
        for context in forest.contexts() {
            let key = context.key();
            if contexts.get(&key).is_some_and(|old| old != context) {
                return Err(fault("generated attribute context conflict"));
            }
            contexts.insert(key, context.clone());
        }
        for span in forest.spans() {
            let key = span.key();
            if spans.get(&key).is_some_and(|old| old != span) {
                return Err(fault("generated attribute span conflict"));
            }
            spans.insert(key, span.clone());
        }
    }
    let item = TokenForest::new(
        vec![TokenTree::Group {
            delimiter: Delimiter::Invisible,
            open: *open,
            close: *close,
            children: output,
        }],
        contexts.into_values().collect(),
        spans.into_values().collect(),
    )?;
    Ok(AttributeInput {
        item,
        arguments,
        path,
        site,
    })
}
/// Original name tokens for an exact generated module declaration. This bridge
/// supplies physical spans only; the coordinator validates lexical/context authority.
pub fn module_binding_forest_for_grammar(
    input: &TokenForest,
    start: u32,
    end: u32,
    grammar: DeclarationGrammar,
) -> Result<TokenForest, AdapterError> {
    module_binding_forests_for_grammar(input, &[(start, end)], grammar)?
        .pop()
        .ok_or_else(|| fault("generated module declaration absent"))
}
/// One pinned syntax replay for all exact requested module ranges. Results keep
/// caller order; duplicate/absent physical rows refuse the whole batch.
pub fn module_binding_forests_for_grammar(
    input: &TokenForest,
    ranges: &[(u32, u32)],
    grammar: DeclarationGrammar,
) -> Result<Vec<TokenForest>, AdapterError> {
    let mut requested = std::collections::BTreeMap::new();
    for (index, &(start, end)) in ranges.iter().enumerate() {
        if start >= end {
            return Err(fault("generated module input range or byte limit"));
        }
        if requested.insert((start, end), index).is_some() {
            return Err(fault("generated module duplicate requested range"));
        }
    }
    let mut result: Vec<Option<TokenForest>> = Vec::new();
    result
        .try_reserve_exact(ranges.len())
        .map_err(|_| fault("generated module batch allocation limit"))?;
    result.resize_with(ranges.len(), || None);
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) =
        syntax_bridge::token_tree_to_syntax_node(&tree, grammar.entry_point(), &mut |context| {
            context.edition(store.database())
        });
    if !parsed.errors().is_empty() {
        return Err(fault("generated module syntax invalid"));
    }
    let mut retained = 0usize;
    for node in parsed.syntax_node().descendants() {
        let Some(module) = ast::Module::cast(node) else {
            continue;
        };
        let range = module.syntax().text_range();
        let Some(index) = requested.remove(&(u32::from(range.start()), u32::from(range.end())))
        else {
            continue;
        };
        let name = module
            .name()
            .ok_or_else(|| fault("generated module binding name absent"))?;
        let original = store.export(&syntax_bridge::syntax_node_to_token_tree(
            name.syntax(),
            &map,
            map.span_at(name.syntax().text_range().start()),
            syntax_bridge::DocCommentDesugarMode::Mbe,
        ))?;
        retained = retained
            .checked_add(original.allocated_bytes()?)
            .ok_or_else(|| fault("generated module binding aggregate retention limit"))?;
        result[index] = Some(original);
        if requested.is_empty() {
            break;
        }
    }
    if !requested.is_empty() {
        return Err(fault("generated module declaration absent"));
    }
    let mut forests = Vec::new();
    forests
        .try_reserve_exact(result.len())
        .map_err(|_| fault("generated module batch allocation limit"))?;
    for forest in result {
        forests.push(forest.ok_or_else(|| fault("generated module declaration absent"))?);
    }
    Ok(forests)
}
