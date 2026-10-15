//! Original match IdentPat and prelude-header source selection; no name authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    ir::pc::{NodeId, PcKey, Phase},
    source::snapshot::CfgSet,
    syntax::{
        body_index::{BodyIndex, NodeRole},
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
    range: SourceRange,
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
            .scalar_match_binding_site(handle, index, pc)?
            .as_ref()
            != Some(self)
        {
            return Err(fault("plain pattern structural source certificate differs"));
        }
        Ok(())
    }
}
impl ParsedFile {
    pub(crate) fn scalar_match_binding_site(
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
        if node.role != NodeRole::Pattern
            || !matches!(Phase::from_code(pc.phase), Phase::Entry | Phase::Commit)
        {
            return Ok(None);
        }
        let start = handle
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| fault("plain pattern start overflow"))?;
        let end = handle
            .range
            .start
            .checked_add(node.range.end)
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
        if pat.mut_token().is_some()
            || pat.ref_token().is_some()
            || pat.at_token().is_some()
            || pat
                .syntax()
                .children()
                .any(|child| ast::Attr::can_cast(child.kind()) || ast::Pat::can_cast(child.kind()))
        {
            return Ok(None);
        }
        let Some(arm) = pat.syntax().parent().and_then(ast::MatchArm::cast) else {
            return Ok(None);
        };
        if arm.pat().is_none_or(|p| p.syntax().text_range() != range) {
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
        let name = super::ast_adapter::semantic_name(&name.syntax().text().to_string())
            .ok_or_else(|| fault("plain pattern name invalid"))?;
        if name.len() > 256 {
            return Err(fault("plain pattern name budget"));
        }
        Ok(Some(ScalarPatternSite {
            parse: self.key(),
            body: pc.body.clone(),
            pattern: NodeId(pc.node),
            range: SourceRange { start, end },
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
