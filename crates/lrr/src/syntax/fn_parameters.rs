//! Pattern parameters of `fn` items (E-R5-010).
//!
//! `fn f(PAT: T) { body }` is rustc's `fn f(arg0: T) { let PAT = arg0; body }`.
//! A function with such a parameter (anything but `x`, `mut x`, `_`) is a
//! *pattern function*: its body handle spans its parameter list and block, so
//! the one `BodyIndex` also indexes the parameter patterns and the shared
//! pattern transaction binds them at entry, exactly as a closure's parameters.
//! Functions without a pattern parameter keep the block-only handle.
use super::{ast_adapter::range, parse::ParsedFile, spans::SourceRange};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncoder},
};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};

/// A written parameter that runs the pattern transaction: anything but `x`,
/// `mut x` and `_` (`ref x` and `x @ ..` do). A plain or `_` parameter stays
/// a frame parameter read at each use and dropped with the frame. The
/// adapter's `entered_body` applies this same rule to locate a generated
/// function's handle range.
pub(crate) fn pattern_parameter(parameter: &ast::Param) -> bool {
    match parameter.pat() {
        None | Some(ast::Pat::WildcardPat(_)) => false,
        Some(ast::Pat::IdentPat(pattern)) => {
            pattern.ref_token().is_some() || pattern.at_token().is_some()
        }
        Some(_) => true,
    }
}

pub(crate) fn runs_patterns(list: &ast::ParamList) -> bool {
    list.params().any(|parameter| pattern_parameter(&parameter))
}

/// The parameter list of the pattern function whose body block is `block`.
pub(crate) fn pattern_list(block: &SyntaxNode) -> Option<ast::ParamList> {
    let function = ast::Fn::cast(block.parent()?)?;
    if function.body()?.syntax() != block {
        return None;
    }
    function.param_list().filter(runs_patterns)
}

/// The handle range of the body whose block is `block`: the block, or for a
/// pattern function its parameter list through the block.
pub(crate) fn body_range(block: &SyntaxNode) -> SourceRange {
    let end = range(block.text_range()).end;
    match pattern_list(block) {
        Some(list) => SourceRange {
            start: range(list.syntax().text_range()).start,
            end,
        },
        None => range(block.text_range()),
    }
}

/// Bytes of the handle range that precede the block (0 unless a pattern
/// function).
pub(crate) fn block_offset(block: &SyntaxNode) -> u32 {
    range(block.text_range())
        .start
        .saturating_sub(body_range(block).start)
}

/// The body block of the pattern function whose handle range is `body`,
/// found from the element covering that range.
pub(crate) fn pattern_function_block(
    covering: &ra_ap_syntax::SyntaxElement,
    body: SourceRange,
) -> Option<SyntaxNode> {
    covering.ancestors().find_map(|node| {
        let block = ast::Fn::cast(node)?.body()?.syntax().clone();
        (pattern_list(&block).is_some() && body_range(&block) == body).then_some(block)
    })
}

impl ParsedFile {
    /// Content identity of the body whose block is `block`: the block alone,
    /// or for a pattern function its parameter list and block.
    pub(crate) fn fn_body_identity(&self, block: &SyntaxNode) -> Result<Digest, Diagnostic> {
        let Some(list) = pattern_list(block) else {
            return self.body_content_identity(block);
        };
        if self.retained_syntax_identity().is_some() {
            let mut identity = StableEncoder::new("lrr/v2/retained-fn-parameters-body");
            identity.digest(self.body_content_identity(list.syntax())?);
            identity.digest(self.body_content_identity(block)?);
            return Ok(identity.finish());
        }
        // Text sources hash the exact bytes of the handle range.
        let function = block.parent().ok_or_else(|| {
            Diagnostic::new("LRR-E-BODY-INDEX", "function body parent absent".into())
        })?;
        let extent = body_range(block);
        let base = u32::from(function.text_range().start());
        let text = function.text().slice(ra_ap_syntax::TextRange::new(
            (extent.start - base).into(),
            (extent.end - base).into(),
        ));
        let mut content = String::new();
        content
            .try_reserve_exact(usize::from(text.len()))
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-BODY-INDEX",
                    "body content verification allocation".into(),
                )
            })?;
        text.for_each_chunk(|chunk| content.push_str(chunk));
        Ok(Digest::of_bytes(content.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        ir::pc::{NodeId, Phase},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::body_index::{BodyIndex, NodeRole, ScopeKind},
        syntax::outline::BodyHandle,
    };

    /// The parsed file, its body handles by function name and their indexes.
    fn indexed(source: &str, name: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([77; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .and_then(|item| item.body.clone())
            .unwrap();
        let index = parsed.index_body(&handle, Default::default()).unwrap();
        (parsed, handle, index)
    }

    #[test]
    fn a_pattern_function_spans_its_parameter_list_and_owns_one_parameter_scope() {
        let source = "fn f(x: i32, (a, b): (i32, i32), _: u8) -> i32 { a + b + x }";
        let (parsed, handle, index) = indexed(source, "f");
        let block = source.find('{').unwrap() as u32;
        let list = source.find('(').unwrap() as u32;
        assert_eq!(handle.range.start, list);
        assert_eq!(handle.range.end, source.len() as u32);
        assert_eq!(handle.block_range().start, block);
        assert_eq!(handle.block_offset, block - list);
        // The root is still the block; the parameter list follows it.
        assert_eq!(
            parsed.entered_body_root(&handle).unwrap().kind(),
            ra_ap_syntax::SyntaxKind::BLOCK_EXPR
        );
        assert_eq!(index.scopes()[0].kind, ScopeKind::Parameters);
        assert_eq!(index.scopes()[1].kind, ScopeKind::Block);
        assert_eq!(index.scopes()[1].parent, Some(index.scopes()[0].id));
        let owner = index.parameter_owner().unwrap();
        let owner = &index.nodes()[owner.0 as usize];
        assert_eq!(owner.role, NodeRole::Syntax);
        assert!(owner.pattern_statement);
        assert!(owner.runtime_temporaries[0].is_some());
        // One staging slot per written parameter (receiver excluded).
        assert_eq!(owner.runtime_scalar_slots.len(), 3);
        assert_eq!(index.nodes()[0].range.start, block - list);
        assert_eq!(index.scopes()[0].node, owner.id);
        // Every parameter pattern node lies in the parameter scope.
        for node in index.nodes().iter().filter(|n| n.role == NodeRole::Pattern) {
            if node.range.start < block - list {
                assert_eq!(node.scope, index.scopes()[0].id);
            }
        }
        // The function entry stays the root block's `Entry` phase.
        assert!(index.pc(NodeId(0), Phase::Entry).is_ok());
    }

    #[test]
    fn plain_underscore_mut_and_receiver_parameters_keep_the_block_only_body() {
        let source = "struct S; impl S { fn m(&self, mut a: i32, _: u8, c: i32) -> i32 { a + c } }";
        let (parsed, handle, index) = indexed(source, "m");
        let block = parsed.entered_body_root(&handle).unwrap();
        assert_eq!(
            handle.range,
            crate::syntax::ast_adapter::range(block.text_range())
        );
        assert_eq!(handle.block_offset, 0);
        assert_eq!(index.parameter_owner(), None);
        assert_ne!(index.scopes()[0].kind, ScopeKind::Parameters);
        assert_eq!(index.nodes()[0].range.start, 0);
    }

    #[test]
    fn editing_a_parameter_pattern_changes_the_body_identity_and_ref_or_at_make_a_pattern_function()
    {
        let (_, a, _) = indexed("fn f((a, b): (i32, i32)) -> i32 { a + b }", "f");
        let (_, b, _) = indexed("fn f((a, c): (i32, i32)) -> i32 { a + c }", "f");
        let (_, same, _) = indexed("fn f((a, b): (i32, i32)) -> i32 { a + b }", "f");
        assert_ne!(a.body_hash, b.body_hash);
        assert_eq!(a.body_hash, same.body_hash);
        for source in [
            "fn f(ref x: i32) -> i32 { *x }",
            "fn f(x @ (_, _): (i32, i32)) -> i32 { x.0 }",
            "fn f(&x: &i32) -> i32 { x }",
        ] {
            let (_, handle, index) = indexed(source, "f");
            assert!(index.parameter_owner().is_some(), "{source}");
            assert!(handle.block_offset > 0, "{source}");
        }
    }
}
