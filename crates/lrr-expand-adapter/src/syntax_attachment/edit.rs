//! Exact successful splice coordinates for translating retained source anchors.
use super::*;

/// This receipt describes the actual tree edit, including any caller semicolon
/// carried by a list replacement. It grants no declaration identity/authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxEdit {
    start: u32,
    removed_end: u32,
    inserted_end: u32,
    old_len: u32,
    new_len: u32,
}
impl SyntaxEdit {
    pub(super) fn new(
        range: TextRange,
        replacement_len: u32,
        old_len: u32,
        new_len: u32,
    ) -> Result<Self, AdapterError> {
        let inserted_end = u32::from(range.start())
            .checked_add(replacement_len)
            .filter(|end| *end <= new_len)
            .ok_or_else(|| error("syntax edit insertion extent overflow"))?;
        if range.is_empty()
            || u32::from(range.end()) > old_len
            || old_len
                .checked_sub(u32::from(range.len()))
                .and_then(|n| n.checked_add(replacement_len))
                != Some(new_len)
        {
            return Err(error("syntax edit old/new extent differs"));
        }
        Ok(Self {
            start: range.start().into(),
            removed_end: range.end().into(),
            inserted_end,
            old_len,
            new_len,
        })
    }
    pub fn removed(&self) -> (u32, u32) {
        (self.start, self.removed_end)
    }
    pub fn inserted(&self) -> (u32, u32) {
        (self.start, self.inserted_end)
    }

    /// Translate a nonempty original syntax-node range. A wholly replaced node
    /// has no original identity in the new tree, even if identical tokens were
    /// emitted. Containing nodes survive and acquire the exact new extent.
    pub fn map_original_range(
        &self,
        start: u32,
        end: u32,
    ) -> Result<Option<(u32, u32)>, AdapterError> {
        if start >= end || end > self.old_len {
            return Err(error("original syntax anchor extent differs"));
        }
        if end <= self.start {
            return Ok(Some((start, end)));
        }
        let shift = |offset: u32| {
            offset
                .checked_sub(self.removed_end)
                .and_then(|n| self.inserted_end.checked_add(n))
                .filter(|n| *n <= self.new_len)
                .ok_or_else(|| error("original syntax anchor translation overflow"))
        };
        if start >= self.removed_end {
            return Ok(Some((shift(start)?, shift(end)?)));
        }
        if self.start <= start && end <= self.removed_end {
            return Ok(None);
        }
        if start <= self.start && self.removed_end <= end {
            return Ok(Some((start, shift(end)?)));
        }
        Err(error("original syntax anchor crosses a splice boundary"))
    }
}
