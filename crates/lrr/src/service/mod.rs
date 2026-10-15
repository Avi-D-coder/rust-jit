//! Coordinator contracts; these modules do not confer native execution authority.
pub mod expansion;
pub mod function_macro_requests;
pub mod item_macro_inputs;
pub mod item_macro_requests;
pub mod macro_session;
pub mod source_capture;

pub mod eager_compile_inputs;

pub mod run;

pub mod derive_declarations;
mod generated_host_derives;
pub mod source_attribute_requests;
pub mod source_derive_requests;

#[cfg(test)]
mod prepared_module_registry_tests;

#[cfg(test)]
mod retained_syntax_tests;

#[cfg(test)]
mod retained_body_tests;

#[cfg(test)]
mod retained_callable_tests;

#[cfg(test)]
mod retained_type_name_tests;

#[cfg(test)]
mod retained_source_preparation_tests;
