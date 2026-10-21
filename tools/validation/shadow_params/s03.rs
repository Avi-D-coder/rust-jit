fn t(foo: i32) -> i32 { match foo { foo => foo + 10 } }
fn main() { assert_eq!(t(3), 13); }
