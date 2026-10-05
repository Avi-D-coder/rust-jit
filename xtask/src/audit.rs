use std::{fs, path::Path};

pub fn source_violations(module: &str, source: &str) -> Vec<String> {
    let facts = lrr::syntax::audit::source_facts(source);
    let mut errors = Vec::new();
    if facts.parse_errors != 0 {
        errors.push(format!("{module}: {} syntax errors", facts.parse_errors));
    }
    let worker_pure = module.starts_with("db/") || module.starts_with("frontend/");
    let runtime = module.starts_with("runtime/");
    for path in facts.paths {
        let forbidden = if worker_pure {
            [
                "std::fs",
                "std::process",
                "std::net",
                "effects::broker",
                "crate::effects::broker",
                "cranelift",
            ]
            .iter()
            .any(|prefix| path.starts_with(prefix))
        } else if runtime {
            ["ra_ap_syntax", "cranelift"]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        } else {
            false
        };
        if forbidden {
            errors.push(format!("{module}: forbidden import/path {path}"));
        }
        if path.starts_with("ra_ap_syntax") && !module.starts_with("syntax/") {
            errors.push(format!("{module}: parser import outside syntax"));
        }
        if path.starts_with("libloading") && module != "native/loader.rs" {
            errors.push(format!("{module}: loader import outside native loader"));
        }
    }
    if facts.unsafe_tokens != 0
        && ![
            "jit/abi.rs",
            "jit/helpers.rs",
            "jit/module_cache.rs",
            "native/loader.rs",
            "native/ffi.rs",
        ]
        .contains(&module)
    {
        errors.push(format!("{module}: unsafe outside boundary allowlist"));
    }
    errors
}

pub fn architecture(root: &Path) -> Result<(), String> {
    fn visit(base: &Path, at: &Path, errors: &mut Vec<String>) -> Result<(), String> {
        for entry in fs::read_dir(at).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                visit(base, &path, errors)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let module = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                errors.extend(source_violations(
                    &module,
                    &fs::read_to_string(path).map_err(|e| e.to_string())?,
                ));
            }
        }
        Ok(())
    }
    let base = root.join("crates/lrr/src");
    let mut errors = Vec::new();
    visit(&base, &base, &mut errors)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forbidden_worker_import_is_rejected() {
        for source in [
            "use std::fs;",
            "use std::{fs as files};",
            "fn f() { std::fs::read(\"x\"); }",
        ] {
            assert!(
                !source_violations("db/provider.rs", source).is_empty(),
                "{source}"
            );
        }
        assert!(
            source_violations(
                "db/provider.rs",
                "// use std::fs;\nconst S: &str = \"std::fs\";"
            )
            .is_empty()
        );
    }
    #[test]
    fn unsafe_outside_boundary_is_rejected() {
        assert!(!source_violations("runtime/value.rs", "unsafe fn f() {}").is_empty());
        assert!(source_violations("jit/abi.rs", "unsafe fn f() {}").is_empty());
    }
}

pub fn validate_references(root: &Path) -> Result<(), String> {
    let doc: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("compat/fixtures.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut ids = std::collections::BTreeSet::new();
    for fixture in doc["fixture"]
        .as_array()
        .ok_or("missing fixture inventory")?
    {
        let id = fixture["id"].as_str().ok_or("missing fixture id")?;
        if !ids.insert(id) {
            return Err(format!("duplicate fixture {id}"));
        }
        let path = fixture["path"].as_str().ok_or("missing fixture path")?;
        if Path::new(path).is_absolute()
            || Path::new(path)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("fixture path escape: {path}"));
        }
        let source = fs::read_to_string(root.join(path))
            .map_err(|e| format!("missing fixture {id}: {e}"))?;
        let test = fixture["test"].as_str().ok_or("missing fixture test")?;
        if !source.contains(&format!("fn {test}(")) {
            return Err(format!("missing named acceptance test for {id}"));
        }
    }
    for (path, kind) in [
        ("compat/features.toml", "feature"),
        ("compat/diagnostics.toml", "diagnostic"),
    ] {
        let doc: toml::Value =
            toml::from_str(&fs::read_to_string(root.join(path)).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        for entry in doc[kind].as_array().ok_or("missing inventory entries")? {
            let references = entry["fixtures"]
                .as_array()
                .ok_or("missing fixture references")?;
            if references.is_empty() {
                return Err(format!("{path}: empty fixture references"));
            }
            for id in references {
                let id = id.as_str().ok_or("non-string fixture reference")?;
                if !ids.contains(id) {
                    return Err(format!("{path}: unknown fixture {id}"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod inventory_tests {
    #[test]
    fn missing_fixture_reference_fails_audit() {
        use std::fs;
        let temp = std::env::temp_dir().join(format!(
            "lrr-audit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp.join("compat")).unwrap();
        fs::write(
            temp.join("compat/fixtures.toml"),
            "schema = 1\nfixture = []\n",
        )
        .unwrap();
        fs::write(
            temp.join("compat/features.toml"),
            "[[feature]]\nfixtures = [\"MISSING\"]\n",
        )
        .unwrap();
        fs::write(temp.join("compat/diagnostics.toml"), "diagnostic = []\n").unwrap();
        let result = super::validate_references(&temp);
        fs::remove_dir_all(temp).unwrap();
        assert!(result.unwrap_err().contains("unknown fixture MISSING"));
    }
}
