use lrr::{
    core::digest::Digest,
    expand::compile_input_identity::descriptor,
    source::{
        include_paths::{
            IncludeAuthorities, IncludeKind, IncludeLocation, IncludeRoot, IncludeRootKind,
        },
        snapshot::SourcePath,
    },
    syntax::token_wire::Key,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn location(unit: Digest, root: Digest, path: &str) -> IncludeLocation {
    IncludeLocation {
        unit,
        root,
        path: SourcePath::new(path).unwrap(),
        origin: Key([3; 32]),
    }
}
fn authorities(generation: Digest) -> IncludeAuthorities {
    IncludeAuthorities::new(
        Digest([1; 32]),
        generation,
        [
            IncludeRoot {
                identity: Digest([2; 32]),
                path: "/controlled/pkg".into(),
                kind: IncludeRootKind::Source,
            },
            IncludeRoot {
                identity: Digest([4; 32]),
                path: "/controlled/pkg/target/out".into(),
                kind: IncludeRootKind::BuildOutput {
                    producer: Digest([5; 32]),
                },
            },
        ],
    )
    .unwrap()
}
#[test]
fn include_requests_preserve_nested_bases_and_select_exact_build_output_authority() {
    let a = authorities(Digest([6; 32]));
    let from = location(Digest([1; 32]), Digest([2; 32]), "src/root.rs");
    let d = descriptor("include").unwrap().unwrap();
    let first = a.request(&d, &from, "nested/one.rs", Key([7; 32])).unwrap();
    assert_eq!(first.path().as_str(), "src/nested/one.rs");
    assert_eq!(first.kind(), IncludeKind::Syntax);
    assert_eq!(first.from(), &from);
    let second = a
        .request(
            &d,
            &first.included_location(),
            "deeper/two.rs",
            Key([8; 32]),
        )
        .unwrap();
    assert_eq!(second.path().as_str(), "src/nested/deeper/two.rs");
    assert_eq!(second.root(), from.root);
    let parent = a
        .request(&d, &first.included_location(), "../shared.rs", Key([9; 32]))
        .unwrap();
    assert_eq!(parent.path().as_str(), "src/shared.rs");
    assert!(
        parent
            .original_target()
            .to_str()
            .unwrap()
            .contains("nested/../shared.rs")
    );
    let out = a
        .request(
            &d,
            &from,
            "/controlled/pkg/target/out/generated.rs",
            Key([10; 32]),
        )
        .unwrap();
    assert_eq!(out.root(), Digest([4; 32]));
    assert_eq!(out.path().as_str(), "generated.rs");
    assert_eq!(
        out.root_kind(),
        IncludeRootKind::BuildOutput {
            producer: Digest([5; 32])
        }
    );
    let nested = a
        .request(&d, &out.included_location(), "sub/value.rs", Key([11; 32]))
        .unwrap();
    assert_eq!(nested.root(), out.root());
    assert_eq!(nested.path().as_str(), "sub/value.rs");
    assert_eq!(
        first.identity(),
        a.request(&d, &from, "nested/one.rs", Key([7; 32]))
            .unwrap()
            .identity()
    );
    assert_ne!(
        first.identity(),
        authorities(Digest([12; 32]))
            .request(&d, &from, "nested/one.rs", Key([7; 32]))
            .unwrap()
            .identity()
    );
    assert_ne!(
        first.identity(),
        a.request(&d, &from, "nested/one.rs", Key([13; 32]))
            .unwrap()
            .identity()
    );
}
#[test]
fn include_authority_denial_preserves_both_origins_and_does_not_use_runtime_grants() {
    let a = authorities(Digest([6; 32]));
    let from = location(Digest([1; 32]), Digest([2; 32]), "src/root.rs");
    let d = descriptor("include_bytes").unwrap().unwrap();
    for path in ["../../outside.rs", "/runtime-readable/secret.rs"] {
        let error = a.request(&d, &from, path, Key([8; 32])).err().unwrap();
        assert_eq!(error.code, "LRR-E-SOURCE-PATH");
        assert!(error.message.contains("src/root.rs"));
        assert!(error.message.contains(path));
        assert!(error.message.contains(&Digest(from.origin.0).hex()));
        assert!(error.message.contains(&Digest([8; 32]).hex()));
    }
    let mut foreign = from.clone();
    foreign.unit = Digest([0; 32]);
    assert!(a.request(&d, &foreign, "file.bin", Key([8; 32])).is_err());
    foreign = from.clone();
    foreign.root = Digest([0; 32]);
    assert!(a.request(&d, &foreign, "file.bin", Key([8; 32])).is_err());
    assert!(
        a.request(
            &descriptor("env").unwrap().unwrap(),
            &from,
            "file",
            Key([8; 32])
        )
        .is_err()
    );
    for invalid in ["".to_owned(), "bad\0path".to_owned(), "x".repeat(65_537)] {
        let error = a.request(&d, &from, &invalid, Key([8; 32])).err().unwrap();
        assert!(error.message.len() < 3000);
    }
    let path = PathBuf::from("/controlled/pkg");
    assert!(
        IncludeAuthorities::new(
            from.unit,
            Digest([6; 32]),
            [
                IncludeRoot {
                    identity: from.root,
                    path: path.clone(),
                    kind: IncludeRootKind::Source
                },
                IncludeRoot {
                    identity: Digest([9; 32]),
                    path,
                    kind: IncludeRootKind::Source
                }
            ]
        )
        .is_err()
    );
    assert!(
        IncludeAuthorities::new(
            from.unit,
            Digest([6; 32]),
            [IncludeRoot {
                identity: from.root,
                path: PathBuf::from("relative"),
                kind: IncludeRootKind::Source
            }]
        )
        .is_err()
    );
    let deep = format!("/{}", vec!["a"; 257].join("/"));
    assert!(
        IncludeAuthorities::new(
            from.unit,
            Digest([6; 32]),
            [IncludeRoot {
                identity: from.root,
                path: deep.into(),
                kind: IncludeRootKind::Source
            }]
        )
        .is_err()
    );
    let bytes = a.request(&d, &from, "file.bin", Key([8; 32])).unwrap();
    assert_eq!(bytes.kind(), IncludeKind::Bytes);
    let string = a
        .request(
            &descriptor("include_str").unwrap().unwrap(),
            &from,
            "file.bin",
            Key([8; 32]),
        )
        .unwrap();
    assert_eq!(string.kind(), IncludeKind::Utf8);
    assert_ne!(bytes.identity(), string.identity());
}
#[test]
fn actual_native_nested_relative_and_outdir_includes_agree_with_pure_routes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-include-routes-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(root.join("src/nested/deeper")).unwrap();
    fs::create_dir_all(root.join("out")).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let unit = Digest([1; 32]);
    let source_root = Digest([2; 32]);
    let output_root = Digest([4; 32]);
    let a = IncludeAuthorities::new(
        unit,
        Digest([6; 32]),
        [
            IncludeRoot {
                identity: source_root,
                path: root.clone(),
                kind: IncludeRootKind::Source,
            },
            IncludeRoot {
                identity: output_root,
                path: root.join("out"),
                kind: IncludeRootKind::BuildOutput {
                    producer: Digest([5; 32]),
                },
            },
        ],
    )
    .unwrap();
    let d = descriptor("include").unwrap().unwrap();
    let caller = location(unit, source_root, "src/root.rs");
    let first = a
        .request(&d, &caller, "nested/one.rs", Key([7; 32]))
        .unwrap();
    let second = a
        .request(
            &d,
            &first.included_location(),
            "deeper/two.rs",
            Key([8; 32]),
        )
        .unwrap();
    let generated = a
        .request(
            &d,
            &caller,
            root.join("out/generated.rs").to_str().unwrap(),
            Key([9; 32]),
        )
        .unwrap();
    assert_eq!(second.path().as_str(), "src/nested/deeper/two.rs");
    assert_eq!(generated.root(), output_root);
    fs::write(
        root.join(first.path().as_str()),
        "include!(\"deeper/two.rs\");",
    )
    .unwrap();
    fs::write(
        root.join(second.path().as_str()),
        "fn nested_value()->i32{42}",
    )
    .unwrap();
    fs::write(root.join("out/generated.rs"), "const GENERATED:i32=31;").unwrap();
    fs::write(root.join(caller.path.as_str()),"include!(\"nested/one.rs\"); include!(concat!(env!(\"OUT_DIR\"),\"/generated.rs\")); fn main(){println!(\"{}\",nested_value()+GENERATED);}").unwrap();
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "include_route_oracle",
        ])
        .arg(root.join(caller.path.as_str()))
        .arg("-o")
        .arg(root.join("native"))
        .env("OUT_DIR", root.join("out"))
        .output()
        .unwrap();
    fs::write(root.join("native.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let native = Command::new(root.join("native")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "73");
    let missing = a
        .request(&d, &caller, "not_created.rs", Key([10; 32]))
        .unwrap();
    assert!(!Path::new(missing.original_target()).exists());
    fs::write(root.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"pure include path routing/native lexical base comparison; no LRR capture or nested builtin execution claim","first":first.path().as_str(),"second":second.path().as_str(),"generated":generated.path().as_str(),"generated_root":generated.root(),"missing_request":missing.identity(),"native_stdout":stdout})).unwrap()).unwrap();
}
