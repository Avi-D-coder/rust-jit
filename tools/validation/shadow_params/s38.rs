fn t(x: i32) -> i32 { let x = if x > 0 { x } else { -x }; x }
fn main() { assert_eq!(t(-3), 3); assert_eq!(t(4), 4); }
