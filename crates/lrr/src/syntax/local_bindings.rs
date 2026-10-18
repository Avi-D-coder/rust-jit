//! Owned syntactic binding candidates; constant-pattern resolution is a later demand.
use super::{ast_adapter::range, outline::BodyHandle, spans::SourceRange};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncoder},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingForm {
    Parameter,
    Let,
    For,
    Match,
    Receiver,
    Condition,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PatternNameKind {
    ExplicitBinding,
    BindingOrConstant,
}
#[derive(Debug, Clone)]
pub struct LocalBindingRecipe {
    pub identity: Digest,
    pub scope: u32,
    pub name: String,
    pub raw_name: String,
    pub name_range: SourceRange,
    pub pattern_range: SourceRange,
    pub visible_from: u64,
    pub form: BindingForm,
    pub classification: PatternNameKind,
}
#[derive(Default)]
pub(crate) struct BindingBudget {
    bytes: usize,
    visited: usize,
    ordinals: BTreeMap<(u32, u32, String), u32>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-LIMIT", message.into())
}
impl BindingBudget {
    fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| error("local binding metadata limit exceeded"))?;
        Ok(())
    }
    pub fn receiver(
        &mut self,
        receiver: &ast::SelfParam,
        owner: &BodyHandle,
        result: &mut Vec<LocalBindingRecipe>,
    ) -> Result<(), Diagnostic> {
        let name = receiver
            .name()
            .ok_or_else(|| error("receiver name missing"))?;
        if name.syntax().text() != "self" {
            return Err(error("receiver name is not self"));
        }
        self.charge(
            std::mem::size_of::<LocalBindingRecipe>()
                + std::mem::size_of::<(u32, u32, String)>()
                + 16,
        )?;
        let ordinal = self.ordinals.entry((0, 5, "self".into())).or_default();
        let mut e = StableEncoder::new("lrr/v2/original-local-binding");
        e.digest(owner.def_key);
        e.u32(0);
        e.u32(5);
        e.string("self");
        e.u32(*ordinal);
        *ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| error("receiver ordinal overflow"))?;
        result
            .try_reserve(1)
            .map_err(|_| error("receiver record allocation failed"))?;
        result.push(LocalBindingRecipe {
            identity: e.finish(),
            scope: 0,
            name: "self".into(),
            raw_name: "self".into(),
            name_range: range(name.syntax().text_range()),
            pattern_range: range(receiver.syntax().text_range()),
            visible_from: u64::from(owner.block_range().start),
            form: BindingForm::Receiver,
            classification: PatternNameKind::ExplicitBinding,
        });
        Ok(())
    }
    pub fn collect(
        &mut self,
        pattern: &ast::Pat,
        owner: &BodyHandle,
        scope: u32,
        visible_from: u64,
        form: BindingForm,
        result: &mut Vec<LocalBindingRecipe>,
    ) -> Result<(), Diagnostic> {
        let tag = match form {
            BindingForm::Parameter => 1,
            BindingForm::Let => 2,
            BindingForm::For => 3,
            BindingForm::Match => 4,
            BindingForm::Receiver => 5,
            BindingForm::Condition => 6,
        };
        let mut stack = vec![pattern.syntax().clone()];
        while let Some(node) = stack.pop() {
            self.visited = self
                .visited
                .checked_add(1)
                .ok_or_else(|| error("binding pattern syntax limit exceeded"))?;
            if ast::Expr::can_cast(node.kind())
                || ast::Item::can_cast(node.kind())
                || ast::TokenTree::can_cast(node.kind())
            {
                continue;
            }
            let mut children = Vec::new();
            for child in node.children() {
                children
                    .try_reserve(1)
                    .map_err(|_| error("binding pattern allocation failed"))?;
                children.push(child);
            }
            stack
                .len()
                .checked_add(children.len())
                .ok_or_else(|| error("binding pattern frontier limit exceeded"))?;
            stack
                .try_reserve(children.len())
                .map_err(|_| error("binding pattern stack allocation failed"))?;
            stack.extend(children.into_iter().rev());
            let Some(pat) = ast::IdentPat::cast(node) else {
                continue;
            };
            let name = pat
                .name()
                .ok_or_else(|| error("binding pattern name missing"))?;
            let len = usize::from(name.syntax().text_range().len());
            self.charge(
                len.checked_mul(2)
                    .and_then(|n| {
                        n.checked_add(
                            std::mem::size_of::<LocalBindingRecipe>()
                                + std::mem::size_of::<(u32, u32, String)>(),
                        )
                    })
                    .ok_or_else(|| error("binding name size overflow"))?,
            )?;
            let raw_name = name.syntax().text().to_string();
            let canonical = super::ast_adapter::semantic_name(&raw_name)
                .ok_or_else(|| error("binding name invalid or oversized"))?;
            self.charge(
                canonical
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| error("canonical binding size overflow"))?,
            )?;
            let ordinal = self
                .ordinals
                .entry((scope, tag, canonical.clone()))
                .or_default();
            let mut e = StableEncoder::new("lrr/v2/original-local-binding");
            e.digest(owner.def_key);
            e.u32(scope);
            e.u32(tag);
            e.string(&canonical);
            e.u32(*ordinal);
            *ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| error("binding duplicate ordinal overflow"))?;
            let classification = if pat.mut_token().is_some()
                || pat.ref_token().is_some()
                || pat.at_token().is_some()
                || pat
                    .syntax()
                    .parent()
                    .and_then(ast::RecordPatField::cast)
                    .is_some_and(|field| field.colon_token().is_none())
            {
                PatternNameKind::ExplicitBinding
            } else {
                PatternNameKind::BindingOrConstant
            };
            result
                .try_reserve(1)
                .map_err(|_| error("binding record allocation failed"))?;
            result.push(LocalBindingRecipe {
                identity: e.finish(),
                scope,
                name: canonical,
                raw_name,
                name_range: range(name.syntax().text_range()),
                pattern_range: range(pattern.syntax().text_range()),
                visible_from,
                form,
                classification,
            });
        }
        Ok(())
    }
}

pub(crate) fn cfg_enabled(
    attrs: impl Iterator<Item = ast::Attr>,
    cfg: &crate::source::snapshot::CfgSet,
) -> Result<bool, Diagnostic> {
    let mut pending = Vec::new();
    for meta in attrs.filter_map(|a| a.meta()) {
        pending
            .try_reserve(1)
            .map_err(|_| error("binding cfg allocation failed"))?;
        pending.push(meta);
    }
    let mut count = 0usize;
    let mut enabled = true;
    while let Some(meta) = pending.pop() {
        count = count
            .checked_add(1)
            .ok_or_else(|| error("binding cfg attribute limit exceeded"))?;
        match meta {
            ast::Meta::CfgMeta(c) => {
                enabled &= super::modules::cfg(
                    c.cfg_predicate()
                        .ok_or_else(|| error("binding cfg predicate missing"))?,
                    cfg,
                    0,
                )?
            }
            ast::Meta::CfgAttrMeta(c)
                if super::modules::cfg(
                    c.cfg_predicate()
                        .ok_or_else(|| error("binding cfg_attr predicate missing"))?,
                    cfg,
                    0,
                )? =>
            {
                for m in c.metas() {
                    pending
                        .try_reserve(1)
                        .map_err(|_| error("binding cfg allocation failed"))?;
                    pending.push(m);
                }
            }
            _ => {}
        }
    }
    Ok(enabled)
}
