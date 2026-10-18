//! std's real layouts for the library types whose facade declaration does not
//! mirror std's fields (E-R5-012). The table is keyed, like the lang items in
//! `facade/mod.rs`, by the defining crate and path, never by a name a caller
//! writes. A type absent from the table is laid out from its own facade
//! fields, which agree with std for `Option`, `Result`, `Cell`, `RefCell`,
//! ranges, `Wrapping`, `PhantomData` and the other plain declarations.
//!
//! The numbers are `size_of`/`align_of` of the same types from
//! `rustc +1.99.0` on a 64-bit host; `niche` is the count of unused bit
//! patterns std's declaration leaves (`Option<Vec<T>>` is 24 bytes because a
//! vector's capacity has `2^63` of them, `Option<BTreeMap>` is 32 because the
//! map has none).
use crate::{
    core::profile::ProfileIdentity,
    runtime::cell::layouts::{LibraryLayout, LibraryShape},
    source::manifest::DefKey,
};

pub(crate) fn library_layout(
    profile: &ProfileIdentity,
    definition: &DefKey,
) -> Option<LibraryLayout> {
    let crate_name = ["core", "alloc", "std"]
        .into_iter()
        .find(|name| super::unit(profile, name) == definition.crate_key)?;
    let path: Vec<&str> = definition.module_path.iter().map(String::as_str).collect();
    let owning = |shape: LibraryShape, needs_drop: bool| {
        Some(LibraryLayout {
            shape,
            needs_drop: Some(needs_drop),
        })
    };
    let fixed = |size: u64, niche: u128| LibraryShape::Fixed {
        size,
        align: 8,
        niche,
    };
    const CAPACITY_NICHE: u128 = 1 << 63;
    match (crate_name, path.as_slice(), definition.declared_name.as_str()) {
        // `UnsafeCell` hides the niche of its contents.
        ("core", ["cell"], "UnsafeCell") => Some(LibraryLayout {
            shape: LibraryShape::Fields {
                strip_niche: true,
                nonzero: false,
            },
            needs_drop: None,
        }),
        ("core", ["mem"], "ManuallyDrop") => owning(
            LibraryShape::Fields {
                strip_niche: false,
                nonzero: false,
            },
            false,
        ),
        ("core", ["num"], "NonZero") => Some(LibraryLayout {
            shape: LibraryShape::Fields {
                strip_niche: false,
                nonzero: true,
            },
            needs_drop: None,
        }),
        ("core", ["ptr"], "NonNull") => owning(LibraryShape::Pointer { nonnull: true }, false),
        ("alloc", ["boxed"], "Box")
        | ("alloc", ["rc"], "Rc" | "Weak")
        | ("alloc", ["sync"], "Arc" | "Weak") => {
            owning(LibraryShape::Pointer { nonnull: true }, true)
        }
        ("alloc", ["vec"], "Vec") => owning(fixed(24, CAPACITY_NICHE), true),
        ("alloc", ["collections"], "BTreeMap" | "BTreeSet") => owning(fixed(24, 0), true),
        ("alloc", ["collections"], "VecDeque") => owning(fixed(32, CAPACITY_NICHE), true),
        ("alloc", ["collections"], "BinaryHeap") => owning(fixed(24, CAPACITY_NICHE), true),
        ("alloc", ["collections"], "LinkedList") => owning(fixed(24, 0), true),
        ("std", ["collections"], "HashMap" | "HashSet") => owning(fixed(48, 1), true),
        _ => None,
    }
}
