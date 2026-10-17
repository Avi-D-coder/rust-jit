//! Whole-program controls: the common source-constraint Types reach the
//! ordinary interpreter (literal hints, compound places, call contracts).
//! Overflow panics are rustc's own observable for the selected width.
use super::*;

enum Expected {
    Stdout(&'static str),
    Overflow,
}
fn run(text: &str, edition: u16) -> Result<String, String> {
    let mut h = start(text, edition, "main");
    let contexts = h.macros.contexts().unwrap();
    match h.run.poll_with_runtime_macros(&mut h.db, &contexts) {
        RunPoll::Completed(report) => Ok(report.output.stdout),
        RunPoll::Panicked(report) => Err(serde_json::to_value(&report.panic).unwrap()["kind"]
            .as_str()
            .unwrap_or_default()
            .to_owned()),
        RunPoll::Fault(error) => panic!("{edition} {text}: {error:?}"),
        RunPoll::NeedInput(input) => panic!("{edition} {text}: {input:?}"),
    }
}
#[test]
fn inferred_numeric_types_reach_execution_all_editions() {
    use Expected::*;
    let cases: &[(&str, Expected)] = &[
        ("fn main() { let n = 3; if n == 4 { } }", Stdout("")),
        ("fn main() { let n = 2147483647; let m = n + 1; }", Overflow),
        (
            "fn main() { let a: u8 = 10; let n = 250; let c = a + n; }",
            Overflow,
        ),
        (
            "fn main() { let n = 100; let x: i8 = n; let y = 2 * n; }",
            Overflow,
        ),
        (
            "fn takes(x: u8) -> u8 { x }\nfn main() { let n = 250; let m = takes(n) + 10; }",
            Overflow,
        ),
        (
            "fn f() -> u8 { 5 }\nfn main() { let n = 251; let m = n + f(); }",
            Overflow,
        ),
        (
            "fn main() { let mut i = 0; while i < 3 { i += 1; } assert!(i == 3); }",
            Stdout(""),
        ),
        (
            "fn main() { let mut i = 250; let x: u8 = i; i += 10; }",
            Overflow,
        ),
        (
            "fn main() { let mut t = 0; let mut k = 1; while k <= 4 { t += k; k += 1; } assert!(t == 10); }",
            Stdout(""),
        ),
        ("fn main() { let x: u8 = 200; let y = 100 + x; }", Overflow),
        ("fn main() { 2147483647 + 1; }", Overflow),
        // The standard assertions' ordinary expansions: a borrowed tuple
        // matched by a tuple pattern, compared through dereferences, with a
        // closed library facade `assert_failed` on the cold failure branch.
        ("fn main() { let x = 5; assert_eq!(x, 5); }", Stdout("")),
        ("fn main() { let x: u8 = 5; assert_eq!(x, 5); }", Stdout("")),
        ("fn main() { assert_eq!(2 + 2, 4); }", Stdout("")),
        ("fn main() { assert_ne!(1, 2); }", Stdout("")),
        (
            "fn main() { let x: u8 = 250; assert_eq!(x + 10, 4); }",
            Overflow,
        ),
        (
            "fn main() { let (a, b) = (3, 4); assert_eq!(a + b, 7); }",
            Stdout(""),
        ),
        (
            "fn main() { let n = 250; let r = &n; let s: &u8 = r; let m = n + 10; }",
            Overflow,
        ),
        // Module-qualified and type-relative associated callees.
        (
            "mod m { pub fn f(x: u8) -> u8 { x } }\nfn main() { let n = 250; let k = m::f(n) + 10; }",
            Overflow,
        ),
        (
            "struct P;\nimpl P { fn f(x: u8) -> u8 { x } }\nfn main() { let n = 250; let m = P::f(n) + 10; }",
            Overflow,
        ),
        // Arrays, tuple positions and literal indexing carry element Types;
        // a callee body types reads of its own parameters.
        (
            "fn main() { let arr = [1, 2, 3]; assert!(arr[1] == 2); }",
            Stdout(""),
        ),
        (
            "fn main() { let t = (1, 2); let t2 = t; assert!(t2.0 == 1); }",
            Stdout(""),
        ),
        (
            "fn main() { let arr = [250, 2]; let x: u8 = arr[0]; let y = arr[1] + 255; }",
            Overflow,
        ),
        (
            "fn main() { let [a, b] = [1, 2]; let c: u8 = a; let d = b + 255; }",
            Overflow,
        ),
        (
            "fn test(foo: isize) { assert_eq!(foo, 10); }\nfn main() { test(10); }",
            Stdout(""),
        ),
        (
            "fn test(foo: i8) { let k = 100; if foo == k {} let z = k + 100; }\nfn main() { test(1); }",
            Overflow,
        ),
        (
            "pub fn main() { let d = -0xBEEF_isize; assert_eq!(d, -48879); }",
            Stdout(""),
        ),
    ];
    for edition in EDITIONS {
        for (text, expected) in cases {
            match (run(text, edition), expected) {
                (Ok(stdout), Stdout(want)) => assert_eq!(stdout, *want, "{edition} {text}"),
                (Err(kind), Overflow) => assert_eq!(kind, "overflow", "{edition} {text}"),
                (actual, _) => panic!("{edition} {text}: {actual:?}"),
            }
        }
    }
}
