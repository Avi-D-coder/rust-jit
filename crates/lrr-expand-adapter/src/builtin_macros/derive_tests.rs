//! Builtin derive expander tests. Expected shapes follow the pinned rustc
//! `rustc_builtin_macros/src/deriving/` sources, with the runtime-equivalent
//! differences documented in `derive.rs`.
use super::*;
use crate::token_wire::{ContextRecord, SourceIdentity, parse_source};

const CALLER: Key = Key([2; 32]);
const CORE: Key = Key([7; 32]);

fn site() -> BuiltinSite {
    BuiltinSite {
        expansion: Key([9; 32]),
        defining_unit: CORE,
        definition_edition: Edition::E2024,
    }
}

/// The item tokens in one invisible root group whose delimiters carry the
/// first token's span (the stand-in for the derive path's call site).
fn item(text: &str) -> (TokenForest, Key) {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: CALLER,
        edition: Edition::E2021,
    };
    let forest = parse_source(
        text,
        SourceIdentity {
            unit: CALLER,
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "test.rs".into(),
        },
        context,
        Key([3; 32]),
    )
    .unwrap();
    let roots = match forest.roots() {
        [TokenTree::Group {
            delimiter: Delimiter::Invisible,
            children,
            ..
        }] => children.clone(),
        roots => roots.to_vec(),
    };
    let call_site = forest.spans()[0].key();
    (
        TokenForest::new(
            vec![TokenTree::Group {
                delimiter: Delimiter::Invisible,
                open: call_site,
                close: call_site,
                children: roots,
            }],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap(),
        call_site,
    )
}

fn render(trees: &[TokenTree], out: &mut String) {
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
                    Delimiter::Invisible => ("", ""),
                };
                out.push_str(o);
                out.push(' ');
                render(children, out);
                out.push_str(c);
                out.push(' ');
            }
            TokenTree::Ident { text, .. } => {
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

fn derive_with(kind: BuiltinDeriveKind, text: &str, facts: DeriveContainerFacts) -> String {
    let (input, call_site) = item(text);
    let output = expand_builtin_derive(kind, &input, call_site, &site(), facts)
        .unwrap()
        .unwrap();
    output.tokens.validate().unwrap();
    let [TokenTree::Group { children, .. }] = output.tokens.roots() else {
        panic!("derive output root");
    };
    let mut out = String::new();
    render(children, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn derive(kind: BuiltinDeriveKind, text: &str) -> String {
    derive_with(kind, text, DeriveContainerFacts::default())
}

#[test]
fn clone_struct_fields_use_trait_paths_and_generic_bounds() {
    assert_eq!(
        derive(BuiltinDeriveKind::Clone, "struct P<T> { x: i32, y: T }"),
        "impl < T : $crate :: clone :: Clone > $crate :: clone :: Clone for P < T > \
         { fn clone ( & self ) -> Self { P { x : $crate :: clone :: Clone :: clone ( & self . x ) , \
         y : $crate :: clone :: Clone :: clone ( & self . y ) } } }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Clone, "pub struct W<'a, T: Copy = u8>(&'a T, pub T);"),
        "impl < 'a , T : $crate :: clone :: Clone + Copy > $crate :: clone :: Clone for W < 'a , T > \
         { fn clone ( & self ) -> Self { W ( $crate :: clone :: Clone :: clone ( & self . 0 ) , \
         $crate :: clone :: Clone :: clone ( & self . 1 ) ) } }"
    );
}

#[test]
fn clone_of_a_copy_container_without_type_parameters_is_a_copy() {
    let facts = DeriveContainerFacts {
        derives_copy: true,
        derives_ord: false,
    };
    assert_eq!(
        derive_with(BuiltinDeriveKind::Clone, "struct P { x: i32 }", facts),
        "impl $crate :: clone :: Clone for P { fn clone ( & self ) -> Self { * self } }"
    );
    assert!(
        derive_with(BuiltinDeriveKind::Clone, "struct P<T>(T);", facts)
            .contains("$crate :: clone :: Clone :: clone ( & self . 0 )")
    );
}

#[test]
fn clone_enum_matches_each_variant() {
    assert_eq!(
        derive(BuiltinDeriveKind::Clone, "enum E { A, B(u8, String), C { x: u8 } }"),
        "impl $crate :: clone :: Clone for E { fn clone ( & self ) -> Self { match self { \
         E :: A => E :: A , \
         E :: B ( __self_0 , __self_1 ) => E :: B ( $crate :: clone :: Clone :: clone ( __self_0 ) , \
         $crate :: clone :: Clone :: clone ( __self_1 ) ) , \
         E :: C { x : __self_0 } => E :: C { x : $crate :: clone :: Clone :: clone ( __self_0 ) } , } } }"
    );
}

#[test]
fn partial_eq_compares_scalar_fields_first() {
    assert_eq!(
        derive(BuiltinDeriveKind::PartialEq, "struct P { s: String, x: i32 }"),
        "impl $crate :: cmp :: PartialEq for P { fn eq ( & self , other : & Self ) -> bool { \
         self . x == other . x && self . s == other . s } }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::PartialEq, "struct U;"),
        "impl $crate :: cmp :: PartialEq for U { fn eq ( & self , other : & Self ) -> bool { true } }"
    );
}

#[test]
fn partial_eq_enum_checks_discriminants_then_fields() {
    let out = derive(BuiltinDeriveKind::PartialEq, "enum E { A = 3, B(u8), C }");
    assert_eq!(
        out,
        "impl $crate :: cmp :: PartialEq for E { fn eq ( & self , other : & Self ) -> bool { \
         let __self_discr : isize = match self { E :: A => ( 3 ) , E :: B ( .. ) => ( 3 ) + 1 , E :: C => ( 3 ) + 2 , } ; \
         let __arg1_discr : isize = match other { E :: A => ( 3 ) , E :: B ( .. ) => ( 3 ) + 1 , E :: C => ( 3 ) + 2 , } ; \
         __self_discr == __arg1_discr && match ( self , other ) { \
         ( E :: B ( __self_0 ) , E :: B ( __arg1_0 ) ) => * __self_0 == * __arg1_0 , _ => true , } } }"
    );
    let fieldless = derive(BuiltinDeriveKind::PartialEq, "#[repr(u8)] enum F { X, Y }");
    assert!(fieldless.contains("let __self_discr : u8 = match self { F :: X => 0 , F :: Y => 1 , } ;"));
    assert!(fieldless.ends_with("__self_discr == __arg1_discr } }"));
}

#[test]
fn ord_derives_are_lexicographic_and_partial_ord_uses_derived_ord() {
    assert_eq!(
        derive(BuiltinDeriveKind::Ord, "struct P { a: u8, b: u8 }"),
        "impl $crate :: cmp :: Ord for P { fn cmp ( & self , other : & Self ) -> $crate :: cmp :: Ordering { \
         match $crate :: cmp :: Ord :: cmp ( & self . a , & other . a ) { \
         $crate :: cmp :: Ordering :: Equal => $crate :: cmp :: Ord :: cmp ( & self . b , & other . b ) , \
         cmp => cmp , } } }"
    );
    let facts = DeriveContainerFacts {
        derives_copy: false,
        derives_ord: true,
    };
    assert_eq!(
        derive_with(BuiltinDeriveKind::PartialOrd, "struct P { a: u8 }", facts),
        "impl $crate :: cmp :: PartialOrd for P { fn partial_cmp ( & self , other : & Self ) -> \
         $crate :: option :: Option < $crate :: cmp :: Ordering > { $crate :: option :: Option :: Some ( \
         $crate :: cmp :: Ord :: cmp ( self , other ) ) } }"
    );
    let general = derive(BuiltinDeriveKind::PartialOrd, "enum E { A(u8), B }");
    assert!(general.contains(
        "match $crate :: cmp :: PartialOrd :: partial_cmp ( & __self_discr , & __arg1_discr ) { \
         $crate :: option :: Option :: Some ( $crate :: cmp :: Ordering :: Equal ) => match ( self , other ) {"
    ));
}

#[test]
fn hash_debug_default_copy_and_eq_shapes() {
    assert_eq!(
        derive(BuiltinDeriveKind::Hash, "struct P(u8);"),
        "impl $crate :: hash :: Hash for P { fn hash < __H : $crate :: hash :: Hasher > ( & self , state : & mut __H ) -> ( ) { \
         $crate :: hash :: Hash :: hash ( & self . 0 , state ) ; } }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Debug, "struct P { x: u8, y: str }"),
        "impl $crate :: fmt :: Debug for P { fn fmt ( & self , f : & mut $crate :: fmt :: Formatter ) -> $crate :: fmt :: Result { \
         { let mut __builder = $crate :: fmt :: Formatter :: debug_struct ( f , \"P\" ) ; \
         $crate :: fmt :: DebugStruct :: field ( & mut __builder , \"x\" , & self . x ) ; \
         $crate :: fmt :: DebugStruct :: field ( & mut __builder , \"y\" , & & self . y ) ; \
         $crate :: fmt :: DebugStruct :: finish ( & mut __builder ) } } }"
    );
    assert!(
        derive(BuiltinDeriveKind::Debug, "enum E { A, B }").contains(
            "$crate :: fmt :: Formatter :: write_str ( f , match self { E :: A => \"A\" , E :: B => \"B\" , } )"
        )
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Default, "enum E { A(u8), #[default] B }"),
        "impl $crate :: default :: Default for E { fn default ( ) -> Self { E :: B } }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Default, "struct D<T> { a: T }"),
        "impl < T : $crate :: default :: Default > $crate :: default :: Default for D < T > { fn default ( ) -> Self { \
         D { a : $crate :: default :: Default :: default ( ) } } }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Copy, "struct C<T> where T: Sized;"),
        "impl < T : $crate :: marker :: Copy > $crate :: marker :: Copy for C < T > where T : Sized { }"
    );
    assert_eq!(
        derive(BuiltinDeriveKind::Eq, "enum Z {}"),
        "impl $crate :: cmp :: Eq for Z { }"
    );
}

#[test]
fn generated_names_use_the_def_site_and_user_names_keep_their_spans() {
    let (input, call_site) = item("struct P { x: i32 }");
    let output = expand_builtin_derive(
        BuiltinDeriveKind::Clone,
        &input,
        call_site,
        &site(),
        DeriveContainerFacts::default(),
    )
    .unwrap()
    .unwrap()
    .tokens;
    let user: std::collections::BTreeSet<_> = input.spans().iter().map(|s| s.key()).collect();
    let mut seen_user = Vec::new();
    let mut seen_def = Vec::new();
    let mut stack: Vec<&TokenTree> = output.roots().iter().collect();
    while let Some(tree) = stack.pop() {
        match tree {
            TokenTree::Group { children, .. } => stack.extend(children),
            TokenTree::Ident { text, span, .. } => {
                if user.contains(span) {
                    seen_user.push(text.clone());
                } else {
                    seen_def.push(text.clone());
                }
            }
            _ => {}
        }
    }
    seen_user.sort();
    assert_eq!(seen_user, ["P", "P", "x", "x"]);
    assert!(seen_def.iter().any(|t| t == "$crate"));
    assert!(seen_def.iter().any(|t| t == "self"));
}

#[test]
fn unsupported_inputs_fail_like_rustc() {
    let (input, call_site) = item("union U { a: u8 }");
    let failure = expand_builtin_derive(
        BuiltinDeriveKind::Debug,
        &input,
        call_site,
        &site(),
        DeriveContainerFacts::default(),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(failure.kind, BuiltinFailureKind::Syntax);
    let (input, call_site) = item("enum E { A, B }");
    assert!(
        expand_builtin_derive(
            BuiltinDeriveKind::Default,
            &input,
            call_site,
            &site(),
            DeriveContainerFacts::default(),
        )
        .unwrap()
        .is_err()
    );
}
