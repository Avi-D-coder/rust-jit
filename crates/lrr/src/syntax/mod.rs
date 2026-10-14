pub mod ast_adapter;
pub mod audit;
pub mod declaration_scan;
pub mod modules;
pub mod origin;
pub mod outline;
pub mod parse;
pub mod spans;

pub mod cfg_input;
pub mod constructors;

// Only this owned schema is shared. It imports no upstream parser, interner or arena IDs.
pub mod entered_macros;
pub mod imports;
#[path = "../../../lrr-expand-adapter/src/token_wire/model.rs"]
pub mod token_wire;
pub mod visibility;

pub mod macro_calls;

pub mod body_items;

pub mod local_bindings;

pub mod attributes;

pub mod body_index;

mod thunks;

pub mod signatures;

pub mod type_aliases;
pub mod type_schemas;

pub mod fixture_markers;

pub mod runtime_macro_arguments;

pub(crate) mod macro_prelude;

pub mod conditions;

pub mod temporary_extensions;

#[cfg(test)]
pub(crate) mod place_fixture_facts;
#[cfg(test)]
pub(crate) mod scope_fixture_facts;

pub(crate) mod fields;

pub(crate) mod runtime_macro_scalars;

pub(crate) mod numeric_fallback;

pub mod symbolic_headers;

#[cfg(test)]
pub(crate) mod discarded_numeric_fixture_facts;

pub(crate) mod generated_expression;

pub(crate) mod floating_paths;

#[cfg(test)]
pub(crate) mod floating_fixture_hygiene;

pub(crate) mod literal_decode;

pub(crate) mod lexical_numeric_callees;

pub mod pure_constants;
