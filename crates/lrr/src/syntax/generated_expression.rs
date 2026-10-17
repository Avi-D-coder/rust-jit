//! Reached expression decoding from pinned owned syntax, never printed-text parsing.
use crate::{
    core::diagnostic::Diagnostic,
    expand::identity::GrammarSlot,
    ir::op::LiteralRecipe,
    syntax::{
        parse::ParsedFile,
        spans::{BodySpan, TokenOrigin},
        token_wire::{Key, TokenForest},
    },
};
use ra_ap_syntax::{AstNode, ast};
pub(crate) struct GeneratedLiteral {
    literal: LiteralRecipe,
    tokens: TokenForest,
    origins: Vec<TokenOrigin>,
    expansion: crate::core::digest::Digest,
    dependency: crate::db::dependency::DependencyStamp,
    caller: crate::ir::pc::PcKey,
}
pub(crate) fn literal(
    input: &crate::db::dependency::InputValue,
    source: BodySpan,
    caller: &crate::ir::pc::PcKey,
) -> Result<GeneratedLiteral, Diagnostic> {
    use crate::core::digest::StableEncode;
    if source.body_key != caller.body || caller.phase != crate::ir::pc::Phase::Entry.code()? {
        return Err(Diagnostic::new(
            "LRR-E-EXPANSION-INPUT",
            "literal witness caller/body/phase differs".into(),
        ));
    }
    let crate::db::dependency::InputValue::ExpansionRecord(record) = input else {
        return Err(Diagnostic::new(
            "LRR-E-EXPANSION-INPUT",
            "literal expansion record input differs".into(),
        ));
    };
    if record.key().grammar != GrammarSlot::Expression
        || !matches!(
            record.key().macro_identity,
            crate::expand::identity::MacroIdentity::Declaration { .. }
        )
    {
        return Err(Diagnostic::new(
            "LRR-E-EXPANSION-INPUT",
            "literal expansion record grammar/identity differs".into(),
        ));
    }
    let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(record.wire())
        .map_err(|error| Diagnostic::new(error.code, error.message))?;
    let output = lrr_expand_adapter::declaration_output::materialize(
        &adapter,
        lrr_expand_adapter::declaration_output::DeclarationGrammar::Expression,
    )
    .map_err(|error| Diagnostic::new(error.code, error.message))?;
    let parsed = ParsedFile::from_generated_declarations(
        &output.syntax,
        output.text.len(),
        record.descriptor().fingerprint(),
        record.key().call_edition,
        GrammarSlot::Expression,
    )?
    .with_generated_token_editions(&adapter, &output.origins)?;
    let tree = parsed.parsed.tree();
    let mut expressions = tree.syntax().children().filter_map(ast::Expr::cast);
    let mut expression = expressions.next().ok_or_else(|| {
        Diagnostic::new(
            "LRR-E-MACRO-OUTPUT",
            "generated expression root absent".into(),
        )
    })?;
    if expressions.next().is_some() {
        return Err(Diagnostic::new(
            "LRR-E-MACRO-OUTPUT",
            "generated expression roots differ".into(),
        ));
    }
    loop {
        match expression {
            ast::Expr::Literal(literal) => {
                let mut origins = Vec::new();
                origins
                    .try_reserve_exact(output.origins.len())
                    .map_err(|_| {
                        Diagnostic::new(
                            "LRR-E-METADATA-LIMIT",
                            "expression origin allocation".into(),
                        )
                    })?;
                origins.extend(output.origins.into_iter().map(|origin| TokenOrigin {
                    start: origin.start,
                    end: origin.end,
                    span: Key(origin.span.0),
                }));
                return Ok(GeneratedLiteral {
                    literal: super::thunks::literal_recipe(&literal, source)?,
                    tokens: record.tokens()?,
                    origins,
                    caller: caller.clone(),
                    expansion: record.descriptor().fingerprint(),
                    dependency: crate::db::dependency::DependencyStamp::Input {
                        key: crate::db::dependency::InputKey::ExpansionRecord(
                            record.key().fingerprint(),
                        ),
                        fingerprint: input.fingerprint(),
                    },
                });
            }
            ast::Expr::ParenExpr(paren) => {
                expression = paren.expr().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-MACRO-OUTPUT",
                        "generated parenthesized expression absent".into(),
                    )
                })?;
            }
            _ => return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "reached generated expression requires pending non-literal lowering integration"
                    .into(),
            )),
        }
    }
}

impl GeneratedLiteral {
    pub(crate) fn caller(&self) -> &crate::ir::pc::PcKey {
        &self.caller
    }
    pub(crate) fn literal(&self) -> &LiteralRecipe {
        &self.literal
    }
    pub(crate) fn expansion(&self) -> crate::core::digest::Digest {
        self.expansion
    }
    pub(crate) fn dependency(&self) -> &crate::db::dependency::DependencyStamp {
        &self.dependency
    }
    pub(crate) fn into_provenance(self) -> (TokenForest, Vec<TokenOrigin>) {
        (self.tokens, self.origins)
    }
}
