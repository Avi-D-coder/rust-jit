struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
fn make(name: &'static str, yes: bool) -> Choice { println!("evaluate:{}", name); if yes { Choice::Yes(Probe(name)) } else { Choice::No(Probe(name)) } }
#[test]
fn case() {
if let Choice::Yes(ref first) = make("first", true)
&& let Choice::Yes(ref second) = make("second", false) {
println!("unexpected:{}:{}", first.0, second.0);
} else { println!("else_entered"); }
println!("after_if");
}
