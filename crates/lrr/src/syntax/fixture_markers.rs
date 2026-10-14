//! Fixture metadata over pinned syntax. Owned ranges leave this boundary;
//! token-tree arguments remain an explicit expansion demand, never guessed spans.
use super::{
    ast_adapter::range,
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{StableEncode, StableEncoder},
};
use ra_ap_syntax::{AstNode, SyntaxKind, ast};
use std::collections::BTreeMap;
const MAX_METADATA: usize = 32 * 1024 * 1024;
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-FIXTURE-MARKER", message.into())
}
fn charge(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= MAX_METADATA)
        .ok_or_else(|| fault("fixture marker metadata limit"))?;
    Ok(())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MarkerTarget {
    Expression(SourceRange),
    NeedsMacroExpansion { invocation: SourceRange },
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FixtureMarker {
    pub name: String,
    pub comment: SourceRange,
    pub target: MarkerTarget,
}
pub struct FixtureMarkers {
    source: ParseKey,
    markers: BTreeMap<String, FixtureMarker>,
}
impl FixtureMarkers {
    pub fn source(&self) -> &ParseKey {
        &self.source
    }
    pub fn get(&self, name: &str) -> Option<&FixtureMarker> {
        self.markers.get(name)
    }
    pub fn markers(&self) -> impl Iterator<Item = &FixtureMarker> {
        self.markers.values()
    }
    pub fn require_source(&self, source: &ParseKey) -> Result<(), Diagnostic> {
        if &self.source != source {
            return Err(fault("fixture marker source/profile is stale"));
        }
        Ok(())
    }
}
impl StableEncode for FixtureMarkers {
    const DOMAIN: &'static str = "lrr/v2/fixture-markers";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.u64(self.markers.len() as u64);
        for m in self.markers.values() {
            e.string(&m.name);
            e.u32(m.comment.start);
            e.u32(m.comment.end);
            let (tag, r) = match m.target {
                MarkerTarget::Expression(r) => (0, r),
                MarkerTarget::NeedsMacroExpansion { invocation } => (1, invocation),
            };
            e.u32(tag);
            e.u32(r.start);
            e.u32(r.end);
        }
    }
}
impl ParsedFile {
    pub fn fixture_markers(&self) -> Result<FixtureMarkers, Diagnostic> {
        let tree = self.parsed.tree();
        let mut bytes = 0usize;
        let mut expressions: BTreeMap<u32, SourceRange> = BTreeMap::new();
        let mut count = 0usize;
        for expr in tree.syntax().descendants().filter_map(ast::Expr::cast) {
            count = count
                .checked_add(1)
                .ok_or_else(|| fault("fixture expression count overflow"))?;
            if count > 100_000 {
                return Err(fault("fixture expression count limit"));
            }
            let r = range(expr.syntax().text_range());
            if let Some(old) = expressions.get_mut(&r.start) {
                if r.end > old.end {
                    *old = r;
                }
            } else {
                charge(&mut bytes, 128)?;
                expressions.insert(r.start, r);
            }
        }
        if self.errors.len() > 100_000 {
            return Err(fault("fixture parse-error count limit"));
        }
        let mut errors: BTreeMap<u32, u32> = BTreeMap::new();
        for error in &self.errors {
            if let Some(end) = errors.get_mut(&error.range.start) {
                *end = (*end).max(error.range.end);
            } else {
                charge(&mut bytes, 128)?;
                errors.insert(error.range.start, error.range.end);
            }
        }
        let mut prefix_end = 0;
        for end in errors.values_mut() {
            prefix_end = prefix_end.max(*end);
            *end = prefix_end;
        }
        let mut tokens = Vec::new();
        for token in tree
            .syntax()
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if tokens.len() >= 500_000 {
                return Err(fault("fixture token count limit"));
            }
            if tokens.len() == tokens.capacity() {
                let before = tokens.capacity();
                tokens
                    .try_reserve_exact(1024)
                    .map_err(|_| fault("fixture token allocation failed"))?;
                charge(
                    &mut bytes,
                    (tokens.capacity() - before)
                        .checked_mul(std::mem::size_of_val(&token))
                        .ok_or_else(|| fault("fixture token size overflow"))?,
                )?;
            }
            tokens.push(token);
        }
        let mut markers = BTreeMap::new();
        let mut following = None;
        for token in tokens.into_iter().rev() {
            if !token.kind().is_trivia() {
                following = Some(token.clone());
            }
            if token.kind() != SyntaxKind::COMMENT {
                continue;
            }
            let Some(text) = token.text().strip_prefix("/*lrr-marker:") else {
                continue;
            };
            let name = text
                .strip_suffix("*/")
                .ok_or_else(|| fault("unterminated fixture marker"))?;
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                return Err(fault("invalid fixture marker name"));
            }
            if markers.len() >= 10_000 {
                return Err(fault("fixture marker count limit"));
            }
            if markers.contains_key(name) {
                return Err(fault(format!("duplicate fixture marker: {name}")));
            }
            let next = following.as_ref().ok_or_else(|| {
                fault(format!(
                    "fixture marker has no following expression: {name}"
                ))
            })?;
            let start = u32::from(next.text_range().start());
            let target = if let Some(r) = expressions.get(&start) {
                if errors
                    .range(..=r.end)
                    .next_back()
                    .is_some_and(|(_, end)| *end >= r.start)
                {
                    return Err(fault(
                        "fixture marker expression has unreliable parse boundaries",
                    ));
                }
                MarkerTarget::Expression(*r)
            } else if let Some(call) = token
                .parent()
                .into_iter()
                .flat_map(|p| p.ancestors())
                .find_map(ast::MacroCall::cast)
            {
                MarkerTarget::NeedsMacroExpansion {
                    invocation: range(call.syntax().text_range()),
                }
            } else {
                return Err(fault(format!(
                    "fixture marker is not followed by an expression: {name}"
                )));
            };
            charge(
                &mut bytes,
                name.len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(|| fault("fixture marker size overflow"))?,
            )?;
            markers.insert(
                name.to_owned(),
                FixtureMarker {
                    name: name.to_owned(),
                    comment: range(token.text_range()),
                    target,
                },
            );
        }
        Ok(FixtureMarkers {
            source: self.key(),
            markers,
        })
    }
}
