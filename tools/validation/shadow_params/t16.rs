fn t(x: u8) -> i64 { let x = 70000i64; let a = [x, x]; a[0] + a[1] }
fn main() { assert_eq!(t(1), 140000); }
