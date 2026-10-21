macro_rules! m { ($e:expr) => {{ let foo = 100; $e + foo }} }
fn t(foo: i32) -> i32 { m!(foo) }
fn main() { assert_eq!(t(3), 103); }
