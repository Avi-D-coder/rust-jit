fn t(x: u8) -> i64 { let x = 70000i64; let c = move || x + 1; c() }
fn main() { assert_eq!(t(1), 70001); }
