//! Checkout ownership for explicit developer validation.
use std::path::{Path, PathBuf};

pub(crate) fn checked_root(
    compiled_manifest: &Path,
    expected: Option<&Path>,
) -> Result<PathBuf, String> {
    let compiled = compiled_manifest
        .parent()
        .ok_or("compiled manifest directory lacks its workspace root")?;
    let Some(expected) = expected else {
        return Ok(compiled.to_owned());
    };
    let expected = expected
        .canonicalize()
        .map_err(|error| format!("cannot resolve validation source root: {error}"))?;
    let compiled = compiled
        .canonicalize()
        .map_err(|error| format!("cannot resolve compiled source root: {error}"))?;
    if compiled != expected {
        return Err(format!(
            "validation binary belongs to another checkout: compiled={}, expected={}",
            compiled.display(),
            expected.display()
        ));
    }
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_source_accepts_the_compiled_checkout_and_canonical_alias() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().unwrap();
        assert_eq!(
            checked_root(manifest, Some(root)).unwrap(),
            root.canonicalize().unwrap()
        );
        assert_eq!(
            checked_root(manifest, Some(&root.join("."))).unwrap(),
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn validation_source_refuses_a_different_existing_checkout_before_dispatch() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let other = manifest.parent().unwrap().join("fixtures");
        assert!(other.is_dir());
        let error = checked_root(manifest, Some(&other)).unwrap_err();
        assert!(error.contains("validation binary belongs to another checkout"));
    }

    #[test]
    fn validation_source_refuses_an_unresolvable_expected_root() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let expected = manifest.join("Cargo.toml").join("not-a-directory");
        assert!(
            checked_root(manifest, Some(&expected))
                .unwrap_err()
                .contains("validation source root")
        );
    }

    #[test]
    fn validation_source_preserves_ordinary_invocation_without_an_expected_root() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            checked_root(manifest, None).unwrap(),
            manifest.parent().unwrap()
        );
    }
}
