struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
#[test]
fn case() {
let result: i64 = { let _local = Probe("local"); Probe("tail").value() };
println!("after:{}", result);
}
