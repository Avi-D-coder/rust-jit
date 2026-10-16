//! Shared non-consuming scalar pattern tests over admitted ordinary syntax.
//!
//! This is the test stage, not a binding transaction. Owners supply the actual
//! indexed slots and continuations; this module cannot move the scrutinee,
//! publish bindings, enter scopes, resolve names or execute a cold result.
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        op::{
            DecodedLiteral, HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, Op,
            PrimitiveType,
        },
        pc::{PcKey, SlotId},
    },
    syntax::{outline::BodyHandle, spans::BodySpan},
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{self, RangeItem, RangeOp},
};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar pattern source budget".into(),
    )
}

#[cfg(test)]
thread_local! {
    static TEST_LOWERINGS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn observe_lowerings<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let before = TEST_LOWERINGS.with(|count| count.get());
    let result = work();
    (result, TEST_LOWERINGS.with(|count| count.get() - before))
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Family {
    Bool,
    Integer,
    Floating,
    Char,
}

fn suffix_type(raw: &str) -> Option<PrimitiveType> {
    Some(match raw {
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "usize" => PrimitiveType::Usize,
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        _ => return None,
    })
}
fn floating_suffix(raw: &str) -> Option<PrimitiveType> {
    suffix_type(raw).filter(|ty| matches!(ty, PrimitiveType::F32 | PrimitiveType::F64))
}

/// Intrinsic token/suffix evidence only; no Type-path lookup or magnitude decode.
pub(super) fn floating_context(
    literal: &ast::Literal,
    expected: Option<PrimitiveType>,
) -> Option<PrimitiveType> {
    match literal.token().kind() {
        SyntaxKind::FLOAT_NUMBER => {
            let number = ast::FloatNumber::cast(literal.token())?;
            match number.suffix() {
                Some(suffix) => floating_suffix(suffix),
                None => Some(
                    expected
                        .filter(|ty| matches!(ty, PrimitiveType::F32 | PrimitiveType::F64))
                        .unwrap_or(PrimitiveType::F64),
                ),
            }
        }
        SyntaxKind::INT_NUMBER => {
            let number = ast::IntNumber::cast(literal.token())?;
            if number.radix() != ast::Radix::Decimal {
                return None;
            }
            number.suffix().and_then(floating_suffix)
        }
        _ => None,
    }
}

fn family(pattern: &ast::LiteralPat) -> Option<Family> {
    let literal = pattern.literal()?;
    if floating_context(&literal, None).is_some() {
        return Some(Family::Floating);
    }
    match literal.token().kind() {
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if pattern.minus_token().is_none() => {
            Some(Family::Bool)
        }
        SyntaxKind::CHAR if pattern.minus_token().is_none() => Some(Family::Char),
        SyntaxKind::BYTE if pattern.minus_token().is_none() => Some(Family::Integer),
        SyntaxKind::INT_NUMBER => {
            let number = ast::IntNumber::cast(literal.token())?;
            match number.suffix() {
                Some(suffix) if suffix_type(suffix).is_none() => None,
                // Float suffixes on non-decimal integers are not float evidence.
                Some(suffix) if floating_suffix(suffix).is_some() => None,
                _ => Some(Family::Integer),
            }
        }
        _ => None,
    }
}

fn written_type(pattern: &ast::LiteralPat) -> Option<PrimitiveType> {
    let literal = pattern.literal()?;
    match literal.token().kind() {
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if pattern.minus_token().is_none() => {
            Some(PrimitiveType::Bool)
        }
        SyntaxKind::CHAR if pattern.minus_token().is_none() => Some(PrimitiveType::Char),
        SyntaxKind::BYTE if pattern.minus_token().is_none() => Some(PrimitiveType::U8),
        SyntaxKind::INT_NUMBER => ast::IntNumber::cast(literal.token())?
            .suffix()
            .and_then(suffix_type),
        SyntaxKind::FLOAT_NUMBER => ast::FloatNumber::cast(literal.token())?
            .suffix()
            .and_then(floating_suffix),
        _ => None,
    }
}

/// A source constraint, never executable preparation of an alternative.
pub(super) fn source_type(pattern: &ast::Pat) -> Option<PrimitiveType> {
    match pattern {
        ast::Pat::LiteralPat(literal) => written_type(literal),
        ast::Pat::RangePat(range) => {
            [range.start(), range.end()]
                .into_iter()
                .flatten()
                .find_map(|endpoint| match endpoint {
                    ast::Pat::LiteralPat(literal) => written_type(&literal),
                    _ => None,
                })
        }
        _ => None,
    }
}

pub(super) fn source_span(
    body: &BodyHandle,
    key: &PcKey,
    syntax: &SyntaxNode,
) -> Result<BodySpan, Diagnostic> {
    if key.body != body.key() {
        return Err(super::error("pattern literal owning body differs"));
    }
    let range = syntax.text_range();
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    if start >= end || end > body.range.end || end - start > 65_536 {
        return Err(limit());
    }
    Ok(BodySpan {
        body_key: key.body.clone(),
        relative_start: start.checked_sub(body.range.start).ok_or_else(limit)?,
        relative_end: end.checked_sub(body.range.start).ok_or_else(limit)?,
    })
}

fn reached_literal(pattern: Option<&ast::Pat>) -> Result<&ast::LiteralPat, Diagnostic> {
    let Some(ast::Pat::LiteralPat(literal)) = pattern else {
        return Err(unsupported(
            "reached scalar pattern/range requires a literal endpoint",
        ));
    };
    if literal
        .syntax()
        .children()
        .any(|node| ast::Attr::can_cast(node.kind()))
        || family(literal).is_none()
    {
        return Err(unsupported(
            "reached scalar pattern/range requires an unattributed supported literal",
        ));
    }
    Ok(literal)
}

fn decode(
    pattern: &ast::LiteralPat,
    body: &BodyHandle,
    key: &PcKey,
) -> Result<LiteralRecipe, Diagnostic> {
    let source = source_span(body, key, pattern.syntax())?;
    let literal = pattern
        .literal()
        .ok_or_else(|| super::error("validated pattern literal disappeared"))?;
    if pattern.minus_token().is_some() && family(pattern) == Some(Family::Floating) {
        let mut recipe = super::literal_recipe(&literal, source)?;
        let Some(DecodedLiteral::Float { decimal, .. }) = &mut recipe.decoded else {
            return Err(super::error(
                "pattern float decoder differs from source family",
            ));
        };
        let capacity = decimal.len().checked_add(1).ok_or_else(limit)?;
        let mut signed = String::new();
        signed.try_reserve_exact(capacity).map_err(|_| limit())?;
        signed.push('-');
        signed.push_str(decimal);
        *decimal = signed;
        recipe.spelling = pattern.syntax().text().to_string();
        Ok(recipe)
    } else if pattern.minus_token().is_some() {
        let number = ast::IntNumber::cast(literal.token())
            .ok_or_else(|| super::error("pattern signed integer token absent"))?;
        Ok(LiteralRecipe {
            kind: LiteralKind::NegativeInteger,
            spelling: pattern.syntax().text().to_string(),
            decoded: Some(crate::syntax::literal_decode::integer(&number, true)),
            source,
        })
    } else {
        super::literal_recipe(&literal, source)
    }
}

/// Actual syntax handles, with no guest values or executable result bodies.
pub(super) enum ScalarTestPlan {
    Literal(ast::Pat),
    Range {
        lower: Option<ast::Pat>,
        upper: Option<ast::Pat>,
        inclusive: bool,
    },
}

impl ScalarTestPlan {
    pub(super) fn from_source(pattern: &ast::Pat) -> Result<Self, Diagnostic> {
        if pattern
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
        {
            return Err(unsupported(
                "scalar pattern attributes require source admission",
            ));
        }
        match pattern {
            ast::Pat::LiteralPat(_) => Ok(Self::Literal(pattern.clone())),
            ast::Pat::RangePat(range) => Ok(Self::Range {
                lower: range.start(),
                upper: range.end(),
                inclusive: match range.op_kind() {
                    Some(RangeOp::Inclusive) => true,
                    Some(RangeOp::Exclusive) => false,
                    None => return Err(unsupported("scalar range operator absent")),
                },
            }),
            _ => Err(unsupported(
                "reached pattern requires scalar literal or range",
            )),
        }
    }

    pub(super) fn first(
        &self,
        body: &BodyHandle,
        key: &PcKey,
    ) -> Result<ScalarComparison, Diagnostic> {
        match self {
            Self::Literal(pattern) => Ok(ScalarComparison {
                literal: decode(reached_literal(Some(pattern))?, body, key)?,
                operator: NumericBinary::Eq,
                upper_pending: false,
            }),
            Self::Range {
                lower,
                upper,
                inclusive,
            } => {
                let endpoint = reached_literal(lower.as_ref().or(upper.as_ref()))?;
                if family(endpoint) == Some(Family::Bool) {
                    return Err(unsupported(
                        "scalar range requires numeric, byte or char literals",
                    ));
                }
                Ok(ScalarComparison {
                    literal: decode(endpoint, body, key)?,
                    operator: if lower.is_some() {
                        NumericBinary::Ge
                    } else if *inclusive {
                        NumericBinary::Le
                    } else {
                        NumericBinary::Lt
                    },
                    upper_pending: lower.is_some() && upper.is_some(),
                })
            }
        }
    }

    pub(super) fn upper(
        &self,
        body: &BodyHandle,
        key: &PcKey,
    ) -> Result<ScalarComparison, Diagnostic> {
        let Self::Range {
            lower,
            upper,
            inclusive,
        } = self
        else {
            return Err(super::error("non-range pattern has no upper continuation"));
        };
        let lower = reached_literal(lower.as_ref())?;
        let upper = reached_literal(upper.as_ref())?;
        if family(lower) == Some(Family::Bool) || family(lower) != family(upper) {
            return Err(unsupported(
                "scalar range endpoints require the same literal family",
            ));
        }
        Ok(ScalarComparison {
            literal: decode(upper, body, key)?,
            operator: if *inclusive {
                NumericBinary::Le
            } else {
                NumericBinary::Lt
            },
            upper_pending: false,
        })
    }
}

pub(super) struct ScalarComparison {
    pub(super) literal: LiteralRecipe,
    pub(super) operator: NumericBinary,
    pub(super) upper_pending: bool,
}
impl ScalarComparison {
    /// Only reads the scrutinee. The expected temporary is retired exactly once;
    /// the owner retains/discards the Bool and scrutinee at its continuation.
    pub(super) fn operations(
        self,
        source: SlotId,
        expected: SlotId,
        destination: SlotId,
        context: &HintRecipe,
    ) -> Vec<Op> {
        #[cfg(test)]
        TEST_LOWERINGS.with(|count| count.set(count.get().checked_add(1).unwrap()));
        let hint = match &self.literal.decoded {
            Some(DecodedLiteral::Integer {
                suffix: Some(ty), ..
            })
            | Some(DecodedLiteral::Float {
                suffix: Some(ty), ..
            }) => HintRecipe::Primitive(*ty),
            Some(DecodedLiteral::Char(_)) => HintRecipe::Primitive(PrimitiveType::Char),
            Some(DecodedLiteral::Byte(_)) => HintRecipe::Primitive(PrimitiveType::U8),
            _ if self.literal.kind == LiteralKind::Bool => {
                HintRecipe::Primitive(PrimitiveType::Bool)
            }
            _ => context.clone(),
        };
        vec![
            Op::Literal {
                literal: self.literal,
                hint,
                destination: expected,
            },
            Op::NumericBinary {
                operator: self.operator,
                left: source,
                right: expected,
                destination,
            },
            Op::Discard { source: expected },
        ]
    }
}

#[cfg(test)]
#[path = "pattern_tests_tests.rs"]
mod tests;
