use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    frontend::impl_index::ImplHeaderIndex,
    source::snapshot::{CfgSet, SourceBlob, SourcePath},
    syntax::{declaration_scan::DeclarationScanLimits, outline::ScopeKey, parse::ParsedFile},
};
use std::sync::Arc;

fn scan(text: &str, edition: u16) -> Arc<lrr::syntax::declaration_scan::DeclarationScan> {
    scan_at(text, edition, "main.rs")
}
fn scan_at(
    text: &str,
    edition: u16,
    path: &str,
) -> Arc<lrr::syntax::declaration_scan::DeclarationScan> {
    let mut profile = ProfileIdentity::current();
    profile.edition = edition;
    let parsed = ParsedFile::parse_declarations(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        profile,
    )
    .unwrap();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    Arc::new(
        parsed
            .scan_declarations(
                SourcePath::new(path).unwrap(),
                ScopeKey {
                    crate_key: Digest([61; 32]),
                    module_path: vec![],
                    owner: None,
                },
                &CfgSet::guest_default(),
                DeclarationScanLimits::default(),
            )
            .unwrap(),
    )
}

#[test]
fn rustc_cold_statement_macro_keeps_real_definition_and_grammar() {
    use lrr::{expand::identity::GrammarSlot, frontend::impl_index::DeclarationMacroIndex};
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let scan = scan(source, 2024);
    assert_eq!(scan.macro_definitions().len(), 2);
    assert_eq!(scan.producers().len(), 1);
    assert_eq!(scan.producers()[0].grammar, GrammarSlot::Statements);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    let definition = index.lookup(scan.producers()[0].identity).unwrap();
    let lrr::frontend::macro_scopes::HygienicLookup::Selection(
        lrr::frontend::macro_origins::MacroSelection::Defined(definition),
    ) = definition
    else {
        panic!("cold source macro did not resolve")
    };
    assert_eq!(definition.identity, scan.macro_definitions()[1].identity);
    assert!(
        scan.impls().is_empty(),
        "definition token trees were traversed"
    );
}

#[test]
fn cold_macro_scopes_shadow_locally_without_leaking_to_siblings() {
    use lrr::frontend::{
        impl_index::DeclarationMacroIndex, macro_origins::MacroSelection,
        macro_scopes::HygienicLookup,
    };
    let scan = scan(
        "macro_rules! make {()=>{struct Root;}} fn cold(){ {macro_rules! make {()=>{impl S{}}} make!();} } fn sibling(){make!();}",
        2024,
    );
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    assert_ne!(
        scan.producers()[0].lexical_scope,
        scan.producers()[1].lexical_scope
    );
    for (producer, expected) in scan.producers().iter().zip([1, 0]) {
        let HygienicLookup::Selection(MacroSelection::Defined(found)) =
            index.lookup(producer.identity).unwrap()
        else {
            panic!("shadow selection absent")
        };
        assert_eq!(found.identity, scan.macro_definitions()[expected].identity);
    }
}

#[test]
fn cold_macro_parent_visibility_freezes_before_later_source_definition() {
    use lrr::frontend::{
        impl_index::DeclarationMacroIndex, macro_origins::MacroSelection,
        macro_scopes::HygienicLookup,
    };
    let scan = scan(
        "fn cold(){later!();} macro_rules! later {()=>{impl S{}}}",
        2024,
    );
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    assert_eq!(
        index.lookup(scan.producers()[0].identity).unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
}

#[test]
fn cold_macro_definition_is_visible_at_adjacent_invocation_and_child_open() {
    use lrr::{
        expand::source_declarative::SourceDeclarativeDemand,
        frontend::{
            impl_index::DeclarationMacroIndex, macro_origins::MacroSelection,
            macro_scopes::HygienicLookup,
        },
    };
    for source in [
        "fn cold(){macro_rules! make{()=>{impl S{fn generated(){missing()}}}}make!();}",
        "fn cold(){macro_rules! make{()=>{impl S{fn generated(){missing()}}}}{make!();}}",
    ] {
        let scan = scan(source, 2024);
        let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
        let producer = scan.producers()[0].identity;
        let HygienicLookup::Selection(MacroSelection::Defined(found)) =
            index.lookup(producer).unwrap()
        else {
            panic!("adjacent definition is not visible")
        };
        assert_eq!(found.identity, scan.macro_definitions()[0].identity);
        let SourceDeclarativeDemand::Ready(request) = declarative_source(source)
            .declaration_demand(&index, producer, Digest([81; 32]), Digest([82; 32]))
            .unwrap()
        else {
            panic!("adjacent macro demand did not resume")
        };
        assert!(
            output_identifiers(&request.expand().unwrap())
                .iter()
                .any(|n| n == "generated")
        );
    }
    let scan = scan("fn cold(){make!();macro_rules! make{()=>{impl S{}}}}", 2024);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    assert_eq!(
        index.lookup(scan.producers()[0].identity).unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
}

#[test]
fn cold_macro_unknown_predecessor_is_input_demand_and_unused_body_stays_opaque() {
    use lrr::frontend::{impl_index::DeclarationMacroIndex, macro_scopes::HygienicLookup};
    let scan = scan(
        "macro_rules! unused {not valid rules} fn cold(){unknown!(); missing!();}",
        2024,
    );
    assert_eq!(scan.macro_definitions().len(), 1);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    assert_eq!(
        index.lookup(scan.producers()[1].identity).unwrap(),
        HygienicLookup::NeedInput(scan.producers()[0].identity)
    );
    assert_eq!(
        index.lookup(Digest([0; 32])).unwrap_err().code,
        "LRR-E-IMPL-INVENTORY"
    );
}

fn captured_source(text: &str) -> lrr::source::changes::PreparedSourceView {
    use lrr::{
        db::key::OutlineKey,
        source::changes::{
            PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
        },
    };
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([61; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: lrr::syntax::parse::ParseKey {
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
        panic!("captured source absent")
    };
    view
}
fn declarative_source(text: &str) -> lrr::expand::source_declarative::SourceDeclarative {
    lrr::expand::source_declarative::SourceDeclarative::new(captured_source(text)).unwrap()
}

fn output_identifiers(record: &lrr::expand::record::ExpansionRecord) -> Vec<String> {
    let forest = record.tokens().unwrap();
    forest_identifiers(&forest)
}
fn forest_identifiers(forest: &lrr::syntax::token_wire::TokenForest) -> Vec<String> {
    use lrr::syntax::token_wire::TokenTree;
    let mut names = Vec::new();
    let mut stack = forest.roots().iter().collect::<Vec<_>>();
    while let Some(tree) = stack.pop() {
        match tree {
            TokenTree::Group { children, .. } => stack.extend(children.iter()),
            TokenTree::Ident { text, .. } => names.push(text.clone()),
            _ => {}
        }
    }
    names
}

fn cold_expansion(
    source: &str,
) -> (
    Arc<lrr::syntax::declaration_scan::DeclarationScan>,
    lrr::expand::record::ExpansionRecord,
) {
    use lrr::{
        expand::source_declarative::SourceDeclarativeDemand,
        frontend::impl_index::DeclarationMacroIndex,
    };
    let scan = scan(source, 2024);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    let SourceDeclarativeDemand::Ready(request) = declarative_source(source)
        .declaration_demand(
            &index,
            scan.producers()[0].identity,
            Digest([81; 32]),
            Digest([82; 32]),
        )
        .unwrap()
    else {
        panic!("cold expansion unresolved")
    };
    (scan, request.expand().unwrap())
}

#[test]
fn cold_expression_output_admits_nested_impl_and_keeps_unknown_producer_pending() {
    use lrr::{
        expand::generated_declarations::GeneratedDeclarations,
        frontend::impl_index::DeclarationMacroIndex,
    };
    let (parent, record) = cold_expansion(
        "struct S; fn cold(){macro_rules! make{()=>{{#[cfg(any())] impl Disabled{} impl S{fn generated(){missing()}} later!(); 7}}} let _:Missing=make!();}",
    );
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(output.scan().impls().len(), 1);
    assert_eq!(output.scan().producers().len(), 1);
    let header = &output.scan().impls()[0];
    assert_eq!(header.scope.owner, parent.producers()[0].scope.owner);
    assert!(header.scope.owner.is_some());
    assert!(
        !lrr::syntax::ast_adapter::token_text(&header.associated[0].header).contains("missing")
    );
    assert!(output.scan().cost().disabled_subtrees > 0);
    assert_eq!(output.scan().generated_origin(), Some(output.fingerprint()));
    assert_eq!(
        DeclarationMacroIndex::new(output.scan().clone())
            .err()
            .unwrap()
            .code,
        "LRR-E-IMPL-INVENTORY"
    );
    let index = ImplHeaderIndex::from_scans(vec![parent, output.scan().clone()]).unwrap();
    assert_eq!(index.headers().len(), 1);
    assert_eq!(
        index.pending_producers().len(),
        2,
        "expansion transport is not a completion proof"
    );
}

#[test]
fn rustc_cold_statement_output_preserves_nested_producer_and_origin_rows() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let (parent, record) = cold_expansion(source);
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert!(output.text().contains("Xyzzy"));
    assert_eq!(output.scan().producers().len(), 1);
    assert_eq!(output.scan().producers()[0].path[0].text, "foo");
    assert!(output.scan().impls().is_empty());
    assert!(!output.origins().is_empty());
    for origin in output.origins() {
        assert!(
            record
                .tokens()
                .unwrap()
                .spans()
                .iter()
                .any(|s| s.key().0 == origin.span.0)
        );
    }
}

#[test]
fn generated_declaration_admission_rejects_invalid_and_wrong_slot_output() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    for source in [
        "macro_rules! make{()=>{struct S;}}fn cold(){let x=make!();}",
        "macro_rules! make{()=>{impl}}fn cold(){make!();}",
        "macro_rules! make{()=>{struct S;}}trait T{make!();}",
        "macro_rules! make{()=>{struct S;}}impl S{make!();}",
    ] {
        let (parent, record) = cold_expansion(source);
        assert_eq!(
            GeneratedDeclarations::admit(
                record.key(),
                Arc::new(record.clone()),
                1,
                &parent,
                &CfgSet::guest_default(),
                DeclarationScanLimits::default()
            )
            .err()
            .unwrap()
            .code,
            "LRR-E-MACRO-OUTPUT"
        );
    }
}

#[test]
fn generated_declaration_admission_binds_parent_cfg_slot_key_and_epoch() {
    use lrr::expand::{generated_declarations::GeneratedDeclarations, identity::GrammarSlot};
    let (parent, record) =
        cold_expansion("macro_rules! make{()=>{{impl S{} 7}}}fn cold(){let x=make!();}");
    let cfg = CfgSet::guest_default();
    let limits = DeclarationScanLimits::default();
    let admit = |record: &lrr::expand::record::ExpansionRecord, epoch, cfg: &CfgSet| {
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            epoch,
            &parent,
            cfg,
            limits,
        )
    };
    assert_eq!(
        admit(&record, 0, &cfg).err().unwrap().code,
        "LRR-E-EXPANSION-EPOCH"
    );
    let a = admit(&record, 1, &cfg).unwrap();
    let b = admit(&record, 2, &cfg).unwrap();
    assert_ne!(a.fingerprint(), b.fingerprint());
    assert_ne!(a.scan().fingerprint(), b.scan().fingerprint());
    let mut changed_cfg = cfg.clone();
    changed_cfg.atoms.insert("changed_configuration".into());
    assert_eq!(
        admit(&record, 1, &changed_cfg).err().unwrap().code,
        "LRR-E-EXPANSION-INPUT"
    );
    for variant in 0..3 {
        let mut key = record.key().clone();
        match variant {
            0 => key.invocation = Digest([0; 32]),
            1 => key.grammar = GrammarSlot::Statements,
            _ => key.call_edition = 2021,
        }
        let forged =
            lrr::expand::record::ExpansionRecord::from_tokens(key, &record.tokens().unwrap())
                .unwrap();
        assert_eq!(
            admit(&forged, 1, &cfg).err().unwrap().code,
            "LRR-E-EXPANSION-INPUT"
        );
    }
    let mut wrong_key = record.key().clone();
    wrong_key.engine_version = Digest([0; 32]);
    assert_eq!(
        GeneratedDeclarations::admit(&wrong_key, Arc::new(record), 1, &parent, &cfg, limits)
            .err()
            .unwrap()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
}

#[test]
fn generated_declaration_scan_budget_refuses_without_partial_inventory() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let (parent, record) =
        cold_expansion("macro_rules! make{()=>{{impl S{} 7}}}fn cold(){let x=make!();}");
    assert_eq!(
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits {
                impls: 0,
                ..DeclarationScanLimits::default()
            }
        )
        .err()
        .unwrap()
        .code,
        "LRR-E-DECLARATION-SCAN-LIMIT"
    );
}

#[test]
fn generated_declaration_slots_use_pinned_type_pattern_and_associated_syntax() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    for (source, expected_impls) in [
        ("macro_rules! make{()=>{impl S{}}}make!();", 1),
        ("macro_rules! make{()=>{impl S{}}}fn cold(){make!();}", 1),
        (
            "macro_rules! make{()=>{[u8; {impl S{} 0}]}}type X=make!();",
            1,
        ),
        (
            "macro_rules! make{()=>{Some(inner!())}}fn cold(){let make!()=x;}",
            0,
        ),
        ("macro_rules! make{()=>{fn made();}}trait T{make!();}", 0),
        (
            "macro_rules! make{()=>{fn made(){impl S{}}}}impl S{make!();}",
            1,
        ),
    ] {
        let (parent, record) = cold_expansion(source);
        let output = GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap();
        assert_eq!(output.scan().impls().len(), expected_impls, "{source}");
    }
}

#[test]
fn generated_nested_producers_retain_statement_and_associated_slots() {
    use lrr::expand::{generated_declarations::GeneratedDeclarations, identity::GrammarSlot};
    for (source, grammar) in [
        (
            "macro_rules! make{()=>{nested!();}}fn cold(){make!();}",
            GrammarSlot::Statements,
        ),
        (
            "macro_rules! make{()=>{nested!();}}trait T{make!();}",
            GrammarSlot::TraitItems,
        ),
        (
            "macro_rules! make{()=>{nested!();}}impl S{make!();}",
            GrammarSlot::ImplItems,
        ),
    ] {
        let (parent, record) = cold_expansion(source);
        let output = GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap();
        assert_eq!(output.scan().producers().len(), 1);
        assert_eq!(output.scan().producers()[0].grammar, grammar);
    }
}

#[test]
fn rustc_macro_named_default_keeps_associated_demand_behind_root_expansion() {
    use lrr::{
        expand::{generated_declarations::GeneratedDeclarations, identity::GrammarSlot},
        frontend::{impl_index::DeclarationMacroIndex, macro_scopes::HygienicLookup},
    };
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-named-default.rs"
    );
    let (parent, record) = cold_expansion(source);
    assert_eq!(parent.producers().len(), 2);
    assert_eq!(parent.producers()[0].grammar, GrammarSlot::Items);
    assert_eq!(parent.producers()[1].grammar, GrammarSlot::ImplItems);
    let index = DeclarationMacroIndex::new(parent.clone()).unwrap();
    assert_eq!(
        index.lookup(parent.producers()[1].identity).unwrap(),
        HygienicLookup::NeedInput(parent.producers()[0].identity)
    );
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    let names = output_identifiers(&record);
    assert!(names.iter().any(|n| n == "struct"));
    assert!(names.iter().any(|n| n == "A"));
    assert!(output.scan().impls().is_empty());
    let index = ImplHeaderIndex::from_scans(vec![parent, output.scan().clone()]).unwrap();
    assert_eq!(index.headers().len(), 1);
    assert_eq!(
        index.pending_producers().len(),
        2,
        "admission silently retired a discovery frontier"
    );
}

#[test]
fn generated_definition_and_call_captures_reexpand_actual_owned_tokens() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let (parent, record) = cold_expansion(
        "macro_rules! outer{()=>{{macro_rules! inner{()=>{impl S{fn nested(){missing()}}}} inner!(); 7}}}fn cold(){let x=outer!();}",
    );
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(output.scan().macro_definitions().len(), 1);
    assert_eq!(output.scan().producers().len(), 1);
    let definition = output
        .macro_definition(output.scan().macro_definitions()[0].identity)
        .unwrap();
    let call = output
        .macro_call(output.scan().producers()[0].identity)
        .unwrap();
    assert_eq!(definition.edition, 2024);
    assert!(
        output
            .tokens()
            .spans()
            .iter()
            .any(|s| s.key().0 == definition.name_span.0)
    );
    assert!(call.input.spans().iter().any(|s| s.key() == call.site));
    let expanded = lrr_expand_adapter::declarative::DeclarativeMacro::new(definition.body.clone())
        .unwrap()
        .expand(&call.input, call.site)
        .unwrap()
        .unwrap();
    let wire = expanded.tokens.encode().unwrap();
    let forest = lrr::syntax::token_wire::TokenForest::decode(&wire).unwrap();
    let names = forest_identifiers(&forest);
    for name in ["impl", "S", "nested", "missing"] {
        assert!(names.iter().any(|n| n == name));
    }
    assert_eq!(
        output.macro_call(Digest([0; 32])).err().unwrap().code,
        "LRR-E-EXPANSION-INPUT"
    );
    assert_eq!(
        output.macro_definition(Digest([0; 32])).err().unwrap().code,
        "LRR-E-EXPANSION-INPUT"
    );
}

#[test]
fn rustc_generated_statement_call_capture_preserves_actual_span_contexts() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let (parent, record) = cold_expansion(source);
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    let call = output
        .macro_call(output.scan().producers()[0].identity)
        .unwrap();
    assert_eq!(call.edition, 2024);
    assert!(call.path.spans().iter().all(|s| s.source.path == "main.rs"));
    assert!(call.path.contexts().iter().any(|c| c.expansion.is_some()));
    assert!(
        matches!(call.input.roots(), [lrr_expand_adapter::token_wire::TokenTree::Group {children, ..}] if children.is_empty())
    );
}

#[test]
fn generated_capture_filters_cfg_and_does_not_parse_unused_definition_bodies() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let original = scan(
        "fn cold(){#[cfg(any())] hidden!(); unknown!(fake!());}",
        2024,
    );
    assert_eq!(
        original.producers().len(),
        1,
        "source statement cfg was ignored"
    );
    let (parent, record) = cold_expansion(
        "macro_rules! outer{()=>{{#[cfg(any())] macro_rules! hidden{()=>{}} #[cfg(any())] hidden!(); macro_rules! unused{not rules} unknown!(fake!()); 7}}}fn cold(){let x=outer!();}",
    );
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(output.scan().macro_definitions().len(), 1);
    assert_eq!(output.scan().macro_definitions()[0].name, "unused");
    assert_eq!(
        output.scan().producers().len(),
        1,
        "macro payload or disabled subtree was incorrectly scanned as syntax: {:?}",
        output.scan().producers()
    );
    let definition = output
        .macro_definition(output.scan().macro_definitions()[0].identity)
        .unwrap();
    let call = output
        .macro_call(output.scan().producers()[0].identity)
        .unwrap();
    let parsed =
        lrr_expand_adapter::declarative::DeclarativeMacro::new(definition.body.clone()).unwrap();
    assert!(
        parsed.expand(&call.input, call.site).unwrap().is_err(),
        "invalid unused definition was guessed valid"
    );
}

#[test]
fn generated_call_capture_keeps_exact_arguments_and_grammar_slots() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    for source in [
        "macro_rules! outer{()=>{{inner!(13u8); 7}}}fn cold(){let x=outer!();}",
        "macro_rules! outer{()=>{Some(inner!(13u8))}}fn cold(){let outer!()=x;}",
        "macro_rules! outer{()=>{inner!(13u8);}}trait T{outer!();}",
    ] {
        let (parent, record) = cold_expansion(source);
        let output = GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap();
        let producer = &output.scan().producers()[0];
        let call = output.macro_call(producer.identity).unwrap();
        let mut stack = call.input.roots().iter().collect::<Vec<_>>();
        let mut found = false;
        while let Some(token) = stack.pop() {
            match token {
                lrr_expand_adapter::token_wire::TokenTree::Group { children, .. } => {
                    stack.extend(children)
                }
                lrr_expand_adapter::token_wire::TokenTree::Literal { token_text, .. }
                    if token_text == "13u8" =>
                {
                    found = true
                }
                _ => {}
            }
        }
        assert!(found);
        assert_eq!(call.start, producer.range.start);
        assert_eq!(call.end, producer.range.end);
    }
}

#[test]
fn generated_scalar_output_has_no_macro_capture_or_phantom_demand() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let (parent, record) = cold_expansion("macro_rules! outer{()=>{7}}fn cold(){let x=outer!();}");
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert!(output.scan().macro_definitions().is_empty());
    assert!(output.scan().producers().is_empty());
    assert!(output.metadata_bytes().unwrap() < 256 * 1024 * 1024);
}

fn generated_index(
    source: &str,
) -> (
    lrr::expand::source_declarative::SourceDeclarative,
    Arc<lrr::frontend::impl_index::GeneratedDeclarationMacroIndex>,
) {
    use lrr::{
        expand::generated_declarations::GeneratedDeclarations,
        frontend::impl_index::{
            DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
        },
    };
    let (parent, record) = cold_expansion(source);
    let source_index = Arc::new(DeclarationMacroIndex::new(parent.clone()).unwrap());
    let output = Arc::new(
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let index = Arc::new(
        GeneratedDeclarationMacroIndex::new(output, DeclarationMacroParent::Source(source_index))
            .unwrap(),
    );
    (declarative_source(source), index)
}

#[test]
fn rustc_generated_statement_resolves_original_macro_and_expands_hygienically() {
    use lrr::expand::source_declarative::SourceDeclarativeDemand;
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let (provider, index) = generated_index(source);
    let invocation = index.output().scan().producers()[0].identity;
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&index, invocation)
        .unwrap()
    else {
        panic!("original foo macro selection remained pending")
    };
    assert_eq!(
        request.key().grammar,
        lrr::expand::identity::GrammarSlot::Statements
    );
    let record = request.expand().unwrap();
    let names = output_identifiers(&record);
    for name in ["Bar", "Baz"] {
        assert!(names.iter().any(|n| n == name));
    }
    assert!(
        record
            .tokens()
            .unwrap()
            .contexts()
            .iter()
            .any(|c| c.expansion.is_some())
    );
}

#[test]
fn generated_local_definition_is_selected_with_actual_hygiene_and_impl_output() {
    use lrr::{
        expand::{
            generated_declarations::GeneratedDeclarations,
            source_declarative::SourceDeclarativeDemand,
        },
        frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup},
    };
    let source = "macro_rules! outer{()=>{{macro_rules! inner{()=>{{impl S{fn generated(){missing()}} 7}}} let x=inner!(); 7}}}fn cold(){let x=outer!();}";
    let (provider, index) = generated_index(source);
    let invocation = index.output().scan().producers()[0].identity;
    let HygienicLookup::Selection(MacroSelection::Defined(definition)) =
        index.lookup(invocation).unwrap()
    else {
        panic!("generated definition was not selected")
    };
    assert_eq!(
        definition.identity,
        index.output().scan().macro_definitions()[0].identity
    );
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&index, invocation)
        .unwrap()
    else {
        panic!("generated body capture unresolved")
    };
    let record = request.expand().unwrap();
    let child = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        index.output().scan(),
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(child.scan().impls().len(), 1);
    assert!(child.scan().producers().is_empty());
}

#[test]
fn generated_macro_lookup_keeps_source_parent_visibility_and_local_frontiers() {
    use lrr::frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup};
    for source in [
        "macro_rules! outer{()=>{{later!(); 7}}}fn cold(){let x=outer!(); macro_rules! later{()=>{}}}",
        "macro_rules! outer{()=>{{{macro_rules! hidden{()=>{}}} hidden!();7}}}fn cold(){let x=outer!();}",
    ] {
        let (_, index) = generated_index(source);
        let invocation = index.output().scan().producers()[0].identity;
        assert_eq!(
            index.lookup(invocation).unwrap(),
            HygienicLookup::Selection(MacroSelection::Absent)
        );
    }
    let (_, index) = generated_index(
        "macro_rules! outer{()=>{{unknown!(); missing!(); 7}}}fn cold(){let x=outer!();}",
    );
    let rows = index.output().scan().producers();
    assert_eq!(
        index.lookup(rows[1].identity).unwrap(),
        HygienicLookup::NeedInput(rows[0].identity)
    );
}

#[test]
fn generated_macro_index_refuses_foreign_parent_and_stale_source_provider() {
    use lrr::{
        expand::generated_declarations::GeneratedDeclarations,
        frontend::impl_index::{
            DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
        },
    };
    let source = "macro_rules! outer{()=>{{nested!();7}}}fn cold(){let x=outer!();}";
    let (parent, record) = cold_expansion(source);
    let output = Arc::new(
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let foreign = Arc::new(DeclarationMacroIndex::new(scan_at(source, 2024, "other.rs")).unwrap());
    assert_eq!(
        GeneratedDeclarationMacroIndex::new(output, DeclarationMacroParent::Source(foreign))
            .err()
            .unwrap()
            .code,
        "LRR-E-IMPL-INVENTORY"
    );
    let (_, index) = generated_index(source);
    let stale = declarative_source(&source.replace("nested", "another"));
    assert_eq!(
        stale
            .generated_declaration_demand(&index, index.output().scan().producers()[0].identity)
            .err()
            .unwrap()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
}

#[test]
fn generated_macro_ancestry_supports_second_level_original_definition_lookup() {
    use lrr::{
        expand::{
            generated_declarations::GeneratedDeclarations,
            source_declarative::SourceDeclarativeDemand,
        },
        frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    };
    let source = "macro_rules! leaf{()=>{{impl S{} 7}}}macro_rules! outer{()=>{{macro_rules! inner{()=>{{let x=leaf!();7}}}let x=inner!();7}}}fn cold(){let x=outer!();}";
    let (provider, first) = generated_index(source);
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&first, first.output().scan().producers()[0].identity)
        .unwrap()
    else {
        panic!("first child pending")
    };
    let record = request.expand().unwrap();
    let output = Arc::new(
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            first.output().scan(),
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let second =
        GeneratedDeclarationMacroIndex::new(output, DeclarationMacroParent::Generated(first))
            .unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&second, second.output().scan().producers()[0].identity)
        .unwrap()
    else {
        panic!("source leaf not visible through actual ancestry")
    };
    let record = request.expand().unwrap();
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        second.output().scan(),
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(output.scan().impls().len(), 1);
}

#[test]
fn rustc_cold_statement_demand_expands_original_tokens_with_pinned_engine() {
    use lrr::{
        expand::source_declarative::SourceDeclarativeDemand,
        frontend::impl_index::DeclarationMacroIndex,
    };
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let scan = scan(source, 2024);
    let provider = declarative_source(source);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            &index,
            scan.producers()[0].identity,
            Digest([81; 32]),
            Digest([82; 32]),
        )
        .unwrap()
    else {
        panic!("cold declaration demand unresolved")
    };
    assert_eq!(
        request.key().grammar,
        lrr::expand::identity::GrammarSlot::Statements
    );
    let output = request.expand().unwrap();
    let names = output_identifiers(&output);
    assert!(names.iter().any(|n| n == "Xyzzy"));
    assert!(
        names.iter().any(|n| n == "foo"),
        "nested producer was discarded"
    );
    assert!(
        output
            .tokens()
            .unwrap()
            .spans()
            .iter()
            .all(|s| s.source.path == "main.rs")
    );
    assert!(
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), Arc::new(output), 1)
            .is_err(),
        "statement output must not masquerade as an item-only admission"
    );
}

#[test]
fn cold_expression_macro_expands_impl_tokens_without_executing_body() {
    use lrr::{
        expand::source_declarative::SourceDeclarativeDemand,
        frontend::impl_index::DeclarationMacroIndex,
    };
    let source = "struct S; fn cold(){macro_rules! make{($name:ident)=>{{impl S{fn $name(){missing()}} 7}}} let _: Missing=make!(generated);}";
    let scan = scan(source, 2024);
    assert!(scan.impls().is_empty());
    assert_eq!(
        scan.producers()[0].grammar,
        lrr::expand::identity::GrammarSlot::Expression
    );
    let provider = declarative_source(source);
    let index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            &index,
            scan.producers()[0].identity,
            Digest([81; 32]),
            Digest([82; 32]),
        )
        .unwrap()
    else {
        panic!("expression demand unresolved")
    };
    let output = request.expand().unwrap();
    let names = output_identifiers(&output);
    for expected in ["impl", "S", "generated", "missing"] {
        assert!(names.iter().any(|n| n == expected));
    }
    assert_eq!(
        output.key().grammar,
        lrr::expand::identity::GrammarSlot::Expression
    );
}

#[test]
fn cold_declaration_demand_refuses_foreign_parse_path_and_cfg() {
    use lrr::frontend::impl_index::DeclarationMacroIndex;
    let source = "macro_rules! make{()=>{impl S{}}} fn cold(){make!();}";
    let provider = declarative_source(source);
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("changed_configuration".into());
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let cfg_scan = Arc::new(
        parsed
            .scan_declarations(
                SourcePath::new("main.rs").unwrap(),
                ScopeKey {
                    crate_key: Digest([61; 32]),
                    module_path: vec![],
                    owner: None,
                },
                &cfg,
                DeclarationScanLimits::default(),
            )
            .unwrap(),
    );
    for scan in [
        scan_at(source, 2024, "other.rs"),
        scan(&source.replace("impl S", "impl T"), 2024),
        cfg_scan,
    ] {
        let invocation = scan.producers()[0].identity;
        let index = DeclarationMacroIndex::new(scan).unwrap();
        assert_eq!(
            provider
                .declaration_demand(&index, invocation, Digest([81; 32]), Digest([82; 32]))
                .err()
                .unwrap()
                .code,
            "LRR-E-EXPANSION-INPUT"
        );
    }
}

#[test]
fn cold_macro_definition_budget_and_disabled_scopes_refuse_or_prune() {
    let source = "#[cfg(any())] fn disabled(){macro_rules! hidden{()=>{impl S{}}} hidden!();} fn cold(){macro_rules! unused{not rules}}";
    let scan = scan(source, 2024);
    assert_eq!(scan.macro_definitions().len(), 1);
    assert_eq!(scan.macro_definitions()[0].name, "unused");
    assert!(scan.producers().is_empty());
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert_eq!(
        parsed
            .scan_declarations(
                SourcePath::new("main.rs").unwrap(),
                scan.scope().clone(),
                &CfgSet::guest_default(),
                DeclarationScanLimits {
                    producers: 0,
                    ..DeclarationScanLimits::default()
                }
            )
            .err()
            .unwrap()
            .code,
        "LRR-E-DECLARATION-SCAN-LIMIT"
    );
}

#[test]
fn cold_declaration_scan_preserves_all_macro_grammar_slots() {
    use lrr::expand::identity::GrammarSlot;
    let scan = scan(
        "make!(); trait T{make!();} impl S{make!();} type X=make!(); fn cold(){make!(); let x=make!(); let make!()=x;}",
        2024,
    );
    assert_eq!(
        scan.producers()
            .iter()
            .map(|p| p.grammar)
            .collect::<Vec<_>>(),
        [
            GrammarSlot::Items,
            GrammarSlot::TraitItems,
            GrammarSlot::ImplItems,
            GrammarSlot::Type,
            GrammarSlot::Statements,
            GrammarSlot::Expression,
            GrammarSlot::Pattern
        ]
    );
}

#[test]
fn rustc_local_impls_are_discovered_in_cold_functions_and_anonymous_consts() {
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/lint/non-local-defs/local.rs"
    );
    let scan = scan(source, 2021);
    assert_eq!(scan.impls().len(), 10);
    assert_eq!(scan.parse().profile.edition, 2021);
    assert!(scan.cost().visited_nodes > 0);
    let index = ImplHeaderIndex::from_scans(vec![scan.clone()]).unwrap();
    assert_eq!(index.headers().len(), 10);
    assert_eq!(index.pending_producers().len(), scan.producers().len());
    assert!(
        !index.pending_producers().is_empty(),
        "cold todo invocations were guessed inert"
    );
    for header in scan.impls() {
        assert!(!lrr::syntax::ast_adapter::token_text(&header.header).contains("todo"));
    }
}

#[test]
fn rustc_impls_in_signature_and_array_const_expressions_remain_visible() {
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/lint/non-local-defs/weird-exprs.rs"
    );
    let scan = scan(source, 2021);
    assert_eq!(scan.impls().len(), 6);
    assert!(
        scan.impls()
            .iter()
            .any(|h| lrr::syntax::ast_adapter::token_text(&h.header).contains("* mut Test"))
    );
    assert!(
        scan.impls()
            .iter()
            .any(|h| lrr::syntax::ast_adapter::token_text(&h.header).contains("Test , Test"))
    );
    assert_eq!(
        ImplHeaderIndex::from_scans(vec![scan])
            .unwrap()
            .headers()
            .len(),
        6
    );
}

#[test]
fn declaration_scan_prunes_disabled_subtrees_and_keeps_closure_async_producers() {
    let source = "struct S; #[cfg(any())] fn disabled(){impl S {fn hidden(){}} hidden!();} fn cold(){let _: Missing = absent(); let f=||{impl S {fn closure(){}} producer!();}; let a=async{impl S {fn asynchronous(){}}}; let _=stringify!(impl S {fn fake(){}});} ";
    let scan = scan(source, 2024);
    assert_eq!(scan.impls().len(), 2);
    let headers = scan
        .impls()
        .iter()
        .map(|h| lrr::syntax::ast_adapter::token_text(&h.header))
        .collect::<Vec<_>>();
    assert!(
        headers
            .iter()
            .all(|h| !h.contains("hidden") && !h.contains("fake"))
    );
    assert_eq!(
        scan.producers().len(),
        2,
        "macro token trees must remain opaque, including stringify until exact identity is resolved"
    );
    assert!(scan.cost().disabled_subtrees > 0);
}

#[test]
fn impl_scan_header_identity_ignores_method_bodies_but_source_and_member_headers_invalidate() {
    let a = scan(
        "struct S; fn cold(){impl S {fn member()->u8{missing()}}}",
        2024,
    );
    let b = scan(
        "struct S; fn cold(){impl S {fn member()->u8{another_missing()}}}",
        2024,
    );
    let c = scan(
        "struct S; fn cold(){impl S {fn member()->u16{another_missing()}}}",
        2024,
    );
    assert_eq!(a.impls()[0].header_identity, b.impls()[0].header_identity);
    assert_ne!(a.fingerprint(), b.fingerprint());
    assert_ne!(a.impls()[0].header_identity, c.impls()[0].header_identity);
    assert_eq!(
        ImplHeaderIndex::from_scans(vec![a.clone()])
            .unwrap()
            .fingerprint(),
        ImplHeaderIndex::from_scans(vec![a]).unwrap().fingerprint()
    );
}

#[test]
fn declaration_scan_limit_refuses_without_a_partial_impl_inventory() {
    let blob = SourceBlob::new(b"fn cold(){impl S{} impl T{}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let scope = ScopeKey {
        crate_key: Digest([62; 32]),
        module_path: vec![],
        owner: None,
    };
    let error = parsed
        .scan_declarations(
            SourcePath::new("main.rs").unwrap(),
            scope,
            &CfgSet::guest_default(),
            DeclarationScanLimits {
                impls: 1,
                ..DeclarationScanLimits::default()
            },
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "LRR-E-DECLARATION-SCAN-LIMIT");
}
#[test]
fn impl_inventory_retains_nested_module_and_function_context_and_refuses_duplicate_source() {
    let source = "struct S;mod first{fn cold(){impl super::S{fn value()->u8{missing()}}}}mod second{fn cold(){impl super::S{fn value()->u8{missing()}}}}";
    let scan = scan(source, 2024);
    assert_eq!(scan.impls().len(), 2);
    assert_eq!(scan.impls()[0].scope.module_path, ["first"]);
    assert_eq!(scan.impls()[1].scope.module_path, ["second"]);
    assert!(
        scan.impls()
            .iter()
            .all(|h| h.scope.owner.is_some() && !h.containers.is_empty())
    );
    assert_ne!(
        scan.impls()[0].header_identity,
        scan.impls()[1].header_identity
    );
    assert_eq!(
        ImplHeaderIndex::from_scans(vec![scan.clone(), scan])
            .err()
            .unwrap()
            .code,
        "LRR-E-IMPL-INVENTORY"
    );
}
#[test]
fn impl_inventory_distinguishes_identical_files_and_is_independent_of_source_order() {
    let text = "fn cold(){impl S {fn member(){missing()}}}";
    let a = scan_at(text, 2024, "first.rs");
    let b = scan_at(text, 2024, "second.rs");
    assert_eq!(a.impls()[0].header_identity, b.impls()[0].header_identity);
    assert_ne!(a.impls()[0].identity, b.impls()[0].identity);
    let first = ImplHeaderIndex::from_scans(vec![a.clone(), b.clone()]).unwrap();
    let second = ImplHeaderIndex::from_scans(vec![b, a]).unwrap();
    assert_eq!(first.headers().len(), 2);
    assert_eq!(first.fingerprint(), second.fingerprint());
}
#[test]
fn declaration_scan_syntax_failure_is_not_an_empty_impl_inventory() {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"fn cold(){impl S {fn broken( }}".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(!parsed.errors.is_empty());
    let error = parsed
        .scan_declarations(
            SourcePath::new("main.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([63; 32]),
                module_path: vec![],
                owner: None,
            },
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "LRR-E-SOURCE-PARSE");
}
#[test]
fn impl_header_in_a_signature_const_does_not_inherit_nested_method_body_text() {
    let a = scan(
        "struct S; fn cold(_: [u8; {impl S {fn member(){missing()}} 1}]){}",
        2024,
    );
    let b = scan(
        "struct S; fn cold(_: [u8; {impl S {fn member(){another_missing()}} 1}]){}",
        2024,
    );
    assert_eq!(a.impls().len(), 1);
    assert_eq!(b.impls().len(), 1);
    assert_eq!(a.impls()[0].scope.owner, b.impls()[0].scope.owner);
    assert_eq!(a.impls()[0].header_identity, b.impls()[0].header_identity);
    assert_ne!(a.fingerprint(), b.fingerprint());
}

#[test]
fn recursive_adoption_keeps_parent_pending_until_nested_producer_finishes() {
    use lrr::frontend::impl_index::DeclarationMacroIndex;
    let source = "macro_rules! leaf{()=>{{impl S{} 7}}}macro_rules! outer{()=>{{let x=leaf!();7}}}fn cold(){let x=outer!();missing();}";
    let (_, child) = generated_index(source);
    let mut parent = DeclarationMacroIndex::new(scan(source, 2024)).unwrap();
    let key = child.output().record().key();
    let before = parent.lookup(key.invocation).unwrap();
    assert!(!child.is_complete());
    assert!(parent.adopt_completed_child(key, &child).is_err());
    assert_eq!(parent.lookup(key.invocation).unwrap(), before);
}

#[test]
fn completed_statement_expansion_publishes_generated_macro_to_following_sibling() {
    use lrr::{
        expand::{
            generated_declarations::GeneratedDeclarations,
            source_declarative::SourceDeclarativeDemand,
        },
        frontend::impl_index::{
            DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
        },
    };
    let source = "macro_rules! outer{()=>{macro_rules! next{()=>{{impl S{} 7}}}}}fn cold(){outer!();let x=next!();missing();}";
    let scan = scan(source, 2024);
    let provider = declarative_source(source);
    let mut parent = DeclarationMacroIndex::new(scan.clone()).unwrap();
    let invocation = scan.producers()[0].identity;
    let sibling = scan.producers()[1].identity;
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(&parent, invocation, Digest([71; 32]), Digest([72; 32]))
        .unwrap()
    else {
        panic!("outer pending")
    };
    let record = Arc::new(request.expand().unwrap());
    let child = GeneratedDeclarationMacroIndex::new(
        Arc::new(
            GeneratedDeclarations::admit(
                request.key(),
                record,
                1,
                &scan,
                &CfgSet::guest_default(),
                DeclarationScanLimits::default(),
            )
            .unwrap(),
        ),
        DeclarationMacroParent::Source(Arc::new(parent.clone())),
    )
    .unwrap();
    assert!(child.is_complete());
    assert!(
        matches!(provider.declaration_demand(&parent,sibling,Digest([71;32]),Digest([72;32])).unwrap(),SourceDeclarativeDemand::NeedInput(id) if id==invocation)
    );
    parent.adopt_completed_child(request.key(), &child).unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(&parent, sibling, Digest([71; 32]), Digest([72; 32]))
        .unwrap()
    else {
        panic!("published next unavailable")
    };
    let record = Arc::new(request.expand().unwrap());
    let output = GeneratedDeclarations::admit(
        request.key(),
        record,
        1,
        &scan,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert_eq!(output.scan().impls().len(), 1);
}

#[test]
fn completed_expression_block_keeps_generated_macro_out_of_outer_sibling() {
    use lrr::{
        expand::source_declarative::SourceDeclarativeDemand,
        frontend::impl_index::DeclarationMacroIndex,
    };
    let source = "macro_rules! outer{()=>{{macro_rules! hidden{()=>{7}}7}}}fn cold(){let x=outer!();let y=hidden!();}";
    let (provider, child) = generated_index(source);
    let mut parent = DeclarationMacroIndex::new(scan(source, 2024)).unwrap();
    parent
        .adopt_completed_child(child.output().record().key(), &child)
        .unwrap();
    let sibling = parent.scan().producers()[1].identity;
    assert!(
        matches!(provider.declaration_demand(&parent,sibling,Digest([71;32]),Digest([72;32])).unwrap(),SourceDeclarativeDemand::NeedInput(id) if id==sibling)
    );
}

#[test]
fn completed_child_adoption_rejects_wrong_key_and_duplicate_reply_atomically() {
    use lrr::frontend::impl_index::DeclarationMacroIndex;
    let source = "macro_rules! outer{()=>{}}fn cold(){outer!();missing();}";
    let (_, child) = generated_index(source);
    let mut parent = DeclarationMacroIndex::new(scan(source, 2024)).unwrap();
    let key = child.output().record().key();
    let mut wrong = key.clone();
    wrong.engine_version = Digest([99; 32]);
    assert!(parent.adopt_completed_child(&wrong, &child).is_err());
    assert!(!parent.is_complete());
    parent.adopt_completed_child(key, &child).unwrap();
    assert!(parent.is_complete());
    assert!(parent.adopt_completed_child(key, &child).is_err());
    assert!(parent.is_complete());
}

#[test]
fn recursive_generated_adoption_releases_sibling_and_preserves_real_body_owner() {
    use lrr::{
        expand::{
            generated_declarations::GeneratedDeclarations,
            source_declarative::SourceDeclarativeDemand,
        },
        frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    };
    let source = "macro_rules! outer{()=>{{macro_rules! make{()=>{macro_rules! next{()=>{{impl S{} 7}}}}}make!();let x=next!();7}}}fn cold(){let x=outer!();missing();}";
    let (provider, first) = generated_index(source);
    let mut first = (*first).clone();
    let sibling = first.output().scan().producers()[1].identity;
    let invocation = first.output().scan().producers()[0].identity;
    assert!(
        matches!(provider.generated_declaration_demand(&first,sibling).unwrap(),SourceDeclarativeDemand::NeedInput(id) if id==invocation)
    );
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&first, invocation)
        .unwrap()
    else {
        panic!("make pending")
    };
    let record = Arc::new(request.expand().unwrap());
    let output = Arc::new(
        GeneratedDeclarations::admit(
            request.key(),
            record,
            1,
            first.output().scan(),
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let child = GeneratedDeclarationMacroIndex::new(
        output,
        DeclarationMacroParent::Generated(Arc::new(first.clone())),
    )
    .unwrap();
    first.adopt_completed_child(request.key(), &child).unwrap();
    assert!(!first.is_complete());
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&first, sibling)
        .unwrap()
    else {
        panic!("published generated sibling pending")
    };
    let record = Arc::new(request.expand().unwrap());
    let output = Arc::new(
        GeneratedDeclarations::admit(
            request.key(),
            record,
            1,
            first.output().scan(),
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(output.scan().impls().len(), 1);
    let child = GeneratedDeclarationMacroIndex::new(
        output,
        DeclarationMacroParent::Generated(Arc::new(first.clone())),
    )
    .unwrap();
    first.adopt_completed_child(request.key(), &child).unwrap();
    assert!(first.is_complete());
}

#[test]
fn rustc_nested_statement_adoption_finishes_child_before_source_frontier() {
    use lrr::{
        expand::{
            generated_declarations::GeneratedDeclarations,
            source_declarative::SourceDeclarativeDemand,
        },
        frontend::impl_index::{
            DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
        },
    };
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let (provider, first) = generated_index(source);
    let mut source_index = DeclarationMacroIndex::new(scan(source, 2024)).unwrap();
    let mut first = (*first).clone();
    let root_key = first.output().record().key().clone();
    assert!(
        source_index
            .adopt_completed_child(&root_key, &first)
            .is_err()
    );
    assert!(!source_index.is_complete());
    let invocation = first.output().scan().producers()[0].identity;
    let SourceDeclarativeDemand::Ready(request) = provider
        .generated_declaration_demand(&first, invocation)
        .unwrap()
    else {
        panic!("original foo pending")
    };
    let record = Arc::new(request.expand().unwrap());
    let names = output_identifiers(&record);
    for name in ["Bar", "Baz"] {
        assert!(names.iter().any(|n| n == name));
    }
    let output = Arc::new(
        GeneratedDeclarations::admit(
            request.key(),
            record,
            1,
            first.output().scan(),
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let child = GeneratedDeclarationMacroIndex::new(
        output,
        DeclarationMacroParent::Generated(Arc::new(first.clone())),
    )
    .unwrap();
    first.adopt_completed_child(request.key(), &child).unwrap();
    assert!(first.is_complete());
    source_index
        .adopt_completed_child(&root_key, &first)
        .unwrap();
    assert!(source_index.is_complete());
}

fn declaration_fragment(
    source: &str,
) -> (
    lrr::expand::source_declarative::SourceDeclarative,
    lrr::expand::declaration_fragment::DeclarationFragment,
    lrr::expand::budget::SharedExpansionBudget,
) {
    use lrr::{
        expand::{
            budget::SharedExpansionBudget,
            declaration_fragment::{DeclarationFragment, DeclarationFragmentInput},
        },
        frontend::impl_index::DeclarationMacroIndex,
    };
    let provider = declarative_source(source);
    let scan = scan(source, 2024);
    let invocation = scan.producers()[0].identity;
    let parent = Arc::new(DeclarationMacroIndex::new(scan).unwrap());
    let budget =
        SharedExpansionBudget::new(Digest([91; 32]), Digest([92; 32]), 1, 2_000_000).unwrap();
    let fragment = DeclarationFragment::new(
        &provider,
        parent,
        DeclarationFragmentInput {
            invocation,
            request: Digest([91; 32]),
            graph: Digest([92; 32]),
            compile_env: Digest([71; 32]),
            engine: Digest([72; 32]),
            epoch: 1,
            cfg: CfgSet::guest_default(),
        },
        budget.clone(),
    )
    .unwrap();
    (provider, fragment, budget)
}

#[test]
fn recursive_declaration_fragment_suspends_without_executing_and_adopts_real_impls() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let source = "macro_rules! leaf{()=>{{impl S{} 7}}}macro_rules! outer{()=>{{let x=leaf!();7}}}fn cold(){let x=outer!();missing();}";
    let (provider, mut fragment, budget) = declaration_fragment(source);
    let DeclarationFragmentStep::NeedExpansion(first) = fragment.poll(&provider).unwrap() else {
        panic!("root demand absent")
    };
    let DeclarationFragmentStep::NeedExpansion(repeat) = fragment.poll(&provider).unwrap() else {
        panic!("root demand changed")
    };
    assert!(Arc::ptr_eq(&first, &repeat));
    assert_eq!(budget.totals().unwrap().outputs, 0);
    fragment.admit(Arc::new(first.expand().unwrap())).unwrap();
    let DeclarationFragmentStep::NeedExpansion(second) = fragment.poll(&provider).unwrap() else {
        panic!("child demand absent")
    };
    assert_ne!(first.key(), second.key());
    fragment.admit(Arc::new(second.expand().unwrap())).unwrap();
    let DeclarationFragmentStep::Complete(output) = fragment.poll(&provider).unwrap() else {
        panic!("actual subtree incomplete")
    };
    assert_eq!(output.inventory().headers().len(), 1);
    assert_eq!(output.records().len(), 2);
    assert!(output.index().is_complete());
    assert_eq!(budget.totals().unwrap().outputs, 2);
}

#[test]
fn recursive_declaration_fragment_keeps_unknown_child_as_input_demand() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let (provider, mut fragment, budget) = declaration_fragment(
        "macro_rules! outer{()=>{{let x=unknown!();7}}}fn cold(){let x=outer!();}",
    );
    let DeclarationFragmentStep::NeedExpansion(request) = fragment.poll(&provider).unwrap() else {
        panic!("root demand absent")
    };
    fragment.admit(Arc::new(request.expand().unwrap())).unwrap();
    let DeclarationFragmentStep::NeedInput(first) = fragment.poll(&provider).unwrap() else {
        panic!("unknown child completed")
    };
    let DeclarationFragmentStep::NeedInput(second) = fragment.poll(&provider).unwrap() else {
        panic!("unknown child disappeared")
    };
    assert_eq!(first, second);
    assert_eq!(budget.totals().unwrap().outputs, 1);
}

#[test]
fn recursive_declaration_fragment_refuses_foreign_reply_without_accounting_it() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let (provider, mut fragment, budget) =
        declaration_fragment("macro_rules! outer{()=>{7}}fn cold(){let x=outer!();}");
    let DeclarationFragmentStep::NeedExpansion(request) = fragment.poll(&provider).unwrap() else {
        panic!("demand absent")
    };
    let (_, foreign) = cold_expansion("macro_rules! other{()=>{9}}fn cold(){let x=other!();}");
    assert_eq!(
        fragment.admit(Arc::new(foreign)).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    let DeclarationFragmentStep::NeedExpansion(repeat) = fragment.poll(&provider).unwrap() else {
        panic!("bad reply retired request")
    };
    assert!(Arc::ptr_eq(&request, &repeat));
    assert_eq!(budget.totals().unwrap().outputs, 0);
}

#[test]
fn recursive_declaration_fragment_rejects_malformed_output_before_commit() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let (provider, mut fragment, budget) =
        declaration_fragment("macro_rules! outer{()=>{{let =;}}}fn cold(){let x=outer!();}");
    let DeclarationFragmentStep::NeedExpansion(request) = fragment.poll(&provider).unwrap() else {
        panic!("demand absent")
    };
    assert!(fragment.admit(Arc::new(request.expand().unwrap())).is_err());
    assert!(matches!(
        fragment.poll(&provider).unwrap(),
        DeclarationFragmentStep::NeedExpansion(_)
    ));
    assert_eq!(budget.totals().unwrap().outputs, 0);
}

#[test]
fn recursive_declaration_fragment_refuses_foreign_run_budget() {
    use lrr::{
        expand::{
            budget::SharedExpansionBudget,
            declaration_fragment::{DeclarationFragment, DeclarationFragmentInput},
        },
        frontend::impl_index::DeclarationMacroIndex,
    };
    let source = "macro_rules! outer{()=>{7}}fn cold(){let x=outer!();}";
    let provider = declarative_source(source);
    let scan = scan(source, 2024);
    let invocation = scan.producers()[0].identity;
    let parent = Arc::new(DeclarationMacroIndex::new(scan).unwrap());
    let budget =
        SharedExpansionBudget::new(Digest([93; 32]), Digest([92; 32]), 1, 2_000_000).unwrap();
    let error = DeclarationFragment::new(
        &provider,
        parent,
        DeclarationFragmentInput {
            invocation,
            request: Digest([91; 32]),
            graph: Digest([92; 32]),
            compile_env: Digest([71; 32]),
            engine: Digest([72; 32]),
            epoch: 1,
            cfg: CfgSet::guest_default(),
        },
        budget.clone(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
    assert_eq!(budget.totals().unwrap().outputs, 0);
}

#[test]
fn rustc_nested_statement_fragment_adopts_every_original_child() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    let (provider, mut fragment, budget) = declaration_fragment(source);
    let mut names = Vec::new();
    let mut invocations = Vec::new();
    loop {
        assert!(invocations.len() <= 2, "unexpected extra fixture producers");
        match fragment.poll(&provider).unwrap() {
            DeclarationFragmentStep::NeedAttributeSelection(_) => {
                panic!("unexpected procedural attribute selection in declarative fixture")
            }
            DeclarationFragmentStep::NeedProceduralSelection(_) => {
                panic!("unexpected procedural selection")
            }
            DeclarationFragmentStep::NeedExpansion(request) => {
                let record = Arc::new(request.expand().unwrap());
                names.extend(output_identifiers(&record));
                invocations.push(request.key().invocation);
                fragment.admit(record).unwrap();
            }
            DeclarationFragmentStep::NeedInput(input) => {
                panic!("original fixture producer unresolved: {input:?}")
            }
            DeclarationFragmentStep::Complete(output) => {
                assert!(output.index().is_complete());
                assert_eq!(output.records().len(), 2);
                break;
            }
        }
    }
    assert_ne!(invocations[0], invocations[1]);
    for name in ["Xyzzy", "Bar", "Baz"] {
        assert!(names.iter().any(|n| n == name));
    }
    assert_eq!(budget.totals().unwrap().outputs, 2);
}

#[test]
fn cold_raw_keyword_macro_preserves_source_spelling_and_semantic_resolution() {
    use lrr::frontend::{
        impl_index::DeclarationMacroIndex, macro_origins::MacroSelection,
        macro_scopes::HygienicLookup,
    };
    let inventory = scan("macro_rules! r#gen{()=>{true}}fn cold(){r#gen!();}", 2024);
    assert_eq!(inventory.macro_definitions()[0].name, "r#gen");
    let index = DeclarationMacroIndex::new(inventory.clone()).unwrap();
    assert!(
        matches!(index.lookup(inventory.producers()[0].identity).unwrap(),HygienicLookup::Selection(MacroSelection::Defined(d)) if d.identity==inventory.macro_definitions()[0].identity)
    );
}

#[test]
fn generated_intrinsic_arguments_use_owned_emitted_parser_ranges() {
    let (_, index) = generated_index(
        r#"macro_rules! outer{()=>{{assert!({impl S{}missing();true},"{}",other());true}}}fn cold(){outer!();}"#,
    );
    let producer = index
        .output()
        .scan()
        .producers()
        .iter()
        .find(|p| p.path[0].text == "assert")
        .unwrap();
    let arguments = index
        .output()
        .runtime_argument_tokens(producer.identity)
        .unwrap();
    assert_eq!(arguments.len(), 3);
    let first = lrr_expand_adapter::declaration_output::materialize(
        &arguments[0],
        lrr_expand_adapter::declaration_output::DeclarationGrammar::Expression,
    )
    .unwrap();
    assert!(first.syntax.iter().any(|event|matches!(event,lrr_expand_adapter::item_output::SyntaxEvent::Token{kind:_,text} if text=="impl")));
    assert!(
        arguments
            .iter()
            .all(|argument| argument.spans().iter().all(|span| index
                .output()
                .tokens()
                .spans()
                .iter()
                .any(|source| source.key().0 == span.key().0)))
    );
}

#[test]
fn generated_procedural_input_keeps_emitting_record_tokens_and_exact_slot() {
    let text =
        "macro_rules! make{()=>{{tool::identity!({impl S{}true});}}}fn cold(){make!();missing();}";
    let (provider, index) = generated_index(text);
    let invocation = index.output().scan().producers()[0].identity;
    let input = provider
        .generated_function_input(&index, invocation, std::slice::from_ref(&provider))
        .unwrap();
    assert_eq!(
        input.grammar(),
        lrr::expand::identity::GrammarSlot::Statements
    );
    assert_eq!(
        input.parent_origin().0,
        index.output().record().key().fingerprint().0
    );
    assert!(
        input
            .arguments()
            .contexts()
            .iter()
            .any(|c| c.expansion.is_some())
    );
    assert!(
        provider
            .generated_function_input(&index, Digest([255; 32]), std::slice::from_ref(&provider))
            .is_err()
    );
    let foreign = declarative_source(&text.replace("true", "false"));
    assert!(
        foreign
            .generated_function_input(&index, invocation, std::slice::from_ref(&foreign))
            .is_err()
    );
}

#[test]
fn rustc_nested_macro_rules_auxiliary_keeps_generated_definition_body_opaque() {
    use lrr::expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep};
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/nested-macro-rules.rs"
    );
    let view = captured_source(text);
    let mut work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Type,
        Digest([83; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([81; 32]), Digest([82; 32]))
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("official outer macro demand absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    let mut records = work.sealed_expansions().unwrap();
    assert_eq!(records.len(), 1);
    let names = output_identifiers(records.next().unwrap());
    assert!(names.iter().any(|name| name == "inner_macro"));
    assert!(names.iter().any(|name| name == "FirstStruct"));
    assert!(work.impl_header_inventory().unwrap().headers().is_empty());
    println!(
        "{}",
        serde_json::json!({"upstream_case":"tests/ui/proc-macro/nested-macro-rules.rs","component":"unchanged auxiliary expansion and quoted definition body under LRR2024","full_fixture_passed":false,"procedural_execution":false})
    );
}

#[test]
fn generated_item_owned_declaration_facet_keeps_cold_producers_and_source_spans() {
    use lrr::expand::generated_items::GeneratedItems;
    let text = "macro_rules! outer{()=>{struct Global;fn cold(){macro_rules! inner{()=>{{impl Global{}true}}}inner!();let wrong:i32=\"cold\";missing();}}}outer!();";
    let view = captured_source(text);
    let provider = declarative_source(text);
    let invocation = view.outline().item_invocations[0].item_input;
    let lrr::expand::source_declarative::SourceDeclarativeDemand::Ready(request) = provider
        .demand(invocation, Digest([62; 32]), Digest([63; 32]))
        .unwrap()
    else {
        panic!("item source demand absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let items = GeneratedItems::admit(record.key(), record.clone(), 1).unwrap();
    let facet = items
        .declaration_facet(
            &request,
            &view,
            &CfgSet::guest_default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        facet.record().descriptor().fingerprint(),
        record.descriptor().fingerprint()
    );
    assert_eq!(facet.scan().macro_definitions().len(), 1);
    assert_eq!(facet.scan().producers().len(), 1);
    assert_eq!(
        facet.scan().producers()[0].grammar,
        lrr::expand::identity::GrammarSlot::Statements
    );
    let call = facet
        .macro_call(facet.scan().producers()[0].identity)
        .unwrap();
    assert!(
        items
            .tokens()
            .spans()
            .iter()
            .any(|span| span.key().0 == call.site.0)
    );
    assert!(
        facet.scan().impls().is_empty(),
        "quoted macro body must stay opaque"
    );
    assert!(
        items
            .declaration_facet(
                &request,
                &captured_source(text.replace("cold", "other").as_str()),
                &CfgSet::guest_default(),
                Default::default()
            )
            .is_err()
    );
}

#[test]
fn item_output_declaration_subtree_expands_cold_impl_using_real_lexical_attachment() {
    use lrr::expand::{
        budget::SharedExpansionBudget,
        declaration_fragment::{
            DeclarationFragment, DeclarationFragmentInput, DeclarationFragmentStep,
        },
        generated_items::GeneratedItems,
        source_declarative::SourceDeclarativeDemand,
    };
    use lrr::frontend::impl_index::DeclarationMacroIndex;
    let text = "struct Global;macro_rules! inner{()=>{{impl Global{fn marker(){missing();}}true}}}macro_rules! outer{()=>{fn cold(){inner!();let wrong:i32=\"cold\";missing();}}}outer!();";
    let view = captured_source(text);
    let provider = declarative_source(text);
    let invocation = view.outline().item_invocations[0].item_input;
    let compile_env = Digest([62; 32]);
    let engine = Digest([63; 32]);
    let SourceDeclarativeDemand::Ready(request) =
        provider.demand(invocation, compile_env, engine).unwrap()
    else {
        panic!("item demand absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let items = GeneratedItems::admit(record.key(), record.clone(), 1).unwrap();
    let output = Arc::new(
        items
            .declaration_facet(
                &request,
                &view,
                &CfgSet::guest_default(),
                Default::default(),
            )
            .unwrap(),
    );
    let parent = Arc::new(DeclarationMacroIndex::new(scan(text, 2024)).unwrap());
    let request_id = Digest([64; 32]);
    let graph = Digest([65; 32]);
    let budget = SharedExpansionBudget::new(request_id, graph, 1, 2_000_000).unwrap();
    let mut subtree = DeclarationFragment::from_item_output(
        parent,
        DeclarationFragmentInput {
            invocation,
            request: request_id,
            graph,
            compile_env,
            engine,
            epoch: 1,
            cfg: CfgSet::guest_default(),
        },
        budget.clone(),
        output,
    )
    .unwrap();
    assert_eq!(budget.totals().unwrap().outputs, 1);
    let DeclarationFragmentStep::NeedExpansion(child) = subtree.poll(&provider).unwrap() else {
        panic!("cold generated child absent")
    };
    assert_eq!(
        child.key().grammar,
        lrr::expand::identity::GrammarSlot::Statements
    );
    subtree.admit(Arc::new(child.expand().unwrap())).unwrap();
    let DeclarationFragmentStep::Complete(completed) = subtree.poll(&provider).unwrap() else {
        panic!("cold declaration subtree incomplete")
    };
    assert_eq!(completed.inventory().headers().len(), 1);
    assert_eq!(completed.records().len(), 2);
    assert_eq!(
        completed.records()[0].descriptor().fingerprint(),
        record.descriptor().fingerprint()
    );
    assert_eq!(budget.totals().unwrap().outputs, 2);
}

#[test]
fn cold_import_headers_retain_block_ancestry_and_grouped_aliases() {
    use lrr::syntax::imports::ImportKind;
    let scan = scan(
        "use host::root; fn outer(){ use host::{first as keep, second, *}; { use other::first as keep; #[keep] fn inside(){} } } fn sibling(){ #[keep] fn outside(){} }",
        2024,
    );
    let imports = scan.imports();
    assert_eq!(imports.len(), 5);
    assert_eq!(
        imports[1].source.kind,
        ImportKind::Named {
            alias: Some("keep".into())
        }
    );
    assert_eq!(imports[3].source.kind, ImportKind::Glob);
    assert_eq!(imports[1].lexical_scope, imports[2].lexical_scope);
    assert_ne!(imports[1].lexical_scope, imports[4].lexical_scope);
    let inside = &scan.producers()[0];
    let outside = &scan.producers()[1];
    assert_eq!(inside.lexical_scope, imports[4].lexical_scope);
    assert_ne!(outside.lexical_scope, imports[1].lexical_scope);
    assert!(imports.iter().all(|row| row.attribute_inputs.is_empty()));
}
#[test]
fn cold_import_capture_prunes_cfg_and_preserves_annotation_demands() {
    let scan = scan(
        "fn outer(){ #[cfg(lrr_disabled)] use host::hidden; #[replace] use host::visible as kept; }",
        2024,
    );
    assert_eq!(scan.imports().len(), 1);
    let row = &scan.imports()[0];
    assert_eq!(row.attribute_inputs, vec![scan.producers()[0].identity]);
    assert!(row.source.scope.owner.is_some());
    assert_eq!(scan.cost().disabled_subtrees, 1);
}
#[test]
fn cold_import_source_identity_binds_scope_cfg_path_and_alias() {
    let first = scan("fn outer(){use host::one as selected;}", 2024);
    let same = scan("fn outer(){use host::one as selected;}", 2024);
    let other = scan("fn outer(){use host::one as changed;}", 2024);
    assert_eq!(first.fingerprint(), same.fingerprint());
    assert_ne!(first.fingerprint(), other.fingerprint());
    assert_ne!(
        first.imports()[0].source.fingerprint(),
        other.imports()[0].source.fingerprint()
    );
}

#[test]
fn generated_declaration_coordinator_inherits_matched_item_insertion_scope_kind() {
    use lrr::{
        expand::{generated_declarations::GeneratedDeclarations, identity::GrammarSlot},
        syntax::declaration_scan::DeclarationScopeKind,
    };
    for (source, expected_kind) in [
        (
            "macro_rules! make{()=>{use host::one as keep;#[keep]fn inner(){}}}make!();",
            DeclarationScopeKind::Module,
        ),
        (
            "macro_rules! make{()=>{use host::one as keep;#[keep]fn inner(){}}}mod nested{make!();}",
            DeclarationScopeKind::Module,
        ),
    ] {
        let (parent, record) = cold_expansion(source);
        assert_eq!(record.key().grammar, GrammarSlot::Items);
        let producer = &parent.producers()[0];
        let insertion = parent
            .lexical_scopes()
            .iter()
            .find(|scope| scope.identity == producer.lexical_scope)
            .unwrap();
        assert_eq!(insertion.kind, expected_kind);
        let output = GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            Default::default(),
        )
        .unwrap();
        let root = &output.scan().lexical_scopes()[0];
        assert_eq!(root.kind, expected_kind);
        assert_eq!(output.scan().imports().len(), 1);
        assert!(
            output
                .scan()
                .producers()
                .iter()
                .all(|row| row.lexical_scope == root.identity)
        );
        assert_eq!(
            output.scan().scope().module_path,
            producer.scope.module_path
        );
    }
}

#[test]
fn generated_declaration_coordinator_keeps_statement_output_in_block_scope() {
    use lrr::{
        expand::{generated_declarations::GeneratedDeclarations, identity::GrammarSlot},
        syntax::declaration_scan::DeclarationScopeKind,
    };
    let (parent, record) = cold_expansion(
        "macro_rules! make{()=>{use host::one as keep;#[keep]fn inner(){}}}fn cold(){make!();}",
    );
    assert_eq!(record.key().grammar, GrammarSlot::Statements);
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        output.scan().lexical_scopes()[0].kind,
        DeclarationScopeKind::Block
    );
    assert_eq!(output.scan().imports().len(), 1);
    let item_key = {
        let mut key = record.key().clone();
        key.grammar = GrammarSlot::Items;
        key
    };
    assert_eq!(
        GeneratedDeclarations::admit(
            &item_key,
            Arc::new(record),
            1,
            &parent,
            &CfgSet::guest_default(),
            Default::default()
        )
        .err()
        .unwrap()
        .code,
        "LRR-E-EXPANSION-INPUT"
    );
}

// LRR token transport controls: final module registration still needs the
// coordinator's epoch, grammar and resolution-authority admission.
#[test]
fn generated_module_name_transport_preserves_raw_name_and_original_context_in_actual_slots() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    use lrr_expand_adapter::token_wire::TokenTree;
    for source in [
        "macro_rules! make{()=>{mod r#type {}}}make!();",
        "macro_rules! make{()=>{mod r#type {}}}fn cold(){make!();}",
        "macro_rules! make{()=>{{mod r#type {}}}}fn cold(){let _=make!();}",
    ] {
        let (parent, record) = cold_expansion(source);
        let output = GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap();
        assert_eq!(output.scan().modules().len(), 1);
        let name = output
            .module_name_tokens(output.scan().modules()[0].identity)
            .unwrap();
        let roots = match name.roots() {
            [TokenTree::Group { children, .. }] => children.as_slice(),
            roots => roots,
        };
        let [TokenTree::Ident { text, raw, span }] = roots else {
            panic!("one original name required")
        };
        assert_eq!(text, "type");
        assert!(*raw);
        let original = record.tokens().unwrap();
        let kept = name.spans().iter().find(|s| s.key() == *span).unwrap();
        assert!(
            original
                .spans()
                .iter()
                .any(|s| s.key().0 == span.0 && s.context.0 == kept.context.0)
        );
        assert_eq!(
            name.contexts()
                .iter()
                .find(|c| c.key() == kept.context)
                .unwrap()
                .edition,
            lrr_expand_adapter::token_wire::Edition::E2024
        );
        assert_eq!(
            output
                .module_name_tokens(Digest([253; 32]))
                .unwrap_err()
                .code,
            "LRR-E-EXPANSION-INPUT"
        );
    }
}
#[test]
fn generated_module_name_transport_refuses_opaque_argument_module_text() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    let (parent, record) =
        cold_expansion("macro_rules! make{()=>{unknown!(mod hidden {});}}fn cold(){make!();}");
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    assert!(output.scan().modules().is_empty());
    assert_eq!(
        output
            .module_name_tokens(Digest([253; 32]))
            .unwrap_err()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
}

#[test]
fn generated_module_binding_admission_keeps_exact_generation_context_and_annotation_frontier() {
    use lrr::{
        expand::generated_declarations::GeneratedDeclarations,
        frontend::{
            impl_index::{
                DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
            },
            macro_names::DeclarationModuleOrigin,
        },
    };
    for source in [
        "macro_rules! make{()=>{mod r#type {} #[first] mod pending {}}}make!();",
        "macro_rules! make{()=>{mod r#type {} #[first] mod pending {}}}fn cold(){make!();}",
        "macro_rules! make{()=>{{mod r#type {} #[first] mod pending {}}}}fn cold(){let _=make!();}",
    ] {
        let (parent, record) = cold_expansion(source);
        let output = Arc::new(
            GeneratedDeclarations::admit(
                record.key(),
                Arc::new(record.clone()),
                1,
                &parent,
                &CfgSet::guest_default(),
                DeclarationScanLimits::default(),
            )
            .unwrap(),
        );
        let source_index = Arc::new(DeclarationMacroIndex::new(parent.clone()).unwrap());
        let index = GeneratedDeclarationMacroIndex::new(
            output.clone(),
            DeclarationMacroParent::Source(source_index),
        )
        .unwrap();
        let bindings = index.module_bindings(1).unwrap();
        assert_eq!(bindings.len(), 2);
        for (binding, row) in bindings.iter().zip(output.scan().modules()) {
            assert_eq!(binding.identity, row.identity);
            assert_eq!(binding.pending, row.attribute_inputs.first().copied());
            assert_eq!(binding.scope, row.scope);
            assert_eq!(binding.route_unit, row.scope.crate_key);
            assert_eq!(
                binding.lexical_scope,
                if row.lexical_scope == output.scan().lexical_scopes()[0].identity {
                    parent.producers()[0].lexical_scope
                } else {
                    row.lexical_scope
                }
            );
            let DeclarationModuleOrigin::Generated {
                producer,
                generation,
                context,
            } = binding.origin
            else {
                panic!("owned generated provenance required")
            };
            assert_eq!(producer, record.key().invocation);
            assert_eq!(generation, output.fingerprint());
            // Context comparison below uses public token metadata; no name
            // comparison class or matching spelling grants admission authority.
            let name_tokens = output.module_name_tokens(row.identity).unwrap();
            let roots = match name_tokens.roots() {
                [lrr_expand_adapter::token_wire::TokenTree::Group { children, .. }] => {
                    children.as_slice()
                }
                roots => roots,
            };
            let [lrr_expand_adapter::token_wire::TokenTree::Ident { span, .. }] = roots else {
                panic!("one module name required")
            };
            let physical = name_tokens
                .spans()
                .iter()
                .find(|s| s.key() == *span)
                .unwrap()
                .context;
            let mut store = lrr_expand_adapter::hygiene::HygieneStore::default();
            store.import(name_tokens.contexts()).unwrap();
            let expected = store.resolution_contexts(&[physical]).unwrap()[0].item;
            assert_eq!(context.0, expected.0);
            assert_eq!(
                binding.canonical_attachment,
                record.key().grammar == lrr::expand::identity::GrammarSlot::Items
            );
        }
        assert!(bindings[0].pending.is_none());
        assert!(bindings[1].pending.is_some());
        assert_eq!(
            index.module_bindings(2).unwrap_err().code,
            "LRR-E-EXPANSION-EPOCH"
        );
    }
}

// Component adaptation: unchanged rustc rule plus its r#gen invocation, e2024.
// Other keywords/editions and semantic typing of the generated items are unexecuted.
#[test]
fn rustc_raw_module_names_share_capture_but_keep_distinct_owned_identities() {
    use lrr::{
        expand::generated_declarations::GeneratedDeclarations,
        frontend::{
            impl_index::{
                DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
            },
            macro_names::MacroNamespaces,
        },
    };
    use lrr_expand_adapter::token_wire::TokenTree;
    let upstream =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/parser/raw/raw-idents.rs");
    let start = upstream.find("macro_rules! tests {").unwrap();
    let end = upstream.find("tests!(r#break);").unwrap();
    let invocation = upstream
        .lines()
        .find(|line| *line == "tests!(r#gen);")
        .unwrap();
    let source = format!("{}\n{}", &upstream[start..end], invocation);
    let (parent, record) = cold_expansion(&source);
    let output = Arc::new(
        GeneratedDeclarations::admit(
            record.key(),
            Arc::new(record.clone()),
            1,
            &parent,
            &CfgSet::guest_default(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    );
    let index = GeneratedDeclarationMacroIndex::new(
        output.clone(),
        DeclarationMacroParent::Source(Arc::new(DeclarationMacroIndex::new(parent).unwrap())),
    )
    .unwrap();
    let bindings = index.module_bindings(1).unwrap();
    let raw = bindings
        .iter()
        .filter(|b| b.name.as_str() == "gen")
        .collect::<Vec<_>>();
    assert_eq!(raw.len(), 2);
    assert_ne!(raw[0].identity, raw[1].identity);
    assert_ne!(raw[0].lexical_scope, raw[1].lexical_scope);
    assert_ne!(raw[0].scope, raw[1].scope);
    assert_ne!(raw[0].route_components, raw[1].route_components);
    let mut spans = Vec::new();
    for binding in &raw {
        let tokens = output.module_name_tokens(binding.identity).unwrap();
        let roots = match tokens.roots() {
            [TokenTree::Group { children, .. }] => children.as_slice(),
            roots => roots,
        };
        let [TokenTree::Ident { text, raw, span }] = roots else {
            panic!("exact physical module identifier required")
        };
        assert_eq!(text, "gen");
        assert!(*raw);
        spans.push(*span);
    }
    assert_eq!(
        spans[0], spans[1],
        "both substitutions retain the actual invocation token span"
    );
    let mut names = MacroNamespaces::default();
    for binding in bindings {
        names
            .declare_scoped_module_binding(binding.clone())
            .unwrap();
        names.declare_scoped_module_binding(binding).unwrap();
    }
    assert_eq!(
        index.module_bindings(2).unwrap_err().code,
        "LRR-E-EXPANSION-EPOCH"
    );
}

#[test]
fn rustc_raw_module_batch_preserves_requested_rows_and_refuses_duplicate_ranges() {
    use lrr::expand::generated_declarations::GeneratedDeclarations;
    use lrr_expand_adapter::{
        declaration_output::DeclarationGrammar,
        generated_attribute::module_binding_forests_for_grammar, token_wire::TokenTree,
    };
    let upstream =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/parser/raw/raw-idents.rs");
    let start = upstream.find("macro_rules! tests {").unwrap();
    let end = upstream.find("tests!(r#break);").unwrap();
    let invocation = upstream
        .lines()
        .find(|line| *line == "tests!(r#gen);")
        .unwrap();
    let (parent, record) = cold_expansion(&format!("{}\n{}", &upstream[start..end], invocation));
    let output = GeneratedDeclarations::admit(
        record.key(),
        Arc::new(record.clone()),
        1,
        &parent,
        &CfgSet::guest_default(),
        DeclarationScanLimits::default(),
    )
    .unwrap();
    let mut ranges = output
        .scan()
        .modules()
        .iter()
        .map(|row| (row.range.start, row.range.end))
        .collect::<Vec<_>>();
    ranges.reverse();
    let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(
        &record.tokens().unwrap().encode().unwrap(),
    )
    .unwrap();
    let names =
        module_binding_forests_for_grammar(&tokens, &ranges, DeclarationGrammar::Items).unwrap();
    assert_eq!(names.len(), ranges.len());
    for (forest, row) in names.iter().zip(output.scan().modules().iter().rev()) {
        let roots = match forest.roots() {
            [TokenTree::Group { children, .. }] => children.as_slice(),
            roots => roots,
        };
        let [TokenTree::Ident { text, raw, .. }] = roots else {
            panic!("physical module identifier required")
        };
        assert_eq!(text, row.name.strip_prefix("r#").unwrap_or(&row.name));
        assert_eq!(*raw, row.name.starts_with("r#"));
    }
    let duplicate = [ranges[0], ranges[0]];
    let error = module_binding_forests_for_grammar(&tokens, &duplicate, DeclarationGrammar::Items)
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-OUTPUT");
    assert!(error.message.contains("duplicate"));
    let missing = [(0, 1)];
    let error = module_binding_forests_for_grammar(&tokens, &missing, DeclarationGrammar::Items)
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-OUTPUT");
    assert!(error.message.contains("absent"));
}

#[test]
fn completed_declaration_fragment_retains_child_module_facts_without_route_authority() {
    use lrr::{
        expand::declaration_fragment::DeclarationFragmentStep,
        frontend::macro_names::DeclarationModuleOrigin,
    };
    let source = "macro_rules! leaf{()=>{mod leaf {}}}macro_rules! outer{()=>{mod root {} leaf!();}}fn cold(){outer!();}";
    let (provider, mut fragment, budget) = declaration_fragment(source);
    let mut invocations = Vec::new();
    let completed = loop {
        assert!(invocations.len() <= 2);
        match fragment.poll(&provider).unwrap() {
            DeclarationFragmentStep::NeedExpansion(request) => {
                invocations.push(request.key().invocation);
                fragment.admit(Arc::new(request.expand().unwrap())).unwrap();
            }
            DeclarationFragmentStep::Complete(output) => break output,
            _ => panic!("module metadata supplement must finish its real declarative producers"),
        }
    };
    assert_eq!(invocations.len(), 2);
    assert_eq!(completed.records().len(), 2);
    assert_eq!(budget.totals().unwrap().outputs, 2);
    assert_eq!(completed.index().module_bindings(1).unwrap().len(), 1);
    let bindings = completed.module_bindings(1).unwrap();
    assert_eq!(bindings.len(), 2);
    let names = bindings
        .iter()
        .map(|b| b.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names, std::collections::BTreeSet::from(["leaf", "root"]));
    assert_eq!(
        bindings
            .iter()
            .map(|b| b.identity)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    let mut generations = std::collections::BTreeSet::new();
    for binding in &bindings {
        assert!(
            !binding.canonical_attachment,
            "block module must not gain a global route"
        );
        assert!(binding.scope.owner.is_some());
        let DeclarationModuleOrigin::Generated {
            producer,
            generation,
            ..
        } = binding.origin
        else {
            panic!("matched owned generation required");
        };
        assert!(invocations.contains(&producer));
        generations.insert(generation);
    }
    assert_eq!(generations.len(), 2);
    assert_eq!(
        completed.module_bindings(1).unwrap(),
        bindings,
        "metadata reads are repeatable"
    );
    for epoch in [0, 2] {
        assert_eq!(
            completed.module_bindings(epoch).unwrap_err().code,
            "LRR-E-EXPANSION-EPOCH"
        );
    }
}

#[test]
fn completed_module_facts_keep_annotation_frontier_until_exact_proof_is_installed() {
    use lrr::expand::declaration_fragment::DeclarationFragmentStep;
    let (provider, mut fragment, _) = declaration_fragment(
        "macro_rules! outer{()=>{#[allow(dead_code)] mod guarded{}}}fn cold(){outer!();}",
    );
    let DeclarationFragmentStep::NeedExpansion(request) = fragment.poll(&provider).unwrap() else {
        panic!("actual root demand required")
    };
    fragment.admit(Arc::new(request.expand().unwrap())).unwrap();
    let DeclarationFragmentStep::Complete(completed) = fragment.poll(&provider).unwrap() else {
        panic!("pinned inert attribute should complete scheduling")
    };
    assert!(completed.index().is_complete());
    let bindings = completed.module_bindings(1).unwrap();
    assert_eq!(bindings.len(), 1);
    let row = &completed.index().output().scan().modules()[0];
    assert_eq!(row.attribute_inputs.len(), 1);
    assert_eq!(bindings[0].pending, Some(row.attribute_inputs[0]));
    assert!(!bindings[0].canonical_attachment);
}
