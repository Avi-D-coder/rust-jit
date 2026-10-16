//! Pure, single-threaded dependency-tracked query storage.
pub mod dependency;
pub mod key;
pub mod memo;
pub mod poll;
pub mod provider;
pub mod trace;
pub mod value;
pub use memo::macro_export_inputs::MacroExportSourceReceipt;

pub mod retained_syntax;
pub mod seals;

mod pattern_plan_provider;
mod source_plan_provider;
