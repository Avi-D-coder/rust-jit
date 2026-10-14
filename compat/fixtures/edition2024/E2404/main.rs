struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
#[test]
fn case() {
let direct = &Probe("direct"); let tuple = (&Probe("tuple"),);
let holder = Holder { probe: &Probe("struct") };
println!("using:{}:{}:{}", direct.0, tuple.0.0, holder.probe.0);
println!("block_end");
}
