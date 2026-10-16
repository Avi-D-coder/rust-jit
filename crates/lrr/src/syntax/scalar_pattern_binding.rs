//! Reached direct pattern name and prelude-header source selection; no name authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    ir::pc::{NodeId, PcKey, Phase},
    source::snapshot::CfgSet,
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::{BodyHandle, ScopeKey},
        parse::{ParseKey, ParsedFile},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode, TextRange, TextSize,
    ast::{self, HasAttrs, HasName},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ScalarPatternSite {
    parse: ParseKey,
    body: crate::source::manifest::BodyKey,
    pattern: NodeId,
    canonical: NodeId,
    range: SourceRange,
    name_range: SourceRange,
    name: String,
    index: Digest,
}
impl ScalarPatternSite {
    pub(crate) fn pattern(&self) -> NodeId {
        self.pattern
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn name_range(&self) -> SourceRange {
        self.name_range
    }
    pub(crate) fn canonical(&self) -> NodeId {
        self.canonical
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn require(
        &self,
        parsed: &ParsedFile,
        handle: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<(), Diagnostic> {
        if parsed
            .scalar_pattern_binding_site(handle, index, pc)?
            .as_ref()
            != Some(self)
        {
            return Err(fault("plain pattern structural source certificate differs"));
        }
        Ok(())
    }
}
impl ParsedFile {
    pub(crate) fn scalar_pattern_binding_site(
        &self,
        handle: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<Option<ScalarPatternSite>, Diagnostic> {
        handle.require_entry()?;
        if self.key().profile != *index.profile()
            || handle.key() != pc.body
            || index.body() != &pc.body
        {
            return Err(fault("plain pattern original source/body differs"));
        }
        let Some(node) = index.nodes().get(pc.node as usize) else {
            return Err(fault("plain pattern node absent"));
        };
        if !matches!(Phase::from_code(pc.phase), Phase::Entry | Phase::Commit) {
            return Ok(None);
        }
        let condition =
            node.role == NodeRole::Expression(ExpressionKind::Other) && node.clause_scope.is_some();
        let pattern_node = if condition {
            let Some(expression) = self
                .entered_node_syntax(handle, index, node.id)?
                .and_then(ast::LetExpr::cast)
            else {
                return Err(fault("condition binding owner syntax absent"));
            };
            let Some(pattern) = expression.pat() else {
                return Err(fault("condition binding pattern absent"));
            };
            let range = pattern.syntax().text_range();
            let relative = SourceRange {
                start: u32::from(range.start())
                    .checked_sub(handle.range.start)
                    .ok_or_else(|| fault("condition binding precedes body"))?,
                end: u32::from(range.end())
                    .checked_sub(handle.range.start)
                    .ok_or_else(|| fault("condition binding precedes body"))?,
            };
            index
                .nodes_at_range(relative)
                .find(|candidate| candidate.role == NodeRole::Pattern)
                .ok_or_else(|| fault("condition binding pattern identity absent"))?
        } else if node.role == NodeRole::Pattern {
            node
        } else {
            return Ok(None);
        };
        let start = handle
            .range
            .start
            .checked_add(pattern_node.range.start)
            .ok_or_else(|| fault("plain pattern start overflow"))?;
        let end = handle
            .range
            .start
            .checked_add(pattern_node.range.end)
            .ok_or_else(|| fault("plain pattern end overflow"))?;
        if end > handle.range.end || start > end {
            return Err(fault("plain pattern range escapes body"));
        }
        let range = TextRange::new(TextSize::from(start), TextSize::from(end));
        let selected = self.parsed.tree().syntax().covering_element(range);
        let selected = match selected {
            ra_ap_syntax::NodeOrToken::Node(node) => node,
            ra_ap_syntax::NodeOrToken::Token(token) => token
                .parent()
                .ok_or_else(|| fault("plain pattern token parent absent"))?,
        };
        let Some(pat) = selected
            .ancestors()
            .take(256)
            .filter_map(ast::IdentPat::cast)
            .find(|pat| pat.syntax().text_range() == range)
        else {
            return Ok(None);
        };
        if pat
            .syntax()
            .children()
            .any(|child| ast::Attr::can_cast(child.kind()))
        {
            return Ok(None);
        }
        let mut root = ast::Pat::IdentPat(pat.clone());
        let mut parent = pat.syntax().parent();
        let mut owner = false;
        // Cross source pattern structure only, never a statement/body boundary.
        for _ in 0..256 {
            let Some(current) = parent else { break };
            if let Some(pattern) = ast::Pat::cast(current.clone()) {
                root = pattern;
                parent = current.parent();
            } else if let Some(arm) = ast::MatchArm::cast(current.clone()) {
                owner = arm.pat().is_some_and(|p| p.syntax() == root.syntax());
                break;
            } else if let Some(expression) = ast::LetExpr::cast(current.clone()) {
                owner = expression
                    .pat()
                    .is_some_and(|p| p.syntax() == root.syntax());
                break;
            } else if matches!(
                current.kind(),
                ra_ap_syntax::SyntaxKind::RECORD_PAT_FIELD
                    | ra_ap_syntax::SyntaxKind::RECORD_PAT_FIELD_LIST
            ) {
                parent = current.parent();
            } else {
                break;
            }
        }
        if !owner {
            return Ok(None);
        }
        if self.generated_origin.is_some() {
            return Err(fault(
                "plain generated match binding requires actual origin/hygiene admission",
            ));
        }
        let name = pat
            .name()
            .ok_or_else(|| fault("plain pattern name absent"))?;
        let name_range = super::ast_adapter::range(name.syntax().text_range());
        let name = super::ast_adapter::semantic_name(&name.syntax().text().to_string())
            .ok_or_else(|| fault("plain pattern name invalid"))?;
        if name.len() > 256 {
            return Err(fault("plain pattern name budget"));
        }
        let group = index
            .pattern_bindings()
            .group_for(pattern_node.id)
            .ok_or_else(|| fault("pattern occurrence lacks retained binding candidates"))?;
        let canonical = group
            .canonical_candidate()
            .ok_or_else(|| fault("pattern occurrence has unresolved binding group facts"))?;
        if group.name() != name
            || group
                .occurrence(pattern_node.id)
                .is_none_or(|occurrence| occurrence.name_range != name_range)
        {
            return Err(fault(
                "pattern occurrence differs from its retained candidate group",
            ));
        }
        Ok(Some(ScalarPatternSite {
            parse: self.key(),
            body: pc.body.clone(),
            pattern: pattern_node.id,
            canonical,
            range: SourceRange { start, end },
            name_range,
            name,
            index: index.fingerprint(),
        }))
    }
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct BindingPreludePolicy {
    standard: bool,
    disabled: bool,
}
impl BindingPreludePolicy {
    pub(crate) fn standard(self) -> bool {
        self.standard
    }
    pub(crate) fn disabled(self) -> bool {
        self.disabled
    }
}
impl ParsedFile {
    pub(crate) fn binding_prelude_policy(
        &self,
        cfg: &CfgSet,
        scope: &ScopeKey,
    ) -> Result<Option<BindingPreludePolicy>, Diagnostic> {
        if self.generated_origin.is_some() {
            return Err(fault(
                "generated Value prelude policy requires actual owning-crate origin",
            ));
        }
        let policy = self.macro_prelude_policy(cfg)?;
        let mut stack = vec![self.parsed.tree().syntax().clone()];
        let mut work = 0usize;
        while let Some(node) = stack.pop() {
            work = work
                .checked_add(1)
                .filter(|n| *n <= 100_000)
                .ok_or_else(|| fault("Value prelude header work limit"))?;
            if let Some(module) = ast::Module::cast(node.clone()) {
                let mut attrs: Vec<_> = module.attrs().collect();
                if let Some(list) = module.item_list() {
                    attrs.extend(list.attrs());
                }
                let (enabled, _) = super::attributes::effective_metas(
                    attrs.into_iter().filter_map(|a| a.meta()).collect(),
                    cfg,
                )?;
                if !enabled {
                    continue;
                }
            }
            if let Some(import) = ast::Use::cast(node.clone()) {
                let (enabled, metas) = super::attributes::effective_metas(
                    import.attrs().filter_map(|a| a.meta()).collect(),
                    cfg,
                )?;
                if !enabled {
                    continue;
                }
                if metas
                    .iter()
                    .any(|m| m.simple_name().is_some_and(|n| n == "prelude_import"))
                {
                    return Ok(None);
                }
            }
            if ast::Fn::can_cast(node.kind())
                || ast::Const::can_cast(node.kind())
                || ast::Static::can_cast(node.kind())
                || ast::TokenTree::can_cast(node.kind())
            {
                continue;
            }
            for child in node.children() {
                if stack.len() >= 100_000 {
                    return Err(fault("Value prelude header frontier limit"));
                }
                stack
                    .try_reserve(1)
                    .map_err(|_| fault("Value prelude header allocation"))?;
                stack.push(child);
            }
        }
        Ok(Some(BindingPreludePolicy {
            standard: policy.standard,
            disabled: policy
                .disabled
                .iter()
                .any(|prefix| scope.module_path.starts_with(prefix)),
        }))
    }
}
