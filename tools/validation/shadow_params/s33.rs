fn t(foo: &mut i32) { let foo = &mut *foo; *foo += 1; }
fn main() { let mut a = 1; t(&mut a); assert_eq!(a, 2); }
