use lrr::{
    core::digest::{Digest, StableEncode},
    expand::compile_input_identity::descriptor,
    service::source_capture::SourceCapture,
    source::{
        generated::GeneratedRoot,
        include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
        snapshot::SourcePath,
    },
    syntax::token_wire::Key,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn root(label: &str) -> PathBuf {
    static DIRECTORY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = DIRECTORY_SEQUENCE
        .try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m011-capture-{label}-{sequence}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&path).unwrap();
    fs::canonicalize(path).unwrap()
}
fn caller() -> IncludeLocation {
    IncludeLocation {
        unit: Digest([1; 32]),
        root: Digest([2; 32]),
        path: SourcePath::new("root.rs").unwrap(),
        origin: Key([3; 32]),
    }
}
fn source_authority(path: &std::path::Path, generation: Digest) -> IncludeAuthorities {
    IncludeAuthorities::new(
        caller().unit,
        generation,
        [IncludeRoot {
            identity: caller().root,
            path: path.to_owned(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap()
}
#[test]
fn actual_include_capture_preserves_binary_bytes_utf8_errors_and_stale_request_denial() {
    let path = root("binary");
    fs::write(path.join("data.bin"), [255u8, 0]).unwrap();
    let authorities = source_authority(&path, Digest([4; 32]));
    let request = authorities
        .request(
            &descriptor("include_bytes").unwrap().unwrap(),
            &caller(),
            "data.bin",
            Key([5; 32]),
        )
        .unwrap();
    let string = authorities
        .request(
            &descriptor("include_str").unwrap().unwrap(),
            &caller(),
            "data.bin",
            Key([5; 32]),
        )
        .unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture(&request).unwrap();
    assert_eq!(record.blob().bytes(), [255u8, 0]);
    assert_eq!(record.request(), request.identity());
    assert_eq!(record.location().path.as_str(), "data.bin");
    let error = capture.capture(&string).err().unwrap();
    assert_eq!(error.code, "LRR-E-SOURCE-UTF8");
    assert!(error.message.contains(&Digest([3; 32]).hex()));
    assert!(error.message.contains(&Digest([5; 32]).hex()));
    fs::write(
        path.join("root.rs"),
        "fn main(){let b=include_bytes!(\"data.bin\");println!(\"{}:{}\",b[0],b[1]);}",
    )
    .unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name", "binary_oracle"])
        .arg(path.join("root.rs"))
        .arg("-o")
        .arg(path.join("native"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(path.join("native")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!("{}:{}", record.blob().bytes()[0], record.blob().bytes()[1])
    );
    fs::write(path.join("data.bin"), [1u8, 2]).unwrap();
    assert_eq!(record.blob().bytes(), [255u8, 0]);
    let fresh =
        SourceCapture::open(source_authority(&path, Digest([6; 32])), BTreeMap::new()).unwrap();
    assert_eq!(
        fresh.capture(&request).err().unwrap().code,
        "LRR-E-SOURCE-PATH"
    );
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"coordinator binary capture/native bytes; no source adoption/guest claim","capture":record.identity(),"native_stdout":stdout,"utf8_code":error.code})).unwrap()).unwrap();
}
#[test]
fn actual_capability_include_capture_denies_escape_and_preserves_raw_symlink_parent_semantics() {
    let path = root("symlinks");
    let outside = root("outside");
    fs::write(outside.join("secret.rs"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.rs"), path.join("escape.rs")).unwrap();
    fs::create_dir_all(path.join("real/deep")).unwrap();
    std::os::unix::fs::symlink("real/deep", path.join("link")).unwrap();
    fs::write(path.join("target.rs"), "fn value()->i32{11}").unwrap();
    fs::write(path.join("real/target.rs"), "fn value()->i32{73}").unwrap();
    let authorities = source_authority(&path, Digest([4; 32]));
    let d = descriptor("include").unwrap().unwrap();
    let escape = authorities
        .request(&d, &caller(), "escape.rs", Key([5; 32]))
        .unwrap();
    let raw = authorities
        .request(&d, &caller(), "link/../target.rs", Key([5; 32]))
        .unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let error = capture.capture(&escape).err().unwrap();
    assert_eq!(error.code, "LRR-E-SOURCE-PATH");
    assert!(error.message.contains(&Digest([3; 32]).hex()));
    assert!(error.message.contains(&Digest([5; 32]).hex()));
    let record = capture.capture(&raw).unwrap();
    assert_eq!(record.location().path.as_str(), "real/target.rs");
    assert!(
        std::str::from_utf8(record.blob().bytes())
            .unwrap()
            .contains("73")
    );
    fs::write(
        path.join("root.rs"),
        "include!(\"link/../target.rs\");fn main(){println!(\"{}\",value());}",
    )
    .unwrap();
    let built = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "symlink_base_oracle",
        ])
        .arg(path.join("root.rs"))
        .arg("-o")
        .arg(path.join("native"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(path.join("native")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "73");
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","scope":"capability canonicalization/source capture; no query/guest adoption claim","escape_code":error.code,"original":record.original_path(),"canonical":record.location().path.as_str(),"native_stdout":stdout})).unwrap()).unwrap();
}
#[test]
fn actual_include_build_output_capture_uses_immutable_tree_after_directory_removal() {
    use lrr::project::cargo_units::{CargoUnitKey, TargetKind, UnitRole};
    let path = root("output");
    let out = path.join("out");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("generated.rs"), "const GENERATED:i32=31;").unwrap();
    let unit = CargoUnitKey {
        package: "controlled fixture 0.1.0".into(),
        target_name: "build-script-build".into(),
        target_kind: TargetKind::BuildScript,
        role: UnitRole::BuildHost,
        compile_target: "aarch64-apple-darwin".into(),
        test_intent: false,
        edition: 2024,
        features: BTreeSet::new(),
        profile_fingerprint: Digest([7; 32]),
        cfg_fingerprint: Digest([8; 32]),
        dependency_edges: Digest([9; 32]),
        rustc_fingerprint: Digest([10; 32]),
    };
    let tree = GeneratedRoot::open(&unit, &out).unwrap().capture().unwrap();
    std::os::unix::fs::symlink(out.join("generated.rs"), path.join("output-alias.rs")).unwrap();
    std::os::unix::fs::symlink(out.join("new.rs"), path.join("missing-alias.rs")).unwrap();
    let out_id = Digest([11; 32]);
    let authorities = IncludeAuthorities::new(
        caller().unit,
        Digest([4; 32]),
        [
            IncludeRoot {
                identity: caller().root,
                path: path.clone(),
                kind: IncludeRootKind::Source,
            },
            IncludeRoot {
                identity: out_id,
                path: out.clone(),
                kind: IncludeRootKind::BuildOutput {
                    producer: unit.fingerprint(),
                },
            },
        ],
    )
    .unwrap();
    let request = authorities
        .request(
            &descriptor("include").unwrap().unwrap(),
            &caller(),
            out.join("generated.rs").to_str().unwrap(),
            Key([5; 32]),
        )
        .unwrap();
    let missing = authorities
        .request(
            &descriptor("include").unwrap().unwrap(),
            &caller(),
            out.join("new.rs").to_str().unwrap(),
            Key([5; 32]),
        )
        .unwrap();
    let alias = authorities
        .request(
            &descriptor("include").unwrap().unwrap(),
            &caller(),
            "output-alias.rs",
            Key([5; 32]),
        )
        .unwrap();
    let missing_alias = authorities
        .request(
            &descriptor("include").unwrap().unwrap(),
            &caller(),
            "missing-alias.rs",
            Key([5; 32]),
        )
        .unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::from([(out_id, tree)])).unwrap();
    fs::remove_dir_all(&out).unwrap();
    let record = capture.capture(&request).unwrap();
    assert_eq!(record.blob().bytes(), b"const GENERATED:i32=31;");
    let alias_record = capture.capture(&alias).unwrap();
    assert_eq!(alias_record.blob().bytes(), record.blob().bytes());
    assert_eq!(alias_record.location().root, out_id);
    assert_eq!(alias_record.virtual_location().root, caller().root);
    assert_eq!(
        alias_record.virtual_location().path.as_str(),
        "output-alias.rs"
    );
    // Live changes beneath OUT_DIR cannot alter this prepared generation.
    fs::create_dir(&out).unwrap();
    fs::write(out.join("generated.rs"), "const GENERATED:i32=999;").unwrap();
    fs::write(out.join("new.rs"), "const NEW:i32=999;").unwrap();
    assert_eq!(
        capture.capture(&alias).unwrap().blob().bytes(),
        record.blob().bytes()
    );
    assert_eq!(
        capture.capture(&missing_alias).err().unwrap().code,
        "LRR-E-GENERATED-SOURCE"
    );
    fs::remove_dir_all(&out).unwrap();
    assert_eq!(
        capture.capture(&missing).err().unwrap().code,
        "LRR-E-GENERATED-SOURCE"
    );
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","scope":"immutable previously captured build output; no live OUT_DIR read","capture":record.identity(),"producer":unit.fingerprint(),"original_directory_exists":out.exists()})).unwrap()).unwrap();
}

#[test]
fn actual_include_source_sets_revalidate_changes_removals_order_and_new_generations() {
    let path = root("revalidate");
    fs::write(path.join("one.rs"), b"31").unwrap();
    fs::write(path.join("two.rs"), b"73").unwrap();
    let authorities = source_authority(&path, Digest([71; 32]));
    let definition = descriptor("include").unwrap().unwrap();
    let one = authorities
        .request(&definition, &caller(), "one.rs", Key([72; 32]))
        .unwrap();
    let two = authorities
        .request(&definition, &caller(), "two.rs", Key([73; 32]))
        .unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let before = capture.capture_set(&[one.clone(), two.clone()]).unwrap();
    assert_eq!(
        before.identity(),
        capture
            .capture_set(&[two.clone(), one.clone()])
            .unwrap()
            .identity()
    );
    assert!(capture.capture_set(&[one.clone(), one.clone()]).is_err());
    fs::write(path.join("one.rs"), b"999").unwrap();
    let error = capture
        .revalidate(&one, &before.records()[&one.identity()])
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-SNAPSHOT-RACE");
    assert!(error.message.contains(&Digest(caller().origin.0).hex()));
    assert!(error.message.contains(&Digest([72; 32]).hex()));
    assert_eq!(before.records()[&one.identity()].blob().bytes(), b"31");
    let changed = capture.capture_set(&[one.clone(), two.clone()]).unwrap();
    assert_ne!(changed.identity(), before.identity());
    assert_eq!(changed.records()[&one.identity()].blob().bytes(), b"999");
    fs::remove_file(path.join("two.rs")).unwrap();
    assert!(
        capture
            .revalidate(&two, &before.records()[&two.identity()])
            .is_err()
    );
    assert!(capture.capture_set(&[one.clone(), two.clone()]).is_err());
    let next_authorities = source_authority(&path, Digest([74; 32]));
    let next_one = next_authorities
        .request(&definition, &caller(), "one.rs", Key([72; 32]))
        .unwrap();
    let next = SourceCapture::open(next_authorities, BTreeMap::new()).unwrap();
    assert!(next.capture(&one).is_err());
    let new_snapshot = next.capture_set(&[next_one]).unwrap();
    assert_ne!(new_snapshot.identity(), changed.identity());
    let old_path = path.with_extension("old");
    fs::rename(&path, &old_path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("one.rs"), b"999").unwrap();
    assert_eq!(
        capture.capture(&one).unwrap_err().code,
        "LRR-E-SNAPSHOT-RACE"
    );
    fs::remove_dir_all(old_path).unwrap();
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","before":before.identity().hex(),"changed":changed.identity().hex(),"next":new_snapshot.identity().hex(),"scope":"explicit coordinator source-set revalidation; no automatic prepare/guest acceptance"})).unwrap()).unwrap();
}

#[test]
fn actual_include_str_emission_requires_adoption_preserves_snapshot_and_matches_native() {
    include_data_emission_fixture(None, false);
}
#[test]
fn actual_include_bytes_emission_preserves_all_bytes_empty_output_and_snapshot_adoption() {
    let bytes = (0..=255).collect::<Vec<u8>>();
    include_data_emission_fixture(Some(&bytes), false);
    include_data_emission_fixture(Some(&[]), false);
}
#[test]
fn actual_nested_concat_include_output_revalidates_adopted_producers_before_capture() {
    include_data_emission_fixture(None, true);
}
fn include_data_emission_fixture(binary: Option<&[u8]>, nested_path: bool) {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::InputRequest,
        },
        expand::{
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            include_string::{IncludeStringExpansion, IncludeStringPoll},
        },
        frontend::compile_input_catalog::CompileInputCatalog,
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source::manifest::BodyKey,
    };
    let path = root(if binary.is_some() {
        "bytes-output"
    } else {
        "string-output"
    });
    let text = "quote\" 🙂\n";
    let name = if binary.is_some() {
        "include_bytes"
    } else {
        "include_str"
    };
    let filename = if binary.is_some() {
        "data.bin"
    } else {
        "text.txt"
    };
    fs::write(path.join(filename), binary.unwrap_or(text.as_bytes())).unwrap();
    let source = if nested_path {
        format!("(core::concat!({filename:?}, \"\"),)")
    } else {
        format!("({filename:?})")
    };
    let parsed = lrr_expand_adapter::token_wire::parse_source(
        &source,
        lrr_expand_adapter::token_wire::SourceIdentity {
            unit: lrr_expand_adapter::token_wire::Key(caller().unit.0),
            content: lrr_expand_adapter::token_wire::Key(
                *blake3::hash(source.as_bytes()).as_bytes(),
            ),
            path: "root.rs".into(),
        },
        lrr_expand_adapter::token_wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
            defining_unit: lrr_expand_adapter::token_wire::Key(caller().unit.0),
            edition: lrr_expand_adapter::token_wire::Edition::E2024,
        },
        lrr_expand_adapter::token_wire::Key([151; 32]),
    )
    .unwrap();
    let tokens = lrr::syntax::token_wire::TokenForest::decode(&parsed.encode().unwrap()).unwrap();
    let site = tokens.spans()[0].key();
    let catalog = CompileInputCatalog::new(
        Digest([152; 32]),
        2024,
        [
            descriptor(name).unwrap().unwrap(),
            descriptor("concat").unwrap().unwrap(),
        ],
    )
    .unwrap();
    let selected = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap().unwrap().name() == name)
        .unwrap();
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Builtin {
            definition: selected.identity,
        },
        invocation: Digest([153; 32]),
        input_tokens: Digest::of_bytes(&tokens.encode().unwrap()),
        consumer_compile_env: Digest([154; 32]),
        parent_context: Digest([155; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([156; 32]),
        procedural_epoch: None,
    };
    let generation = Digest([157; 32]);
    let authorities = source_authority(&path, generation);
    let from = caller();
    let emitter = IncludeStringExpansion {
        catalog: &catalog,
        selected,
        key: &key,
        original: &tokens,
        site,
        authorities: &authorities,
        from: &from,
        max_wire_bytes: 256 * 1024 * 1024,
    };
    let mut db = Database::default();
    let context = DemandContext {
        request: Digest([158; 32]),
        session: Digest([159; 32]),
        unit: caller().unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: generation,
        native_generation: None,
        compile_environment: key.consumer_compile_env,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([160; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let phase = PhaseIdentity {
        frame: Digest([161; 32]),
        body: BodyKey {
            definition: Digest([162; 32]),
            exact_body_bytes: Digest([163; 32]),
        },
        index: 0,
    };
    let resolved = if nested_path {
        use lrr::{
            db::poll::{ExpansionRequest, QueryPoll},
            expand::include_arguments::IncludeArguments,
            service::expansion::Reply,
        };
        assert!(matches!(
            emitter.poll(&mut db),
            IncludeStringPoll::NeedExpansion { .. }
        ));
        let args =
            IncludeArguments::parse(catalog.select(selected).unwrap().unwrap(), &tokens).unwrap();
        let nested = args.nested_invocation().unwrap();
        let concat = catalog
            .definitions()
            .find(|d| catalog.select(d).unwrap().unwrap().name() == "concat")
            .unwrap();
        let mut producer = key.clone();
        producer.macro_identity = MacroIdentity::Builtin {
            definition: concat.identity,
        };
        producer.invocation = nested.identity();
        producer.input_tokens = nested.argument_identity();
        assert!(matches!(
            args.resume_adopted(
                &mut db,
                catalog.select(selected).unwrap().unwrap(),
                &producer
            ),
            QueryPoll::NeedInput(_)
        ));
        let value = lrr::expand::concat::literal_value(
            catalog.select(concat).unwrap().unwrap(),
            nested.arguments(),
            site,
            64 * 1024 * 1024,
            32 * 1024 * 1024,
        )
        .unwrap();
        let record = value
            .expansion_record(
                &catalog,
                concat,
                &producer,
                nested.arguments(),
                256 * 1024 * 1024,
            )
            .unwrap();
        let unrelated_output = IncludeArguments::parse(
            catalog.select(selected).unwrap().unwrap(),
            &record.tokens().unwrap(),
        )
        .unwrap();
        assert!(matches!(
            emitter.poll_resolved(&mut db, &unrelated_output),
            IncludeStringPoll::Ready(Err(_))
        ));
        let token = coordinator
            .issue(
                query.clone(),
                phase.clone(),
                InputRequest::Expand(Box::new(ExpansionRequest {
                    key: producer.clone(),
                })),
            )
            .unwrap();
        coordinator.start(&context, &token).unwrap();
        coordinator
            .accept(&context, &token, Reply::success(&token, record.output()))
            .unwrap();
        coordinator
            .adopt(&context, &token, &mut db, record)
            .unwrap();
        let resolved = args
            .resume_adopted(
                &mut db,
                catalog.select(selected).unwrap().unwrap(),
                &producer,
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let unrelated =
            IncludeArguments::parse(catalog.select(selected).unwrap().unwrap(), &tokens).unwrap();
        assert!(matches!(
            emitter.poll_resolved(&mut db, &unrelated),
            IncludeStringPoll::NeedExpansion { .. }
        ));
        let mut foreign = key.clone();
        foreign.consumer_compile_env = Digest([164; 32]);
        let foreign_emitter = IncludeStringExpansion {
            key: &foreign,
            ..emitter
        };
        assert!(matches!(
            foreign_emitter.poll_resolved(&mut db, &resolved),
            IncludeStringPoll::Ready(Err(_))
        ));
        Some(resolved)
    } else {
        None
    };
    let poll = |emitter: &IncludeStringExpansion<'_>, db: &mut Database| match &resolved {
        Some(args) => emitter.poll_resolved(db, args),
        None => emitter.poll(db),
    };
    let IncludeStringPoll::NeedInput(input) = poll(&emitter, &mut db) else {
        panic!("missing capture did not suspend")
    };
    let token = coordinator.issue(query, phase, input.clone()).unwrap();
    coordinator.start(&context, &token).unwrap();
    let capture =
        SourceCapture::open(source_authority(&path, generation), BTreeMap::new()).unwrap();
    let InputRequest::CaptureInclude { request, .. } = &input else {
        panic!("wrong capture demand")
    };
    let captured = capture.capture(request).unwrap();
    coordinator
        .accept_include(&context, &token, &captured)
        .unwrap();
    assert!(matches!(
        poll(&emitter, &mut db),
        IncludeStringPoll::NeedInput(_)
    ));
    coordinator
        .adopt_include(&context, &token, &mut db, captured)
        .unwrap();
    let IncludeStringPoll::Ready(Ok(record)) = poll(&emitter, &mut db) else {
        panic!("adopted capture did not emit")
    };
    let output = record.tokens().unwrap();
    let [
        lrr::syntax::token_wire::TokenTree::Group {
            open,
            close,
            children,
            ..
        },
    ] = output.roots()
    else {
        panic!("output group absent")
    };
    assert_eq!(*open, site);
    assert_eq!(*close, site);
    let [lrr::syntax::token_wire::TokenTree::Literal { token_text, span }] = children.as_slice()
    else {
        panic!("output literal absent")
    };
    assert_eq!(*span, site);
    let expected = match binary {
        Some(bytes) => format!("{bytes:?}\n"),
        None => format!("{text:?}\n"),
    };
    fs::write(path.join("oracle.rs"),format!("fn main(){{let original={name}!{source};let emitted={token_text};assert_eq!(original,emitted);println!(\"{{:?}}\",original);}}")).unwrap();
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(path.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout, expected);
    fs::remove_file(path.join(filename)).unwrap();
    assert!(
        matches!(poll(&emitter,&mut db),IncludeStringPoll::Ready(Ok(current)) if current.output()==record.output())
    );
    let bytes = output.encode().unwrap().len();
    let reduced = IncludeStringExpansion {
        max_wire_bytes: bytes - 1,
        ..emitter
    };
    assert!(matches!(
        poll(&reduced, &mut db),
        IncludeStringPoll::Ready(Err(_))
    ));
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(matches!(
        poll(&reduced, &mut db),
        IncludeStringPoll::NeedInput(_)
    ));
    if nested_path {
        assert!(matches!(
            poll(&reduced, &mut db),
            IncludeStringPoll::NeedInput(InputRequest::Expand(_))
        ));
    }
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"output":record.output().hex(),"scope":"pure adopted include data output; no recursive eager/guest claim", "kind":name, "binary_bytes":binary.map(|b| b.len()), "nested_path":nested_path})).unwrap()).unwrap();
}

#[test]
fn actual_eager_include_data_driver_captures_only_after_path_adoption_and_matches_native() {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        expand::{
            builtins::FrozenCompileEnvironment,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            scopes::{MacroName, ScopeKey as TextualScope, TextualScopes},
        },
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            hygiene::HygieneFacts,
            macro_names::{MacroNamespaces, MacroPath},
            macro_origins::ExpansionAncestry,
            macro_scopes::MacroDeclarations,
            nested_compile_inputs::{NestedCompileInputResolver, NestedLexicalContext},
        },
        project::tool_manifest::CompileEnv,
        service::{
            eager_compile_inputs::{
                EagerCompileInputPoll, EagerCompilerRequest, EagerLimits, EagerSourceContext,
                poll_eager_compile_input_with_sources,
            },
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        },
        source::{manifest::BodyKey, snapshot::CfgSet},
        syntax::{outline::ScopeKey, parse::ParseKey},
    };
    let all_bytes = (0..=255).map(|n| n as u8).collect::<Vec<_>>();
    let mut observations = vec![];
    for (name, data, root_name) in [
        ("include_str", "quote\" 🙂\n".as_bytes(), "concat"),
        ("include_bytes", all_bytes.as_slice(), "include_bytes"),
    ] {
        let path = root(&format!("eager-{name}"));
        let filename = if name == "include_str" {
            "text.txt"
        } else {
            "data.bin"
        };
        fs::write(path.join(filename), data).unwrap();
        let inner =
            format!("core::{name}!(core::concat!(core::env!(\"LRR_DIRECTORY\"), \"/{filename}\"))");
        let source = if name == "include_str" {
            format!("({inner}, \"!\")")
        } else {
            format!("(core::concat!(core::env!(\"LRR_DIRECTORY\"), \"/{filename}\"))")
        };
        let from = caller();
        let unit = from.unit;
        let generation = Digest([251; 32]);
        let authorities = source_authority(&path, generation);
        let prepared = CompileEnv {
            unit,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: CfgSet::default(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::new(),
            declared: BTreeMap::from([(
                "LRR_DIRECTORY".into(),
                Some(path.to_str().unwrap().into()),
            )]),
            out_dir: None,
        };
        let environment = FrozenCompileEnvironment::new(unit, &prepared).unwrap();
        let parsed = lrr_expand_adapter::token_wire::parse_source(
            &source,
            lrr_expand_adapter::token_wire::SourceIdentity {
                unit: lrr_expand_adapter::token_wire::Key(unit.0),
                content: lrr_expand_adapter::token_wire::Key(
                    *blake3::hash(source.as_bytes()).as_bytes(),
                ),
                path: "root.rs".into(),
            },
            lrr_expand_adapter::token_wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
                defining_unit: lrr_expand_adapter::token_wire::Key(unit.0),
                edition: lrr_expand_adapter::token_wire::Edition::E2024,
            },
            lrr_expand_adapter::token_wire::Key([252; 32]),
        )
        .unwrap();
        let original =
            lrr::syntax::token_wire::TokenForest::decode(&parsed.encode().unwrap()).unwrap();
        let site = original.spans()[0].key();
        let span = &original.spans()[0];
        let core = Digest([253; 32]);
        let catalog = CompileInputCatalog::new(
            core,
            2024,
            [
                descriptor("concat").unwrap().unwrap(),
                descriptor("env").unwrap().unwrap(),
                descriptor(name).unwrap().unwrap(),
            ],
        )
        .unwrap();
        let selected = catalog
            .definitions()
            .find(|d| catalog.select(d).unwrap().unwrap().name() == root_name)
            .unwrap();
        let mut names = MacroNamespaces::default();
        for definition in catalog.definitions() {
            names
                .define(
                    MacroPath::new(
                        core,
                        vec![
                            MacroName::parse(
                                catalog.select(definition).unwrap().unwrap().name(),
                                2024,
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap(),
                    definition.clone(),
                )
                .unwrap();
        }
        names
            .dependency(unit, MacroName::parse("core", 2024).unwrap(), core)
            .unwrap();
        let scope = TextualScope::root(unit);
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope).unwrap();
        let declarations = MacroDeclarations::default();
        let ancestry = ExpansionAncestry::default();
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        hygiene
            .import(
                &original
                    .contexts()
                    .iter()
                    .map(|c| serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let context_keys = original
            .contexts()
            .iter()
            .map(|c| lrr_expand_adapter::token_wire::Key(c.key().0))
            .collect::<Vec<_>>();
        let facts = HygieneFacts::new(
            original.contexts().to_vec(),
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.resolution_contexts(&context_keys).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_name_contexts(
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.name_contexts(&context_keys).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let caller = NestedLexicalContext {
            source: ParseKey {
                content: Digest(span.source.content.0),
                profile: ProfileIdentity::current(),
            },
            scope: ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            textual_scope: scope,
            origin: None,
        };
        let resolver = NestedCompileInputResolver {
            namespaces: &names,
            textual: &scopes,
            declarations: &declarations,
            hygiene: &facts,
            ancestry: &ancestry,
            compiler: &catalog,
        };
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: selected.identity,
            },
            invocation: Digest([254; 32]),
            input_tokens: Digest::of_bytes(&original.encode().unwrap()),
            consumer_compile_env: environment.fingerprint(),
            parent_context: Digest(span.context.0),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([255; 32]),
            procedural_epoch: None,
        };
        let mut db = Database::default();
        let revision = db.source_revision();
        let poll = |db: &mut Database, from: &IncludeLocation| {
            poll_eager_compile_input_with_sources(
                db,
                EagerCompilerRequest {
                    selected,
                    key: &key,
                    original: &original,
                    site,
                    source_revision: revision,
                    limits: EagerLimits::default(),
                },
                &resolver,
                &caller,
                &environment,
                EagerSourceContext {
                    authorities: &authorities,
                    from,
                    included: None,
                },
            )
        };
        let mut wrong = from.clone();
        wrong.path = SourcePath::new("wrong.rs").unwrap();
        assert!(poll(&mut db, &wrong).is_err());
        let context = DemandContext {
            request: Digest([40; 32]),
            session: Digest([41; 32]),
            unit,
            revision,
            epoch: 1,
            source_generation: generation,
            native_generation: None,
            compile_environment: environment.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let capture =
            SourceCapture::open(source_authority(&path, generation), BTreeMap::new()).unwrap();
        let query = QueryKey::Constant(ConstantKey {
            definition: Digest([42; 32]),
            arguments: vec![],
            cfg: Digest::default(),
            profile: ProfileIdentity::current(),
        });
        let phase = |index| PhaseIdentity {
            frame: Digest([43; 32]),
            body: BodyKey {
                definition: Digest([44; 32]),
                exact_body_bytes: Digest([45; 32]),
            },
            index,
        };
        let mut outputs = BTreeSet::new();
        let mut captures = 0;
        let mut final_record = None;
        for index in 0..12 {
            match poll(&mut db, &from).unwrap() {
                EagerCompileInputPoll::Produced(record) => {
                    assert!(outputs.insert(record.key().fingerprint()));
                    let token = coordinator
                        .issue(
                            query.clone(),
                            phase(index),
                            InputRequest::Expand(Box::new(ExpansionRequest {
                                key: record.key().clone(),
                            })),
                        )
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    let EagerCompileInputPoll::Produced(pending) = poll(&mut db, &from).unwrap()
                    else {
                        panic!("accepted output advanced without adoption")
                    };
                    assert_eq!(pending.key(), record.key());
                    coordinator
                        .adopt(&context, &token, &mut db, record)
                        .unwrap();
                }
                EagerCompileInputPoll::NeedInput(input @ InputRequest::CaptureInclude { .. }) => {
                    captures += 1;
                    assert_eq!(captures, 1);
                    assert_eq!(
                        outputs.len(),
                        2,
                        "capture started before env and concat adoption"
                    );
                    let InputRequest::CaptureInclude {
                        request,
                        compile_environment,
                    } = &input
                    else {
                        unreachable!()
                    };
                    assert_eq!(*compile_environment, environment.fingerprint());
                    assert_eq!(request.path().as_str(), filename);
                    let token = coordinator
                        .issue(query.clone(), phase(index), input.clone())
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    let captured = capture.capture(request).unwrap();
                    coordinator
                        .accept_include(&context, &token, &captured)
                        .unwrap();
                    let EagerCompileInputPoll::NeedInput(pending) = poll(&mut db, &from).unwrap()
                    else {
                        panic!("accepted source advanced without adoption")
                    };
                    assert_eq!(pending, input);
                    coordinator
                        .adopt_include(&context, &token, &mut db, captured)
                        .unwrap();
                    fs::remove_file(path.join(filename)).unwrap();
                }
                EagerCompileInputPoll::Complete(record) => {
                    assert_eq!(record.key(), &key);
                    final_record = Some(record);
                    break;
                }
                _ => panic!("unexpected include driver outcome"),
            }
            assert!(index < 11, "include driver did not finish");
        }
        assert_eq!(captures, 1);
        assert!(!path.join(filename).exists());
        let record = final_record.unwrap();
        let output = record.tokens().unwrap();
        let [lrr::syntax::token_wire::TokenTree::Group { children, .. }] = output.roots() else {
            panic!("output root absent")
        };
        let [lrr::syntax::token_wire::TokenTree::Literal { token_text, span }] =
            children.as_slice()
        else {
            panic!("output literal absent")
        };
        assert_eq!(*span, site);
        fs::write(path.join(filename), data).unwrap();
        let native = source.replace("core::", "std::");
        fs::write(path.join("oracle.rs"),format!("fn main(){{let native={root_name}!{native};let emitted={token_text};assert_eq!(native,emitted);println!(\"{{:?}}\",native);}}" )).unwrap();
        let build = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024"])
            .env("LRR_DIRECTORY", path.to_str().unwrap())
            .arg(path.join("oracle.rs"))
            .arg("-o")
            .arg(path.join("oracle"))
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(path.join("oracle")).output().unwrap();
        assert!(native.status.success());
        fs::remove_file(path.join(filename)).unwrap();
        assert!(matches!(
            poll(&mut db, &from).unwrap(),
            EagerCompileInputPoll::Complete(_)
        ));
        assert!(poll(&mut db, &wrong).is_err());
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert!(poll(&mut db, &from).is_err());
        let observed = serde_json::json!({"kind":name,"outputs":outputs.len(),"captures":captures,"native_stdout":String::from_utf8(native.stdout).unwrap(),"root_output":record.output().hex(),"bytes":data.len()});
        observations.push(observed.clone());
        fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observation":observed,"scope":"iterative path construction/source capture/adopted include data output; no syntax include or full guest acceptance"})).unwrap()).unwrap();
    }
    assert_eq!(observations.len(), 2);
}

#[test]
fn actual_syntax_include_driver_preserves_source_context_grammar_and_native_lexical_scope() {
    syntax_include_driver_fixture(false, false, false);
}
#[test]
fn actual_nested_syntax_include_source_view_preserves_base_adoption_and_native_scope() {
    syntax_include_driver_fixture(true, false, false);
}
#[test]
fn actual_symlink_file_include_view_uses_virtual_base_and_matches_native() {
    syntax_include_driver_fixture(true, true, false);
}
#[test]
fn actual_directory_symlink_dot_parent_view_retains_raw_base_and_matches_native() {
    syntax_include_driver_fixture(true, false, true);
}
fn syntax_include_driver_fixture(nested: bool, symlink: bool, dot_parent: bool) {
    syntax_include_driver_edition_fixture(nested, symlink, dot_parent, 2024);
}
#[test]
fn rust2015_captured_include_driver_preserves_edition_scope_and_nested_adoption() {
    syntax_include_driver_edition_fixture(false, false, false, 2015);
    syntax_include_driver_edition_fixture(true, false, false, 2015);
}
#[test]
fn rust2018_captured_include_driver_preserves_edition_scope_and_nested_adoption() {
    syntax_include_driver_edition_fixture(false, false, false, 2018);
    syntax_include_driver_edition_fixture(true, false, false, 2018);
}
#[test]
fn rust2021_captured_include_driver_preserves_edition_scope_and_nested_adoption() {
    syntax_include_driver_edition_fixture(false, false, false, 2021);
    syntax_include_driver_edition_fixture(true, false, false, 2021);
}
fn syntax_include_driver_edition_fixture(
    nested: bool,
    symlink: bool,
    dot_parent: bool,
    edition: u16,
) {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{ExpansionRequest, InputRequest},
        },
        expand::{
            builtins::FrozenCompileEnvironment,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            scopes::{MacroName, ScopeKey as TextualScope, TextualScopes},
        },
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            hygiene::HygieneFacts,
            macro_names::{MacroNamespaces, MacroPath},
            macro_origins::ExpansionAncestry,
            macro_scopes::MacroDeclarations,
            nested_compile_inputs::{NestedCompileInputResolver, NestedLexicalContext},
        },
        project::tool_manifest::CompileEnv,
        service::{
            eager_compile_inputs::{
                EagerCompileInputPoll, EagerCompilerRequest, EagerLimits, EagerSourceContext,
                poll_eager_compile_input_with_sources,
            },
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        },
        source::{manifest::BodyKey, snapshot::CfgSet},
        syntax::{outline::ScopeKey, parse::ParseKey},
    };
    let cases = if nested {
        vec![(
            "nested",
            GrammarSlot::Expression,
            r#"core::include!(core::concat!("deeper/", "two.rs"))"#,
        )]
    } else {
        vec![
            (
                "items",
                GrammarSlot::Items,
                if edition == 2024 {
                    "fn generated() -> Marker { Marker }\nimpl Value for Marker { fn value(&self) -> u32 { 19 } }\n"
                } else {
                    "fn gen() -> Marker { Marker }\nimpl Value for Marker { fn value(&self) -> u32 { 19 } }\n"
                },
            ),
            (
                "expression",
                GrammarSlot::Expression,
                "{let shadow = base + 3; shadow * 2}",
            ),
            ("malformed", GrammarSlot::Items, "fn broken( {"),
        ]
    };
    for (case, grammar, body) in cases {
        let path = root(&format!(
            "syntax-{case}-edition-{edition}-symlink-{symlink}-dot-{dot_parent}"
        ));
        let filename = if dot_parent {
            "one.rs"
        } else if symlink {
            "alias.rs"
        } else if nested {
            "nested/one.rs"
        } else {
            "included.rs"
        };
        if nested {
            fs::create_dir_all(path.join("nested/deeper")).unwrap();
            fs::write(path.join("nested/deeper/two.rs"), "base + 7").unwrap();
        }
        if dot_parent {
            fs::create_dir_all(path.join("nested/deep")).unwrap();
            fs::create_dir_all(path.join("deeper")).unwrap();
            fs::write(path.join("deeper/two.rs"), "base + 29").unwrap();
            fs::write(path.join("nested/one.rs"), body).unwrap();
            std::os::unix::fs::symlink("nested/deep", path.join("link")).unwrap();
        } else if symlink {
            fs::create_dir_all(path.join("deeper")).unwrap();
            fs::write(path.join("deeper/two.rs"), "base + 7").unwrap();
            fs::write(path.join("nested/deeper/two.rs"), "base + 29").unwrap();
            fs::write(path.join("nested/one.rs"), body).unwrap();
            std::os::unix::fs::symlink("nested/one.rs", path.join(filename)).unwrap();
        } else {
            fs::write(path.join(filename), body).unwrap();
        }

        let from = caller();
        let unit = from.unit;
        let generation = Digest([91; 32]);
        let authorities = source_authority(&path, generation);
        let source = if dot_parent {
            r#"("link/../one.rs")"#
        } else if symlink {
            r#"("alias.rs")"#
        } else if nested {
            r#"("nested/one.rs")"#
        } else {
            r#"(core::concat!("included", ".rs"))"#
        };
        let parsed = lrr_expand_adapter::token_wire::parse_source(
            source,
            lrr_expand_adapter::token_wire::SourceIdentity {
                unit: lrr_expand_adapter::token_wire::Key(unit.0),
                content: lrr_expand_adapter::token_wire::Key(
                    *blake3::hash(source.as_bytes()).as_bytes(),
                ),
                path: "root.rs".into(),
            },
            lrr_expand_adapter::token_wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
                defining_unit: lrr_expand_adapter::token_wire::Key(unit.0),
                edition: match edition {
                    2015 => lrr_expand_adapter::token_wire::Edition::E2015,
                    2018 => lrr_expand_adapter::token_wire::Edition::E2018,
                    2021 => lrr_expand_adapter::token_wire::Edition::E2021,
                    2024 => lrr_expand_adapter::token_wire::Edition::E2024,
                    _ => panic!("invalid test edition"),
                },
            },
            lrr_expand_adapter::token_wire::Key([92; 32]),
        )
        .unwrap();
        let original =
            lrr::syntax::token_wire::TokenForest::decode(&parsed.encode().unwrap()).unwrap();
        let site = original.spans()[0].key();
        let context_key = original.spans()[0].context;
        let prepared = CompileEnv {
            unit,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: CfgSet::default(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::new(),
            declared: BTreeMap::new(),
            out_dir: None,
        };
        let environment = FrozenCompileEnvironment::new(unit, &prepared).unwrap();
        let core = Digest([93; 32]);
        let catalog = CompileInputCatalog::new(
            core,
            2024,
            [
                descriptor("include").unwrap().unwrap(),
                descriptor("concat").unwrap().unwrap(),
            ],
        )
        .unwrap();
        let selected = catalog
            .definitions()
            .find(|d| catalog.select(d).unwrap().unwrap().name() == "include")
            .unwrap();
        let mut names = MacroNamespaces::default();
        for d in catalog.definitions() {
            names
                .define(
                    MacroPath::new(
                        core,
                        vec![
                            MacroName::parse(catalog.select(d).unwrap().unwrap().name(), 2024)
                                .unwrap(),
                        ],
                    )
                    .unwrap(),
                    d.clone(),
                )
                .unwrap();
        }
        names
            .dependency(unit, MacroName::parse("core", 2024).unwrap(), core)
            .unwrap();
        let scope = TextualScope::root(unit);
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope).unwrap();
        let declarations = MacroDeclarations::default();
        let ancestry = ExpansionAncestry::default();
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        hygiene.import(parsed.contexts()).unwrap();
        let keys = parsed
            .contexts()
            .iter()
            .map(|c| c.key())
            .collect::<Vec<_>>();
        let facts = HygieneFacts::new(
            original.contexts().to_vec(),
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.resolution_contexts(&keys).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_name_contexts(
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.name_contexts(&keys).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let caller = NestedLexicalContext {
            source: ParseKey {
                content: Digest(original.spans()[0].source.content.0),
                profile: ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            },
            scope: ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            textual_scope: scope,
            origin: None,
        };
        let resolver = NestedCompileInputResolver {
            namespaces: &names,
            textual: &scopes,
            declarations: &declarations,
            hygiene: &facts,
            ancestry: &ancestry,
            compiler: &catalog,
        };
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: selected.identity,
            },
            invocation: Digest([94; 32]),
            input_tokens: Digest::of_bytes(&original.encode().unwrap()),
            consumer_compile_env: environment.fingerprint(),
            parent_context: Digest(context_key.0),
            definition_edition: 2024,
            call_edition: edition,
            grammar,
            engine_version: Digest([95; 32]),
            procedural_epoch: None,
        };
        let mut db = Database::default();
        let revision = db.source_revision();
        let poll = |db: &mut Database, max_wire_bytes| {
            poll_eager_compile_input_with_sources(
                db,
                EagerCompilerRequest {
                    selected,
                    key: &key,
                    original: &original,
                    site,
                    source_revision: revision,
                    limits: EagerLimits {
                        wire_bytes: max_wire_bytes,
                        ..EagerLimits::default()
                    },
                },
                &resolver,
                &caller,
                &environment,
                EagerSourceContext {
                    authorities: &authorities,
                    from: &from,
                    included: None,
                },
            )
        };
        for (wrong_edition, semantics, code) in [
            (edition, "lrr-2024-v1", "LRR-E-SEMANTICS-PROFILE"),
            (
                if edition == 2024 { 2021 } else { 2024 },
                "lrr-2024-v2",
                "LRR-E-COMPILE-INPUT",
            ),
        ] {
            let invalid = NestedLexicalContext {
                source: ParseKey {
                    content: caller.source.content,
                    profile: ProfileIdentity {
                        edition: wrong_edition,
                        semantics: semantics.into(),
                    },
                },
                scope: caller.scope.clone(),
                textual_scope: caller.textual_scope,
                origin: caller.origin,
            };
            let error = match poll_eager_compile_input_with_sources(
                &mut db,
                EagerCompilerRequest {
                    selected,
                    key: &key,
                    original: &original,
                    site,
                    source_revision: revision,
                    limits: EagerLimits::default(),
                },
                &resolver,
                &invalid,
                &environment,
                EagerSourceContext {
                    authorities: &authorities,
                    from: &from,
                    included: None,
                },
            ) {
                Err(error) => error,
                Ok(_) => panic!("incompatible include source context admitted"),
            };
            assert_eq!(error.code, code);
        }
        let context = DemandContext {
            request: Digest([96; 32]),
            session: Digest([97; 32]),
            unit,
            revision,
            epoch: 1,
            source_generation: generation,
            native_generation: None,
            compile_environment: environment.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let capture =
            SourceCapture::open(source_authority(&path, generation), BTreeMap::new()).unwrap();
        let query = QueryKey::Constant(ConstantKey {
            definition: Digest([98; 32]),
            arguments: vec![],
            cfg: Digest::default(),
            profile: ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        });
        let phase = |index| PhaseIdentity {
            frame: Digest([99; 32]),
            body: BodyKey {
                definition: Digest([100; 32]),
                exact_body_bytes: Digest([101; 32]),
            },
            index,
        };
        let mut capture_input = None;
        let mut retained_capture = None;
        let mut captured_identity = None;
        let mut final_record = None;
        let mut malformed_error = None;
        let mut outputs = 0;
        for index in 0..8 {
            match poll(&mut db, 256 * 1024 * 1024) {
                Ok(EagerCompileInputPoll::Produced(record)) => {
                    outputs += 1;
                    let token = coordinator
                        .issue(
                            query.clone(),
                            phase(index),
                            InputRequest::Expand(Box::new(ExpansionRequest {
                                key: record.key().clone(),
                            })),
                        )
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    if nested && record.key() == &key {
                        let emitter = lrr::expand::include_string::IncludeExpansion {
                            catalog: &catalog,
                            selected,
                            key: &key,
                            original: &original,
                            site,
                            authorities: &authorities,
                            from: &from,
                            max_wire_bytes: 256 * 1024 * 1024,
                        };
                        assert!(
                            matches!(emitter.adopted_source_view(&mut db,capture_input.as_ref().unwrap(),None),lrr::db::poll::QueryPoll::NeedInput(InputRequest::Expand(r)) if r.key==key)
                        );
                    }
                    coordinator
                        .adopt(&context, &token, &mut db, record)
                        .unwrap();
                }
                Ok(EagerCompileInputPoll::NeedInput(
                    input @ InputRequest::CaptureInclude { .. },
                )) => {
                    assert_eq!(outputs, if nested { 0 } else { 1 });
                    capture_input = Some(input.clone());
                    let InputRequest::CaptureInclude { request, .. } = &input else {
                        unreachable!()
                    };
                    assert_eq!(
                        request.kind(),
                        lrr::source::include_paths::IncludeKind::Syntax
                    );
                    let captured = capture.capture(request).unwrap();
                    captured_identity = Some(captured.identity());
                    retained_capture = Some(captured.clone());
                    let token = coordinator
                        .issue(query.clone(), phase(index), input.clone())
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept_include(&context, &token, &captured)
                        .unwrap();
                    assert!(matches!(
                        poll(&mut db, 256 * 1024 * 1024),
                        Ok(EagerCompileInputPoll::NeedInput(_))
                    ));
                    coordinator
                        .adopt_include(&context, &token, &mut db, captured)
                        .unwrap();
                    fs::remove_file(path.join(if dot_parent {
                        "nested/one.rs"
                    } else {
                        filename
                    }))
                    .unwrap();
                }
                Ok(EagerCompileInputPoll::Complete(record)) => {
                    assert_ne!(case, "malformed");
                    final_record = Some(record);
                    break;
                }
                Err(error) => {
                    assert_eq!(case, "malformed", "unexpected include failure: {error:?}");
                    assert_eq!(error.code, "LRR-E-INCLUDE-PARSE");
                    assert!(error.message.contains("included.rs"));
                    assert!(error.message.contains("caller origin="));
                    assert!(error.message.contains("include origin="));
                    assert!(error.message.len() < 1024);
                    malformed_error = Some(error);
                    break;
                }
                _ => panic!("unexpected syntax include outcome"),
            }
            assert!(index < 7, "syntax include did not finish");
        }
        assert!(captured_identity.is_some());
        assert!(!path.join(filename).exists());
        if let Some(record) = &final_record {
            assert_eq!(record.key(), &key);
            let tokens = record.tokens().unwrap();
            assert!(tokens.contexts().iter().any(|c| c.key() == context_key));
            for span in tokens.spans() {
                assert_eq!(span.context, context_key);
                assert_eq!(
                    span.edition,
                    match edition {
                        2015 => lrr::syntax::token_wire::Edition::E2015,
                        2018 => lrr::syntax::token_wire::Edition::E2018,
                        2021 => lrr::syntax::token_wire::Edition::E2021,
                        2024 => lrr::syntax::token_wire::Edition::E2024,
                        _ => unreachable!(),
                    }
                );
                assert_eq!(span.source.unit.0, unit.0);
                assert_eq!(span.source.content.0, Digest::of_bytes(body.as_bytes()).0);
                assert_eq!(span.source.path, filename);
                assert_eq!(span.origin.0, captured_identity.unwrap().0);
                assert!(span.end as usize <= body.len());
            }
            assert!(tokens.spans().iter().any(|s| s.start != 0));
        }
        let mut view_evidence = None;
        if nested {
            let emitter = lrr::expand::include_string::IncludeExpansion {
                catalog: &catalog,
                selected,
                key: &key,
                original: &original,
                site,
                authorities: &authorities,
                from: &from,
                max_wire_bytes: 256 * 1024 * 1024,
            };
            let mut forged_db = Database::default();
            let mut forged_coordinator = ExpansionCoordinator::new(context.clone());
            let forged_capture = retained_capture.as_ref().unwrap();
            let input = capture_input.as_ref().unwrap();
            let token = forged_coordinator
                .issue(query.clone(), phase(30), input.clone())
                .unwrap();
            forged_coordinator.start(&context, &token).unwrap();
            forged_coordinator
                .accept_include(&context, &token, forged_capture)
                .unwrap();
            forged_coordinator
                .adopt_include(&context, &token, &mut forged_db, forged_capture.clone())
                .unwrap();
            let forged =
                lrr::expand::record::ExpansionRecord::from_tokens(key.clone(), &original).unwrap();
            let token = forged_coordinator
                .issue(
                    query.clone(),
                    phase(31),
                    InputRequest::Expand(Box::new(ExpansionRequest { key: key.clone() })),
                )
                .unwrap();
            forged_coordinator.start(&context, &token).unwrap();
            forged_coordinator
                .accept(&context, &token, Reply::success(&token, forged.output()))
                .unwrap();
            forged_coordinator
                .adopt(&context, &token, &mut forged_db, forged)
                .unwrap();
            assert!(matches!(
                emitter.adopted_source_view(&mut forged_db, input, None),
                lrr::db::poll::QueryPoll::Ready(Err(_))
            ));
            let view = emitter
                .adopted_source_view(&mut db, capture_input.as_ref().unwrap(), None)
                .into_ready()
                .unwrap()
                .unwrap();
            assert_eq!(view.location().path.as_str(), filename);
            assert!(view.validate(&mut db).into_ready().unwrap().is_ok());
            let args = lrr::expand::include_arguments::IncludeArguments::parse(
                catalog.select(selected).unwrap().unwrap(),
                view.tokens(),
            )
            .unwrap();
            let invocation = args.nested_invocation().unwrap();
            let child_caller = NestedLexicalContext {
                source: ParseKey {
                    content: view.blob().content(),
                    profile: caller.source.profile.clone(),
                },
                scope: caller.scope.clone(),
                textual_scope: caller.textual_scope,
                origin: Some(view.origin()),
            };
            let mut child_ancestry = ExpansionAncestry::default();
            child_ancestry.register(view.origin(), None).unwrap();
            let child_resolver = NestedCompileInputResolver {
                namespaces: &names,
                textual: &scopes,
                declarations: &declarations,
                hygiene: &facts,
                ancestry: &child_ancestry,
                compiler: &catalog,
            };
            let selection = child_resolver
                .resolve_expansion(
                    invocation,
                    &child_caller,
                    &lrr::frontend::nested_compile_inputs::NestedExpansionContext {
                        environment: &environment,
                        engine_version: key.engine_version,
                    },
                )
                .unwrap();
            let (lookup, child_key) = selection.into_parts();
            let lrr::frontend::nested_compile_inputs::NestedMacroLookup::Compiler {
                definition: child_definition,
                ..
            } = lookup
            else {
                panic!("nested include definition missing")
            };
            let child_key = child_key.unwrap();
            let child_poll = |db: &mut Database| {
                poll_eager_compile_input_with_sources(
                    db,
                    EagerCompilerRequest {
                        selected: &child_definition,
                        key: &child_key,
                        original: invocation.arguments(),
                        site: invocation.reference_span(),
                        source_revision: revision,
                        limits: EagerLimits::default(),
                    },
                    &child_resolver,
                    &child_caller,
                    &environment,
                    EagerSourceContext {
                        authorities: &authorities,
                        from: view.location(),
                        included: Some(&view),
                    },
                )
            };
            let mut child_records = Vec::new();
            let mut child_source = None;
            let mut child_capture = 0;
            let mut child_complete = false;
            let mut child_output = None;
            for index in 10..18 {
                match child_poll(&mut db).unwrap() {
                    EagerCompileInputPoll::Produced(record) => {
                        child_records.push(record.clone());
                        let token = coordinator
                            .issue(
                                query.clone(),
                                phase(index),
                                InputRequest::Expand(Box::new(ExpansionRequest {
                                    key: record.key().clone(),
                                })),
                            )
                            .unwrap();
                        coordinator.start(&context, &token).unwrap();
                        coordinator
                            .accept(&context, &token, Reply::success(&token, record.output()))
                            .unwrap();
                        coordinator
                            .adopt(&context, &token, &mut db, record)
                            .unwrap();
                    }
                    EagerCompileInputPoll::NeedInput(
                        input @ InputRequest::CaptureInclude { .. },
                    ) => {
                        child_capture += 1;
                        assert_eq!(child_capture, 1);
                        let InputRequest::CaptureInclude { request, .. } = &input else {
                            unreachable!()
                        };
                        assert_eq!(
                            request.path().as_str(),
                            if symlink || dot_parent {
                                "deeper/two.rs"
                            } else {
                                "nested/deeper/two.rs"
                            }
                        );
                        assert_eq!(request.from().path.as_str(), filename);
                        let captured = capture.capture(request).unwrap();
                        let captured_file = captured.original_path().to_owned();
                        child_source = Some((input.clone(), captured.clone()));
                        if dot_parent {
                            assert_eq!(captured.location().path.as_str(), "nested/deeper/two.rs");
                            assert!(
                                request
                                    .original_target()
                                    .to_str()
                                    .unwrap()
                                    .contains("link/../deeper/two.rs")
                            );
                        }
                        let token = coordinator
                            .issue(query.clone(), phase(index), input)
                            .unwrap();
                        coordinator.start(&context, &token).unwrap();
                        coordinator
                            .accept_include(&context, &token, &captured)
                            .unwrap();
                        coordinator
                            .adopt_include(&context, &token, &mut db, captured)
                            .unwrap();
                        fs::remove_file(captured_file).unwrap();
                    }
                    EagerCompileInputPoll::Complete(record) => {
                        assert_eq!(record.key(), &child_key);
                        assert!(
                            record
                                .tokens()
                                .unwrap()
                                .spans()
                                .iter()
                                .all(|s| s.source.path
                                    == if symlink || dot_parent {
                                        "deeper/two.rs"
                                    } else {
                                        "nested/deeper/two.rs"
                                    })
                        );
                        child_output = Some(record.output().hex());
                        child_complete = true;
                        break;
                    }
                    _ => panic!("unexpected nested include outcome"),
                }
                assert!(index < 17, "nested include did not finish");
            }
            assert!(child_complete);
            assert_eq!(child_capture, 1);
            let child_emitter = lrr::expand::include_string::IncludeExpansion {
                catalog: &catalog,
                selected: &child_definition,
                key: &child_key,
                original: invocation.arguments(),
                site: invocation.reference_span(),
                authorities: &authorities,
                from: view.location(),
                max_wire_bytes: 256 * 1024 * 1024,
            };
            let child_args = lrr::expand::include_arguments::IncludeArguments::parse(
                catalog.select(&child_definition).unwrap().unwrap(),
                invocation.arguments(),
            )
            .unwrap();
            let producer = child_records
                .iter()
                .find(|r| r.key().invocation == child_args.nested_invocation().unwrap().identity())
                .unwrap();
            let resolved = child_args
                .resume_adopted(
                    &mut db,
                    catalog.select(&child_definition).unwrap().unwrap(),
                    producer.key(),
                )
                .into_ready()
                .unwrap()
                .unwrap();
            let (child_capture_input, child_captured) = child_source.as_ref().unwrap();
            let child_view = child_emitter
                .adopted_source_view_included(&mut db, child_capture_input, Some(&resolved), &view)
                .into_ready()
                .unwrap()
                .unwrap();
            assert!(child_view.validate(&mut db).into_ready().unwrap().is_ok());
            let mut isolated = Database::default();
            let mut isolated_coordinator = ExpansionCoordinator::new(context.clone());
            let token = isolated_coordinator
                .issue(query.clone(), phase(40), child_capture_input.clone())
                .unwrap();
            isolated_coordinator.start(&context, &token).unwrap();
            isolated_coordinator
                .accept_include(&context, &token, child_captured)
                .unwrap();
            isolated_coordinator
                .adopt_include(&context, &token, &mut isolated, child_captured.clone())
                .unwrap();
            for (index, record) in child_records.iter().enumerate() {
                let token = isolated_coordinator
                    .issue(
                        query.clone(),
                        phase(41 + u32::try_from(index).unwrap()),
                        InputRequest::Expand(Box::new(ExpansionRequest {
                            key: record.key().clone(),
                        })),
                    )
                    .unwrap();
                isolated_coordinator.start(&context, &token).unwrap();
                isolated_coordinator
                    .accept(&context, &token, Reply::success(&token, record.output()))
                    .unwrap();
                isolated_coordinator
                    .adopt(&context, &token, &mut isolated, record.clone())
                    .unwrap();
            }
            assert!(
                matches!(child_view.validate(&mut isolated),lrr::db::poll::QueryPoll::NeedInput(InputRequest::Expand(r)) if r.key==key)
            );
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(matches!(
                view.validate(&mut db),
                lrr::db::poll::QueryPoll::NeedInput(_)
            ));
            assert!(matches!(
                emitter.adopted_source_view(&mut db, capture_input.as_ref().unwrap(), None),
                lrr::db::poll::QueryPoll::NeedInput(_)
            ));
            assert_eq!(
                view.blob().text().unwrap(),
                body,
                "retained snapshot changed"
            );
            view_evidence = Some(
                serde_json::json!({"view":view.identity().hex(),"virtual_source":view.location().path.as_str(),"nested_capture":if symlink || dot_parent {"deeper/two.rs"} else {"nested/deeper/two.rs"},"nested_output":child_output,"adoption_required":true,"mismatched_output_refused":true,"removed_inputs_suspend":true}),
            );
            fs::write(
                path.join(if symlink {
                    "deeper/two.rs"
                } else {
                    "nested/deeper/two.rs"
                }),
                "base + 7",
            )
            .unwrap();
        }
        if dot_parent {
            fs::write(path.join("nested/one.rs"), body).unwrap();
        } else if symlink {
            std::os::unix::fs::symlink("nested/one.rs", path.join(filename)).unwrap();
        } else {
            fs::write(path.join(filename), body).unwrap();
        }
        let native_source = match grammar {
            GrammarSlot::Items => format!(
                "struct Marker;trait Value{{fn value(&self)->u32;}}include!{source};fn main(){{println!(\"{{}}\",generated().value());}}"
            ),
            _ => format!(
                "fn main(){{let base=5;let value=include!{source};println!(\"{{}}\",value);}}"
            ),
        };
        fs::write(
            path.join("oracle.rs"),
            if edition == 2024 {
                native_source
            } else {
                native_source.replace("generated()", "gen()")
            },
        )
        .unwrap();
        let build = Command::new("rustc")
            .arg("+1.99.0")
            .arg(format!("--edition={edition}"))
            .arg(path.join("oracle.rs"))
            .arg("-o")
            .arg(path.join("oracle"))
            .output()
            .unwrap();
        let observed = if case == "malformed" {
            assert!(!build.status.success());
            assert!(malformed_error.is_some());
            String::from_utf8(build.stderr).unwrap()
        } else {
            assert!(
                build.status.success(),
                "{}",
                String::from_utf8_lossy(&build.stderr)
            );
            let native = Command::new(path.join("oracle")).output().unwrap();
            assert!(native.status.success());
            String::from_utf8(native.stdout).unwrap()
        };
        fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","case":case,"edition":edition,"source_view":view_evidence,"native_success":build.status.success(),"observed":observed,"root_output":final_record.as_ref().map(|r|r.output().hex()),"scope":"captured syntax token grammar/context/source transport and native lexical-scope oracle; no LRR guest execution or generated identity integration"})).unwrap()).unwrap();
    }
}

#[test]
fn syntax_include_lexer_retains_nonroot_caller_context_ancestry_without_a_fresh_mark() {
    use lrr_expand_adapter::token_wire::{
        ContextRecord, Edition, Key as AdapterKey, SourceIdentity, SpanRecord, TokenForest,
        Transparency,
    };
    let root = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: AdapterKey([1; 32]),
        edition: Edition::E2024,
    };
    let child = ContextRecord {
        parent: Some(root.key()),
        expansion: Some(AdapterKey([2; 32])),
        transparency: Transparency::SemiTransparent,
        defining_unit: AdapterKey([1; 32]),
        edition: Edition::E2024,
    };
    let span = SpanRecord {
        source: SourceIdentity {
            unit: AdapterKey([1; 32]),
            content: AdapterKey([3; 32]),
            path: "caller.rs".into(),
        },
        start: 0,
        end: 1,
        context: child.key(),
        edition: Edition::E2024,
        origin: AdapterKey([4; 32]),
    };
    let caller = TokenForest::new(
        vec![],
        vec![root.clone(), child.clone()],
        vec![span.clone()],
    )
    .unwrap();
    let body = "local + 1";
    let output = lrr_expand_adapter::token_wire::parse_source_in_context(
        body,
        SourceIdentity {
            unit: AdapterKey([1; 32]),
            content: AdapterKey(*blake3::hash(body.as_bytes()).as_bytes()),
            path: "included.rs".into(),
        },
        &caller,
        span.key(),
        AdapterKey([5; 32]),
    )
    .unwrap();
    lrr_expand_adapter::grammar::validate(
        &output,
        lrr_expand_adapter::grammar::Grammar::Expression,
    )
    .unwrap();
    assert_eq!(output.contexts().len(), 2);
    assert!(output.contexts().contains(&root));
    assert!(output.contexts().contains(&child));
    assert!(output.spans().iter().all(|s| s.context == child.key()));
    let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
    hygiene.import(output.contexts()).unwrap();
    let facts = hygiene.name_contexts(&[child.key()]).unwrap();
    assert_eq!(facts.len(), 1);
}

#[test]
fn actual_symlink_capture_retains_physical_authority_and_native_virtual_base() {
    let path = root("symlink-addresses");
    fs::create_dir(path.join("real")).unwrap();
    fs::write(path.join("real/one.rs"), "include!(\"inner.rs\")").unwrap();
    fs::write(path.join("inner.rs"), "17").unwrap();
    fs::write(path.join("real/inner.rs"), "29").unwrap();
    std::os::unix::fs::symlink("real/one.rs", path.join("link.rs")).unwrap();
    let generation = Digest([110; 32]);
    let authorities = source_authority(&path, generation);
    let capture =
        SourceCapture::open(source_authority(&path, generation), BTreeMap::new()).unwrap();
    let include = descriptor("include").unwrap().unwrap();
    let outer = authorities
        .request(&include, &caller(), "link.rs", Key([111; 32]))
        .unwrap();
    let captured = capture.capture(&outer).unwrap();
    assert_eq!(captured.location().path.as_str(), "real/one.rs");
    assert_eq!(captured.virtual_location().path.as_str(), "link.rs");
    let physical_request = authorities
        .request(&include, captured.location(), "inner.rs", Key([112; 32]))
        .unwrap();
    let virtual_request = authorities
        .request(
            &include,
            captured.virtual_location(),
            "inner.rs",
            Key([112; 32]),
        )
        .unwrap();
    let physical = capture.capture(&physical_request).unwrap();
    let lexical = capture.capture(&virtual_request).unwrap();
    fs::write(
        path.join("oracle.rs"),
        "fn main(){println!(\"{}\",include!(\"link.rs\"));}",
    )
    .unwrap();
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(path.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_ne!(physical.blob().text().unwrap(), stdout.trim());
    assert_eq!(lexical.blob().text().unwrap(), stdout.trim());
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"physical_lookup":physical.blob().text().unwrap(),"virtual_lookup":lexical.blob().text().unwrap(),"physical_location":captured.location().path.as_str(),"virtual_location":captured.virtual_location().path.as_str(),"physical_lookup_equivalent":false,"virtual_lookup_equivalent":true,"scope":"minimal native file-symlink base witness plus corrected capture routing; physical canonicalization still governs authority"})).unwrap()).unwrap();
}

#[test]
fn actual_cross_root_source_aliases_require_explicit_authority_and_match_native() {
    let path = root("cross-root");
    let a = path.join("a");
    let b = path.join("b");
    let outside = path.join("outside");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(b.join("target.txt"), "approved🙂").unwrap();
    fs::write(outside.join("target.txt"), "not authorized").unwrap();
    std::os::unix::fs::symlink(b.join("target.txt"), a.join("absolute.txt")).unwrap();
    std::os::unix::fs::symlink("../b/target.txt", a.join("relative.txt")).unwrap();
    std::os::unix::fs::symlink(outside.join("target.txt"), a.join("denied.txt")).unwrap();
    std::os::unix::fs::symlink("cycle.txt", a.join("cycle.txt")).unwrap();
    let generation = Digest([121; 32]);
    let make = || {
        IncludeAuthorities::new(
            caller().unit,
            generation,
            [
                IncludeRoot {
                    identity: caller().root,
                    path: a.clone(),
                    kind: IncludeRootKind::Source,
                },
                IncludeRoot {
                    identity: Digest([122; 32]),
                    path: b.clone(),
                    kind: IncludeRootKind::Source,
                },
            ],
        )
        .unwrap()
    };
    let authorities = make();
    let capture = SourceCapture::open(make(), BTreeMap::new()).unwrap();
    let include = descriptor("include_str").unwrap().unwrap();
    let mut observations = vec![];
    for name in ["absolute.txt", "relative.txt"] {
        let request = authorities
            .request(&include, &caller(), name, Key([123; 32]))
            .unwrap();
        let record = capture.capture(&request).unwrap();
        assert_eq!(record.location().root, Digest([122; 32]));
        assert_eq!(record.location().path.as_str(), "target.txt");
        assert_eq!(record.virtual_location().root, caller().root);
        assert_eq!(record.virtual_location().path.as_str(), name);
        let source = a.join(format!("oracle-{name}.rs"));
        let binary = a.join(format!("oracle-{name}"));
        fs::write(
            &source,
            format!("fn main(){{println!(\"{{}}\",include_str!({name:?}));}}"),
        )
        .unwrap();
        let build = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name",
                "cross_root_oracle",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(binary).output().unwrap();
        assert!(native.status.success());
        let stdout = String::from_utf8(native.stdout).unwrap();
        assert_eq!(stdout.trim(), record.blob().text().unwrap());
        observations.push(serde_json::json!({"alias":name,"native_stdout":stdout,"physical_root":record.location().root.hex(),"virtual_path":record.virtual_location().path.as_str()}));
    }
    for name in ["denied.txt", "cycle.txt"] {
        let request = authorities
            .request(&include, &caller(), name, Key([124; 32]))
            .unwrap();
        let error = capture.capture(&request).err().unwrap();
        assert_eq!(error.code, "LRR-E-SOURCE-PATH");
        assert!(error.message.contains(&Digest(caller().origin.0).hex()));
        assert!(error.message.contains(&Digest([124; 32]).hex()));
        observations.push(
            serde_json::json!({"alias":name,"denied_code":error.code,"message":error.message}),
        );
    }
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","observations":observations,"scope":"cap-std multi-root link resolution; approved absolute/relative alias native agreement, unapproved/loop denial; no full guest release claim"})).unwrap()).unwrap();
}

#[test]
fn actual_include_capture_enforces_exact_file_size_boundary_and_regular_file_kind() {
    use lrr::source::snapshot::MAX_SOURCE_FILE;
    let path = root("file-limits");
    let mut bytes = vec![0u8; MAX_SOURCE_FILE];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(i % 256).unwrap();
    }
    fs::write(path.join("boundary.bin"), &bytes).unwrap();
    let oversized = fs::File::create(path.join("oversized.bin")).unwrap();
    oversized
        .set_len(u64::try_from(MAX_SOURCE_FILE).unwrap() + 1)
        .unwrap();
    fs::create_dir(path.join("directory.bin")).unwrap();
    let authorities = source_authority(&path, Digest([125; 32]));
    let definition = descriptor("include_bytes").unwrap().unwrap();
    let make = |name: &str| {
        authorities
            .request(&definition, &caller(), name, Key([126; 32]))
            .unwrap()
    };
    let boundary = make("boundary.bin");
    let oversized = make("oversized.bin");
    let directory = make("directory.bin");
    let capture =
        SourceCapture::open(source_authority(&path, Digest([125; 32])), BTreeMap::new()).unwrap();
    let record = capture.capture(&boundary).unwrap();
    assert_eq!(record.blob().bytes(), bytes);
    for (request, code) in [
        (&oversized, "LRR-E-SOURCE-LIMIT"),
        (&directory, "LRR-E-SOURCE-IO"),
    ] {
        let error = capture.capture(request).err().unwrap();
        assert_eq!(error.code, code);
        assert!(error.message.contains(&Digest(caller().origin.0).hex()));
        assert!(error.message.contains(&Digest([126; 32]).hex()));
    }
    assert!(capture.capture_set(&[boundary.clone(), oversized]).is_err());
    assert_eq!(record.blob().bytes(), bytes);
    fs::write(path.join("evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "platform":"aarch64-apple-darwin", "rust":"1.99.0", "accepted_bytes":record.blob().bytes().len(),
        "content":record.blob().content(), "oversized_bytes":MAX_SOURCE_FILE+1,
        "scope":"actual capture size/regular-file boundary and atomic set failure; no byte-token maximum or guest release claim"
    })).unwrap()).unwrap();
}

#[test]
fn actual_maximum_byte_output_preserves_bytes_and_enforces_text_and_wire_limits() {
    use lrr::source::snapshot::MAX_SOURCE_FILE;
    use lrr_expand_adapter::{
        bytes_output,
        token_wire::{self, ContextRecord, Edition, SourceIdentity, TokenTree, Transparency},
    };
    let path = root("maximum-byte-output");
    let input = token_wire::parse_source(
        "()",
        SourceIdentity {
            unit: token_wire::Key(caller().unit.0),
            content: token_wire::Key([131; 32]),
            path: "root.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: token_wire::Key(caller().unit.0),
            edition: Edition::E2024,
        },
        token_wire::Key([132; 32]),
    )
    .unwrap();
    let site = input.spans()[0].key();
    let bytes = vec![b'a'; MAX_SOURCE_FILE];
    fs::write(path.join("maximum.bin"), &bytes).unwrap();
    let output = bytes_output::emit(&input, site, &bytes, 256 * 1024 * 1024).unwrap();
    let required = output.encoded_len().unwrap();
    assert_eq!(
        bytes_output::emit(&input, site, &bytes, required).unwrap(),
        output
    );
    assert!(bytes_output::emit(&input, site, &bytes, required - 1).is_err());
    let [TokenTree::Group { children, .. }] = output.roots() else {
        panic!("missing byte output group")
    };
    let [
        TokenTree::Literal {
            token_text: text,
            span,
            ..
        },
    ] = children.as_slice()
    else {
        panic!("missing byte literal")
    };
    assert_eq!(*span, site);
    fs::write(path.join("oracle.rs"), format!("fn main(){{let emitted={text};let captured=include_bytes!(\"maximum.bin\");assert_eq!(emitted.as_slice(),captured.as_slice());println!(\"{{}}:{{}}\",emitted.len(),emitted.iter().map(|b|u64::from(*b)).sum::<u64>());}}" )).unwrap();
    let build = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name",
            "maximum_byte_oracle",
        ])
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(path.join("oracle")).output().unwrap();
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!(
            "{}:{}",
            bytes.len(),
            bytes.iter().map(|b| u64::from(*b)).sum::<u64>()
        )
    );
    let source_text: usize = output.spans().iter().map(|s| s.source.path.len()).sum();
    drop(output);
    let skeleton_wire = bytes_output::emit(&input, site, &[], 256 * 1024 * 1024)
        .unwrap()
        .encoded_len()
        .unwrap();
    let escaped_capacity =
        ((32 * 1024 * 1024 - source_text - 3) / 4).min((32 * 1024 * 1024 - skeleton_wire) / 4);
    let escaped_bytes = vec![255u8; escaped_capacity];
    let escaped = bytes_output::emit(&input, site, &escaped_bytes, 256 * 1024 * 1024).unwrap();
    let [TokenTree::Group { children, .. }] = escaped.roots() else {
        panic!("missing escaped output")
    };
    let [
        TokenTree::Literal {
            token_text: text, ..
        },
    ] = children.as_slice()
    else {
        panic!("missing escaped literal")
    };
    assert_eq!(text.len(), escaped_bytes.len() * 4 + 3);
    assert_eq!(&text[..2], "b\"");
    assert_eq!(
        &text[2..text.len() - 1],
        "\\xff".repeat(escaped_bytes.len())
    );
    drop(escaped);
    let aggregate_error = bytes_output::emit(
        &input,
        site,
        &vec![255u8; escaped_capacity + 1],
        256 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert_eq!(aggregate_error.code, "LRR-E-COMPILE-INPUT");
    assert!(aggregate_error.message.contains("wire budget"));
    let span_text_error = bytes_output::emit(
        &input,
        site,
        &vec![255u8; MAX_SOURCE_FILE - 1],
        256 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert!(span_text_error.message.contains("aggregate text limit"));
    let string_text_error = lrr_expand_adapter::string_output::emit(
        &input,
        site,
        &"a".repeat(32 * 1024 * 1024 - 2),
        256 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert!(string_text_error.message.contains("aggregate text limit"));
    let error = bytes_output::emit(
        &input,
        site,
        &vec![255u8; MAX_SOURCE_FILE],
        256 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-COMPILE-INPUT");
    assert!(error.message.contains("escaped text limit"));
    fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "platform":"aarch64-apple-darwin","rust":"1.99.0","accepted_source_bytes":bytes.len(),"wire_bytes":required,
        "native_stdout":stdout,"largest_four_byte_escaped_input":escaped_bytes.len(),"escaped_limit_code":error.code,
        "scope":"actual maximum-size byte emitter/native literal comparison and exact wire/text refusal; no full guest runtime claim"
    })).unwrap()).unwrap();
}

fn module_request(
    authorities: &IncludeAuthorities,
) -> lrr::source::module_capture::ModuleCaptureRequest {
    authorities
        .module_request(
            &caller(),
            std::path::Path::new(""),
            lrr::syntax::modules::ModuleRequest {
                module_path: vec!["child".into()],
                directory: vec![],
                file_relative: false,
                name: "child".into(),
                path_override: None,
            },
            Digest([219; 32]),
            Key([220; 32]),
        )
        .unwrap()
}
#[test]
fn module_capture_observes_both_candidates_and_retains_selected_source_bytes() {
    let path = root("module-candidates");
    fs::write(path.join("child.rs"), "pub fn made()->i64{1597_i64}").unwrap();
    let authorities = source_authority(&path, Digest([221; 32]));
    let request = module_request(&authorities);
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(&request).unwrap();
    assert_eq!(record.observations().len(), 2);
    assert_eq!(
        record.selected().unwrap().blob().text().unwrap(),
        "pub fn made()->i64{1597_i64}"
    );
    assert_eq!(
        record.selected().unwrap().virtual_location().path.as_str(),
        "child.rs"
    );
    assert_eq!(record.directory(), std::path::Path::new("child"));
    capture.revalidate_module(&request, &record).unwrap();
    fs::create_dir(path.join("child")).unwrap();
    fs::write(path.join("child/mod.rs"), "pub fn other(){}").unwrap();
    assert_eq!(
        capture
            .revalidate_module(&request, &record)
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
    let changed = capture.capture_module(&request).unwrap();
    assert_eq!(
        changed.selected().unwrap_err().code,
        "LRR-E-MODULE-AMBIGUOUS"
    );
    assert_ne!(changed.identity(), record.identity());
    assert_eq!(
        record.selected().unwrap().blob().text().unwrap(),
        "pub fn made()->i64{1597_i64}"
    );
}
#[test]
fn module_capture_records_absence_and_rejects_wrong_authority_or_changed_bytes() {
    let path = root("module-absence");
    let authorities = source_authority(&path, Digest([222; 32]));
    let request = module_request(&authorities);
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let absent = capture.capture_module(&request).unwrap();
    assert_eq!(absent.selected().unwrap_err().code, "LRR-E-SOURCE-MISSING");
    capture.revalidate_module(&request, &absent).unwrap();
    fs::write(path.join("child.rs"), "pub fn made()->i64{1601_i64}").unwrap();
    assert_eq!(
        capture
            .revalidate_module(&request, &absent)
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
    let present = capture.capture_module(&request).unwrap();
    fs::write(path.join("child.rs"), "pub fn made()->i64{1607_i64}").unwrap();
    assert_eq!(
        capture
            .revalidate_module(&request, &present)
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
    let foreign =
        SourceCapture::open(source_authority(&path, Digest([223; 32])), BTreeMap::new()).unwrap();
    assert_eq!(
        foreign.capture_module(&request).unwrap_err().code,
        "LRR-E-SOURCE-PATH"
    );
}

#[test]
fn module_capture_preserves_raw_symlink_parent_order_and_rejects_escape() {
    let path = root("module-links");
    fs::create_dir_all(path.join("real/deep")).unwrap();
    std::os::unix::fs::symlink("real/deep", path.join("link")).unwrap();
    fs::write(path.join("chosen.rs"), "wrong").unwrap();
    fs::write(path.join("real/chosen.rs"), "pub fn made()->i64{1619_i64}").unwrap();
    let authorities = source_authority(&path, Digest([224; 32]));
    let make = |value: &str| {
        authorities
            .module_request(
                &caller(),
                std::path::Path::new(""),
                lrr::syntax::modules::ModuleRequest {
                    module_path: vec!["child".into()],
                    directory: vec![],
                    file_relative: false,
                    name: "child".into(),
                    path_override: Some(value.into()),
                },
                Digest([219; 32]),
                Key([220; 32]),
            )
            .unwrap()
    };
    let request = make("link/../chosen.rs");
    let outside = root("module-outside");
    fs::write(outside.join("secret.rs"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.rs"), path.join("escape.rs")).unwrap();
    let escape = make("escape.rs");
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(&request).unwrap();
    assert_eq!(record.observations().len(), 1);
    assert_eq!(
        record.selected().unwrap().blob().text().unwrap(),
        "pub fn made()->i64{1619_i64}"
    );
    assert_eq!(
        record.selected().unwrap().location().path.as_str(),
        "real/chosen.rs"
    );
    assert_eq!(
        capture.capture_module(&escape).unwrap_err().code,
        "LRR-E-SOURCE-PATH"
    );
}
#[test]
fn module_capture_nonfile_presence_cannot_be_reinterpreted_as_absence() {
    let path = root("module-nonfile");
    fs::create_dir(path.join("child.rs")).unwrap();
    fs::create_dir(path.join("child")).unwrap();
    fs::write(path.join("child/mod.rs"), "pub fn made()->i64{1621_i64}").unwrap();
    let authorities = source_authority(&path, Digest([225; 32]));
    let request = module_request(&authorities);
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(&request).unwrap();
    assert!(matches!(
        record.observations()[0],
        lrr::source::module_capture::ModuleObservation::NonFile { .. }
    ));
    assert_eq!(
        record.selected().unwrap_err().code,
        "LRR-E-MODULE-AMBIGUOUS"
    );
    fs::remove_file(path.join("child/mod.rs")).unwrap();
    let nonfile = capture.capture_module(&request).unwrap();
    assert_eq!(nonfile.selected().unwrap_err().code, "LRR-E-SOURCE-IO");
    assert_eq!(
        capture
            .revalidate_module(&request, &record)
            .unwrap_err()
            .code,
        "LRR-E-SNAPSHOT-RACE"
    );
}

#[test]
fn module_capture_candidate_bytes_and_virtual_origins_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    for case in 0..4 {
        let path = root("module-native");
        let child = "pub fn source()->&'static str{include_str!(file!())}pub fn origin()->&'static str{file!()}\n";
        let (target, override_path) = match case {
            0 => ("child.rs", None),
            1 => {
                fs::create_dir(path.join("child")).unwrap();
                ("child/mod.rs", None)
            }
            2 => ("chosen.rs", Some("chosen.rs")),
            _ => {
                fs::create_dir_all(path.join("real/deep")).unwrap();
                std::os::unix::fs::symlink("real/deep", path.join("link")).unwrap();
                ("real/chosen.rs", Some("link/../chosen.rs"))
            }
        };
        fs::write(path.join(target), child).unwrap();
        let authorities = source_authority(&path, Digest([226; 32]));
        let request = authorities
            .module_request(
                &caller(),
                std::path::Path::new(""),
                lrr::syntax::modules::ModuleRequest {
                    module_path: vec!["child".into()],
                    directory: vec![],
                    file_relative: false,
                    name: "child".into(),
                    path_override: override_path.map(str::to_owned),
                },
                Digest([219; 32]),
                Key([220; 32]),
            )
            .unwrap();
        let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
        let record = capture.capture_module(&request).unwrap();
        let selected = record.selected().unwrap();
        let prefix = override_path
            .map(|path| format!("#[path={path:?}]"))
            .unwrap_or_default();
        let oracle = format!(
            "{prefix}mod child;fn main(){{print!(\"{{}}\\n---ORIGIN---\\n{{}}\",child::source(),child::origin());}}"
        );
        fs::write(path.join("oracle.rs"), oracle).unwrap();
        fs::write(path.join("rustc.stdout"), &version.stdout).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=module_native"])
            .arg(path.join("oracle.rs"))
            .arg("-o")
            .arg(path.join("oracle"))
            .output()
            .unwrap();
        fs::write(path.join("compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(path.join("oracle")).output().unwrap();
        fs::write(path.join("native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        let expected = format!(
            "{}\n---ORIGIN---\n{}",
            selected.blob().text().unwrap(),
            selected.original_path().display()
        );
        assert_eq!(String::from_utf8(native.stdout).unwrap(), expected);
        fs::write(path.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","capture":record.identity().hex(),"scope":"explicit module-candidate capture and native source/origin comparison; no DB adoption or LRR child execution claim"})).unwrap()).unwrap();
    }
}

#[test]
fn module_capture_uses_frozen_build_output_bytes_and_empty_directory_membership() {
    use lrr::project::cargo_units::{CargoUnitKey, TargetKind, UnitRole};
    let path = root("module-output");
    let out = path.join("out");
    fs::create_dir(&out).unwrap();
    fs::create_dir(out.join("child.rs")).unwrap();
    fs::create_dir(out.join("child")).unwrap();
    fs::write(out.join("child/mod.rs"), "pub fn made()->i64{1637_i64}").unwrap();
    let unit = CargoUnitKey {
        package: "controlled module fixture 0.1.0".into(),
        target_name: "build-script-build".into(),
        target_kind: TargetKind::BuildScript,
        role: UnitRole::BuildHost,
        compile_target: "aarch64-apple-darwin".into(),
        test_intent: false,
        edition: 2024,
        features: BTreeSet::new(),
        profile_fingerprint: Digest([7; 32]),
        cfg_fingerprint: Digest([8; 32]),
        dependency_edges: Digest([9; 32]),
        rustc_fingerprint: Digest([10; 32]),
    };
    let tree = GeneratedRoot::open(&unit, &out).unwrap().capture().unwrap();
    let out_id = Digest([227; 32]);
    let authorities = IncludeAuthorities::new(
        caller().unit,
        Digest([228; 32]),
        [
            IncludeRoot {
                identity: caller().root,
                path: path.clone(),
                kind: IncludeRootKind::Source,
            },
            IncludeRoot {
                identity: out_id,
                path: out.clone(),
                kind: IncludeRootKind::BuildOutput {
                    producer: unit.fingerprint(),
                },
            },
        ],
    )
    .unwrap();
    let from = IncludeLocation {
        unit: caller().unit,
        root: out_id,
        path: SourcePath::new("generated.rs").unwrap(),
        origin: Key([3; 32]),
    };
    let make = |override_path: Option<&str>| {
        authorities
            .module_request(
                &from,
                std::path::Path::new(""),
                lrr::syntax::modules::ModuleRequest {
                    module_path: vec!["child".into()],
                    directory: vec![],
                    file_relative: false,
                    name: "child".into(),
                    path_override: override_path.map(str::to_owned),
                },
                Digest([219; 32]),
                Key([220; 32]),
            )
            .unwrap()
    };
    let ambiguous = make(None);
    let explicit = make(Some("child/mod.rs"));
    let capture = SourceCapture::open(authorities, BTreeMap::from([(out_id, tree)])).unwrap();
    fs::remove_dir_all(&out).unwrap();
    let record = capture.capture_module(&ambiguous).unwrap();
    assert!(matches!(
        record.observations()[0],
        lrr::source::module_capture::ModuleObservation::NonFile { .. }
    ));
    assert_eq!(
        record.selected().unwrap_err().code,
        "LRR-E-MODULE-AMBIGUOUS"
    );
    let selected = capture.capture_module(&explicit).unwrap();
    assert_eq!(
        selected.selected().unwrap().blob().text().unwrap(),
        "pub fn made()->i64{1637_i64}"
    );
    fs::create_dir_all(out.join("child")).unwrap();
    fs::write(out.join("child/mod.rs"), "changed disk").unwrap();
    capture.revalidate_module(&explicit, &selected).unwrap();
    assert_eq!(
        capture.capture_module(&explicit).unwrap().identity(),
        selected.identity()
    );
}
#[test]
fn module_capture_request_rejects_unbounded_paths_before_source_observation() {
    let path = root("module-limits");
    let authorities = source_authority(&path, Digest([229; 32]));
    let mut module = lrr::syntax::modules::ModuleRequest {
        module_path: vec!["child".into()],
        directory: vec![],
        file_relative: false,
        name: "child".into(),
        path_override: Some("x".repeat(65537)),
    };
    assert_eq!(
        authorities
            .module_request(
                &caller(),
                std::path::Path::new(""),
                module.clone(),
                Digest([219; 32]),
                Key([220; 32])
            )
            .unwrap_err()
            .code,
        "LRR-E-SOURCE-LIMIT"
    );
    module.path_override = None;
    module.module_path = vec!["child".into(); 257];
    assert_eq!(
        authorities
            .module_request(
                &caller(),
                std::path::Path::new(""),
                module,
                Digest([219; 32]),
                Key([220; 32])
            )
            .unwrap_err()
            .code,
        "LRR-E-SOURCE-LIMIT"
    );
}

#[test]
fn module_capture_acceptance_requires_explicit_adoption_before_pure_input_resume() {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{ConstantKey, QueryKey},
            memo::Database,
            poll::{InputRequest, QueryPoll},
            provider::SyntaxProvider,
        },
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source::manifest::BodyKey,
    };
    let path = root("module-adoption");
    fs::write(path.join("child.rs"), "pub fn child(){}").unwrap();
    let generation = Digest([230; 32]);
    let authorities = source_authority(&path, generation);
    let request = module_request(&authorities);
    let input = InputRequest::CaptureModule {
        request: Box::new(request.clone()),
        compile_environment: Digest([231; 32]),
    };
    let mut db = Database::default();
    let context = DemandContext {
        request: Digest([232; 32]),
        session: Digest([233; 32]),
        unit: caller().unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: generation,
        native_generation: None,
        compile_environment: Digest([231; 32]),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([234; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let phase = PhaseIdentity {
        frame: Digest([235; 32]),
        body: BodyKey {
            definition: Digest([236; 32]),
            exact_body_bytes: Digest([237; 32]),
        },
        index: 0,
    };
    let token = coordinator.issue(query, phase, input.clone()).unwrap();
    coordinator.start(&context, &token).unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let record = capture.capture_module(&request).unwrap();
    assert!(matches!(
        SyntaxProvider::module_capture(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    assert!(coordinator.take_ready(&context, &token).unwrap().is_none());
    assert!(matches!(
        SyntaxProvider::module_capture(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    coordinator
        .adopt_module(&context, &token, &mut db, record.clone())
        .unwrap();
    let QueryPoll::Ready(Ok(adopted)) = SyntaxProvider::module_capture(&mut db, &input) else {
        panic!("adopted module did not resume pure input lookup");
    };
    assert_eq!(adopted.record().identity(), record.identity());
    assert_eq!(
        adopted.record().selected().unwrap().blob().text().unwrap(),
        "pub fn child(){}"
    );
    assert_eq!(
        db.cache_usage().unwrap().source_input_bytes,
        record.selected().unwrap().blob().bytes().len()
    );
    assert!(coordinator.take_ready(&context, &token).unwrap().is_some());
    assert!(coordinator.take_ready(&context, &token).is_err());
}
