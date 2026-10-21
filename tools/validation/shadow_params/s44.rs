struct S { v: i32 }
impl S {
    fn m(self, v: i32) -> i32 { let S { v: sv } = self; let v = v + sv; v }
    fn n(&self) -> i32 { let v = self.v; let v = v + 1; v }
}
fn main() { assert_eq!(S { v: 2 }.m(3), 5); assert_eq!(S { v: 2 }.n(), 3); }
