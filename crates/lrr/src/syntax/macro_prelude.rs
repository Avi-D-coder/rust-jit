//! Pure cfg-aware source prelude facts; the bound world admits live roles.
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    source::snapshot::CfgSet,
    syntax::parse::ParsedFile,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};
#[derive(Clone)]
pub(crate) struct MacroPreludePolicy {
    pub standard: bool,
    pub no_core: bool,
    pub disabled: Vec<Vec<String>>,
}
impl ParsedFile {
    pub(crate) fn macro_prelude_policy(
        &self,
        cfg: &CfgSet,
    ) -> Result<MacroPreludePolicy, Diagnostic> {
        let root = self.parsed.tree();
        let flags = |attrs: Vec<ast::Attr>| -> Result<Vec<String>, Diagnostic> {
            let (_, metas) = super::attributes::effective_metas(
                attrs.into_iter().filter_map(|a| a.meta()).collect(),
                cfg,
            )?;
            Ok(metas
                .into_iter()
                .filter_map(|m| m.simple_name().map(|s| s.to_string()))
                .collect())
        };
        let root_flags = flags(root.attrs().collect())?;
        let no_core = root_flags.iter().any(|s| s == "no_core");
        let mut policy = MacroPreludePolicy {
            standard: !no_core && !root_flags.iter().any(|s| s == "no_std"),
            no_core,
            disabled: if no_core || root_flags.iter().any(|s| s == "no_implicit_prelude") {
                vec![vec![]]
            } else {
                vec![]
            },
        };
        let mut disabled_bytes = 0usize;
        for module in root.syntax().descendants().filter_map(ast::Module::cast) {
            if module.syntax().ancestors().any(|n| {
                ast::Fn::can_cast(n.kind())
                    || ast::Const::can_cast(n.kind())
                    || ast::Static::can_cast(n.kind())
            }) {
                continue;
            }
            let mut attrs: Vec<_> = module.attrs().collect();
            if let Some(list) = module.item_list() {
                attrs.extend(list.attrs());
            }
            if !flags(attrs)?.iter().any(|s| s == "no_implicit_prelude") {
                continue;
            }
            let mut path = module
                .syntax()
                .ancestors()
                .filter_map(ast::Module::cast)
                .filter_map(|m| m.name())
                .map(|n| MacroName::parse(n.text(), self.edition).map(|n| n.as_str().to_owned()))
                .collect::<Result<Vec<_>, _>>()?;
            path.reverse();
            disabled_bytes = path
                .iter()
                .try_fold(disabled_bytes, |total, name| {
                    total.checked_add(name.capacity())
                })
                .and_then(|n| n.checked_add(64))
                .ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "macro prelude aggregate policy budget".into(),
                    )
                })?;
            policy.disabled.push(path);
        }
        policy.disabled.sort();
        policy.disabled.dedup();
        Ok(policy)
    }
    pub(crate) fn live_macro_prelude_policy(
        &self,
        cfg: &CfgSet,
    ) -> Result<MacroPreludePolicy, Diagnostic> {
        let policy = self.macro_prelude_policy(cfg)?;
        if policy.no_core {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "no_core is outside the live runtime profile".into(),
            ));
        }
        Ok(policy)
    }
}
