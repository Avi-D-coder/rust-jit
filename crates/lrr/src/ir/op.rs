//! Owned initial LRIR inventory. No callbacks, guest handles or native addresses.
use super::pc::SlotId;
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{body_index::ScopeId, spans::BodySpan},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PrimitiveType {
    Unit,
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
}
impl PrimitiveType {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Unit => 0,
            Self::Bool => 1,
            Self::Char => 2,
            Self::I8 => 3,
            Self::I16 => 4,
            Self::I32 => 5,
            Self::I64 => 6,
            Self::I128 => 7,
            Self::Isize => 8,
            Self::U8 => 9,
            Self::U16 => 10,
            Self::U32 => 11,
            Self::U64 => 12,
            Self::U128 => 13,
            Self::Usize => 14,
            Self::F32 => 15,
            Self::F64 => 16,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HintRecipe {
    /// The reached borrow result supplies its existing pointee context. Only a
    /// SetHint at that borrow's Entry may use this source-indexed value slot.
    BorrowTarget {
        reference: SlotId,
    },
    ArrayPrimitive {
        element: PrimitiveType,
        len: u32,
    },
    Parameter(crate::traits::term::ParamKey),
    None,
    Primitive(PrimitiveType),
    Reference {
        region: crate::traits::term::RegionTerm,
        mutable: bool,
        target: PrimitiveType,
    },
}
/// Selected from definition-source types before either operand executes.
/// Generic/overloaded assignment requires an exact trait-member recipe instead.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AssignKind {
    Primitive {
        left: PrimitiveType,
        right: PrimitiveType,
    },
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LiteralKind {
    Integer,
    NegativeInteger,
    Float,
    Bool,
    Char,
    Byte,
    String,
    ByteString,
}
impl LiteralKind {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Integer => 0,
            Self::NegativeInteger => 7,
            Self::Float => 1,
            Self::Bool => 2,
            Self::Char => 3,
            Self::Byte => 4,
            Self::String => 5,
            Self::ByteString => 6,
        }
    }
}
/// Explicit recipe/cache compatibility, independent of host enum layout.
pub const LRIR_RECIPE_SCHEMA: u32 = 5;
pub const DECODED_INTEGER_SCHEMA: u32 = 1;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IntegerLexicalError {
    InvalidMagnitude,
    InvalidSuffix,
    UnsupportedWidth,
}
impl IntegerLexicalError {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::InvalidMagnitude => 1,
            Self::InvalidSuffix => 2,
            Self::UnsupportedWidth => 3,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DecodedLiteral {
    Integer {
        negative: bool,
        radix: u8,
        magnitude: u128,
        suffix: Option<PrimitiveType>,
    },
    InvalidInteger {
        negative: bool,
        radix: u8,
        suffix: Option<PrimitiveType>,
        reason: IntegerLexicalError,
    },
    Char(char),
    Byte(u8),
    Float {
        decimal: String,
        suffix: Option<PrimitiveType>,
    },
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LiteralRecipe {
    pub decoded: Option<DecodedLiteral>,
    pub kind: LiteralKind,
    pub spelling: String,
    pub source: BodySpan,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticTemplate {
    pub message: String,
}
impl DiagnosticTemplate {
    pub fn reached_unsupported(&self) -> Diagnostic {
        Diagnostic::new("LRR-E-UNSUPPORTED", self.message.clone())
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NumericBinary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NumericUnary {
    Neg,
    Not,
}
impl NumericBinary {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Add => 0,
            Self::Sub => 1,
            Self::Mul => 2,
            Self::Div => 3,
            Self::Rem => 4,
            Self::Shl => 5,
            Self::Shr => 6,
            Self::BitAnd => 7,
            Self::BitOr => 8,
            Self::BitXor => 9,
            Self::Eq => 10,
            Self::Ne => 11,
            Self::Lt => 12,
            Self::Le => 13,
            Self::Gt => 14,
            Self::Ge => 15,
        }
    }
}
impl NumericUnary {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Neg => 0,
            Self::Not => 1,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BorrowAccess {
    Shared,
    Mutable,
}
/// Stable opcode inventory. Runtime counter storage must use this bound.
pub const OPCODE_COUNT: usize = 42;
pub type OpcodeCounters = [u64; OPCODE_COUNT];

/// Closed source-bound floating value. Public callers can inspect or clone a
/// validated instruction, but cannot supply arbitrary scalar bits or identity.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrimitiveFloatingConstant {
    plan_identity: crate::core::digest::Digest,
    body_structure: crate::core::digest::Digest,
    key: super::pc::PcKey,
    next: super::pc::PcKey,
    source: crate::syntax::spans::BodySpan,
    destination: super::pc::SlotId,
    primitive: PrimitiveType,
    scalar: crate::runtime::value::ValueRepr,
    edition: u16,
    hygiene: crate::syntax::token_wire::Key,
    dependencies: Box<[super::recipe::FactDependency]>,
}
pub(crate) struct FloatingConstantBinding {
    pub plan_identity: crate::core::digest::Digest,
    pub body_structure: crate::core::digest::Digest,
    pub key: super::pc::PcKey,
    pub next: super::pc::PcKey,
    pub source: crate::syntax::spans::BodySpan,
    pub destination: super::pc::SlotId,
    pub primitive: PrimitiveType,
    pub scalar: crate::runtime::value::ValueRepr,
    pub edition: u16,
    pub hygiene: crate::syntax::token_wire::Key,
    pub dependencies: Vec<super::recipe::FactDependency>,
}
impl PrimitiveFloatingConstant {
    pub(crate) fn bind(
        mut binding: FloatingConstantBinding,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        let invalid = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "floating instruction source binding differs".into(),
            )
        };
        binding.scalar.validate_scalar()?;
        if !matches!(
            (
                binding.primitive,
                binding.scalar.kind,
                binding.scalar.type_id
            ),
            (PrimitiveType::F32, 6, 14) | (PrimitiveType::F64, 7, 15)
        ) || !matches!(binding.edition, 2015 | 2018 | 2021 | 2024)
            || binding.key.body != binding.source.body_key
            || binding.next.body != binding.key.body
            || binding.key.phase != super::pc::Phase::Entry.code()?
            || binding.dependencies.is_empty()
            || binding.dependencies.len() > 16
        {
            return Err(invalid());
        }
        binding.dependencies.sort_unstable();
        if binding
            .dependencies
            .windows(2)
            .any(|pair| pair[0].same_key(&pair[1]))
        {
            return Err(invalid());
        }
        Ok(Self {
            plan_identity: binding.plan_identity,
            body_structure: binding.body_structure,
            key: binding.key,
            next: binding.next,
            source: binding.source,
            destination: binding.destination,
            primitive: binding.primitive,
            scalar: binding.scalar,
            edition: binding.edition,
            hygiene: binding.hygiene,
            dependencies: binding.dependencies.into_boxed_slice(),
        })
    }
    pub fn scalar(&self) -> crate::runtime::value::ValueRepr {
        self.scalar
    }
    pub fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    pub fn source_span(&self) -> &crate::syntax::spans::BodySpan {
        &self.source
    }
    pub fn plan_identity(&self) -> crate::core::digest::Digest {
        self.plan_identity
    }
    pub fn edition(&self) -> u16 {
        self.edition
    }
    pub fn hygiene_context(&self) -> crate::syntax::token_wire::Key {
        self.hygiene
    }
    pub fn destination(&self) -> super::pc::SlotId {
        self.destination
    }
    pub fn successor(&self) -> &super::pc::PcKey {
        &self.next
    }
    pub fn dependencies(&self) -> &[super::recipe::FactDependency] {
        &self.dependencies
    }
    pub fn opcode_tag(&self) -> u8 {
        41
    }
    pub(crate) fn matches(
        &self,
        body: &crate::syntax::body_index::BodyIndex,
        draft: &super::recipe::RecipeDraft,
        destination: super::pc::SlotId,
    ) -> Result<bool, crate::core::diagnostic::Diagnostic> {
        use crate::core::digest::StableEncode;
        self.scalar.validate_scalar()?;
        Ok(self.key == draft.key
            && self.source == draft.origin.span
            && self.body_structure == body.fingerprint()
            && self.destination == destination
            && matches!(
                (self.primitive, self.scalar.kind, self.scalar.type_id),
                (PrimitiveType::F32, 6, 14) | (PrimitiveType::F64, 7, 15)
            )
            && matches!(draft.exits.as_slice(), [super::recipe::RecipeExit::Next(next)] if next == &self.next)
            && draft.dependencies.len() == self.dependencies.len()
            && self
                .dependencies
                .iter()
                .all(|dependency| draft.dependencies.contains(dependency)))
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        self.dependencies
            .iter()
            .try_fold(std::mem::size_of::<Self>(), |bytes, dependency| {
                bytes
                    .checked_add(std::mem::size_of::<super::recipe::FactDependency>())?
                    .checked_add(dependency.heap_bytes())
            })
    }
}
impl crate::core::digest::StableEncode for PrimitiveFloatingConstant {
    const DOMAIN: &'static str = "lrr/v5/primitive-floating-constant-instruction";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.digest(self.plan_identity);
        e.digest(self.body_structure);
        e.record(&self.key);
        e.record(&self.next);
        e.record(&self.source.body_key);
        e.u32(self.source.relative_start);
        e.u32(self.source.relative_end);
        e.u32(self.destination.0);
        e.u32(self.primitive.tag());
        e.u32(self.scalar.kind);
        e.u32(self.scalar.type_id);
        e.u64(self.scalar.payload);
        e.u16(self.edition);
        e.digest(crate::core::digest::Digest(self.hygiene.0));
        e.sequence(&self.dependencies, |e, dependency| e.record(dependency));
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Op {
    PrimitiveFloatingConstant {
        constant: Box<PrimitiveFloatingConstant>,
        destination: SlotId,
    },
    ProjectField {
        base: SlotId,
        destination: SlotId,
    },
    ProjectTuple {
        base: SlotId,
        field: u32,
        destination: SlotId,
    },
    ProjectIndex {
        base: SlotId,
        index: SlotId,
        destination: SlotId,
    },
    DerefPlace {
        reference: SlotId,
        destination: SlotId,
    },
    ReadPlace {
        place: SlotId,
        destination: SlotId,
    },
    AssignPlace {
        place: SlotId,
        source: SlotId,
        destination: SlotId,
    },
    MakePlace {
        binding: super::pc::NodeId,
        destination: SlotId,
    },
    /// Address the owned value of an already executed receiver. Keep its cell
    /// in the original temporary scope; do not copy or move the receiver.
    ValuePlace {
        source: SlotId,
        destination: SlotId,
    },
    /// Parentheses carry an existing checked address without reading its value.
    PlaceTransfer {
        source: SlotId,
        destination: SlotId,
    },
    Borrow {
        place: SlotId,
        access: BorrowAccess,
        destination: SlotId,
    },
    /// The dispatcher prepares a type/variant witness for this exact source PC.
    /// Runtime descriptor IDs never belong to a cached pure recipe.
    ConstructStart {
        guard: SlotId,
    },
    ConstructField {
        guard: SlotId,
        field: u32,
        source: SlotId,
    },
    ConstructFinish {
        guard: SlotId,
        destination: SlotId,
    },
    /// Conditional guest panic after the frontend has evaluated a boolean operand.
    Assert {
        condition: SlotId,
        destination: SlotId,
        message: String,
    },
    /// Explicit guest panic; emitted only after frontend intrinsic identity resolution.
    Panic {
        message: String,
    },
    /// An entered ordinary item declaration has no guest initializer/body action.
    Declaration,
    CallPrepare {
        site: super::recipe::CallSiteId,
        name: String,
        explicit: Vec<String>,
        arguments: Vec<SlotId>,
    },
    CallAdvance {
        site: super::recipe::CallSiteId,
        argument: u32,
    },
    CheckParameters {
        count: u32,
    },
    ReadParameter {
        ordinal: u32,
        destination: SlotId,
    },
    CallSeal {
        site: super::recipe::CallSiteId,
        name: String,
        explicit: Vec<String>,
        arguments: Vec<SlotId>,
    },
    CheckUnit {
        source: SlotId,
    },
    BranchHint {
        parent: SlotId,
        condition: SlotId,
        yes: Option<SlotId>,
        no: Option<SlotId>,
        boolean: bool,
    },
    DeclareLocal {
        binding: super::pc::NodeId,
        hint: HintRecipe,
        writable: bool,
    },
    CompoundHint {
        binding: super::pc::NodeId,
        child: SlotId,
        kind: AssignKind,
    },
    LocalHint {
        binding: super::pc::NodeId,
        child: SlotId,
    },
    AssignLocal {
        binding: super::pc::NodeId,
        source: SlotId,
        destination: SlotId,
    },
    NumericHint {
        parent: SlotId,
        child: SlotId,
        left: Option<SlotId>,
        operator: Option<NumericBinary>,
    },
    Literal {
        literal: LiteralRecipe,
        hint: HintRecipe,
        destination: SlotId,
    },
    Unit {
        destination: SlotId,
    },
    SetHint {
        child: SlotId,
        hint: HintRecipe,
    },
    ResultTransfer {
        source: SlotId,
        destination: SlotId,
    },
    ScopeEnter {
        scope: ScopeId,
    },
    ScopeLeave {
        scope: ScopeId,
    },
    BindLocal {
        binding: super::pc::NodeId,
        source: SlotId,
        hint: HintRecipe,
        writable: bool,
    },
    ReadLocal {
        binding: super::pc::NodeId,
        destination: SlotId,
    },
    Discard {
        source: SlotId,
    },
    NumericBinary {
        operator: NumericBinary,
        left: SlotId,
        right: SlotId,
        destination: SlotId,
    },
    NumericUnary {
        operator: NumericUnary,
        source: SlotId,
        destination: SlotId,
    },
    PrimitiveCast {
        source: SlotId,
        target: PrimitiveType,
        destination: SlotId,
    },
    Unsupported(DiagnosticTemplate),
}
impl Op {
    /// Stable opcode IDs shared by execution counters and redacted failure records.
    pub(crate) fn opcode_tag(&self) -> u8 {
        match self {
            Self::CompoundHint { .. } => 40,
            Self::ProjectTuple { .. } => 35,
            Self::ProjectIndex { .. } => 36,
            Self::ProjectField { .. } => 37,
            Self::DerefPlace { .. } => 32,
            Self::ReadPlace { .. } => 33,
            Self::AssignPlace { .. } => 34,
            Self::MakePlace { .. } => 30,
            Self::ValuePlace { .. } => 38,
            Self::PlaceTransfer { .. } => 39,
            Self::Borrow { .. } => 31,
            Self::PrimitiveFloatingConstant { .. } => 41,
            Self::Literal { .. } => 0,
            Self::Unit { .. } => 1,
            Self::SetHint { .. } => 2,
            Self::ResultTransfer { .. } => 3,
            Self::ScopeEnter { .. } => 4,
            Self::ScopeLeave { .. } => 5,
            Self::Unsupported(_) => 6,
            Self::BindLocal { .. } => 7,
            Self::ReadLocal { .. } => 8,
            Self::Discard { .. } => 9,
            Self::NumericBinary { .. } => 10,
            Self::NumericUnary { .. } => 11,
            Self::PrimitiveCast { .. } => 12,
            Self::NumericHint { .. } => 13,
            Self::DeclareLocal { .. } => 14,
            Self::LocalHint { .. } => 15,
            Self::AssignLocal { .. } => 16,
            Self::BranchHint { .. } => 17,
            Self::CheckUnit { .. } => 18,
            Self::CallSeal { .. } => 19,
            Self::CheckParameters { .. } => 20,
            Self::ReadParameter { .. } => 21,
            Self::CallPrepare { .. } => 22,
            Self::CallAdvance { .. } => 23,
            Self::Declaration => 24,
            Self::Panic { .. } => 25,
            Self::Assert { .. } => 26,
            Self::ConstructStart { .. } => 27,
            Self::ConstructField { .. } => 28,
            Self::ConstructFinish { .. } => 29,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EffectClass {
    Primitive,
    OwnershipTransfer,
    ScopeActivation,
    CleanupBoundary,
    ReachedFault,
    GuestPanic,
}
impl Op {
    pub fn effect_class(&self) -> EffectClass {
        match self {
            Self::MakePlace { .. }
            | Self::ValuePlace { .. }
            | Self::PlaceTransfer { .. }
            | Self::ProjectTuple { .. }
            | Self::ProjectIndex { .. }
            | Self::ProjectField { .. }
            | Self::DerefPlace { .. }
            | Self::Declaration
            | Self::PrimitiveFloatingConstant { .. }
            | Self::Literal { .. }
            | Self::Unit { .. }
            | Self::SetHint { .. }
            | Self::NumericBinary { .. }
            | Self::NumericUnary { .. }
            | Self::PrimitiveCast { .. }
            | Self::NumericHint { .. }
            | Self::LocalHint { .. }
            | Self::CompoundHint { .. }
            | Self::BranchHint { .. }
            | Self::CheckParameters { .. }
            | Self::CheckUnit { .. } => EffectClass::Primitive,
            Self::Borrow { .. }
            | Self::ReadPlace { .. }
            | Self::AssignPlace { .. }
            | Self::CallPrepare { .. }
            | Self::ConstructStart { .. }
            | Self::ConstructField { .. }
            | Self::ConstructFinish { .. }
            | Self::CallAdvance { .. }
            | Self::ReadParameter { .. }
            | Self::CallSeal { .. }
            | Self::ResultTransfer { .. }
            | Self::BindLocal { .. }
            | Self::ReadLocal { .. }
            | Self::DeclareLocal { .. }
            | Self::AssignLocal { .. } => EffectClass::OwnershipTransfer,
            Self::Discard { .. } => EffectClass::CleanupBoundary,
            Self::ScopeEnter { .. } => EffectClass::ScopeActivation,
            Self::ScopeLeave { .. } => EffectClass::CleanupBoundary,
            Self::Unsupported(_) => EffectClass::ReachedFault,
            Self::Panic { .. } | Self::Assert { .. } => EffectClass::GuestPanic,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_and_transfer_are_not_classified_as_pure_primitives() {
        assert_eq!(
            Op::ScopeLeave { scope: ScopeId(0) }.effect_class(),
            EffectClass::CleanupBoundary
        );
        assert_eq!(
            Op::ResultTransfer {
                source: SlotId(0),
                destination: SlotId(2)
            }
            .effect_class(),
            EffectClass::OwnershipTransfer
        );
        assert_eq!(
            Op::Unit {
                destination: SlotId(0)
            }
            .effect_class(),
            EffectClass::Primitive
        );
    }
}
