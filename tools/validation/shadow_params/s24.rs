fn t(foo: (i32, i32)) -> i32 { let (foo, bar) = foo; foo + bar }
fn main() { assert_eq!(t((3, 4)), 7); }
