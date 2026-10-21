fn t(foo: &i32) -> i32 { let foo = *foo; let foo = &foo; *foo + 1 }
fn main() { assert_eq!(t(&3), 4); }
