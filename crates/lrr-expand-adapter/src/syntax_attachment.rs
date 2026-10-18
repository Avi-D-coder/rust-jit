//! Structural expansion attachment, without an expression evaluator or text
//! substitution. The caller still owns expansion admission, epochs and seals.
use crate::{
    AdapterError,
    declaration_output::{self, DeclarationGrammar, DeclarationSyntax},
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
use span::{SpanMap, TextRange};
use std::sync::{Arc, Mutex};
use syntax::{AstNode, SyntaxNode, WalkEvent, ast};
mod lists;
pub use lists::ListSlot;
mod snapshot;
pub use snapshot::SyntaxSnapshot;
mod edit;
pub use edit::SyntaxEdit;
mod queries;
pub use queries::{BodyOwners, RangeCaptures};
mod seed;
pub use seed::SyntaxAttachmentSeed;

/// Single-node grammar slots. Lists use `attach_list` to preserve caller scope.
#[derive(Clone, Copy, Debug)]
pub enum FragmentSlot {
    Expression,
    Pattern,
    Type,
}
impl FragmentSlot {
    fn grammar(self) -> DeclarationGrammar {
        match self {
            Self::Expression => DeclarationGrammar::Expression,
            Self::Pattern => DeclarationGrammar::Pattern,
            Self::Type => DeclarationGrammar::Type,
        }
    }
    fn accepts_call(self, node: &SyntaxNode) -> bool {
        match self {
            Self::Expression => ast::MacroExpr::can_cast(node.kind()),
            Self::Pattern => ast::MacroPat::can_cast(node.kind()),
            Self::Type => ast::MacroType::can_cast(node.kind()),
        }
    }
    fn accepts_output(self, node: &SyntaxNode) -> bool {
        match self {
            Self::Expression => ast::Expr::can_cast(node.kind()),
            Self::Pattern => ast::Pat::can_cast(node.kind()),
            Self::Type => ast::Type::can_cast(node.kind()),
        }
    }
}

/// Ephemeral pinned trees stay inside the adapter. Materialization exports only
/// owned syntax events, captures and stable origin keys. Retain this object for
/// successive attachments: reparsing its display text would erase boundaries.
pub struct SyntaxAttachment {
    root: SyntaxNode,
    grammar: DeclarationGrammar,
    origins: Arc<SpanMap>,
    store: Arc<Mutex<SpanStore>>,
    // Conservative cumulative charge, including superseded records. Admission
    // happens before cloning the span store, not after retaining another input.
    retained_input_bytes: usize,
    /// The owned snapshot of this exact tree state, built on first request
    /// and dropped by any in-place edit (`apply`).
    snapshot_cache: std::sync::OnceLock<Arc<snapshot::SyntaxSnapshot>>,
}

struct PreparedFragment {
    root: SyntaxNode,
    origins: SpanMap,
    /// A private copy of a store that other holders (a seed, sibling forks)
    /// still share. `None` when the attachment owned its store and extended it
    /// in place.
    store: Option<SpanStore>,
    retained_input_bytes: usize,
}

fn error(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}

/// The outermost node whose range is exactly `target` and which satisfies
/// `selected`, descending by the tree's binary child lookup.
fn outermost_at(
    root: &SyntaxNode,
    target: TextRange,
    mut selected: impl FnMut(&SyntaxNode) -> bool,
) -> Option<SyntaxNode> {
    if target.is_empty() || !root.text_range().contains_range(target) {
        return None;
    }
    let mut node = root.clone();
    loop {
        if node.text_range() == target && selected(&node) {
            return Some(node);
        }
        match node.child_or_token_at_range(target) {
            Some(syntax::NodeOrToken::Node(child)) => node = child,
            _ => return None,
        }
    }
}

fn check_tree(root: &SyntaxNode) -> Result<(), AdapterError> {
    crate::grammar::validate_complete_syntax(root)
}

fn check_origins(map: &SpanMap, length: u32) -> Result<(), AdapterError> {
    let mut previous = 0;
    for (end, _) in map.iter() {
        let end = u32::from(end);
        if end <= previous || end > length {
            return Err(error("attached syntax origin ordering/budget"));
        }
        previous = end;
    }
    if previous != length {
        return Err(error("attached syntax origin coverage"));
    }
    Ok(())
}

/// Clip intervals at both splice boundaries. A parser's origin interval may
/// cover trivia as well as a token, so simply deleting its end offset would
/// incorrectly attribute surviving caller bytes to the replacement.
fn splice_origins(
    original: &SpanMap,
    range: TextRange,
    replacement: &SpanMap,
    replacement_len: u32,
    final_len: u32,
) -> Result<SpanMap, AdapterError> {
    let mut result = SpanMap::empty();
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    let mut last = 0;
    let mut count = 0;
    let mut push = |offset: u32, origin| -> Result<(), AdapterError> {
        if offset <= last || offset > final_len {
            return Err(error("attached origin splice ordering/budget"));
        }
        result.push(offset.into(), origin);
        count += 1;
        last = offset;
        Ok(())
    };
    for (offset, origin) in original.iter() {
        let offset = u32::from(offset);
        if start == 0 {
            break;
        }
        push(offset.min(start), origin)?;
        if offset >= start {
            break;
        }
    }
    for (offset, origin) in replacement.iter() {
        let offset = start
            .checked_add(u32::from(offset))
            .ok_or_else(|| error("attached origin addition overflow"))?;
        push(offset, origin)?;
    }
    for (offset, origin) in original.iter() {
        let offset = u32::from(offset);
        if offset > end {
            let offset = offset
                .checked_sub(end - start)
                .and_then(|offset| offset.checked_add(replacement_len))
                .ok_or_else(|| error("attached origin shift overflow"))?;
            push(offset, origin)?;
        }
    }
    check_origins(&result, final_len)?;
    result.finish();
    Ok(result)
}

fn replace_node(target: &SyntaxNode, replacement: &SyntaxNode) -> Result<SyntaxNode, AdapterError> {
    // Rowan's replace_with requires the same kind. Replace a selected child
    // inside its parent before rebuilding the unchanged-kind ancestor chain.
    let green = replacement.green().to_owned();
    Ok(if let Some(parent) = target.parent() {
        // The node's slot among its parent's children (tokens included).
        let index = target.index();
        let green = parent.green().replace_child(index, green.into());
        SyntaxNode::new_root(parent.replace_with(green))
    } else {
        SyntaxNode::new_root(green)
    })
}

impl SyntaxAttachment {
    pub fn parse(input: &TokenForest, grammar: DeclarationGrammar) -> Result<Self, AdapterError> {
        let retained_input_bytes = input.allocated_bytes()?;
        let mut store = SpanStore::default();
        let (root, origins) = declaration_output::parse_root(input, grammar, &mut store)?;
        check_tree(&root)?;
        check_origins(&origins, u32::from(root.text_range().len()))?;
        Ok(Self {
            root,
            grammar,
            origins: Arc::new(origins),
            store: Arc::new(Mutex::new(store)),
            retained_input_bytes,
            snapshot_cache: std::sync::OnceLock::new(),
        })
    }

    /// Coordinates are those of the current materialized tree, never raw file
    /// offsets. No macro name, standard-library identity or runtime value is
    /// consulted. On error the retained caller tree and span store are intact.
    pub fn attach(
        &mut self,
        start: u32,
        end: u32,
        output: &TokenForest,
        slot: FragmentSlot,
    ) -> Result<(), AdapterError> {
        self.attach_with_edit(start, end, output, slot).map(|_| ())
    }

    /// Attach syntax and retain the exact edit used to translate original
    /// declaration anchors. The receipt is produced only after successful commit.
    pub fn attach_with_edit(
        &mut self,
        start: u32,
        end: u32,
        output: &TokenForest,
        slot: FragmentSlot,
    ) -> Result<SyntaxEdit, AdapterError> {
        let call = self.select_call(start, end)?;
        let range = call.text_range();
        let target = call
            .parent()
            .filter(|node| slot.accepts_call(node) && node.text_range() == range)
            .ok_or_else(|| error("attachment grammar differs from the caller slot"))?;
        let prepared = self.prepare(output, slot.grammar())?;
        if !slot.accepts_output(&prepared.root) {
            return Err(error(
                "expansion did not produce the selected fragment grammar",
            ));
        }
        self.apply(range, prepared, |replacement| {
            replace_node(&target, replacement)
        })
    }

    /// The outermost macro-call node with exactly this range, found by the
    /// tree's binary child lookup (a path of `O(depth * log width)`), never by
    /// a walk of every earlier node.
    fn select_call(&self, start: u32, end: u32) -> Result<SyntaxNode, AdapterError> {
        if start >= end || end > u32::from(self.root.text_range().len()) {
            return Err(error("attachment range is outside the current syntax"));
        }
        let range = TextRange::new(start.into(), end.into());
        outermost_at(&self.root, range, |node| ast::MacroCall::can_cast(node.kind()))
            .ok_or_else(|| error("attachment range does not select a macro call"))
    }

    fn prepare(
        &mut self,
        output: &TokenForest,
        grammar: DeclarationGrammar,
    ) -> Result<PreparedFragment, AdapterError> {
        let retained_input_bytes = self
            .retained_input_bytes
            .checked_add(output.allocated_bytes()?)
            .ok_or_else(|| error("attachment retained input budget"))?;
        // A store shared with a seed or another fork is copied first, so a
        // failed edit can never reach another holder. A store this attachment
        // owns alone is extended in place instead of copied for every edit; a
        // failed edit may then leave unreferenced records in it, which no tree
        // refers to (records are content-addressed and only ever added).
        let (root, origins, store) = match Arc::get_mut(&mut self.store) {
            Some(owned) => {
                let store = owned
                    .get_mut()
                    .map_err(|_| error("attachment span store lock poisoned"))?;
                let (root, origins) = declaration_output::parse_root(output, grammar, store)?;
                (root, origins, None)
            }
            None => {
                let mut store = self
                    .store
                    .lock()
                    .map_err(|_| error("attachment span store lock poisoned"))?
                    .clone();
                let (root, origins) =
                    declaration_output::parse_root(output, grammar, &mut store)?;
                (root, origins, Some(store))
            }
        };
        check_tree(&root)?;
        check_origins(&origins, u32::from(root.text_range().len()))?;
        Ok(PreparedFragment {
            root,
            origins,
            store,
            retained_input_bytes,
        })
    }

    fn apply(
        &mut self,
        range: TextRange,
        prepared: PreparedFragment,
        rebuild: impl FnOnce(&SyntaxNode) -> Result<SyntaxNode, AdapterError>,
    ) -> Result<SyntaxEdit, AdapterError> {
        check_tree(&prepared.root)?;
        let replacement_len = u32::from(prepared.root.text_range().len());
        check_origins(&prepared.origins, replacement_len)?;
        let old_len = u32::from(self.root.text_range().len());
        let final_len = old_len
            .checked_sub(u32::from(range.len()))
            .and_then(|length| length.checked_add(replacement_len))
            .ok_or_else(|| error("attached syntax text budget"))?;
        let origins = splice_origins(
            &self.origins,
            range,
            &prepared.origins,
            replacement_len,
            final_len,
        )?;
        let root = rebuild(&prepared.root)?;
        // The retained tree and the replacement were each validated complete
        // (no ERROR node) and a splice adds only the replacement's nodes, so
        // the whole-tree walk is not repeated for every edit; every export of
        // the tree (`materialize`, `into_seed`) still runs it.
        if u32::from(root.text_range().len()) != final_len {
            return Err(error(
                "attached syntax length differs from the origin splice",
            ));
        }
        let edit = SyntaxEdit::new(range, replacement_len, old_len, final_len)?;
        self.root = root;
        self.origins = Arc::new(origins);
        if let Some(store) = prepared.store {
            self.store = Arc::new(Mutex::new(store));
        }
        self.retained_input_bytes = prepared.retained_input_bytes;
        self.snapshot_cache = std::sync::OnceLock::new();
        Ok(edit)
    }

    /// Content identity only, never expansion or source authority. Include
    /// structure and origin keys: display text alone does not identify a tree
    /// after fragment attachment. Pin syntax kinds to the adapter revision.
    pub fn syntax_identity(&self) -> Result<Key, AdapterError> {
        let store = self
            .store
            .lock()
            .map_err(|_| error("attachment span store lock poisoned"))?;
        let mut hash = blake3::Hasher::new();
        hash.update(b"lrr/v2/attached-syntax\0");
        hash.update(crate::UPSTREAM_COMMIT.as_bytes());
        hash.update(&[match self.grammar {
            DeclarationGrammar::Items => 0,
            DeclarationGrammar::Expression => 1,
            DeclarationGrammar::Statements => 2,
            DeclarationGrammar::Type => 3,
            DeclarationGrammar::Pattern => 4,
            DeclarationGrammar::TraitItems => 5,
            DeclarationGrammar::ImplItems => 6,
        }]);
        for event in self.root.preorder_with_tokens() {
            match event {
                WalkEvent::Enter(syntax::NodeOrToken::Node(node)) => {
                    hash.update(&[0]);
                    hash.update(&u16::from(node.kind()).to_le_bytes());
                }
                WalkEvent::Enter(syntax::NodeOrToken::Token(token)) => {
                    hash.update(&[1]);
                    hash.update(&u16::from(token.kind()).to_le_bytes());
                    hash.update(&(token.text().len() as u64).to_le_bytes());
                    hash.update(token.text().as_bytes());
                }
                WalkEvent::Leave(syntax::NodeOrToken::Node(_)) => {
                    hash.update(&[2]);
                }
                WalkEvent::Leave(syntax::NodeOrToken::Token(_)) => {}
            }
        }
        hash.update(&[3]);
        hash.update(&(self.origins.iter().count() as u64).to_le_bytes());
        for (end, origin) in self.origins.iter() {
            hash.update(&u32::from(end).to_le_bytes());
            // Encode semantic coordinates/context, never a private slot ID.
            hash.update(&store.key(origin)?.0);
        }
        Ok(Key(*hash.finalize().as_bytes()))
    }

    /// Replay these owned events; `text` is for display, not reparsing or body
    /// cache identity. Source publication must also retain admission/epoch proof.
    pub fn materialize(&self) -> Result<DeclarationSyntax, AdapterError> {
        check_tree(&self.root)?;
        let store = self
            .store
            .lock()
            .map_err(|_| error("attachment span store lock poisoned"))?;
        declaration_output::materialize_root(&self.root, &self.origins, &store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use span::{
        Edition, EditionedFileId, FileId, ROOT_ERASED_FILE_AST_ID, Span, SpanAnchor, SyntaxContext,
    };

    fn origin(id: u32) -> Span {
        Span {
            range: TextRange::empty(0.into()),
            anchor: SpanAnchor {
                file_id: EditionedFileId::new(FileId::from_raw(id), Edition::Edition2024),
                ast_id: ROOT_ERASED_FILE_AST_ID,
            },
            ctx: SyntaxContext::root(Edition::Edition2024),
        }
    }

    #[test]
    fn splice_clips_origin_intervals_at_both_non_token_boundaries() {
        let mut original = SpanMap::empty();
        for (end, id) in [(4, 1), (8, 2), (12, 3)] {
            original.push(end.into(), origin(id));
        }
        let mut replacement = SpanMap::empty();
        replacement.push(2.into(), origin(4));
        let result = splice_origins(
            &original,
            TextRange::new(3.into(), 9.into()),
            &replacement,
            2,
            8,
        )
        .unwrap();
        assert_eq!(
            result
                .iter()
                .map(|(end, span)| (u32::from(end), span))
                .collect::<Vec<_>>(),
            [(3, origin(1)), (5, origin(4)), (8, origin(3))]
        );
    }

    #[test]
    fn attachment_identity_distinguishes_structure_with_identical_text_and_origins() {
        use crate::token_wire::{
            ContextRecord, Edition as WireEdition, Key, SourceIdentity, Transparency, parse_source,
        };
        use syntax::{SyntaxKind, SyntaxTreeBuilder};
        let input = parse_source(
            "1+2*3",
            SourceIdentity {
                unit: Key([1; 32]),
                content: Key([2; 32]),
                path: "shape.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key([1; 32]),
                edition: WireEdition::E2024,
            },
            Key([3; 32]),
        )
        .unwrap();
        let original = SyntaxAttachment::parse(&input, DeclarationGrammar::Expression).unwrap();
        let mut attached = SyntaxAttachment::parse(&input, DeclarationGrammar::Expression).unwrap();
        // The same text can encode (1 + 2) * 3 after structural expansion.
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::BIN_EXPR);
        builder.start_node(SyntaxKind::BIN_EXPR);
        for token in original
            .root
            .descendants_with_tokens()
            .filter_map(|n| n.into_token())
        {
            if token.kind() == SyntaxKind::INT_NUMBER {
                builder.start_node(SyntaxKind::LITERAL);
                builder.token(token.kind(), token.text());
                builder.finish_node();
                if token.text() == "2" {
                    builder.finish_node();
                }
            } else {
                builder.token(token.kind(), token.text());
            }
        }
        builder.finish_node();
        attached.root = builder.finish().syntax_node();
        assert_eq!(
            original.root.text().to_string(),
            attached.root.text().to_string()
        );
        assert_eq!(original.origins, attached.origins);
        assert_ne!(
            original.syntax_identity().unwrap(),
            attached.syntax_identity().unwrap()
        );
    }
}
