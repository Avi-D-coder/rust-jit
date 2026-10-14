// Synthetic query-graph key only; this does not admit a source or pass a fixture.
fn fixture_key(
    definition: lrr::core::digest::Digest,
    arguments: Vec<lrr::core::digest::Digest>,
    cfg: lrr::core::digest::Digest,
    profile: lrr::core::profile::ProfileIdentity,
) -> lrr::db::key::ConstantKey {
    let mut e = lrr::core::digest::StableEncoder::new("lrr/test/query-graph-constant-selection");
    e.digest(definition);
    e.sequence(&arguments, |e, argument| e.digest(*argument));
    lrr::db::key::ConstantKey::capture_content(
        definition,
        e.finish(),
        lrr::core::digest::Digest::default(),
        cfg,
        profile,
        &lrr::frontend::constants::wire::ConstantWireType::Scalar(lrr::ir::op::PrimitiveType::I64),
        lrr::core::digest::Digest::default(),
    )
    .unwrap()
}
