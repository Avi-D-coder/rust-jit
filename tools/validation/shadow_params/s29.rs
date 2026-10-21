fn t(x: Option<i32>) -> i32 { let Some(x) = x else { return -1 }; x + 1 }
fn main() { assert_eq!(t(Some(3)), 4); assert_eq!(t(None), -1); }
