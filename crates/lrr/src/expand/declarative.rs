//! Iterative declaration expansion admission/adoption. The frontend resolves definitions;
//! the pinned adapter supplies tokens. No compiler, filesystem, macro helper or native loader.
use crate::core::{diagnostic::Diagnostic, digest::Digest};
use crate::syntax::token_wire::{TokenForest, TokenTree};
use serde::Serialize;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ExpansionLimits {
    pub frames: u32,
    pub invocations: u64,
    pub generated_tokens: u64,
    pub single_output_bytes: u64,
    pub cumulative_output_bytes: u64,
}
impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            frames: 256,
            invocations: 100_000,
            generated_tokens: 2_000_000,
            single_output_bytes: 32 * 1024 * 1024,
            cumulative_output_bytes: 256 * 1024 * 1024,
        }
    }
}
impl ExpansionLimits {
    pub fn validate(self) -> Result<Self, Diagnostic> {
        let max = Self::default();
        if self.frames == 0
            || self.frames > max.frames
            || self.invocations == 0
            || self.invocations > max.invocations
            || self.generated_tokens == 0
            || self.generated_tokens > max.generated_tokens
            || self.single_output_bytes == 0
            || self.single_output_bytes > max.single_output_bytes
            || self.cumulative_output_bytes == 0
            || self.cumulative_output_bytes > max.cumulative_output_bytes
        {
            Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "expansion limits must be positive reductions of the fixed defaults",
            ))
        } else {
            Ok(self)
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct FrameId(u64);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct ExpansionCounts {
    pub invocations: u64,
    pub generated_tokens: u64,
    pub generated_bytes: u64,
    pub max_frames: u32,
}
struct Frame {
    id: FrameId,
    definition: Digest,
    adopted: bool,
}
/// One request owns this state. A failure poisons the request; a fresh request is explicit.
pub struct DeclarativeSession {
    limits: ExpansionLimits,
    frames: Vec<Frame>,
    next_id: u64,
    counts: ExpansionCounts,
    failed: Option<Diagnostic>,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
impl DeclarativeSession {
    pub fn new(limits: ExpansionLimits) -> Result<Self, Diagnostic> {
        Ok(Self {
            limits: limits.validate()?,
            frames: Vec::new(),
            next_id: 1,
            counts: ExpansionCounts {
                invocations: 0,
                generated_tokens: 0,
                generated_bytes: 0,
                max_frames: 0,
            },
            failed: None,
        })
    }
    pub fn counts(&self) -> ExpansionCounts {
        self.counts
    }
    pub fn active_definitions(&self) -> impl Iterator<Item = Digest> + '_ {
        self.frames.iter().map(|f| f.definition)
    }
    fn ready(&self) -> Result<(), Diagnostic> {
        match &self.failed {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn reject<T>(&mut self, code: &'static str, message: &str) -> Result<T, Diagnostic> {
        let error = fault(code, message);
        self.failed = Some(error.clone());
        Err(error)
    }
    /// Admit before invoking the backend; parent must be the current active frame.
    pub fn begin(
        &mut self,
        parent: Option<FrameId>,
        definition: Digest,
    ) -> Result<FrameId, Diagnostic> {
        self.ready()?;
        if self.frames.last().map(|f| f.id) != parent {
            return self.reject(
                "LRR-E-MACRO-TOKEN-WIRE",
                "declarative frame parent differs from active stack",
            );
        }
        if self.frames.last().is_some_and(|f| !f.adopted) {
            return self.reject(
                "LRR-E-MACRO-TOKEN-WIRE",
                "parent output must be adopted before nested expansion",
            );
        }
        if self.frames.len() >= self.limits.frames as usize {
            return self.reject(
                "LRR-E-EXPANSION-CYCLE",
                "declarative expansion frame limit exceeded",
            );
        }
        let Some(invocations) = self.counts.invocations.checked_add(1) else {
            return self.reject("LRR-E-MACRO-OUTPUT", "invocation count overflow");
        };
        if invocations > self.limits.invocations {
            return self.reject(
                "LRR-E-MACRO-OUTPUT",
                "declarative invocation limit exceeded",
            );
        }
        let Some(next) = self.next_id.checked_add(1) else {
            return self.reject("LRR-E-MACRO-OUTPUT", "declarative frame identity exhausted");
        };
        if self.frames.try_reserve(1).is_err() {
            return self.reject("LRR-E-MACRO-OUTPUT", "declarative frame allocation failed");
        }
        let id = FrameId(self.next_id);
        self.next_id = next;
        self.frames.push(Frame {
            id,
            definition,
            adopted: false,
        });
        self.counts.invocations = invocations;
        self.counts.max_frames = self.counts.max_frames.max(
            u32::try_from(self.frames.len())
                .map_err(|_| fault("LRR-E-MACRO-OUTPUT", "frame count overflow"))?,
        );
        Ok(id)
    }
    /// Count/check a completed candidate before publishing its tokens to nested demands/scopes.
    pub fn adopt(&mut self, id: FrameId, output: &TokenForest) -> Result<(), Diagnostic> {
        self.ready()?;
        if !self.frames.last().is_some_and(|f| f.id == id && !f.adopted) {
            return self.reject(
                "LRR-E-MACRO-TOKEN-WIRE",
                "declarative frame output already adopted or out of order",
            );
        }
        if let Err(error) = output.validate() {
            return self.reject(error.code, &error.message);
        }
        let mut count = 0u64;
        let mut stack: Vec<_> = output.roots().iter().collect();
        while let Some(token) = stack.pop() {
            let Some(next) = count.checked_add(1) else {
                return self.reject("LRR-E-MACRO-OUTPUT", "generated token count overflow");
            };
            count = next;
            if let TokenTree::Group { children, .. } = token {
                stack.extend(children)
            }
        }
        let Some(tokens) = self.counts.generated_tokens.checked_add(count) else {
            return self.reject("LRR-E-MACRO-OUTPUT", "generated token total overflow");
        };
        if tokens > self.limits.generated_tokens {
            return self.reject(
                "LRR-E-MACRO-OUTPUT",
                "cumulative generated token limit exceeded",
            );
        }
        let remaining = self
            .limits
            .cumulative_output_bytes
            .saturating_sub(self.counts.generated_bytes);
        let byte_limit = self.limits.single_output_bytes.min(remaining);
        let byte_limit = match usize::try_from(byte_limit) {
            Ok(limit) => limit,
            Err(_) => return self.reject("LRR-E-MACRO-OUTPUT", "generated output size overflow"),
        };
        let bytes = match output.encoded_len_with_limit(byte_limit) {
            Ok(bytes) => match u64::try_from(bytes) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return self.reject("LRR-E-MACRO-OUTPUT", "generated output size overflow");
                }
            },
            Err(error) => return self.reject(error.code, &error.message),
        };
        let Some(total) = self.counts.generated_bytes.checked_add(bytes) else {
            return self.reject("LRR-E-MACRO-OUTPUT", "generated byte total overflow");
        };
        if bytes > self.limits.single_output_bytes || total > self.limits.cumulative_output_bytes {
            return self.reject("LRR-E-MACRO-OUTPUT", "generated output byte limit exceeded");
        }
        let Some(frame) = self.frames.last_mut() else {
            return self.reject(
                "LRR-E-MACRO-TOKEN-WIRE",
                "active frame disappeared before adoption",
            );
        };
        frame.adopted = true;
        self.counts.generated_tokens = tokens;
        self.counts.generated_bytes = total;
        Ok(())
    }
    pub fn finish(&mut self, id: FrameId) -> Result<(), Diagnostic> {
        self.ready()?;
        if !self.frames.last().is_some_and(|f| f.id == id && f.adopted) {
            return self.reject(
                "LRR-E-MACRO-TOKEN-WIRE",
                "declarative frame finish is out of order or unadopted",
            );
        };
        self.frames.pop();
        Ok(())
    }
    pub fn abort(&mut self, error: Diagnostic) {
        if self.failed.is_none() {
            self.failed = Some(error)
        }
    }
}
