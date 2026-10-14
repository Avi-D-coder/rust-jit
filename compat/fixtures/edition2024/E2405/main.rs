struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
fn skipped_bool() -> bool { println!("forbidden_later_call"); true }
#[test]
fn case() {
if false && let true = skipped_bool() { println!("unexpected"); } else { println!("else_entered"); }
println!("after_if");
}
