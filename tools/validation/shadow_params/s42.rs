fn t(foo: i32) -> i32 { let foo = (|| foo + 1)(); foo }
fn main() { assert_eq!(t(3), 4); }
