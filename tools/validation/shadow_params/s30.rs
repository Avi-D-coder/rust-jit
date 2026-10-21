fn t(x: Option<i32>) -> i32 { while let Some(x) = x { return x; } 0 }
fn main() { assert_eq!(t(Some(3)), 3); assert_eq!(t(None), 0); }
