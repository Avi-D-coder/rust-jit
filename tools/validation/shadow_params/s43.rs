fn t(x: Option<i32>) -> i32 { if let Some(x) = x && x > 1 { x } else { 0 } }
fn main() { assert_eq!(t(Some(3)), 3); assert_eq!(t(Some(1)), 0); }
