fn t(foo: i32) -> i32 { let foo = foo; let foo = foo + 1; let foo = foo * 2; foo }
fn main() { assert_eq!(t(3), 8); }
