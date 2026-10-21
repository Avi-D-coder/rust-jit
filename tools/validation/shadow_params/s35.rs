struct P { x: i32, y: i32 }
fn t(x: i32) -> P { let x = x + 1; P { x, y: x } }
fn main() { let p = t(3); assert_eq!((p.x, p.y), (4, 4)); }
