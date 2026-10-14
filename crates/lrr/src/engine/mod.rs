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
