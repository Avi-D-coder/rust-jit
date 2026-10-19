//! Actual return-position opaque syntax retained in the declaration memo.
//! Syntax occurrences are not opaque Type IDs, capture schemas or trait proofs.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::{
        ast_adapter::range,
        parse::{ParseKey, ParsedFile},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode, WalkEvent,
    ast::{self, HasTypeBounds},
};

#[derive(Debug)]
pub(crate) struct OpaqueReturnSite {
    position: u32,
    parent: Option<u32>,
    syntax: SourceRange,
    bounds: Option<SourceRange>,
    edition: u16,
}
impl OpaqueReturnSite {
    pub(crate) fn position(&self) -> u32 {
        self.position
    }
    pub(crate) fn parent(&self) -> Option<u32> {
        self.parent
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.syntax
    }
    pub(crate) fn bounds_range(&self) -> Option<SourceRange> {
        self.bounds
    }
    pub(crate) fn edition(&self) -> u16 {
        self.edition
    }
}
#[derive(Debug)]
pub(crate) struct OpaqueReturnHeaders {
    definition: Digest,
    parse: ParseKey,
    result: SourceRange,
    sites: Vec<OpaqueReturnSite>,
    macro_frontiers: Vec<SourceRange>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-OPAQUE-RETURN-SOURCE", message.into())
}
impl OpaqueReturnHeaders {
    pub(crate) fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn result_range(&self) -> SourceRange {
        self.result
    }
    pub(crate) fn sites(&self) -> &[OpaqueReturnSite] {
        &self.sites
    }
    pub(crate) fn macro_frontiers(&self) -> &[SourceRange] {
        &self.macro_frontiers
    }
    pub(crate) fn capture(
        parsed: &ParsedFile,
        definition: Digest,
        function: &ast::Fn,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(result) = function.ret_type().and_then(|r| r.ty()) else {
            return Ok(None);
        };
        let mut plan = Self {
            definition,
            parse: parsed.key(),
            result: range(result.syntax().text_range()),
            sites: Vec::new(),
            macro_frontiers: Vec::new(),
        };
        let mut parents = Vec::new();
        let mut walk = result.syntax().preorder();
        while let Some(event) = walk.next() {
            match event {
                WalkEvent::Enter(node) => {
                    // A constant argument can contain executable declarations.
                    // Those belong to their own demand, not this return Type.
                    if ast::BlockExpr::can_cast(node.kind())
                        || ast::Fn::can_cast(node.kind())
                        || ast::Const::can_cast(node.kind())
                        || ast::Static::can_cast(node.kind())
                        || ast::ClosureExpr::can_cast(node.kind())
                    {
                        walk.skip_subtree();
                        continue;
                    }
                    if ast::MacroType::can_cast(node.kind()) {
                        plan.macro_frontiers.push(range(node.text_range()));
                        walk.skip_subtree();
                        continue;
                    }
                    if let Some(opaque) = ast::ImplTraitType::cast(node) {
                        let position = u32::try_from(plan.sites.len())
                            .map_err(|_| fault("opaque return ordinal overflow"))?;
                        let keyword = opaque
                            .impl_token()
                            .ok_or_else(|| fault("opaque return lacks its impl token"))?;
                        plan.sites.push(OpaqueReturnSite {
                            position,
                            parent: parents.last().copied(),
                            syntax: range(opaque.syntax().text_range()),
                            bounds: opaque
                                .type_bound_list()
                                .map(|b| range(b.syntax().text_range())),
                            edition: parsed.token_edition(range(keyword.text_range()))?,
                        });
                        parents.push(position);
                    }
                }
                WalkEvent::Leave(node) if ast::ImplTraitType::can_cast(node.kind()) => {
                    parents
                        .pop()
                        .ok_or_else(|| fault("opaque return nesting differs"))?;
                }
                WalkEvent::Leave(_) => {}
            }
        }
        if !parents.is_empty() {
            return Err(fault("opaque return nesting is not closed"));
        }
        Ok(Some(plan))
    }
    /// Resolve directly in this exact retained parser snapshot; never reparse
    /// displayed tokens or silently attach a different source/expansion world.
    pub(super) fn syntax(
        &self,
        parsed: &ParsedFile,
        position: u32,
    ) -> Result<Option<ast::ImplTraitType>, Diagnostic> {
        if parsed.key() != self.parse {
            return Err(fault("opaque return parser snapshot differs"));
        }
        let Some(site) = usize::try_from(position)
            .ok()
            .and_then(|p| self.sites.get(p))
        else {
            return Ok(None);
        };
        parsed
            .syntax_at(site.syntax, |n| ast::ImplTraitType::can_cast(n.kind()))
            .map(|n| n.and_then(ast::ImplTraitType::cast))
    }
}
impl StableEncode for OpaqueReturnHeaders {
    const DOMAIN: &'static str = "lrr/v1/actual-opaque-return-headers";
    fn encode_fields(&self, e: &mut StableEncoder) {
        fn span(e: &mut StableEncoder, r: SourceRange) {
            e.u32(r.start);
            e.u32(r.end);
        }
        e.digest(self.definition);
        e.record(&self.parse);
        span(e, self.result);
        e.sequence(&self.sites, |e, s| {
            e.u32(s.position);
            e.option(&s.parent, |e, p| e.u32(*p));
            span(e, s.syntax);
            e.option(&s.bounds, |e, r| span(e, *r));
            e.u32(u32::from(s.edition));
        });
        e.sequence(&self.macro_frontiers, |e, r| span(e, *r));
    }
}
