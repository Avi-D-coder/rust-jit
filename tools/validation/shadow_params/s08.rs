fn t(foo: Option<i32>) -> i32 { match foo { Some(foo) => foo * 2, None => 0 } }
fn main() { assert_eq!(t(Some(3)), 6); assert_eq!(t(None), 0); }
