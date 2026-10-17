//! Closed reached control syntax from an immutable declarative statement record.
//! No caller label is resolved here. Spelling and span facts grant no target authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::dependency::{DependencyStamp, InputKey, InputValue},
    expand::identity::{GrammarSlot, MacroIdentity},
    ir::{
        op::LiteralRecipe,
        pc::{NodeId, PcKey, Phase},
    },
    syntax::{
        body_index::BodyIndex,
        outline::BodyHandle,
        parse::ParsedFile,
        spans::{BodySpan, SourceRange, TokenOrigin},
        token_wire::{Key, SourceIdentity, SpanRecord, TokenForest},
    },
};
use ra_ap_syntax::{
    AstNode, SyntaxKind,
    ast::{self, HasLoopBody},
};

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated break metadata/work budget".into(),
    )
}
fn text(value: &str) -> Result<String, Diagnostic> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| limit())?;
    output.push_str(value);
    Ok(output)
}

/// Actual lifetime token origin. This is evidence to the source/hygiene selector,
/// not a declaration lookup result or a certificate that a caller label exists.
pub(crate) struct GeneratedLifetime {
    spelling: String,
    generated_range: SourceRange,
    span_key: Key,
    span: SpanRecord,
}
impl GeneratedLifetime {
    pub(crate) fn spelling(&self) -> &str {
        &self.spelling
    }
    pub(crate) fn generated_range(&self) -> SourceRange {
        self.generated_range
    }
    pub(crate) fn span_key(&self) -> Key {
        self.span_key
    }
    pub(crate) fn span(&self) -> &SpanRecord {
        &self.span
    }
    pub(crate) fn context(&self) -> Key {
        self.span.context
    }
}

/// Privately decoded original output. Retains the owned pinned MacroStmts arena
/// through source selection; executable admission still requires a separate proof.
pub(crate) struct GeneratedBreak {
    parsed: ParsedFile,
    caller: PcKey,
    span: BodySpan,
    lifetime: Option<GeneratedLifetime>,
    payload: Option<LiteralRecipe>,
    tokens: TokenForest,
    origins: Vec<TokenOrigin>,
    expansion: Digest,
    dependency: DependencyStamp,
    call_edition: u16,
    definition_edition: u16,
}
impl GeneratedBreak {
    pub(crate) fn caller(&self) -> &PcKey {
        &self.caller
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn lifetime(&self) -> Option<&GeneratedLifetime> {
        self.lifetime.as_ref()
    }
    pub(crate) fn payload(&self) -> Option<&LiteralRecipe> {
        self.payload.as_ref()
    }
    pub(crate) fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
    pub(crate) fn origins(&self) -> &[TokenOrigin] {
        &self.origins
    }
    pub(crate) fn expansion(&self) -> Digest {
        self.expansion
    }
    pub(crate) fn dependency(&self) -> &DependencyStamp {
        &self.dependency
    }
    pub(crate) fn call_edition(&self) -> u16 {
        self.call_edition
    }
    pub(crate) fn definition_edition(&self) -> u16 {
        self.definition_edition
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let literal = if let Some(literal) = &self.payload {
            literal
                .spelling
                .capacity()
                .checked_add(match &literal.decoded {
                    Some(crate::ir::op::DecodedLiteral::Float { decimal, .. }) => {
                        decimal.capacity()
                    }
                    _ => 0,
                })
                .ok_or_else(limit)?
        } else {
            0
        };
        let lifetime = if let Some(lifetime) = &self.lifetime {
            lifetime
                .spelling
                .capacity()
                .checked_add(lifetime.span.source.path.capacity())
                .ok_or_else(limit)?
        } else {
            0
        };
        let token_bytes = self
            .tokens
            .allocated_bytes()
            .map_err(|error| Diagnostic::new(error.code, error.message))?;
        std::mem::size_of::<Self>()
            .checked_add(self.parsed.estimated_bytes())
            .and_then(|n| n.checked_add(token_bytes))
            .and_then(|n| {
                n.checked_add(
                    self.origins
                        .capacity()
                        .checked_mul(std::mem::size_of::<TokenOrigin>())?,
                )
            })
            .and_then(|n| n.checked_add(literal))
            .and_then(|n| n.checked_add(lifetime))
            .ok_or_else(limit)
    }
    pub(crate) fn into_provenance(self) -> (TokenForest, Vec<TokenOrigin>) {
        (self.tokens, self.origins)
    }
}

fn scalar(mut expression: ast::Expr, span: &BodySpan) -> Result<LiteralRecipe, Diagnostic> {
    for _ in 0usize.. {
        match expression {
            ast::Expr::ParenExpr(group) => {
                expression = group
                    .expr()
                    .ok_or_else(|| invalid("generated break parenthesized payload absent"))?;
            }
            ast::Expr::Literal(literal) => {
                if !matches!(
                    literal.token().kind(),
                    SyntaxKind::INT_NUMBER
                        | SyntaxKind::FLOAT_NUMBER
                        | SyntaxKind::CHAR
                        | SyntaxKind::BYTE
                        | SyntaxKind::TRUE_KW
                        | SyntaxKind::FALSE_KW
                ) {
                    return Err(unsupported(
                        "generated break requires a bounded scalar literal payload",
                    ));
                }
                return super::thunks::literal_recipe(&literal, span.clone());
            }
            _ => {
                return Err(unsupported(
                    "generated break payload requires scalar literal/parenthesis integration",
                ));
            }
        }
    }
    Err(limit())
}

/// Decode only a schema-checked record's owned syntax; this does not certify its
/// publication/world or caller target. The coordinator/frontend must replay them.
/// None means no direct break candidate, never permission to execute the output.
pub(crate) fn try_decode_break(
    input: &InputValue,
    source: BodySpan,
    caller: &PcKey,
) -> Result<Option<GeneratedBreak>, Diagnostic> {
    if source.body_key != caller.body
        || source.relative_start > source.relative_end
        || caller.phase != Phase::Entry.code()?
    {
        return Err(invalid("generated break caller/body/phase differs"));
    }
    let InputValue::ExpansionRecord(record) = input else {
        return Err(invalid("generated break input is not an expansion record"));
    };
    record.key().validate()?;
    if record.key().grammar != GrammarSlot::Statements || !matches!(
            record.key().macro_identity,
            MacroIdentity::Declaration { .. }
        ) {
        return Err(invalid(
            "generated break record grammar/identity/bound differs",
        ));
    }
    let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(record.wire())
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
    let output = lrr_expand_adapter::declaration_output::materialize(
        &adapter,
        lrr_expand_adapter::declaration_output::DeclarationGrammar::Statements,
    )
    .map_err(|e| Diagnostic::new(e.code, e.message))?;
    output
        .syntax
        .len()
        .checked_mul(32)
        .and_then(|n| n.checked_add(output.text.len()))
        .and_then(|n| {
            n.checked_add(
                output
                    .origins
                    .len()
                    .checked_mul(std::mem::size_of::<TokenOrigin>())?,
            )
        })
        .ok_or_else(limit)?;
    let parsed = ParsedFile::from_generated_declarations(
        &output.syntax,
        output.text.len(),
        record.descriptor().fingerprint(),
        record.key().call_edition,
        GrammarSlot::Statements,
    )?
    .with_generated_token_editions(&adapter, &output.origins)?;
    let tree = parsed.parsed.tree();
    let mut roots = tree.syntax().children();
    let root = roots
        .next()
        .filter(|root| root.kind() == SyntaxKind::MACRO_STMTS)
        .ok_or_else(|| invalid("generated break MacroStmts root absent"))?;
    if roots.next().is_some() {
        return Err(unsupported("generated break output has multiple roots"));
    }
    // Applicability is structural, not an Unsupported diagnostic recovery.
    // A nested break in a block/function belongs to that unimplemented source
    // route; it is never promoted to a break of the original caller frame.
    let mut count = 0usize;
    let mut broken = None;
    for statement in root.children() {
        count = count
            .checked_add(1)
            .ok_or_else(limit)?;
        if statement.kind() == SyntaxKind::ERROR {
            return Err(unsupported(
                "generated break output has malformed statement syntax",
            ));
        }
        let expression = if let Some(statement) = ast::ExprStmt::cast(statement.clone()) {
            statement.expr()
        } else {
            ast::Expr::cast(statement)
        };
        if let Some(ast::Expr::BreakExpr(candidate)) = expression {
            broken = Some(candidate);
        }
    }
    let Some(broken) = broken else {
        return Ok(None);
    };
    if count != 1 {
        return Err(unsupported(
            "generated break output must contain exactly one statement",
        ));
    }
    if !output.definitions.is_empty() || !output.calls.is_empty() {
        return Err(unsupported(
            "generated break output contains a declaration or nested macro",
        ));
    }
    let mut work = 0usize;
    for node in broken.syntax().descendants() {
        work = work
            .checked_add(1)
            .ok_or_else(limit)?;
        if node.kind() == SyntaxKind::ATTR {
            return Err(unsupported(
                "attributed generated break requires declaration preparation",
            ));
        }
    }
    let payload = broken
        .expr()
        .map(|expr| scalar(expr, &source))
        .transpose()?;
    let tokens = record.tokens()?;
    let lifetime = broken
        .lifetime()
        .map(|lifetime| -> Result<GeneratedLifetime, Diagnostic> {
            let token = lifetime
                .lifetime_ident_token()
                .ok_or_else(|| invalid("generated break lifetime identifier absent"))?;
            let range = token.text_range();
            let selected = SourceRange {
                start: u32::from(range.start()),
                end: u32::from(range.end()),
            };
            let mut primary: Option<(Key, &SpanRecord)> = None;
            let mut pieces = 0usize;
            for origin in &output.origins {
                if origin.end <= selected.start || origin.start >= selected.end {
                    continue;
                }
                let span_key = Key(origin.span.0);
                pieces = pieces
                    .checked_add(1)
                    .ok_or_else(limit)?;
                let position = tokens
                    .spans()
                    .binary_search_by_key(&span_key, SpanRecord::key)
                    .map_err(|_| invalid("generated lifetime origin span absent"))?;
                let span = &tokens.spans()[position];
                if let Some((_, prior)) = primary
                    && (prior.context != span.context || prior.edition != span.edition)
                {
                    return Err(unsupported(
                        "generated lifetime has mixed local hygiene/edition origins",
                    ));
                }
                primary = Some((span_key, span));
            }
            let (span_key, span): (Key, &SpanRecord) =
                primary.ok_or_else(|| invalid("generated lifetime origin coverage absent"))?;
            let retained = SpanRecord {
                source: SourceIdentity {
                    unit: span.source.unit,
                    content: span.source.content,
                    path: text(&span.source.path)?,
                },
                start: span.start,
                end: span.end,
                context: span.context,
                edition: span.edition,
                origin: span.origin,
            };
            if retained.key() != span_key {
                return Err(invalid("generated lifetime retained span differs"));
            }
            Ok(GeneratedLifetime {
                spelling: text(token.text())?,
                generated_range: selected,
                span_key,
                span: retained,
            })
        })
        .transpose()?;
    let mut origins = Vec::new();
    origins
        .try_reserve_exact(output.origins.len())
        .map_err(|_| limit())?;
    origins.extend(output.origins.into_iter().map(|origin| TokenOrigin {
        start: origin.start,
        end: origin.end,
        span: Key(origin.span.0),
    }));
    let decoded = GeneratedBreak {
        parsed,
        caller: caller.clone(),
        span: source,
        lifetime,
        payload,
        tokens,
        origins,
        expansion: record.descriptor().fingerprint(),
        dependency: DependencyStamp::Input {
            key: InputKey::ExpansionRecord(record.key().fingerprint()),
            fingerprint: input.fingerprint(),
        },
        call_edition: record.key().call_edition,
        definition_edition: record.key().definition_edition,
    };
    decoded.allocated_bytes()?;
    Ok(Some(decoded))
}

/// Strict decoder controls retain their original closed-break expectation.
#[cfg(test)]
pub(crate) fn decode_break(
    input: &InputValue,
    source: BodySpan,
    caller: &PcKey,
) -> Result<GeneratedBreak, Diagnostic> {
    try_decode_break(input, source, caller)?
        .ok_or_else(|| unsupported("generated statement is not a break"))
}

/// Original-source label token range only. The frontend selects the target and
/// validates its hygiene; syntax facts alone never grant control authority.
pub(crate) fn target_label_range(
    parsed: &ParsedFile,
    handle: &BodyHandle,
    index: &BodyIndex,
    target: NodeId,
) -> Result<SourceRange, Diagnostic> {
    if index.body() != &handle.key()
        || index.profile() != &parsed.profile
        || !index.labels().iter().any(|label| label.node == target)
    {
        return Err(invalid("generated break target body/profile/label differs"));
    }
    let node = index
        .nodes()
        .get(target.0 as usize)
        .ok_or_else(|| invalid("generated break target node absent"))?;
    let start = handle
        .range
        .start
        .checked_add(node.range.start)
        .ok_or_else(limit)?;
    let end = handle
        .range
        .start
        .checked_add(node.range.end)
        .ok_or_else(limit)?;
    let root = parsed.entered_body_root(handle)?;
    for syntax in root.descendants() {
        if u32::from(syntax.text_range().start()) != start
            || u32::from(syntax.text_range().end()) != end
        {
            continue;
        }
        let lifetime = match ast::Expr::cast(syntax) {
            Some(ast::Expr::BlockExpr(e)) => e.label().and_then(|l| l.lifetime()),
            Some(ast::Expr::LoopExpr(e)) => e.label().and_then(|l| l.lifetime()),
            Some(ast::Expr::WhileExpr(e)) => e.label().and_then(|l| l.lifetime()),
            Some(ast::Expr::ForExpr(e)) => e.label().and_then(|l| l.lifetime()),
            _ => None,
        };
        if let Some(lifetime) = lifetime {
            let range = lifetime.syntax().text_range();
            return SourceRange::checked(u32::from(range.start()), u32::from(range.end()));
        }
    }
    Err(invalid(
        "generated break original target label syntax absent",
    ))
}

/// Read-only original-source continuation selection. Target admission belongs to
/// the frontend; this helper only reuses the existing ordinary continuation rule.
pub(crate) fn target_continuation(
    parsed: &ParsedFile,
    handle: &BodyHandle,
    index: &BodyIndex,
    target: NodeId,
) -> Result<PcKey, Diagnostic> {
    if index.body() != &handle.key()
        || index.profile() != &parsed.profile
        || !index.labels().iter().any(|label| label.node == target)
    {
        return Err(invalid("generated break target body/profile/label differs"));
    }
    let node = index
        .nodes()
        .get(target.0 as usize)
        .ok_or_else(|| invalid("generated break target node absent"))?;
    let start = handle
        .range
        .start
        .checked_add(node.range.start)
        .ok_or_else(limit)?;
    let end = handle
        .range
        .start
        .checked_add(node.range.end)
        .ok_or_else(limit)?;
    let root = parsed.entered_body_root(handle)?;
    for syntax in root.descendants() {
        if ast::Expr::can_cast(syntax.kind())
            && u32::from(syntax.text_range().start()) == start
            && u32::from(syntax.text_range().end()) == end
        {
            return super::thunks::return_pc(index, handle, &syntax);
        }
    }
    Err(invalid(
        "generated break original target continuation syntax absent",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        expand::{
            declarative_test_fixture as fixture, identity::ExpansionKey, record::ExpansionRecord,
        },
        source::snapshot::{CfgSet, SourceBlob},
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    use lrr_expand_adapter::{
        declarative::{DeclarativeMacro, DeclarativeSite},
        token_wire::{self as adapter, ContextRecord, Delimiter, Edition, TokenTree, Transparency},
    };
    use ra_ap_syntax::ast::HasName;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    fn edition(value: u16) -> Edition {
        match value {
            2015 => Edition::E2015,
            2018 => Edition::E2018,
            2021 => Edition::E2021,
            2024 => Edition::E2024,
            _ => panic!("test edition"),
        }
    }
    fn input(argument: &str, edition: Edition) -> adapter::TokenForest {
        let source = format!("fixture!({argument});");
        let context = ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: adapter::Key([2; 32]),
            edition,
        };
        let forest = adapter::parse_source(
            &source,
            adapter::SourceIdentity {
                unit: context.defining_unit,
                content: adapter::Key(Digest::of_bytes(source.as_bytes()).0),
                path: "caller.rs".into(),
            },
            context,
            adapter::Key([3; 32]),
        )
        .unwrap();
        let [TokenTree::Group { children, .. }] = forest.roots() else {
            panic!("test root");
        };
        let group = children
            .iter()
            .find(|node| {
                matches!(
                    node,
                    TokenTree::Group {
                        delimiter: Delimiter::Parenthesis,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        adapter::TokenForest::new(
            vec![group],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap()
    }
    fn expanded(rules: &str, argument: &str, owning: u16) -> InputValue {
        let input = input(argument, edition(owning));
        let site = fixture::call_site(&input);
        let context = input
            .spans()
            .iter()
            .find(|span| span.key() == site)
            .unwrap()
            .context;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: Digest::of_bytes(rules.as_bytes()),
            },
            invocation: Digest([3; 32]),
            input_tokens: Digest::of_bytes(&input.encode().unwrap()),
            consumer_compile_env: Digest([4; 32]),
            parent_context: Digest(context.0),
            definition_edition: owning,
            call_edition: owning,
            grammar: GrammarSlot::Statements,
            engine_version: crate::expand::source_declarative::engine_version(),
            procedural_epoch: None,
        };
        let rules = DeclarativeMacro::new(fixture::definition(rules, edition(owning))).unwrap();
        let (result, _) = rules
            .expand_hygienic_with_limits(
                &input,
                site,
                DeclarativeSite {
                    expansion: adapter::Key(key.fingerprint().0),
                    defining_unit: adapter::Key([1; 32]),
                    definition_edition: edition(owning),
                },
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        InputValue::ExpansionRecord(
            ExpansionRecord::from_wire(key, result.unwrap().tokens.encode().unwrap()).unwrap(),
        )
    }
    fn caller(edition: u16) -> (BodySpan, PcKey) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(b"fn caller(){fixture!();}".to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([91; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let handle = outline.items[0].body.as_ref().unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
            .unwrap();
        (
            body.span(node.id).unwrap(),
            body.pc(node.id, Phase::Entry).unwrap(),
        )
    }
    #[test]
    fn unchanged_mac1_transcriber_and_arguments_decode_real_lifetime_expansion() {
        // Exact template and arguments come from unchanged original AST. The
        // independent caller above is an LRR decoder adapter, not original main.
        for owning in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition: owning,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let tree = parsed.parsed.tree();
            let definition = tree
                .syntax()
                .descendants()
                .filter_map(ast::MacroRules::cast)
                .find(|definition| definition.name().unwrap().text() == "mac1")
                .unwrap();
            let rules = definition.token_tree().unwrap().syntax().text().to_string();
            let rules = rules.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
            let calls: Vec<_> = tree
                .syntax()
                .descendants()
                .filter_map(ast::MacroCall::cast)
                .filter(|call| {
                    call.path()
                        .is_some_and(|path| path.syntax().text() == "mac1")
                })
                .collect();
            assert_eq!(calls.len(), 2);
            for call in calls {
                let argument = call.token_tree().unwrap().syntax().text().to_string();
                let argument = argument
                    .strip_prefix('(')
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                let value = expanded(rules, argument, owning);
                let (span, pc) = caller(owning);
                let decoded = decode_break(&value, span.clone(), &pc).unwrap();
                assert_eq!(decoded.caller(), &pc);
                assert_eq!(decoded.span(), &span);
                assert_eq!(decoded.call_edition(), owning);
                assert_eq!(decoded.definition_edition(), owning);
                assert_eq!(decoded.payload().unwrap().spelling, "1");
                assert_eq!(decoded.payload().unwrap().source, span);
                let lifetime = decoded.lifetime().unwrap();
                assert!(argument.starts_with(lifetime.spelling()));
                assert_eq!(lifetime.span().key(), lifetime.span_key());
                assert_eq!(lifetime.context(), lifetime.span().context);
                assert!(lifetime.generated_range().start < lifetime.generated_range().end);
                assert!(
                    decoded
                        .tokens()
                        .spans()
                        .iter()
                        .any(|span| span.key() == lifetime.span_key())
                );
                assert!(
                    decoded
                        .origins()
                        .iter()
                        .any(|origin| origin.span == lifetime.span_key())
                );
                assert!(decoded.allocated_bytes().unwrap() > std::mem::size_of::<GeneratedBreak>());
            }
        }
    }
    #[test]
    fn single_break_payload_lane_is_closed_and_callers_are_exact() {
        let (span, pc) = caller(2024);
        for rules in [
            "($t:lifetime)=>{break $t;}",
            "($t:lifetime)=>{break $t ((1));}",
            "($t:lifetime)=>{break $t true}",
            "($t:lifetime)=>{break $t 'x';}",
            "($t:lifetime)=>{break $t b'x';}",
            "($t:lifetime)=>{break $t 1.5;}",
        ] {
            decode_break(&expanded(rules, "'a", 2024), span.clone(), &pc).unwrap();
        }
        for rules in [
            "($t:lifetime)=>{break $t 1;break $t 2;}",
            "($t:lifetime)=>{fn cold(){} break $t 1;}",
            "($t:lifetime)=>{break $t missing();}",
            "($t:lifetime)=>{break $t -1;}",
            "($t:lifetime)=>{break $t \"text\";}",
            "($t:lifetime)=>{break $t ();}",
            "($t:lifetime)=>{return 1;}",
        ] {
            assert_eq!(
                decode_break(&expanded(rules, "'a", 2024), span.clone(), &pc)
                    .err()
                    .unwrap()
                    .code,
                "LRR-E-UNSUPPORTED",
                "{rules}"
            );
        }
        let value = expanded("($t:lifetime)=>{break $t 1;}", "'a", 2024);
        let mut other = pc.clone();
        other.phase = Phase::Commit.code().unwrap();
        assert_eq!(
            decode_break(&value, span.clone(), &other)
                .err()
                .unwrap()
                .code,
            "LRR-E-EXPANSION-INPUT"
        );
        other = pc.clone();
        other.body.definition = Digest([99; 32]);
        assert_eq!(
            decode_break(&value, span.clone(), &other)
                .err()
                .unwrap()
                .code,
            "LRR-E-EXPANSION-INPUT"
        );
        let InputValue::ExpansionRecord(record) = value else {
            panic!("test record")
        };
        for (grammar, identity) in [
            (GrammarSlot::Expression, record.key().macro_identity.clone()),
            (
                GrammarSlot::Statements,
                MacroIdentity::Builtin {
                    definition: Digest([1; 32]),
                },
            ),
        ] {
            let mut key = record.key().clone();
            key.grammar = grammar;
            key.macro_identity = identity;
            let value = InputValue::ExpansionRecord(
                ExpansionRecord::from_wire(key, record.wire().to_vec()).unwrap(),
            );
            assert_eq!(
                decode_break(&value, span.clone(), &pc).err().unwrap().code,
                "LRR-E-EXPANSION-INPUT"
            );
        }
    }
    #[test]
    fn non_break_routes_are_not_applicable_without_hiding_real_break_or_input_errors() {
        for owning in [2015, 2018, 2021, 2024] {
            let (span, pc) = caller(owning);
            for rules in [
                "($t:lifetime)=>{1;}",
                "($t:lifetime)=>{return 1;}",
                "($t:lifetime)=>{{break $t 1;}}",
                "($t:lifetime)=>{{fn cold(){} 1;}}",
                "($t:lifetime)=>{another!();}",
            ] {
                let value = expanded(rules, "'a", owning);
                assert!(
                    try_decode_break(&value, span.clone(), &pc)
                        .unwrap()
                        .is_none(),
                    "{rules}"
                );
                let mut foreign = pc.clone();
                foreign.phase = Phase::Commit.code().unwrap();
                assert_eq!(
                    try_decode_break(&value, span.clone(), &foreign)
                        .err()
                        .unwrap()
                        .code,
                    "LRR-E-EXPANSION-INPUT"
                );
            }
            for rules in [
                "($t:lifetime)=>{break $t missing();}",
                "($t:lifetime)=>{break $t another!();}",
                "($t:lifetime)=>{break $t 1;break $t 2;}",
                "($t:lifetime)=>{fn cold(){}break $t 1;}",
            ] {
                assert_eq!(
                    try_decode_break(&expanded(rules, "'a", owning), span.clone(), &pc)
                        .err()
                        .unwrap()
                        .code,
                    "LRR-E-UNSUPPORTED",
                    "{rules}"
                );
            }
            assert!(
                try_decode_break(
                    &expanded("($t:lifetime)=>{break $t 1;}", "'a", owning),
                    span,
                    &pc
                )
                .unwrap()
                .is_some()
            );
        }
    }
    #[test]
    fn target_continuation_reuses_original_block_commit_and_refuses_foreign_inputs() {
        const SOURCE: &str = "fn caller(){let x:u8='a:{'b:{fixture!();};0};}fn other(){}";
        for owning in [2015, 2018, 2021, 2024] {
            let blob = SourceBlob::new(SOURCE.as_bytes().to_vec()).unwrap();
            let parsed = ParsedFile::parse(
                &blob,
                ProfileIdentity {
                    edition: owning,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let outline = parsed
                .outline(Digest([91; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let handle = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "caller")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let other = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "other")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let index = parsed.index_body(handle, Default::default()).unwrap();
            assert_eq!(index.labels().len(), 2);
            for label in index.labels() {
                let range = target_label_range(&parsed, handle, &index, label.node).unwrap();
                assert_eq!(
                    &SOURCE[range.start as usize..range.end as usize],
                    label.spelling.as_ref().unwrap().as_str()
                );
                assert!(target_label_range(&parsed, other, &index, label.node).is_err());
                let expected = parsed
                    .initial_thunk(
                        handle,
                        &index,
                        &index.pc(label.node, Phase::Commit).unwrap(),
                    )
                    .unwrap();
                let crate::ir::recipe::RecipeExit::Next(expected) = &expected.recipe().exit else {
                    panic!("test block continuation");
                };
                assert_eq!(
                    &target_continuation(&parsed, handle, &index, label.node).unwrap(),
                    expected
                );
                assert_eq!(
                    target_continuation(&parsed, other, &index, label.node)
                        .unwrap_err()
                        .code,
                    "LRR-E-EXPANSION-INPUT"
                );
                let mut stale = handle.clone();
                stale.range.end -= 1;
                assert!(target_label_range(&parsed, &stale, &index, label.node).is_err());
                assert!(target_continuation(&parsed, &stale, &index, label.node).is_err());
                let foreign = ParsedFile::parse(
                    &blob,
                    ProfileIdentity {
                        edition: if owning == 2024 { 2021 } else { 2024 },
                        ..ProfileIdentity::current()
                    },
                )
                .unwrap();
                assert!(target_label_range(&foreign, handle, &index, label.node).is_err());
                assert_eq!(
                    target_continuation(&foreign, handle, &index, label.node)
                        .unwrap_err()
                        .code,
                    "LRR-E-EXPANSION-INPUT"
                );
            }
            assert_eq!(
                target_continuation(&parsed, handle, &index, NodeId(0))
                    .unwrap_err()
                    .code,
                "LRR-E-EXPANSION-INPUT"
            );
        }
    }
}
