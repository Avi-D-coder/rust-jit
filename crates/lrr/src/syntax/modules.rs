//! Structural module discovery; returns owned requests, never executable meaning.
use crate::{core::diagnostic::Diagnostic, source::snapshot::CfgSet};
use ra_ap_syntax::{
    AstNode, AstToken, Edition,
    ast::{self, HasAttrs, HasName},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleRequest {
    /// Logical module suffix; path attributes change the disk directory separately.
    pub module_path: Vec<String>,
    pub directory: Vec<String>,
    pub file_relative: bool,
    pub name: String,
    pub path_override: Option<String>,
}
/// Physical directory components of an actual captured inline-module chain.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct InlineModuleLayout {
    pub directory: Vec<String>,
    pub file_relative: bool,
}
impl InlineModuleLayout {
    pub(crate) fn append(&mut self, child: Self) -> Result<(), Diagnostic> {
        self.directory
            .len()
            .checked_add(child.directory.len())            .ok_or_else(|| error("combined inline layout depth limit"))?;
        self.directory
            .iter()
            .chain(&child.directory)
            .try_fold(0usize, |n, part| {
                n.checked_add(part.len()).and_then(|n| n.checked_add(64))
            })
            .ok_or_else(|| error("combined inline layout metadata limit"))?;
        self.directory
            .try_reserve(child.directory.len())
            .map_err(|_| error("combined inline layout allocation"))?;
        if self.directory.is_empty() {
            self.file_relative = child.file_relative;
        }
        self.directory.extend(child.directory);
        Ok(())
    }
}
/// Resolve only structurally present, cfg-active inline parents. No filesystem access.
pub(crate) fn inline_module_layout(
    tree: &ast::SourceFile,
    cfg_values: &CfgSet,
    module_path: &[String],
) -> Result<InlineModuleLayout, Diagnostic> {
    if !super::attributes::source_root_enabled(tree, cfg_values)? {
        return Err(error("inline caller root is cfg-disabled"));
    }
    let mut parent = tree.syntax().clone();
    let mut layout = InlineModuleLayout::default();
    let mut work = 0usize;
    let mut bytes = 0usize;
    for wanted in module_path {
        let mut selected = None;
        for module in parent.children().filter_map(ast::Module::cast) {
            work = work
                .checked_add(1)
                .ok_or_else(|| error("inline module layout work limit"))?;
            let Some(name) = module.name() else {
                continue;
            };
            let text = name.text();
            let name = super::ast_adapter::semantic_name(text)
                .ok_or_else(|| error("inline module canonical name unavailable"))?;
            if &name != wanted {
                continue;
            }
            let (enabled, override_path) = attributes(&module, cfg_values)?;
            if !enabled {
                continue;
            }
            let items = module
                .item_list()
                .ok_or_else(|| error("inline caller layout is an external module"))?;
            if selected.replace((items, override_path)).is_some() {
                return Err(error("inline caller layout is ambiguous"));
            }
        }
        let (items, override_path) =
            selected.ok_or_else(|| error("actual captured inline caller layout absent"))?;
        let part = match override_path {
            Some(path) => {
                if layout.directory.is_empty() {
                    layout.file_relative = true;
                }
                path
            }
            None => wanted.clone(),
        };
        bytes = bytes
            .checked_add(part.len())
            .and_then(|n| n.checked_add(64))
            .ok_or_else(|| error("inline module layout metadata limit"))?;
        layout
            .directory
            .try_reserve(1)
            .map_err(|_| error("inline module layout allocation"))?;
        layout.directory.push(part);
        parent = items.syntax().clone();
    }
    Ok(layout)
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-MODULE", message.into())
}

pub(crate) fn cfg(
    predicate: ast::CfgPredicate,
    values: &CfgSet,
    depth: usize,
) -> Result<bool, Diagnostic> {
    match predicate {
        ast::CfgPredicate::CfgAtom(atom) => {
            if atom.true_token().is_some() {
                return Ok(true);
            }
            if atom.false_token().is_some() {
                return Ok(false);
            }
            let name = atom
                .ident_token()
                .ok_or_else(|| error("missing cfg atom"))?
                .text()
                .to_owned();
            if atom.eq_token().is_some() {
                let token = atom
                    .string_token()
                    .ok_or_else(|| error("cfg value must be a string"))?;
                let string = ast::String::cast(token).ok_or_else(|| error("invalid cfg string"))?;
                let value = string
                    .value()
                    .map_err(|e| error(format!("invalid cfg string: {e:?}")))?;
                Ok(values
                    .values
                    .get(&name)
                    .is_some_and(|set| set.contains(value.as_ref())))
            } else {
                Ok(values.atoms.contains(&name))
            }
        }
        ast::CfgPredicate::CfgComposite(composite) => {
            let keyword = composite
                .keyword()
                .ok_or_else(|| error("missing cfg operator"))?
                .text()
                .to_owned();
            let children: Vec<_> = composite.cfg_predicates().collect();
            match keyword.as_str() {
                "all" | "any" => {
                    let mut results = Vec::new();
                    for child in children {
                        results.push(cfg(child, values, depth + 1)?);
                    }
                    Ok(if keyword == "all" {
                        results.into_iter().all(|v| v)
                    } else {
                        results.into_iter().any(|v| v)
                    })
                }
                "not" if children.len() == 1 => Ok(!cfg(
                    children
                        .into_iter()
                        .next()
                        .ok_or_else(|| error("missing not operand"))?,
                    values,
                    depth + 1,
                )?),
                _ => Err(error(format!(
                    "unsupported cfg operator or arity: {keyword}"
                ))),
            }
        }
    }
}

fn attributes(module: &ast::Module, values: &CfgSet) -> Result<(bool, Option<String>), Diagnostic> {
    let mut pending: Vec<_> = module.attrs().filter_map(|a| a.meta()).collect();
    let mut path = None;
    let mut enabled = true;
    while let Some(meta) = pending.pop() {
        match meta {
            ast::Meta::CfgMeta(meta) => {
                enabled &= cfg(
                    meta.cfg_predicate()
                        .ok_or_else(|| error("missing cfg predicate"))?,
                    values,
                    0,
                )?;
            }
            ast::Meta::CfgAttrMeta(meta) => {
                if cfg(
                    meta.cfg_predicate()
                        .ok_or_else(|| error("missing cfg_attr predicate"))?,
                    values,
                    0,
                )? {
                    pending.extend(meta.metas());
                }
            }
            ast::Meta::KeyValueMeta(meta)
                if meta.path().is_some_and(|p| p.syntax().text() == "path") =>
            {
                let expr = meta
                    .expr()
                    .ok_or_else(|| error("path attribute requires a string"))?;
                let ast::Expr::Literal(literal) = expr else {
                    return Err(error("path attribute must be a literal string"));
                };
                let token = literal
                    .syntax()
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find_map(ast::String::cast)
                    .ok_or_else(|| error("path attribute must be a UTF-8 string"))?;
                let value = token
                    .value()
                    .map_err(|e| error(format!("invalid path string: {e:?}")))?
                    .into_owned();
                if path.replace(value).is_some() {
                    return Err(error("duplicate path attribute"));
                }
            }
            _ => {} // Other attributes are retained/checked by the declaration outline.
        }
    }
    Ok((enabled, path))
}

pub fn external_modules(text: &str, cfg_values: &CfgSet) -> Result<Vec<ModuleRequest>, Diagnostic> {
    let parsed = ast::SourceFile::parse(text, Edition::Edition2024);
    external_modules_from_tree(&parsed.tree(), cfg_values)
}
pub fn external_modules_in_profile(
    text: &str,
    cfg_values: &CfgSet,
    profile: &crate::core::profile::ProfileIdentity,
) -> Result<Vec<ModuleRequest>, Diagnostic> {
    let blob = crate::source::snapshot::SourceBlob::new(text.as_bytes().to_vec())?;
    let parsed = super::parse::ParsedFile::parse(&blob, profile.clone())?;
    external_modules_from_tree(&parsed.parsed.tree(), cfg_values)
}
pub(crate) fn external_modules_from_tree(
    tree: &ast::SourceFile,
    cfg_values: &CfgSet,
) -> Result<Vec<ModuleRequest>, Diagnostic> {
    let mut result = external_module_declarations_from_tree(tree, cfg_values)?
        .into_iter()
        .map(|declaration| declaration.request)
        .collect::<Vec<_>>();
    result.sort_by(|a, b| {
        (&a.directory, &a.name, &a.path_override).cmp(&(&b.directory, &b.name, &b.path_override))
    });
    Ok(result)
}
#[derive(Debug, Clone)]
pub struct ExternalModuleDeclaration {
    pub request: ModuleRequest,
    pub range: super::spans::SourceRange,
}
pub(crate) fn external_module_declarations_from_tree(
    tree: &ast::SourceFile,
    cfg_values: &CfgSet,
) -> Result<Vec<ExternalModuleDeclaration>, Diagnostic> {
    if !super::attributes::source_root_enabled(tree, cfg_values)? {
        return Ok(Vec::new());
    }
    let mut queue: Vec<_> = tree
        .syntax()
        .children()
        .filter_map(ast::Module::cast)
        .map(|m| (m, Vec::<String>::new(), false, Vec::<String>::new()))
        .collect();
    let mut result = Vec::new();
    while let Some((module, directory, file_relative, mut module_path)) = queue.pop() {
        let (enabled, path_override) = attributes(&module, cfg_values)?;
        if !enabled {
            continue;
        }
        let name = module
            .name()
            .ok_or_else(|| error("module name unavailable"))?;
        let text = name.text();
        let name = super::ast_adapter::semantic_name(text)
            .ok_or_else(|| error("module name unavailable"))?;
        module_path.push(name.clone());
        if let Some(items) = module.item_list() {
            let (mut child_directory, child_file_relative) = if let Some(path) = path_override {
                if directory.is_empty() {
                    (vec![path], true)
                } else {
                    let mut d = directory;
                    d.push(path);
                    (d, file_relative)
                }
            } else {
                let mut d = directory;
                d.push(name);
                (d, file_relative)
            };
            child_directory.shrink_to_fit();
            queue.extend(
                items
                    .syntax()
                    .children()
                    .filter_map(ast::Module::cast)
                    .map(|m| {
                        (
                            m,
                            child_directory.clone(),
                            child_file_relative,
                            module_path.clone(),
                        )
                    }),
            );
        } else {
            if module.semicolon_token().is_none() {
                return Err(error("module boundary unavailable after parser recovery"));
            }
            let range = module.syntax().text_range();
            result.push(ExternalModuleDeclaration {
                request: ModuleRequest {
                    module_path,
                    directory,
                    file_relative,
                    name,
                    path_override,
                },
                range: super::spans::SourceRange {
                    start: range.start().into(),
                    end: range.end().into(),
                },
            });
        }
    }
    result.sort_by_key(|declaration| (declaration.range.start, declaration.range.end));
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn inline_caller_layout_refuses_cfg_disabled_source_root() {
        let cfg = crate::source::snapshot::CfgSet::guest_default();
        let disabled = ra_ap_syntax::ast::SourceFile::parse(
            "#![cfg(false)] mod outer {}",
            ra_ap_syntax::Edition::Edition2024,
        );
        let error =
            super::inline_module_layout(&disabled.tree(), &cfg, &["outer".into()]).unwrap_err();
        assert_eq!(error.code, "LRR-E-SOURCE-MODULE");
        assert_eq!(error.message, "inline caller root is cfg-disabled");
        let active = ra_ap_syntax::ast::SourceFile::parse(
            "#![cfg(true)] mod outer {}",
            ra_ap_syntax::Edition::Edition2024,
        );
        let layout = super::inline_module_layout(&active.tree(), &cfg, &["outer".into()]).unwrap();
        assert_eq!(layout.directory, ["outer"]);
    }
    #[test]
    fn module_discovery_respects_cfg_and_inline_directories() {
        let modules = super::external_modules("mod a { mod b; } #[cfg(any())] mod absent; #[cfg_attr(unix, path = r#\"other.rs\"#)] mod c; fn cold() { missing(); }", &crate::source::snapshot::CfgSet::guest_default()).unwrap();
        assert_eq!(modules.len(), 2);
        assert_eq!(
            modules.iter().find(|m| m.name == "b").unwrap().directory,
            vec!["a"]
        );
        assert_eq!(
            modules
                .iter()
                .find(|m| m.name == "c")
                .unwrap()
                .path_override
                .as_deref(),
            Some("other.rs")
        );
    }
}

#[cfg(test)]
mod invalid_tests {
    #[test]
    fn invalid_module_path_attribute_is_error() {
        assert_eq!(
            super::external_modules(
                "#[path=42] mod child;",
                &crate::source::snapshot::CfgSet::guest_default()
            )
            .unwrap_err()
            .code,
            "LRR-E-SOURCE-MODULE"
        );
    }
}
