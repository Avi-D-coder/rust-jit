//! Structural module discovery; returns owned requests, never executable meaning.
use crate::{core::diagnostic::Diagnostic, source::snapshot::CfgSet};
use ra_ap_syntax::{
    AstNode, AstToken, Edition,
    ast::{self, HasAttrs, HasName},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleRequest {
    pub directory: Vec<String>,
    pub file_relative: bool,
    pub name: String,
    pub path_override: Option<String>,
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: "LRR-E-SOURCE-MODULE",
        message: message.into(),
    }
}

fn cfg(predicate: ast::CfgPredicate, values: &CfgSet, depth: usize) -> Result<bool, Diagnostic> {
    if depth >= 128 {
        return Err(error("cfg nesting exceeds 128"));
    }
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
    let mut processed = 0;
    while let Some(meta) = pending.pop() {
        processed += 1;
        if processed > 100_000 {
            return Err(error("module attribute limit exceeded"));
        }
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
                if meta
                    .path()
                    .is_some_and(|p| p.syntax().text().to_string() == "path") =>
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
    let mut queue: Vec<_> = parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::Module::cast)
        .map(|m| (m, Vec::<String>::new(), false))
        .collect();
    let mut result = Vec::new();
    let mut count = 0;
    while let Some((module, directory, file_relative)) = queue.pop() {
        count += 1;
        if count > 100_000 {
            return Err(error("module declaration limit exceeded"));
        }
        let (enabled, path_override) = attributes(&module, cfg_values)?;
        if !enabled {
            continue;
        }
        let name = module
            .name()
            .ok_or_else(|| error("module name unavailable"))?
            .text()
            .to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name).to_owned();
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
                    .map(|m| (m, child_directory.clone(), child_file_relative)),
            );
        } else {
            if module.semicolon_token().is_none() {
                return Err(error("module boundary unavailable after parser recovery"));
            }
            result.push(ModuleRequest {
                directory,
                file_relative,
                name,
                path_override,
            });
        }
    }
    result.sort_by(|a, b| {
        (&a.directory, &a.name, &a.path_override).cmp(&(&b.directory, &b.name, &b.path_override))
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
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
