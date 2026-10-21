struct Unit;
fn t(Unit: Unit) -> i32 { 7 }
fn main() { assert_eq!(t(Unit), 7); }
