fn t(foo: i32) -> i32 { let foo = foo + 1; let c = || foo * 2; c() }
fn main() { assert_eq!(t(3), 8); }
