fn t(foo: i32) -> i32 { let a = { let foo = foo * 10; foo + 1 }; a + foo }
fn main() { assert_eq!(t(3), 34); }
