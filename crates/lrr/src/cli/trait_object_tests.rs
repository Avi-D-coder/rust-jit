//! Trait objects (`dyn Trait`) in declared positions (E-R5-011): a value keeps
//! its concrete runtime Type and a declared `dyn` position admits it. Expected
//! output is real rustc 1.99.0 output, in all four editions.
use super::format_trait_facade_tests::prints;

#[test]
fn boxed_and_borrowed_trait_objects_in_locals_fields_vectors_and_arrays() {
    prints(
        "trait-objects-places",
        "trait Shape {
    fn area(&self) -> i32;
    fn doubled(&self) -> i32 {
        self.area() * 2
    }
}
struct Square(i32);
struct Rect(i32, i32);
impl Shape for Square {
    fn area(&self) -> i32 {
        self.0 * self.0
    }
}
impl Shape for Rect {
    fn area(&self) -> i32 {
        self.0 * self.1
    }
    fn doubled(&self) -> i32 {
        -1
    }
}
struct Holder {
    shape: Box<dyn Shape>,
}
struct Borrowed<'a> {
    shape: &'a dyn Shape,
}
fn describe(s: &dyn Shape) -> i32 {
    s.area() + s.doubled()
}
fn make(square: bool) -> Box<dyn Shape> {
    if square { Box::new(Square(3)) } else { Box::new(Rect(2, 5)) }
}
fn main() {
    let mut s: Box<dyn Shape> = Box::new(Square(2));
    println!(\"{}\", s.area());
    s = Box::new(Rect(3, 4));
    println!(\"{} {}\", s.area(), s.doubled());
    let mut h = Holder { shape: Box::new(Square(5)) };
    println!(\"{}\", h.shape.doubled());
    h.shape = make(false);
    println!(\"{}\", h.shape.area());
    let a = Square(6);
    let b = Rect(1, 7);
    let mut r = Borrowed { shape: &a };
    println!(\"{}\", r.shape.area());
    r.shape = &b;
    println!(\"{}\", describe(r.shape));
    println!(\"{} {}\", describe(&a), describe(&*make(true)));
    let list: Vec<Box<dyn Shape>> = vec![Box::new(Square(1)), Box::new(Rect(2, 3)), make(true)];
    let mut total = 0;
    for shape in &list {
        total += shape.area();
    }
    println!(\"{} {}\", list.len(), total);
    let mut grown: Vec<Box<dyn Shape>> = Vec::new();
    grown.push(Box::new(Square(4)));
    grown.push(make(false));
    while let Some(shape) = grown.pop() {
        println!(\"{}\", shape.area());
    }
    let refs: [&dyn Shape; 2] = [&a, &b];
    for shape in refs {
        println!(\"{}\", shape.doubled());
    }
}
",
        "4\n12 -1\n50\n10\n36\n6\n108 27\n3 16\n10\n16\n72\n-1\n",
    );
}

#[test]
fn trait_objects_in_tuples_closures_maps_rc_and_drop_order() {
    prints(
        "trait-objects-containers",
        "use std::collections::HashMap;
use std::rc::Rc;

trait Named {
    fn name(&self) -> String;
}
struct Loud(&'static str);
struct Quiet(i32);
impl Named for Loud {
    fn name(&self) -> String {
        self.0.to_uppercase()
    }
}
impl Named for Quiet {
    fn name(&self) -> String {
        format!(\"q{}\", self.0)
    }
}
impl Drop for Loud {
    fn drop(&mut self) {
        println!(\"drop loud {}\", self.0);
    }
}
impl Drop for Quiet {
    fn drop(&mut self) {
        println!(\"drop quiet {}\", self.0);
    }
}
struct Plain(i32);
impl Named for Plain {
    fn name(&self) -> String {
        format!(\"p{}\", self.0)
    }
}
struct Registry {
    entries: Vec<(String, Box<dyn Named>)>,
    ops: Vec<Box<dyn Fn(i32) -> i32>>,
}
fn main() {
    let mut r = Registry { entries: Vec::new(), ops: Vec::new() };
    r.entries.push((\"a\".to_string(), Box::new(Loud(\"x\"))));
    r.entries.push((\"b\".to_string(), Box::new(Quiet(7))));
    for (key, value) in &r.entries {
        println!(\"{} {}\", key, value.name());
    }
    let k = 10;
    r.ops.push(Box::new(|x| x + 1));
    r.ops.push(Box::new(move |x| x * k));
    let mut acc = 1;
    for op in &r.ops {
        acc = op(acc);
    }
    println!(\"{}\", acc);
    let mut by_name: HashMap<String, Box<dyn Named>> = HashMap::new();
    by_name.insert(\"p\".to_string(), Box::new(Plain(1)));
    by_name.insert(\"q\".to_string(), Box::new(Plain(2)));
    println!(\"{} {}\", by_name[\"p\"].name(), by_name.get(\"q\").unwrap().name());
    let shared: Rc<dyn Named> = Rc::new(Quiet(9));
    let again = Rc::clone(&shared);
    println!(\"{} {}\", shared.name(), Rc::strong_count(&again));
    println!(\"end\");
}
",
        "a X\nb q7\n20\np1 p2\nq9 2\nend\ndrop quiet 9\ndrop loud x\ndrop quiet 7\n",
    );
}

#[test]
fn trait_object_operands_keep_their_concrete_literal_type() {
    // A bare `dyn Trait` is a coercion target: the literal under `Box::new(8)`
    // or `&5` takes its own (fallback) integer Type, not the object type.
    prints(
        "trait-objects-literals",
        "trait Foo {
    fn f(&self) -> i32;
}
impl Foo for i32 {
    fn f(&self) -> i32 {
        *self + 1
    }
}
fn main() {
    let bo: Box<dyn Foo> = Box::new(8);
    println!(\"{}\", bo.f());
    let r: &dyn Foo = &5;
    println!(\"{}\", r.f());
}
",
        "9\n6\n",
    );
}
