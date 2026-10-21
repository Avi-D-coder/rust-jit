macro_rules! m { () => { let foo = 100; } }
fn t(foo: i32) -> i32 { m!(); foo }
fn main() { assert_eq!(t(3), 3); }
