fn t(x: u8) -> i64 { let mut x = 5i64; let mut n = 0; while x < 1000 { x *= 2; n += 1; } x * 100 + n }
fn main() { assert_eq!(t(1), 128008); }
