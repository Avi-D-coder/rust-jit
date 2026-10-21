fn t(x: i32) -> String { let x = x + 1; assert_eq!(x, 4); format!("{}", x) }
fn main() { assert_eq!(t(3), "4"); }
