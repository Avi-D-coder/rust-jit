//! Engine contracts; execution dispatch is implemented by its later task cards.
pub mod dispatch;
pub mod telemetry;

pub mod install;

pub(crate) mod failure;
pub mod run;

pub mod test_catalog;
pub mod test_harness;

pub mod fixture;

pub mod fixture_observer;

pub mod runtime_macros;

pub mod standalone_macros;

pub(crate) mod declarative_runtime;

// Opaque witness producer remains an explicit T054 prerequisite.
pub(crate) mod constant_array_publication;

// Historical one-shot adapter: regression model only. Production uses
// normal SourceConstant dispatch and coordinator-only reached preparation.
#[cfg(test)]
pub(crate) mod scalar_constant_use;
