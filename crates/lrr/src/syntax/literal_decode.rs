//! Immutable reached integer lexical decoding. No guest binding, query or effect.
use crate::ir::op::{DecodedLiteral, IntegerLexicalError, PrimitiveType};
use ra_ap_syntax::ast;

/// An actual integer literal used as retained array-length metadata. Lexical
/// decoding is shared with expressions and pure constants; no text is reparsed
/// and no runtime binding or static const-type check is performed here.
pub(crate) fn array_length(literal: &ast::Literal) -> Option<u64> {
    let ast::LiteralKind::IntNumber(number) = literal.kind() else {
        return None;
    };
    match integer(&number, false) {
        DecodedLiteral::Integer { magnitude, .. } => u64::try_from(magnitude).ok(),
        _ => None,
    }
}

pub(super) fn integer(number: &ast::IntNumber, negative: bool) -> DecodedLiteral {
    let radix = number.radix() as u8;
    let written_suffix = number.suffix();
    let suffix = match written_suffix {
        None => None,
        Some("i8") => Some(PrimitiveType::I8),
        Some("i16") => Some(PrimitiveType::I16),
        Some("i32") => Some(PrimitiveType::I32),
        Some("i64") => Some(PrimitiveType::I64),
        Some("i128") => Some(PrimitiveType::I128),
        Some("isize") => Some(PrimitiveType::Isize),
        Some("u8") => Some(PrimitiveType::U8),
        Some("u16") => Some(PrimitiveType::U16),
        Some("u32") => Some(PrimitiveType::U32),
        Some("u64") => Some(PrimitiveType::U64),
        Some("u128") => Some(PrimitiveType::U128),
        Some("usize") => Some(PrimitiveType::Usize),
        Some(raw) => {
            // Preserve the old reached refusal without assigning an unknown suffix a type.
            let reason = if raw.ends_with("i128") || raw.ends_with("u128") {
                IntegerLexicalError::UnsupportedWidth
            } else {
                IntegerLexicalError::InvalidSuffix
            };
            #[cfg(test)]
            record(false);
            return DecodedLiteral::InvalidInteger {
                negative,
                radix,
                suffix: None,
                reason,
            };
        }
    };
    match number.value() {
        Ok(magnitude) => {
            #[cfg(test)]
            record(true);
            DecodedLiteral::Integer {
                negative,
                radix,
                magnitude,
                suffix,
            }
        }
        Err(_) => {
            #[cfg(test)]
            record(false);
            DecodedLiteral::InvalidInteger {
                negative,
                radix,
                suffix,
                reason: IntegerLexicalError::InvalidMagnitude,
            }
        }
    }
}
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub(crate) struct IntegerDecodeCounts {
    pub(crate) attempts: u64,
    pub(crate) successes: u64,
    pub(crate) failures: u64,
}
#[cfg(test)]
std::thread_local! { static OBSERVER: std::cell::Cell<Option<IntegerDecodeCounts>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
fn record(success: bool) {
    OBSERVER.with(|observer| {
        if let Some(mut c) = observer.get() {
            c.attempts = c.attempts.checked_add(1).expect("decode observer overflow");
            if success {
                c.successes = c
                    .successes
                    .checked_add(1)
                    .expect("decode observer overflow");
            } else {
                c.failures = c.failures.checked_add(1).expect("decode observer overflow");
            }
            observer.set(Some(c));
        }
    });
}
#[cfg(test)]
pub(crate) fn observe_integer_decoding<T>(work: impl FnOnce() -> T) -> (T, IntegerDecodeCounts) {
    struct Restore(Option<IntegerDecodeCounts>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OBSERVER.with(|c| c.set(self.0));
        }
    }
    let previous = OBSERVER.with(|c| c.replace(Some(IntegerDecodeCounts::default())));
    let restore = Restore(previous);
    let value = work();
    let counts = OBSERVER.with(|c| c.get().expect("observer retained during scoped work"));
    drop(restore);
    (value, counts)
}
