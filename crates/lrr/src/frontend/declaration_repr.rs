//! One captured-header repr parser, shared by source Types and runtime layout.
use crate::{
    core::diagnostic::Diagnostic,
    frontend::types::NominalSchema,
    ir::op::PrimitiveType,
    runtime::layout::{IntRepr, Repr},
};
#[derive(Debug, Clone, Copy)]
pub(crate) struct DeclaredRepr {
    pub(crate) layout: Repr,
    /// The actual integer spelling; byte width cannot distinguish usize/u64.
    pub(crate) integer: Option<PrimitiveType>,
}
fn layout_unsupported(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}
/// The `#[repr(..)]` hints of a declaration.
pub(crate) fn declared_repr(
    schema: &NominalSchema,
    name: &str,
) -> Result<DeclaredRepr, Diagnostic> {
    let mut repr = Repr::default();
    let mut integer = None;
    for attribute in schema
        .header
        .attributes
        .iter()
        .filter(|attribute| attribute.name == "repr")
    {
        let tokens: Vec<&str> = attribute
            .tokens
            .iter()
            .map(|token| token.text.trim())
            .filter(|text| !text.is_empty())
            .collect();
        // `repr ( C , packed ( 2 ) )`
        let mut index = tokens
            .iter()
            .position(|text| *text == "(")
            .map_or(tokens.len(), |at| at + 1);
        while index < tokens.len() {
            let hint = tokens[index];
            index += 1;
            let mut argument = None;
            if tokens.get(index) == Some(&"(") {
                argument = tokens.get(index + 1).copied();
                index += 3;
            }
            let number = |argument: Option<&str>| -> Result<Option<u64>, Diagnostic> {
                argument
                    .map(|text| {
                        let text = text.replace('_', "");
                        let parsed = match text.strip_prefix("0x") {
                            Some(hex) => u64::from_str_radix(hex, 16),
                            None => text.parse::<u64>(),
                        };
                        parsed.map_err(|_| {
                            layout_unsupported(format!(
                                "the repr argument `{text}` of `{name}` is not an integer literal"
                            ))
                        })
                    })
                    .transpose()
            };
            let int = |bytes, signed| Some(IntRepr { bytes, signed });
            match hint {
                "C" => repr.c = true,
                "transparent" => repr.transparent = true,
                "packed" => {
                    let pack = number(argument)?.unwrap_or(1);
                    repr.packed = Some(repr.packed.map_or(pack, |prior| prior.min(pack)));
                }
                "align" => repr.align = repr.align.max(number(argument)?),
                "u8" => {
                    repr.int = int(1, false);
                    integer = Some(PrimitiveType::U8);
                }
                "i8" => {
                    repr.int = int(1, true);
                    integer = Some(PrimitiveType::I8);
                }
                "u16" => {
                    repr.int = int(2, false);
                    integer = Some(PrimitiveType::U16);
                }
                "i16" => {
                    repr.int = int(2, true);
                    integer = Some(PrimitiveType::I16);
                }
                "u32" => {
                    repr.int = int(4, false);
                    integer = Some(PrimitiveType::U32);
                }
                "i32" => {
                    repr.int = int(4, true);
                    integer = Some(PrimitiveType::I32);
                }
                "u64" => {
                    repr.int = int(8, false);
                    integer = Some(PrimitiveType::U64);
                }
                "usize" => {
                    repr.int = int(8, false);
                    integer = Some(PrimitiveType::Usize);
                }
                "i64" => {
                    repr.int = int(8, true);
                    integer = Some(PrimitiveType::I64);
                }
                "isize" => {
                    repr.int = int(8, true);
                    integer = Some(PrimitiveType::Isize);
                }
                "u128" => {
                    repr.int = int(16, false);
                    integer = Some(PrimitiveType::U128);
                }
                "i128" => {
                    repr.int = int(16, true);
                    integer = Some(PrimitiveType::I128);
                }
                _ => {}
            }
            // The separating comma.
            if tokens.get(index) == Some(&",") {
                index += 1;
            }
        }
    }
    Ok(DeclaredRepr {
        layout: repr,
        integer,
    })
}
