fn t(foo: i32) -> i32 { let c = |foo: i32| foo + 1; c(foo) + foo }
fn main() { assert_eq!(t(3), 7); }
