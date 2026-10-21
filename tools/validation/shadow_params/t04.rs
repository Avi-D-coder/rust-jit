fn t(a: u8) -> usize { let a = String::from("abc"); a.len() }
fn main() { assert_eq!(t(1), 3); }
