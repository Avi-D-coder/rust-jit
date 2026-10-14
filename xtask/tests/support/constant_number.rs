// Decode the checked signed scalar produced by the synthetic fixture provider.
fn fixture_number(value: &lrr::db::value::QueryValue) -> i128 {
    let lrr::db::value::QueryValue::Constant(value) = value else {
        panic!("wrong typed fixture result")
    };
    let value = value.payload().decode_value().unwrap().scalar().unwrap();
    assert_eq!(value.kind, 4);
    i128::from(value.payload as i64)
}
