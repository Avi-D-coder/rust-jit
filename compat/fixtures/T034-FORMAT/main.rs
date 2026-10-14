fn broken() { unknown(); }
#[test]
fn case() {
    if true {} else { /*lrr-marker:cold_branch*/ unknown(); }
}
