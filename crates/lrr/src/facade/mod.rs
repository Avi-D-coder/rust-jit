pub mod macros;

pub mod prelude;

/// Preserve the existing standalone facet domain while sharing one core/std
/// unit identity across macro and ordinary facade namespaces.
pub(crate) fn unit(
    profile: &crate::core::profile::ProfileIdentity,
    name: &str,
) -> crate::core::digest::Digest {
    let mut e = crate::core::digest::StableEncoder::new("lrr/v2/synthetic-runtime-macro-facade");
    e.record(profile);
    e.string(name);
    e.finish()
}
