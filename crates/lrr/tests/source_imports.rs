use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        imports::{ImportKind, ImportSegment},
        outline::{Outline, Visibility},
        parse::ParsedFile,
    },
};
fn outline(source: &str) -> Result<Outline, lrr::core::diagnostic::Diagnostic> {
    ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(Digest([1; 32]), vec![], &CfgSet::default())
}
#[test]
fn source_imports_preserve_group_paths_raw_names_visibility_and_cold_bodies() {
    let index = outline("pub(crate) use ::dep::{self, r#gen as r#type, nested::{cafe\u{301}, *}, hidden as _}; mod inner { use super::r#gen; use crate::m; use self::n; } #[cfg(any())] use absent::*; fn cold(){use unavailable::*;} ").unwrap();
    assert_eq!(index.imports.len(), 8);
    let first = &index.imports[0];
    assert_eq!(
        first.path,
        vec![ImportSegment::Name("dep".into()), ImportSegment::SelfModule]
    );
    assert!(first.absolute);
    assert_eq!(first.visibility, Visibility::Crate);
    assert_eq!(
        index.imports[1].path[1],
        ImportSegment::Name("r#gen".into())
    );
    assert_eq!(
        index.imports[1].kind,
        ImportKind::Named {
            alias: Some("r#type".into())
        }
    );
    assert_eq!(
        index.imports[2].path[2],
        ImportSegment::Name("cafe\u{301}".into())
    );
    assert_eq!(index.imports[3].kind, ImportKind::Glob);
    assert_eq!(index.imports[4].kind, ImportKind::Anonymous);
    for (import, prefix) in index.imports[5..].iter().zip([
        ImportSegment::Super,
        ImportSegment::Crate,
        ImportSegment::SelfModule,
    ]) {
        assert_eq!(import.path[0], prefix);
        assert_eq!(import.scope.module_path, vec!["inner"]);
        assert!(!import.absolute);
    }
    assert_eq!(index.imports[0].identity, index.imports[4].identity);
    assert_eq!(index.imports[0].leaf_ordinal, 0);
    assert_eq!(index.imports[4].leaf_ordinal, 4);
    assert!(index.imports[0].range.start < index.imports[4].range.start);
}
#[test]
fn source_imports_reject_deep_paths_and_expanded_metadata_budgets() {
    let deep = format!("use {}::m;", vec!["a"; 256].join("::"));
    assert!(outline(&deep).is_err());
    let prefix = "a".repeat(4096);
    let leaves = vec!["m"; 8200].join(",");
    assert!(outline(&format!("use {prefix}::{{{leaves}}};")).is_err());
}
