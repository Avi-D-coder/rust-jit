pub use ena::unify::{NoError, UnifyKey, UnifyValue};
use rustc_hash::FxBuildHasher;
pub use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, FxBuildHasher>;
pub type IndexSet<V> = indexmap::IndexSet<V, FxBuildHasher>;

mod delayed_map;

#[cfg(feature = "nightly")]
mod impl_ {
    pub use rustc_data_structures::sso::{SsoHashMap, SsoHashSet};
}

#[cfg(not(feature = "nightly"))]
mod impl_ {
    pub use std::collections::{HashMap as SsoHashMap, HashSet as SsoHashSet};
}

pub use delayed_map::{DelayedMap, DelayedSet};
pub use impl_::*;

// Stable build of the exact pinned rustc_data_structures helper.
#[cfg(not(feature = "nightly"))]
mod frozen;
#[cfg(not(feature = "nightly"))]
pub use frozen::Frozen;
#[cfg(not(feature = "nightly"))]
mod transitive_relation;
#[cfg(not(feature = "nightly"))]
pub use transitive_relation::{TransitiveRelation, TransitiveRelationBuilder};
pub mod leak_check_indexes;
