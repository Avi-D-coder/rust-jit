fn t(x: u8) { let x = 1000i64; assert_eq!(x, 1000); assert!(x > 5); assert_ne!(x, 999); }
fn main() { t(1); }
