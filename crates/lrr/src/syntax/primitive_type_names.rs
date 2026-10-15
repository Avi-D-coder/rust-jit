//! Actual reached primitive Type-name syntax and original-token attachments.
//! These source facts grant no namespace absence or execution authority.
use super::{
    ast_adapter::range,
    body_index::{BodyIndex, ExpressionKind, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
    spans::SourceRange,
    token_wire::SpanRecord,
};
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        op::{HintRecipe, PrimitiveType},
        pc::{NodeId, PcKey},
    },
};
use ra_ap_syntax::{AstNode, ast};

pub(crate) struct PrimitiveTypeNameSite {
    parse: super::parse::ParseKey,
    body: crate::source::manifest::BodyKey,
    node: u32,
    range: SourceRange,
    name: String,
    primitive: PrimitiveType,
    origins: Vec<SpanRecord>,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
impl PrimitiveTypeNameSite {
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    pub(crate) fn origins(&self) -> &[SpanRecord] {
        &self.origins
    }
    pub(crate) fn require(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        pc: &PcKey,
    ) -> Result<(), Diagnostic> {
        if self.parse != parsed.key()
            || self.body != body.key()
            || self.body != pc.body
            || self.node != pc.node
        {
            return Err(invalid(
                "Type name site belongs to another source occurrence",
            ));
        }
        Ok(())
    }
}
impl ParsedFile {
    /// A reached ordinary cast is one consumer of the shared Type-name service.
    /// Other syntax kinds retain their existing ordinary lowering path.
    pub(crate) fn retained_primitive_type_name_site(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<Option<PrimitiveTypeNameSite>, Diagnostic> {
        if self.generated_origin.is_none() {
            return Ok(None);
        }
        if body.key() != *index.body() || pc.body != body.key() {
            return Err(invalid("Type name owning body differs"));
        }
        index.pc(NodeId(pc.node), crate::ir::pc::Phase::from_code(pc.phase))?;
        let node = &index.nodes()[pc.node as usize];
        if node.role != NodeRole::Expression(ExpressionKind::Other) {
            return Ok(None);
        }
        let selected = self
            .entered_node_syntax(body, index, NodeId(pc.node))?
            .and_then(ast::CastExpr::cast)
            .and_then(|cast| cast.ty());
        let Some(ty) = selected else {
            return Ok(None);
        };
        let site = range(ty.syntax().text_range());
        if site.is_empty() || site.len() > 32 {
            return Ok(None);
        }
        let Some(HintRecipe::Primitive(primitive)) = super::thunks::hint(Some(ty.clone())) else {
            return Ok(None);
        };
        if primitive == PrimitiveType::Unit {
            return Ok(None);
        }
        let Some(origins) = self.retained_source_spans(site)? else {
            return Ok(None);
        };
        Ok(Some(PrimitiveTypeNameSite {
            parse: self.key(),
            body: body.key(),
            node: pc.node,
            range: site,
            name: ty.syntax().text().to_string(),
            primitive,
            origins,
        }))
    }
    /// Confirm this is actual original Type syntax inside the selected original
    /// body, rather than tokens in a macro definition/argument or another member.
    pub(crate) fn require_original_type_name(
        &self,
        body: &BodyHandle,
        source_range: SourceRange,
        name: &str,
    ) -> Result<(), Diagnostic> {
        if self.generated_origin.is_some()
            || !body.range.contains(source_range)
            || source_range.len() > 32
        {
            return Err(invalid("Type name original source attachment differs"));
        }
        self.entered_body_root(body)?;
        if self
            .syntax_at(source_range, |syntax| {
                ast::Type::can_cast(syntax.kind()) && syntax.text() == name
            })?
            .is_some()
        {
            return Ok(());
        }
        Err(invalid(
            "Type name has no actual original Type syntax at its origin",
        ))
    }
}
