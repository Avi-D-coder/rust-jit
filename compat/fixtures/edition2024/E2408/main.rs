#[test]
fn case() {
    let &(mut explicit,) = &(2_i64,);
    explicit += 1;
    let (inherited,) = &(4_i64,);
    let &(&nested,) = &(&5_i64,);
    let mut value = (6_i64,);
    let (borrowed,) = &mut value;
    *borrowed += 1;
    println!("explicit:{explicit}");
    println!("inherited:{inherited}");
    println!("nested:{nested}");
    println!("mutable:{}", value.0);
}
