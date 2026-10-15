//! Written receiver evidence only. Parsing a method never grants a library member.
use super::*;
use crate::{
    ir::pc::{NodeId, Phase},
    syntax::parse::ParseKey,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RuntimePrimitiveMethod {
    source: ParseKey,
    key: Option<PcKey>,
    span: BodySpan,
    receiver: SlotId,
    destination: SlotId,
    written_receiver: Option<PrimitiveType>,
}
impl RuntimePrimitiveMethod {
    pub(super) fn new(
        source: ParseKey,
        span: BodySpan,
        receiver: SlotId,
        destination: SlotId,
        expression: &ast::Expr,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            source,
            key: None,
            span,
            receiver,
            destination,
            written_receiver: written_receiver(expression)?,
        })
    }
    pub(super) fn bind(
        &mut self,
        index: &BodyIndex,
        key: &PcKey,
        ordinal: usize,
    ) -> Result<(), Diagnostic> {
        if self.key.is_some() || self.span.body_key != key.body || self.receiver == self.destination
        {
            return Err(invalid("primitive method source/slots differ"));
        }
        let phase = if ordinal == 0 {
            Phase::Entry
        } else {
            Phase::Operand(
                u16::try_from(ordinal - 1)
                    .map_err(|_| invalid("primitive method phase overflow"))?,
            )
        };
        let pc = index.pc(NodeId(key.node), phase)?;
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("primitive method node absent"))?;
        if self.span.relative_start < node.range.start
            || self.span.relative_end > node.range.end
            || self.span.relative_start >= self.span.relative_end
        {
            return Err(invalid("primitive method outside source macro"));
        }
        self.key = Some(pc);
        Ok(())
    }
    pub(crate) fn require(&self, source: &ParseKey, key: &PcKey) -> Result<(), Diagnostic> {
        if &self.source != source || self.key.as_ref() != Some(key) {
            return Err(invalid("primitive method source/profile/phase differs"));
        }
        Ok(())
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn receiver(&self) -> SlotId {
        self.receiver
    }
    pub(crate) fn destination(&self) -> SlotId {
        self.destination
    }
    pub(crate) fn written_receiver(&self) -> Option<PrimitiveType> {
        self.written_receiver
    }
    pub(crate) fn allocated_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>().checked_add(self.source.profile.semantics.capacity())
    }
}

/// An explicit suffix constrains this closed arithmetic tree. Unwritten numeric
/// defaults, names, calls and casts do not manufacture source receiver evidence.
fn written_receiver(expression: &ast::Expr) -> Result<Option<PrimitiveType>, Diagnostic> {
    let mut selected = None;
    for (visited, node) in expression.syntax().descendants().enumerate() {
        if visited >= 65_536 {
            return Err(invalid("primitive receiver source work limit"));
        }
        if ast::Attr::can_cast(node.kind()) {
            return Ok(None);
        }
        let Some(expression) = ast::Expr::cast(node) else {
            continue;
        };
        let suffix = match expression {
            ast::Expr::Literal(literal) => {
                let token = literal.token();
                match token.kind() {
                    SyntaxKind::INT_NUMBER => {
                        ast::IntNumber::cast(token).and_then(|n| n.suffix().map(str::to_owned))
                    }
                    SyntaxKind::FLOAT_NUMBER => {
                        ast::FloatNumber::cast(token).and_then(|n| n.suffix().map(str::to_owned))
                    }
                    _ => return Ok(None),
                }
            }
            ast::Expr::ParenExpr(_) => continue,
            ast::Expr::PrefixExpr(prefix) if prefix.op_token().is_some_and(|t| t.text() == "-") => {
                continue;
            }
            ast::Expr::BinExpr(binary)
                if binary
                    .op_token()
                    .is_some_and(|t| matches!(t.text(), "+" | "-" | "*" | "/" | "%")) =>
            {
                continue;
            }
            _ => return Ok(None),
        };
        let primitive = match suffix.as_deref() {
            Some("f32") => PrimitiveType::F32,
            Some("f64") => PrimitiveType::F64,
            None => continue,
            _ => return Ok(None),
        };
        if selected.is_some_and(|old| old != primitive) {
            return Ok(None);
        }
        selected = Some(primitive);
    }
    Ok(selected)
}
