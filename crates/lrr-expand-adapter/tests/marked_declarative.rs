use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite, FailureKind},
    token_wire::{Edition, Key, TokenForest, TokenTree, Transparency},
};
#[path = "support/declarative.rs"]
mod fixture;
fn site(expansion: u8) -> DeclarativeSite {
    DeclarativeSite {
        expansion: Key([expansion; 32]),
        defining_unit: Key([1; 32]),
        definition_edition: Edition::E2021,
    }
}
fn identifier_span(forest: &TokenForest, text: &str) -> Key {
    let mut stack: Vec<_> = forest.roots().iter().collect();
    while let Some(token) = stack.pop() {
        match token {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Ident {
                text: token_text,
                span,
                ..
            } if token_text == text => return *span,
            _ => {}
        }
    }
    panic!("missing identifier {text}");
}
#[test]
fn marked_transcription_preserves_template_and_argument_resolution_locations() {
    let rules = DeclarativeMacro::new(fixture::definition(
        "($value:ident) => { (helper, $value) };",
        Edition::E2021,
    ))
    .unwrap();
    let args = fixture::input("caller");
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), site(3), 100, 4096)
        .unwrap();
    let tokens = result.unwrap().tokens;
    tokens.validate().unwrap();
    assert_eq!(stats.admitted_bytes, tokens.encode().unwrap().len());
    let helper = tokens
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&tokens, "helper"))
        .unwrap();
    let caller = tokens
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&tokens, "caller"))
        .unwrap();
    let context = tokens
        .contexts()
        .iter()
        .find(|c| c.key() == helper.context)
        .unwrap();
    assert_eq!(context.expansion, Some(site(3).expansion));
    assert_eq!(context.transparency, Transparency::SemiTransparent);
    assert_eq!(context.defining_unit, site(3).defining_unit);
    assert_eq!(context.edition, Edition::E2021);
    assert_eq!(helper.edition, Edition::E2021);
    assert_eq!(helper.source.unit, Key([1; 32]));
    let original_caller = args
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&args, "caller"))
        .unwrap();
    assert_eq!(caller, original_caller);
    assert!(
        tokens
            .contexts()
            .iter()
            .any(|c| Some(c.key()) == context.parent)
    );
}
#[test]
fn marked_byte_admission_accepts_exact_boundary_and_rejects_before_materialization() {
    let rules = DeclarativeMacro::new(fixture::definition(
        "($($v:ident),*) => {[$((helper,$v)),*]};",
        Edition::E2021,
    ))
    .unwrap();
    let args = fixture::input("one,two,three");
    let call = fixture::call_site(&args);
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, call, site(3), 100, 4096)
        .unwrap();
    let bytes = result.unwrap().tokens.encoded_len().unwrap();
    assert_eq!(stats.admitted_bytes, bytes);
    assert_eq!(
        rules
            .expand_hygienic_with_limits(&args, call, site(3), 100, bytes)
            .unwrap()
            .1
            .materializations,
        1
    );
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, call, site(3), 100, bytes - 1)
        .unwrap();
    assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
    assert_eq!(stats.materializations, 0);
}
#[test]
fn a_generated_declarative_definition_reimports_marked_contexts_and_ancestors() {
    let generate = DeclarativeMacro::new(fixture::definition(
        "() => { { () => { helper }; } };",
        Edition::E2021,
    ))
    .unwrap();
    let args = fixture::input("");
    let (result, _) = generate
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), site(3), 100, 4096)
        .unwrap();
    let output = result.unwrap().tokens;
    let [TokenTree::Group { children, .. }] = output.roots() else {
        panic!("missing root")
    };
    let definition = TokenForest::new(
        vec![children[0].clone()],
        output.contexts().to_vec(),
        output.spans().to_vec(),
    )
    .unwrap();
    let generated = DeclarativeMacro::new(definition).unwrap();
    let (result, stats) = generated
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), site(4), 100, 4096)
        .unwrap();
    let output = result.unwrap().tokens;
    assert_eq!(stats.admitted_bytes, output.encoded_len().unwrap());
    let span = output
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&output, "helper"))
        .unwrap();
    let context = output
        .contexts()
        .iter()
        .find(|c| c.key() == span.context)
        .unwrap();
    assert_eq!(context.expansion, Some(site(4).expansion));
    let parent = output
        .contexts()
        .iter()
        .find(|c| Some(c.key()) == context.parent)
        .unwrap();
    assert_eq!(parent.expansion, Some(site(3).expansion));
    assert!(
        output
            .contexts()
            .iter()
            .any(|c| Some(c.key()) == parent.parent)
    );
}

#[test]
fn marked_actual_wire_ceiling_and_byte_plus_one_use_identical_codec_costs() {
    const LIMIT: usize = 32 * 1024 * 1024;
    let args = fixture::input("");
    let call = fixture::call_site(&args);
    let baseline =
        DeclarativeMacro::new(fixture::definition("() => {\"\"};", Edition::E2021)).unwrap();
    let (result, stats) = baseline
        .expand_hygienic_with_limits(&args, call, site(3), 100, LIMIT)
        .unwrap();
    assert_eq!(fixture::literal_texts(&result.unwrap().tokens), ["\"\""]);
    let overhead = stats.admitted_bytes - 2;
    for extra in 0..=1 {
        let body = format!(
            "() => {{\"{}\"}};",
            "x".repeat(LIMIT - overhead - 2 + extra)
        );
        let rules = DeclarativeMacro::new(fixture::definition(&body, Edition::E2021)).unwrap();
        let (result, stats) = rules
            .expand_hygienic_with_limits(&args, call, site(3), 100, LIMIT)
            .unwrap();
        if extra == 0 {
            assert_eq!(result.unwrap().tokens.encoded_len().unwrap(), LIMIT);
            assert_eq!(stats.admitted_bytes, LIMIT);
            assert_eq!(stats.materializations, 1);
        } else {
            assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
            assert_eq!(stats.materializations, 0);
            assert_eq!(stats.max_preflight_bytes, LIMIT + 1);
        }
    }
}

#[test]
fn marked_empty_repeat_rollback_drops_unused_context_ancestors() {
    let rules = DeclarativeMacro::new(fixture::definition(
        "($($v:ident),*) => {$(helper $v)*};",
        Edition::E2021,
    ))
    .unwrap();
    let args = fixture::input("");
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), site(3), 100, 4096)
        .unwrap();
    let output = result.unwrap().tokens;
    assert_eq!(stats.admitted_bytes, output.encoded_len().unwrap());
    assert_eq!(output.contexts().len(), 1);
    assert_eq!(output.contexts()[0].defining_unit, Key([2; 32]));
    assert_eq!(output.contexts()[0].expansion, None);
    assert_eq!(stats.materialized_leaves, 0);
}

#[test]
fn generated_definition_owner_is_independent_of_token_source_unit_and_edition() {
    let rules =
        DeclarativeMacro::new(fixture::definition("() => {helper};", Edition::E2021)).unwrap();
    let args = fixture::input("");
    let site = DeclarativeSite {
        expansion: Key([3; 32]),
        defining_unit: Key([9; 32]),
        definition_edition: Edition::E2024,
    };
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), site, 100, 4096)
        .unwrap();
    let output = result.unwrap().tokens;
    let record = output
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&output, "helper"))
        .unwrap();
    assert_eq!(record.source.unit, Key([1; 32]));
    assert_eq!(record.edition, Edition::E2021);
    let context = output
        .contexts()
        .iter()
        .find(|c| c.key() == record.context)
        .unwrap();
    assert_eq!(context.defining_unit, site.defining_unit);
    assert_eq!(context.edition, Edition::E2024);
    assert_eq!(stats.admitted_bytes, output.encoded_len().unwrap());
}

#[test]
fn dollar_crate_token_carries_explicit_definition_authority_after_marking() {
    let rules = DeclarativeMacro::new(fixture::definition(
        "() => {$crate::helper()};",
        Edition::E2021,
    ))
    .unwrap();
    let args = fixture::input("");
    let mut call = site(3);
    call.defining_unit = Key([9; 32]);
    let (result, stats) = rules
        .expand_hygienic_with_limits(&args, fixture::call_site(&args), call, 100, 4096)
        .unwrap();
    let output = result.unwrap().tokens;
    let record = output
        .spans()
        .iter()
        .find(|s| s.key() == identifier_span(&output, "$crate"))
        .unwrap();
    let context = output
        .contexts()
        .iter()
        .find(|c| c.key() == record.context)
        .unwrap();
    assert_eq!(context.defining_unit, call.defining_unit);
    assert_eq!(context.expansion, Some(call.expansion));
    assert_eq!(record.source.unit, Key([1; 32]));
    assert_eq!(stats.admitted_bytes, output.encoded_len().unwrap());
}
