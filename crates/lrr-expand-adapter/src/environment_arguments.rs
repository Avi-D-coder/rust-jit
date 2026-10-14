//! Environment operands use the shared pinned string/syntax parser.
pub use crate::string_arguments::StringOperand;
use crate::{AdapterError, token_wire::TokenForest};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EnvironmentArguments {
    pub name: StringOperand,
    pub missing_message: Option<StringOperand>,
}
pub fn parse(input: &TokenForest, required: bool) -> Result<EnvironmentArguments, AdapterError> {
    let mut parsed =
        crate::string_arguments::parse_operands(input, if required { 2 } else { 1 })?.into_iter();
    let name = parsed.next().ok_or_else(|| AdapterError {
        code: "LRR-E-COMPILE-INPUT",
        message: "environment name operand is missing".into(),
    })?;
    Ok(EnvironmentArguments {
        name,
        missing_message: parsed.next(),
    })
}
