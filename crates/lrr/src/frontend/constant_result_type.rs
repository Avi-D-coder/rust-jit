//! T054 source-proven result-type readiness, never a constant evaluator.
#[cfg(test)]
use crate::frontend::constant_source_admission::ConstantRequest;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    db::memo::Database,
    frontend::{
        constant_source_admission::ConstantSourceAdmission, constants::wire::ConstantWireType,
    },
    ir::op::PrimitiveType,
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{
        outline::{BodyHandle, DeclarationContext},
        pure_constants::{ConstType, ConstantPlan},
        spans::SourceRange,
    },
};
const MAX_BYTES: usize = 32 << 20;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-RESULT-TYPE-SOURCE", message.into())
}
fn mismatch() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CONST-TYPE",
        "caller expected type differs from the exact declared constant result type".into(),
    )
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "constant result-type readiness metadata limit".into(),
    )
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Role {
    Constant,
    ConstCall,
    ArrayLength,
}
impl Role {
    fn tag(self) -> u8 {
        match self {
            Self::Constant => 1,
            Self::ConstCall => 2,
            Self::ArrayLength => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum HeaderShape {
    Scalar(PrimitiveType),
    Array(PrimitiveType, SourceRange),
}
fn declared_shape(plan: &ConstantPlan) -> Result<(Role, HeaderShape), Diagnostic> {
    let (constant, call, length) = plan.entry()?;
    let (role, ty) = match (constant, call, length) {
        (Some(constant), None, None) => (Role::Constant, constant.ty),
        (None, Some((function, _)), None) => (Role::ConstCall, function.result),
        // This is the language's array-length role, independent of a user
        // declaration named usize. The source plan already proves exact syntax.
        (None, None, Some(_)) => {
            return Ok((Role::ArrayLength, HeaderShape::Scalar(PrimitiveType::Usize)));
        }
        _ => {
            return Err(fault(
                "constant selected entry does not have one exact result role",
            ));
        }
    };
    Ok((
        role,
        match ty {
            ConstType::Scalar(primitive) => HeaderShape::Scalar(primitive),
            ConstType::Array(element, length) => HeaderShape::Array(element, length.range()),
        },
    ))
}
/// No mutable fields or descriptor/name constructor supply source authority.
#[derive(Debug)]
pub(crate) struct SourceScalarExpected {
    primitive: PrimitiveType,
    binding: Digest,
}
impl SourceScalarExpected {
    /// Concrete wire data only. Currentness remains with the owning packet.
    pub(crate) fn wire_type(&self) -> ConstantWireType {
        ConstantWireType::Scalar(self.primitive)
    }
    pub(crate) fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    #[cfg(test)]
    pub(crate) fn source_identity(&self) -> Digest {
        self.binding
    }
}
/// Exact unresolved type-length site, not a length value or evaluation permission.
#[derive(Debug)]
pub(crate) struct CurrentArrayLengthDemand {
    selection: SourceSelection,
    definition: Digest,
    context: DeclarationContext,
    body: BodyHandle,
    range: SourceRange,
    parent_plan: Digest,
    binding: Digest,
}
impl CurrentArrayLengthDemand {
    /// Parent has separately replayed current inputs. This private demand must
    /// match the freshly derived declared array shape, not merely a body/range.
    pub(crate) fn validate_parent(
        &self,
        admission: &ConstantSourceAdmission,
    ) -> Result<(), Diagnostic> {
        let plan = admission.plan();
        let (role, shape) = declared_shape(plan)?;
        let HeaderShape::Array(_, range) = shape else {
            return Err(fault(
                "constant child parent is not an array result declaration",
            ));
        };
        if matches!(role, Role::ArrayLength)
            || self.range != range
            || self.binding != binding(admission, role, shape)?
            || self.parent_plan != plan.identity()
            || self.definition != plan.owner_definition()
            || self.context != *plan.owner_context()
            || Some(&self.body) != plan.owner_body()
            || Some(&self.selection) != plan.source().selection()
        {
            return Err(fault(
                "constant child demand differs from its exact parent source/result signature",
            ));
        }
        Ok(())
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.selection
    }
    pub(crate) fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn parent_plan_identity(&self) -> Digest {
        self.parent_plan
    }
    #[cfg(test)]
    pub(crate) fn identity(&self) -> Digest {
        self.binding
    }
    pub(crate) fn expected_wire(&self) -> ConstantWireType {
        ConstantWireType::Scalar(PrimitiveType::Usize)
    }
    /// Data request only: root must establish current ArrayLength admission/query
    /// at this exact source site and check its result before initializer work.
    #[cfg(test)]
    pub(crate) fn request(&self) -> ConstantRequest {
        ConstantRequest::ArrayLength(self.range)
    }
}
#[derive(Debug)]
enum ResultReadiness {
    Scalar(SourceScalarExpected),
    Array {
        element: SourceScalarExpected,
        length: Box<CurrentArrayLengthDemand>,
    },
}
/// Matching element type never completes an array from the caller's count.
#[derive(Debug)]
pub(crate) enum ExpectedResultReadiness<'a> {
    ReadyScalar(&'a SourceScalarExpected),
    RequiredArrayLength(&'a CurrentArrayLengthDemand),
}
/// Consumes and retains actual source/type admission, including const-call
/// concrete argument identity. No caller-built ConstType or length is trusted.
#[derive(Debug)]
pub(crate) struct CurrentConstantResultType {
    admission: ConstantSourceAdmission,
    readiness: ResultReadiness,
    role: Role,
    binding: Digest,
    bytes: usize,
}
fn binding(
    admission: &ConstantSourceAdmission,
    role: Role,
    shape: HeaderShape,
) -> Result<Digest, Diagnostic> {
    let plan = admission.plan();
    let source = plan.source().selection().ok_or_else(|| {
        fault("legacy content-only constant plan grants no current result authority")
    })?;
    let body = plan
        .owner_body()
        .ok_or_else(|| fault("constant result owning body is absent"))?;
    let mut e = StableEncoder::new("lrr/v1/current-constant-result-type");
    e.digest(admission.identity());
    e.digest(plan.identity());
    e.string(source.path.as_str());
    e.record(&source.outline);
    e.digest(plan.owner_definition());
    e.record(plan.owner_context());
    e.record(body);
    // BodyHandle's historical stable encoding omits range. This proof binds it.
    e.u32(body.range.start);
    e.u32(body.range.end);
    e.u8(role.tag());
    match shape {
        HeaderShape::Scalar(ty) => {
            e.u8(1);
            e.u32(ty.tag());
        }
        HeaderShape::Array(element, length) => {
            e.u8(2);
            e.u32(element.tag());
            e.u32(length.start);
            e.u32(length.end);
        }
    }
    Ok(e.finish())
}
impl CurrentConstantResultType {
    pub(crate) fn capture_current(
        db: &Database,
        source: &PreparedSourceView,
        admission: ConstantSourceAdmission,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if byte_limit == 0 || byte_limit > MAX_BYTES {
            return Err(limit());
        }
        let plan = admission.plan();
        let selection = plan
            .source()
            .selection()
            .ok_or_else(|| fault("current constant result source selection absent"))?;
        // Reserve retained metadata before current-source replay/header allocation.
        let bytes = std::mem::size_of::<Self>()
            .checked_add(256)
            .and_then(|n| n.checked_add(admission.metadata_bytes()))
            .and_then(|n| n.checked_add(selection.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(plan.owner_context().scope.heap_bytes()))
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<CurrentArrayLengthDemand>()
                        + std::mem::size_of::<SourceScalarExpected>(),
                )
            })
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        admission.revalidate(db, source)?;
        let (role, shape) = declared_shape(plan)?;
        let binding = binding(&admission, role, shape)?;
        let readiness = match shape {
            HeaderShape::Scalar(primitive) => {
                ResultReadiness::Scalar(SourceScalarExpected { primitive, binding })
            }
            HeaderShape::Array(primitive, range) => {
                let body = plan
                    .owner_body()
                    .ok_or_else(|| fault("constant result body absent"))?;
                let context = plan.owner_context();
                // A type length is inside the owning declaration header; it need
                // not be inside the initializer/body expression's narrower range.
                if range.start >= range.end
                    || range.start < context.range.start
                    || range.end > context.range.end
                {
                    return Err(fault(
                        "array result length is not within its exact owning declaration",
                    ));
                }
                ResultReadiness::Array {
                    element: SourceScalarExpected { primitive, binding },
                    length: Box::new(CurrentArrayLengthDemand {
                        selection: selection.clone(),
                        definition: plan.owner_definition(),
                        context: context.clone(),
                        body: body.clone(),
                        range,
                        parent_plan: plan.identity(),
                        binding,
                    }),
                }
            }
        };
        Ok(Self {
            admission,
            readiness,
            role,
            binding,
            bytes,
        })
    }
    pub(crate) fn into_admission(self) -> ConstantSourceAdmission {
        self.admission
    }
    pub(crate) fn admission(&self) -> &ConstantSourceAdmission {
        &self.admission
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        self.binding
    }
    pub(crate) fn scalar(&self) -> Option<&SourceScalarExpected> {
        match &self.readiness {
            ResultReadiness::Scalar(scalar) => Some(scalar),
            _ => None,
        }
    }
    pub(crate) fn array(&self) -> Option<(&SourceScalarExpected, &CurrentArrayLengthDemand)> {
        match &self.readiness {
            ResultReadiness::Array { element, length } => Some((element, length)),
            _ => None,
        }
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        source: &PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        self.admission.revalidate(db, source)?;
        let (role, shape) = declared_shape(self.admission.plan())?;
        let fresh = binding(&self.admission, role, shape)?;
        if role != self.role || fresh != self.binding {
            return Err(fault(
                "constant result signature/argument source identity changed",
            ));
        }
        let matches = match (&self.readiness, shape) {
            (ResultReadiness::Scalar(scalar), HeaderShape::Scalar(primitive)) => {
                scalar.primitive == primitive && scalar.binding == fresh
            }
            (ResultReadiness::Array { element, length }, HeaderShape::Array(primitive, range)) => {
                let plan = self.admission.plan();
                element.primitive == primitive
                    && element.binding == fresh
                    && length.binding == fresh
                    && length.range == range
                    && length.definition == plan.owner_definition()
                    && length.context == *plan.owner_context()
                    && Some(&length.body) == plan.owner_body()
                    && Some(&length.selection) == plan.source().selection()
                    && length.parent_plan == plan.identity()
            }
            _ => false,
        };
        if !matches {
            return Err(fault(
                "constant retained result/length proof differs from its admitted header",
            ));
        }
        Ok(())
    }
    /// Use before provider/memo work. Known type mismatches require no length,
    /// initializer, const callee execution, or caller descriptor recovery.
    pub(crate) fn check_current_expected<'a>(
        &'a self,
        db: &Database,
        source: &PreparedSourceView,
        expected: &ConstantWireType,
    ) -> Result<ExpectedResultReadiness<'a>, Diagnostic> {
        self.revalidate(db, source)?;
        match (&self.readiness, expected) {
            (ResultReadiness::Scalar(scalar), ConstantWireType::Scalar(primitive))
                if scalar.primitive == *primitive =>
            {
                Ok(ExpectedResultReadiness::ReadyScalar(scalar))
            }
            (
                ResultReadiness::Array { element, length },
                ConstantWireType::Array {
                    element: expected, ..
                },
            ) => {
                if !matches!(expected.as_ref(),ConstantWireType::Scalar(primitive) if element.primitive==*primitive)
                {
                    return Err(mismatch());
                }
                Ok(ExpectedResultReadiness::RequiredArrayLength(length))
            }
            _ => Err(mismatch()),
        }
    }
}
#[cfg(test)]
#[path = "constant_result_type_tests.rs"]
mod tests;
