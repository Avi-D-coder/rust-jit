//! Reached original literal range shapes. No constant/name/descriptor authority.
use super::*;
use ra_ap_syntax::ast::{RangeItem, RangeOp};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Family {
    Integer,
    Floating,
    Char,
}

fn floating_suffix(raw: &str) -> Option<PrimitiveType> {
    match raw {
        "f32" => Some(PrimitiveType::F32),
        "f64" => Some(PrimitiveType::F64),
        _ => None,
    }
}

// A real float token or a decimal integer token with an explicit float suffix
// supplies floating-family source evidence. An unrelated expected integer/bool
// type never selects the float family or becomes its source identity.
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

fn endpoint_family(pattern: &ast::LiteralPat) -> Option<Family> {
    let literal = pattern.literal()?;
    if floating_context(&literal, None).is_some() {
        return Some(Family::Floating);
    }
    match literal.token().kind() {
        SyntaxKind::INT_NUMBER => {
            let number = ast::IntNumber::cast(literal.token())?;
            match number.suffix() {
                Some(suffix) if primitive(suffix).is_none() => None,
                _ => Some(Family::Integer),
            }
        }
        SyntaxKind::CHAR if pattern.minus_token().is_none() => Some(Family::Char),
        SyntaxKind::BYTE if pattern.minus_token().is_none() => Some(Family::Integer),
        _ => None,
    }
}

// Only written intrinsic kind/suffix evidence, without decoding any magnitude
// or looking up a path. This may participate in the existing source context
// collection while unsupported cold alternatives remain unprepared.
pub(super) fn source_type(pattern: &ast::RangePat) -> Option<PrimitiveType> {
    for endpoint in [pattern.start(), pattern.end()].into_iter().flatten() {
        let ast::Pat::LiteralPat(endpoint) = endpoint else {
            continue;
        };
        let literal = endpoint.literal()?;
        if let Some(ty) = match literal.token().kind() {
            SyntaxKind::FLOAT_NUMBER => ast::FloatNumber::cast(literal.token())?
                .suffix()
                .and_then(floating_suffix),
            SyntaxKind::INT_NUMBER => floating_context(&literal, None),
            _ => None,
        } {
            return Some(ty);
        }
        match literal.token().kind() {
            SyntaxKind::CHAR if endpoint.minus_token().is_none() => {
                return Some(PrimitiveType::Char);
            }
            SyntaxKind::BYTE if endpoint.minus_token().is_none() => {
                return Some(PrimitiveType::U8);
            }
            SyntaxKind::INT_NUMBER => {
                if let Some(ty) = ast::IntNumber::cast(literal.token())?
                    .suffix()
                    .and_then(primitive)
                {
                    return Some(ty);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) struct ScalarRange {
    lower: Option<ast::Pat>,
    upper: Option<ast::Pat>,
    inclusive: bool,
}

impl ScalarRange {
    pub(super) fn from_source(pattern: &ast::RangePat) -> Result<Self, Diagnostic> {
        if pattern
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
        {
            return Err(unsupported(
                "range attributes require prepared source admission",
            ));
        }
        let inclusive = match pattern.op_kind() {
            Some(RangeOp::Inclusive) => true,
            Some(RangeOp::Exclusive) => false,
            None => {
                return Err(unsupported(
                    "range operator requires supported inclusive/exclusive source syntax",
                ));
            }
        };
        // Keep source syntax without admitting the unentered upper endpoint.
        // A lower mismatch must reach Exit even when the upper source form
        // still requires a feature that has not been implemented.
        Ok(Self {
            lower: pattern.start(),
            upper: pattern.end(),
            inclusive,
        })
    }

    pub(super) fn upper_comparison(&self) -> NumericBinary {
        if self.inclusive {
            NumericBinary::Le
        } else {
            NumericBinary::Lt
        }
    }

    pub(super) fn lower(
        &self,
        body: &BodyHandle,
        key: &PcKey,
    ) -> Result<LiteralRecipe, Diagnostic> {
        let lower = reached_endpoint(self.lower.as_ref())?;
        endpoint(lower, body, key)
    }

    pub(super) fn upper(
        &self,
        body: &BodyHandle,
        key: &PcKey,
    ) -> Result<LiteralRecipe, Diagnostic> {
        let lower = reached_endpoint(self.lower.as_ref())?;
        let upper = reached_endpoint(self.upper.as_ref())?;
        if endpoint_family(upper) != endpoint_family(lower) {
            return Err(unsupported(
                "range endpoints require the same supported literal family",
            ));
        }
        endpoint(upper, body, key)
    }
}

fn reached_endpoint(pattern: Option<&ast::Pat>) -> Result<&ast::LiteralPat, Diagnostic> {
    let Some(ast::Pat::LiteralPat(endpoint)) = pattern else {
        return Err(unsupported(
            "reached range endpoint requires an original integer, floating, char or byte literal",
        ));
    };
    if endpoint
        .syntax()
        .children()
        .any(|node| ast::Attr::can_cast(node.kind()))
    {
        return Err(unsupported(
            "reached range endpoint attributes require prepared source admission",
        ));
    }
    endpoint_family(endpoint)
        .ok_or_else(|| unsupported("reached range endpoint requires a supported literal family"))?;
    Ok(endpoint)
}

fn endpoint(
    pattern: &ast::LiteralPat,
    body: &BodyHandle,
    key: &PcKey,
) -> Result<LiteralRecipe, Diagnostic> {
    if key.body != body.key() {
        return Err(error("range endpoint owning body differs"));
    }
    let range = pattern.syntax().text_range();
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    if end > body.range.end || start >= end || end - start > 65_536 {
        return Err(limit());
    }
    let span = BodySpan {
        body_key: key.body.clone(),
        relative_start: start.checked_sub(body.range.start).ok_or_else(limit)?,
        relative_end: end.checked_sub(body.range.start).ok_or_else(limit)?,
    };
    let literal = pattern
        .literal()
        .ok_or_else(|| error("validated range endpoint literal disappeared"))?;
    if pattern.minus_token().is_some() && floating_context(&literal, None).is_some() {
        let mut recipe = super::super::literal_recipe(&literal, span)?;
        let Some(DecodedLiteral::Float { decimal, .. }) = &mut recipe.decoded else {
            return Err(error(
                "range floating literal decoder disagrees with source family",
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
            .ok_or_else(|| error("range signed integer token absent"))?;
        Ok(LiteralRecipe {
            kind: LiteralKind::NegativeInteger,
            spelling: pattern.syntax().text().to_string(),
            decoded: Some(crate::syntax::literal_decode::integer(&number, true)),
            source: span,
        })
    } else {
        super::super::literal_recipe(&literal, span)
    }
}

pub(super) fn hint(literal: &LiteralRecipe, context: &HintRecipe) -> HintRecipe {
    match &literal.decoded {
        Some(DecodedLiteral::Integer {
            suffix: Some(ty), ..
        }) => HintRecipe::Primitive(*ty),
        Some(DecodedLiteral::Float {
            suffix: Some(ty), ..
        }) => HintRecipe::Primitive(*ty),
        Some(DecodedLiteral::Char(_)) => HintRecipe::Primitive(PrimitiveType::Char),
        Some(DecodedLiteral::Byte(_)) => HintRecipe::Primitive(PrimitiveType::U8),
        _ => context.clone(),
    }
}
