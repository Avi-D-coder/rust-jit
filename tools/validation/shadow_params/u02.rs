fn t(f: fn(i32) -> i32) -> i32 { let f = |x: i32| x + 100; f(1) }
fn inc(x: i32) -> i32 { x + 1 }
fn main() { assert_eq!(t(inc), 101); }
