//! The `?` operator through the ordinary standalone pipeline with the LRR
//! standard facade. `expr?` is rustc's `match Try::branch(expr) {
//! Continue(val) => val, Break(r) => return FromResidual::from_residual(r) }`:
//! the `Try::branch` call, the `Continue(val)` test of the one pattern
//! transaction and the early `return` (locals cleaned up, `From` conversion
//! observable) all run through the ordinary call, pattern and return
//! machinery. Expected outputs are rustc 1.99.0's (natively compiled) for the
//! same programs under each edition. Unchanged upstream witnesses run from
//! `fixtures/upstream/rustc-checkout` under their own directive edition.
use super::capture_with;
use crate::{
    cli::{StandaloneOutcome, runtime_capture::Request, standalone_captured},
    core::profile::ProfileIdentity,
};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-try-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(
    label: &str,
    source: &str,
    edition: u16,
) -> Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic> {
    let temp = Temp::new(label);
    let root = temp.0.join("main.rs");
    std::fs::write(&root, source).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let (prepared, view, sources, mut snapshot) = capture_with(
        &root,
        false,
        edition,
        &super::ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )
    .unwrap()
    .0;
    standalone_captured(
        &root,
        prepared,
        view,
        sources,
        &mut snapshot,
        None,
        Request {
            enabled: false,
            context: None,
        },
        None,
    )
}

/// The program's stdout is `expected` under every edition.
fn prints(label: &str, source: &str, expected: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "edition {edition}\n{source}")
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{source}", report.panic)
            }
            Ok(_) => panic!("edition {edition}: unexpected outcome\n{source}"),
            Err(error) => panic!("edition {edition}: {error}\n{source}"),
        }
    }
}

/// The program's stdout, stderr and process status are `expected` under every
/// edition (`main` returning a `Termination` value is reported by the entry).
fn reports(label: &str, source: &str, stdout: &str, stderr: &str, status: i32) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, stdout, "edition {edition} stdout\n{source}");
                assert_eq!(report.output.stderr, stderr, "edition {edition} stderr\n{source}");
                assert_eq!(report.exit_code, status, "edition {edition} status\n{source}");
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{source}", report.panic)
            }
            Ok(_) => panic!("edition {edition}: unexpected outcome\n{source}"),
            Err(error) => panic!("edition {edition}: {error}\n{source}"),
        }
    }
}

/// An unchanged upstream run-pass witness completes under its directive
/// edition (rustc's default 2015 when it names none).
fn upstream_completes(path: &str, text: &str) {
    let edition = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("//@ edition:"))
        .map_or(2015, |edition| edition.trim().parse().unwrap());
    match run("upstream", text, edition) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(StandaloneOutcome::Panicked(report)) => panic!("{path} panicked: {:?}", report.panic),
        Ok(_) => panic!("{path}: unexpected outcome"),
        Err(error) => panic!("{path}: {error}"),
    }
}

#[test]
fn result_forms_drop_order_and_from_conversions() {
    prints(
        "result_forms_drop_order_and_from_conversions",
        r####"use std::num::ParseIntError;

struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

fn parse(s: &str) -> Result<i64, ParseIntError> {
    let _g = Guard("parse");
    let v = s.parse::<i64>()?;
    println!("parsed {}", v);
    Ok(v * 2)
}
fn first_char(s: &str) -> Option<char> {
    let _g = Guard("first_char");
    let c = s.chars().next()?;
    Some(c)
}
#[derive(Debug)]
enum MyErr {
    Parse(ParseIntError),
    Neg,
}
impl From<ParseIntError> for MyErr {
    fn from(e: ParseIntError) -> MyErr {
        println!("from");
        MyErr::Parse(e)
    }
}
fn conv(s: &str) -> Result<i64, MyErr> {
    let _g = Guard("conv");
    let v = s.parse::<i64>()?;
    if v < 0 {
        return Err(MyErr::Neg);
    }
    Ok(v)
}
fn boxed(s: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let _g = Guard("boxed");
    let v = s.parse::<i64>()?;
    Ok(v)
}
fn chain(s: &str) -> Result<i64, MyErr> {
    let a = conv(s)?;
    let b = conv("5")?;
    Ok(a + b)
}
fn main() {
    println!("{:?}", parse("21"));
    println!("{:?}", parse("x").is_err());
    println!("{:?}", first_char("hi"));
    println!("{:?}", first_char(""));
    println!("{:?}", conv("3"));
    println!("{:?}", conv("-3"));
    match conv("zz") {
        Err(MyErr::Parse(_)) => println!("conv zz parse err"),
        other => println!("unexpected {:?}", other),
    }
    println!("{}", boxed("77").unwrap());
    println!("{}", boxed("q").is_err());
    println!("{:?}", chain("10").unwrap());
    println!("{}", chain("a").is_err());
}
"####,
        r####"parsed 21
drop parse
Ok(42)
drop parse
true
drop first_char
Some('h')
drop first_char
None
drop conv
Ok(3)
drop conv
Err(Neg)
from
drop conv
conv zz parse err
drop boxed
77
drop boxed
true
drop conv
drop conv
15
from
drop conv
true
"####,
    );
}

#[test]
fn option_controlflow_loops_and_trait_methods() {
    prints(
        "option_controlflow_loops_and_trait_methods",
        r####"// generic Try users, trait methods, nested closure/loop forms, Option in Option, ControlFlow
use std::ops::ControlFlow;
use std::num::ParseIntError;

struct G(&'static str);
impl Drop for G {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

trait Parser {
    fn parse(&self, s: &str) -> Result<i32, ParseIntError>;
    fn twice(&self, s: &str) -> Result<i32, ParseIntError> {
        let a = self.parse(s)?;
        let b = self.parse(s)?;
        Ok(a + b)
    }
}
struct P;
impl Parser for P {
    fn parse(&self, s: &str) -> Result<i32, ParseIntError> {
        let _g = G("P::parse");
        let v = s.parse::<i32>()?;
        Ok(v)
    }
}

fn opt_chain(v: &[i32]) -> Option<i32> {
    let a = v.first()?;
    let b = v.get(1)?;
    Some(*a + *b)
}

fn cf(n: i32) -> ControlFlow<i32, i32> {
    let a = if n > 2 { ControlFlow::Break(n) } else { ControlFlow::Continue(n) }?;
    ControlFlow::Continue(a * 10)
}

fn in_loop() -> Result<i32, ParseIntError> {
    let mut total: i32 = 0;
    for s in ["1", "2", "x", "4"] {
        let _g = G("loop-iter");
        total += s.parse::<i32>()?;
    }
    Ok(total)
}

fn in_while() -> Option<i32> {
    let v: Vec<i32> = vec![3, 2, 1, 0];
    let mut i: usize = 0;
    let mut acc: i32 = 0;
    while i < 10 {
        acc += *v.get(i)?;
        i += 1;
    }
    Some(acc)
}

fn question_tail(s: &str) -> Result<i32, ParseIntError> {
    s.parse::<i32>()
}
fn question_stmt(s: &str) -> Result<(), ParseIntError> {
    question_tail(s)?;
    println!("stmt ok {}", s);
    Ok(())
}

fn main() {
    let p = P;
    println!("{:?}", p.twice("4"));
    println!("{:?}", p.twice("z").is_err());
    println!("{:?}", opt_chain(&[1, 2]));
    println!("{:?}", opt_chain(&[1]));
    println!("{:?}", opt_chain(&[]));
    match cf(1) {
        ControlFlow::Continue(v) => println!("cf cont {}", v),
        ControlFlow::Break(v) => println!("cf break {}", v),
    }
    match cf(5) {
        ControlFlow::Continue(v) => println!("cf cont {}", v),
        ControlFlow::Break(v) => println!("cf break {}", v),
    }
    println!("{:?}", in_loop().is_err());
    println!("{:?}", in_while());
    println!("{:?}", question_stmt("8"));
    println!("{:?}", question_stmt("q").is_err());
}
"####,
        r####"drop P::parse
drop P::parse
Ok(8)
drop P::parse
true
Some(3)
None
None
cf cont 10
cf break 5
drop loop-iter
drop loop-iter
drop loop-iter
true
None
stmt ok 8
Ok(())
true
"####,
    );
}

#[test]
fn box_dyn_error_and_custom_error_conversions() {
    prints(
        "box_dyn_error_and_custom_error_conversions",
        r####"use std::error::Error;
use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
struct MyErr(i32);
impl fmt::Display for MyErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MyErr({})", self.0)
    }
}
impl Error for MyErr {}

struct G(&'static str);
impl Drop for G {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

fn boxed(s: &str) -> Result<i64, Box<dyn Error>> {
    let _g = G("boxed");
    let v = s.parse::<i64>()?;
    Ok(v + 1)
}
fn custom(n: i32) -> Result<i32, MyErr> {
    if n < 0 { Err(MyErr(n)) } else { Ok(n) }
}
fn via_custom(n: i32) -> Result<i32, Box<dyn Error>> {
    let _g = G("via_custom");
    let v = custom(n)?;
    Ok(v * 2)
}
fn strs(n: i32) -> Result<i32, Box<dyn Error>> {
    if n == 0 {
        let r: Result<(), &str> = Err("zero is bad");
        r?;
    }
    if n == 1 {
        return Err(Box::new(MyErr(1)));
    }
    Ok(n)
}
fn main() {
    println!("{:?}", boxed("5").unwrap());
    match boxed("x") {
        Ok(_) => println!("unexpected"),
        Err(e) => println!("err: {}", e),
    }
    println!("{:?}", via_custom(4).unwrap());
    match via_custom(-3) {
        Ok(_) => println!("unexpected"),
        Err(e) => println!("err: {} / {:?}", e, e),
    }
    match strs(0) {
        Ok(_) => println!("unexpected"),
        Err(e) => println!("err: {}", e),
    }
    match strs(1) {
        Ok(_) => println!("unexpected"),
        Err(e) => println!("err: {}", e),
    }
    println!("{:?}", strs(7).unwrap());
}
"####,
        r####"drop boxed
6
drop boxed
err: invalid digit found in string
drop via_custom
8
drop via_custom
err: MyErr(-3) / MyErr(-3)
err: zero is bad
err: MyErr(1)
7
"####,
    );
}

#[test]
fn error_conversion_selected_by_the_functions_result_type() {
    prints(
        "error_conversion_selected_by_the_functions_result_type",
        r####"use std::num::ParseIntError;
#[derive(Debug)]
enum MyErr {
    Parse(ParseIntError),
}
impl From<ParseIntError> for MyErr {
    fn from(e: ParseIntError) -> MyErr {
        MyErr::Parse(e)
    }
}
fn conv(s: &str) -> Result<i64, MyErr> {
    let v = s.parse::<i64>()?;
    Ok(v)
}
fn boxed(s: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let v = s.parse::<i64>()?;
    Ok(v)
}
fn main() {
    println!("{}", boxed("77").unwrap());
    println!("{}", conv("x").is_err());
    println!("{}", boxed("q").is_err());
}
"####,
        r####"77
true
true
"####,
    );
}

#[test]
fn result_type_inferred_through_the_operand_of_a_question_mark() {
    prints(
        "result_type_inferred_through_the_operand_of_a_question_mark",
        r####"use std::num::ParseIntError;
fn a() -> i32 {
    let n: i32 = "5".parse().unwrap();
    n + 1
}
fn b() -> Result<i32, ParseIntError> {
    let n: i32 = "6".parse()?;
    Ok(n + 1)
}
fn c() -> Result<u8, ParseIntError> {
    Ok("7".parse()?)
}
fn main() {
    println!("{}", a());
    println!("{:?}", b());
    println!("{:?}", c());
}
"####,
        r####"6
Ok(7)
Ok(7)
"####,
    );
}

#[test]
fn err_and_none_operands_take_the_diverging_fallback() {
    prints(
        "err_and_none_operands_take_the_diverging_fallback",
        r####"use std::error::Error;
fn f(n: i32) -> Result<i32, Box<dyn Error>> {
    if n == 0 {
        Err("zero")?;
    }
    if n == 1 {
        Err(String::from("one"))?
    }
    Ok(n)
}
fn g(n: i32) -> Option<i32> {
    if n == 0 {
        None?;
    }
    Some(n)
}
fn main() {
    match f(0) {
        Ok(v) => println!("ok {}", v),
        Err(e) => println!("err {}", e),
    }
    match f(1) {
        Ok(v) => println!("ok {}", v),
        Err(e) => println!("err {}", e),
    }
    println!("{:?}", f(2).unwrap());
    println!("{:?} {:?}", g(0), g(3));
}
"####,
        r####"err zero
err one
2
None Some(3)
"####,
    );
}

#[test]
fn generic_functions_methods_and_ok_or_chains() {
    prints(
        "generic_functions_methods_and_ok_or_chains",
        r####"use std::str::FromStr;
use std::fmt::Debug;

fn parse_gen<T: FromStr>(s: &str) -> Result<T, T::Err> {
    let v = s.parse::<T>()?;
    Ok(v)
}
fn sum_all<T: FromStr + Copy>(items: &[&str]) -> Result<Vec<T>, T::Err> {
    let mut out: Vec<T> = Vec::new();
    for s in items {
        out.push(s.parse::<T>()?);
    }
    Ok(out)
}
fn ok_or_chain(v: &Vec<i32>, i: usize) -> Result<i32, String> {
    let x = v.get(i).copied().ok_or("missing".to_string())?;
    Ok(x + 1)
}
struct Counter {
    n: i32,
}
impl Counter {
    fn step(&mut self, s: &str) -> Result<i32, std::num::ParseIntError> {
        let d: i32 = s.parse()?;
        self.n += d;
        Ok(self.n)
    }
}
fn show<T: Debug>(t: T) {
    println!("{:?}", t);
}
fn main() {
    show(parse_gen::<i32>("12"));
    show(parse_gen::<i32>("zz").is_err());
    show(parse_gen::<u8>("300").is_err());
    show(sum_all::<i32>(&["1", "2", "3"]));
    show(sum_all::<i32>(&["1", "x", "3"]).is_err());
    let v = vec![10, 20];
    show(ok_or_chain(&v, 1));
    show(ok_or_chain(&v, 5));
    let mut c = Counter { n: 0 };
    show(c.step("5"));
    show(c.step("q").is_err());
    show(c.step("7"));
}
"####,
        r####"Ok(12)
true
true
Ok([1, 2, 3])
true
Ok(21)
Err("missing")
Ok(5)
true
Ok(12)
"####,
    );
}

#[test]
fn operand_in_macro_arguments_and_for_loops_drop_in_order() {
    prints(
        "operand_in_macro_arguments_and_for_loops_drop_in_order",
        r####"use std::fmt;
#[derive(Debug)]
struct E(i32);
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}
impl std::error::Error for E {}

struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
fn ok(n: i32) -> Result<i32, E> {
    if n > 0 { Ok(n) } else { Err(E(n)) }
}
fn in_macro(n: i32) -> Result<(), E> {
    let _g = Guard("in_macro");
    println!("value {}", ok(n)?);
    println!("after");
    Ok(())
}
fn nested_loops() -> Result<i32, E> {
    let _g = Guard("loops");
    let mut sum = 0;
    for i in 0..5 {
        let _l = Guard("iter");
        sum += ok(2 - i)?;
        println!("sum {}", sum);
    }
    Ok(sum)
}
fn main() {
    println!("{:?}", in_macro(3));
    println!("{:?}", in_macro(-3));
    println!("{:?}", nested_loops());
}
"####,
        r####"value 3
after
drop in_macro
Ok(())
drop in_macro
Err(E(-3))
sum 2
drop iter
sum 3
drop iter
drop iter
drop loops
Err(E(0))
"####,
    );
}

#[test]
fn closures_with_declared_results_return_and_convert_through_the_operator() {
    prints(
        "closures",
        r####"use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
struct E(i32);
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}
impl std::error::Error for E {}

#[derive(Debug)]
enum Wide {
    Parse(ParseIntError),
    E(E),
}
impl From<ParseIntError> for Wide {
    fn from(e: ParseIntError) -> Wide {
        Wide::Parse(e)
    }
}
impl From<E> for Wide {
    fn from(e: E) -> Wide {
        Wide::E(e)
    }
}

struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

fn ok(n: i32) -> Result<i32, E> {
    if n > 0 { Ok(n) } else { Err(E(n)) }
}

fn closures() -> Result<i32, E> {
    let f = |n: i32| -> Result<i32, E> {
        let _g = Guard("closure");
        let v = ok(n)?;
        Ok(v + 100)
    };
    let a = f(1)?;
    println!("a {}", a);
    let r = f(-1);
    println!("r {:?}", r);
    let b = f(-2)?;
    println!("unreachable {}", b);
    Ok(a)
}

fn wide_closure() -> Wide {
    let g = |s: &str| -> Result<i32, Wide> {
        let n: i32 = s.parse()?;
        let m = ok(n)?;
        Ok(m)
    };
    println!("{:?}", g("4"));
    println!("{:?}", g("x").is_err());
    match g("-9") {
        Err(w) => w,
        Ok(_) => Wide::E(E(0)),
    }
}

fn opt_closure() -> Option<i32> {
    let h = |v: &Vec<i32>| -> Option<i32> {
        let a = v.first()?;
        let b = v.last()?;
        Some(*a + *b)
    };
    println!("{:?}", h(&vec![1, 2, 3]));
    println!("{:?}", h(&vec![]));
    h(&vec![5])
}

fn main() {
    println!("{:?}", closures());
    println!("{:?}", wide_closure());
    println!("{:?}", opt_closure());
}
"####,
        r####"drop closure
a 101
drop closure
r Err(E(-1))
drop closure
Err(E(-2))
Ok(4)
true
E(E(-9))
Some(4)
None
Some(10)
"####,
    );
}

#[test]
fn main_result_with_closures_macros_and_loops_reports_its_error() {
    reports(
        "t2",
        r####"use std::fmt;
#[derive(Debug)]
struct E(i32);
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}
impl std::error::Error for E {}

struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
fn ok(n: i32) -> Result<i32, E> {
    if n > 0 { Ok(n) } else { Err(E(n)) }
}
fn in_macro(n: i32) -> Result<(), E> {
    let _g = Guard("in_macro");
    println!("value {}", ok(n)?);
    println!("after");
    Ok(())
}
fn closures() -> Result<i32, E> {
    let f = |n: i32| -> Result<i32, E> {
        let _g = Guard("closure");
        let v = ok(n)?;
        Ok(v + 100)
    };
    let a = f(1)?;
    println!("a {}", a);
    let r = f(-1);
    println!("r {:?}", r);
    let b = f(-2)?;
    println!("unreachable {}", b);
    Ok(a)
}
fn nested_loops() -> Result<i32, E> {
    let _g = Guard("loops");
    let mut sum = 0;
    for i in 0..5 {
        let _l = Guard("iter");
        sum += ok(2 - i)?;
        println!("sum {}", sum);
    }
    Ok(sum)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{:?}", in_macro(3));
    println!("{:?}", in_macro(-3));
    println!("{:?}", closures());
    println!("{:?}", nested_loops());
    let v = ok(5)?;
    println!("main v {}", v);
    ok(-9)?;
    println!("unreachable");
    Ok(())
}
"####,
        r####"value 3
after
drop in_macro
Ok(())
drop in_macro
Err(E(-3))
drop closure
a 101
drop closure
r Err(E(-1))
drop closure
Err(E(-2))
sum 2
drop iter
sum 3
drop iter
drop iter
drop loops
Err(E(0))
main v 5
"####,
        r####"Error: E(-9)
"####,
        1,
    );
}

#[test]
fn main_result_box_dyn_error_drops_locals_before_reporting() {
    reports(
        "n1",
        r####"use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct E(i32);
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}
impl Error for E {}

struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
fn ok(n: i32) -> Result<i32, E> {
    if n > 0 { Ok(n) } else { Err(E(n)) }
}
fn main() -> Result<(), Box<dyn Error>> {
    let _g = Guard("main");
    let v = ok(5)?;
    println!("main v {}", v);
    ok(-9)?;
    println!("unreachable");
    Ok(())
}
"####,
        r####"main v 5
drop main
"####,
        r####"Error: E(-9)
"####,
        1,
    );
}

#[test]
fn main_exit_code_is_the_process_status() {
    reports(
        "n3",
        r####"use std::process::ExitCode;
fn main() -> ExitCode {
    println!("code");
    ExitCode::from(7)
}
"####,
        r####"code
"####,
        r####""####,
        7,
    );
}

#[test]
fn main_result_string_error_is_reported_with_debug() {
    reports(
        "n4",
        r####"fn main() -> Result<(), String> {
    println!("start");
    Err("bad".to_string())
}
"####,
        r####"start
"####,
        r####"Error: "bad"
"####,
        1,
    );
}

#[test]
fn main_question_mark_converts_parse_errors_into_box_dyn_error() {
    reports(
        "n5",
        r####"use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let n: i32 = "x".parse()?;
    println!("unreachable {}", n);
    Ok(())
}
"####,
        r####""####,
        r####"Error: ParseIntError { kind: InvalidDigit }
"####,
        1,
    );
}

#[test]
fn upstream_try_operator_expansion_hygiene() {
    upstream_completes(
        "tests/ui/try-trait/try-operator-expansion-hygiene.rs",
        include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/try-trait/try-operator-expansion-hygiene.rs"
        ),
    );
}
