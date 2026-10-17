//! Source relations from already resolved compiler comparisons. No expansion.
use super::*;
use crate::{
    core::digest::StableEncode,
    facade::macros::RuntimeMacroKind,
    syntax::{macro_calls::SourceMacroCall, runtime_macro_arguments::RuntimeMacroArguments},
};
use ra_ap_syntax::{Edition, ast::HasArgList};

#[derive(Debug)]
pub(crate) struct MacroNumericSite {
    source: super::super::parse::ParseKey,
    key: PcKey,
    index: crate::core::digest::Digest,
    binding: NodeId,
    floating: bool,
    initializer: bool,
}
impl MacroNumericSite {
    pub(crate) fn binding(&self) -> NodeId {
        self.binding
    }
    pub(crate) fn floating(&self) -> bool {
        self.floating
    }
    pub(crate) fn is_initializer(&self) -> bool {
        self.initializer
    }
    pub(crate) fn require(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<(), Diagnostic> {
        if parsed.key() != self.source
            || body.key() != key.body
            || self.key != *key
            || index.body() != &key.body
            || index.fingerprint() != self.index
        {
            return Err(error(
                "numeric macro relation belongs to another source/body/site",
            ));
        }
        Ok(())
    }
    pub(crate) fn comparison_parameter(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        call: &SourceMacroCall,
        args: &RuntimeMacroArguments,
    ) -> Result<Option<u32>, Diagnostic> {
        self.require(parsed, body, index, &self.key)?;
        args.require_source(&parsed.key())?;
        if !matches!(
            args.kind(),
            RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe
        ) || args.invocation() != call.range()
            || args.operands().len() != 2
            || !body.range.contains(call.range())
        {
            return Ok(None);
        }
        let root = parsed.entered_body_root(body)?;
        let syntax = root
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .find(|node| {
                super::super::ast_adapter::range(node.syntax().text_range()) == call.range()
            })
            .ok_or_else(|| error("certified numeric macro invocation source absent"))?;
        let mut names = Vec::new();
        for range in args.operands() {
            let Some(name) = operand_name(parsed, *range)? else {
                return Ok(None);
            };
            names.push(name);
        }
        let mut selected = None;
        for (local, parameter) in [(0, 1), (1, 0)] {
            if local_binding_at(parsed, index, body, syntax.syntax(), &names[local])?
                != Some(self.binding)
                || local_binding_at(parsed, index, body, syntax.syntax(), &names[parameter])?
                    .is_some()
            {
                continue;
            }
            if let Some(ordinal) = parameter_at(parsed, syntax.syntax(), &names[parameter])? {
                if selected.is_some_and(|previous| previous != ordinal) {
                    return Ok(None);
                }
                selected = Some(ordinal);
            }
        }
        Ok(selected)
    }
}

fn operand_name(
    parsed: &ParsedFile,
    range: super::super::spans::SourceRange,
) -> Result<Option<MacroName>, Diagnostic> {
    let bytes = range
        .end
        .checked_sub(range.start)
        .ok_or_else(|| error("numeric comparison operand source budget"))?;
    let root = parsed.parsed.tree();
    let text_range = ra_ap_syntax::TextRange::new(range.start.into(), range.end.into());
    if !root.syntax().text_range().contains_range(text_range) {
        return Err(error("numeric comparison operand escapes source"));
    }
    let text = root.syntax().text().slice(text_range).to_string();
    if text.len() != bytes as usize {
        return Err(error("numeric comparison operand byte range differs"));
    }
    let wrapped = format!("fn __lrr_numeric(){{__lrr_operand({text});}}");
    let edition = match parsed.edition {
        2015 => Edition::Edition2015,
        2018 => Edition::Edition2018,
        2021 => Edition::Edition2021,
        2024 => Edition::Edition2024,
        _ => return Err(error("numeric comparison operand edition differs")),
    };
    let syntax = ast::SourceFile::parse(&wrapped, edition);
    if !syntax.errors().is_empty() {
        return Ok(None);
    }
    let tree = syntax.tree();
    let Some(call) = tree.syntax().descendants().find_map(ast::CallExpr::cast) else {
        return Ok(None);
    };
    let Some(list) = call.arg_list() else {
        return Ok(None);
    };
    let mut args = list.args();
    let Some(mut expression) = args.next() else {
        return Ok(None);
    };
    if args.next().is_some() {
        return Ok(None);
    }
    for _ in 0usize.. {
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PathExpr(path) => {
                let Some(segment) = path
                    .path()
                    .filter(|path| path.qualifier().is_none())
                    .and_then(|path| path.segment())
                else {
                    return Ok(None);
                };
                if segment.coloncolon_token().is_some()
                    || segment.generic_arg_list().is_some()
                    || segment.parenthesized_arg_list().is_some()
                    || segment.type_anchor().is_some()
                {
                    return Ok(None);
                }
                return segment
                    .name_ref()
                    .map(|name| MacroName::parse(name.text(), parsed.edition))
                    .transpose();
            }
            _ => return Ok(None),
        }
    }
    Err(error("numeric comparison operand nesting limit"))
}

fn source_node(parsed: &ParsedFile, start: u32, end: u32) -> Result<SyntaxNode, Diagnostic> {
    let root = parsed.parsed.tree();
    if start > end {
        return Err(error("numeric request reversed source range"));
    }
    let range = ra_ap_syntax::TextRange::new(start.into(), end.into());
    if !root.syntax().text_range().contains_range(range) {
        return Err(error("numeric request source range escapes parser"));
    }
    match root.syntax().covering_element(range) {
        ra_ap_syntax::NodeOrToken::Node(node) => Ok(node),
        ra_ap_syntax::NodeOrToken::Token(token) => token
            .parent()
            .ok_or_else(|| error("numeric source token parent absent")),
    }
}

pub(super) fn request(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
) -> Result<Option<MacroNumericSite>, Diagnostic> {
    if parsed.generated_origin.is_some()
        || body.key() != key.body
        || index.body() != &key.body
        || index.profile() != &parsed.profile
    {
        return Ok(None);
    }
    let Some(node) = index.nodes().get(key.node as usize) else {
        return Err(error("numeric comparison request node absent"));
    };
    let phase = crate::ir::pc::Phase::from_code(key.phase);
    let initializer_site = node.role == NodeRole::Statement
        && matches!(
            phase,
            crate::ir::pc::Phase::Entry | crate::ir::pc::Phase::Commit
        );
    if !initializer_site
        && !(node.role == NodeRole::Expression(ExpressionKind::Other)
            && matches!(
                phase,
                crate::ir::pc::Phase::Entry
                    | crate::ir::pc::Phase::Commit
                    | crate::ir::pc::Phase::Operand(0 | 1)
            ))
    {
        return Ok(None);
    }
    index.pc(NodeId(key.node), crate::ir::pc::Phase::from_code(key.phase))?;
    let root = parsed.entered_body_root(body)?;
    let start = body
        .range
        .start
        .checked_add(node.range.start)
        .ok_or_else(|| error("numeric request source range overflow"))?;
    let end = body
        .range
        .start
        .checked_add(node.range.end)
        .filter(|end| *end <= body.range.end)
        .ok_or_else(|| error("numeric request source range overflow"))?;
    let source = source_node(parsed, start, end)?;
    if initializer_site {
        let statement = source
            .ancestors()
            .filter_map(ast::LetStmt::cast)
            .find(|statement| {
                let range = statement.syntax().text_range();
                u32::from(range.start()) == start && u32::from(range.end()) == end
            });
        let Some(statement) = statement else {
            return Ok(None);
        };
        let Some(ast::Pat::IdentPat(pattern)) = statement.pat() else {
            return Ok(None);
        };
        let (enabled, attributed) = statement_cfg(statement.syntax(), index.cfg())?;
        if !enabled
            || attributed
            || statement.ty().is_some()
            || statement.let_else().is_some()
            || pattern.ref_token().is_some()
            || pattern.at_token().is_some()
        {
            return Ok(None);
        }
        let Some(initializer) = statement.initializer() else {
            return Ok(None);
        };
        let Some(floating) = runtime_local_numeric_context::unsuffixed_family(initializer)? else {
            return Ok(None);
        };
        if runtime_local_numeric_context::inferred_context(
            parsed, index, body, &statement, &pattern,
        )?
        .is_some()
        {
            return Ok(None);
        }
        return Ok(Some(MacroNumericSite {
            source: parsed.key(),
            key: key.clone(),
            index: index.fingerprint(),
            binding: id_for(index, body, pattern.syntax())?,
            floating,
            initializer: true,
        }));
    }
    let selected = source
        .ancestors()
        .filter_map(ast::BinExpr::cast)
        .find(|binary| {
            let range = binary.syntax().text_range();
            u32::from(range.start()) == start && u32::from(range.end()) == end
        });
    let Some(binary) = selected else {
        return Ok(None);
    };
    if !binary.op_token().is_some_and(|token| {
        matches!(
            token.text(),
            "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
        )
    }) {
        return Ok(None);
    }
    let Some(ast::Expr::PathExpr(left)) = binary.lhs() else {
        return Ok(None);
    };
    if compound_local_primitive(parsed, index, body, &left, None)?.is_some()
        || compound_local_definition_context(parsed, index, body, &left)?.is_some()
    {
        return Ok(None);
    }
    let Some(binding) = local_binding(parsed, index, body, &left)? else {
        return Ok(None);
    };
    let wanted = index
        .nodes()
        .get(binding.0 as usize)
        .ok_or_else(|| error("numeric binding source absent"))?
        .range;
    for syntax in root.descendants() {
        let Some(statement) = ast::LetStmt::cast(syntax) else {
            continue;
        };
        let Some(ast::Pat::IdentPat(pattern)) = statement.pat() else {
            continue;
        };
        let range = pattern.syntax().text_range();
        if u32::from(range.start()).checked_sub(body.range.start) != Some(wanted.start)
            || u32::from(range.end()).checked_sub(body.range.start) != Some(wanted.end)
        {
            continue;
        }
        let (enabled, attributed) = statement_cfg(statement.syntax(), index.cfg())?;
        if !enabled
            || attributed
            || statement.ty().is_some()
            || pattern.ref_token().is_some()
            || pattern.at_token().is_some()
        {
            return Ok(None);
        }
        let Some(initializer) = statement.initializer() else {
            return Ok(None);
        };
        let Some(floating) = runtime_local_numeric_context::unsuffixed_family(initializer)? else {
            return Ok(None);
        };
        return Ok(Some(MacroNumericSite {
            source: parsed.key(),
            key: key.clone(),
            index: index.fingerprint(),
            binding,
            floating,
            initializer: false,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    fn source(edition: u16) -> (ParsedFile, BodyHandle, BodyIndex) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([239; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "label_break_match")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let index = parsed.index_body(&body, Default::default()).unwrap();
        (parsed, body, index)
    }
    #[test]
    fn unchanged_compound_sites_retain_exact_comparison_parameter_relation_all_editions() {
        // Pure source-fact component only. Selecting a compiler identity and its
        // current Type namespace remains the frontend producer's responsibility.
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = source(edition);
            let root = parsed.entered_body_root(&body).unwrap();
            let macros = root
                .descendants()
                .filter_map(ast::MacroCall::cast)
                .map(|node| {
                    parsed
                        .macro_call_at(super::super::super::ast_adapter::range(
                            node.syntax().text_range(),
                        ))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(macros.len(), 2);
            let compared_x = parsed
                .runtime_macro_arguments(&macros[0], RuntimeMacroKind::AssertEq)
                .unwrap();
            let compared_y = parsed
                .runtime_macro_arguments(&macros[1], RuntimeMacroKind::AssertEq)
                .unwrap();
            let mut count = 0;
            for binary in root
                .descendants()
                .filter_map(ast::BinExpr::cast)
                .filter(|binary| binary.op_token().is_some_and(|token| token.text() == "+="))
            {
                let id = id_for(&index, &body, binary.syntax()).unwrap();
                let key = index.pc(id, crate::ir::pc::Phase::Entry).unwrap();
                let site = request(&parsed, &body, &index, &key).unwrap().unwrap();
                assert!(!site.floating());
                assert_eq!(
                    site.comparison_parameter(&parsed, &body, &index, &macros[0], &compared_x)
                        .unwrap(),
                    Some(1)
                );
                assert_eq!(
                    site.comparison_parameter(&parsed, &body, &index, &macros[1], &compared_y)
                        .unwrap(),
                    None
                );
                count += 1;
            }
            assert_eq!(count, 2);
        }
    }
    #[test]
    fn unchanged_comparison_request_has_no_literal_site_or_foreign_phase_authority() {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = source(edition);
            let root = parsed.entered_body_root(&body).unwrap();
            let binary = root
                .descendants()
                .filter_map(ast::BinExpr::cast)
                .find(|binary| binary.op_token().is_some_and(|token| token.text() == "+="))
                .unwrap();
            let key = index
                .pc(
                    id_for(&index, &body, binary.syntax()).unwrap(),
                    crate::ir::pc::Phase::Entry,
                )
                .unwrap();
            let site = request(&parsed, &body, &index, &key).unwrap().unwrap();
            let foreign = index
                .pc(NodeId(key.node), crate::ir::pc::Phase::Commit)
                .unwrap();
            assert!(site.require(&parsed, &body, &index, &foreign).is_err());
            let literal = root.descendants().find_map(ast::Literal::cast).unwrap();
            let literal = index
                .pc(
                    id_for(&index, &body, literal.syntax()).unwrap(),
                    crate::ir::pc::Phase::Entry,
                )
                .unwrap();
            assert!(request(&parsed, &body, &index, &literal).unwrap().is_none());
        }
    }
}
