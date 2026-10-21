fn t(foo: Option<i32>) -> i32 { if let Some(foo) = foo { foo + 1 } else { 0 } }
fn main() { assert_eq!(t(Some(3)), 4); assert_eq!(t(None), 0); }
