fn t(foo: i32) -> i32 { let foo = foo; foo }
fn main() { assert_eq!(t(3), 3); }
