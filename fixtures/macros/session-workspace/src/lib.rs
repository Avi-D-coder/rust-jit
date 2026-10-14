//! Controlled, explicitly prepared host tools; never selected live runtime units.
use proc_macro::TokenStream;
use std::{str::FromStr, sync::atomic::{AtomicU64, Ordering}};

#[proc_macro]
pub fn identity(input: TokenStream) -> TokenStream { input }

#[proc_macro_derive(Empty)]
pub fn empty(item: TokenStream) -> TokenStream {
    if item.to_string().contains("MarkerPrefix") {
        TokenStream::from_str("prefix!(); macro_rules! prefix { () => {const SECOND_PREFIX: usize = 2;} } prefix!();").unwrap()
    } else {
        TokenStream::new()
    }
}

#[proc_macro_derive(r#gen)]
pub fn raw_gen(_: TokenStream) -> TokenStream { TokenStream::new() }

#[proc_macro]
pub fn relocate_tokens(input: TokenStream) -> TokenStream {
    let mut input = input.into_iter();
    let mut first = input.next().unwrap();
    let mut second = input.next().unwrap();
    assert!(input.next().is_none());
    let context = first.span();
    let location = second.span();
    first.set_span(context.located_at(location));
    second.set_span(location.resolved_at(context));
    let comma = proc_macro::TokenTree::Punct(proc_macro::Punct::new(',', proc_macro::Spacing::Alone));
    let contents = proc_macro::TokenStream::from_iter([first, comma, second]);
    proc_macro::TokenTree::Group(proc_macro::Group::new(proc_macro::Delimiter::Parenthesis, contents)).into()
}

#[proc_macro]
pub fn span_info(input: TokenStream) -> TokenStream {
    let span = input.into_iter().next().unwrap().span();
    let value = format!("{:?}|{}:{}|{}:{}|{}|{:?}",
        span.source_text(), span.line(), span.column(),
        span.start().line(), span.end().column(),
        span.file(), span.local_file());
    proc_macro::TokenTree::Literal(proc_macro::Literal::string(&value)).into()
}

#[proc_macro]
pub fn span_relocate_info(input: TokenStream) -> TokenStream {
    let mut input = input.into_iter();
    let first = input.next().unwrap().span();
    let second = input.next().unwrap().span();
    assert!(input.next().is_none());
    let mut result = String::new();
    for span in [first, second, first.resolved_at(second), first.located_at(second)] {
        result.push_str(&format!("{:?}|{}:{}|{}:{}|{}|{:?};",
            span.source_text(), span.line(), span.column(),
            span.start().line(), span.end().column(), span.file(), span.local_file()));
    }
    proc_macro::TokenTree::Literal(proc_macro::Literal::string(&result)).into()
}

#[proc_macro_attribute]
pub fn make_tests(_: TokenStream, _: TokenStream) -> TokenStream {
    TokenStream::from_str("#[test] fn generated_one() { assert_eq!(2_i64 + 3_i64, 5_i64); } #[test] fn generated_two() { assert_eq!(7_i64, 7_i64); }").unwrap()
}

#[proc_macro_attribute]
pub fn attribute_tokens(arguments: TokenStream, _: TokenStream) -> TokenStream { arguments }

#[proc_macro_attribute]
pub fn check_applied_absent(_: TokenStream, item: TokenStream) -> TokenStream {
    fn has_attribute(item: &TokenStream, name: &str) -> bool {
        let tokens: Vec<_> = item.clone().into_iter().collect();
        tokens.windows(2).any(|pair| matches!(&pair[0], proc_macro::TokenTree::Punct(p) if p.as_char()=='#')
            && matches!(&pair[1],proc_macro::TokenTree::Group(g) if g.delimiter()==proc_macro::Delimiter::Bracket
                && g.stream().into_iter().any(|t|matches!(t,proc_macro::TokenTree::Ident(id) if id.to_string()==name))))
    }
    assert!(!has_attribute(&item,"check_applied_absent"));
    assert!(!has_attribute(&item,"cfg"));
    assert!(!has_attribute(&item,"cfg_attr"));
    assert!(has_attribute(&item,"inline"));
    item
}

#[proc_macro_derive(EmitItems, attributes(lrr_helper))]
pub fn emit_items(_: TokenStream) -> TokenStream {
    TokenStream::from_str("fn generated_helper() -> i64 { 41_i64 } impl Mark for Other { fn mark(&self) -> i64 { 43_i64 } }").unwrap()
}

static COUNTER: AtomicU64 = AtomicU64::new(0);
#[proc_macro]
pub fn session_counter(_: TokenStream) -> TokenStream {
    let value = COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    TokenStream::from_str(&format!("{value}_u64")).unwrap()
}

#[proc_macro]
pub fn deliberately_panics(_: TokenStream) -> TokenStream { panic!("controlled fixture macro panic") }

#[proc_macro_attribute]
pub fn panic_attribute(_: TokenStream, _: TokenStream) -> TokenStream {
    panic!("controlled inactive attribute must not run")
}

#[proc_macro]
pub fn panics_with_diagnostic(_: TokenStream) -> TokenStream {
    eprintln!("controlled macro stderr diagnostic");
    panic!("controlled fixture macro panic")
}

#[proc_macro]
pub fn deliberately_crashes(_: TokenStream) -> TokenStream { std::process::abort() }

#[proc_macro]
pub fn deliberately_hangs(_: TokenStream) -> TokenStream { loop { std::thread::sleep(std::time::Duration::from_secs(30)); } }

#[proc_macro]
pub fn oversized(_: TokenStream) -> TokenStream { TokenStream::from_str(&format!("[{}]", "1_u64,".repeat(2048))).unwrap() }

#[proc_macro]
pub fn malformed(_: TokenStream) -> TokenStream { TokenStream::from_str("let").unwrap() }

#[proc_macro]
pub fn noisy_stdout(_: TokenStream) -> TokenStream {
    println!("{}", "x".repeat(32*1024));
    TokenStream::from_str("7_i64").unwrap()
}

#[proc_macro]
pub fn noisy_stderr(_: TokenStream) -> TokenStream {
    use std::io::Write;
    std::io::stderr().write_all(&vec![b'x';8*1024*1024]).unwrap();
    TokenStream::from_str("7_i64").unwrap()
}

#[proc_macro]
pub fn child_hangs(_: TokenStream) -> TokenStream {
    let child=std::process::Command::new("/bin/sleep").arg("30").spawn().unwrap();
    std::fs::write(std::env::var("LRR_MACRO_CHILD_PID_FILE").unwrap(),child.id().to_string()).unwrap();
    loop{std::thread::sleep(std::time::Duration::from_secs(30));}
}

#[proc_macro_derive(CheckFutureHelpers)]
pub fn check_future_helpers(item: TokenStream) -> TokenStream {
    let text = item.to_string();
    assert!(text.contains("derive") && text.contains("Empty"));
    assert!(!text.contains("First"));
    assert_eq!(text.matches("lrr_derive_helper").count(), 2);
    TokenStream::from_str("pub const FUTURE_HELPERS_SEEN: usize = 2_usize;").unwrap()
}

#[proc_macro_derive(CheckHelperCollision, attributes(lrr_derive_helper))]
pub fn check_helper_collision(item: TokenStream) -> TokenStream {
    let text = item.to_string();
    assert!(text.contains("derive") && text.contains("Empty2"));
    assert!(!text.contains("Empty)"));
    assert_eq!(text.matches("lrr_derive_helper").count(), 1);
    TokenStream::from_str("pub const COLLIDING_HELPER_SEEN: usize = 1_usize;").unwrap()
}

#[proc_macro_derive(SameHelper, attributes(lrr_derive_helper))]
pub fn same_helper(_: TokenStream) -> TokenStream { TokenStream::new() }

#[proc_macro_derive(CheckDeriveInput, attributes(lrr_derive_helper))]
pub fn check_derive_input(item: TokenStream) -> TokenStream {
    let source_append = item.to_string().contains("MarkerSourceAppend");
    let source_pending_outer = item.to_string().contains("MarkerSourcePendingOuter");
    let prefix = item.to_string().contains("MarkerPrefix");
    let later_attribute = item.to_string().contains("after_derive");
    if later_attribute { assert!(!item.to_string().contains("cold")); assert!(!item.to_string().contains("u16")); }
    let tokens: Vec<_> = item.into_iter().collect();
    let names: Vec<_> = tokens.windows(2).filter_map(|pair| {
        if matches!(&pair[0], proc_macro::TokenTree::Punct(p) if p.as_char() == '#') {
            if let proc_macro::TokenTree::Group(g) = &pair[1] {
                if g.delimiter() == proc_macro::Delimiter::Bracket {
                    if let Some(proc_macro::TokenTree::Ident(id)) = g.stream().into_iter().next() {
                        return Some(id.to_string());
                    }
                }
            }
        }
        None
    }).collect();
    if source_pending_outer {
        assert_eq!(names, ["repr", "lrr_derive_helper", "not_a_helper"]);
    } else if later_attribute {
        if names.len() == 4 && names[0] == "lrr_fixture_macros" && names[1] == "lrr_fixture_macros" {
            assert_eq!(names,["lrr_fixture_macros","lrr_fixture_macros","repr","lrr_derive_helper"]);
        } else
        if names.first().is_some_and(|name| name == "derive") {
            assert_eq!(names, ["derive", "lrr_fixture_macros", "repr", "lrr_derive_helper"]);
        } else {
            assert_eq!(names, ["lrr_fixture_macros", "repr", "lrr_derive_helper"]);
        }
    } else {
        if names.first().is_some_and(|name| name == "derive") {
            assert_eq!(names, ["derive", "repr", "lrr_derive_helper"]);
        } else {
            assert_eq!(names, ["repr", "lrr_derive_helper"]);
        }
    }
    let mut output = format!("const DERIVE_INPUT_ATTRIBUTES: usize = {}_usize;", names.len());
    if prefix {
        output.push_str("macro_rules! prefix { () => {const FIRST_PREFIX: usize = 1;} }");
    }
    if source_append {
        output.push_str("macro_rules! source_append_leaf { () => { pub fn source_append_child() -> i64 { 7 } } } source_append_leaf!();");
    }
    TokenStream::from_str(&output).unwrap()
}


#[proc_macro_attribute]
pub fn chain_begin(_: TokenStream, item: TokenStream) -> TokenStream {
    assert!(item.to_string().contains("chain_finish"));
    assert!(item.to_string().contains("undefined_cold"));
    item
}
#[proc_macro_attribute]
pub fn chain_finish(arguments: TokenStream, _: TokenStream) -> TokenStream {
    if arguments.to_string() == "helper_chain" {
        return TokenStream::from_str("#[derive(lrr_fixture_macros::CheckDeriveInput,lrr_fixture_macros::Empty)] #[lrr_fixture_macros::after_derive(identity)] #[lrr_fixture_macros::after_helper] #[repr(C)] #[lrr_derive_helper] pub struct Marker(#[lrr_derive_helper] pub u8, #[cfg(any())] pub u16);").unwrap();
    }
    if arguments.to_string() == "helper_module" {
        return TokenStream::from_str("#[derive(lrr_fixture_macros::CheckDeriveInput,lrr_fixture_macros::Empty)] #[lrr_fixture_macros::after_derive(module)] #[repr(C)] #[lrr_derive_helper] pub struct Marker(#[lrr_derive_helper] pub u8, #[cfg(any())] pub u16);").unwrap();
    }
    if arguments.to_string() == "helper_identity" || arguments.to_string() == "helper_bang" {
        let mode = if arguments.to_string() == "helper_bang" { "bang" } else { "identity" };
        return TokenStream::from_str(&format!("#[derive(lrr_fixture_macros::CheckDeriveInput,lrr_fixture_macros::Empty)] #[lrr_fixture_macros::after_derive({mode})] #[repr(C)] #[lrr_derive_helper] struct Marker(#[lrr_derive_helper] u8, #[cfg(any())] u16);")).unwrap();
    }
    if arguments.to_string() == "surround_empty" {
        return TokenStream::from_str("#[derive()] #[derive(lrr_fixture_macros::CheckDeriveInput,lrr_fixture_macros::Empty)] #[derive()] #[repr(C)] #[lrr_derive_helper] struct Marker;").unwrap();
    }
    if arguments.to_string() == "empty_attribute" {
        return TokenStream::from_str("#[derive()] #[lrr_fixture_macros::after_empty] struct Marker { #[cfg(any())] cold:u8 }").unwrap();
    }
    if arguments.to_string() == "later_attribute" {
        return TokenStream::from_str("#[derive(lrr_fixture_macros::CheckDeriveInput, lrr_fixture_macros::Empty)] #[lrr_fixture_macros::after_derive] #[repr(C)] #[lrr_derive_helper] struct Marker { #[cfg(any())] cold: u8 }").unwrap();
    }
    if arguments.to_string() == "later_groups" {
        return TokenStream::from_str("#[derive(lrr_fixture_macros::CheckDeriveInput)] #[derive(lrr_fixture_macros::Empty)] #[lrr_fixture_macros::after_derive] #[repr(C)] #[lrr_derive_helper] struct Marker { #[cfg(any())] cold: u8 }").unwrap();
    }
    let name = if arguments.to_string() == "prefix" { "MarkerPrefix" } else { "Marker" };
    TokenStream::from_str(&format!("#[derive(lrr_fixture_macros::CheckDeriveInput, lrr_fixture_macros::Empty)] #[repr(C)] #[lrr_derive_helper] struct {name};")).unwrap()
}

#[proc_macro_attribute]
pub fn after_derive(arguments: TokenStream, item: TokenStream) -> TokenStream {
    let text = item.to_string();
    assert!(!text.contains("derive("));
    assert!(!text.contains("CheckDeriveInput"));
    assert!((text.contains("cold") || text.contains("u16")) && text.contains("cfg"));
    assert!(text.contains("lrr_derive_helper") && text.contains("repr"));
    if arguments.to_string() == "identity" { return item; }
    if arguments.to_string() == "module" {
        let mut tokens: Vec<_> = TokenStream::from_str("pub mod helper_module {}").unwrap().into_iter().collect();
        let Some(proc_macro::TokenTree::Group(group)) = tokens.last_mut() else { panic!("module group absent") };
        let span = group.span();
        *group = proc_macro::Group::new(proc_macro::Delimiter::Brace,item);
        group.set_span(span);
        let mut output: TokenStream = tokens.into_iter().collect();
        output.extend(TokenStream::from_str("pub use helper_module::Marker;").unwrap());
        return output;
    }
    if arguments.to_string() == "bang" {
        let mut output = item;
        output.extend(TokenStream::from_str("macro_rules! leak {()=>{#[lrr_derive_helper] struct Leak;}} leak!();").unwrap());
        return output;
    }
    TokenStream::from_str("struct AfterDerived;").unwrap()
}

#[proc_macro_attribute]
pub fn after_empty(_: TokenStream, item: TokenStream) -> TokenStream {
    let text = item.to_string();
    assert!(!text.contains("derive"));
    assert!(text.contains("cold") && text.contains("cfg"));
    TokenStream::from_str("struct AfterEmpty;").unwrap()
}

#[proc_macro_attribute]
pub fn after_helper(_: TokenStream, item: TokenStream) -> TokenStream {
    let text = item.to_string();
    assert!(!text.contains("derive(") && !text.contains("CheckDeriveInput"));
    assert!(!text.contains("after_derive") && !text.contains("after_helper"));
    assert!(text.contains("lrr_derive_helper") && text.contains("u16") && text.contains("cfg"));
    item
}

#[proc_macro_derive(MemberCfg, attributes(member_helper))]
pub fn member_cfg(item: TokenStream) -> TokenStream {
    fn collect(stream: TokenStream, names: &mut Vec<String>) {
        for token in stream {
            match token {
                proc_macro::TokenTree::Ident(id) => names.push(id.to_string()),
                proc_macro::TokenTree::Group(g) => collect(g.stream(), names),
                _ => {}
            }
        }
    }
    let mut names = Vec::new(); collect(item, &mut names);
    TokenStream::from_str(&format!("const MEMBER_NAMES: &str = {:?};", names.join(","))).unwrap()
}

#[cfg_attr(all(), proc_macro_derive(r#type, attributes(first, r#type, first,)))]
pub fn source_facet_metadata(_: TokenStream) -> TokenStream { TokenStream::new() }

// Deliberately undeclared external input: controlled M012 non-hermetic fixture.
#[proc_macro]
pub fn read_external(input: TokenStream) -> TokenStream {
    let spelling = input.to_string();
    let path = spelling.strip_prefix('"').unwrap().strip_suffix('"').unwrap();
    TokenStream::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[proc_macro]
pub fn mutate_recorded_input(input: TokenStream) -> TokenStream {
    let spelling = input.to_string();
    let path = spelling.strip_prefix('"').unwrap().strip_suffix('"').unwrap();
    std::fs::write(path,"changed_by_macro").unwrap();
    TokenStream::from_str("31_u64").unwrap()
}
