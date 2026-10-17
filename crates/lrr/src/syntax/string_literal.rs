//! Opaque intrinsic source receipt for one actual ordinary indexed String literal.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::{
        op::{LiteralKind, LiteralRecipe},
        pc::{NodeId, PcKey, SlotId},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        spans::BodySpan,
    },
    traits::term::{RegionTerm, TyTerm},
};
use ra_ap_syntax::{AstNode, AstToken, ast};
use std::sync::Arc;
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
/// Construction is confined to actual parser-owned ordinary literal Entry.
/// No public byte/source/descriptor constructor and no runtime parser retained.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedStringLiteral {
    entry: PcKey,
    profile: crate::core::profile::ProfileIdentity,
    structure: Digest,
    span: BodySpan,
    destination: SlotId,
    edition: u16,
    spelling: Digest,
    utf8: String,
    source: TyTerm,
}
#[cfg(test)]
thread_local! { static DECODINGS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn observe_decoding<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let before = DECODINGS.with(|n| n.get());
    let result = work();
    (result, DECODINGS.with(|n| n.get() - before))
}
impl PreparedStringLiteral {
    pub(in crate::syntax::thunks) fn capture(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        entry: &PcKey,
        literal: &ast::Literal,
        destination: SlotId,
    ) -> Result<Arc<Self>, Diagnostic> {
        let actual = parsed
            .entered_node_syntax(body, index, NodeId(entry.node))?
            .and_then(ast::Literal::cast)
            .ok_or_else(|| invalid("string literal source attachment absent"))?;
        if actual.syntax() != literal.syntax()
            || entry.body != body.key()
            || index.body() != &entry.body
            || parsed.key().profile != *index.profile()
        {
            return Err(invalid("string literal owning source/body/profile differs"));
        }
        let token = ast::String::cast(literal.token())
            .ok_or_else(|| invalid("string receipt requires actual String token"))?;
        let edition = parsed.token_edition(crate::syntax::ast_adapter::range(
            token.syntax().text_range(),
        ))?;
        #[cfg(test)]
        DECODINGS.with(|n| n.set(n.get().checked_add(1).unwrap()));
        let value = token
            .value()
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-LITERAL",
                    "invalid pinned string literal decoding".into(),
                )
            })?
            .into_owned();
        let proof = Self {
            entry: entry.clone(),
            profile: index.profile().clone(),
            structure: index.semantic_identity(),
            span: index.span(NodeId(entry.node))?,
            destination,
            edition,
            spelling: Digest::of_bytes(token.syntax().text().as_bytes()),
            utf8: value,
            source: TyTerm::Ref(RegionTerm::Static, false, Box::new(TyTerm::Str)),
        };
        proof.require_entry(index, entry, destination, &proof.span)?;
        Ok(Arc::new(proof))
    }
    pub(crate) fn require_entry(
        &self,
        index: &BodyIndex,
        entry: &PcKey,
        destination: SlotId,
        span: &BodySpan,
    ) -> Result<(), Diagnostic> {
        let node = index
            .nodes()
            .get(entry.node as usize)
            .ok_or_else(|| invalid("string literal indexed node absent"))?;
        if self.entry != *entry
            || self.span != *span
            || self.destination != destination
            || entry.phase != 0
            || entry.body != *index.body()
            || self.profile != *index.profile()
            || self.structure != index.semantic_identity()
            || node.role != NodeRole::Expression(ExpressionKind::Literal)
            || node.value != Some(destination)
            || index.span(node.id)? != *span
            || !matches!(self.edition, 2015 | 2018 | 2021 | 2024)
        {
            return Err(invalid(
                "string literal source entry/span/destination/edition differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn require_recipe(
        &self,
        index: &BodyIndex,
        entry: &PcKey,
        literal: &LiteralRecipe,
        destination: SlotId,
    ) -> Result<(), Diagnostic> {
        self.require_entry(index, entry, destination, &literal.source)?;
        if literal.kind != LiteralKind::String
            || Digest::of_bytes(literal.spelling.as_bytes()) != self.spelling
        {
            return Err(invalid(
                "string literal receipt cannot relabel spelling or literal kind",
            ));
        }
        Ok(())
    }
    pub(crate) fn utf8(&self) -> &str {
        &self.utf8
    }
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(2 * std::mem::size_of::<usize>())?
            .checked_add(self.utf8.capacity())?
            .checked_add(self.profile.semantics.capacity())?
            .checked_add(self.source.heap_bytes()?)
    }
}
impl StableEncode for PreparedStringLiteral {
    const DOMAIN: &'static str = "lrr/v5/ordinary-string-literal-source-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.entry);
        e.record(&self.profile);
        e.digest(self.structure);
        e.record(&self.span.body_key);
        e.u32(self.span.relative_start);
        e.u32(self.span.relative_end);
        e.u32(self.destination.0);
        e.u32(u32::from(self.edition));
        e.digest(self.spelling);
        e.string(&self.utf8);
        e.record(&self.source);
    }
}
