//! Match original lexical occurrences to pinned syntax coordinates without reparsing.
use crate::AdapterError;
use std::collections::{BTreeMap, BTreeSet};
use syntax::{NodeOrToken, SyntaxNode, WalkEvent};
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
struct Unit {
    text: String,
    leaf: Option<tt::Leaf>,
}
pub(super) struct Leaves {
    insert: BTreeMap<u32, Vec<tt::Leaf>>,
    suppress: BTreeSet<u32>,
}
impl Leaves {
    pub(super) fn new(tree: &tt::TopSubtree, root: &SyntaxNode) -> Result<Self, AdapterError> {
        let mut units = Vec::new();
        let mut bytes = 0usize;
        flatten(
            tree.top_subtree().delimiter,
            tree.iter(),
            &mut units,
            &mut bytes,
            0,
        )?;
        let mut insert: BTreeMap<u32, Vec<tt::Leaf>> = BTreeMap::new();
        let mut suppress = BTreeSet::new();
        let mut at = 0;
        let mut offset = 0;
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia())
        {
            let start = u32::from(token.text_range().start());
            let mut position = 0;
            while position < token.text().len() {
                let unit = units.get(at).ok_or_else(|| {
                    fault("generated derive syntax has no original lexical occurrence")
                })?;
                let amount = (unit.text.len() - offset).min(token.text().len() - position);
                if amount == 0
                    || unit.text.as_bytes()[offset..offset + amount]
                        != token.text().as_bytes()[position..position + amount]
                {
                    return Err(fault("generated derive syntax/original spelling mismatch"));
                }
                if let Some(leaf) = &unit.leaf {
                    suppress.insert(start);
                    if offset == 0 {
                        let leaves = insert.entry(start).or_default();
                        leaves
                            .try_reserve(1)
                            .map_err(|_| fault("generated derive original leaf allocation"))?;
                        leaves.push(leaf.clone());
                    }
                }
                offset += amount;
                position += amount;
                if offset == unit.text.len() {
                    offset = 0;
                    at += 1;
                }
            }
        }
        if at != units.len() || offset != 0 {
            return Err(fault(
                "generated derive original lexical occurrences remain unmatched",
            ));
        }
        Ok(Self { insert, suppress })
    }
    pub(super) fn event(&self, event: &WalkEvent<syntax::SyntaxElement>) -> Option<Vec<tt::Leaf>> {
        let WalkEvent::Enter(NodeOrToken::Token(token)) = event else {
            return None;
        };
        let start = u32::from(token.text_range().start());
        self.suppress
            .contains(&start)
            .then(|| self.insert.get(&start).cloned().unwrap_or_default())
    }
}
fn push(
    units: &mut Vec<Unit>,
    bytes: &mut usize,
    text: String,
    leaf: Option<tt::Leaf>,
) -> Result<(), AdapterError> {
    *bytes = bytes
        .checked_add(text.capacity())
        .and_then(|n| n.checked_add(128))
        .filter(|n| *n <= 128 * 1024 * 1024)
        .ok_or_else(|| fault("generated derive original lexical retention limit"))?;
    if units.len() >= 2_000_000 {
        return Err(fault("generated derive original lexical count limit"));
    }
    units
        .try_reserve(1)
        .map_err(|_| fault("generated derive original lexical allocation"))?;
    units.push(Unit { text, leaf });
    Ok(())
}
fn flatten(
    delimiter: tt::Delimiter,
    iter: tt::TtIter<'_>,
    units: &mut Vec<Unit>,
    bytes: &mut usize,
    depth: usize,
) -> Result<(), AdapterError> {
    if depth > 256 {
        return Err(fault("generated derive original lexical depth limit"));
    }
    let delimiter = match delimiter.kind {
        tt::DelimiterKind::Parenthesis => Some(("(", ")")),
        tt::DelimiterKind::Brace => Some(("{", "}")),
        tt::DelimiterKind::Bracket => Some(("[", "]")),
        tt::DelimiterKind::Invisible => None,
    };
    if let Some((open, _)) = delimiter {
        push(units, bytes, open.into(), None)?;
    }
    for token in iter {
        match token {
            tt::TtElement::Leaf(leaf) => {
                let text = match &leaf {
                    tt::Leaf::Ident(i) => format!(
                        "{}{}",
                        if i.is_raw.yes() { "r#" } else { "" },
                        i.sym.as_str()
                    ),
                    tt::Leaf::Literal(l) => l.text().to_string(),
                    tt::Leaf::Punct(p) => p.char.to_string(),
                };
                push(units, bytes, text, Some(leaf.clone()))?;
            }
            tt::TtElement::Subtree(child, children) => {
                flatten(child.delimiter, children, units, bytes, depth + 1)?
            }
        }
    }
    if let Some((_, close)) = delimiter {
        push(units, bytes, close.into(), None)?;
    }
    Ok(())
}
