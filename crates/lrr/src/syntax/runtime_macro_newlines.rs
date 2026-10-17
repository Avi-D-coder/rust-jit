//! Scratch-only CRLF normalization with exact original argument boundaries.
//! This mapping grants no source, namespace, token or expansion authority.
use crate::core::diagnostic::Diagnostic;
use std::borrow::Cow;


fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-MACRO-ARGUMENTS", message.into())
}

pub(super) struct NormalizedArguments<'a> {
    original: &'a str,
    normalized: Cow<'a, str>,
    // Normalized offsets immediately before each retained LF. The removed CR
    // belongs before an expression starting here, but after one ending here.
    removed: Vec<usize>,
}

impl<'a> NormalizedArguments<'a> {
    pub(super) fn new(original: &'a str) -> Result<Self, Diagnostic> {
        let count = original
            .as_bytes()
            .windows(2)
            .filter(|pair| *pair == b"\r\n")
            .count();
        if count == 0 {
            return Ok(Self {
                original,
                normalized: Cow::Borrowed(original),
                removed: Vec::new(),
            });
        }
        // Input length bounds the text and at most one table entry per CRLF.
        // Check the complete scratch size before either fallible reservation.
        count
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(original.len()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut normalized = String::new();
        normalized
            .try_reserve_exact(original.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        let mut removed = Vec::new();
        removed
            .try_reserve_exact(count)
            .map_err(|_| crate::runtime::arena::resource())?;
        // One pass only: an original CRCRLF keeps its first CR. All slicing is
        // at ASCII delimiters and therefore preserves the original UTF-8.
        for line in original.split_inclusive('\n') {
            if let Some(prefix) = line.strip_suffix("\r\n") {
                let at = normalized
                    .len()
                    .checked_add(prefix.len())
                    .ok_or_else(crate::runtime::arena::resource)?;
                normalized.push_str(prefix);
                removed.push(at);
                normalized.push('\n');
            } else {
                normalized.push_str(line);
            }
        }
        Ok(Self {
            original,
            normalized: Cow::Owned(normalized),
            removed,
        })
    }

    pub(super) fn as_str(&self) -> &str {
        &self.normalized
    }

    /// Offsets are relative to this normalized input, after the caller removes
    /// any parser-wrapper prefix. Only the caller can attach an original span.
    pub(super) fn original_range(
        &self,
        start: usize,
        end: usize,
    ) -> Result<(usize, usize), Diagnostic> {
        let normalized = self.as_str();
        if start > end
            || end > normalized.len()
            || !normalized.is_char_boundary(start)
            || !normalized.is_char_boundary(end)
        {
            return Err(invalid("normalized macro argument range is invalid"));
        }
        let start = start
            .checked_add(self.removed.partition_point(|at| *at <= start))
            .ok_or_else(crate::runtime::arena::resource)?;
        let end = end
            .checked_add(self.removed.partition_point(|at| *at < end))
            .ok_or_else(crate::runtime::arena::resource)?;
        if start > end
            || end > self.original.len()
            || !self.original.is_char_boundary(start)
            || !self.original.is_char_boundary(end)
        {
            // A zero-width range exactly at a removed CR has incompatible
            // start/end biases; refuse it rather than invent an original span.
            return Err(invalid("original macro argument range is invalid"));
        }
        Ok((start, end))
    }
}
