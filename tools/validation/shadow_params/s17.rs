macro_rules! sq { ($x:expr) => {{ let x = $x; x * x }} }
fn t(x: i32) -> i32 { sq!(x + 1) }
fn main() { assert_eq!(t(3), 16); }
