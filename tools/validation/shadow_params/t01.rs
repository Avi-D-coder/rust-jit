fn t(a: u8) -> bool { let a = 1000i64; a == 1000 }
fn u(a: u8) -> bool { let a = 3i32; a < 10 }
fn main() { assert!(t(1)); assert!(u(200)); }
