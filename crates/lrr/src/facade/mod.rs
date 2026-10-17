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

/// A struct declared directly in one module of the alloc facade, fixed by its
/// defining crate and path (not by the names a caller writes), like a rustc
/// lang item.
fn alloc_struct(
    profile: &crate::core::profile::ProfileIdentity,
    module: &str,
    name: &str,
) -> crate::core::digest::Digest {
    use crate::core::digest::StableEncode;
    crate::source::manifest::DefKey {
        crate_key: unit(profile, "alloc"),
        module_path: vec![module.to_owned()],
        namespace: crate::source::manifest::Namespace::Type,
        item_kind: crate::source::manifest::ItemKind::Struct,
        declared_name: name.to_owned(),
        enclosing_impl: None,
        duplicate_ordinal: 0,
    }
    .fingerprint()
}

/// The `unsafe_cell` lang item: the identity of the core facade's
/// `cell::UnsafeCell` declaration. A value containing one has interior
/// mutability, so rustc does not promote a borrow of it to `'static`.
pub(crate) fn unsafe_cell(profile: &crate::core::profile::ProfileIdentity) -> crate::core::digest::Digest {
    use crate::core::digest::StableEncode;
    crate::source::manifest::DefKey {
        crate_key: unit(profile, "core"),
        module_path: vec!["cell".to_owned()],
        namespace: crate::source::manifest::Namespace::Type,
        item_kind: crate::source::manifest::ItemKind::Struct,
        declared_name: "UnsafeCell".to_owned(),
        enclosing_impl: None,
        duplicate_ordinal: 0,
    }
    .fingerprint()
}

/// The `owned_box` lang item: the identity of the alloc facade's
/// `boxed::Box` declaration in `profile`'s facade crates. Like rustc's lang
/// items it is fixed by its defining crate and path, not by the names a
/// caller writes; compare it with a nominal Type's definition identity.
pub(crate) fn owned_box(profile: &crate::core::profile::ProfileIdentity) -> crate::core::digest::Digest {
    alloc_struct(profile, "boxed", "Box")
}

/// The growable-array lang item: the identity of the alloc facade's
/// `vec::Vec` declaration. A `Vec<T>` value is an aggregate object whose
/// fields are its element cells (see `runtime/cell_vecs.rs`).
pub(crate) fn owned_vec(profile: &crate::core::profile::ProfileIdentity) -> crate::core::digest::Digest {
    alloc_struct(profile, "vec", "Vec")
}
