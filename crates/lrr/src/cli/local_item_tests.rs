//! Items declared inside function bodies and block-level `use` declarations
//! run through the same entered lexical Value goal as module items: a block
//! item or import shadows module items, is selected from its owning body, and
//! reaches the one constructor, pattern and call services that module items
//! use. Expected output is the program's output under real rustc
//! (`rustc +1.99.0`); each program runs in all four editions.
use super::format_trait_facade_tests::prints;

#[test]
fn block_local_tuple_struct_constructs_and_projects() {
    prints(
        "local-tuple-struct",
        r#"fn main() {
    struct T(i32);
    let t = T(5);
    println!("{}", t.0);
}
"#,
        "5\n",
    );
}

#[test]
fn block_local_record_struct_patterns() {
    prints(
        "local-record-pattern",
        r#"fn main() {
    struct Foo { x: isize, y: isize }
    let mut f = Foo { x: 10, y: 0 };
    match f {
        Foo { x, .. } => println!("{}", x),
    }
    match f {
        Foo { ref mut x, .. } => *x = 11,
    }
    println!("{}", f.x + f.y);
}
"#,
        "10\n11\n",
    );
}

#[test]
fn block_local_enum_variants_construct_and_match() {
    prints(
        "local-enum",
        r#"fn main() {
    enum E { A, B(i32), C(i32, i32) }
    let x = E::B(2);
    match x {
        E::B(n) => println!("{}", n),
        _ => {}
    }
    let y = E::A;
    match y {
        E::A => println!("a"),
        _ => println!("other"),
    }
    match E::C(3, 4) {
        E::C(p, q) => println!("{}", p * q),
        _ => {}
    }
}
"#,
        "2\na\n12\n",
    );
}

#[test]
fn block_local_enums_in_closures_cast_to_their_discriminant() {
    prints(
        "local-enum-closure",
        r#"fn main() {
    let one = || {
        enum r { a, b }
        r::b as usize
    };
    let two = || {
        enum r { a }
        r::a as usize
    };
    println!("{} {}", one(), two());
}
"#,
        "1 0\n",
    );
}

#[test]
fn block_level_named_imports_select_variants_and_functions() {
    prints(
        "block-use-named",
        r#"enum E { A, B(i32) }
mod m {
    pub fn f() -> i32 { 7 }
}
fn main() {
    use E::A;
    use E::B;
    use m::f;
    let _x = A;
    match B(3) {
        B(n) => println!("{} {}", n, f()),
        A => println!("a"),
    }
}
"#,
        "3 7\n",
    );
}

#[test]
fn block_level_glob_imports_do_not_block_later_macros() {
    prints(
        "block-use-glob",
        r#"mod m {
    pub fn f() -> i32 { 7 }
    pub fn g() -> i32 { 8 }
}
enum E { A, B(i32) }
fn main() {
    use m::*;
    println!("{} {}", f(), g());
    use self::E::*;
    match B(3) {
        B(n) => println!("{}", n),
        A => println!("a"),
    }
}
"#,
        "7 8\n3\n",
    );
}

#[test]
fn module_level_type_alias_to_a_primitive_annotates_a_local() {
    prints(
        "alias-annotation",
        r#"type N = i32;
fn main() {
    let x: N = 3;
    println!("{}", x + 1);
}
"#,
        "4\n",
    );
}

#[test]
fn block_local_enum_record_variants_construct_and_match() {
    prints(
        "local-enum-record",
        r#"fn main() {
    enum E { A { v: i32, w: i32 }, B }
    let x = E::A { v: 3, w: 4 };
    match x {
        E::A { v, w } => println!("{} {}", v, w),
        E::B => println!("b"),
    }
    let y = E::B;
    match y {
        E::A { v, .. } => println!("{}", v),
        E::B => println!("b"),
    }
}
"#,
        "3 4\nb\n",
    );
}

#[test]
fn block_level_imports_of_enums_modules_and_renamed_functions() {
    prints(
        "block-use-roots",
        r#"mod m {
    pub enum Color { Red, Green(i32) }
    pub const K: i32 = 41;
    pub fn f() -> i32 { 1 }
}
fn main() {
    use m::Color;
    use m::f as g;
    use m as mm;
    use std::mem;
    let c = Color::Green(5);
    match c {
        Color::Red => println!("r"),
        Color::Green(n) => println!("{}", n),
    }
    let mut a = 1;
    let mut b = 2;
    mem::swap(&mut a, &mut b);
    println!("{} {} {} {}", a, b, g(), mm::f() + mm::K);
}
"#,
        "5\n2 1 1 42\n",
    );
}

#[test]
fn block_level_imports_of_library_functions() {
    prints(
        "block-use-library",
        r#"fn main() {
    use std::cmp::{max, min};
    println!("{} {}", max(1, 2), min(5, 3));
}
"#,
        "2 3\n",
    );
}

#[test]
fn block_items_in_closures_nested_functions_and_inner_blocks() {
    prints(
        "local-items-nested",
        r#"fn main() {
    let f = |x: i32| {
        struct W(i32);
        let w = W(x);
        w.0 + 1
    };
    println!("{}", f(4));
    fn inner() -> i32 {
        enum E { A(i32) }
        match E::A(9) { E::A(n) => n }
    }
    println!("{}", inner());
    {
        use std::cmp::min;
        println!("{}", min(5, 3));
    }
}
"#,
        "5\n9\n3\n",
    );
}

#[test]
fn macro_emitted_struct_import_and_enum_inside_a_body() {
    prints(
        "local-items-macro",
        r#"macro_rules! decl {
    () => {
        struct T(i32);
        use std::cmp::max;
        enum E { A, B(i32) }
    };
}
macro_rules! wrap { () => { decl!(); } }
fn main() {
    wrap!();
    let t = T(3);
    match E::B(t.0) {
        E::B(n) => println!("{}", max(n, 2)),
        E::A => println!("a"),
    }
}
"#,
        "3\n",
    );
}

#[test]
fn block_local_type_alias_to_a_builtin_annotates_a_local() {
    prints(
        "local-alias-annotation",
        r#"fn main() {
    type N = i32;
    let x: N = 3;
    println!("{}", x + 1);
}
"#,
        "4\n",
    );
}

/// The same block-local enum written by hand, emitted by a user macro and
/// emitted through a wrapper macro must take the one route and agree.
#[test]
fn block_items_match_handwritten_user_macro_and_wrapper_macro() {
    let expected = "7\na\n";
    let body = r#"
    let x = E::B(7);
    match x {
        E::B(n) => println!("{}", n),
        E::A => println!("a"),
    }
    match E::A {
        E::B(_) => println!("b"),
        E::A => println!("a"),
    }
"#;
    prints(
        "local-enum-handwritten",
        &format!("fn main() {{\n    enum E {{ A, B(i32) }}\n{body}}}\n"),
        expected,
    );
    prints(
        "local-enum-user-macro",
        &format!(
            "macro_rules! decl {{ () => {{ enum E {{ A, B(i32) }} }} }}\nfn main() {{\n    decl!();\n{body}}}\n"
        ),
        expected,
    );
    prints(
        "local-enum-wrapper-macro",
        &format!(
            "macro_rules! decl {{ () => {{ enum E {{ A, B(i32) }} }} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nfn main() {{\n    wrap!();\n{body}}}\n"
        ),
        expected,
    );
}
