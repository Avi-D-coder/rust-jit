//! Prelude-header source selection for pattern names; no name authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::snapshot::CfgSet,
    syntax::{outline::ScopeKey, parse::ParsedFile},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
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
pub(crate) struct BindingPreludePolicies {
    source: crate::syntax::parse::ParseKey,
    cfg: Digest,
    standard: bool,
    disabled: Vec<Vec<String>>,
}
impl BindingPreludePolicies {
    pub(crate) fn for_scope(
        &self,
        source: &crate::syntax::parse::ParseKey,
        cfg: Digest,
        scope: &ScopeKey,
    ) -> Result<BindingPreludePolicy, Diagnostic> {
        if &self.source != source || self.cfg != cfg {
            return Err(fault("Value prelude policy source/cfg differs"));
        }
        Ok(BindingPreludePolicy {
            standard: self.standard,
            disabled: self
                .disabled
                .iter()
                .any(|prefix| scope.module_path.starts_with(prefix)),
        })
    }
}
impl ParsedFile {
    pub(crate) fn prepare_binding_prelude_policies(
        &self,
        cfg: &CfgSet,
    ) -> Result<Option<BindingPreludePolicies>, Diagnostic> {
        if self.generated_origin.is_some() {
            return Err(fault(
                "generated Value prelude policy requires actual owning-crate origin",
            ));
        }
        let policy = self.live_macro_prelude_policy(cfg)?;
        let mut stack = vec![self.parsed.tree().syntax().clone()];
        while let Some(node) = stack.pop() {
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
                stack
                    .try_reserve(1)
                    .map_err(|_| fault("Value prelude header allocation"))?;
                stack.push(child);
            }
        }
        Ok(Some(BindingPreludePolicies {
            source: self.key(),
            cfg: cfg.fingerprint(),
            standard: policy.standard,
            disabled: policy.disabled,
        }))
    }
}
