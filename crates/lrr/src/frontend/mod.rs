pub mod builtin_attributes;
pub mod compile_input_catalog;
mod glob_lookup;
pub mod hygiene;
pub mod item_attributes;
pub mod local_scopes;
pub(crate) mod macro_boundary;
pub(crate) mod macro_definition_modules;
pub mod macro_names;
pub mod macro_origins;
pub mod macro_scopes;
mod module_aliases;
mod module_paths;
pub mod original_attributes;
pub mod procedural_exports;
pub mod source_macro_routes;
pub mod source_macros;
pub mod types;

mod body_imports;

mod body_modules;

mod body_namespace;

pub mod container_catalogue;

pub mod source_locals;

pub(crate) mod source_body_context;

pub mod nested_compile_inputs;

pub mod impl_index;
pub mod names;

pub mod thunks;

pub mod signature_subst;

pub mod signature_names;

pub mod type_aliases;

pub mod callables;

pub mod call_contracts;

pub mod runtime_macro_catalog;

pub mod runtime_macro_plan;

pub mod generated_headers;

pub mod generated_derive_exports;
pub mod generated_derives;
pub mod generated_macros;

pub mod source_module_declarations;

pub mod generated_derive_helpers;

pub mod constructors;
pub mod declaration_imports;

pub mod source_view;

pub mod primitive_constants;

pub(crate) mod constant_macro_admission;
pub(crate) mod constant_source_admission;
pub mod constants;

pub(crate) mod constant_result_type;

pub(crate) mod constant_path_selection;

pub(crate) mod constant_construction;

#[cfg(test)]
mod constant_foreign_path_tests;

pub(crate) mod scalar_pattern_binding;

pub(crate) mod runtime_macro_numeric;

pub(crate) mod primitive_type_names;
pub(crate) mod type_name_namespace;

pub(crate) mod source_types;

pub mod current_module_names;
