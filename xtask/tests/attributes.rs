//! M010 pure cfg/attribute inputs, with an actual compiler short-circuit oracle.
use lrr::{
    core::{digest::StableEncode, profile::ProfileIdentity},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, spans::SourceRange},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn facts(
    source: &str,
    cfg: &CfgSet,
) -> Result<lrr::syntax::attributes::ItemAttributeInputs, lrr::core::diagnostic::Diagnostic> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    parsed.attribute_inputs_at(
        SourceRange {
            start: 0,
            end: source.len().try_into().unwrap(),
        },
        cfg,
    )
}
#[test]
fn nested_cfg_attr_preserves_lexical_macro_input_order_and_explicit_cfg_identity() {
    let source = "#[before] #[cfg_attr(on, first, cfg_attr(inner, second, third), fourth)] #[after] fn selected(){missing();}";
    let mut cfg = CfgSet::default();
    cfg.atoms.extend(["on".into(), "inner".into()]);
    let full = facts(source, &cfg).unwrap();
    assert!(full.enabled);
    assert_eq!(full.cfg, cfg.fingerprint());
    assert_eq!(
        full.attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["before", "first", "second", "third", "fourth", "after"]
    );
    for attr in &full.attributes {
        assert_eq!(
            &source[attr.range.start as usize..attr.range.end as usize],
            attr.name
        );
    }
    cfg.atoms.remove("inner");
    let reduced = facts(source, &cfg).unwrap();
    assert_eq!(
        reduced
            .attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["before", "first", "fourth", "after"]
    );
    assert_ne!(full.cfg, reduced.cfg);
}
#[test]
fn inactive_cfg_prevents_later_attribute_demands_and_matches_native_short_circuit() {
    let inactive =
        "#[cfg(any())] #[cfg(not(on, on))] #[deliberately_panics] fn absent(){missing();}";
    let cfg = CfgSet::default();
    let inputs = facts(inactive, &cfg).unwrap();
    assert!(!inputs.enabled);
    assert!(inputs.attributes.is_empty());
    let reversed = "#[cfg(not(on, on))] #[cfg(any())] fn absent(){missing();}";
    assert!(facts(reversed, &cfg).is_err());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m010-cfg-order-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    for (name, source, expected) in [("inactive", inactive, true), ("reversed", reversed, false)] {
        let path = root.join(format!("{name}.rs"));
        fs::write(&path, source).unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "--check-cfg=cfg(on)",
            ])
            .arg(&path)
            .arg("-o")
            .arg(root.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &output.stderr).unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !expected {
            assert!(String::from_utf8_lossy(&output.stderr).contains("not"));
        }
    }
}

#[test]
fn original_attribute_argument_selection_preserves_tokens_spans_and_cfg_attr_groups() {
    use lrr::core::digest::Digest;
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenTree, Transparency,
    };
    let source = "#[cfg_attr(on, derive(First, ::renamed::r#gen, Last), transform('a, r#name += 7_i64))] struct Selected;";
    let mut cfg = CfgSet::default();
    cfg.atoms.insert("on".into());
    let inputs = facts(source, &cfg).unwrap();
    let identity = SourceIdentity {
        unit: Key([31; 32]),
        content: Key(Digest::of_bytes(source.as_bytes()).0),
        path: "original.rs".into(),
    };
    let original = token_wire::parse_source(
        source,
        identity.clone(),
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: identity.unit,
            edition: Edition::E2024,
        },
        Key([32; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let runtime_identity = runtime.spans()[0].source.clone();
    let selected =
        lrr::expand::attributes::argument_forest(&inputs, 0, &runtime_identity, &runtime)
            .unwrap()
            .unwrap();
    let adapter = token_wire::TokenForest::decode(&selected.encode().unwrap()).unwrap();
    let paths = lrr_expand_adapter::derive_paths::parse(&adapter).unwrap();
    assert_eq!(
        paths.iter().map(|p| p.segments.clone()).collect::<Vec<_>>(),
        vec![vec!["First"], vec!["renamed", "r#gen"], vec!["Last"]]
    );
    let selected =
        lrr::expand::attributes::argument_forest(&inputs, 1, &runtime_identity, &runtime)
            .unwrap()
            .unwrap();
    let adapter = token_wire::TokenForest::decode(&selected.encode().unwrap()).unwrap();
    assert!(adapter.spans().len() < original.spans().len());
    for span in adapter.spans() {
        assert!(original.spans().contains(span));
    }
    for context in adapter.contexts() {
        assert!(original.contexts().contains(context));
    }
    let [TokenTree::Group { children, .. }] = adapter.roots() else {
        unreachable!()
    };
    assert!(
        children
            .iter()
            .any(|t| matches!(t,TokenTree::Ident{raw:true,text,..} if text=="name"))
    );
    assert!(
        children
            .iter()
            .any(|t| matches!(t,TokenTree::Literal{token_text,..} if token_text=="7_i64"))
    );
    let mut stale = runtime_identity.clone();
    stale.content = lrr::syntax::token_wire::Key([33; 32]);
    assert!(lrr::expand::attributes::argument_forest(&inputs, 0, &stale, &runtime).is_err());
    assert!(
        lrr::expand::attributes::argument_forest(&inputs, 99, &runtime_identity, &runtime).is_err()
    );
    let mut inactive = inputs.clone();
    inactive.enabled = false;
    assert!(
        lrr::expand::attributes::argument_forest(&inactive, 0, &runtime_identity, &runtime)
            .is_err()
    );
}

#[test]
fn applied_attribute_removal_retains_later_attributes_and_defers_cfg_materialization() {
    use lrr::{
        core::digest::Digest,
        expand::attributes::{AttributeItemInput, item_without_applied_attribute},
    };
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source = "#[first] #[inline] fn selected()->i64{7_i64}";
    let cfg = CfgSet::default();
    let inputs = facts(source, &cfg).unwrap();
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([41; 32]),
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: "item.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([41; 32]),
            edition: Edition::E2024,
        },
        Key([42; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let identity = runtime.spans()[0].source.clone();
    let AttributeItemInput::Ready(output) =
        item_without_applied_attribute(&inputs, 0, &identity, &runtime).unwrap()
    else {
        panic!("ordinary attribute removal must be ready")
    };
    let removed = inputs.attributes[0].original_attribute_range.unwrap();
    for span in output.spans() {
        assert!(runtime.spans().contains(span));
    }
    assert!(
        output
            .spans()
            .iter()
            .all(|s| !(s.start >= removed.start && s.end <= removed.end))
    );
    let names: Vec<_> = output
        .roots()
        .iter()
        .flat_map(|t| match t {
            lrr::syntax::token_wire::TokenTree::Group { children, .. } => {
                children.iter().collect::<Vec<_>>()
            }
            _ => vec![],
        })
        .collect();
    assert!(!names.is_empty());
    let cfg_source = "#[cfg(on)] #[first] fn selected(){}";
    let mut enabled_cfg = CfgSet::default();
    enabled_cfg.atoms.insert("on".into());
    let cfg_inputs = facts(cfg_source, &enabled_cfg).unwrap();
    let mut cfg_identity = identity.clone();
    cfg_identity.content = lrr::syntax::token_wire::Key(cfg_inputs.parse.content.0);
    assert!(matches!(
        item_without_applied_attribute(&cfg_inputs, 1, &cfg_identity, &runtime).unwrap(),
        AttributeItemInput::NeedCfgMaterialization
    ));
    let conditional = "#[cfg_attr(on, first, inline)] fn selected(){}";
    let mut cfg = CfgSet::default();
    cfg.atoms.insert("on".into());
    let conditional = facts(conditional, &cfg).unwrap();
    let mut identity = identity;
    identity.content = lrr::syntax::token_wire::Key(conditional.parse.content.0);
    assert!(matches!(
        item_without_applied_attribute(&conditional, 0, &identity, &runtime).unwrap(),
        AttributeItemInput::NeedCfgMaterialization
    ));
}

#[test]
fn effective_cfg_attributes_materialize_from_original_tokens_in_lexical_order() {
    use lrr::{core::digest::Digest, expand::attribute_materialize};
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source = "#[cfg(on)] #[cfg_attr(off, unavailable)] #[cfg_attr(on, first, cfg_attr(inner, inline, cold), must_use)] fn selected()->i64{7_i64}";
    let mut cfg = CfgSet::default();
    cfg.atoms.extend(["on".into(), "inner".into()]);
    let inputs = facts(source, &cfg).unwrap();
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([51; 32]),
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: "effective.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([51; 32]),
            edition: Edition::E2024,
        },
        Key([52; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let identity = runtime.spans()[0].source.clone();
    let output = attribute_materialize::materialize(&inputs, 1, &identity, &runtime).unwrap();
    use lrr::syntax::token_wire::{Delimiter, TokenTree};
    let [TokenTree::Group { children, .. }] = output.roots() else {
        unreachable!()
    };
    let names: Vec<_> = children
        .iter()
        .filter_map(|t| match t {
            TokenTree::Group {
                delimiter: Delimiter::Bracket,
                children,
                ..
            } => match &children[0] {
                TokenTree::Ident { text, .. } => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(names, ["inline", "cold", "must_use"]);
    for span in output.spans() {
        assert!(runtime.spans().contains(span));
    }
    for context in output.contexts() {
        assert!(runtime.contexts().contains(context));
    }
    assert!(output.spans().len() < runtime.spans().len());
}

#[test]
fn derive_input_removes_all_derive_attributes_preserving_helpers_and_original_records() {
    use lrr::{core::digest::Digest, expand::attribute_materialize};
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source =
        "#[derive(First)] #[cfg_attr(on, derive(Second), repr(C), helper)] struct Selected;";
    let mut cfg = CfgSet::default();
    cfg.atoms.insert("on".into());
    let inputs = facts(source, &cfg).unwrap();
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([171; 32]),
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: "derive.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([171; 32]),
            edition: Edition::E2024,
        },
        Key([172; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let output =
        attribute_materialize::derive_input(&inputs, &runtime.spans()[0].source, &runtime).unwrap();
    use lrr::syntax::token_wire::{Delimiter, TokenTree};
    let [TokenTree::Group { children, .. }] = output.roots() else {
        unreachable!()
    };
    let attrs: Vec<_> = children
        .iter()
        .filter_map(|t| match t {
            TokenTree::Group {
                delimiter: Delimiter::Bracket,
                children,
                ..
            } => match &children[0] {
                TokenTree::Ident { text, .. } => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(attrs, ["repr", "helper"]);
    assert!(
        children
            .iter()
            .any(|t| matches!(t, TokenTree::Ident{text,..} if text == "Selected"))
    );
    assert!(output.spans().iter().all(|s| runtime.spans().contains(s)));
    assert!(
        output
            .contexts()
            .iter()
            .all(|c| runtime.contexts().contains(c))
    );
    assert!(output.spans().len() < runtime.spans().len());
    let mut stale = runtime.spans()[0].source.clone();
    stale.content = lrr::syntax::token_wire::Key([0; 32]);
    assert!(attribute_materialize::derive_input(&inputs, &stale, &runtime).is_err());
    let mut inactive = inputs.clone();
    inactive.enabled = false;
    assert!(
        attribute_materialize::derive_input(&inactive, &runtime.spans()[0].source, &runtime)
            .is_err()
    );
}

#[test]
fn derive_member_facts_suppress_disabled_descendants_and_preserve_native_cfg_attrs() {
    use lrr::{core::digest::Digest, expand::member_cfg};
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source = "#[derive(MemberCfg)] enum Value { #[cfg(any())] DROP(#[cfg(not(a,a))] Missing), Kept { #[cfg(any())] DROP: Missing, #[cfg_attr(all(), member_helper, cfg_attr(all(), another))] kept: i64, #[cfg(all())] last: i64 } }";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let selected = SourceRange {
        start: 0,
        end: source.len().try_into().unwrap(),
    };
    let facts = parsed
        .derive_item_inputs_at(selected, &CfgSet::default())
        .unwrap();
    assert_eq!(facts.members.len(), 5);
    assert_eq!(facts.members.iter().filter(|m| !m.enabled).count(), 2);
    assert!(facts.members.iter().any(|m| {
        m.attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            == ["member_helper", "another"]
    }));
    assert!(
        facts
            .members
            .iter()
            .any(|m| m.attributes.iter().any(|a| a.name == "cfg"))
    );
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([231; 32]),
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: "members.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([231; 32]),
            edition: Edition::E2024,
        },
        Key([232; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let identity = &runtime.spans()[0].source;
    let output = member_cfg::derive_input(&facts, identity, &runtime).unwrap();
    let mut names = Vec::new();
    let mut pending: Vec<_> = output.roots().iter().collect();
    while let Some(token) = pending.pop() {
        match token {
            lrr::syntax::token_wire::TokenTree::Group { children, .. } => pending.extend(children),
            lrr::syntax::token_wire::TokenTree::Ident { text, .. } => names.push(text.as_str()),
            _ => {}
        }
    }
    assert!(
        !names.contains(&"DROP") && !names.contains(&"Missing") && !names.contains(&"cfg_attr")
    );
    assert!(names.contains(&"cfg") && names.contains(&"kept") && names.contains(&"another"));
    assert!(output.spans().iter().all(|s| runtime.spans().contains(s)));
    assert!(
        output
            .contexts()
            .iter()
            .all(|c| runtime.contexts().contains(c))
    );
    let mut duplicate = facts.clone();
    duplicate.members.push(duplicate.members[0].clone());
    assert!(member_cfg::derive_input(&duplicate, identity, &runtime).is_err());
    let mut stale = facts.clone();
    stale.members[0].cfg = Digest([0; 32]);
    assert!(member_cfg::derive_input(&stale, identity, &runtime).is_err());
    let inactive = "#[cfg(any())] #[derive(MemberCfg)] enum Value { #[cfg(not(a,a))] Gone }";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(inactive.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let facts = parsed
        .derive_item_inputs_at(
            SourceRange {
                start: 0,
                end: inactive.len().try_into().unwrap(),
            },
            &CfgSet::default(),
        )
        .unwrap();
    assert!(!facts.item.enabled && facts.members.is_empty());
}

#[test]
fn original_macro_input_preparation_binds_owner_tokens_arguments_and_derive_order() {
    use lrr::{core::digest::Digest, service::item_macro_inputs};
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source = "#[transform(fn generated()->i64{7_i64})] #[derive(One, ::renamed::r#gen, Last)] struct Value { #[cfg(any())] hidden: Missing, kept: i64 }";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let range = SourceRange {
        start: 0,
        end: source.len().try_into().unwrap(),
    };
    let cfg = CfgSet::default();
    let outer = parsed.attribute_inputs_at(range, &cfg).unwrap();
    let derivative = parsed.derive_item_inputs_at(range, &cfg).unwrap();
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([21; 32]),
            content: Key(Digest::of_bytes(source.as_bytes()).0),
            path: "inputs.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([21; 32]),
            edition: Edition::E2024,
        },
        Key([22; 32]),
    )
    .unwrap();
    let runtime =
        lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let identity = &runtime.spans()[0].source;
    let owner = Digest([23; 32]);
    let attr = item_macro_inputs::attribute(owner, &outer, 0, identity, &runtime).unwrap();
    assert!(attr.arguments.is_some());
    assert_eq!(
        attr.identity,
        item_macro_inputs::attribute(owner, &outer, 0, identity, &runtime)
            .unwrap()
            .identity
    );
    assert_ne!(
        attr.identity,
        item_macro_inputs::attribute(Digest([24; 32]), &outer, 0, identity, &runtime)
            .unwrap()
            .identity
    );
    let derived = item_macro_inputs::derive(owner, &derivative, 1, identity, &runtime).unwrap();
    assert_eq!(
        derived
            .selections
            .iter()
            .map(|s| s.segments.iter().map(String::as_str).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec!["One"], vec!["renamed", "r#gen"], vec!["Last"]]
    );
    assert!(derived.selections[1].absolute);
    let repeated = item_macro_inputs::DeriveInvocationInput {
        identity: derived.identity,
        item: derived.item.clone(),
        selections: vec![derived.selections[0].clone(), derived.selections[0].clone()],
    };
    assert_ne!(
        repeated.selection_identity(0).unwrap(),
        repeated.selection_identity(1).unwrap()
    );
    assert_eq!(
        repeated.selection_identity(0).unwrap(),
        repeated.selection_identity(0).unwrap()
    );
    assert!(repeated.selection_identity(2).is_err());
    for selection in &derived.selections {
        assert!(
            runtime
                .spans()
                .iter()
                .any(|s| s.key() == selection.first_span)
        );
        assert!(
            runtime
                .spans()
                .iter()
                .any(|s| s.key() == selection.last_span)
        );
    }
    assert_ne!(attr.identity, derived.identity);
    assert!(item_macro_inputs::derive(owner, &derivative, 0, identity, &runtime).is_err());
    assert!(item_macro_inputs::attribute(owner, &outer, 99, identity, &runtime).is_err());
    let mut inactive = outer.clone();
    inactive.enabled = false;
    assert!(item_macro_inputs::attribute(owner, &inactive, 0, identity, &runtime).is_err());
}

#[test]
fn resolved_derive_annotations_remove_only_certified_indexes_and_bind_source_context() {
    use lrr::{
        core::digest::Digest,
        service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
    };
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenTree, Transparency,
    };
    let source = "#[generate(First)] #[derive(user_tokens)] #[cfg_attr(all(),generate(Last))] struct Value{#[cfg(any())] hidden:Missing,kept:i64}";
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let cfg = CfgSet::default();
    let inputs = parsed
        .derive_item_inputs_at(
            SourceRange {
                start: 0,
                end: u32::try_from(source.len()).unwrap(),
            },
            &cfg,
        )
        .unwrap();
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key([101; 32]),
            content: Key(blob.content().0),
            path: "resolved.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([101; 32]),
            edition: Edition::E2024,
        },
        Key([102; 32]),
    )
    .unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let source_identity = forest.spans()[0].source.clone();
    let owner = Digest([103; 32]);
    let descriptor = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let annotations = ResolvedDeriveAnnotations::new(
        owner,
        &inputs,
        [(2, descriptor.clone()), (0, descriptor.clone())],
    )
    .unwrap();
    let reordered =
        ResolvedDeriveAnnotations::new(owner, &inputs, [(0, descriptor.clone()), (2, descriptor)])
            .unwrap();
    let first = item_macro_inputs::derive_resolved(
        owner,
        &inputs,
        0,
        &annotations,
        &source_identity,
        &forest,
    )
    .unwrap();
    let last = item_macro_inputs::derive_resolved(
        owner,
        &inputs,
        2,
        &annotations,
        &source_identity,
        &forest,
    )
    .unwrap();
    assert_ne!(first.identity, last.identity);
    assert_eq!(
        first.identity,
        item_macro_inputs::derive_resolved(
            owner,
            &inputs,
            0,
            &reordered,
            &source_identity,
            &forest
        )
        .unwrap()
        .identity
    );
    assert_eq!(first.selections[0].segments, ["First"]);
    assert_eq!(last.selections[0].segments, ["Last"]);
    assert_eq!(first.item.encode().unwrap(), last.item.encode().unwrap());
    let item =
        lrr_expand_adapter::token_wire::TokenForest::decode(&first.item.encode().unwrap()).unwrap();
    let mut names = vec![];
    let mut pending: Vec<_> = item.roots().iter().collect();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children),
            TokenTree::Ident { text, .. } => names.push(text.as_str()),
            _ => {}
        }
    }
    assert!(names.contains(&"derive") && names.contains(&"user_tokens") && names.contains(&"kept"));
    for removed in ["generate", "First", "Last", "hidden", "Missing"] {
        assert!(!names.contains(&removed));
    }
    assert!(
        item_macro_inputs::derive_resolved(
            owner,
            &inputs,
            1,
            &annotations,
            &source_identity,
            &forest
        )
        .is_err()
    );
    let mut stale = inputs.clone();
    stale.item.cfg = Digest([255; 32]);
    assert!(
        item_macro_inputs::derive_resolved(
            owner,
            &stale,
            0,
            &annotations,
            &source_identity,
            &forest
        )
        .is_err()
    );
    let mut stale = inputs.clone();
    stale.item.parse.content = Digest([255; 32]);
    assert!(
        item_macro_inputs::derive_resolved(
            owner,
            &stale,
            0,
            &annotations,
            &source_identity,
            &forest
        )
        .is_err()
    );
    let mut mutated = inputs.clone();
    mutated
        .members
        .iter_mut()
        .find(|m| m.enabled)
        .unwrap()
        .enabled = false;
    assert!(
        item_macro_inputs::derive_resolved(
            owner,
            &mutated,
            0,
            &annotations,
            &source_identity,
            &forest
        )
        .is_err()
    );
    let mut mutated = inputs.clone();
    mutated.item.attributes[0].name = "cfg".into();
    assert!(
        item_macro_inputs::derive_resolved(
            owner,
            &mutated,
            0,
            &annotations,
            &source_identity,
            &forest
        )
        .is_err()
    );
    let mut mutations = vec![];
    let mut changed = inputs.clone();
    changed.item.requires_cfg_materialization = !changed.item.requires_cfg_materialization;
    mutations.push(changed);
    let mut changed = inputs.clone();
    changed.item.original_attribute_ranges.pop();
    mutations.push(changed);
    let mut changed = inputs.clone();
    changed.item.attributes[0].tokens[0].text.push('x');
    mutations.push(changed);
    let mut changed = inputs.clone();
    changed.item.attributes[0]
        .argument_range
        .as_mut()
        .unwrap()
        .start += 1;
    mutations.push(changed);
    for changed in mutations {
        let error = item_macro_inputs::derive_resolved(
            owner,
            &changed,
            0,
            &annotations,
            &source_identity,
            &forest,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.message,
            "derive annotation proof differs from selected source item"
        );
    }
    let mut excessive = inputs.clone();
    excessive.members = vec![inputs.members[0].clone(); 100_001];
    assert!(
        ResolvedDeriveAnnotations::new(
            owner,
            &excessive,
            [(
                0,
                lrr::expand::builtin_attributes::descriptor("derive")
                    .unwrap()
                    .unwrap()
            )]
        )
        .is_err()
    );
}

#[test]
fn rustc_original_derive_groups_preserve_later_annotations_and_group_payloads() {
    use lrr::{
        core::digest::Digest,
        service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
    };
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    );
    let expected = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.stdout"
    );
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let unit = Digest([151; 32]);
    let cfg = CfgSet::guest_default();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let item = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Foo")
        .unwrap();
    let facts = parsed
        .derive_item_inputs_at(item.declaration_span, &cfg)
        .unwrap();
    assert_eq!(facts.item.attributes.len(), 4);
    assert_eq!(expected.lines().count(), 5);
    let original = token_wire::parse_source(
        source,
        SourceIdentity {
            unit: Key(unit.0),
            content: Key(blob.content().0),
            path: "tests/ui/proc-macro/derive-expand-order.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key(unit.0),
            edition: Edition::E2024,
        },
        Key([152; 32]),
    )
    .unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&original.encode().unwrap()).unwrap();
    let owner = item.def_key.fingerprint();
    let descriptor = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let proof =
        ResolvedDeriveAnnotations::new(owner, &facts, (0..4).map(|at| (at, descriptor.clone())))
            .unwrap();
    let mut third = None;
    let mut identities = std::collections::BTreeSet::new();
    for ((annotation, ordinal), line) in [(0, 0), (1, 0), (2, 0), (2, 1), (3, 0)]
        .into_iter()
        .zip(expected.lines())
    {
        let input = item_macro_inputs::derive_occurrence_resolved(
            owner,
            &facts,
            (annotation, ordinal),
            &proof,
            &forest.spans()[0].source,
            &forest,
        )
        .unwrap();
        assert!(identities.insert(input.identity()));
        assert!(input.retained_bytes() <= 64 * 1024 * 1024);
        for span in input.item().spans() {
            assert_eq!(
                forest.spans().iter().find(|s| s.key() == span.key()),
                Some(span)
            );
        }
        for context in input.item().contexts() {
            assert!(forest.contexts().contains(context));
        }
        let (prefix, payload) = line.split_once(": ").unwrap();
        assert_eq!(
            input.selected().segments,
            [prefix.strip_prefix("Derive ").unwrap()]
        );
        let actual = token_wire::TokenForest::decode(&input.item().encode().unwrap()).unwrap();
        let expected = token_wire::parse_source(
            payload,
            SourceIdentity {
                unit: Key([153; 32]),
                content: Key([154; 32]),
                path: "expected-payload.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key([153; 32]),
                edition: Edition::E2024,
            },
            Key([155; 32]),
        )
        .unwrap();
        // Ignore only provenance keys; retain complete token spelling, groups and jointness.
        fn shape(tokens: &[token_wire::TokenTree]) -> serde_json::Value {
            serde_json::Value::Array(
                tokens
                    .iter()
                    .map(|t| match t {
                        token_wire::TokenTree::Group {
                            delimiter,
                            children,
                            ..
                        } => serde_json::json!([format!("{delimiter:?}"), shape(children)]),
                        token_wire::TokenTree::Ident { text, raw, .. } => {
                            serde_json::json!(["ident", text, raw])
                        }
                        token_wire::TokenTree::Punct {
                            scalar, spacing, ..
                        } => serde_json::json!(["punct", scalar, format!("{spacing:?}")]),
                        token_wire::TokenTree::Literal { token_text, .. } => {
                            serde_json::json!(["literal", token_text])
                        }
                    })
                    .collect(),
            )
        }
        assert_eq!(shape(actual.roots()), shape(expected.roots()), "{prefix}");
        if ordinal == 1 {
            assert_eq!(third.as_ref().unwrap(), &input.item().encode().unwrap());
        }
        if annotation == 2 && ordinal == 0 {
            third = Some(input.item().encode().unwrap());
        }
    }
    assert_eq!(identities.len(), 5);
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            owner,
            &facts,
            (2, 2),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
    let mut changed = facts.clone();
    changed.item.attributes[3].tokens[0].text.push('x');
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            owner,
            &changed,
            (0, 0),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            Digest([255; 32]),
            &facts,
            (0, 0),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
    let mut changed = facts.clone();
    changed.item.cfg = Digest([255; 32]);
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            owner,
            &changed,
            (0, 0),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
}

#[test]
fn original_derive_occurrences_keep_repeated_paths_member_cfg_and_later_attributes() {
    use lrr::{
        core::digest::Digest,
        service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
    };
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenTree, Transparency,
    };
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    );
    // LRR-specific adaptations remain outside the licensed original directory.
    let source=original.replace("Third, Fourth","Third, Third").replace("pub struct Foo {}","#[later(kept)] pub struct Foo { #[cfg(any())] unavailable:Missing, #[doc=\"kept\"] kept:u8 }");
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let unit = Digest([156; 32]);
    let cfg = CfgSet::guest_default();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let item = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Foo")
        .unwrap();
    let facts = parsed
        .derive_item_inputs_at(item.declaration_span, &cfg)
        .unwrap();
    assert_eq!(facts.item.attributes.len(), 5);
    let forest = token_wire::parse_source(
        &source,
        SourceIdentity {
            unit: Key(unit.0),
            content: Key(blob.content().0),
            path: "adapted-repeated-derive.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key(unit.0),
            edition: Edition::E2024,
        },
        Key([157; 32]),
    )
    .unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&forest.encode().unwrap()).unwrap();
    let owner = item.def_key.fingerprint();
    let descriptor = lrr::expand::builtin_attributes::descriptor("derive")
        .unwrap()
        .unwrap();
    let proof =
        ResolvedDeriveAnnotations::new(owner, &facts, (0..4).map(|at| (at, descriptor.clone())))
            .unwrap();
    let first = item_macro_inputs::derive_occurrence_resolved(
        owner,
        &facts,
        (2, 0),
        &proof,
        &forest.spans()[0].source,
        &forest,
    )
    .unwrap();
    let second = item_macro_inputs::derive_occurrence_resolved(
        owner,
        &facts,
        (2, 1),
        &proof,
        &forest.spans()[0].source,
        &forest,
    )
    .unwrap();
    assert_eq!(first.selected().segments, ["Third"]);
    assert_eq!(second.selected().segments, ["Third"]);
    assert_ne!(first.identity(), second.identity());
    assert_eq!(
        first.item().encode().unwrap(),
        second.item().encode().unwrap()
    );
    let again = item_macro_inputs::derive_occurrence_resolved(
        owner,
        &facts,
        (2, 0),
        &proof,
        &forest.spans()[0].source,
        &forest,
    )
    .unwrap();
    assert_eq!(first.identity(), again.identity());
    let input = token_wire::TokenForest::decode(&first.item().encode().unwrap()).unwrap();
    let mut names = Vec::new();
    let mut literals = Vec::new();
    let mut pending: Vec<_> = input.roots().iter().collect();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children),
            TokenTree::Ident { text, .. } => names.push(text.as_str()),
            TokenTree::Literal { token_text, .. } => literals.push(token_text.as_str()),
            _ => {}
        }
    }
    for kept in ["Fifth", "later", "kept", "doc", "u8"] {
        assert!(names.contains(&kept), "missing {kept}");
    }
    for removed in ["First", "Second", "Third", "unavailable", "Missing"] {
        assert!(!names.contains(&removed), "retained {removed}");
    }
    assert!(literals.contains(&"\"kept\""));
    let mut changed = facts.clone();
    changed
        .members
        .iter_mut()
        .find(|m| m.enabled)
        .unwrap()
        .enabled = false;
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            owner,
            &changed,
            (2, 0),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
    // A retained later attribute is not certified as another compiler derive.
    assert!(
        item_macro_inputs::derive_occurrence_resolved(
            owner,
            &facts,
            (4, 0),
            &proof,
            &forest.spans()[0].source,
            &forest
        )
        .is_err()
    );
}

#[test]
fn original_attribute_paths_preserve_qualifiers_and_require_correct_entry_context() {
    use lrr::syntax::imports::ImportSegment;
    let source = "mod r#gen { #[cfg_attr(all(), ::dependency::r#gen)] fn cold(){missing_typechecking();} trait Tr { #[self::marker] fn member(); } fn outer(){ #[super::marker] fn nested(){} } }";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let select = |text: &str| {
        let start = source.find(text).unwrap();
        parsed
            .attribute_path_at(SourceRange {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + text.len()).unwrap(),
            })
            .unwrap()
    };
    let qualified = select("::dependency::r#gen");
    assert!(qualified.absolute());
    assert_eq!(qualified.module_suffix(), ["gen"]);
    assert_eq!(
        qualified.segments(),
        [
            ImportSegment::Name("dependency".into()),
            ImportSegment::Name("r#gen".into())
        ]
    );
    assert!(!qualified.requires_body_entry());
    assert!(!qualified.requires_container_entry());
    let member = select("self::marker");
    assert_eq!(
        member.segments(),
        [
            ImportSegment::SelfModule,
            ImportSegment::Name("marker".into())
        ]
    );
    assert!(member.requires_container_entry());
    assert!(!member.requires_body_entry());
    let nested = select("super::marker");
    assert!(nested.requires_body_entry());
    assert_eq!(
        nested.segments(),
        [ImportSegment::Super, ImportSegment::Name("marker".into())]
    );
    let mut partial = qualified.range();
    partial.start += 2;
    assert!(parsed.attribute_path_at(partial).is_err());
    assert_eq!(qualified.key(), &parsed.key());
    let bare_source = "#[marker] fn outer(){ #[marker] fn inner(){} }";
    let bare = ParsedFile::parse(
        &SourceBlob::new(bare_source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outer = bare
        .attribute_path_at(SourceRange { start: 2, end: 8 })
        .unwrap();
    assert!(!outer.requires_body_entry());
    let start = bare_source.rfind("marker").unwrap();
    let inner = bare
        .attribute_path_at(SourceRange {
            start: u32::try_from(start).unwrap(),
            end: u32::try_from(start + 6).unwrap(),
        })
        .unwrap();
    assert!(inner.requires_body_entry());
}

#[test]
fn attribute_candidates_require_enabled_indexed_meta_and_source_ranges_do_not_change_headers() {
    use lrr::{
        core::digest::Digest,
        expand::scopes::{MacroDefinition, MacroName},
        frontend::{
            hygiene::HygieneFacts,
            macro_names::{MacroNamespaces, MacroPath, PathLookup},
            source_macros::SourceMacroIndex,
        },
        syntax::{outline::ScopeKey, token_wire as runtime},
    };
    use lrr_expand_adapter::{
        hygiene::HygieneStore,
        token_wire::{Edition, Key, TokenForest},
    };
    let source = "#[cfg(any())] #[chosen] fn absent(){} #[cfg_attr(any(),chosen)] #[chosen] fn kept(){missing();} struct Fields{#[chosen] field:i64}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let unit = Digest([111; 32]);
    let cfg = CfgSet::default();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let mut hygiene = HygieneStore::default();
    let ctx = hygiene.root(Key(unit.0), Edition::E2024).unwrap();
    let resolution = hygiene.resolution_contexts(&[ctx]).unwrap();
    let names = hygiene.name_contexts(&[ctx]).unwrap();
    let facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(
                &TokenForest::new(vec![], hygiene.records(), vec![])
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .unwrap()
            .contexts()
            .to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&resolution).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap()
    };
    let index = SourceMacroIndex::from_outline(&outline, runtime::Key(ctx.0), facts()).unwrap();
    let definition = MacroDefinition {
        identity: Digest([112; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    let mut namespaces = MacroNamespaces::default();
    namespaces
        .define(
            MacroPath::new(unit, vec![MacroName::parse("chosen", 2024).unwrap()]).unwrap(),
            definition.clone(),
        )
        .unwrap();
    let scope = ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    let occurrences: Vec<_> = source
        .match_indices("chosen")
        .map(|(start, _)| {
            parsed
                .attribute_path_at(SourceRange {
                    start: u32::try_from(start).unwrap(),
                    end: u32::try_from(start + 6).unwrap(),
                })
                .unwrap()
        })
        .collect();
    assert!(matches!(
        index
            .lookup_attribute_candidate(&occurrences[0], &namespaces, &scope)
            .unwrap(),
        PathLookup::Absent
    ));
    assert!(matches!(
        index
            .lookup_attribute_candidate(&occurrences[1], &namespaces, &scope)
            .unwrap(),
        PathLookup::Absent
    ));
    assert_eq!(
        index
            .lookup_attribute_candidate(&occurrences[2], &namespaces, &scope)
            .unwrap(),
        PathLookup::Defined(definition)
    );
    assert!(occurrences[3].requires_container_entry());
    assert!(
        index
            .lookup_attribute_candidate(&occurrences[3], &namespaces, &scope)
            .is_err()
    );
    let mut changed = outline.clone();
    let header = changed
        .items
        .iter_mut()
        .find(|i| !i.header.attributes.is_empty())
        .unwrap();
    let before = header.header.fingerprint();
    header.header.attributes[0].source_meta = None;
    assert_eq!(before, header.header.fingerprint());
    assert!(SourceMacroIndex::from_outline(&changed, runtime::Key(ctx.0), facts()).is_err());
    let mut changed = outline.clone();
    let header = changed
        .items
        .iter_mut()
        .find(|i| !i.header.attributes.is_empty())
        .unwrap();
    header.header.attributes[0].source_meta = Some(SourceRange { start: 0, end: 1 });
    assert!(SourceMacroIndex::from_outline(&changed, runtime::Key(ctx.0), facts()).is_err());
}

#[test]
fn pinned_inert_attribute_metadata_is_distinct_from_expansion_handlers() {
    use lrr::expand::builtin_attributes::{descriptor, inert_descriptor};
    let path = inert_descriptor("path").unwrap().unwrap();
    assert_eq!(path.name(), "path");
    assert_eq!(path, inert_descriptor("path").unwrap().unwrap());
    let allow = inert_descriptor("allow").unwrap().unwrap();
    assert_ne!(path.definition_identity(), allow.definition_identity());
    assert!(inert_descriptor("derive").unwrap().is_none());
    assert!(inert_descriptor("test").unwrap().is_none());
    assert!(descriptor("derive").unwrap().is_some());
    assert!(descriptor("test").unwrap().is_some());
    assert!(inert_descriptor("lrr_user_attribute").unwrap().is_none());
    assert!(inert_descriptor(&"x".repeat(65537)).is_err());
}
