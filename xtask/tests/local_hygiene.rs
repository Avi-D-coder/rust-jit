//! Actual pinned marked expansion and owned local lookup against a standalone
//! native oracle. Source facts are explicitly supplied; discovery/VM are later.
use lrr::{
    core::digest::Digest,
    expand::scopes::{MacroDefinition, MacroName, ScopeKey},
    frontend::{
        hygiene::HygieneFacts,
        local_scopes::{LocalLookup, LocalScopes},
    },
    syntax::token_wire as runtime,
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    hygiene::HygieneStore,
    token_wire::*,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
#[test]
fn marked_local_lookup_matches_native_definition_argument_and_generated_collisions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m007-local-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc-version.txt"), version.stdout).unwrap();
    let body = "($argument:ident) => {(x,$argument,{let x=43;x})};";
    let source = format!(
        "fn main() {{let x=7;macro_rules! pick {{{body}}} let result={{let x=31;pick!(x)}};println!(\"[{{}},{{}},{{}},{{}}]\",result.0,result.1,result.2,x);}}"
    );
    let path = evidence.join("locals.rs");
    let binary = evidence.join("locals");
    let original_source = source.clone();
    fs::write(&path, source).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(evidence.join("compiler.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(evidence.join("native.stdout"), &native.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let source = "pick!(x);";
    let parsed = parse_source(
        source,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(*blake3::hash(source.as_bytes()).as_bytes()),
            path: "locals.rs".into(),
        },
        context.clone(),
        Key([3; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = parsed.roots() else {
        panic!("missing source root")
    };
    let input_group = children
        .iter()
        .find(|t| {
            matches!(
                t,
                TokenTree::Group {
                    delimiter: Delimiter::Parenthesis,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let input = TokenForest::new(
        vec![input_group],
        parsed.contexts().to_vec(),
        parsed.spans().to_vec(),
    )
    .unwrap();
    let definition_text = format!("macro_rules! pick {{ {body} }}");
    let definition_source = parse_source(
        &definition_text,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(*blake3::hash(definition_text.as_bytes()).as_bytes()),
            path: "locals.rs".into(),
        },
        context.clone(),
        Key([4; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = definition_source.roots() else {
        panic!("missing definition source root")
    };
    let definition_group = children
        .iter()
        .find(|token| {
            matches!(
                token,
                TokenTree::Group {
                    delimiter: Delimiter::Brace,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let definition = TokenForest::new(
        vec![definition_group],
        definition_source.contexts().to_vec(),
        definition_source.spans().to_vec(),
    )
    .unwrap();
    let rules = DeclarativeMacro::new(definition).unwrap();
    let [
        TokenTree::Group {
            open: call_site, ..
        },
    ] = input.roots()
    else {
        panic!("missing call group")
    };
    let site = DeclarativeSite {
        expansion: Key([8; 32]),
        defining_unit: Key([1; 32]),
        definition_edition: Edition::E2024,
    };
    let (expansion, stats) = rules
        .expand_hygienic_with_limits(&input, *call_site, site, 100, 4096)
        .unwrap();
    let expanded = expansion.unwrap().tokens;
    assert_eq!(stats.admitted_bytes, expanded.encoded_len().unwrap());
    let mut adapter = HygieneStore::default();
    adapter.import(expanded.contexts()).unwrap();
    let demanded = adapter
        .records()
        .iter()
        .map(|r| r.key())
        .collect::<Vec<_>>();
    let rows = adapter.resolution_contexts(&demanded).unwrap();
    let owned = runtime::TokenForest::decode(&expanded.encode().unwrap()).unwrap();
    let rows = serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap();
    let contexts = runtime::TokenForest::decode(
        &TokenForest::new(vec![], adapter.records(), vec![])
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    let names = adapter
        .name_contexts(
            &adapter
                .records()
                .iter()
                .map(|r| r.key())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let names = serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap();
    let facts = Arc::new(
        HygieneFacts::new(contexts.contexts().to_vec(), rows)
            .unwrap()
            .with_name_contexts(names)
            .unwrap(),
    );
    let mut identifiers = Vec::new();
    let mut stack = owned.roots().iter().rev().collect::<Vec<_>>();
    while let Some(token) = stack.pop() {
        match token {
            runtime::TokenTree::Group { children, .. } => stack.extend(children.iter().rev()),
            runtime::TokenTree::Ident { text, span, .. } if text == "x" => identifiers.push(
                owned
                    .spans()
                    .iter()
                    .find(|r| r.key() == *span)
                    .unwrap()
                    .context,
            ),
            _ => {}
        }
    }
    assert_eq!(identifiers.len(), 4);
    assert_ne!(identifiers[0], identifiers[1]);
    assert_eq!(identifiers[0], identifiers[2]);
    assert_eq!(identifiers[2], identifiers[3]);
    let root_context = runtime::Key(context.key().0);
    let unit = Digest([1; 32]);
    let name = MacroName::parse("x", 2024).unwrap();
    let root_scope = ScopeKey::root(unit);
    let mut scopes = LocalScopes::new(facts);
    scopes.add_root(root_scope).unwrap();
    scopes
        .bind(root_scope, 5, name.clone(), root_context, Digest([2; 32]))
        .unwrap();
    let definition = MacroDefinition {
        identity: Digest([9; 32]),
        defining_unit: unit,
        definition_edition: 2024,
    };
    scopes
        .define_macro(root_scope, 10, definition.clone())
        .unwrap();
    scopes
        .register_expansion(runtime::Key(site.expansion.0), definition)
        .unwrap();
    let caller = scopes.add_child(root_scope, 20, 0, false).unwrap();
    scopes
        .bind(caller, 25, name.clone(), root_context, Digest([3; 32]))
        .unwrap();
    let generated = scopes.add_child(caller, 30, 0, false).unwrap();
    scopes
        .bind(generated, 1, name.clone(), identifiers[2], Digest([4; 32]))
        .unwrap();
    let values = BTreeMap::from([
        (Digest([2; 32]), 7u64),
        (Digest([3; 32]), 31),
        (Digest([4; 32]), 43),
    ]);
    let mut observed = Vec::new();
    for (scope, before, context) in [
        (caller, 30, identifiers[0]),
        (caller, 30, identifiers[1]),
        (generated, 2, identifiers[3]),
        (root_scope, 40, root_context),
    ] {
        let LocalLookup::Binding(binding) = scopes.lookup(scope, before, &name, context).unwrap()
        else {
            panic!("missing local binding")
        };
        observed.push(values[&binding]);
    }
    assert_eq!(observed, native);

    // Source-driven original bindings and declaration ribs, using the same pinned marks.
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::key::OutlineKey,
        frontend::{source_locals::PatternResolution, source_macros::PublishedMacroIndex},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("locals.rs").unwrap(),
        crate_key: unit,
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(original_source.as_bytes().to_vec()).unwrap(),
    };
    let selected = SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile: unit.profile.clone(),
            },
            crate_key: unit.crate_key,
            module_path: vec![],
            cfg: unit.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![unit], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selected).unwrap() else {
        panic!("original source missing")
    };
    let body_key = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let demanded = adapter
        .records()
        .iter()
        .map(|r| r.key())
        .collect::<Vec<_>>();
    let rows = adapter.resolution_contexts(&demanded).unwrap();
    let names = adapter.name_contexts(&demanded).unwrap();
    let contexts = runtime::TokenForest::decode(
        &TokenForest::new(vec![], adapter.records(), vec![])
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    let source_facts = HygieneFacts::new(
        contexts.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let published = PublishedMacroIndex::from_view(view, root_context, source_facts).unwrap();
    let entered = published.enter_body(&selected, &body_key).unwrap();
    let definition = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            lrr::syntax::entered_macros::BodyMacroEventKind::Definition {
                identity, name, ..
            } if name == "pick" => Some(MacroDefinition {
                identity: *identity,
                defining_unit: Digest([1; 32]),
                definition_edition: 2024,
            }),
            _ => None,
        })
        .unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let source_index = entered
        .local_index(
            &selected,
            &body_key,
            &resolutions,
            &[(runtime::Key(site.expansion.0), definition)],
        )
        .unwrap();
    let source_values: BTreeMap<_, _> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .map(|b| {
            let bytes = &original_source
                [b.name_range.end as usize..usize::try_from(b.visible_from).unwrap()];
            let value = bytes
                .trim()
                .strip_prefix('=')
                .unwrap()
                .trim()
                .trim_end_matches(';')
                .trim()
                .parse::<u64>()
                .unwrap();
            (b.identity, value)
        })
        .collect();
    let call_at = u64::try_from(original_source.find("pick!(x)").unwrap()).unwrap();
    let root_at = u64::try_from(original_source.find("println!").unwrap()).unwrap();
    let mut source_observed = Vec::new();
    let mut source_bindings = Vec::new();
    for (at, context) in [
        (call_at, identifiers[0]),
        (call_at, identifiers[1]),
        (root_at, root_context),
    ] {
        let LocalLookup::Binding(binding) = source_index
            .lookup(&selected, &body_key, at, "x", context)
            .unwrap()
        else {
            panic!("original local source selection failed")
        };
        source_observed.push(source_values[&binding]);
        source_bindings.push(binding.hex());
    }
    assert_eq!(source_observed, vec![native[0], native[1], native[3]]);
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual marked-token and owned local-name selection component; original source binding/rib lookup integrated; generated-source publication and guest VM pending","observed":observed,"native":native,"original_source_observed":source_observed,"original_source_binding_ids":source_bindings,"passed":true})).unwrap()).unwrap();
}

#[test]
fn supplied_statement_local_publication_matches_actual_marked_expansion_and_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let evidence = root.join(format!(
        "artifacts/macos-first/m007-local-publications-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc-version.txt"), version.stdout).unwrap();
    let body = "($argument:ident) => {let x=43;$argument=x};";
    let source = format!(
        "fn main() {{let mut x=7;let mut result=0;macro_rules! choose {{()=>{{17}};}} macro_rules! emit {{{body}}} let mut samples=Vec::new();samples.push(choose!());{{samples.push(choose!());macro_rules! choose {{()=>{{41}};}} samples.push(choose!());emit!(result);samples.push(result);samples.push(x);}}samples.push(choose!());println!(\"{{:?}}\",samples);}}"
    );
    let path = evidence.join("locals.rs");
    let binary = evidence.join("locals");
    let original_source = source.clone();
    fs::write(&path, source).unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(evidence.join("compiler.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let native = Command::new(binary).output().unwrap();
    assert!(native.status.success());
    fs::write(evidence.join("native.stdout"), &native.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let source = "emit!(result);";
    let parsed = parse_source(
        source,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(*blake3::hash(source.as_bytes()).as_bytes()),
            path: "locals.rs".into(),
        },
        context.clone(),
        Key([3; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = parsed.roots() else {
        panic!("missing source root")
    };
    let input_group = children
        .iter()
        .find(|t| {
            matches!(
                t,
                TokenTree::Group {
                    delimiter: Delimiter::Parenthesis,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let input = TokenForest::new(
        vec![input_group],
        parsed.contexts().to_vec(),
        parsed.spans().to_vec(),
    )
    .unwrap();
    let definition_text = format!("macro_rules! emit {{ {body} }}");
    let definition_source = parse_source(
        &definition_text,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(*blake3::hash(definition_text.as_bytes()).as_bytes()),
            path: "locals.rs".into(),
        },
        context.clone(),
        Key([4; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = definition_source.roots() else {
        panic!("missing definition source root")
    };
    let definition_group = children
        .iter()
        .find(|token| {
            matches!(
                token,
                TokenTree::Group {
                    delimiter: Delimiter::Brace,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let definition = TokenForest::new(
        vec![definition_group],
        definition_source.contexts().to_vec(),
        definition_source.spans().to_vec(),
    )
    .unwrap();
    let rules = DeclarativeMacro::new(definition).unwrap();
    let [
        TokenTree::Group {
            open: call_site, ..
        },
    ] = input.roots()
    else {
        panic!("missing call group")
    };
    let site = DeclarativeSite {
        expansion: Key([8; 32]),
        defining_unit: Key([1; 32]),
        definition_edition: Edition::E2024,
    };
    let (expansion, stats) = rules
        .expand_hygienic_with_limits(&input, *call_site, site, 100, 4096)
        .unwrap();
    let expanded = expansion.unwrap().tokens;
    assert_eq!(stats.admitted_bytes, expanded.encoded_len().unwrap());
    let mut adapter = HygieneStore::default();
    adapter.import(expanded.contexts()).unwrap();
    let owned = runtime::TokenForest::decode(&expanded.encode().unwrap()).unwrap();
    let mut identifiers = Vec::new();
    let mut stack = owned.roots().iter().rev().collect::<Vec<_>>();
    while let Some(token) = stack.pop() {
        match token {
            runtime::TokenTree::Group { children, .. } => stack.extend(children.iter().rev()),
            runtime::TokenTree::Ident { text, span, .. } if text == "x" => identifiers.push(
                owned
                    .spans()
                    .iter()
                    .find(|r| r.key() == *span)
                    .unwrap()
                    .context,
            ),
            _ => {}
        }
    }
    assert_eq!(identifiers.len(), 2);
    assert_eq!(identifiers[0], identifiers[1]);
    let root_context = runtime::Key(context.key().0);
    let unit = Digest([1; 32]);
    // Source-driven original bindings and declaration ribs, using the same pinned marks.
    use lrr::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::key::OutlineKey,
        frontend::{
            source_locals::{EmittedLocalDeclaration, PatternResolution, SourceLocalPublication},
            source_macros::PublishedMacroIndex,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("locals.rs").unwrap(),
        crate_key: unit,
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::default(),
        blob: SourceBlob::new(original_source.as_bytes().to_vec()).unwrap(),
    };
    let selected = SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile: unit.profile.clone(),
            },
            crate_key: unit.crate_key,
            module_path: vec![],
            cfg: unit.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![unit], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selected).unwrap() else {
        panic!("original source missing")
    };
    let body_key = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let demanded = adapter
        .records()
        .iter()
        .map(|r| r.key())
        .collect::<Vec<_>>();
    let rows = adapter.resolution_contexts(&demanded).unwrap();
    let names = adapter.name_contexts(&demanded).unwrap();
    let contexts = runtime::TokenForest::decode(
        &TokenForest::new(vec![], adapter.records(), vec![])
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    let source_facts = HygieneFacts::new(
        contexts.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let published = PublishedMacroIndex::from_view(view, root_context, source_facts).unwrap();
    let entered = published.enter_body(&selected, &body_key).unwrap();
    let definition = entered
        .facts()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            lrr::syntax::entered_macros::BodyMacroEventKind::Definition {
                identity, name, ..
            } if name == "emit" => Some(MacroDefinition {
                identity: *identity,
                defining_unit: Digest([1; 32]),
                definition_edition: 2024,
            }),
            _ => None,
        })
        .unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let source_index = entered
        .local_index(
            &selected,
            &body_key,
            &resolutions,
            &[(runtime::Key(site.expansion.0), definition.clone())],
        )
        .unwrap();
    let source_values: BTreeMap<_, _> = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .map(|b| {
            let bytes = &original_source
                [b.name_range.end as usize..usize::try_from(b.visible_from).unwrap()];
            let value = bytes
                .trim()
                .strip_prefix('=')
                .unwrap()
                .trim()
                .trim_end_matches(';')
                .trim()
                .parse::<u64>()
                .unwrap();
            (b.identity, value)
        })
        .collect();
    let call_at = u64::try_from(original_source.find("emit!(result)").unwrap()).unwrap();
    let after = u64::try_from(original_source.find("samples.push(result)").unwrap()).unwrap();
    let producer = entered
        .facts()
        .events
        .iter()
        .find_map(|event| match event.kind {
            lrr::syntax::entered_macros::BodyMacroEventKind::Producer { identity, .. }
                if u64::from(event.range.start) == call_at =>
            {
                Some(identity)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        source_index
            .lookup(&selected, &body_key, after, "x", identifiers[1])
            .unwrap(),
        LocalLookup::NeedInput(producer)
    );
    // This fixture's one emitted declaration is read from actual owned output,
    // not an expected name/value table. General generated AST discovery is M013.
    let mut output_tokens = Vec::new();
    let mut stack = owned.roots().iter().rev().collect::<Vec<_>>();
    while let Some(token) = stack.pop() {
        match token {
            runtime::TokenTree::Group { children, .. } => stack.extend(children.iter().rev()),
            _ => output_tokens.push(token),
        }
    }
    let [
        runtime::TokenTree::Ident { text: keyword, .. },
        runtime::TokenTree::Ident {
            text: name, span, ..
        },
        runtime::TokenTree::Punct { scalar: '=', .. },
        runtime::TokenTree::Literal { token_text, .. },
        runtime::TokenTree::Punct { scalar: ';', .. },
        ..,
    ] = output_tokens.as_slice()
    else {
        panic!("fixture emitted declaration shape missing")
    };
    assert_eq!(keyword, "let");
    let declaration_context = owned
        .spans()
        .iter()
        .find(|s| s.key() == *span)
        .unwrap()
        .context;
    let emitted_value = token_text.parse::<u64>().unwrap();
    let publication = SourceLocalPublication {
        source: selected.clone(),
        body: body_key.clone(),
        producer,
        expansion: runtime::Key(site.expansion.0),
        output: Digest(*blake3::hash(&owned.encode().unwrap()).as_bytes()),
        declarations: vec![EmittedLocalDeclaration::Binding {
            name: (*name).clone(),
            context: declaration_context,
        }],
    };
    let published_locals = entered
        .local_index_with_publications(
            &selected,
            &body_key,
            &resolutions,
            &[(runtime::Key(site.expansion.0), definition)],
            std::slice::from_ref(&publication),
        )
        .unwrap();
    let LocalLookup::Binding(generated) = published_locals
        .lookup(&selected, &body_key, after, name, identifiers[1])
        .unwrap()
    else {
        panic!("published generated name missing")
    };
    assert_eq!(
        generated,
        publication.binding_identity(&body_key, 0).unwrap()
    );
    let LocalLookup::Binding(original) = published_locals
        .lookup(&selected, &body_key, after, "x", root_context)
        .unwrap()
    else {
        panic!("original caller name missing")
    };
    use lrr::frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup};
    let mut textual_values = Vec::new();
    let mut selected_macro_ids = Vec::new();
    for (at, _) in original_source.match_indices("choose!()") {
        let HygienicLookup::Selection(MacroSelection::Defined(selected_definition)) = entered
            .lookup(&selected, &body_key, at as u64, "choose")
            .unwrap()
        else {
            panic!("source textual macro selection missing")
        };
        let declaration = entered.facts().events.iter().find(|event| matches!(event.kind,lrr::syntax::entered_macros::BodyMacroEventKind::Definition{identity,..} if identity==selected_definition.identity)).unwrap();
        let text =
            &original_source[declaration.range.start as usize..declaration.range.end as usize];
        let forest = parse_source(
            text,
            SourceIdentity {
                unit: Key([1; 32]),
                content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                path: "locals.rs".into(),
            },
            context.clone(),
            Key([4; 32]),
        )
        .unwrap();
        let [TokenTree::Group { children, .. }] = forest.roots() else {
            panic!("definition source root missing")
        };
        let body = children
            .iter()
            .find(|token| {
                matches!(
                    token,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let definition = DeclarativeMacro::new(
            TokenForest::new(
                vec![body],
                forest.contexts().to_vec(),
                forest.spans().to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
        let input = fixture::input("");
        let output = definition
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        let literals = fixture::literal_texts(&output.tokens);
        assert_eq!(literals.len(), 1);
        textual_values.push(literals[0].parse::<u64>().unwrap());
        selected_macro_ids.push(selected_definition.identity.hex());
    }
    assert_eq!(textual_values.len(), 4);
    let mut observed = textual_values[..3].to_vec();
    observed.extend([emitted_value, source_values[&original], textual_values[3]]);
    assert_eq!(observed, native);
    assert_eq!(observed.len(), 6);
    fs::write(evidence.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","fixture":"MX012","scope":"one actual native macOS program versus original source textual macro selection and pinned selected-template expansion, supplied statement-local publication from actual marked expansion and caller/generated local collision; general generated AST discovery, nested generated scopes and guest VM pending","observed":observed,"native":native,"selected_macro_ids":selected_macro_ids,"original_source_binding_id":original.hex(),"generated_binding_id":generated.hex(),"producer":producer.hex(),"output":publication.output.hex(),"passed":true})).unwrap()).unwrap();
}
