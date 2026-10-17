//! Builtin expander unit tests. Expected strings are rustc's behaviour: cited
//! from the pinned rustc checkout (`rustc/rust`, commit ae0fe511) sources and
//! upstream ui tests, and observed with `rustc 1.99.0` on the same inputs.
use super::*;
use crate::declarative::{DeclarativeMacro, DeclarativeSite};
use crate::token_wire::{ContextRecord, SourceIdentity, SpanRecord, parse_source};
use syntax::AstNode;

const CALLER: Key = Key([2; 32]);
const CORE: Key = Key([7; 32]);

fn site() -> BuiltinSite {
    BuiltinSite {
        expansion: Key([9; 32]),
        defining_unit: CORE,
        definition_edition: Edition::E2024,
    }
}

fn facts(edition: Edition) -> BuiltinCallSite {
    BuiltinCallSite {
        edition,
        line: 16,
        column: 5,
        file: "src/m\u{e9}.rs".into(),
        module_path: "demo::m1::m2".into(),
        cfg: [
            ("unix".to_owned(), None),
            ("target_os".to_owned(), Some("macos".to_owned())),
        ]
        .into_iter()
        .collect(),
        crate_names: [(CORE, "core".to_owned())].into_iter().collect(),
    }
}

fn source(text: &str, unit: Key, edition: Edition) -> TokenForest {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: unit,
        edition,
    };
    parse_source(
        text,
        SourceIdentity {
            unit,
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "test.rs".into(),
        },
        context,
        Key([3; 32]),
    )
    .unwrap()
}

/// The argument group of the first macro call in `text`, with its open span.
fn call_group(forest: &TokenForest) -> (TokenForest, Key) {
    fn find(trees: &[TokenTree]) -> Option<TokenTree> {
        for (i, tree) in trees.iter().enumerate() {
            if let (
                TokenTree::Punct { scalar: '!', .. },
                Some(group @ TokenTree::Group { delimiter, .. }),
            ) = (tree, trees.get(i + 1))
                && *delimiter != Delimiter::Invisible
            {
                return Some(group.clone());
            }
            if let TokenTree::Group { children, .. } = tree
                && let Some(found) = find(children)
            {
                return Some(found);
            }
        }
        None
    }
    let group = find(forest.roots()).expect("macro call group");
    let TokenTree::Group { open, .. } = &group else {
        unreachable!()
    };
    let open = *open;
    (
        TokenForest::new(
            vec![group],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap(),
        open,
    )
}

fn expand_in(
    kind: BuiltinKind,
    text: &str,
    edition: Edition,
) -> Result<TokenForest, BuiltinFailure> {
    let forest = source(text, CALLER, edition);
    let (input, call_site) = call_group(&forest);
    expand_builtin(kind, &input, call_site, &site(), &facts(edition))
        .unwrap()
        .map(|expansion| {
            expansion.tokens.validate().unwrap();
            expansion.tokens
        })
}

fn expand(kind: BuiltinKind, text: &str) -> TokenForest {
    expand_in(kind, text, Edition::E2021).unwrap()
}

/// Deterministic token rendering: tokens separated by spaces, `Joint`
/// punctuation glued to its successor.
fn render(forest: &TokenForest) -> String {
    fn walk(trees: &[TokenTree], out: &mut String) {
        for tree in trees {
            match tree {
                TokenTree::Group {
                    delimiter,
                    children,
                    ..
                } => {
                    let (o, c) = match delimiter {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::Invisible => ("<<", ">>"),
                    };
                    out.push_str(o);
                    out.push(' ');
                    walk(children, out);
                    out.push_str(c);
                    out.push(' ');
                }
                TokenTree::Ident { text, raw, .. } => {
                    if *raw {
                        out.push_str("r#");
                    }
                    out.push_str(text);
                    out.push(' ');
                }
                TokenTree::Punct {
                    scalar, spacing, ..
                } => {
                    out.push(*scalar);
                    if *spacing != Spacing::Joint {
                        out.push(' ');
                    }
                }
                TokenTree::Literal { token_text, .. } => {
                    out.push_str(token_text);
                    out.push(' ');
                }
            }
        }
    }
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("output root")
    };
    let mut out = String::new();
    walk(children, &mut out);
    out.trim_end().to_owned()
}

fn only_literal(forest: &TokenForest) -> (String, Key) {
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("output root")
    };
    let [TokenTree::Literal { token_text, span }] = children.as_slice() else {
        panic!("expected one literal, got {}", render(forest))
    };
    (token_text.clone(), *span)
}

fn stringify(text: &str) -> String {
    let (literal, _) = only_literal(&expand(BuiltinKind::Stringify, text));
    tts::unquote_str(&literal).unwrap()
}

fn assertion(text: &str) -> String {
    let output = expand(BuiltinKind::Assert, text);
    let mut stack: Vec<&TokenTree> = output.roots().iter().collect();
    while let Some(tree) = stack.pop() {
        match tree {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Literal { token_text, .. } if token_text.contains("assertion failed") => {
                return tts::unquote_str(token_text).unwrap();
            }
            _ => {}
        }
    }
    panic!("no assertion message in {}", render(&output))
}

fn span_context(forest: &TokenForest, span: Key) -> (&SpanRecord, &ContextRecord) {
    let record = forest.spans().iter().find(|s| s.key() == span).unwrap();
    let context = forest
        .contexts()
        .iter()
        .find(|c| c.key() == record.context)
        .unwrap();
    (record, context)
}

fn root_of<'a>(forest: &'a TokenForest, mut context: &'a ContextRecord) -> &'a ContextRecord {
    while let Some(parent) = context.parent {
        context = forest
            .contexts()
            .iter()
            .find(|c| c.key() == parent)
            .unwrap();
    }
    context
}

fn ident_span(forest: &TokenForest, text: &str) -> Key {
    let mut stack: Vec<&TokenTree> = forest.roots().iter().collect();
    while let Some(tree) = stack.pop() {
        match tree {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Ident { text: t, span, .. } if t == text => return *span,
            _ => {}
        }
    }
    panic!("missing {text}")
}

// ----- classification ------------------------------------------------------

#[test]
fn builtin_names_follow_rustc_register_bang() {
    assert_eq!(builtin_kind("core_panic"), Some(BuiltinKind::CorePanic));
    assert_eq!(builtin_kind("std_panic"), Some(BuiltinKind::StdPanic));
    assert_eq!(builtin_kind("unreachable"), Some(BuiltinKind::Unreachable));
    assert_eq!(builtin_kind("assert"), Some(BuiltinKind::Assert));
    assert_eq!(
        builtin_kind("panic"),
        None,
        "`panic` is named by its attribute argument"
    );
    assert_eq!(
        builtin_kind("concat").map(BuiltinKind::disposition),
        Some(BuiltinDisposition::HandledByLrr)
    );
    assert_eq!(
        builtin_kind("cfg_select").map(BuiltinKind::disposition),
        Some(BuiltinDisposition::Unsupported)
    );
    for (name, kind) in BUILTINS {
        assert_eq!(kind.name(), *name);
    }
}

// ----- assert! -------------------------------------------------------------

#[test]
fn assert_without_message_calls_panicking_panic_with_pretty_condition() {
    // rustc assert.rs: `if !cond { $crate::panicking::panic("assertion failed: ..") }`.
    let output = expand(BuiltinKind::Assert, "assert!(a+b==4);");
    assert_eq!(
        render(&output),
        "if ! ( a + b == 4 ) { $crate :: panicking :: panic ( \"assertion failed: a + b == 4\" ) }"
    );
}

#[test]
fn assert_message_matches_rustc_expr_to_string() {
    // Observed with rustc 1.99.0 (`assertion failed: ..` payloads).
    let cases = [
        ("a+b==4", "a + b == 4"),
        ("a  +  b ==  4", "a + b == 4"),
        ("!(a == 1)", "!(a == 1)"),
        ("f(a)||f( b )", "f(a) || f(b)"),
        ("v.iter().any(|&x|x==5)", "v.iter().any(|&x| x == 5)"),
        ("v.len()>3 && v[0]==2", "v.len() > 3 && v[0] == 2"),
        ("matches!(a, 2|3)", "matches!(a, 2|3)"),
        ("s.x as i64 == 5i64", "s.x as i64 == 5i64"),
        ("-a == 5 && *&a == 3", "-a == 5 && *&a == 3"),
        ("{ let z = a; z == 9 }", "{ let z = a; z == 9 }"),
        (
            "if a == 1 { false } else { true }",
            "if a == 1 { false } else { true }",
        ),
        (
            "match a { 1 => false, _ => true }",
            "match a { 1 => false, _ => true, }",
        ),
        ("v == vec![9, 8]", "v == vec![9, 8]"),
        ("S { x: 5 }.x == 3", "S { x: 5 }.x == 3"),
        ("a.max(b) == 5, ", "a.max(b) == 5"),
        ("(a, b) == (2, 1)", "(a, b) == (2, 1)"),
        ("[a; 2] == [3, 3]", "[a; 2] == [3, 3]"),
        ("(0..a).contains(&5)", "(0..a).contains(&5)"),
        (
            "std::mem::size_of::<u8>() == 2",
            "std::mem::size_of::<u8>() == 2",
        ),
        (
            "<i32 as Default>::default() == 1",
            "<i32 as Default>::default() == 1",
        ),
        (
            "&raw const a == &raw const b",
            "&raw const a == &raw const b",
        ),
        ("unsafe { false }", "unsafe { false }"),
        ("false", "false"),
        // tests/ui/macros/assert-as-macro.rs, assert-macro-explicit.rs patterns.
        ("1 == 2", "1 == 2"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            assertion(&format!("assert!({input});")),
            format!("assertion failed: {expected}"),
            "input: {input}"
        );
    }
}

#[test]
fn assert_long_condition_breaks_lines_like_rustc() {
    // tests/ui/macros/assert-long-condition.run.stderr (unchanged upstream).
    let condition = "1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18 + 19 + 20 + 21 + 22 + 23 + 24 + 25 == 0";
    assert_eq!(
        assertion(&format!("assert!({condition});")),
        "assertion failed: 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18\n                                + 19 + 20 + 21 + 22 + 23 + 24 + 25 == 0"
    );
    // Observed with rustc 1.99.0.
    assert_eq!(
        assertion(
            "assert!(a == 1 && b == 2 && false   &&   aaaaaaaaaaaaaaaaaaa(a) == bbbbbbbbbbbbbbbbbbbbbbbbbbbbb(b) && xxxxxxxxxxxxxxxxxxx(4));"
        ),
        "assertion failed: a == 1 && b == 2 && false &&\n        aaaaaaaaaaaaaaaaaaa(a) == bbbbbbbbbbbbbbbbbbbbbbbbbbbbb(b) &&\n    xxxxxxxxxxxxxxxxxxx(4)"
    );
}

#[test]
fn assert_with_message_uses_edition_panic_path() {
    // 2021+: `$crate::panic::panic_2021!(..)`; earlier editions: unqualified
    // call-site `panic!(..)` (rustc assert.rs `panic_path`).
    let output = expand(BuiltinKind::Assert, "assert!(x, \"bad {}\", y);");
    assert_eq!(
        render(&output),
        "if ! ( x ) { $crate :: panic :: panic_2021 ! ( \"bad {}\" , y ) }"
    );
    let output = expand_in(
        BuiltinKind::Assert,
        "assert!(x, \"bad {}\", y);",
        Edition::E2018,
    )
    .unwrap();
    assert_eq!(render(&output), "if ! ( x ) { panic ! ( \"bad {}\" , y ) }");
    let (_, context) = span_context(&output, ident_span(&output, "panic"));
    assert_eq!(context.transparency, Transparency::Transparent);
}

#[test]
fn assert_rejects_what_rustc_rejects() {
    let fail = |text: &str| expand_in(BuiltinKind::Assert, text, Edition::E2021).unwrap_err();
    assert_eq!(
        fail("assert!();").message,
        "macro requires a boolean expression as an argument"
    );
    assert_eq!(
        fail("assert!(x;);").message,
        "macro requires an expression as an argument"
    );
    assert_eq!(
        fail("assert!(x \"m\");").message,
        "unexpected string literal"
    );
    assert_eq!(fail("assert!(x y);").kind, BuiltinFailureKind::Syntax);
}

#[test]
fn assert_def_site_and_call_site_hygiene() {
    let forest = source("fn main() { assert!(a); }", CALLER, Edition::E2021);
    let (input, call_site) = call_group(&forest);
    let output = expand_builtin(
        BuiltinKind::Assert,
        &input,
        call_site,
        &site(),
        &facts(Edition::E2021),
    )
    .unwrap()
    .unwrap()
    .tokens;
    let call_record = input.spans().iter().find(|s| s.key() == call_site).unwrap();
    // `$crate`: opaque mark of this expansion carrying core's authority, at the
    // invocation location.
    let (record, context) = span_context(&output, ident_span(&output, "$crate"));
    assert_eq!(context.expansion, Some(site().expansion));
    assert_eq!(context.transparency, Transparency::Opaque);
    assert_eq!(context.defining_unit, CORE);
    assert_eq!(root_of(&output, context).defining_unit, CORE);
    assert_eq!(
        (record.start, record.end),
        (call_record.start, call_record.end)
    );
    assert_eq!(record.source, call_record.source);
    // `if`: transparent mark over the caller's root.
    let (_, context) = span_context(&output, ident_span(&output, "if"));
    assert_eq!(context.expansion, Some(site().expansion));
    assert_eq!(context.transparency, Transparency::Transparent);
    assert_eq!(root_of(&output, context).defining_unit, CALLER);
    // The condition keeps its original span record.
    let a = ident_span(&output, "a");
    assert!(input.spans().iter().any(|s| s.key() == a));
}

#[test]
fn assert_through_macro_fragments_matches_rustc() {
    // Observed with rustc 1.99.0: `$e:expr` fragments print without the
    // parenthesis group Rust-analyzer inserts, and with precedence parens.
    let cases = [
        ("($e:expr) => { assert!($e) };", "a == b", "a == b"),
        (
            "($e:expr) => { assert!($e * 2 == 7) };",
            "1 + 2",
            "(1 + 2) * 2 == 7",
        ),
        ("() => { assert!(1+1==3) };", "", "1 + 1 == 3"),
    ];
    for (rules, argument, expected) in cases {
        let definition = source(
            &format!("macro_rules! m {{ {rules} }}"),
            CORE,
            Edition::E2021,
        );
        let [TokenTree::Group { children, .. }] = definition.roots() else {
            panic!()
        };
        let body = children
            .iter()
            .find(|t| {
                matches!(
                    t,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let body = TokenForest::new(
            vec![body],
            definition.contexts().to_vec(),
            definition.spans().to_vec(),
        )
        .unwrap();
        let declared = DeclarativeMacro::new(body).unwrap();
        let call = source(&format!("m!({argument});"), CALLER, Edition::E2021);
        let (args, call_site) = call_group(&call);
        let expanded = declared
            .expand_hygienic_with_limits(
                &args,
                call_site,
                DeclarativeSite {
                    expansion: Key([5; 32]),
                    defining_unit: CORE,
                    definition_edition: Edition::E2021,
                },
                1_000,
                1 << 20,
            )
            .unwrap()
            .0
            .unwrap()
            .tokens;
        let (input, inner_site) = call_group(&expanded);
        let output = expand_builtin(
            BuiltinKind::Assert,
            &input,
            inner_site,
            &site(),
            &facts(Edition::E2021),
        )
        .unwrap()
        .unwrap()
        .tokens;
        let message = only_message(&output);
        assert_eq!(
            message,
            format!("assertion failed: {expected}"),
            "rules: {rules:?}"
        );
    }
}

fn only_message(output: &TokenForest) -> String {
    let mut stack: Vec<&TokenTree> = output.roots().iter().collect();
    while let Some(tree) = stack.pop() {
        match tree {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Literal { token_text, .. } => return tts::unquote_str(token_text).unwrap(),
            _ => {}
        }
    }
    panic!("no message")
}

// ----- panic! / unreachable! ----------------------------------------------

#[test]
fn panic_and_unreachable_select_edition_variant() {
    // rustc edition_panic.rs: `$crate::panic::panic_2015|2021!(..)`, delimiters
    // replaced by parentheses.
    let cases = [
        (BuiltinKind::CorePanic, Edition::E2015, "panic_2015"),
        (BuiltinKind::StdPanic, Edition::E2018, "panic_2015"),
        (BuiltinKind::CorePanic, Edition::E2021, "panic_2021"),
        (BuiltinKind::StdPanic, Edition::E2024, "panic_2021"),
        (BuiltinKind::Unreachable, Edition::E2018, "unreachable_2015"),
        (BuiltinKind::Unreachable, Edition::E2024, "unreachable_2021"),
    ];
    for (kind, edition, mac) in cases {
        let output = expand_in(kind, "panic!{\"x {}\", 1}", edition).unwrap();
        assert_eq!(
            render(&output),
            format!("$crate :: panic :: {mac} ! ( \"x {{}}\" , 1 )")
        );
    }
    let output = expand(BuiltinKind::CorePanic, "panic!()");
    assert_eq!(render(&output), "$crate :: panic :: panic_2021 ! ( )");
}

// ----- format_args! --------------------------------------------------------

#[test]
fn format_args_emit_builtin_syntax_and_newline_variant() {
    assert_eq!(
        render(&expand(
            BuiltinKind::FormatArgs,
            "format_args!(\"{} {x}\", a, x = 1)"
        )),
        "builtin # format_args ( \"{} {x}\" , a , x = 1 )"
    );
    assert_eq!(
        render(&expand(
            BuiltinKind::ConstFormatArgs,
            "const_format_args![\"a\"]"
        )),
        "builtin # format_args ( \"a\" )"
    );
    assert_eq!(
        render(&expand(
            BuiltinKind::FormatArgsNl,
            "format_args_nl!(\"{}\", a)"
        )),
        "builtin # format_args ( \"{}\\n\" , a )"
    );
    assert_eq!(
        render(&expand(
            BuiltinKind::FormatArgsNl,
            "format_args_nl!(r#\"q\"\"#)"
        )),
        "builtin # format_args ( r#\"q\"\n\"# )"
    );
    assert_eq!(
        render(&expand(
            BuiltinKind::FormatArgsNl,
            "format_args_nl!(concat!(\"a\", \"b\"), x)"
        )),
        "builtin # format_args ( $crate :: concat ! ( concat ! ( \"a\" , \"b\" ) , \"\\n\" ) , x )"
    );
    // The parsed output is the pinned parser's FORMAT_ARGS_EXPR.
    let text = "builtin # format_args(\"{}\\n\", a)";
    let parse =
        syntax::SourceFile::parse(&format!("fn f() {{ {text}; }}"), span::Edition::Edition2021);
    assert!(parse.errors().is_empty());
    assert!(
        parse
            .tree()
            .syntax()
            .descendants()
            .any(|n| n.kind() == syntax::SyntaxKind::FORMAT_ARGS_EXPR)
    );
    assert_eq!(
        expand_in(BuiltinKind::FormatArgs, "format_args!()", Edition::E2021)
            .unwrap_err()
            .message,
        "requires at least a format string argument"
    );
}

// ----- source facts --------------------------------------------------------

#[test]
fn source_location_builtins_emit_rustc_literals() {
    let (line, line_span) = only_literal(&expand(BuiltinKind::Line, "line!()"));
    assert_eq!(line, "16u32");
    let (column, _) = only_literal(&expand(BuiltinKind::Column, "column!()"));
    assert_eq!(column, "5u32");
    let (file, _) = only_literal(&expand(BuiltinKind::File, "file!()"));
    assert_eq!(file, "\"src/m\\u{e9}.rs\"");
    assert_eq!(tts::unquote_str(&file).unwrap(), "src/m\u{e9}.rs");
    let output = expand(BuiltinKind::ModulePath, "module_path!()");
    let (module, span) = only_literal(&output);
    assert_eq!(module, "\"demo::m1::m2\"");
    let (_, context) = span_context(&output, span);
    assert_eq!(context.transparency, Transparency::Opaque);
    // `line!` uses the expansion-cause span itself (no new mark).
    let output = expand(BuiltinKind::Line, "line!()");
    let (_, context) = span_context(&output, line_span);
    assert!(context.parent.is_none());
    assert_eq!(
        expand_in(BuiltinKind::Line, "line!(1)", Edition::E2021)
            .unwrap_err()
            .message,
        "`line!` takes no arguments"
    );
}

// ----- cfg! / compile_error! ---------------------------------------------

#[test]
fn cfg_evaluates_predicates_against_supplied_configuration() {
    let value = |text: &str| render(&expand(BuiltinKind::Cfg, text));
    assert_eq!(value("cfg!(unix)"), "true");
    assert_eq!(value("cfg!(windows)"), "false");
    assert_eq!(value("cfg!(target_os = \"macos\")"), "true");
    assert_eq!(value("cfg!(target_os = r\"linux\")"), "false");
    assert_eq!(value("cfg!(all(unix, not(windows)),)"), "true");
    assert_eq!(value("cfg!(any())"), "false");
    assert_eq!(value("cfg!(all())"), "true");
    assert_eq!(value("cfg!(true)"), "true");
    assert_eq!(value("cfg!(not(false))"), "true");
    let fail = |text: &str| expand_in(BuiltinKind::Cfg, text, Edition::E2021).unwrap_err();
    assert_eq!(
        fail("cfg!()").message,
        "macro requires a cfg-pattern as an argument"
    );
    assert_eq!(fail("cfg!(a, b)").message, "expected 1 cfg-pattern");
    assert_eq!(fail("cfg!(a = 1)").kind, BuiltinFailureKind::Syntax);
    assert_eq!(fail("cfg!(a::b)").kind, BuiltinFailureKind::Syntax);
    assert_eq!(fail("cfg!(not(a, b))").kind, BuiltinFailureKind::Syntax);
}

#[test]
fn compile_error_reports_its_message() {
    let failure = expand_in(
        BuiltinKind::CompileError,
        "compile_error!(\"no \\\"x\\\"\");",
        Edition::E2021,
    )
    .unwrap_err();
    assert_eq!(failure.kind, BuiltinFailureKind::CompileError);
    assert_eq!(failure.message, "no \"x\"");
    let failure = expand_in(
        BuiltinKind::CompileError,
        "compile_error!(r\"raw\",);",
        Edition::E2021,
    )
    .unwrap_err();
    assert_eq!(failure.message, "raw");
    let failure = expand_in(
        BuiltinKind::CompileError,
        "compile_error!(concat!(\"a\"));",
        Edition::E2021,
    )
    .unwrap_err();
    assert_eq!(failure.kind, BuiltinFailureKind::RequiresEagerExpansion);
}

// ----- stringify! -----------------------------------------------------------

#[test]
fn stringify_reproduces_rustc_token_spacing() {
    // Observed with rustc 1.99.0.
    assert_eq!(
        stringify(
            "stringify!(a+b a + b a+ b f (x) f(x) x . y x.y #[a] # [b] {x} { x } {} ( a ) [ a , b ])"
        ),
        "a+b a + b a+ b f(x) f(x) x.y x.y #[a] #[b] {x} { x } {} (a) [a, b]"
    );
    assert_eq!(
        stringify("stringify!(r#type $crate if !x { } -1 a::<b>() c ..= d &&e ||f <-g)"),
        "r#type $crate if !x {} -1 a::<b>() c ..= d &&e ||f <-g"
    );
    assert_eq!(
        stringify("stringify!(\"\u{e9}\\n\" 'x' b\"a\" r\"raw\" 1u8 1.0e3)"),
        "\"\u{e9}\\n\" 'x' b\"a\" r\"raw\" 1u8 1.0e3"
    );
    assert_eq!(
        stringify("stringify!(/// doc\n    x)"),
        "#[doc = r\" doc\"] x"
    );
    // tests/ui/macros/syntax-extension-source-utils.rs
    assert_eq!(stringify("stringify!((2*3) + 5)"), "(2*3) + 5");
}

#[test]
fn stringify_matches_upstream_stringify_rs_direct_cases() {
    // tests/ui/macros/stringify.rs `c1!` expectations, which also assert the
    // direct `stringify!($($tt)*)` form.
    let cases = [
        ("{}", "{}"),
        ("{ true }", "{ true }"),
        ("{ return; }", "{ return; }"),
        (
            "{\n            let _;\n            true\n        }",
            "{ let _; true }",
        ),
        ("[true,]", "[true,]"),
        ("f::<u8>()", "f::<u8>()"),
        ("f ::  < u8>( )", "f :: < u8>()"),
        ("f::<'a, u8, 1>()", "f::<'a, u8, 1>()"),
        ("()()", "()()"),
        ("x.collect::<Vec<_>>()", "x.collect::<Vec<_>>()"),
        ("(true, false,)", "(true, false,)"),
        (
            "a < 1 && 2 < b && c > 3 && 4 > d",
            "a < 1 && 2 < b && c > 3 && 4 > d",
        ),
        (
            "a + b * c - d + -1 * -2 - -3",
            "a + b * c - d + -1 * -2 - -3",
        ),
        ("move(expr)", "move(expr)"),
        ("1_000_i8", "1_000_i8"),
        ("if let _ = true && false {}", "if let _ = true && false {}"),
        (
            "if ::std::blah() { } else { }",
            "if ::std::blah() {} else {}",
        ),
        (
            "match self {\n            Ok => 1,\n            Err => 0,\n        }",
            "match self { Ok => 1, Err => 0, }",
        ),
        ("|x: u8| {}", "|x: u8| {}"),
        ("|| -> u8 { self }", "|| -> u8 { self }"),
        ("#[attr] {}", "#[attr] {}"),
        ("{\n                #![attr]\n            }", "{ #![attr] }"),
        ("expr.await", "expr.await"),
        ("lo .. hi", "lo .. hi"),
        ("-2..=-1", "-2..=-1"),
        ("<Self as T>::thing", "<Self as T>::thing"),
        ("Self::<'static>", "Self::<'static>"),
        ("&raw const expr", "&raw const expr"),
        ("break 'a true", "break 'a true"),
        ("mac! { ... }", "mac! { ... }"),
        ("mac![...]", "mac![...]"),
        ("Struct { x, ..base }", "Struct { x, ..base }"),
        ("[(); 0]", "[(); 0]"),
        ("expr?", "expr?"),
        ("do yeet 0", "do yeet 0"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            stringify(&format!("stringify!({input})")),
            expected,
            "input: {input}"
        );
    }
}

#[test]
fn stringify_long_streams_break_lines_like_rustc() {
    // Observed with rustc 1.99.0 (pp.rs margin 78).
    assert_eq!(
        stringify(
            "stringify!(aaaaaaaaaa bbbbbbbbbbbb cccccccccccc dddddddddddd eeeeeeeeeeee ffffffffff gggggggggg hhhhhhhhhh iiiiiiiii jjjjjjjjj)"
        ),
        "aaaaaaaaaa bbbbbbbbbbbb cccccccccccc dddddddddddd eeeeeeeeeeee ffffffffff\ngggggggggg hhhhhhhhhh iiiiiiiii jjjjjjjjj"
    );
    assert_eq!(
        stringify(
            "stringify!(fn f() { let x = 1; let y = 2; let zzzzzzzzzzzzzzzzzzz = 3; let wwwwwwwwwwwwwwww = 4; x + y })"
        ),
        "fn f()\n{\n    let x = 1; let y = 2; let zzzzzzzzzzzzzzzzzzz = 3; let wwwwwwwwwwwwwwww =\n    4; x + y\n}"
    );
}

#[test]
fn stringify_of_macro_transcribed_tokens_uses_rustc_template_spacing() {
    // Observed with rustc 1.99.0: template tokens are `Spacing::Alone`,
    // template delimiters keep their spacing, `]` is forced `Alone`, and an
    // `$e:expr` fragment is an invisible group.
    let cases = [
        ("() => { stringify!(a::b + c) };", "", "a :: b + c"),
        ("() => { stringify!({x} [a]b (c)d) };", "", "{x} [a] b(c)d"),
        ("($e:expr) => { stringify!($e+1) };", "a + b", "a + b + 1"),
        (
            "($x:tt, $y:tt) => { stringify!($x + $y) };",
            "a , b",
            "a + b",
        ),
        (
            "($($b:ident),* ; $m:ident ; $($e:ident),*) => { stringify!($($b,)* $m $(,$e)*) };",
            "a, b; c; d, e",
            "a, b, c, d, e",
        ),
    ];
    for (rules, argument, expected) in cases {
        let definition = source(
            &format!("macro_rules! m {{ {rules} }}"),
            CORE,
            Edition::E2021,
        );
        let [TokenTree::Group { children, .. }] = definition.roots() else {
            panic!()
        };
        let body = children
            .iter()
            .find(|t| {
                matches!(
                    t,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let body = TokenForest::new(
            vec![body],
            definition.contexts().to_vec(),
            definition.spans().to_vec(),
        )
        .unwrap();
        let declared = DeclarativeMacro::new(body).unwrap();
        let call = source(&format!("m!({argument});"), CALLER, Edition::E2021);
        let (args, call_site) = call_group(&call);
        let expanded = declared
            .expand_hygienic_with_limits(
                &args,
                call_site,
                DeclarativeSite {
                    expansion: Key([5; 32]),
                    defining_unit: CORE,
                    definition_edition: Edition::E2021,
                },
                1_000,
                1 << 20,
            )
            .unwrap()
            .0
            .unwrap()
            .tokens;
        let (input, inner_site) = call_group(&expanded);
        let output = expand_builtin(
            BuiltinKind::Stringify,
            &input,
            inner_site,
            &site(),
            &facts(Edition::E2021),
        )
        .unwrap()
        .unwrap()
        .tokens;
        assert_eq!(only_message(&output), expected, "rules: {rules}");
    }
}

#[test]
fn assert_messages_match_rustc_oracle_battery() {
    #[derive(serde::Deserialize)]
    struct Case {
        condition: String,
        message: String,
    }
    #[derive(serde::Deserialize)]
    struct Data {
        cases: Vec<Case>,
    }
    let data: Data = serde_json::from_str(include_str!("assert_oracle_cases.json")).unwrap();
    assert_eq!(data.cases.len(), 73);
    for case in data.cases {
        let forest = source(
            &format!("assert!({});", case.condition),
            CALLER,
            Edition::E2021,
        );
        let (input, call_site) = call_group(&forest);
        let output = expand_builtin(
            BuiltinKind::Assert,
            &input,
            call_site,
            &site(),
            &facts(Edition::E2021),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            only_message(&output.tokens),
            case.message,
            "{}",
            case.condition
        );
    }
}

#[test]
fn assert_message_converts_dollar_crate_like_rustc() {
    // rustc prints AST paths with `$crate` converted (`IdentPrinter`): the
    // current crate prints `crate`, an extern crate `::name`.
    for (name, expected) in [("crate", "crate::f() == 2"), ("core", "::core::f() == 2")] {
        let definition = source(
            "macro_rules! m { () => { assert!($crate::f() == 2) }; }",
            CORE,
            Edition::E2021,
        );
        let [TokenTree::Group { children, .. }] = definition.roots() else {
            panic!()
        };
        let body = children
            .iter()
            .find(|t| {
                matches!(
                    t,
                    TokenTree::Group {
                        delimiter: Delimiter::Brace,
                        ..
                    }
                )
            })
            .unwrap()
            .clone();
        let body = TokenForest::new(
            vec![body],
            definition.contexts().to_vec(),
            definition.spans().to_vec(),
        )
        .unwrap();
        let declared = DeclarativeMacro::new(body).unwrap();
        let call = source("m!();", CALLER, Edition::E2021);
        let (args, call_site) = call_group(&call);
        let expanded = declared
            .expand_hygienic_with_limits(
                &args,
                call_site,
                DeclarativeSite {
                    expansion: Key([5; 32]),
                    defining_unit: CORE,
                    definition_edition: Edition::E2021,
                },
                1_000,
                1 << 20,
            )
            .unwrap()
            .0
            .unwrap()
            .tokens;
        let (input, inner_site) = call_group(&expanded);
        let mut facts = facts(Edition::E2021);
        facts.crate_names.insert(CORE, name.into());
        let output = expand_builtin(BuiltinKind::Assert, &input, inner_site, &site(), &facts)
            .unwrap()
            .unwrap()
            .tokens;
        assert_eq!(
            only_message(&output),
            format!("assertion failed: {expected}")
        );
    }
}

/// Runs every `c1!` case of the unchanged upstream `tests/ui/macros/stringify.rs`
/// through `stringify!` (rustc asserts the direct `stringify!($($tt)*)` form equals
/// each expectation). The upstream file is not yet imported under
/// `fixtures/upstream/rustc-checkout`; point `LRR_UPSTREAM_STRINGIFY_RS` at it.
#[test]
#[ignore = "requires LRR_UPSTREAM_STRINGIFY_RS (upstream tests/ui/macros/stringify.rs)"]
fn stringify_matches_every_upstream_stringify_rs_case() {
    let path = std::env::var("LRR_UPSTREAM_STRINGIFY_RS").expect("LRR_UPSTREAM_STRINGIFY_RS");
    let text = std::fs::read_to_string(path).unwrap();
    let bytes = text.as_bytes();
    let mut cases = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("c1!(") {
        let start = from + found;
        let open = start + text[start..].find('[').unwrap();
        let mut depth = 0usize;
        let mut end = open;
        loop {
            match bytes[end] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            end += 1;
        }
        let rest = text[end + 1..].trim_start();
        let rest = rest.strip_prefix(',').unwrap().trim_start();
        let mut literal_end = 1;
        while rest.as_bytes()[literal_end] != b'"' {
            literal_end += if rest.as_bytes()[literal_end] == b'\\' {
                2
            } else {
                1
            };
        }
        let expected = tts::unquote_str(&rest[..=literal_end]).unwrap();
        cases.push((text[open + 1..end].to_owned(), expected));
        from = end;
    }
    assert_eq!(cases.len(), 360);
    for (body, expected) in cases {
        assert_eq!(
            stringify(&format!("stringify!({body})")),
            expected,
            "{body}"
        );
    }
}
