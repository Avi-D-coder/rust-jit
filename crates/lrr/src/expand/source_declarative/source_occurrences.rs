//! Structural position provenance, independent of diagnostic token origins.
//! Only the actual assembly edit/record path mutates this bounded private map.
use super::*;
use lrr_expand_adapter::syntax_attachment::SyntaxEdit;


/// Coordinates in the initial normalized attachment root are deliberately not
/// raw SourceBlob offsets. Consumers must separately join actual token origins,
/// source/context/edition and the original AST before granting source authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceOccurrence {
    Original {
        initial_syntax: Digest,
        range: SourceRange,
    },
    Generated {
        record: Digest,
        descriptor: Digest,
        parent_inventory: Digest,
        invocation: Digest,
        attachment: u32,
        range: SourceRange,
    },
    Mixed,
    Pending,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Output {
    record: Digest,
    descriptor: Digest,
    parent_inventory: Digest,
    invocation: Digest,
    attachment: u32,
    length: u32,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Origin {
    Original,
    Generated(Output),
    Pending,
}
#[derive(Debug, Clone, Copy)]
struct Interval {
    range: SourceRange,
    origin: Origin,
    offset: u32,
}
pub(super) struct SourceOccurrences {
    initial_syntax: Digest,
    initial_len: u32,
    len: u32,
    attachments: u32,
    intervals: Vec<Interval>,
}
fn resource() -> Diagnostic {
    crate::runtime::arena::resource()
}
fn bytes(capacity: usize) -> Result<usize, Diagnostic> {
    capacity
        .checked_mul(std::mem::size_of::<Interval>())
        .and_then(|n| n.checked_add(std::mem::size_of::<SourceOccurrences>()))
        .ok_or_else(resource)
}
impl SourceOccurrences {
    pub(super) fn new(initial_syntax: Digest, len: u32) -> Result<Self, Diagnostic> {
        let mut intervals = Vec::new();
        if len > 0 {
            intervals.try_reserve_exact(1).map_err(|_| resource())?;
            intervals.push(Interval {
                range: SourceRange::checked(0, len)?,
                origin: Origin::Original,
                offset: 0,
            });
        }
        let map = Self {
            initial_syntax,
            initial_len: len,
            len,
            attachments: 0,
            intervals,
        };
        map.metadata_bytes()?;
        Ok(map)
    }
    pub(super) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        bytes(self.intervals.capacity())
    }
    fn push(intervals: &mut Vec<Interval>, interval: Interval) -> Result<(), Diagnostic> {
        if interval.range.is_empty() {
            return Ok(());
        }
        if let Some(prior) = intervals.last_mut() {
            if prior.range.end != interval.range.start {
                return Err(fault("structural occurrence coverage differs"));
            }
            let contiguous = prior.origin == Origin::Pending
                || prior.offset.checked_add(prior.range.len()) == Some(interval.offset);
            if prior.origin == interval.origin && contiguous {
                prior.range.end = interval.range.end;
                return Ok(());
            }
        } else if interval.range.start != 0 {
            return Err(fault("structural occurrence initial interval differs"));
        }
        if intervals.len() == intervals.capacity() {
            return Err(resource());
        }
        intervals.push(interval);
        Ok(())
    }
    /// The actual successful SyntaxEdit is the only positional authority.
    /// Generated output may copy any original token origin; this map never
    /// examines those origins when assigning a structural occurrence.
    pub(super) fn splice(
        &mut self,
        edit: SyntaxEdit,
        output: &GeneratedDeclarations,
        output_len: u32,
    ) -> Result<(), Diagnostic> {
        let (start, removed_end) = edit.removed();
        let (inserted_start, inserted_end) = edit.inserted();
        if start != inserted_start || start >= removed_end || removed_end > self.len {
            return Err(fault(
                "structural occurrence edit differs from current root",
            ));
        }
        let inserted_len = inserted_end.checked_sub(start).ok_or_else(resource)?;
        if output_len > inserted_len {
            return Err(fault(
                "structural occurrence output exceeds actual insertion",
            ));
        }
        let len = self
            .len
            .checked_sub(removed_end - start)
            .and_then(|n| n.checked_add(inserted_len))
            .ok_or_else(resource)?;
        let attachment = self.attachments;
        let attachments = attachment
            .checked_add(1)
            .ok_or_else(resource)?;
        let capacity = self
            .intervals
            .len()
            .checked_add(3)
            .ok_or_else(resource)?;
        // Bound both old and candidate allocation while the edit is staged.
        bytes(capacity)?
            .checked_add(self.metadata_bytes()?)
            .ok_or_else(resource)?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity).map_err(|_| resource())?;
        for interval in &self.intervals {
            if interval.range.start < start {
                let end = interval.range.end.min(start);
                Self::push(
                    &mut next,
                    Interval {
                        range: SourceRange::checked(interval.range.start, end)?,
                        ..*interval
                    },
                )?;
            }
        }
        let generated_end = start.checked_add(output_len).ok_or_else(resource)?;
        if output_len > 0 {
            Self::push(
                &mut next,
                Interval {
                    range: SourceRange::checked(start, generated_end)?,
                    origin: Origin::Generated(Output {
                        record: output.record().key().fingerprint(),
                        descriptor: output.record().descriptor().fingerprint(),
                        parent_inventory: output.parent_inventory(),
                        invocation: output.parent_invocation(),
                        attachment,
                        length: output_len,
                    }),
                    offset: 0,
                },
            )?;
        }
        if generated_end < inserted_end {
            // List attachment can carry an actual caller suffix beyond output
            // materialization. Without a retained exact suffix edit receipt it
            // is unjoined, not permission to forge an output-relative range.
            Self::push(
                &mut next,
                Interval {
                    range: SourceRange::checked(generated_end, inserted_end)?,
                    origin: Origin::Pending,
                    offset: 0,
                },
            )?;
        }
        for interval in &self.intervals {
            if interval.range.end > removed_end {
                let prior_start = interval.range.start.max(removed_end);
                let project = |offset: u32| {
                    offset
                        .checked_sub(removed_end)
                        .and_then(|n| inserted_end.checked_add(n))
                        .ok_or_else(resource)
                };
                let offset = interval
                    .offset
                    .checked_add(prior_start - interval.range.start)
                    .ok_or_else(resource)?;
                Self::push(
                    &mut next,
                    Interval {
                        range: SourceRange::checked(
                            project(prior_start)?,
                            project(interval.range.end)?,
                        )?,
                        origin: interval.origin,
                        offset,
                    },
                )?;
            }
        }
        let candidate = Self {
            initial_syntax: self.initial_syntax,
            initial_len: self.initial_len,
            len,
            attachments,
            intervals: next,
        };
        candidate.validate(len)?;
        *self = candidate;
        Ok(())
    }
    pub(super) fn validate(&self, len: u32) -> Result<(), Diagnostic> {
        if self.len != len {
            return Err(fault("structural occurrence root length/count differs"));
        }
        self.metadata_bytes()?;
        let mut position = 0;
        for interval in &self.intervals {
            let bound = match interval.origin {
                Origin::Original => self.initial_len,
                Origin::Generated(output) if output.attachment < self.attachments => output.length,
                Origin::Generated(_) => {
                    return Err(fault("structural occurrence attachment differs"));
                }
                Origin::Pending => u32::MAX,
            };
            if interval.range.start != position
                || interval.range.is_empty()
                || interval
                    .offset
                    .checked_add(interval.range.len())
                    .is_none_or(|n| n > bound)
            {
                return Err(fault("structural occurrence interval extent differs"));
            }
            position = interval.range.end;
        }
        if position != len {
            return Err(fault("structural occurrence final coverage differs"));
        }
        Ok(())
    }
    pub(super) fn occurrence(&self, range: SourceRange) -> Result<SourceOccurrence, Diagnostic> {
        if range.start > range.end || range.end > self.len {
            return Err(fault(
                "structural occurrence requested range escapes current root",
            ));
        }
        if range.is_empty() {
            return Ok(SourceOccurrence::Pending);
        }
        let index = self
            .intervals
            .partition_point(|interval| interval.range.end <= range.start);
        let interval = self
            .intervals
            .get(index)
            .ok_or_else(|| fault("structural occurrence requested interval absent"))?;
        if !interval.range.contains(range) {
            return Ok(SourceOccurrence::Mixed);
        }
        let start = interval
            .offset
            .checked_add(range.start - interval.range.start)
            .ok_or_else(resource)?;
        let projected =
            SourceRange::checked(start, start.checked_add(range.len()).ok_or_else(resource)?)?;
        Ok(match interval.origin {
            Origin::Original => SourceOccurrence::Original {
                initial_syntax: self.initial_syntax,
                range: projected,
            },
            Origin::Generated(output) => SourceOccurrence::Generated {
                record: output.record,
                descriptor: output.descriptor,
                parent_inventory: output.parent_inventory,
                invocation: output.invocation,
                attachment: output.attachment,
                range: projected,
            },
            Origin::Pending => SourceOccurrence::Pending,
        })
    }
    /// Inverse structural projection: the current root range of an exact
    /// output-relative range of one attached record. Only the actual splice
    /// path places output intervals, so this never consults token origins.
    /// None when the range is not (or no longer) one contiguous output run.
    pub(super) fn project_generated(
        &self,
        record: Digest,
        range: SourceRange,
    ) -> Result<Option<SourceRange>, Diagnostic> {
        let mut found = None;
        for interval in &self.intervals {
            let Origin::Generated(output) = interval.origin else {
                continue;
            };
            let end = interval
                .offset
                .checked_add(interval.range.len())
                .ok_or_else(resource)?;
            if output.record != record || range.start < interval.offset || range.end > end {
                continue;
            }
            if found.is_some() {
                return Err(fault("structural generated occurrence is ambiguous"));
            }
            let start = interval
                .range
                .start
                .checked_add(range.start - interval.offset)
                .ok_or_else(resource)?;
            found = Some(SourceRange::checked(
                start,
                start.checked_add(range.len()).ok_or_else(resource)?,
            )?);
        }
        Ok(found)
    }
    pub(super) fn encode(&self, e: &mut StableEncoder) {
        e.digest(self.initial_syntax);
        e.u32(self.initial_len);
        e.u32(self.len);
        e.u32(self.attachments);
        e.sequence(&self.intervals, |e, interval| {
            e.u32(interval.range.start);
            e.u32(interval.range.end);
            e.u32(interval.offset);
            match interval.origin {
                Origin::Original => e.u8(0),
                Origin::Generated(output) => {
                    e.u8(1);
                    e.digest(output.record);
                    e.digest(output.descriptor);
                    e.digest(output.parent_inventory);
                    e.digest(output.invocation);
                    e.u32(output.attachment);
                    e.u32(output.length);
                }
                Origin::Pending => e.u8(2),
            }
        });
    }
}

#[cfg(test)]
#[path = "source_occurrences_tests.rs"]
mod tests;
