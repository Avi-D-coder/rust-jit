pub mod functions;
pub mod macros;

pub mod prelude;
pub(crate) mod primitive_float_methods;
pub(crate) mod value_binding_prelude;

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

/// The `owned_box` lang item: the identity of the alloc facade's
/// `boxed::Box` declaration in `profile`'s facade crates. Like rustc's lang
/// items it is fixed by its defining crate and path, not by the names a
/// caller writes; compare it with a nominal Type's definition identity.
pub(crate) fn owned_box(profile: &crate::core::profile::ProfileIdentity) -> crate::core::digest::Digest {
    use crate::core::digest::StableEncode;
    crate::source::manifest::DefKey {
        crate_key: unit(profile, "alloc"),
        module_path: vec!["boxed".to_owned()],
        namespace: crate::source::manifest::Namespace::Type,
        item_kind: crate::source::manifest::ItemKind::Struct,
        declared_name: "Box".to_owned(),
        enclosing_impl: None,
        duplicate_ordinal: 0,
    }
    .fingerprint()
}
