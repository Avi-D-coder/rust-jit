struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
#[test]
fn case() {
if let Choice::Yes(ref probe) = Choice::Yes(Probe("scrutinee")) {
let _local = Probe("consequent"); println!("borrowed:{}", probe.0);
} else { println!("unexpected"); }
println!("after_if");
}
