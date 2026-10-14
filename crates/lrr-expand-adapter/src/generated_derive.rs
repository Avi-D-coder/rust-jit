//! Cfg-filter generated ADT syntax with original spans and occurrence-local edits.
use crate::{
    AdapterError,
    generated_attribute::AttributeRange,
    span_store::SpanStore,
    token_wire::{Delimiter, TokenForest, TokenTree},
};
use std::collections::{BTreeMap, BTreeSet};
use syntax::{AstNode, NodeOrToken, WalkEvent, ast};
mod leaves;
#[derive(Clone, Copy)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}
pub struct CfgNode {
    pub range: Range,
    pub enabled: bool,
    pub attributes: Vec<AttributeRange>,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
fn range(node: &syntax::SyntaxNode) -> (u32, u32) {
    let r = node.text_range();
    (r.start().into(), r.end().into())
}
fn roots(f: &TokenForest) -> &[TokenTree] {
    match f.roots() {
        [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] => children,
        r => r,
    }
}
struct Content {
    forest: TokenForest,
    count: usize,
}
fn count(tokens: &[TokenTree], depth: usize) -> Result<usize, AdapterError> {
    if depth > 256 {
        return Err(fault("generated derive content depth limit"));
    }
    tokens.iter().try_fold(0usize, |n, t| {
        let children = match t {
            TokenTree::Group { children, .. } => count(children, depth + 1)?,
            _ => 0,
        };
        n.checked_add(1)
            .and_then(|n| n.checked_add(children))
            .filter(|n| *n <= 2_000_000)
            .ok_or_else(|| fault("generated derive content token limit"))
    })
}
/// Syntax coordinates are private selectors, never fabricated output source spans.
pub fn materialize(
    input: &TokenForest,
    selected: Range,
    outer: &[AttributeRange],
    removed: &[usize],
    nodes: &[CfgNode],
) -> Result<TokenForest, AdapterError> {
    if selected.start >= selected.end
        || outer.len() > 4096
        || removed.len() > 4096
        || nodes.len() > 100_000
        || input.encoded_len()? > 32 * 1024 * 1024
        || input.allocated_bytes()? > 64 * 1024 * 1024
    {
        return Err(fault("generated derive input limit"));
    }
    let removed: BTreeSet<_> = removed.iter().copied().collect();
    if removed.iter().any(|i| *i >= outer.len()) {
        return Err(fault("generated derive removal index invalid"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let (parsed, map) = syntax_bridge::token_tree_to_syntax_node(
        &tree,
        parser::TopEntryPoint::MacroItems,
        &mut |ctx| ctx.edition(store.database()),
    );
    if !parsed.errors().is_empty() {
        return Err(fault("generated derive syntax invalid"));
    }
    let original_leaves = leaves::Leaves::new(&tree, &parsed.syntax_node())?;
    let item = parsed
        .syntax_node()
        .descendants()
        .filter_map(ast::Item::cast)
        .find(|i| range(i.syntax()) == (selected.start, selected.end))
        .ok_or_else(|| fault("generated derive item absent"))?;
    if !matches!(
        item,
        ast::Item::Struct(_) | ast::Item::Enum(_) | ast::Item::Union(_)
    ) {
        return Err(fault("generated derive requires ADT"));
    }
    let mut owners = BTreeMap::new();
    let mut wrappers = BTreeMap::new();
    for owner in item
        .syntax()
        .descendants()
        .filter_map(ast::AnyHasAttrs::cast)
    {
        if ast::attrs_including_inner(&owner).next().is_some() {
            if owners.len() >= 100_001 {
                return Err(fault("generated derive owner index limit"));
            }
            for attr in ast::attrs_including_inner(&owner) {
                if wrappers.len() >= 100_000 {
                    return Err(fault("generated derive wrapper index limit"));
                }
                wrappers.insert(range(attr.syntax()), attr);
            }
            owners.insert(range(owner.syntax()), owner);
        }
    }
    let mut disabled = BTreeMap::new();
    let mut commas = BTreeSet::new();
    let mut recipes: BTreeMap<(u32, u32), Vec<AttributeRange>> = BTreeMap::new();
    let mut add = |node: &syntax::SyntaxNode,
                   attributes: &[AttributeRange],
                   retained: Vec<AttributeRange>|
     -> Result<(), AdapterError> {
        let owner = ast::AnyHasAttrs::cast(node.clone())
            .ok_or_else(|| fault("generated derive cfg owner absent"))?;
        for wrapper in ast::attrs_including_inner(&owner) {
            if recipes.len() >= 100_000 {
                return Err(fault("generated derive wrapper count limit"));
            }
            recipes.insert(range(wrapper.syntax()), Vec::new());
        }
        for attribute in attributes {
            let owner_range = range(node);
            if attribute.start < owner_range.0
                || attribute.end > owner_range.1
                || attribute.start >= attribute.end
            {
                return Err(fault("generated derive meta outside owner"));
            }
            if !recipes.contains_key(&(attribute.wrapper_start, attribute.wrapper_end)) {
                return Err(fault("generated derive wrapper outside owner"));
            }
        }
        for attribute in retained {
            recipes
                .get_mut(&(attribute.wrapper_start, attribute.wrapper_end))
                .ok_or_else(|| fault("generated derive wrapper absent"))?
                .push(attribute);
        }
        Ok(())
    };
    add(
        item.syntax(),
        outer,
        outer
            .iter()
            .enumerate()
            .filter_map(|(i, a)| (!removed.contains(&i)).then_some(*a))
            .collect(),
    )?;
    let mut seen = BTreeSet::new();
    for node in nodes {
        let key = (node.range.start, node.range.end);
        if key.0 < selected.start
            || key.1 > selected.end
            || key.0 >= key.1
            || key == (selected.start, selected.end)
            || !seen.insert(key)
            || node.attributes.len() > 4096
        {
            return Err(fault("generated derive cfg node range/count invalid"));
        }
        let owner = owners
            .get(&key)
            .ok_or_else(|| fault("generated derive cfg node absent"))?;
        if node.enabled {
            add(owner.syntax(), &node.attributes, node.attributes.to_vec())?;
        } else {
            if !node.attributes.is_empty() {
                return Err(fault(
                    "disabled generated derive node has effective attributes",
                ));
            }
            disabled.insert(key, owner.syntax().clone());
            let mut next = owner.syntax().next_sibling_or_token();
            while let Some(sibling) = next {
                match &sibling {
                    NodeOrToken::Token(token) if token.kind().is_trivia() => {
                        next = sibling.next_sibling_or_token()
                    }
                    NodeOrToken::Token(token) if token.text() == "," => {
                        let r = token.text_range();
                        commas.insert((u32::from(r.start()), u32::from(r.end())));
                        break;
                    }
                    _ => break,
                }
            }
        }
    }
    let mut replacements = Vec::new();
    replacements
        .try_reserve_exact(recipes.len())
        .map_err(|_| fault("generated derive recipe allocation"))?;
    let mut rule_index = BTreeMap::new();
    let mut retained = 0usize;
    for (wrapper_range, metas) in recipes {
        let wrapper = wrappers
            .get(&wrapper_range)
            .ok_or_else(|| fault("generated derive original wrapper absent"))?;
        let mut meta_index = BTreeMap::new();
        if !metas.is_empty() {
            for meta in wrapper.syntax().descendants().filter_map(ast::Meta::cast) {
                if meta_index.len() >= 100_000 {
                    return Err(fault("generated derive meta index limit"));
                }
                meta_index.insert(range(meta.syntax()), meta);
            }
        }
        let mut contents = Vec::new();
        contents
            .try_reserve_exact(metas.len())
            .map_err(|_| fault("generated derive meta allocation"))?;
        for meta in metas {
            let node = meta_index
                .get(&(meta.start, meta.end))
                .ok_or_else(|| fault("generated derive original meta absent"))?;
            let forest = store.export(&syntax_bridge::syntax_node_to_token_tree_modified(
                node.syntax(),
                &map,
                Default::default(),
                Default::default(),
                map.span_at(node.syntax().text_range().start()),
                syntax_bridge::DocCommentDesugarMode::Mbe,
                |_, event| match original_leaves.event(event) {
                    Some(leaves) => (false, leaves),
                    None => (true, Vec::new()),
                },
            ))?;
            retained = retained
                .checked_add(forest.allocated_bytes()?)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| fault("generated derive recipe retention limit"))?;
            let count = count(roots(&forest), 0)?;
            contents.push(Content { forest, count });
        }
        rule_index.insert(wrapper_range, replacements.len());
        replacements.push(contents);
    }
    let mut occurrences = Vec::new();
    let mut error = None;
    let mut removed_nodes = BTreeSet::new();
    let modified = syntax_bridge::syntax_node_to_token_tree_modified(
        item.syntax(),
        &map,
        Default::default(),
        Default::default(),
        map.span_at(item.syntax().text_range().start()),
        syntax_bridge::DocCommentDesugarMode::Mbe,
        |preorder, event| {
            if let WalkEvent::Enter(element) = event {
                if let NodeOrToken::Node(node) = element
                    && disabled.get(&range(node)) == Some(node)
                {
                    removed_nodes.insert(range(node));
                    preorder.skip_subtree();
                    return (false, Vec::new());
                }
                if let NodeOrToken::Token(token) = element {
                    let r = token.text_range();
                    if commas.contains(&(r.start().into(), r.end().into())) {
                        return (false, Vec::new());
                    }
                    if token.text() == "#" {
                        let mut next = token.next_token();
                        while next.as_ref().is_some_and(|t| t.kind().is_trivia()) {
                            next = next.and_then(|t| t.next_token());
                        }
                        if next.as_ref().is_some_and(|t| t.text() == "!") {
                            next = next.and_then(|t| t.next_token());
                            while next.as_ref().is_some_and(|t| t.kind().is_trivia()) {
                                next = next.and_then(|t| t.next_token());
                            }
                        }
                        if next.as_ref().is_some_and(|t| t.text() == "[") {
                            if occurrences.len() >= 100_000 || occurrences.try_reserve(1).is_err() {
                                error = Some(fault(
                                    "generated derive occurrence allocation/count limit",
                                ));
                                return (false, Vec::new());
                            }
                            let attr = token
                                .parent_ancestors()
                                .find_map(ast::Attr::cast)
                                .filter(|a| a.syntax().first_token().as_ref() == Some(token));
                            occurrences.push(
                                attr.and_then(|a| rule_index.get(&range(a.syntax())).copied()),
                            );
                        }
                    }
                }
            }
            match original_leaves.event(event) {
                Some(leaves) => (false, leaves),
                None => (true, Vec::new()),
            }
        },
    );
    if let Some(error) = error {
        return Err(error);
    }
    if removed_nodes.len() != disabled.len() {
        return Err(fault("generated derive disabled nodes were not visited"));
    }
    let whole = store.export(&modified)?;
    retained
        .checked_add(whole.allocated_bytes()?)
        .filter(|n| *n <= 128 * 1024 * 1024)
        .ok_or_else(|| fault("generated derive edit retention limit"))?;
    let mut edit = Rewrite {
        occurrences: &occurrences,
        replacements: &replacements,
        at: 0,
        tokens: 0,
        seen: BTreeSet::new(),
    };
    let roots = edit.rewrite(whole.roots(), 0)?;
    if edit.at != occurrences.len() || edit.seen.len() != replacements.len() {
        return Err(fault("generated derive occurrence edits incomplete"));
    }
    let mut contexts = BTreeMap::new();
    let mut spans = BTreeMap::new();
    for forest in std::iter::once(&whole).chain(replacements.iter().flatten().map(|c| &c.forest)) {
        for row in forest.contexts() {
            let key = row.key();
            if contexts.get(&key).is_some_and(|old| old != row)
                || (!contexts.contains_key(&key) && contexts.len() >= 2_000_000)
            {
                return Err(fault("generated derive context conflict/count limit"));
            }
            contexts.insert(key, row.clone());
        }
        for row in forest.spans() {
            let key = row.key();
            if spans.get(&key).is_some_and(|old| old != row)
                || (!spans.contains_key(&key) && spans.len() >= 2_000_000)
            {
                return Err(fault("generated derive span conflict/count limit"));
            }
            spans.insert(key, row.clone());
        }
    }
    let result = TokenForest::new(
        roots,
        contexts.into_values().collect(),
        spans.into_values().collect(),
    )?;
    if result.allocated_bytes()? > 64 * 1024 * 1024 || result.encoded_len()? > 32 * 1024 * 1024 {
        return Err(fault("generated derive output limit"));
    }
    let imported = store.import(&result)?;
    store.export(&imported)
}
fn prefix(tokens: &[TokenTree], at: usize) -> Option<(usize, bool)> {
    if !matches!(tokens.get(at), Some(TokenTree::Punct { scalar: '#', .. })) {
        return None;
    }
    let inner = matches!(
        tokens.get(at + 1),
        Some(TokenTree::Punct { scalar: '!', .. })
    );
    let group = at + 1 + usize::from(inner);
    matches!(
        tokens.get(group),
        Some(TokenTree::Group {
            delimiter: Delimiter::Bracket,
            ..
        })
    )
    .then_some((group, inner))
}
struct Rewrite<'a> {
    occurrences: &'a [Option<usize>],
    replacements: &'a [Vec<Content>],
    at: usize,
    tokens: usize,
    seen: BTreeSet<usize>,
}
impl Rewrite<'_> {
    fn charge(&mut self, n: usize) -> Result<(), AdapterError> {
        self.tokens = self
            .tokens
            .checked_add(n)
            .filter(|n| *n <= 2_000_000)
            .ok_or_else(|| fault("generated derive rewritten token limit"))?;
        Ok(())
    }
    fn occurrence(&mut self) -> Result<Option<usize>, AdapterError> {
        let value = *self
            .occurrences
            .get(self.at)
            .ok_or_else(|| fault("generated derive wrapper occurrence absent"))?;
        self.at += 1;
        Ok(value)
    }
    fn consume(&mut self, tokens: &[TokenTree], depth: usize) -> Result<(), AdapterError> {
        if depth > 256 {
            return Err(fault("generated derive consume depth limit"));
        }
        for (at, token) in tokens.iter().enumerate() {
            if prefix(tokens, at).is_some() && self.occurrence()?.is_some() {
                return Err(fault("generated derive nested meta edit conflict"));
            }
            if let TokenTree::Group { children, .. } = token {
                self.consume(children, depth + 1)?;
            }
        }
        Ok(())
    }
    fn rewrite(
        &mut self,
        tokens: &[TokenTree],
        depth: usize,
    ) -> Result<Vec<TokenTree>, AdapterError> {
        if depth > 256 {
            return Err(fault("generated derive rewrite depth limit"));
        }
        let mut output = Vec::new();
        let mut at = 0;
        while at < tokens.len() {
            if let Some((group, inner)) = prefix(tokens, at) {
                let rule = self.occurrence()?;
                if let Some(rule) = rule {
                    if !self.seen.insert(rule) {
                        return Err(fault("generated derive duplicate wrapper edit"));
                    }
                    let TokenTree::Group {
                        open,
                        close,
                        children,
                        ..
                    } = &tokens[group]
                    else {
                        return Err(fault("generated derive wrapper group absent"));
                    };
                    self.consume(children, depth + 1)?;
                    for content in &self.replacements[rule] {
                        self.charge(content.count + 2 + usize::from(inner))?;
                        output
                            .try_reserve(2 + usize::from(inner))
                            .map_err(|_| fault("generated derive rewritten wrapper allocation"))?;
                        output.push(tokens[at].clone());
                        if inner {
                            output.push(tokens[at + 1].clone());
                        }
                        output.push(TokenTree::Group {
                            delimiter: Delimiter::Bracket,
                            open: *open,
                            close: *close,
                            children: roots(&content.forest).to_vec(),
                        });
                    }
                    at = group + 1;
                    continue;
                }
            }
            self.charge(1)?;
            output
                .try_reserve(1)
                .map_err(|_| fault("generated derive rewritten token allocation"))?;
            output.push(match &tokens[at] {
                TokenTree::Group {
                    delimiter,
                    open,
                    close,
                    children,
                } => TokenTree::Group {
                    delimiter: *delimiter,
                    open: *open,
                    close: *close,
                    children: self.rewrite(children, depth + 1)?,
                },
                token => token.clone(),
            });
            at += 1;
        }
        Ok(output)
    }
}
