struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
fn failed_iflet() {
if let Choice::Yes(_) = Choice::No(Probe("scrutinee")) { println!("unexpected"); } else { println!("else_entered"); }
println!("after_if");
}
fn block_tail() {
let result: i64 = { let _local = Probe("local"); Probe("tail").value() };
println!("after:{}", result);
}
#[test]
fn case() { failed_iflet(); block_tail(); }
