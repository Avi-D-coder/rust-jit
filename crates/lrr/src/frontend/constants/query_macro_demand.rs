//! Uncached actual reached-site data, never macro or runtime execution authority.
use super::{ConstFailure, query_identity::ConstQueryIdentity};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    frontend::constant_source_admission::ConstantSourceAdmission,
    source::{changes::PreparedSourceView, manifest::BodyKey},
    syntax::spans::SourceRange,
};
#[derive(Debug, Clone)]
pub struct ConstantMacroDemand {
    key: ConstQueryIdentity,
    evaluated_key: ConstQueryIdentity,
    source: Digest,
    body: BodyKey,
    range: SourceRange,
    steps: u32,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-MACRO-SOURCE", message.into())
}
impl ConstantMacroDemand {
    pub(crate) fn capture(
        key: &ConstQueryIdentity,
        admission: &ConstantSourceAdmission,
        failure: &ConstFailure,
    ) -> Result<Self, Diagnostic> {
        key.validate()?;
        let plan = admission.plan();
        let site = failure
            .required_macro()
            .ok_or_else(|| invalid("constant failure has no reached macro site"))?;
        if key.universe().is_none()
            || key.admission() != admission.identity()
            || key.selection() != plan.identity()
            || key.source() != plan.source().source()
            || key.definition() != plan.owner_definition()
            || key.cfg() != plan.source().cfg()
            || key.profile() != plan.profile()
            || failure.requested_plan() != Some(plan.identity())
            || site.source() != failure.source()
            || site.source() != key.source()
            || site.range() != failure.range()
            || site.range().is_empty()
            || failure.evaluation_steps() == 0
            || failure.panic().is_some()
            || failure.numeric_panic().is_some()
            || failure.diagnostic().code != "LRR-E-CONST-MACRO-SOURCE"
        {
            return Err(invalid(
                "constant macro demand does not match its actual evaluated plan",
            ));
        }
        let value = Self {
            key: key.clone(),
            evaluated_key: key.clone(),
            source: site.source(),
            body: site.body().clone(),
            range: site.range(),
            steps: failure.evaluation_steps(),
        };
        value.metadata_bytes()?;
        Ok(value)
    }
    pub(crate) fn key(&self) -> &ConstQueryIdentity {
        &self.key
    }
    pub(crate) fn evaluated_key(&self) -> &ConstQueryIdentity {
        &self.evaluated_key
    }
    /// Forward only a genuine scalar length child's actual demand. No evaluation,
    /// source selection or macro effects occur in this immutable wrapper factory.
    pub(crate) fn forward_length(
        parent_key: &ConstQueryIdentity,
        parent: &ConstantSourceAdmission,
        child_key: &ConstQueryIdentity,
        child: &ConstantSourceAdmission,
        demand: &Self,
    ) -> Result<Self, Diagnostic> {
        parent_key.validate()?;
        child_key.validate()?;
        let (constant, call, length) = child.plan().entry()?;
        if parent_key == child_key
            || child_key != demand.key()
            || child_key != demand.evaluated_key()
            || constant.is_some()
            || call.is_some()
            || length.is_none()
            || parent_key.admission() != parent.identity()
            || parent_key.selection() != parent.plan().identity()
            || parent_key.definition() != parent.plan().owner_definition()
            || child_key.admission() != child.identity()
            || child_key.selection() != child.plan().identity()
            || parent_key.source() != child_key.source()
            || parent_key.source() != demand.source
            || parent_key.definition() != child_key.definition()
            || parent_key.cfg() != child_key.cfg()
            || parent_key.profile() != child_key.profile()
            || parent_key.universe() != child_key.universe()
            || child_key.result_type()
                != super::wire::ConstantWireType::Scalar(crate::ir::op::PrimitiveType::Usize)
                    .fingerprint_checked()?
        {
            return Err(invalid(
                "constant macro demand is not its actual parent length child",
            ));
        }
        let value = Self {
            key: parent_key.clone(),
            evaluated_key: child_key.clone(),
            source: demand.source,
            body: demand.body.clone(),
            range: demand.range,
            steps: demand.steps,
        };
        value.metadata_bytes()?;
        Ok(value)
    }
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn evaluation_steps(&self) -> u32 {
        self.steps
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(128)
            .and_then(|n| n.checked_add(self.key.heap_bytes().ok()?))
            .and_then(|n| n.checked_add(self.evaluated_key.heap_bytes().ok()?))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| invalid("constant macro demand metadata overflow"))
    }
    pub(crate) fn validate_current(
        &self,
        db: &Database,
        source: &PreparedSourceView,
        admission: &ConstantSourceAdmission,
    ) -> Result<(), Diagnostic> {
        admission.revalidate(db, source)?;
        if self.key.universe() != Some(db.constant_input_universe()?)
            || self.key.admission() != admission.identity()
            || self.key.selection() != admission.plan().identity()
            || self.source != admission.plan().source().source()
            || !matches!(db.published_input(&InputKey::DefinitionBody(self.body.definition)),
                Some((InputValue::DefinitionBody(body), _)) if body.key() == self.body && body.range.contains(self.range))
        {
            return Err(invalid(
                "constant macro reached source/body/publication changed",
            ));
        }
        if self.key != self.evaluated_key {
            admission
                .metadata_bytes()
                .checked_mul(2)
                .and_then(|n| n.checked_add(self.metadata_bytes().ok()?))
                .filter(|n| *n <= 32 << 20)
                .ok_or_else(|| invalid("constant macro child replay metadata limit"))?;
            let proof =
                crate::frontend::constant_result_type::CurrentConstantResultType::capture_current(
                    db,
                    source,
                    admission.clone(),
                    32 << 20,
                )?;
            let (_, length) = proof
                .array()
                .ok_or_else(|| invalid("constant macro parent no longer has a length child"))?;
            let child = admission.reselect_array_length_current(db, source, length, 32 << 20)?;
            let expected = crate::db::key::ConstantKey::capture(
                child.plan(),
                &length.expected_wire(),
                child.identity(),
            )?
            .bind_universe(db.constant_input_universe()?);
            if expected != self.evaluated_key {
                return Err(invalid(
                    "constant macro length child current identity differs",
                ));
            }
        }
        Ok(())
    }
}
impl StableEncode for ConstantMacroDemand {
    const DOMAIN: &'static str = "lrr/v2/uncached-reached-constant-macro-demand";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.evaluated_key);
        e.digest(self.source);
        e.record(&self.body);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.u32(self.steps);
    }
}
