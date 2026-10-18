//! Exact original declaration groups. AST data only, never source authority.
use super::*;
use ra_ap_syntax::ast::HasName;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum MacroDefinitionDialect {
    Rules,
    Macro2,
}
#[derive(Debug, Clone)]
pub(crate) struct MacroDefinitionShape {
    dialect: MacroDefinitionDialect,
    label: SourceRange,
    name_range: SourceRange,
    name: String,
    arguments: Option<SourceRange>,
    body: SourceRange,
}
impl MacroDefinitionShape {
    pub(crate) fn dialect(&self) -> MacroDefinitionDialect {
        self.dialect
    }
    pub(crate) fn label(&self) -> SourceRange {
        self.label
    }
    /// The actual identifier token, distinct from the keyword-through-name
    /// label used by diagnostics and source definition origins.
    pub(crate) fn name_range(&self) -> SourceRange {
        self.name_range
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn arguments(&self) -> Option<SourceRange> {
        self.arguments
    }
    pub(crate) fn body(&self) -> SourceRange {
        self.body
    }
}
impl ParsedFile {
    pub(crate) fn macro_definition_shape_at(
        &self,
        selected: SourceRange,
    ) -> Result<MacroDefinitionShape, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty()
            || selected.start > selected.end
            || !range(root.syntax().text_range()).contains(selected)
        {
            return Err(error(
                "macro declaration selection is outside captured source",
            ));
        }
        let node = root
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(selected.start),
                TextSize::from(selected.end),
            ))
            .into_node()
            .ok_or_else(|| error("macro declaration selection is not a node"))?;
        if self.errors.iter().any(|d| {
            selected.contains(d.range)
                || (d.range.start < selected.end && selected.start < d.range.end)
        }) {
            return Err(error("macro declaration contains syntax diagnostics"));
        }
        for node in node.ancestors() {
            if range(node.text_range()) != selected {
                continue;
            }
            let (dialect, keyword, name, args, body) =
                if let Some(m) = ast::MacroRules::cast(node.clone()) {
                    (
                        MacroDefinitionDialect::Rules,
                        m.macro_rules_token(),
                        m.name(),
                        None,
                        m.token_tree(),
                    )
                } else if let Some(m) = ast::MacroDef::cast(node) {
                    if self.generated_origin.is_some() {
                        return Err(Diagnostic::new(
                            "LRR-E-MACRO2-SOURCE",
                            "generated macro2 source dialect admission is not established".into(),
                        ));
                    }
                    (
                        MacroDefinitionDialect::Macro2,
                        m.macro_token(),
                        m.name(),
                        m.args(),
                        m.body(),
                    )
                } else {
                    continue;
                };
            let keyword = keyword.ok_or_else(|| error("exact macro keyword absent"))?;
            let name = name.ok_or_else(|| error("exact macro name absent"))?;
            let name_range = range(name.syntax().text_range());
            let label =
                SourceRange::checked(u32::from(keyword.text_range().start()), name_range.end)?;
            let name = semantic_name(&name.syntax().text().to_string())
                .ok_or_else(|| error("exact macro name invalid"))?;
            let arguments = args.map(|a| range(a.syntax().text_range()));
            let body = range(
                body.ok_or_else(|| error("exact macro body absent"))?
                    .syntax()
                    .text_range(),
            );
            if !selected.contains(label)
                || !selected.contains(body)
                || arguments.is_some_and(|a| !selected.contains(a) || a.end > body.start)
            {
                return Err(error(
                    "macro argument/body groups differ from declaration owner",
                ));
            }
            return Ok(MacroDefinitionShape {
                dialect,
                label,
                name_range,
                name,
                arguments,
                body,
            });
        }
        Err(error(
            "selection is not an exact original macro declaration",
        ))
    }
}
