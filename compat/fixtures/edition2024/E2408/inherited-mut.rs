fn main() {
    let (mut value,) = &(2_i64,);
    value += 1;
    println!("legacy:{value}");
}
