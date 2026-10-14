// Typed canonical content for synthetic dependency/transport providers only.
fn fixture_value(key: &lrr::db::key::QueryKey, value: i128) -> lrr::db::value::QueryValue {
    use lrr::{
        frontend::constants::{
            ConstValue, memo::ConstMemoPayload, query_value::ConstantQueryValue,
            wire::ConstantWireType,
        },
        ir::op::PrimitiveType,
    };
    let fallback;
    let selected = if let lrr::db::key::QueryKey::Constant(selected) = key {
        selected
    } else {
        // A deliberately mismatched provider in the existing variant test still
        // returns a Constant, so Database must diagnose the wrong variant.
        fallback = fixture_key(
            lrr::core::digest::Digest::default(),
            vec![],
            lrr::core::digest::Digest::default(),
            lrr::core::profile::ProfileIdentity::current(),
        );
        &fallback
    };
    let ty = ConstantWireType::Scalar(PrimitiveType::I64);
    let scalar = lrr::runtime::value::ValueRepr::signed(
        lrr::runtime::shape::PrimitiveIdentity::from_recipe(PrimitiveType::I64)
            .unwrap()
            .id(),
        i64::try_from(value).unwrap(),
    )
    .unwrap();
    let payload = ConstMemoPayload::capture_content(
        selected,
        &ConstValue::from_scalar(scalar).unwrap(),
        &ty,
        lrr::core::digest::Digest::default(),
        128,
    )
    .unwrap();
    lrr::db::value::QueryValue::Constant(Box::new(
        ConstantQueryValue::capture(selected, payload).unwrap(),
    ))
}
