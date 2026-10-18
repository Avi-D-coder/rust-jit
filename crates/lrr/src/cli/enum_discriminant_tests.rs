//! Enum discriminant expressions run through the ordinary interpreter
//! (E-R5-013): an explicit `= expr` that is not a plain integer literal is an
//! anonymous constant of the enum's representation type, demanded by the
//! first `as` cast that needs it like any reached `const`. Expected output
//! is real `rustc +1.99.0` output (all four editions agree).
use super::format_trait_facade_tests::prints;

/// The forms the pure evaluator never covered (shifts, arithmetic over constants, casts, blocks, closures, array indexing) and implicit successors after them.
#[test]
fn expression_discriminants_in_every_written_form() {
    prints(
        "enum-discriminant-forms",
        r##"const BASE: isize = 10;
const FOO: isize = 42;
enum E { A = 1 << 3, B = BASE + 2, C, D = (3 * 5) as isize, F = -1 }
enum Foo { X = 42, Y = Foo::X as isize - 3 }
enum Bar { Bar = { let x = 1; 3 } }
enum G { Boo = *[&FOO; 4][3] }
enum H { Bar = (|x: i32| { }, 42).1 }
#[repr(u8)]
enum I { X = b'a', Y = 'b' as u8, Z }
fn main() {
    println!("{} {} {} {} {}", E::A as i32, E::B as i32, E::C as i32, E::D as i32, E::F as i32);
    println!("{} {}", Foo::X as isize, Foo::Y as isize);
    println!("{}", Bar::Bar as u8);
    println!("{}", G::Boo as isize);
    println!("{}", H::Bar as usize);
    println!("{} {} {}", I::X as u8, I::Y as u8, I::Z as u8);
}
"##,
        "8 12 13 15 -1\n42 39\n3\n42\n42\n97 98 99\n",
    );
}

/// Negative values, each repr width, the u64 range above i64::MAX, and casts to narrower and wider targets.
#[test]
fn repr_integer_discriminants_wrap_and_truncate_in_casts() {
    prints(
        "enum-discriminant-repr",
        r##"enum Signed { Neg = -(1 << 4), Next, Shift = -5 >> 1, Last }
#[repr(i8)]
enum Small { Low = -128 + 1, Mid = 0 - 3, High = 100 + 27 }
#[repr(u8)]
enum Byte { Zero = 255 - 255, Max = 128 + 127, Wrapped = 200 + 50 }
#[repr(i32)]
enum Wide { Big = i32::MAX - 1, Negative = 2 - 5, Plain = 7 }
#[repr(u64)]
enum Huge { Shifted = 1 << 40, Top = 0xFFFF_FFFF_FFFF_FFFF, Plain = 1 }
#[repr(u16)]
enum Mixed { First = 256 * 2, Second }
fn main() {
    println!("{} {} {} {}", Signed::Neg as isize, Signed::Next as isize, Signed::Shift as isize, Signed::Last as isize);
    println!("{} {} {}", Small::Low as i8, Small::Mid as i8, Small::High as i8);
    println!("{} {} {}", Small::Mid as u8, Small::Low as u32, Small::Mid as i64);
    println!("{} {} {}", Byte::Zero as u8, Byte::Max as u8, Byte::Wrapped as u8);
    println!("{} {} {}", Byte::Max as i8, Byte::Max as i32, Byte::Wrapped as i16);
    println!("{} {} {}", Wide::Big as i32, Wide::Negative as i32, Wide::Plain as i32);
    println!("{} {} {}", Wide::Negative as u32, Wide::Negative as i64, Wide::Negative as usize);
    println!("{} {}", Huge::Shifted as u64, Huge::Top as u64);
    println!("{} {} {}", Huge::Top as i64, Huge::Top as u32, Huge::Top as usize);
    println!("{} {}", Mixed::First as u16, Mixed::Second as u16);
}
"##,
        "-16 -15 -3 -2\n-127 -3 127\n253 4294967169 -3\n0 255 250\n-1 255 250\n2147483646 -3 7\n4294967293 -3 18446744073709551613\n1099511627776 18446744073709551615\n-1 4294967295 18446744073709551615\n512 513\n",
    );
}

/// Discriminants naming other discriminants (earlier, later and through chains), const fn calls, and control flow, in the enum body scope.
#[test]
fn discriminant_expressions_name_constants_enums_calls_and_control_flow() {
    prints(
        "enum-discriminant-names",
        r##"const ONE: isize = 1;
const fn pick(flag: bool) -> isize { if flag { 100 } else { 200 } }
const MASK: u8 = 0b1010;
enum Rank { Low = 3, High = Rank::Low as isize * 10 }
enum Late { First = Late::Second as isize * 2, Second = 9, Third }
enum Chain { A = Rank::High as isize + 1, B, C = Chain::A as isize + Chain::B as isize }
enum Calls { Yes = pick(true), No = pick(false), Both = pick(true) + pick(false) }
enum Control {
    Branch = if ONE > 0 { 5 } else { 6 },
    Looped = { let mut n = 0; let mut i = 0; while i < 4 { n += i; i += 1; } n },
    Matched = match ONE { 1 => 11, _ => 12 },
    Masked = (MASK & 0b0110) as isize,
}
enum Shapes { Tuple = (1, 2, 3).2, Array = [4, 5, 6][1], Cast = 4.9f64 as isize, Neg = -(-7), Not = !0 + 10 }
fn main() {
    println!("{} {}", Rank::Low as isize, Rank::High as isize);
    println!("{} {} {}", Late::First as isize, Late::Second as isize, Late::Third as isize);
    println!("{} {} {}", Chain::A as isize, Chain::B as isize, Chain::C as isize);
    println!("{} {} {}", Calls::Yes as isize, Calls::No as isize, Calls::Both as isize);
    println!("{} {} {} {}", Control::Branch as isize, Control::Looped as isize, Control::Matched as isize, Control::Masked as isize);
    println!("{} {} {} {} {}", Shapes::Tuple as isize, Shapes::Array as isize, Shapes::Cast as isize, Shapes::Neg as isize, Shapes::Not as isize);
}
"##,
        "3 30\n18 9 10\n31 32 63\n100 200 300\n5 6 11 2\n3 5 4 7 9\n",
    );
}

/// A discriminant evaluated inside another initializer, derived comparison and hashing, nested modules and by-reference casts.
#[test]
fn discriminants_flow_into_consts_statics_derives_modules_and_collections() {
    prints(
        "enum-discriminant-flow",
        r##"use std::collections::HashMap;
mod lanes {
    pub const BASE: isize = 40;
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Lane { Left = BASE + 2, Center = (BASE + 2) * 2, Right }
    pub mod inner {
        #[derive(Clone, Copy, Debug)]
        pub enum Gear { Park = super::BASE >> 3, Drive = 1 << 5 }
    }
}
use lanes::Lane;
const SUM: isize = Lane::Left as isize + Lane::Right as isize;
static LAST: isize = Lane::Right as isize;
const TABLE: [isize; 3] = [Lane::Left as isize, Lane::Center as isize, Lane::Right as isize];
fn tag(l: Lane) -> isize { l as isize }
fn by_ref(l: &Lane) -> i64 { *l as i64 }
fn main() {
    println!("{} {}", SUM, LAST);
    println!("{:?}", TABLE);
    println!("{} {} {}", tag(Lane::Left), tag(Lane::Center), by_ref(&Lane::Right));
    println!("{} {}", lanes::inner::Gear::Park as u8, lanes::inner::Gear::Drive as u8);
    let mut lanes = vec![Lane::Right, Lane::Left, Lane::Center];
    lanes.sort();
    println!("{:?}", lanes);
    println!("{}", Lane::Right > Lane::Left);
    let mut counts: HashMap<Lane, usize> = HashMap::new();
    for l in [Lane::Left, Lane::Right, Lane::Left] { *counts.entry(l).or_insert(0) += 1; }
    println!("{} {}", counts[&Lane::Left], counts[&Lane::Right]);
    let mut total: isize = 0;
    for l in &lanes { total += *l as isize; }
    println!("{}", total);
    assert_eq!(Lane::Center as isize, 84);
    assert_eq!(lanes::inner::Gear::Drive as isize, 32);
}
"##,
        "127 85\n[42, 84, 85]\n42 84 85\n5 32\n[Left, Center, Right]\ntrue\n2 1\n211\n",
    );
}

/// The cast reads the same table whether it is written by hand, emitted by a
/// user macro, passed through a wrapper macro, or nested in a standard macro.
#[test]
fn discriminant_casts_nested_in_user_macros_wrappers_and_standard_macros() {
    prints(
        "enum-discriminant-macros",
        r##"const STEP: isize = 4;
#[derive(Clone, Copy)]
enum Level { Low = STEP * 2, High = (STEP << 2) + 1, Top }
macro_rules! tag { ($e:expr) => { $e as isize } }
macro_rules! shifted { ($e:expr, $by:expr) => { ($e as isize) << $by } }
macro_rules! via_wrapper { ($e:expr) => { tag!($e) + 1 } }
fn level_tag(l: Level) -> isize { l as isize }
fn main() {
    assert_eq!(Level::Low as isize, 8);
    assert_ne!(Level::High as u8, 0);
    println!("{} {} {}", tag!(Level::Low), tag!(Level::High), tag!(Level::Top));
    println!("{}", shifted!(Level::Low, 2));
    println!("{}", via_wrapper!(Level::Top));
    println!("{}", format!("{} {}", Level::High as u16, level_tag(Level::Top)));
    let all = [Level::Low, Level::High, Level::Top];
    let mut sum = 0;
    for l in all.iter() { sum += *l as isize; }
    println!("{}", sum);
    let picked = match Level::High { Level::Low => 1, Level::High => 2, Level::Top => 3 };
    println!("{}", picked);
}
"##,
        "8 17 18\n32\n19\n17 18\n43\n2\n",
    );
}

/// Block-local enums (in functions, nested blocks and closures, written or
/// emitted by a statement macro whose `expr` fragment is the discriminant)
/// are admitted from their declaration like the members of a block-level
/// impl, and run through the same initializer route.
#[test]
fn block_local_enum_discriminants_written_or_emitted_by_a_statement_macro() {
    prints(
        "enum-discriminant-local",
        r##"const BASE: isize = 10;
macro_rules! make_enum {
    ($name:ident, $first:expr) => {
        #[allow(dead_code)]
        enum $name { A = $first, B = BASE + 2, C }
    };
}
fn helper() -> isize {
    #[repr(u8)]
    enum Inner { P = 3 * 5, Q }
    Inner::Q as isize + Inner::P as isize
}
fn emitted() -> i32 {
    make_enum!(E, 1 << 3);
    E::A as i32 * 100 + E::B as i32 * 10 + E::C as i32
}
fn main() {
    enum Stuff { Bar = BASE * 2, Baz = Stuff::Bar as isize + 1 }
    println!("{} {}", Stuff::Bar as isize, Stuff::Baz as isize);
    println!("{}", helper());
    println!("{}", emitted());
    {
        enum Nested { X = 1 << 4, Y }
        println!("{} {}", Nested::X as u8, Nested::Y as u8);
    }
    let f = || { enum InClosure { Z = 7 * 6 } InClosure::Z as i32 };
    println!("{}", f());
    let g = |n: isize| { enum Step { One = 1 << 1, Two = 2 << 1 } n * Step::Two as isize + Step::One as isize };
    println!("{} {}", g(3), g(4));
}
"##,
        "20 21\n31\n933\n16 17\n42\n14 18\n",
    );
}
