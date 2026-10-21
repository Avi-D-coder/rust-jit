macro_rules! m { ($i:ident) => { let $i = $i + 1; } }
fn t(foo: i32) -> i32 { m!(foo); foo }
fn main() { assert_eq!(t(3), 4); }
