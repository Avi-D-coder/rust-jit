pub mod budget;
pub mod builtins;
pub mod compile_input_identity;
pub mod concat;
pub mod declarative;
pub mod environment_arguments;
pub mod identity;
pub mod scopes;

pub mod record;

pub mod derive;

pub mod attributes;

pub mod attribute_materialize;

pub mod worklist;

pub mod member_cfg;

pub mod builtin_attributes;
pub mod derive_declaration;
pub(crate) mod nested_inert;

pub mod include_arguments;

pub mod include_string;

pub mod concat_arguments;

pub mod environment_plan;

pub mod cache;

pub(crate) mod source_hygiene;

pub mod discovery;

pub mod source_discovery;

pub mod generated_declarations;
pub mod generated_items;

pub mod source_declarative;

pub mod generated_call_request;

pub(crate) mod body_macro_boundary;
pub mod generated_syntax;

pub(crate) mod string_chain;

pub(crate) mod generated_attribute_input;
pub mod generated_derives;

pub(crate) mod derive_origin;
pub mod derive_receipt;
pub(crate) mod source_derives;

pub mod declaration_fragment;

pub mod argument_declarations;

pub(crate) mod argument_discovery;

pub(crate) mod source_replacement;

pub(crate) mod procedural_sites;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../lrr-expand-adapter/tests/support/declarative.rs"]
pub(crate) mod declarative_test_fixture;
