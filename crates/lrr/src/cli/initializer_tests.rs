//! Constant and static initializers run through the ordinary interpreter
//! (E-R5-007): reached `const`/`static` items build nominal values, take
//! references and call `const fn`s as initializer frames; `&CONST` and
//! literal borrows promote to run-long read-only storage, and byte-string
//! literals publish their `[u8; N]` once. By-value methods copy Copy
//! aggregates (temporaries, consts) like any other read.
use super::format_trait_facade_tests::prints;

#[test]
fn const_initializers_build_nominal_values_and_call_const_fns() {
    prints(
        "const-init",
        "#[derive(Clone, Copy)]
enum Shape { Dot, Line(i32), Rect { w: i32, h: i32 } }
struct Config { name: &'static str, sizes: &'static [u32], scale: i32 }
const N: i32 = 5;
const PAIR: (i32, bool) = (N * 2, true);
const SHAPES: [Shape; 3] = [Shape::Dot, Shape::Line(N), Shape::Rect { w: 2, h: 3 }];
const CFG: Config = Config { name: \"cfg\", sizes: &[1, 2, 3], scale: -N };
const NAMES: &[&str] = &[\"a\", \"bc\"];
const LEN: usize = \"hello\".len();
const fn double(x: i32) -> i32 { x * 2 }
const D: i32 = double(21);
const O: Option<i32> = Some(5);
macro_rules! make_const {
    ($name:ident, $value:expr) => { const $name: Option<i32> = Some($value); };
}
make_const!(GEN, N + 1);
fn area(s: Shape) -> i32 {
    match s { Shape::Dot => 0, Shape::Line(n) => n, Shape::Rect { w, h } => w * h }
}
fn main() {
    println!(\"{} {:?} {}\", N, PAIR, LEN);
    let mut total = 0;
    let mut i = 0;
    while i < SHAPES.len() { total += area(SHAPES[i]); i += 1; }
    println!(\"{} {}\", total, area(SHAPES[1]));
    println!(\"{} {} {} {}\", CFG.name, CFG.sizes.len(), CFG.sizes[2], CFG.scale);
    println!(\"{} {}\", NAMES.len(), NAMES[1]);
    println!(\"{} {}\", D, GEN.unwrap());
    println!(\"{} {}\", O.unwrap(), Some(7).unwrap());
    assert_eq!(GEN, Some(6));
}
",
        "5 (10, true) 5\n11 5\ncfg 3 3 -5\n2 bc\n42 6\n5 7\n",
    );
}

#[test]
fn static_initializers_are_run_long_storage_with_mutation() {
    prints(
        "static-init",
        "static GREETING: &str = \"hi\";
static TABLE: [u8; 4] = [1, 2, 3, 4];
static mut COUNTER: u32 = 0;
struct Point { x: i32, y: i32 }
static ORIGIN: Point = Point { x: 3, y: -4 };
fn bump() -> u32 {
    unsafe { COUNTER += 1; COUNTER }
}
fn main() {
    println!(\"{} {}\", GREETING, TABLE[2]);
    let r: &'static [u8; 4] = &TABLE;
    println!(\"{}\", r.len());
    bump(); bump();
    println!(\"{}\", bump());
    println!(\"{} {}\", ORIGIN.x, ORIGIN.y);
    let p = &ORIGIN;
    println!(\"{}\", p.x + p.y);
}
",
        "hi 3\n4\n3\n3 -4\n-1\n",
    );
}

#[test]
fn promoted_borrows_byte_strings_and_copy_receivers() {
    prints(
        "promotion",
        "#[derive(Clone, Copy)]
struct P { x: i32, y: i32 }
impl P { fn sum(self) -> i32 { self.x + self.y } }
const PP: P = P { x: 1, y: 2 };
fn mk() -> Option<i32> { Some(7) }
fn digits() -> &'static [i32] { &[1, 2, 3] }
fn bytes() -> &'static [u8] { b\"abc\" }
fn main() {
    let p = P { x: 3, y: 4 };
    println!(\"{} {} {}\", p.sum(), p.sum(), PP.sum());
    println!(\"{} {}\", mk().unwrap(), Some(5).unwrap());
    let d = digits();
    println!(\"{} {}\", d.len(), d[2]);
    let b = bytes();
    println!(\"{} {}\", b.len(), b[0]);
    let arr: &[u8; 2] = b\"hi\";
    println!(\"{:?} {:?}\", arr, b\"\\x01z\");
    let x: &'static i32 = &42;
    println!(\"{}\", *x + 1);
}
",
        "7 7 3\n7 5\n3 3\n3 97\n[104, 105] [1, 122]\n43\n",
    );
}

#[test]
fn shared_const_borrows_promote_plain_data_only() {
    // `&CONST` and `&LITERAL_STRUCT` get `'static` storage when the value has
    // no interior mutability or drop glue; a `&mut`, a `Cell`/`Drop` value
    // and a by-value copy are fresh temporaries per use, as in rustc.
    prints(
        "const-promotion",
        "use std::cell::Cell;
struct P { x: i32 }
struct D(u8);
impl Drop for D { fn drop(&mut self) { println!(\"drop {}\", self.0); } }
const A: [u8; 1] = [104];
const C: &&&&[u8; 1] = & & & &A;
const Q: P = P { x: 9 };
const CELL: Cell<i32> = Cell::new(1);
const ARR: [i32; 3] = [3, 1, 2];
fn first() -> &'static [u8; 1] { &A }
fn get() -> &'static P { &Q }
fn lit() -> &'static P { &P { x: 3 } }
fn main() {
    println!(\"{} {} {} {}\", C[0], first()[0], get().x, lit().x);
    CELL.set(5);
    println!(\"{}\", CELL.get());
    let r = &CELL;
    r.set(7);
    println!(\"{} {}\", r.get(), CELL.get());
    let mut b = ARR;
    b[0] = 9;
    println!(\"{:?} {:?}\", ARR, b);
    let m = &mut ARR;
    m[1] = 7;
    println!(\"{:?} {}\", ARR, m[1]);
    let n = { let d = &D(4); d.0 };
    println!(\"{}\", n);
}
",
        "104 104 9 3\n1\n7 1\n[3, 1, 2] [9, 1, 2]\n[3, 1, 2] 7\ndrop 4\n4\n",
    );
}

#[test]
fn local_initializers_share_the_vm_across_source_origins() {
    const FUNCTION: &str = "fn local() -> u8 {
        const C: Option<u8> = Some(7);
        static S: Option<u8> = Some(9);
        C.unwrap() + S.unwrap()
    }";
    const MAIN: &str = "fn main() { println!(\"{} {}\", local(), local()); }";
    for (label, source) in [
        ("local-init", format!("{FUNCTION}\n{MAIN}")),
        (
            "local-init-generated",
            format!("macro_rules! make {{ () => {{ {FUNCTION} }} }} make!(); {MAIN}"),
        ),
        (
            "local-init-wrapper",
            format!(
                "macro_rules! make {{ () => {{ {FUNCTION} }} }} macro_rules! wrap {{ () => {{ make!(); }} }} wrap!(); {MAIN}"
            ),
        ),
    ] {
        prints(label, &source, "16 16\n");
    }
}
