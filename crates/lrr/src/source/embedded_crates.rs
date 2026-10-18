//! LRR-provided source crates added to every standalone world.
//!
//! An embedded crate is captured exactly like a command-line `--extern`
//! dependency: each file becomes a source unit of the crate's own unit and
//! owning edition, published in the same `PreparedSourceInputs` as the root.
//! Every captured crate (root, `--extern` dependencies and the other embedded
//! crates) receives a `SourceDependency` edge to each embedded crate by name,
//! so `extern crate core;`, `core::x`, `std::y` and the compiler-injected
//! `std`/`core` imports resolve into these sources through the ordinary name
//! pipeline.
use crate::core::{digest::Digest, profile::ProfileIdentity};

/// One source file of an embedded crate.
#[derive(Debug, Clone)]
pub struct EmbeddedFile {
    /// Semantic module path of the file inside its crate (`[]` is the root).
    pub module_path: Vec<String>,
    /// Display/source path of the file, unique within the crate.
    pub path: &'static str,
    pub text: &'static str,
}

/// One LRR-provided source crate.
#[derive(Debug, Clone)]
pub struct EmbeddedCrate {
    /// Extern-prelude / `extern crate` name (`core`, `alloc`, `std`).
    pub name: &'static str,
    /// Crate unit; for the standard facades `crate::facade::unit(profile, name)`,
    /// the same unit the compiler-injected `std`/`core` imports target.
    pub unit: Digest,
    /// Owning edition of the crate's own sources.
    pub edition: u16,
    /// Every module file. Exactly one file has the empty module path.
    pub files: Vec<EmbeddedFile>,
}

macro_rules! facade_file {
    ([$($module:literal),*], $path:literal) => {
        EmbeddedFile {
            module_path: vec![$(String::from($module)),*],
            path: concat!("<lrr-facade>/", $path),
            text: include_str!(concat!("../../facade/", $path)),
        }
    };
}

/// The standard facade crates (`core`, `alloc`, `std`) as LRR-provided Rust
/// source (Section 17). Their units are the same `crate::facade::unit`
/// identities the library macro definitions and closed facade functions use,
/// so all three join one crate per name.
///
/// The facade unit (and the compiler catalogue that owns it) is keyed by the
/// owning profile, edition included, so the facade sources are captured in
/// that same profile. They are written to mean the same thing in every
/// edition (explicit `crate::`/extern-crate paths, no edition-only syntax).
pub fn standard_facade_crates(profile: &ProfileIdentity) -> Vec<EmbeddedCrate> {
    let krate = |name: &'static str, files: Vec<EmbeddedFile>| EmbeddedCrate {
        name,
        unit: crate::facade::unit(profile, name),
        edition: profile.edition,
        files,
    };
    vec![
        krate(
            "core",
            vec![
                facade_file!([], "core/lib.rs"),
                facade_file!(["any"], "core/any.rs"),
                facade_file!(["array"], "core/array.rs"),
                facade_file!(["borrow"], "core/borrow.rs"),
                facade_file!(["cell"], "core/cell.rs"),
                facade_file!(["char"], "core/char.rs"),
                facade_file!(["clone"], "core/clone.rs"),
                facade_file!(["cmp"], "core/cmp.rs"),
                facade_file!(["convert"], "core/convert.rs"),
                facade_file!(["default"], "core/default.rs"),
                facade_file!(["error"], "core/error.rs"),
                facade_file!(["ffi"], "core/ffi.rs"),
                facade_file!(["float_impls"], "core/float_impls.rs"),
                facade_file!(["fmt"], "core/fmt.rs"),
                facade_file!(["future"], "core/future.rs"),
                facade_file!(["hash"], "core/hash.rs"),
                facade_file!(["hint"], "core/hint.rs"),
                facade_file!(["int_convert"], "core/int_convert.rs"),
                facade_file!(["int_impls"], "core/int_impls.rs"),
                facade_file!(["iter"], "core/iter.rs"),
                facade_file!(["marker"], "core/marker.rs"),
                facade_file!(["mem"], "core/mem.rs"),
                facade_file!(["num"], "core/num.rs"),
                facade_file!(["ops"], "core/ops.rs"),
                facade_file!(["option"], "core/option.rs"),
                facade_file!(["panic"], "core/panic.rs"),
                facade_file!(["panicking"], "core/panicking.rs"),
                facade_file!(["pin"], "core/pin.rs"),
                facade_file!(["prelude"], "core/prelude.rs"),
                facade_file!(["primitive_impls"], "core/primitive_impls.rs"),
                facade_file!(["ptr"], "core/ptr.rs"),
                facade_file!(["result"], "core/result.rs"),
                facade_file!(["slice"], "core/slice.rs"),
                facade_file!(["step_impls"], "core/step_impls.rs"),
                facade_file!(["str"], "core/str.rs"),
                facade_file!(["sync"], "core/sync.rs"),
                facade_file!(["task"], "core/task.rs"),
                facade_file!(["time"], "core/time.rs"),
            ],
        ),
        krate(
            "alloc",
            vec![
                facade_file!([], "alloc/lib.rs"),
                facade_file!(["borrow"], "alloc/borrow.rs"),
                facade_file!(["boxed"], "alloc/boxed.rs"),
                facade_file!(["collections"], "alloc/collections.rs"),
                facade_file!(["fmt"], "alloc/fmt.rs"),
                facade_file!(["intrinsics"], "alloc/intrinsics.rs"),
                facade_file!(["rc"], "alloc/rc.rs"),
                facade_file!(["slice"], "alloc/slice.rs"),
                facade_file!(["str"], "alloc/str.rs"),
                facade_file!(["string"], "alloc/string.rs"),
                facade_file!(["sync"], "alloc/sync.rs"),
                facade_file!(["vec"], "alloc/vec.rs"),
            ],
        ),
        krate(
            "std",
            vec![
                facade_file!([], "std/lib.rs"),
                facade_file!(["collections"], "std/collections.rs"),
                facade_file!(["env"], "std/env.rs"),
                facade_file!(["error"], "std/error.rs"),
                facade_file!(["ffi"], "std/ffi.rs"),
                facade_file!(["hash"], "std/hash.rs"),
                facade_file!(["io"], "std/io.rs"),
                facade_file!(["num"], "std/num.rs"),
                facade_file!(["panic"], "std/panic.rs"),
                facade_file!(["prelude"], "std/prelude.rs"),
                facade_file!(["process"], "std/process.rs"),
                facade_file!(["sync"], "std/sync.rs"),
                facade_file!(["thread"], "std/thread.rs"),
                facade_file!(["time"], "std/time.rs"),
            ],
        ),
    ]
}

/// The embedded crates of a standalone run for the root profile.
///
/// Staged: the standard facade sources join the world only when
/// `LRR_EMBEDDED_FACADE=1` until their integration witnesses pass.
pub fn embedded_crates(profile: &ProfileIdentity) -> Vec<EmbeddedCrate> {
    if std::env::var_os("LRR_EMBEDDED_FACADE").is_some_and(|value| value == "1") {
        standard_facade_crates(profile)
    } else {
        Vec::new()
    }
}
