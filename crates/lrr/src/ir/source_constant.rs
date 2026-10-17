//! Immutable source constant instruction data; never execution permission.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::key::ConstantKey,
    ir::{
        constant_data::{ConstValue, wire::ConstantWireType},
        pc::{PcKey, Phase, SlotId},
        recipe::{FactDependency, RecipeDraft, RecipeExit},
    },
    syntax::{body_index::BodyIndex, spans::BodySpan},
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceConstantInstruction {
    key: PcKey,
    next: PcKey,
    span: BodySpan,
    body_index: Digest,
    destination: SlotId,
    query: ConstantKey,
    path: Digest,
    wire: ConstantWireType,
    value: ConstValue,
    content: Digest,
    dependencies: Box<[FactDependency]>,
    bytes: usize,
    identity: Digest,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
impl SourceConstantInstruction {
    /// Data capture only. The coordinator supplies current source/query facts;
    /// VM consumers must independently require the fresh opaque admitted use.
    pub(crate) fn capture_data(
        body: &BodyIndex,
        key: &PcKey,
        query: &ConstantKey,
        path: Digest,
        wire: &ConstantWireType,
        value: &ConstValue,
        dependencies: Vec<FactDependency>,
    ) -> Result<Self, Diagnostic> {
        query.validate()?;
        let node = body
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("constant instruction node absent"))?;
        let destination = node
            .value
            .ok_or_else(|| invalid("constant instruction result absent"))?;
        if key.body != *body.body()
            || key.phase != Phase::Entry.code()?
            || query.result_type() != wire.fingerprint_checked()?
        {
            return Err(invalid("constant instruction source shape/key differs"));
        }
        let encoded = crate::ir::constant_data::wire::encode_value(value, wire)?;
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(wire.metadata_bytes()?)
            .and_then(|n| n.checked_add(encoded.len().checked_mul(8)?))
            .and_then(|n| n.checked_add(query.heap_bytes().ok()?))
            .ok_or_else(|| invalid("constant instruction metadata overflow"))?;
        for d in &dependencies {
            bytes = bytes
                .checked_add(std::mem::size_of::<FactDependency>())
                .and_then(|n| n.checked_add(d.heap_bytes()))
                .ok_or_else(|| invalid("constant instruction dependency metadata overflow"))?;
        }
        let mut instruction = Self {
            key: key.clone(),
            next: body.pc(node.id, Phase::Exit)?,
            span: body.span(node.id)?,
            body_index: body.fingerprint(),
            destination,
            query: query.clone(),
            path,
            wire: wire.clone(),
            value: value.clone(),
            content: Digest::of_bytes(&encoded),
            dependencies: dependencies.into_boxed_slice(),
            bytes,
            identity: Digest([0; 32]),
        };
        instruction.identity = instruction.fingerprint();
        Ok(instruction)
    }
    pub(crate) fn matches(
        &self,
        body: &BodyIndex,
        draft: &RecipeDraft,
        destination: SlotId,
    ) -> Result<bool, Diagnostic> {
        Ok(self.key == draft.key
            && self.next == body.pc(crate::ir::pc::NodeId(draft.key.node), Phase::Exit)?
            && self.span == draft.origin.span
            && self.body_index == body.fingerprint()
            && self.destination == destination
            && matches!(draft.exits.as_slice(),[RecipeExit::Next(next)] if next==&self.next)
            && self.dependencies.as_ref() == draft.dependencies.as_slice()
            && self.content
                == Digest::of_bytes(&crate::ir::constant_data::wire::encode_value(
                    &self.value,
                    &self.wire,
                )?))
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn destination(&self) -> SlotId {
        self.destination
    }
    pub(crate) fn query(&self) -> &ConstantKey {
        &self.query
    }
    pub(crate) fn path(&self) -> Digest {
        self.path
    }
    pub(crate) fn value(&self) -> &ConstValue {
        &self.value
    }
    pub(crate) fn wire(&self) -> &ConstantWireType {
        &self.wire
    }
    pub(crate) fn content(&self) -> Digest {
        self.content
    }
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn body_index(&self) -> Digest {
        self.body_index
    }
    pub(crate) fn dependencies(&self) -> &[FactDependency] {
        &self.dependencies
    }
    pub(crate) fn next(&self) -> &PcKey {
        &self.next
    }
}
impl StableEncode for SourceConstantInstruction {
    const DOMAIN: &'static str = "lrr/v1/source-constant-instruction";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.next);
        e.record(&self.span.body_key);
        e.u32(self.span.relative_start);
        e.u32(self.span.relative_end);
        e.digest(self.body_index);
        e.u32(self.destination.0);
        e.record(&self.query);
        e.digest(self.path);
        e.digest(self.content);
        // Canonical checked wire identity is already bound by query.result_type.
        e.digest(self.query.result_type());
        e.record(&self.value);
        e.sequence(&self.dependencies, |e, d| e.record(d));
    }
}
