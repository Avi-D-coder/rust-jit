//! Deterministic item transforms. Resolution and execution remain external demands.
use super::{
    identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
    record::ExpansionRecord,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    syntax::token_wire::TokenForest,
};
use std::{collections::BTreeSet, sync::Arc};
const MAX_SLOTS: usize = 4096;
const MAX_TRANSFORMS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-TRANSFORM", message.into())
}
fn bounded_add(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .filter(|v| *v <= MAX_BYTES)
        .ok_or_else(|| fault("transformation metadata limit"))
}
fn token_digest(tokens: &TokenForest) -> Result<Digest, Diagnostic> {
    Ok(Digest::of_bytes(
        &tokens.encode().map_err(|e| fault(&e.to_string()))?,
    ))
}
#[derive(Debug, Clone)]
pub enum AttributeSlot {
    Unresolved(Digest),
    Inert,
    Expand(Box<ExpansionKey>),
}
#[derive(Debug, Clone)]
pub enum DeriveSlot {
    Unresolved(Digest),
    Expand(Box<ExpansionKey>),
}
impl AttributeSlot {
    pub fn expand(key: ExpansionKey) -> Self {
        Self::Expand(Box::new(key))
    }
}
impl DeriveSlot {
    pub fn expand(key: ExpansionKey) -> Self {
        Self::Expand(Box::new(key))
    }
}
/// Owned resolver facts for one item. Helper admission must precede an Inert decision.
#[derive(Debug, Clone)]
pub struct ItemPlan {
    owner: Digest,
    cfg: Digest,
    item: Arc<TokenForest>,
    attributes: Vec<AttributeSlot>,
    derives: Vec<DeriveSlot>,
}
impl ItemPlan {
    pub fn new(
        owner: Digest,
        cfg: Digest,
        item: Arc<TokenForest>,
        attributes: Vec<AttributeSlot>,
        derives: Vec<DeriveSlot>,
    ) -> Result<Self, Diagnostic> {
        let plan = Self {
            owner,
            cfg,
            item,
            attributes,
            derives,
        };
        plan.bytes()?;
        Ok(plan)
    }
    fn bytes(&self) -> Result<usize, Diagnostic> {
        if self.attributes.len() > MAX_SLOTS || self.derives.len() > MAX_SLOTS {
            return Err(fault("transformation slot limit"));
        }
        let mut bytes = self
            .item
            .allocated_bytes()
            .map_err(|e| fault(&e.to_string()))?;
        bytes = bounded_add(
            bytes,
            self.attributes
                .capacity()
                .checked_mul(std::mem::size_of::<AttributeSlot>())
                .ok_or_else(|| fault("attribute allocation overflow"))?,
        )?;
        bytes = bounded_add(
            bytes,
            self.derives
                .capacity()
                .checked_mul(std::mem::size_of::<DeriveSlot>())
                .ok_or_else(|| fault("derive allocation overflow"))?,
        )?;
        for key in self
            .attributes
            .iter()
            .filter_map(|s| {
                if let AttributeSlot::Expand(k) = s {
                    Some(k)
                } else {
                    None
                }
            })
            .chain(self.derives.iter().filter_map(|s| {
                if let DeriveSlot::Expand(k) = s {
                    Some(k)
                } else {
                    None
                }
            }))
        {
            bytes = bounded_add(
                bytes,
                bounded_add(
                    std::mem::size_of::<ExpansionKey>(),
                    super::record::key_heap_bytes(key),
                )?,
            )?;
        }
        bounded_add(bytes, std::mem::size_of::<Self>())
    }
    fn validate(
        &self,
        env: Digest,
        epoch: u64,
        consumed: &BTreeSet<Digest>,
    ) -> Result<(), Diagnostic> {
        self.bytes()?;
        let mut ids = BTreeSet::new();
        for (key, kind) in self
            .attributes
            .iter()
            .filter_map(|s| {
                if let AttributeSlot::Expand(k) = s {
                    Some((k, ProceduralKind::Attribute))
                } else {
                    None
                }
            })
            .chain(self.derives.iter().filter_map(|s| {
                if let DeriveSlot::Expand(k) = s {
                    Some((k, ProceduralKind::Derive))
                } else {
                    None
                }
            }))
        {
            validate_key(key, kind, env, epoch)?;
            if !ids.insert(key.invocation) || consumed.contains(&key.invocation) {
                return Err(fault("duplicate or consumed transformation invocation"));
            }
        }
        Ok(())
    }
}
fn validate_key(
    key: &ExpansionKey,
    kind: ProceduralKind,
    env: Digest,
    epoch: u64,
) -> Result<(), Diagnostic> {
    key.validate()?;
    let identity_matches = match &key.macro_identity {
        MacroIdentity::Procedural { kind: k, .. } => {
            *k == kind && key.procedural_epoch == Some(epoch)
        }
        MacroIdentity::Builtin { .. } => {
            kind == ProceduralKind::Derive && key.procedural_epoch.is_none()
        }
        MacroIdentity::Declaration { .. } => false,
    };
    if key.grammar != GrammarSlot::Items || key.consumer_compile_env != env || !identity_matches {
        return Err(fault(
            "transformation kind/grammar/environment/epoch mismatch",
        ));
    }
    Ok(())
}
struct Replacement {
    ticket: Digest,
    owner: Digest,
    cfg: Digest,
    record: ExpansionRecord,
    tokens: Arc<TokenForest>,
}
/// No action is executable here: callers fulfil Expand through authorized macro service.
pub enum TransformStep<'a> {
    NeedAttribute {
        index: usize,
        revision: u64,
        identity: Digest,
    },
    NeedDerive {
        index: usize,
        revision: u64,
        identity: Digest,
    },
    Expand(&'a ExpansionKey),
    NeedReanalysis {
        ticket: &'a Digest,
        tokens: &'a Arc<TokenForest>,
    },
    Complete {
        item: &'a Arc<TokenForest>,
        appended: &'a [ExpansionRecord],
    },
}
pub struct ItemTransformWorklist {
    plan: ItemPlan,
    env: Digest,
    epoch: u64,
    revision: u64,
    derive_cursor: usize,
    replacement: Option<Replacement>,
    appended: Vec<ExpansionRecord>,
    consumed: BTreeSet<Digest>,
    /// Counts admitted outputs even when a replaced plan releases their storage.
    transforms: usize,
}
impl ItemTransformWorklist {
    pub(crate) fn matches_item_context(&self, owner: Digest, cfg: Digest) -> bool {
        self.plan.owner == owner && self.plan.cfg == cfg
    }
    pub fn new(plan: ItemPlan, env: Digest, epoch: u64) -> Result<Self, Diagnostic> {
        plan.validate(env, epoch, &BTreeSet::new())?;
        Ok(Self {
            plan,
            env,
            epoch,
            revision: 0,
            derive_cursor: 0,
            replacement: None,
            appended: Vec::new(),
            consumed: BTreeSet::new(),
            transforms: 0,
        })
    }
    pub fn poll(&self) -> TransformStep<'_> {
        if let Some(r) = &self.replacement {
            return TransformStep::NeedReanalysis {
                ticket: &r.ticket,
                tokens: &r.tokens,
            };
        }
        for (index, slot) in self.plan.attributes.iter().enumerate() {
            match slot {
                AttributeSlot::Unresolved(identity) => {
                    return TransformStep::NeedAttribute {
                        index,
                        revision: self.revision,
                        identity: *identity,
                    };
                }
                AttributeSlot::Expand(k) => return TransformStep::Expand(k),
                AttributeSlot::Inert => {}
            }
        }
        if let Some(slot) = self.plan.derives.get(self.derive_cursor) {
            match slot {
                DeriveSlot::Unresolved(identity) => {
                    return TransformStep::NeedDerive {
                        index: self.derive_cursor,
                        revision: self.revision,
                        identity: *identity,
                    };
                }
                DeriveSlot::Expand(k) => return TransformStep::Expand(k),
            }
        }
        TransformStep::Complete {
            item: &self.plan.item,
            appended: &self.appended,
        }
    }
    pub fn resolve_attribute(
        &mut self,
        index: usize,
        revision: u64,
        decision: AttributeSlot,
    ) -> Result<(), Diagnostic> {
        if !matches!(self.poll(),TransformStep::NeedAttribute{index:i,revision:r,..} if i==index&&r==revision)
            || matches!(decision, AttributeSlot::Unresolved(_))
        {
            return Err(fault("attribute resolution is stale or incomplete"));
        }
        if let AttributeSlot::Expand(k) = &decision {
            self.check_resolution(k, ProceduralKind::Attribute)?;
        }
        self.plan.attributes[index] = decision;
        Ok(())
    }
    pub fn resolve_derive(
        &mut self,
        index: usize,
        revision: u64,
        key: ExpansionKey,
    ) -> Result<(), Diagnostic> {
        if !matches!(self.poll(),TransformStep::NeedDerive{index:i,revision:r,..} if i==index&&r==revision)
        {
            return Err(fault("derive resolution is stale"));
        }
        self.check_resolution(&key, ProceduralKind::Derive)?;
        self.plan.derives[index] = DeriveSlot::expand(key);
        Ok(())
    }
    fn check_resolution(&self, key: &ExpansionKey, kind: ProceduralKind) -> Result<(), Diagnostic> {
        validate_key(key, kind, self.env, self.epoch)?;
        if self.consumed.contains(&key.invocation)
            || self
                .plan
                .attributes
                .iter()
                .any(|s| matches!(s,AttributeSlot::Expand(k) if k.invocation==key.invocation))
            || self
                .plan
                .derives
                .iter()
                .any(|s| matches!(s,DeriveSlot::Expand(k) if k.invocation==key.invocation))
        {
            return Err(fault("duplicate transformation resolution"));
        }
        bounded_add(
            self.retained_bytes()?,
            bounded_add(
                std::mem::size_of::<ExpansionKey>(),
                super::record::key_heap_bytes(key),
            )?,
        )?;
        Ok(())
    }
    fn retained_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self.plan.bytes()?;
        bytes = bounded_add(
            bytes,
            self.appended
                .capacity()
                .checked_mul(std::mem::size_of::<ExpansionRecord>())
                .ok_or_else(|| fault("append allocation overflow"))?,
        )?;
        bytes = bounded_add(
            bytes,
            self.consumed
                .len()
                .checked_mul(128)
                .ok_or_else(|| fault("invocation index overflow"))?,
        )?;
        for r in &self.appended {
            bytes = bounded_add(bytes, r.estimated_bytes()?)?;
        }
        if let Some(r) = &self.replacement {
            bytes = bounded_add(bytes, r.record.estimated_bytes()?)?;
            bytes = bounded_add(
                bytes,
                r.tokens
                    .allocated_bytes()
                    .map_err(|e| fault(&e.to_string()))?,
            )?;
        }
        Ok(bytes)
    }
    /// Exact immutable reply admission. Failures leave the worklist unchanged.
    pub fn admit(&mut self, record: ExpansionRecord) -> Result<(), Diagnostic> {
        let TransformStep::Expand(expected) = self.poll() else {
            return Err(fault("no transformation awaits a reply"));
        };
        if expected != record.key() || self.consumed.contains(&record.key().invocation) {
            return Err(fault("stale or mismatched transformation reply"));
        }
        if self.transforms >= MAX_TRANSFORMS {
            return Err(fault("transformation count limit"));
        }
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("transformation revision overflow"))?;
        let mut bytes = bounded_add(self.retained_bytes()?, record.estimated_bytes()?)?;
        bytes = bounded_add(bytes, 128 + std::mem::size_of::<ExpansionRecord>())?;
        let replacing = matches!(
            &record.key().macro_identity,
            MacroIdentity::Procedural {
                kind: ProceduralKind::Attribute,
                ..
            }
        );
        let replacement = if replacing {
            let tokens = Arc::new(record.tokens()?);
            bounded_add(
                bytes,
                tokens
                    .allocated_bytes()
                    .map_err(|e| fault(&e.to_string()))?,
            )?;
            let mut e = StableEncoder::new("lrr/v2/item-reanalysis-ticket");
            e.digest(self.plan.owner);
            e.digest(self.plan.cfg);
            e.u64(next_revision);
            e.record(record.key());
            e.digest(record.output());
            Some((e.finish(), tokens))
        } else {
            None
        };
        if !replacing {
            self.appended
                .try_reserve_exact(1)
                .map_err(|_| fault("append allocation failed"))?;
            // Reserve exact storage and account actual capacity before publishing a result.
            bounded_add(
                bounded_add(self.retained_bytes()?, record.estimated_bytes()?)?,
                128,
            )?;
        }
        self.consumed.insert(record.key().invocation);
        self.transforms += 1;
        if let Some((ticket, tokens)) = replacement {
            self.replacement = Some(Replacement {
                ticket,
                owner: self.plan.owner,
                cfg: self.plan.cfg,
                record,
                tokens,
            });
            self.revision = next_revision;
        } else {
            self.appended.push(record);
            self.derive_cursor += 1;
        }
        Ok(())
    }
    /// Reanalysis must cover precisely the replacement output under the same owner/cfg.
    /// Old attributes/derives are discarded; newly generated actions are resolved afresh.
    pub fn replan(&mut self, ticket: Digest, plan: ItemPlan) -> Result<(), Diagnostic> {
        let Some(r) = &self.replacement else {
            return Err(fault("no replacement awaits reanalysis"));
        };
        if ticket != r.ticket
            || plan.owner != r.owner
            || plan.cfg != r.cfg
            || token_digest(&plan.item)? != r.record.output()
        {
            return Err(fault("replacement reanalysis identity mismatch"));
        }
        plan.validate(self.env, self.epoch, &self.consumed)?;
        bounded_add(
            plan.bytes()?,
            self.consumed
                .len()
                .checked_mul(128)
                .ok_or_else(|| fault("invocation index overflow"))?,
        )?;
        self.plan = plan;
        self.replacement = None;
        self.appended.clear();
        self.derive_cursor = 0;
        Ok(())
    }
}
