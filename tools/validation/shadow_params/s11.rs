struct S(i32);
impl S { fn m(&self, foo: i32) -> i32 { let foo = foo + self.0; foo } }
fn main() { assert_eq!(S(10).m(5), 15); }
