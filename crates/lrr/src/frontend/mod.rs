pub(crate) mod body_type_applications;
pub(crate) mod declaration_generic_types;
pub(crate) mod declaration_generics;
pub(crate) mod pattern_access;
pub(crate) mod trait_declarations;
pub(crate) mod language_items;
pub(crate) mod array_repeats;
pub(crate) mod associated_constant_applications;
pub(crate) mod associated_declarations;
pub(crate) mod declaration_owner_templates;
pub(crate) mod associated_types;
pub mod builtin_attributes;
pub mod compile_input_catalog;
mod glob_lookup;
pub mod hygiene;
pub mod item_attributes;
pub(crate) mod library_macros;
pub mod local_scopes;
pub(crate) mod macro_boundary;
pub(crate) mod macro_definition_modules;
pub mod macro_names;
pub mod macro_origins;
pub mod macro_scopes;
pub mod member_index;
pub(crate) mod reference_methods;
mod module_aliases;
mod module_paths;
pub mod original_attributes;
pub mod procedural_exports;
pub mod source_macro_routes;
pub mod source_macros;
pub mod types;
pub(crate) mod written_terms;

mod body_imports;

mod body_modules;

mod body_namespace;

pub mod container_catalogue;

pub mod source_locals;
pub use source_locals::owned_plan::qualified_type_names;

pub(crate) mod source_body_context;

pub mod nested_compile_inputs;

pub mod impl_index;
pub mod names;

pub mod thunks;

pub mod signature_subst;
pub(crate) mod associated_equations;

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
pub(crate) mod nominal_constructors;
pub(crate) mod prepared_constructors;

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

pub(crate) mod pattern_binding_occurrences;
pub(crate) mod pattern_names;

pub(crate) mod runtime_macro_numeric;

pub(crate) mod primitive_type_names;
pub(crate) mod type_name_namespace;

pub(crate) mod source_parameter_goals;
pub(crate) mod source_types;
mod source_type_paths;

pub mod current_module_names;

pub use source_locals::owned_plan::source_value_goals;

pub(crate) mod binding_constraints;
pub(crate) mod prepared_binding_plan;

pub mod entered_type_goals;

pub mod entered_call_goals;
pub mod source_return_goals;

pub(crate) mod explicit_type_arguments;

pub mod nominal_member_goals;

pub(crate) mod impl_head_types;
pub(crate) mod impl_trait_identity;

pub(crate) mod source_predicates;
pub(crate) mod impl_predicates;

pub(crate) mod declaration_repr;

pub(crate) mod trait_associated_types;
pub(crate) mod declaration_source_predicates;

pub(crate) mod source_item_bounds;
