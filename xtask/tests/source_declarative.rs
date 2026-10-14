use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> PreparedSourceView {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    view
}

fn request(text: &str) -> lrr::expand::source_declarative::SourceDeclarativeRequest {
    let view = source(text);
    let id = view.outline().item_invocations[0].item_input;
    let c = SourceDeclarative::new(view).unwrap();
    let SourceDeclarativeDemand::Ready(r) =
        c.demand(id, Digest([243; 32]), Digest([244; 32])).unwrap()
    else {
        panic!("source demand unresolved")
    };
    *r
}
#[test]
fn original_source_macro_demand_expands_actual_argument_tokens_with_captured_origins() {
    let r = request(
        "macro_rules! make{($x:expr)=>{pub fn made(){ $x;}}}make!(7_i64);fn cold(){let _:Missing=missing();}",
    );
    let key = r.key().clone();
    let record = r.expand().unwrap();
    let g =
        lrr::expand::generated_items::GeneratedItems::admit(&key, std::sync::Arc::new(record), 1)
            .unwrap();
    assert_eq!(g.items()[0].name.as_ref().unwrap().text, "made");
    assert!(
        g.tokens()
            .spans()
            .iter()
            .all(|s| s.source.path == "main.rs")
    );
    let mut found = false;
    let mut stack = g.tokens().roots().iter().collect::<Vec<_>>();
    while let Some(t) = stack.pop() {
        match t {
            lrr::syntax::token_wire::TokenTree::Group { children, .. } => stack.extend(children),
            lrr::syntax::token_wire::TokenTree::Literal { token_text, .. }
                if token_text == "7_i64" =>
            {
                found = true
            }
            _ => {}
        }
    }
    assert!(found);
}
#[test]
fn rustc_generated_item_token_origins_project_to_current_argument_source() {
    use lrr::expand::generated_items::GeneratedItems;
    use lrr::syntax::spans::SourceRange;
    // Entire upstream source is unchanged; only explicit newline freshness
    // adaptations live here, outside the licensed copy. This exercises token
    // projection, not the stock fixture's method-dispatch check-pass contract.
    let stock = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-named-default.rs"
    );
    let mut previous_content = None;
    for prefix in ["", "\n\n"] {
        let text = format!("{prefix}{stock}");
        let demand = request(&text);
        let key = demand.key().clone();
        let generated =
            GeneratedItems::admit(&key, std::sync::Arc::new(demand.expand().unwrap()), 1).unwrap();
        let start = u32::try_from(generated.text().find('A').unwrap()).unwrap();
        let selected = generated
            .project_source_spans(SourceRange {
                start,
                end: start + 1,
            })
            .unwrap();
        assert_eq!(selected.len(), 1);
        let span = selected[0];
        assert_eq!(span.source.content.0, Digest::of_bytes(text.as_bytes()).0);
        assert_eq!(span.source.unit.0, [241; 32]);
        assert_eq!(span.source.path, "main.rs");
        let expected = text.find("struct A;").unwrap() + "struct ".len();
        assert_eq!(
            (span.start as usize, span.end as usize),
            (expected, expected + 1)
        );
        assert_eq!(&text[span.start as usize..span.end as usize], "A");
        if let Some(previous) = previous_content {
            assert_ne!(
                span.source.content, previous,
                "projection reused a previous edit's source"
            );
        }
        previous_content = Some(span.source.content);
        let point = generated
            .project_source_spans(SourceRange { start, end: start })
            .unwrap();
        assert_eq!(point, selected);
        let all = generated
            .project_source_spans(SourceRange {
                start: 0,
                end: u32::try_from(generated.text().len()).unwrap(),
            })
            .unwrap();
        assert!(all.len() > 1, "composite source origins were collapsed");
        assert!(all.contains(&span));
        assert_eq!(
            generated
                .project_source_spans(SourceRange {
                    start: start + 1,
                    end: start
                })
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
        assert_eq!(
            generated
                .project_source_spans(SourceRange {
                    start: 0,
                    end: u32::MAX
                })
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
    }
}

#[test]
fn source_declarative_definition_is_parsed_only_on_expansion_demand() {
    let r = request("macro_rules! broken{bad}broken!();");
    assert_eq!(r.expand().unwrap_err().code, "LRR-E-MACRO-EXPANSION");
}
#[test]
fn source_declarative_unknown_or_preceding_producer_stays_typed_pending() {
    for text in [
        "unknown!();",
        "#[cfg(any())]macro_rules! hidden{()=>{}}hidden!();",
        "unknown!();macro_rules! ready{()=>{}}ready!();",
    ] {
        let v = source(text);
        let id = v.outline().item_invocations.last().unwrap().item_input;
        let c = SourceDeclarative::new(v).unwrap();
        assert!(matches!(
            c.demand(id, Digest([243; 32]), Digest([244; 32])).unwrap(),
            SourceDeclarativeDemand::NeedInput(_)
        ));
        assert!(
            matches!(c.demand(Digest([249;32]),Digest([243;32]),Digest([244;32])),Err(e) if e.code=="LRR-E-EXPANSION-INPUT")
        );
    }
}
#[test]
fn source_declarative_shadowing_and_snapshot_identity_are_preserved() {
    let mut records = Vec::new();
    for n in ["1_i64", "2_i64"] {
        let r = request(&format!(
            "macro_rules! make{{()=>{{pub fn first(){{}}}}}}macro_rules! make{{()=>{{pub fn second(){{{n};}}}}}}make!();"
        ));
        let k = r.key().clone();
        let rec = r.expand().unwrap();
        records.push(rec.descriptor().fingerprint());
        let g =
            lrr::expand::generated_items::GeneratedItems::admit(&k, std::sync::Arc::new(rec), 1)
                .unwrap();
        assert_eq!(g.items()[0].name.as_ref().unwrap().text, "second");
    }
    assert_ne!(records[0], records[1]);
}

#[test]
fn captured_ordinary_module_facts_keep_actual_definition_producer_and_source_cutoff() {
    use lrr::frontend::source_module_declarations::SourceModuleDeclaration;
    let source = source("mod b;mod a{#[path=\"chosen.rs\"]pub mod child;}fn cold(){missing();}");
    let facts = SourceModuleDeclaration::from_captured(&source).unwrap();
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].request().name, "b");
    assert_eq!(
        facts[1].request().path_override.as_deref(),
        Some("chosen.rs")
    );
    assert_eq!(facts[1].scope().module_path, ["a", "child"]);
    assert_eq!(facts[1].definition().module_path, ["a"]);
    assert!(facts[0].range().start < facts[1].range().start);
    assert_eq!(facts[0].source().selection(), source.selection());
    assert_ne!(facts[0].producer(), facts[1].producer());
    assert_ne!(facts[0].fingerprint(), facts[1].fingerprint());
}
#[test]
fn captured_ordinary_module_facts_filter_cfg_and_normalize_raw_unicode_names() {
    use lrr::frontend::source_module_declarations::SourceModuleDeclaration;
    let view = source("#[cfg(any())]mod absent;mod r#type;mod cafe\u{301};");
    let facts = SourceModuleDeclaration::from_captured(&view).unwrap();
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].request().name, "type");
    assert_eq!(facts[1].request().name, "café");
    assert_eq!(facts[1].definition().declared_name, "café");
}
#[test]
fn captured_ordinary_module_facts_change_with_exact_source_bytes_and_header_position() {
    use lrr::frontend::source_module_declarations::SourceModuleDeclaration;
    let a =
        SourceModuleDeclaration::from_captured(&source("mod child;fn cold(){first();}")).unwrap();
    let b =
        SourceModuleDeclaration::from_captured(&source("mod child;fn cold(){second();}")).unwrap();
    let c =
        SourceModuleDeclaration::from_captured(&source("\nmod child;fn cold(){first();}")).unwrap();
    assert_eq!(a[0].definition(), b[0].definition());
    assert_eq!(a[0].producer(), b[0].producer());
    assert_ne!(a[0].fingerprint(), b[0].fingerprint());
    assert_ne!(a[0].fingerprint(), c[0].fingerprint());
}

fn ordinary_capture(
    text: &str,
    payload: &str,
) -> (
    lrr::frontend::source_module_declarations::SourceModuleDeclaration,
    lrr::db::poll::InputRequest,
    std::sync::Arc<lrr::source::module_capture::AdoptedModuleCapture>,
) {
    use lrr::{
        db::{memo::Database, poll::InputRequest, provider::SyntaxProvider},
        frontend::source_module_declarations::SourceModuleDeclaration,
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            source_capture::SourceCapture,
        },
        source::{
            include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
            manifest::BodyKey,
        },
    };
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT
        .try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .unwrap();
    let root = std::env::temp_dir().join(format!(
        "lrr-ordinary-capture-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::write(root.join("child.rs"), payload).unwrap();
    let caller = source(text);
    let declaration = SourceModuleDeclaration::from_captured(&caller)
        .unwrap()
        .remove(0);
    let unit = caller.outline().root_scope.crate_key;
    let generation = Digest([88; 32]);
    let environment = Digest([89; 32]);
    let authorities = IncludeAuthorities::new(
        unit,
        generation,
        [IncludeRoot {
            identity: Digest([87; 32]),
            path: root.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let input = declaration
        .capture_request(&authorities, std::path::Path::new(""), environment)
        .unwrap();
    let capture = SourceCapture::open(authorities, std::collections::BTreeMap::new()).unwrap();
    let InputRequest::CaptureModule { request, .. } = &input else {
        panic!("module input")
    };
    let record = capture.capture_module(request).unwrap();
    let mut db = Database::default();
    let context = DemandContext {
        request: Digest([90; 32]),
        session: Digest([91; 32]),
        unit,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: generation,
        native_generation: None,
        compile_environment: environment,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let phase = PhaseIdentity {
        frame: generation,
        body: BodyKey {
            definition: declaration.definition().fingerprint(),
            exact_body_bytes: input.fingerprint(),
        },
        index: 0,
    };
    let token = coordinator
        .issue(
            lrr::db::key::QueryKey::Outline(caller.selection().outline.clone()),
            phase,
            input.clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    coordinator
        .adopt_module(&context, &token, &mut db, record)
        .unwrap();
    let lrr::db::poll::QueryPoll::Ready(Ok(adopted)) =
        SyntaxProvider::module_capture(&mut db, &input)
    else {
        panic!("adoption absent")
    };
    std::fs::remove_dir_all(root).unwrap();
    (declaration, input, adopted)
}
#[test]
fn ordinary_module_capture_projects_actual_adopted_child_without_generated_identity() {
    use lrr::source::ordinary_module::{OrdinaryModuleSourceView, OrdinaryModuleSyntaxInput};
    let (declaration, input, adopted) = ordinary_capture(
        "mod child;",
        "pub fn made()->i64{2053_i64}fn cold<T:Missing>(){unknown();}",
    );
    let origin =
        std::sync::Arc::new(OrdinaryModuleSourceView::new(&declaration, &input, adopted).unwrap());
    let syntax = OrdinaryModuleSyntaxInput::prepare(origin).unwrap();
    assert_eq!(
        syntax.prepared_source().outline().root_scope.module_path,
        ["child"]
    );
    assert_eq!(
        syntax.origin().declaration().definition(),
        declaration.definition()
    );
    assert_eq!(
        syntax.origin().capture().input_identity(),
        input.fingerprint()
    );
    assert!(
        syntax
            .prepared_source()
            .outline()
            .items
            .iter()
            .any(|item| item.def_key.declared_name == "made")
    );
    assert_eq!(
        syntax.prepared_source().outline().profile,
        declaration.source().outline().profile
    );
    assert!(syntax.metadata_bytes() > 0);
    assert!(
        syntax
            .inputs()
            .keys()
            .any(|key| matches!(key, lrr::db::dependency::InputKey::DefinitionBody(_)))
    );
    assert!(
        !syntax
            .inputs()
            .keys()
            .any(|key| matches!(key, lrr::db::dependency::InputKey::ExpansionRecord(_)))
    );
}
#[test]
fn ordinary_module_capture_rejects_changed_source_and_substituted_declaration() {
    use lrr::{
        frontend::source_module_declarations::SourceModuleDeclaration,
        source::ordinary_module::OrdinaryModuleSourceView,
    };
    let (declaration, input, adopted) =
        ordinary_capture("mod child;", "pub fn made()->i64{2053_i64}");
    for text in ["mod child;fn extra(){}", "mod sibling;", "\nmod child;"] {
        let other = SourceModuleDeclaration::from_captured(&source(text))
            .unwrap()
            .remove(0);
        assert!(OrdinaryModuleSourceView::new(&other, &input, adopted.clone()).is_err());
    }
    OrdinaryModuleSourceView::new(&declaration, &input, adopted).unwrap();
}
#[test]
fn ordinary_module_projection_identity_tracks_actual_captured_child_bytes() {
    use lrr::source::ordinary_module::{OrdinaryModuleSourceView, OrdinaryModuleSyntaxInput};
    let project = |payload: &str| {
        let (declaration, input, adopted) = ordinary_capture("mod child;", payload);
        let origin = std::sync::Arc::new(
            OrdinaryModuleSourceView::new(&declaration, &input, adopted).unwrap(),
        );
        OrdinaryModuleSyntaxInput::prepare(origin).unwrap()
    };
    let a = project("pub fn made()->i64{2053_i64}");
    let b = project("pub fn made()->i64{2063_i64}");
    assert_ne!(a.identity(), b.identity());
    assert_ne!(
        a.prepared_source().selection().outline.parse.content,
        b.prepared_source().selection().outline.parse.content
    );
    assert_eq!(
        a.prepared_source().outline().items[0].def_key,
        b.prepared_source().outline().items[0].def_key
    );
}
