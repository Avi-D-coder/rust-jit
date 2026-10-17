//! Validate/reserve/commit guest storage. No guest callbacks during mutation.
#[cfg(test)]
#[path = "cell_test_owners.rs"]
mod test_owners;
#[cfg(test)]
pub(crate) use test_owners::OwnerInventory;
#[path = "cell_aggregate_copy.rs"]
pub(crate) mod aggregate_copy;
#[path = "cell_boxes.rs"]
mod boxes;
#[path = "cell_constant_instantiation.rs"]
pub mod constant_instantiation;
#[path = "cell_construction_cleanup.rs"]
mod construction_cleanup;
#[path = "cell_closures.rs"]
mod closures;
#[path = "cell_reference_return.rs"]
mod reference_return;
#[path = "cell_run_session.rs"]
mod run_session;
#[path = "cell_unsizing.rs"]
mod unsizing;
#[path = "str_storage.rs"]
pub mod str_storage;
use super::{
    arena::{GuestArena, Handle, RunBudget, resource, stale},
    object::{
        AggregateStorage, ConstructionRecord, ConstructionState, InitializedFields, ObjectStorage,
        StructuralConstructionKind,
    },
    place::{
        CellStamp, EnclosingObject, Place, PlaceFailure, PlacePanic, PlaceRoot, Projection,
        ResolvedPlace, SourceFieldProjection, SourceProjectionStep, checked_chain_bytes,
        checked_source_projection_bytes,
    },
    reference::{Access, ReferenceLease, ReferenceRecord, access_fault, lease_fault},
    shape::{ShapeCategory, TypeDescriptor, TypeId, TypeKey, TypeRegistry},
    value::{ValueRepr, invalid},
};
use crate::core::diagnostic::Diagnostic;
pub use construction_cleanup::ConstructionCleanupTransfer;
pub(crate) use reference_return::ReferenceTarget;
pub(crate) use run_session::StoreSession;
/// `[T]` is unsized: its place is borrowed or indexed, never read or assigned.
fn unsized_place() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-OP-TYPE",
        "an unsized slice place can only be borrowed or indexed".into(),
    )
}
fn ownership() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-OWNERSHIP",
        "guest ownership, cell state, or storage contract mismatch".into(),
    )
}
/// Closed intrinsic decision for an ordinary owning read. This is not a
/// permission to bit-copy aggregates or select nominal Copy from runtime shape.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IntrinsicRead {
    CopyScalar,
    CopyAggregate,
    Move,
}

/// Address formation does not read the terminal value. Every subsequent read,
/// borrow and assignment applies its own checked state/ownership contract.
#[derive(Clone, Copy, Eq, PartialEq)]
enum PlaceUse {
    Value,
    OwningRead,
    Address,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum IntrinsicAssignment {
    Initialize { ty: TypeId, target: CellId },
    Replace { ty: TypeId, target: CellId },
    /// The old value is an owning aggregate or nominal value: its drop glue
    /// (and any user `Drop` witness) runs through the frame's cleanup
    /// activation, never as a synchronous discard.
    ReplaceOwned { ty: TypeId, target: CellId },
}
impl IntrinsicAssignment {
    pub(crate) fn ty(self) -> TypeId {
        match self {
            Self::Initialize { ty, .. } | Self::Replace { ty, .. } | Self::ReplaceOwned { ty, .. } => {
                ty
            }
        }
    }
    pub(crate) fn target(self) -> CellId {
        match self {
            Self::Initialize { target, .. }
            | Self::Replace { target, .. }
            | Self::ReplaceOwned { target, .. } => target,
        }
    }
    pub(crate) fn replaces_live(self) -> bool {
        matches!(self, Self::Replace { .. } | Self::ReplaceOwned { .. })
    }
}
/// One bounded declaration-order cleanup transition. Skipping a moved field
/// remains explicit progress, so the VM can charge/poll before the next field.
pub enum AggregateCleanupStep {
    Field(TicketId),
    Skipped,
    Complete,
}
/// Owned ordinary-cleanup continuation. The field location remains Dropping
/// until its destructor and nested obligations finish; a ticket alone must not
/// retire the activation's self location before a guest call can suspend.
pub enum AggregateCleanupTransfer {
    Field { cell: CellId, ticket: TicketId },
    Skipped,
    Complete,
}
pub(crate) enum CleanupInventory {
    Leaf,
    Fields,
    NeedWitness(TypeId),
}
pub(crate) enum CleanupFieldAdmission {
    Live,
    Skip,
    Complete,
}
pub struct FrameAnchor;
macro_rules! id {
    ($name:ident,$record:ty) => {
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
        pub struct $name(Handle<$record>);
        impl $name {
            pub fn bits(self) -> u64 {
                self.0.bits()
            }
        }
        impl From<Handle<$record>> for $name {
            fn from(handle: Handle<$record>) -> Self {
                Self(handle)
            }
        }
        impl From<$name> for Handle<$record> {
            fn from(handle: $name) -> Self {
                handle.0
            }
        }
    };
}
id!(FrameId, FrameAnchor);
id!(CellId, Cell);
id!(ObjectId, ObjectRecord);
id!(TicketId, TransferTicket);
id!(LiteralId, LiteralRecord);
id!(ArgumentBatchId, ArgumentBatch);
id!(ConstructionId, ConstructionRecord);
id!(RefId, ReferenceRecord);
impl ObjectId {
    pub(crate) fn from_bits(bits: u64) -> Self {
        Self(Handle::from_bits(bits))
    }
}
impl RefId {
    pub(crate) fn from_bits(bits: u64) -> Self {
        Self(Handle::from_bits(bits))
    }
}
impl LiteralId {
    pub(crate) fn from_bits(bits: u64) -> Self {
        Self(Handle::from_bits(bits))
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CellOwner {
    FrameSlot { frame: FrameId, slot: u32 },
    Field { object: ObjectId, index: u32 },
    Buffer { object: ObjectId, index: u32 },
    Transfer(TicketId),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StorageContract {
    Unbound,
    NumericLiteral,
    Exact(TypeId),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CellState {
    Uninit,
    Live(ValueRepr),
    Moved,
    Dropping,
    Dropped,
}
pub struct Cell {
    state: CellState,
    declared: StorageContract,
    writable: bool,
    location_epoch: u32,
    owner: CellOwner,
    retired: bool,
    numeric_record: Option<LiteralId>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ValueOwner {
    Ticket(TicketId),
    Cell(CellId),
    Construction { guard: ConstructionId, field: u32 },
}
pub struct ObjectRecord {
    type_id: TypeId,
    storage: ObjectStorage,
    owner: Option<ValueOwner>,
}
pub struct ArgumentBatch {
    tickets: Vec<TicketId>,
    types: Vec<TypeId>,
}
pub struct TransferTicket {
    value: ValueRepr,
}
pub enum LiteralNumber {
    Integer { negative: bool, magnitude: u128 },
    Float,
}
pub struct LiteralRecord {
    number: LiteralNumber,
    spelling: String,
    binding: Option<TypeId>,
}
pub struct GuestStore {
    pattern_binding_session: StoreSession,
    budget: RunBudget,
    types: TypeRegistry,
    cells: GuestArena<Cell>,
    objects: GuestArena<ObjectRecord>,
    tickets: GuestArena<TransferTicket>,
    argument_batches: GuestArena<ArgumentBatch>,
    literals: GuestArena<LiteralRecord>,
    frames: GuestArena<FrameAnchor>,
    constructions: GuestArena<ConstructionRecord>,
    references: GuestArena<ReferenceRecord>,
    strings: str_storage::ImmutableStrStorage,
    /// Nominal declarations with a user `Drop` impl, once the run's complete
    /// impl inventory is known. Absent means every nominal cleanup still
    /// demands its witness.
    user_drop_types: Option<std::collections::BTreeSet<crate::core::digest::Digest>>,
    /// Nominal declarations with a `Copy` impl, once the run's complete impl
    /// inventory is known. A nominal value outside this set is moved by an
    /// owning read; absent means every nominal owning read is still refused.
    user_copy_types: Option<std::collections::BTreeSet<crate::core::digest::Digest>>,
    /// The `owned_box` lang item (the alloc facade's `Box` declaration),
    /// whose places dereference through the builtin box projection.
    owned_box: Option<crate::core::digest::Digest>,
}
impl GuestStore {
    // E-R5-006: ignored, remove at callers (`_max_handles`, `_max_bytes`).
    pub fn new(epoch: u32, _max_handles: u32, _max_bytes: usize) -> Result<Self, Diagnostic> {
        Self::for_epoch(epoch)
    }
    /// A run store with no guest handle or byte limit (E-R5-006).
    pub fn for_epoch(epoch: u32) -> Result<Self, Diagnostic> {
        let session = StoreSession::mint()?;
        Ok(Self {
            pattern_binding_session: session,
            budget: RunBudget::for_epoch(epoch)?,
            types: TypeRegistry::default(),
            cells: GuestArena::new(epoch)?,
            objects: GuestArena::new(epoch)?,
            tickets: GuestArena::new(epoch)?,
            argument_batches: GuestArena::new(epoch)?,
            literals: GuestArena::new(epoch)?,
            frames: GuestArena::new(epoch)?,
            constructions: GuestArena::new(epoch)?,
            references: GuestArena::new(epoch)?,
            strings: str_storage::ImmutableStrStorage::new(epoch, session)?,
            user_drop_types: None,
            user_copy_types: None,
            owned_box: None,
        })
    }
    pub(crate) fn drop_inventory_installed(&self) -> bool {
        self.user_drop_types.is_some()
    }
    /// Install the run's complete set of nominal types that implement `Drop`.
    pub(crate) fn install_drop_inventory(
        &mut self,
        user_drop_types: std::collections::BTreeSet<crate::core::digest::Digest>,
    ) {
        self.user_drop_types = Some(user_drop_types);
    }
    /// Whether the run's complete impl inventory gives this declaration Copy.
    pub(crate) fn nominal_copy(&self, identity: crate::core::digest::Digest) -> bool {
        self.user_copy_types
            .as_ref()
            .is_some_and(|types| types.contains(&identity))
    }
    /// Install the run's complete set of nominal types that implement `Copy`.
    pub(crate) fn install_copy_inventory(
        &mut self,
        user_copy_types: std::collections::BTreeSet<crate::core::digest::Digest>,
    ) {
        self.user_copy_types = Some(user_copy_types);
    }
    pub(crate) fn pattern_binding_session(&self) -> StoreSession {
        self.pattern_binding_session
    }
    pub fn budget(&self) -> &RunBudget {
        &self.budget
    }
    pub fn intern_type(&mut self, key: TypeKey) -> Result<TypeId, Diagnostic> {
        let ty = self.types.intern(key)?;
        self.intern_unsized_box_view(ty)?;
        Ok(ty)
    }
    pub(crate) fn find_type(&self, key: &TypeKey) -> Option<TypeId> {
        self.types.find(key)
    }
    /// The closed source type a runtime descriptor records exactly:
    /// primitives, argument-free nominal declarations and tuples of those.
    /// References keep their evaluated source view (regions are not data).
    pub(crate) fn descriptor_source_term(
        &self,
        ty: TypeId,
    ) -> Option<crate::traits::term::TyTerm> {
        use crate::{ir::op::PrimitiveType as P, traits::term::TyTerm};
        let primitive = match ty.0 {
            1 => Some(P::Unit),
            2 => Some(P::Bool),
            3 => Some(P::Char),
            4 => Some(P::I8),
            5 => Some(P::I16),
            6 => Some(P::I32),
            7 => Some(P::I64),
            8 => Some(P::Isize),
            9 => Some(P::U8),
            10 => Some(P::U16),
            11 => Some(P::U32),
            12 => Some(P::U64),
            13 => Some(P::Usize),
            14 => Some(P::F32),
            15 => Some(P::F64),
            16 => return Some(TyTerm::Str),
            _ => None,
        };
        if let Some(primitive) = primitive {
            return Some(TyTerm::Primitive(primitive));
        }
        match self.type_descriptor(ty).ok()?.key.as_ref() {
            // Regions are not runtime data: a reached reference is `'static`
            // up to the erasure used by structural contracts.
            TypeKey::Ref { mutable, target } => Some(TyTerm::Ref(
                crate::traits::term::RegionTerm::Static,
                *mutable,
                Box::new(self.descriptor_source_term(*target)?),
            )),
            TypeKey::Nominal {
                identity,
                args,
                const_args,
                ..
            } if const_args.is_empty() => Some(TyTerm::Nominal(
                *identity,
                args.iter()
                    .map(|arg| {
                        self.descriptor_source_term(*arg)
                            .map(crate::traits::term::GenericArg::Type)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            TypeKey::Tuple(fields) => fields
                .iter()
                .map(|field| self.descriptor_source_term(*field))
                .collect::<Option<Vec<_>>>()
                .map(TyTerm::Tuple),
            // A function item or closure is its own nominal-like Type,
            // named by its callee identity (no source schema).
            TypeKey::Callable { identity, .. } => Some(TyTerm::Nominal(*identity, Vec::new())),
            TypeKey::Array { element, len } => Some(TyTerm::Array(
                Box::new(self.descriptor_source_term(*element)?),
                crate::traits::term::ConstTerm::Value(u64::from(*len)),
            )),
            TypeKey::Slice { element } => Some(TyTerm::Slice(Box::new(
                self.descriptor_source_term(*element)?,
            ))),
            _ => None,
        }
    }
    /// Intern the runtime descriptor of a closed structural source type built
    /// from primitives (tuples/arrays of them). Nominal types need their
    /// schema and are not formed here.
    pub(crate) fn intern_structural_term(
        &mut self,
        term: &crate::traits::term::TyTerm,
    ) -> Result<Option<TypeId>, Diagnostic> {
        use crate::traits::term::{ConstTerm, TyTerm};
        Ok(match term {
            TyTerm::Primitive(primitive) => {
                Some(super::shape::PrimitiveIdentity::from_recipe(*primitive)?.id())
            }
            TyTerm::Tuple(fields) => {
                let mut ids = Vec::with_capacity(fields.len());
                for field in fields {
                    let Some(id) = self.intern_structural_term(field)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                Some(self.intern_type(TypeKey::Tuple(ids.into_boxed_slice()))?)
            }
            TyTerm::Array(element, ConstTerm::Value(len)) => {
                let Some(element) = self.intern_structural_term(element)? else {
                    return Ok(None);
                };
                let Ok(len) = u32::try_from(*len) else {
                    return Ok(None);
                };
                Some(self.intern_type(TypeKey::Array { element, len })?)
            }
            // `&[T]` / `&mut [T]` of a structural element (regions are not data).
            TyTerm::Slice(element) => {
                let Some(element) = self.intern_structural_term(element)? else {
                    return Ok(None);
                };
                Some(self.intern_type(TypeKey::Slice { element })?)
            }
            TyTerm::Str => Some(TypeId(16)),
            // A reference to a structural target (regions are not data).
            TyTerm::Ref(_, mutable, target) => {
                let Some(target) = self.intern_structural_term(target)? else {
                    return Ok(None);
                };
                Some(self.intern_type(TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })?)
            }
            _ => None,
        })
    }
    /// `intern_structural_term` without interning (preflight).
    pub(crate) fn find_structural_term(
        &self,
        term: &crate::traits::term::TyTerm,
    ) -> Option<TypeId> {
        use crate::traits::term::{ConstTerm, TyTerm};
        match term {
            TyTerm::Primitive(primitive) => super::shape::PrimitiveIdentity::from_recipe(*primitive)
                .ok()
                .map(|identity| identity.id()),
            TyTerm::Tuple(fields) => {
                let ids = fields
                    .iter()
                    .map(|field| self.find_structural_term(field))
                    .collect::<Option<Vec<_>>>()?;
                self.find_type(&TypeKey::Tuple(ids.into_boxed_slice()))
            }
            TyTerm::Array(element, ConstTerm::Value(len)) => {
                let element = self.find_structural_term(element)?;
                self.find_type(&TypeKey::Array {
                    element,
                    len: u32::try_from(*len).ok()?,
                })
            }
            TyTerm::Slice(element) => {
                let element = self.find_structural_term(element)?;
                self.find_type(&TypeKey::Slice { element })
            }
            TyTerm::Str => Some(TypeId(16)),
            TyTerm::Ref(_, mutable, target) => {
                let target = self.find_structural_term(target)?;
                self.find_type(&TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })
            }
            _ => None,
        }
    }
    pub fn type_descriptor(&self, ty: TypeId) -> Result<&TypeDescriptor, Diagnostic> {
        self.types.descriptor(ty)
    }
    pub fn type_metadata_bytes(&self) -> usize {
        self.types.metadata_bytes()
    }
    /// Admit a nominal guard only from a source-checked reached-variant witness.
    /// Its immutable plan cannot be replaced by arbitrary runtime field IDs.
    pub fn begin_nominal_construction(
        &mut self,
        bound: &crate::frontend::types::BoundNominalVariant,
    ) -> Result<(TypeId, ConstructionId), Diagnostic> {
        self.begin_nominal_construction_with(bound, &[])
    }
    /// A generic declaration's variant instantiated by its type arguments,
    /// which become the descriptor's `TypeKey::Nominal` args.
    pub(crate) fn begin_nominal_construction_with(
        &mut self,
        bound: &crate::frontend::types::BoundNominalVariant,
        args: &[TypeId],
    ) -> Result<(TypeId, ConstructionId), Diagnostic> {
        let ty = self.intern_nominal_with(bound, args)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(bound.fields().len())
            .map_err(|_| resource())?;
        for field in bound.fields() {
            fields.push(field.materialize(self, args)?);
        }
        let guard = self.begin_construction(&fields)?;
        self.constructions.get_mut(guard)?.nominal = Some((ty, bound.variant()));
        Ok((ty, guard))
    }
    /// The run-local descriptor of a source-checked variant's nominal type,
    /// with an enum's bound discriminant table attached.
    pub(crate) fn intern_nominal(
        &mut self,
        bound: &crate::frontend::types::BoundNominalVariant,
    ) -> Result<TypeId, Diagnostic> {
        self.intern_nominal_with(bound, &[])
    }
    pub(crate) fn intern_nominal_with(
        &mut self,
        bound: &crate::frontend::types::BoundNominalVariant,
        args: &[TypeId],
    ) -> Result<TypeId, Diagnostic> {
        if args.len() != bound.type_parameters() as usize {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "nominal type arguments differ from its declaration's type parameters".into(),
            ));
        }
        for arg in args {
            self.types.descriptor(*arg)?;
        }
        let ty = self.types.intern(TypeKey::Nominal {
            identity: bound.identity(),
            definition: bound.definition().clone(),
            schema: bound.schema(),
            args: args.into(),
            const_args: Box::new([]),
        })?;
        if bound.definition().item_kind == crate::source::manifest::ItemKind::Enum {
            self.types.attach_discriminants(ty, bound.discriminants())?;
        }
        self.intern_unsized_box_view(ty)?;
        Ok(ty)
    }
    /// `as` operand of a fieldless enum value: its active variant's
    /// discriminant as the default `isize` representation. Other values are
    /// not enum cast operands (`None`); the operand stays in its cell.
    pub(crate) fn enum_cast_operand(&self, id: CellId) -> Result<Option<ValueRepr>, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        let CellState::Live(value) = cell.state else {
            return Ok(None);
        };
        if value.kind != 8 {
            return Ok(None);
        }
        let descriptor = self.types.descriptor(TypeId(value.type_id))?;
        let TypeKey::Nominal { definition, .. } = descriptor.key.as_ref() else {
            return Ok(None);
        };
        if definition.item_kind != crate::source::manifest::ItemKind::Enum {
            return Ok(None);
        }
        self.object_owner(value, ValueOwner::Cell(id))?;
        let ObjectStorage::Aggregate(storage) = &self
            .objects
            .get(ObjectId::from_bits(value.payload))?
            .storage
        else {
            return Err(invalid());
        };
        if !storage.fields.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "only a fieldless enum value can be cast to an integer".into(),
            ));
        }
        let variant = storage.variant.ok_or_else(invalid)?;
        let discriminant = descriptor
            .discriminants
            .as_ref()
            .and_then(|table| table.get(variant as usize).copied())
            .ok_or_else(invalid)?
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "enum discriminant expression requires constant evaluation".into(),
                )
            })?;
        let discriminant = i64::try_from(discriminant).map_err(|_| {
            Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "enum discriminant outside the default isize representation".into(),
            )
        })?;
        ValueRepr::signed(TypeId(8), discriminant).map(Some)
    }
    /// A generic declaration's variant with no type context: concrete fields
    /// keep their bound Types and each parameter-typed field's completed value
    /// determines that parameter (`intern_pending_nominal`).
    pub(crate) fn begin_pending_nominal_construction(
        &mut self,
        bound: &crate::frontend::types::BoundNominalVariant,
    ) -> Result<ConstructionId, Diagnostic> {
        let mut expected = Vec::new();
        expected
            .try_reserve_exact(bound.fields().len())
            .map_err(|_| resource())?;
        for field in bound.fields() {
            expected.push(if field.generic() {
                None
            } else {
                let ty = field.materialize(self, &[])?;
                if self.types.descriptor(ty)?.shape == ShapeCategory::Unsized {
                    return Err(invalid());
                }
                Some(ty)
            });
        }
        self.constructions.reserve(1, &mut self.budget)?;
        let mut record = ConstructionRecord::partial(&expected)?;
        record.pending_nominal = Some(Box::new(bound.clone()));
        let bytes = record.owned_bytes().ok_or_else(resource)?;
        self.constructions
            .allocate(record, bytes, &mut self.budget)
            .map(ConstructionId::from)
    }
    /// The completed pending construction's instantiated nominal descriptor;
    /// it becomes the guard's commit witness.
    pub(crate) fn intern_pending_nominal(
        &mut self,
        guard: ConstructionId,
    ) -> Result<TypeId, Diagnostic> {
        let record = self.constructions.get(guard)?;
        let bound = record.pending_nominal.as_deref().ok_or_else(ownership)?;
        if record.state != ConstructionState::Building
            || record.completed.len() != record.fields.len()
        {
            return Err(ownership());
        }
        let fixed = self.pending_nominal_args(bound, &record.expected)?;
        let args = fixed.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "generic type argument is not determined by any constructed field".into(),
            )
        })?;
        let bound = bound.clone();
        let ty = self.intern_nominal_with(&bound, &args)?;
        let record = self.constructions.get_mut(guard)?;
        record.nominal = Some((ty, bound.variant()));
        record.pending_nominal = None;
        Ok(ty)
    }
    /// The type arguments a construction has fixed so far: all of a bound
    /// generic construction's, the completed fields' for a pending one, none
    /// otherwise.
    pub(crate) fn pending_construction_args(
        &self,
        guard: ConstructionId,
    ) -> Result<Vec<Option<TypeId>>, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if let Some(bound) = record.pending_nominal.as_deref() {
            return self.pending_nominal_args(bound, &record.expected);
        }
        if let Some((ty, _)) = record.nominal
            && let TypeKey::Nominal { args, .. } = self.types.descriptor(ty)?.key.as_ref()
        {
            return Ok(args.iter().copied().map(Some).collect());
        }
        Ok(Vec::new())
    }
    /// The type arguments a pending generic construction's completed fields
    /// determine so far, by matching each completed generic field's Type.
    fn pending_nominal_args(
        &self,
        bound: &crate::frontend::types::BoundNominalVariant,
        expected: &[Option<TypeId>],
    ) -> Result<Vec<Option<TypeId>>, Diagnostic> {
        let mut fixed = vec![None; bound.type_parameters() as usize];
        for (field, ty) in bound.fields().iter().zip(expected) {
            if let Some(ty) = ty
                && field.generic()
                && !field.bind(*ty, self, &mut fixed)?
            {
                return Err(ownership());
            }
        }
        Ok(fixed)
    }
    /// Field types have already been bound by the reached constructor resolver.
    /// This reserves ownership slots without executing or forcing any field.
    pub fn begin_construction(&mut self, fields: &[TypeId]) -> Result<ConstructionId, Diagnostic> {
        for ty in fields {
            if self.types.descriptor(*ty)?.shape == ShapeCategory::Unsized {
                return Err(invalid());
            }
        }
        self.constructions.reserve(1, &mut self.budget)?;
        let record = ConstructionRecord::new(fields)?;
        let bytes = record.owned_bytes().ok_or_else(resource)?;
        self.constructions
            .allocate(record, bytes, &mut self.budget)
            .map(ConstructionId::from)
    }
    /// An entered tuple/array with no source result descriptor holds only its
    /// source cardinality. Unentered fields have no runtime type to force.
    pub(crate) fn begin_observed_construction(
        &mut self,
        kind: StructuralConstructionKind,
        count: usize,
    ) -> Result<ConstructionId, Diagnostic> {
        self.constructions.reserve(1, &mut self.budget)?;
        let record = ConstructionRecord::observed(kind, count)?;
        let bytes = record.owned_bytes().ok_or_else(resource)?;
        self.constructions
            .allocate(record, bytes, &mut self.budget)
            .map(ConstructionId::from)
    }
    fn construction_field_type(
        &self,
        guard: ConstructionId,
        index: usize,
        value: ValueRepr,
    ) -> Result<TypeId, Diagnostic> {
        let record = self.constructions.get(guard)?;
        let expected = record.expected.get(index).ok_or_else(ownership)?;
        if let Some(ty) = expected {
            return Ok(*ty);
        }
        if record.observed_kind == Some(StructuralConstructionKind::Array)
            && let Some(ty) = record.observed_element
        {
            return Ok(ty);
        }
        if let Some(bound) = record.pending_nominal.as_deref() {
            use crate::frontend::types::NominalFieldType;
            let field = bound.fields().get(index).ok_or_else(ownership)?;
            if !field.generic() {
                return Err(ownership());
            }
            // Parameters the already completed fields determined.
            let mut fixed = self.pending_nominal_args(bound, &record.expected)?;
            if let NominalFieldType::Parameter(parameter) = field
                && let Some(Some(ty)) = fixed.get(*parameter as usize)
            {
                return Ok(*ty);
            }
            // Otherwise the completed value's own Type determines them; an
            // unconstrained literal at a bare parameter takes rustc's
            // integer/float fallback.
            let ty = match self.effective_type(value)? {
                Some(ty) if self.types.descriptor(ty)?.shape != ShapeCategory::Unsized => ty,
                Some(_) => return Err(invalid()),
                None if value.kind == 11 && matches!(field, NominalFieldType::Parameter(_)) => {
                    TypeId(6)
                }
                None if value.kind == 12 && matches!(field, NominalFieldType::Parameter(_)) => {
                    TypeId(15)
                }
                None => {
                    return Err(Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "generic constructor field requires its reached concrete type".into(),
                    ));
                }
            };
            return if field.bind(ty, self, &mut fixed)? {
                Ok(ty)
            } else {
                Err(ownership())
            };
        }
        if record.observed_kind.is_none() {
            return Err(ownership());
        }
        // A tuple field is a bare parameter of the tuple type, as a generic
        // constructor's field above: a literal nothing constrained (`(1, 'a')`
        // compared with another temporary) takes rustc's integer/float
        // fallback. An array element keeps its unresolved literal.
        let tuple = record.observed_kind == Some(StructuralConstructionKind::Tuple);
        let ty = match self.effective_type(value)? {
            Some(ty) => ty,
            None if tuple && value.kind == 11 => TypeId(6),
            None if tuple && value.kind == 12 => TypeId(15),
            None => {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "completed structural field requires its reached concrete type".into(),
                ));
            }
        };
        if self.types.descriptor(ty)?.shape == ShapeCategory::Unsized {
            return Err(invalid());
        }
        Ok(ty)
    }
    /// Only a completely observed constructor can form a descriptor. This
    /// bounded pure preview does not intern a type or transfer any owner.
    pub(crate) fn observed_construction_key(
        &self,
        guard: ConstructionId,
    ) -> Result<TypeKey, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.state != ConstructionState::Building
            || record.pending_cleanup.is_some()
            || record.completed.len() != record.fields.len()
            || record.fields.iter().any(Option::is_none)
        {
            return Err(ownership());
        }
        match record.observed_kind.ok_or_else(ownership)? {
            StructuralConstructionKind::Tuple if record.fields.is_empty() => {
                Ok(self.types.descriptor(TypeId(1))?.key.as_ref().clone())
            }
            StructuralConstructionKind::Tuple => {
                let mut fields = Vec::new();
                fields
                    .try_reserve_exact(record.expected.len())
                    .map_err(|_| resource())?;
                for ty in &record.expected {
                    fields.push(ty.ok_or_else(ownership)?);
                }
                Ok(TypeKey::Tuple(fields.into()))
            }
            StructuralConstructionKind::Array => {
                let element = record.observed_element.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "empty array requires its source element type context".into(),
                    )
                })?;
                if !record.expected.iter().all(|ty| *ty == Some(element)) {
                    return Err(ownership());
                }
                Ok(TypeKey::Array {
                    element,
                    len: u32::try_from(record.fields.len()).map_err(|_| resource())?,
                })
            }
        }
    }
    /// Validate, then consume the ticket exactly once into the selected slot.
    /// Completion order is appended only after the successful transfer.
    pub fn construction_push(
        &mut self,
        guard: ConstructionId,
        field: u32,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let record = self.constructions.get(guard)?;
        let index = field as usize;
        if record.state != ConstructionState::Building
            || record.fields.get(index).is_none_or(Option::is_some)
        {
            return Err(ownership());
        }
        let value = self.peek_ticket(ticket)?;
        let expected = self.construction_field_type(guard, index, value)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        self.require_initialized_value(value)?;
        let literal = matches!(value.kind, 11 | 12).then(|| LiteralId::from_bits(value.payload));
        let concrete = if let Some(literal) = literal {
            if self
                .literals
                .get(literal)?
                .binding
                .is_some_and(|bound| bound != expected)
            {
                return Err(ownership());
            }
            self.literal_value(value, expected)?
        } else {
            if value.type_id != expected.0 {
                return Err(ownership());
            }
            value
        };
        self.tickets.take(ticket, &mut self.budget)?;
        if let Some(literal) = literal {
            self.literals.get_mut(literal)?.binding = Some(expected);
        }
        self.set_owner(concrete, ValueOwner::Construction { guard, field })?;
        let record = self.constructions.get_mut(guard)?;
        record.expected[index] = Some(expected);
        if record.observed_kind == Some(StructuralConstructionKind::Array) {
            record.observed_element = Some(expected);
        }
        record.fields[index] = Some(concrete);
        record.completed.push(field);
        Ok(())
    }
    /// Move a frame field into a guard only after the target contract accepts it.
    /// A rejected type, occupied field or ticket reservation leaves the source
    /// cell live. No guest callback can run between admission and transfer.
    pub(crate) fn construction_move_from(
        &mut self,
        guard: ConstructionId,
        field: u32,
        source: CellId,
    ) -> Result<(), Diagnostic> {
        self.admit_construction_move(guard, field, source)?;
        let ticket = self.move_from(source)?;
        self.construction_push(guard, field, ticket)
    }
    pub(crate) fn admit_construction_move(
        &self,
        guard: ConstructionId,
        field: u32,
        source: CellId,
    ) -> Result<(), Diagnostic> {
        let record = self.constructions.get(guard)?;
        let index = field as usize;
        if record.state != ConstructionState::Building
            || record.fields.get(index).is_none_or(Option::is_some)
        {
            return Err(ownership());
        }
        let value = self.admit_move(source)?;
        let expected = self.construction_field_type(guard, index, value)?;
        if matches!(value.kind, 11 | 12) {
            let literal = self.literals.get(LiteralId::from_bits(value.payload))?;
            if literal.binding.is_some_and(|bound| bound != expected) {
                return Err(ownership());
            }
            self.literal_value(value, expected)?;
        } else if value.type_id != expected.0 {
            return Err(ownership());
        }
        Ok(())
    }
    /// Admit only a builtin scalar field for the initial panic-cleanup path.
    /// An unresolved destructor policy must leave the guard and its cursor intact.
    pub(crate) fn admit_scalar_construction_cleanup(
        &self,
        guard: ConstructionId,
    ) -> Result<bool, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let Some(field) = record.completed.last().copied() else {
            return Ok(false);
        };
        let value = record.fields[field as usize].ok_or_else(ownership)?;
        self.object_owner(value, ValueOwner::Construction { guard, field })?;
        if !(1..=7).contains(&value.kind) {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "constructor field cleanup requires ordinary destructor dispatch".into(),
            ));
        }
        value.validate_scalar()?;
        Ok(true)
    }
    /// Move one reverse-initialization-order value to the ordinary cleanup machine.
    /// Advance ownership before a guest Drop can suspend or fault. Reservation
    /// failure leaves the cursor and value untouched; no callback runs here.
    pub fn construction_cleanup_next(
        &mut self,
        guard: ConstructionId,
    ) -> Result<Option<TicketId>, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let Some(field) = record.completed.last().copied() else {
            self.constructions.take(guard, &mut self.budget)?;
            return Ok(None);
        };
        let value = record.fields[field as usize].ok_or_else(ownership)?;
        self.object_owner(value, ValueOwner::Construction { guard, field })?;
        self.tickets.reserve(1, &mut self.budget)?;
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        let record = self.constructions.get_mut(guard)?;
        record.state = ConstructionState::Cleaning;
        record.completed.pop();
        record.fields[field as usize] = None;
        Ok(Some(ticket))
    }
    /// Commit a fully initialized structural or source-admitted nominal aggregate.
    /// Reserve every destination before consuming any guarded field ownership.
    pub fn commit_construction(
        &mut self,
        guard: ConstructionId,
        ty: TypeId,
    ) -> Result<TicketId, Diagnostic> {
        self.commit_construction_with_storage(guard, ty, None)
    }
    fn commit_construction_with_storage(
        &mut self,
        guard: ConstructionId,
        ty: TypeId,
        reserved: Option<constant_instantiation::ReservedArrayStorage>,
    ) -> Result<TicketId, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.state != ConstructionState::Building
            || record.completed.len() != record.fields.len()
            || record.fields.iter().any(Option::is_none)
        {
            return Err(ownership());
        }
        if ty == TypeId(1)
            && record.fields.is_empty()
            && record.nominal.is_none()
            && record.observed_kind != Some(StructuralConstructionKind::Array)
        {
            let ticket = self.ticket(ValueRepr::unit())?;
            self.constructions.take(guard, &mut self.budget)?;
            return Ok(ticket);
        }
        let descriptor = self.types.descriptor(ty)?;
        if record.observed_kind.is_some()
            && descriptor.key.as_ref() != &self.observed_construction_key(guard)?
        {
            return Err(ownership());
        }
        let variant = record.nominal.map(|(_, variant)| variant);
        let valid = match descriptor.key.as_ref() {
            TypeKey::Tuple(members) | TypeKey::Callable { captures: members, .. } => {
                record.nominal.is_none()
                    && members.len() == record.expected.len()
                    && members
                        .iter()
                        .zip(&record.expected)
                        .all(|(member, ty)| Some(*member) == *ty)
            }
            TypeKey::Array { element, len } => {
                record.nominal.is_none()
                    && *len as usize == record.expected.len()
                    && record.expected.iter().all(|ty| *ty == Some(*element))
            }
            TypeKey::Nominal { .. } => record.nominal.is_some_and(|(expected, _)| expected == ty),
            _ => false,
        };
        if !valid {
            return Err(ownership());
        }
        for (field, value) in record.fields.iter().enumerate() {
            self.require_initialized_value(value.ok_or_else(ownership)?)?;
            self.object_owner(
                value.ok_or_else(ownership)?,
                ValueOwner::Construction {
                    guard,
                    field: u32::try_from(field).map_err(|_| resource())?,
                },
            )?;
        }
        let count = record.fields.len();
        self.objects.reserve(1, &mut self.budget)?;
        self.cells.reserve(count, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let reserved = match reserved {
            Some(reserved) => reserved,
            None => constant_instantiation::ReservedArrayStorage::new(count)?,
        };
        let (mut fields, initialized) = reserved.into_parts(count)?;
        let owned = fields
            .capacity()
            .checked_mul(std::mem::size_of::<CellId>())
            .and_then(|bytes| bytes.checked_add(initialized.owned_bytes()?))
            .ok_or_else(resource)?;
        let object = ObjectId::from(self.objects.allocate(
            ObjectRecord {
                type_id: ty,
                storage: ObjectStorage::Constructing,
                owner: None,
            },
            owned,
            &mut self.budget,
        )?);
        // Handles, capacities and byte quota were admitted together. Until the
        // final install the object is inaccessible and guard owners stay live.
        for field in 0..count {
            let expected = self.constructions.get(guard)?.expected[field].ok_or_else(ownership)?;
            fields.push(self.new_cell(
                StorageContract::Exact(expected),
                true,
                CellOwner::Field {
                    object,
                    index: u32::try_from(field).map_err(|_| resource())?,
                },
            )?);
        }
        let value = ValueRepr {
            kind: 8,
            type_id: ty.0,
            payload: object.bits(),
        };
        let ticket = TicketId::from(self.tickets.allocate(
            TransferTicket { value },
            0,
            &mut self.budget,
        )?);
        let record = self.constructions.take(guard, &mut self.budget)?;
        for (field, cell) in fields.iter().enumerate() {
            let value = record.fields[field].ok_or_else(ownership)?;
            self.set_owner(value, ValueOwner::Cell(*cell))?;
            self.cells.get_mut(*cell)?.state = CellState::Live(value);
        }
        let object = self.objects.get_mut(object)?;
        object.storage = ObjectStorage::Aggregate(AggregateStorage {
            fields,
            initialized,
            next_cleanup: 0,
            pending_cleanup: None,
            variant,
        });
        object.owner = Some(ValueOwner::Ticket(ticket));
        Ok(ticket)
    }
    fn place_stamp(&self, id: CellId) -> Result<CellStamp, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired || !matches!(cell.state, CellState::Live(_)) {
            return Err(lease_fault());
        }
        Ok(CellStamp {
            cell: id,
            epoch: cell.location_epoch,
        })
    }
    fn address_stamp(&self, id: CellId) -> Result<CellStamp, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired
            || !matches!(
                cell.state,
                CellState::Live(_) | CellState::Moved | CellState::Uninit
            )
        {
            return Err(lease_fault());
        }
        Ok(CellStamp {
            cell: id,
            epoch: cell.location_epoch,
        })
    }
    fn address_type(&self, id: CellId) -> Result<TypeId, Diagnostic> {
        let cell = self.cells.get(id)?;
        self.address_stamp(id)?;
        match cell.state {
            CellState::Live(_) => self.place_type(id),
            CellState::Moved | CellState::Uninit => match cell.declared {
                StorageContract::Exact(ty) => {
                    self.types.descriptor(ty)?;
                    Ok(ty)
                }
                _ => Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "uninitialized address has no retained exact storage descriptor".into(),
                )),
            },
            _ => Err(lease_fault()),
        }
    }
    fn place_type(&self, id: CellId) -> Result<TypeId, Diagnostic> {
        let cell = self.cells.get(id)?;
        let CellState::Live(value) = cell.state else {
            return Err(lease_fault());
        };
        self.object_owner(value, ValueOwner::Cell(id))?;
        let value = if value.kind == 11 || value.kind == 12 {
            self.bound_literal(value)?
        } else {
            value
        };
        Ok(TypeId(value.type_id))
    }
    fn place_object(&self, id: CellId) -> Result<(ObjectId, &AggregateStorage), Diagnostic> {
        self.place_stamp(id)?;
        let CellState::Live(value) = self.cells.get(id)?.state else {
            return Err(lease_fault());
        };
        self.object_owner(value, ValueOwner::Cell(id))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let object_id = ObjectId::from_bits(value.payload);
        let ObjectStorage::Aggregate(storage) = &self.objects.get(object_id)?.storage else {
            return Err(invalid());
        };
        if storage.next_cleanup != 0 {
            return Err(lease_fault());
        }
        Ok((object_id, storage))
    }
    fn validate_reference_record(&self, reference: &ReferenceRecord) -> Result<(), Diagnostic> {
        match &reference.lease {
            ReferenceLease::Location(location) => {
                self.validate_reference_target(
                    location.root,
                    &location.enclosing,
                    location.final_cell,
                    reference.target,
                )?;
                self.validate_source_projection_steps(
                    &location.enclosing,
                    &location.source_projections,
                )
            }
            ReferenceLease::Buffer(buffer) => {
                if reference.access != Access::Shared
                    || reference.owner.is_some()
                    || reference.target != TypeId(16)
                {
                    return Err(access_fault());
                }
                self.strings.resolve(*buffer).map(|_| ())
            }
            ReferenceLease::Elements(elements) => {
                // The window stays inside the borrowed array's own retained
                // length, and the reference names exactly its `[T]` view.
                let base = &elements.base;
                let array = self.place_type(base.final_cell.cell)?;
                let TypeKey::Array { element, len } = self.types.descriptor(array)?.key.as_ref()
                else {
                    return Err(lease_fault());
                };
                if elements
                    .start
                    .checked_add(elements.len)
                    .is_none_or(|end| end > *len)
                    || self.types.find(&TypeKey::Slice { element: *element })
                        != Some(reference.target)
                {
                    return Err(lease_fault());
                }
                self.validate_reference_target(
                    base.root,
                    &base.enclosing,
                    base.final_cell,
                    array,
                )?;
                self.validate_source_projection_steps(&base.enclosing, &base.source_projections)
            }
        }
    }
    fn validate_source_projection_steps(
        &self,
        enclosing: &[EnclosingObject],
        sources: &[SourceProjectionStep],
    ) -> Result<(), Diagnostic> {
        let mut previous = None;
        for source in sources {
            if previous.is_some_and(|previous| previous >= source.chain_index) {
                return Err(lease_fault());
            }
            previous = Some(source.chain_index);
            let link = enclosing
                .get(source.chain_index as usize)
                .ok_or_else(lease_fault)?;
            let key = self
                .types
                .descriptor(self.place_type(link.parent.cell)?)?
                .key
                .as_ref();
            Self::require_source_projection_parent(&source.member, key)?;
            if link.variant != Some(source.member.variant)
                || link.field != source.member.field
                || self.place_type(link.child)? != source.member.ty
            {
                return Err(lease_fault());
            }
        }
        Ok(())
    }
    fn require_source_projection_parent(
        source: &SourceFieldProjection,
        key: &TypeKey,
    ) -> Result<(), Diagnostic> {
        // An instantiated generic parent is admitted: the projection's own
        // recorded field descriptor already carries its instantiation.
        if !matches!(key,TypeKey::Nominal {identity,schema,const_args,..}
            if *identity==source.definition && *schema==source.schema && const_args.is_empty())
        {
            return Err(ownership());
        }
        Ok(())
    }
    /// The same checked chain serves live leases and retained input-target
    /// receipts. Endpoint stamps alone cannot certify intermediate ownership.
    fn validate_reference_target(
        &self,
        root: CellStamp,
        enclosing: &[EnclosingObject],
        final_cell: CellStamp,
        target: TypeId,
    ) -> Result<(), Diagnostic> {
        if self.place_stamp(root.cell)? != root {
            return Err(lease_fault());
        }
        match self.cells.get(root.cell)?.owner {
            CellOwner::FrameSlot { frame, .. } => {
                self.frames.get(frame)?;
            }
            CellOwner::Transfer(ticket) => {
                self.tickets.get(ticket)?;
            }
            CellOwner::Field { .. } | CellOwner::Buffer { .. } => return Err(lease_fault()),
        }
        let mut parent = root.cell;
        for link in enclosing {
            if link.parent.cell != parent || self.place_stamp(parent)? != link.parent {
                return Err(lease_fault());
            }
            let (object, storage) = self.place_object(parent)?;
            if object != link.object
                || storage.variant != link.variant
                || storage.fields.get(link.field as usize) != Some(&link.child)
                || self.cells.get(link.child)?.owner
                    != (CellOwner::Field {
                        object,
                        index: link.field,
                    })
            {
                return Err(lease_fault());
            }
            parent = link.child;
        }
        if parent != final_cell.cell
            || self.place_stamp(parent)? != final_cell
            || self.place_type(parent)? != target
        {
            return Err(lease_fault());
        }
        Ok(())
    }
    /// One checked parent link contains no allocation or cached host pointer.
    fn owning_parent_link(
        &self,
        current: CellId,
        address_leaf: Option<CellId>,
    ) -> Result<Option<EnclosingObject>, Diagnostic> {
        if address_leaf == Some(current) {
            self.address_stamp(current)?;
        } else {
            self.place_stamp(current)?;
        }
        match self.cells.get(current)?.owner {
            CellOwner::FrameSlot { frame, .. } => {
                self.frames.get(frame)?;
                Ok(None)
            }
            CellOwner::Transfer(ticket) => {
                self.tickets.get(ticket)?;
                Ok(None)
            }
            CellOwner::Buffer { .. } => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "buffer references require a buffer lease".into(),
            )),
            CellOwner::Field { object, index } => {
                let Some(ValueOwner::Cell(parent)) = self.objects.get(object)?.owner else {
                    return Err(lease_fault());
                };
                let (actual, storage) = self.place_object(parent)?;
                if actual != object || storage.fields.get(index as usize) != Some(&current) {
                    return Err(lease_fault());
                }
                Ok(Some(EnclosingObject {
                    parent: self.place_stamp(parent)?,
                    object,
                    field: index,
                    child: current,
                    variant: storage.variant,
                }))
            }
        }
    }
    fn canonical_place_root(
        &self,
        id: CellId,
        address: bool,
    ) -> Result<(CellStamp, Vec<EnclosingObject>, bool), Diagnostic> {
        if address {
            self.address_stamp(id)?;
        } else {
            self.place_stamp(id)?;
        }
        let address_leaf = address.then_some(id);
        let mut current = id;
        let mut enclosing = Vec::new();
        let mut writable = self.cells.get(id)?.writable;
        let mut slow = Some(id);
        let mut fast = Some(id);
        while let Some(link) = self.owning_parent_link(current, address_leaf)? {
            super::place::reserve_chain_link(&mut enclosing)?;
            enclosing.push(link);
            current = link.parent.cell;
            writable &= self.cells.get(current)?.writable;
            // Floyd's checked walk detects forged cycles without a growing
            // set or rescanning the retained ancestry at every guest depth.
            slow = match slow {
                Some(cell) => self
                    .owning_parent_link(cell, address_leaf)?
                    .map(|link| link.parent.cell),
                None => None,
            };
            for _ in 0..2 {
                fast = match fast {
                    Some(cell) => self
                        .owning_parent_link(cell, address_leaf)?
                        .map(|link| link.parent.cell),
                    None => None,
                };
            }
            if slow.is_some() && slow == fast {
                return Err(lease_fault());
            }
        }
        checked_chain_bytes(enclosing.capacity())?;
        enclosing.reverse();
        let root = if address_leaf == Some(current) {
            self.address_stamp(current)?
        } else {
            self.place_stamp(current)?
        };
        Ok((root, enclosing, writable))
    }
    fn resolve_place(&self, place: &Place) -> Result<ResolvedPlace, PlaceFailure> {
        self.resolve_place_for_use(place, PlaceUse::Value)
    }
    /// Sized place resolution: an unsized element window (`*s`, `a[1..3]`)
    /// is only an address (its Type) or a borrow/index base, never a value.
    fn resolve_place_for_use(
        &self,
        place: &Place,
        usage: PlaceUse,
    ) -> Result<ResolvedPlace, PlaceFailure> {
        let resolved = self.resolve_place_windowed(place, usage)?;
        if resolved.window.is_some() && usage != PlaceUse::Address {
            return Err(unsized_place().into());
        }
        Ok(resolved)
    }
    fn resolve_place_windowed(
        &self,
        place: &Place,
        usage: PlaceUse,
    ) -> Result<ResolvedPlace, PlaceFailure> {
        let (root, mut target, writable, prefix, mut source_projections, mut window) = match place
            .root
        {
            PlaceRoot::Cell(id) => {
                if usage == PlaceUse::OwningRead {
                    self.admit_read_location(id)?;
                }
                let (root, enclosing, writable) =
                    self.canonical_place_root(id, usage == PlaceUse::Address)?;
                // Extend the exact owning ancestry even if a trusted caller
                // supplies an already-projected field CellId as its root.
                let count = enclosing
                    .len()
                    .checked_add(place.projections.len())
                    .ok_or_else(resource)?;
                checked_chain_bytes(count)?;
                (root, id, writable, enclosing, Vec::new(), None)
            }
            PlaceRoot::Deref(id) => {
                let reference = self.references.get(id)?;
                self.validate_reference_record(reference)?;
                // A slice reference dereferences to its element window of the
                // borrowed array location.
                let (location, window) = match &reference.lease {
                    ReferenceLease::Elements(elements) => {
                        (&elements.base, Some((elements.start, elements.len)))
                    }
                    _ => (reference.location()?, None),
                };
                (
                    location.root,
                    location.final_cell.cell,
                    reference.access == Access::Mutable
                        && self.cells.get(location.root.cell)?.writable,
                    {
                        let mut prefix = Vec::new();
                        checked_chain_bytes(location.enclosing.len())?;
                        prefix
                            .try_reserve_exact(location.enclosing.len())
                            .map_err(|_| resource())?;
                        prefix.extend_from_slice(&location.enclosing);
                        prefix
                    },
                    {
                        let mut sources = Vec::new();
                        checked_source_projection_bytes(location.source_projections.len())?;
                        sources
                            .try_reserve_exact(location.source_projections.len())
                            .map_err(|_| resource())?;
                        sources.extend_from_slice(&location.source_projections);
                        sources
                    },
                    window,
                )
            }
        };
        let count = prefix
            .len()
            .checked_add(place.projections.len())
            .ok_or_else(resource)?;
        checked_chain_bytes(count)?;
        let mut enclosing = Vec::new();
        enclosing.try_reserve_exact(count).map_err(|_| resource())?;
        enclosing.extend_from_slice(&prefix);
        let additional_sources = place
            .projections
            .iter()
            .filter(|projection| matches!(projection, Projection::SourceField(_)))
            .count();
        let source_count = source_projections
            .len()
            .checked_add(additional_sources)
            .ok_or_else(resource)?;
        checked_source_projection_bytes(source_count)?;
        source_projections
            .try_reserve_exact(additional_sources)
            .map_err(|_| resource())?;
        let mut writable = writable;
        for projection in &place.projections {
            let parent = self.place_stamp(target)?;
            let ty = self.place_type(target)?;
            let key = self.types.descriptor(ty)?.key.as_ref();
            let field = match (projection, key, window) {
                // A range narrows the element window of the same array cell.
                (Projection::Range { start, end }, TypeKey::Array { len, .. }, current) => {
                    let (base, available) = current.unwrap_or((0, *len));
                    if start > end || *end > available {
                        return Err(PlaceFailure::GuestPanic(PlacePanic::RangeEndOutOfBounds {
                            end: u64::from(*end),
                            len: u64::from(available),
                        }));
                    }
                    window = Some((
                        base.checked_add(*start).ok_or_else(resource)?,
                        end - start,
                    ));
                    continue;
                }
                // An index into a window is relative to its first element.
                (Projection::Index(index), TypeKey::Array { .. }, Some((start, len))) => {
                    if *index >= len {
                        return Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds {
                            len: u64::from(len),
                            index: u64::from(*index),
                        }));
                    }
                    window = None;
                    start.checked_add(*index).ok_or_else(resource)?
                }
                (_, _, Some(_)) => return Err(invalid().into()),
                (Projection::SourceField(source), key @ TypeKey::Nominal { .. }, None) => {
                    Self::require_source_projection_parent(source, key)?;
                    source.field
                }
                (Projection::Field(field), TypeKey::Nominal { .. }, None) => *field,
                // `*b` of a box: its one owned heap cell.
                (Projection::BoxContents, TypeKey::Nominal { identity, .. }, None)
                    if self.owned_box == Some(*identity) =>
                {
                    0
                }
                (Projection::Tuple(field), TypeKey::Tuple(_) | TypeKey::Callable { .. }, None) => {
                    *field
                }
                (Projection::Index(index), TypeKey::Array { .. }, None) => *index,
                _ => return Err(invalid().into()),
            };
            let (object, storage) = self.place_object(target)?;
            let child = *storage.fields.get(field as usize).ok_or_else(|| {
                if matches!(projection, Projection::Index(_)) {
                    PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds {
                        len: storage.fields.len() as u64,
                        index: u64::from(field),
                    })
                } else {
                    PlaceFailure::RuntimeFault(Diagnostic::new(
                        "LRR-E-PLACE-FIELD",
                        "place field is absent".into(),
                    ))
                }
            })?;
            if self.cells.get(child)?.owner
                != (CellOwner::Field {
                    object,
                    index: field,
                })
            {
                return Err(lease_fault().into());
            }
            // `*b` of a `Box<[T]>` is its `[T; N]` cell's element window.
            if matches!(projection, Projection::BoxContents)
                && let Some(len) = self.unsized_box_window(ty, child)?
            {
                window = Some((0, len));
            }
            if let Projection::SourceField(source) = projection {
                if storage.variant != Some(source.variant) || self.address_type(child)? != source.ty
                {
                    return Err(ownership().into());
                }
                source_projections.push(SourceProjectionStep {
                    chain_index: u32::try_from(enclosing.len()).map_err(|_| resource())?,
                    member: *source,
                });
            }
            enclosing.push(EnclosingObject {
                parent,
                object,
                field,
                child,
                variant: storage.variant,
            });
            target = child;
            if usage == PlaceUse::OwningRead && matches!(place.root, PlaceRoot::Cell(_)) {
                self.admit_read_location(target)?;
            }
            writable &= self.cells.get(target)?.writable;
        }
        checked_chain_bytes(enclosing.capacity())?
            .checked_add(checked_source_projection_bytes(
                source_projections.capacity(),
            )?)
            .ok_or_else(resource)?;
        let ty = if usage == PlaceUse::Address {
            self.address_type(target)?
        } else {
            self.place_type(target)?
        };
        // A window's Type is its array's `[T]` view, interned with the array.
        let ty = match window {
            Some(_) => match self.types.descriptor(ty)?.key.as_ref() {
                TypeKey::Array { element, .. } => self
                    .types
                    .find(&TypeKey::Slice { element: *element })
                    .ok_or_else(invalid)?,
                _ => return Err(invalid().into()),
            },
            None => ty,
        };
        Ok(ResolvedPlace {
            root,
            enclosing,
            target: if usage == PlaceUse::Address {
                self.address_stamp(target)?
            } else {
                self.place_stamp(target)?
            },
            ty,
            writable,
            source_projections,
            window,
        })
    }
    /// Publish a checked run-owned lease only after validation and quota reservation.
    pub fn borrow(&mut self, place: &Place, access: Access) -> Result<RefId, PlaceFailure> {
        let resolved = self.resolve_place_windowed(place, PlaceUse::Value)?;
        if access == Access::Mutable && !resolved.writable {
            return Err(access_fault().into());
        }
        let record = ReferenceRecord::from_place(resolved, access);
        let bytes = record.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        self.references
            .allocate(record, bytes, &mut self.budget)
            .map(RefId::from)
            .map_err(PlaceFailure::RuntimeFault)
    }
    /// Resolve the full place at this operation, then read only an intrinsic Copy
    /// representation. A mutable reference value cannot be moved through deref.
    pub fn read_place(&self, place: &Place) -> Result<ValueRepr, PlaceFailure> {
        let resolved = self.resolve_place(place)?;
        self.admit_copy(resolved.target.cell)
            .map_err(PlaceFailure::RuntimeFault)
    }
    /// Pure owning-read admission checks the full ancestry. Structural tuples
    /// cannot have a user destructor, but a nominal enclosing owner needs its
    /// source-selected Drop policy; absence of a cached witness is not NoDrop.
    pub(crate) fn admit_owning_place_read(
        &self,
        place: &Place,
    ) -> Result<(CellId, ValueRepr, IntrinsicRead), PlaceFailure> {
        let resolved = self.resolve_place_for_use(place, PlaceUse::OwningRead)?;
        let cell = resolved.target.cell;
        let value = self.admit_move(cell)?;
        let mode = self.intrinsic_read_mode(cell)?;
        if mode == IntrinsicRead::Move {
            if matches!(place.root, PlaceRoot::Deref(_)) {
                return Err(access_fault().into());
            }
            for link in &resolved.enclosing {
                let parent = self.place_type(link.parent.cell)?;
                if !matches!(
                    self.types.descriptor(parent)?.key.as_ref(),
                    TypeKey::Tuple(_) | TypeKey::Callable { .. }
                ) && !self.box_parent(parent)?
                {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "partial field move requires the enclosing source-selected Drop policy"
                            .into(),
                    )
                    .into());
                }
            }
        }
        Ok((cell, value, mode))
    }
    /// Transfer a selected field once; sibling cells and parent epochs remain
    /// unchanged. Reserve the ticket before changing the field's unique owner.
    pub(crate) fn read_owning_place(&mut self, place: &Place) -> Result<TicketId, PlaceFailure> {
        let (cell, _, mode) = self.admit_owning_place_read(place)?;
        match mode {
            IntrinsicRead::CopyScalar => self.copy_from(cell),
            IntrinsicRead::CopyAggregate => Err(aggregate_copy::requires_activation()),
            IntrinsicRead::Move => self.move_from(cell),
        }
        .map_err(PlaceFailure::RuntimeFault)
    }
    /// A completed temporary receiver must still own its live value before
    /// ValuePlace can publish an address. This is distinct from missing-field
    /// write-address metadata and performs no copy, move or allocation.
    pub(crate) fn live_place_type_id(&self, place: &Place) -> Result<TypeId, PlaceFailure> {
        self.resolve_place(place).map(|resolved| resolved.ty)
    }
    /// Inspect retained exact address metadata without reading a missing final
    /// value. All enclosing objects and reference records remain checked live.
    pub(crate) fn place_type_id(&self, place: &Place) -> Result<TypeId, PlaceFailure> {
        self.resolve_place_for_use(place, PlaceUse::Address)
            .map(|resolved| resolved.ty)
    }
    /// Resolve an already-selected declaration member from an opaque source
    /// witness. Runtime spelling lookup and a same-shaped foreign type cannot
    /// supply nominal field identity. Source visibility selection stays with
    /// the frontend; this operation admits only its reached bound variant.
    pub fn project_nominal_field(
        &self,
        base: &Place,
        bound: &crate::frontend::types::BoundNominalVariant,
        field: u32,
    ) -> Result<Place, PlaceFailure> {
        let field_type = bound.fields().get(field as usize).ok_or_else(|| {
            Diagnostic::new("LRR-E-PLACE-FIELD", "source-bound field is absent".into())
        })?;
        self.project_bound_field(
            base,
            bound.identity(),
            bound.definition(),
            bound.schema(),
            bound.variant(),
            field,
            field_type,
        )
    }
    /// One used field can be admitted without binding an unrelated field type.
    pub fn project_source_field(
        &self,
        base: &Place,
        bound: &crate::frontend::types::BoundNominalField,
    ) -> Result<Place, PlaceFailure> {
        self.project_bound_field(
            base,
            bound.identity(),
            bound.definition(),
            bound.schema(),
            bound.variant(),
            bound.field(),
            bound.field_type(),
        )
    }
    fn project_bound_field(
        &self,
        base: &Place,
        identity_witness: crate::core::digest::Digest,
        definition_witness: &crate::source::manifest::DefKey,
        schema_witness: crate::core::digest::Digest,
        variant_witness: u32,
        field: u32,
        field_type: &crate::frontend::types::NominalFieldType,
    ) -> Result<Place, PlaceFailure> {
        let resolved = self.resolve_place(base)?;
        let TypeKey::Nominal {
            identity,
            definition,
            schema,
            args,
            const_args,
        } = self.types.descriptor(resolved.ty)?.key.as_ref()
        else {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "source-bound field projection requires a nominal place".into(),
            )
            .into());
        };
        if *identity != identity_witness
            || definition != definition_witness
            || *schema != schema_witness
            || !const_args.is_empty()
        {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "field source witness differs from its nominal descriptor".into(),
            )
            .into());
        }
        let (_, storage) = self.place_object(resolved.target.cell)?;
        if storage.variant != Some(variant_witness) {
            return Err(lease_fault().into());
        }
        // Validate exact installed child ownership and source-selected identity
        // before allocating a projected address or publishing a new lease.
        let child = *storage.fields.get(field as usize).ok_or_else(|| {
            Diagnostic::new("LRR-E-PLACE-FIELD", "active variant field is absent".into())
        })?;
        let (object, _) = self.place_object(resolved.target.cell)?;
        if self.cells.get(child)?.owner
            != (CellOwner::Field {
                object,
                index: field,
            })
        {
            return Err(lease_fault().into());
        }
        let expected = self.address_type(child)?;
        if field_type.find_descriptor(self, args) != Some(expected) {
            return Err(Diagnostic::new(
                "LRR-E-OWNERSHIP",
                "field source type differs from its installed cell".into(),
            )
            .into());
        }
        let place = base.projected(Projection::SourceField(SourceFieldProjection {
            definition: identity_witness,
            schema: schema_witness,
            variant: variant_witness,
            field,
            ty: expected,
        }))?;
        self.resolve_place_for_use(&place, PlaceUse::Address)?;
        Ok(place)
    }
    /// A tuple selector is a source member ordinal, never a runtime name search.
    pub(crate) fn project_tuple(&self, base: &Place, field: u32) -> Result<Place, PlaceFailure> {
        let ty = self.place_type_id(base)?;
        match self.type_descriptor(ty)?.key.as_ref() {
            TypeKey::Tuple(fields) if (field as usize) < fields.len() => {}
            TypeKey::Tuple(_) => {
                return Err(
                    Diagnostic::new("LRR-E-PLACE-FIELD", "tuple field is absent".into()).into(),
                );
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "tuple projection requires a tuple place".into(),
                )
                .into());
            }
        }
        let place = base.projected(Projection::Tuple(field))?;
        self.resolve_place_for_use(&place, PlaceUse::Address)?;
        Ok(place)
    }
    /// Check the evaluated usize and bounds before narrowing to an address ordinal.
    pub(crate) fn project_index(
        &self,
        base: &Place,
        index: ValueRepr,
    ) -> Result<Place, PlaceFailure> {
        // Index identity has its own typed fault even on an unsupported receiver.
        super::place::checked_index(index)?;
        let resolved = self.resolve_place_for_use(base, PlaceUse::Address)?;
        let ty = resolved.ty;
        let length = match self.type_descriptor(ty)?.key.as_ref() {
            TypeKey::Array { len, .. } => *len,
            // A slice window indexes relative to its first element.
            TypeKey::Slice { .. } => resolved.window.ok_or_else(invalid)?.1,
            TypeKey::Tuple(_) | TypeKey::Primitive(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "intrinsic indexing requires an array place".into(),
                )
                .into());
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "index receiver requires reached source trait selection".into(),
                )
                .into());
            }
        };
        let projection = super::place::checked_array_index(index, length)?;
        let place = base.projected(projection)?;
        self.resolve_place_for_use(&place, PlaceUse::Address)?;
        Ok(place)
    }
    pub(crate) fn reserve_place_read_result(&mut self) -> Result<(), Diagnostic> {
        self.cells.reserve(1, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        Ok(())
    }
    pub(crate) fn dereference_place(&self, place: &Place) -> Result<Place, PlaceFailure> {
        let resolved = self.resolve_place(place)?;
        match self.type_descriptor(resolved.ty)?.key.as_ref() {
            TypeKey::Ref { .. } => {}
            // The builtin box dereference projects the box's heap cell.
            TypeKey::Nominal { identity, .. } if self.owned_box == Some(*identity) => {
                return self.dereference_box(place);
            }
            TypeKey::Primitive(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "intrinsic dereference requires a guest reference".into(),
                )
                .into());
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "non-reference dereference requires reached trait selection".into(),
                )
                .into());
            }
        }
        let reference = self.reference_from_cell(resolved.target.cell)?;
        Ok(Place::deref(reference))
    }
    /// Admit only closed intrinsic projected replacement. No source-selected
    /// destructor query or guest callback can occur before RHS transfer.
    pub(crate) fn preview_intrinsic_place_assignment(
        &self,
        place: &Place,
        source: CellId,
    ) -> Result<IntrinsicAssignment, PlaceFailure> {
        let resolved = self.resolve_place_for_use(place, PlaceUse::Address)?;
        if resolved.window.is_some() {
            return Err(unsized_place().into());
        }
        if !resolved.writable {
            return Err(access_fault().into());
        }
        if source == resolved.target.cell {
            return Err(ownership().into());
        }
        let incoming = self.admit_move(source)?;
        let value = match self.type_descriptor(resolved.ty)?.key.as_ref() {
            TypeKey::Primitive(_) => self.preview_scalar(source, Some(resolved.ty))?,
            TypeKey::Ref { .. } => {
                self.admit_new_contract(Some(resolved.ty), incoming)?;
                incoming
            }
            TypeKey::Facade { .. } if self.types.is_string(resolved.ty) => {
                self.admit_new_contract(Some(resolved.ty), incoming)?;
                incoming
            }
            // Owning values: an old value is dropped by its ordinary glue.
            TypeKey::Tuple(_)
            | TypeKey::Array { .. }
            | TypeKey::Nominal { .. }
            | TypeKey::Callable { .. }
                if incoming.kind == 8 =>
            {
                self.admit_new_contract(Some(resolved.ty), incoming)?;
                incoming
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "projected replacement requires ordinary destructor integration".into(),
                )
                .into());
            }
        };
        let plan = match self.state(resolved.target.cell)? {
            CellState::Uninit | CellState::Moved => {
                self.validate_initialization_value(resolved.target.cell, value)?;
                IntrinsicAssignment::Initialize {
                    ty: resolved.ty,
                    target: resolved.target.cell,
                }
            }
            CellState::Live(old) if old.kind == 8 && !self.types.is_string(resolved.ty) => {
                self.admit_replacement(resolved.target.cell, value)?;
                self.admit_cleanup_cell(resolved.target.cell)?;
                IntrinsicAssignment::ReplaceOwned {
                    ty: resolved.ty,
                    target: resolved.target.cell,
                }
            }
            CellState::Live(_) => {
                self.admit_replacement(resolved.target.cell, value)?;
                self.admit_discard(resolved.target.cell)?;
                IntrinsicAssignment::Replace {
                    ty: resolved.ty,
                    target: resolved.target.cell,
                }
            }
            _ => return Err(lease_fault().into()),
        };
        // This checks the real old-value obligation. Closed references destroy
        // only their lease; String destroys its own accounted byte allocation.
        // An absent nominal destructor witness never proves NoDrop.
        Ok(plan)
    }
    /// Reserve the complete intrinsic replacement/result batch before moving RHS.
    pub(crate) fn reserve_intrinsic_place_assignment(
        &mut self,
        needs_cell: bool,
        plan: IntrinsicAssignment,
        materializes: bool,
    ) -> Result<(), Diagnostic> {
        let tickets = 2 + usize::from(plan.replaces_live()) + usize::from(materializes);
        if needs_cell {
            self.cells.reserve(1, &mut self.budget)?;
        }
        self.tickets.reserve(tickets, &mut self.budget)?;
        Ok(())
    }
    /// Revalidate the write address and initialize only a missing terminal
    /// value. No old-value drop obligation is invented for a moved field.
    pub(crate) fn initialize_place(
        &mut self,
        place: &Place,
        incoming: TicketId,
    ) -> Result<(), PlaceFailure> {
        let resolved = self.resolve_place_for_use(place, PlaceUse::Address)?;
        if resolved.window.is_some() {
            return Err(unsized_place().into());
        }
        if !resolved.writable {
            return Err(access_fault().into());
        }
        self.initialize(resolved.target.cell, incoming)
            .map_err(PlaceFailure::RuntimeFault)
    }
    /// The RHS already has a transfer owner. Validate the reached LHS ancestry
    /// and inherited permission before replacement; retain old-value cleanup.
    pub fn replace_place(
        &mut self,
        place: &Place,
        incoming: TicketId,
    ) -> Result<TicketId, PlaceFailure> {
        let resolved = self.resolve_place(place)?;
        if !resolved.writable {
            return Err(access_fault().into());
        }
        self.replace(resolved.target.cell, incoming)
            .map_err(PlaceFailure::RuntimeFault)
    }
    pub(crate) fn admit_place_borrow(
        &self,
        place: &Place,
        access: Access,
    ) -> Result<TypeId, PlaceFailure> {
        let resolved = self.resolve_place_windowed(place, PlaceUse::Value)?;
        if access == Access::Mutable && !resolved.writable {
            return Err(access_fault().into());
        }
        Ok(resolved.ty)
    }
    /// Reserve the destination cell, lease and first ticket as one batch before
    /// any reference publication. Existing owners remain unchanged on refusal.
    pub(crate) fn reserve_place_borrow_result(
        &mut self,
        place: &Place,
        needs_cell: bool,
    ) -> Result<(), PlaceFailure> {
        self.resolve_place_windowed(place, PlaceUse::Value)?;
        if needs_cell {
            self.cells.reserve(1, &mut self.budget)?;
        }
        self.references.reserve(1, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        Ok(())
    }
    /// Reserve both the lease and its first value owner before publishing either.
    pub fn borrow_ticket(
        &mut self,
        place: &Place,
        access: Access,
    ) -> Result<TicketId, PlaceFailure> {
        let resolved = self.resolve_place_windowed(place, PlaceUse::Value)?;
        if access == Access::Mutable && !resolved.writable {
            return Err(access_fault().into());
        }
        let ty = self.types.intern(TypeKey::Ref {
            mutable: access == Access::Mutable,
            target: resolved.ty,
        })?;
        let record = ReferenceRecord::from_place(resolved, access);
        let bytes = record.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let reference = self.references.allocate(record, bytes, &mut self.budget)?;
        let value = ValueRepr {
            kind: 9,
            type_id: ty.0,
            payload: reference.bits(),
        };
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)?
            .into();
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
    }
    /// Materialize a lease as a guest value. Mutable leases cannot be duplicated
    /// by repeatedly materializing their handle; subsequent transport uses owners.
    pub fn reference_ticket(&mut self, id: RefId) -> Result<TicketId, Diagnostic> {
        let record = self.references.get(id)?;
        self.validate_reference_record(record)?;
        if record.access == Access::Mutable && record.owner.is_some() {
            return Err(ownership());
        }
        let mutable = record.access == Access::Mutable;
        let target = record.target;
        let ty = self.types.intern(TypeKey::Ref { mutable, target })?;
        self.tickets.reserve(1, &mut self.budget)?;
        let value = ValueRepr {
            kind: 9,
            type_id: ty.0,
            payload: id.bits(),
        };
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)?
            .into();
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
    }
    /// Extract a reference for a dereference/reborrow context without performing
    /// a Copy read of its value. Both the containing owner and target lease matter.
    pub fn reference_from_cell(&self, cell: CellId) -> Result<RefId, Diagnostic> {
        let value = self.admit_move(cell)?;
        if value.kind != 9 {
            return Err(invalid());
        }
        let id = RefId::from_bits(value.payload);
        self.validate_reference_record(self.references.get(id)?)?;
        Ok(id)
    }
    pub fn read_reference(&self, id: RefId) -> Result<ValueRepr, Diagnostic> {
        let reference = self.references.get(id)?;
        self.validate_reference_record(reference)?;
        // Intrinsic scalars and shared references may be read by Copy. Aggregate
        // Copy and moves through dereferences require a later checked policy.
        self.read_scalar(reference.location()?.final_cell.cell)
    }
    /// Commit replacement and retain its old-value cleanup obligation explicitly.
    pub fn replace_reference(
        &mut self,
        id: RefId,
        incoming: TicketId,
    ) -> Result<TicketId, Diagnostic> {
        let reference = self.references.get(id)?;
        self.validate_reference_record(reference)?;
        if reference.access != Access::Mutable {
            return Err(access_fault());
        }
        let cell = reference.location()?.final_cell.cell;
        self.replace(cell, incoming)
    }
    pub fn aggregate_variant(&self, ticket: TicketId) -> Result<Option<u32>, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let object = self.objects.get(ObjectId::from_bits(value.payload))?;
        let ObjectStorage::Aggregate(storage) = &object.storage else {
            return Err(ownership());
        };
        Ok(storage.variant)
    }
    /// Read-only active variant of the live aggregate at a checked place. A
    /// pattern test reads it without copying, moving or leasing the value;
    /// structs report their single variant.
    pub(crate) fn place_variant(&self, place: &Place) -> Result<Option<u32>, PlaceFailure> {
        let resolved = self.resolve_place(place)?;
        let (_, storage) = self.place_object(resolved.target.cell)?;
        Ok(storage.variant)
    }
    pub fn aggregate_field(&self, ticket: TicketId, field: u32) -> Result<CellId, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let object = self.objects.get(ObjectId::from_bits(value.payload))?;
        let ObjectStorage::Aggregate(storage) = &object.storage else {
            return Err(invalid());
        };
        if storage.next_cleanup != 0 {
            return Err(ownership());
        }
        storage
            .fields
            .get(field as usize)
            .copied()
            .ok_or_else(invalid)
    }
    /// Declaration-order field cleanup is an explicit continuation. The caller
    /// schedules the returned ticket's ordinary Drop and resumes after it returns.
    pub fn aggregate_cleanup_step(
        &mut self,
        ticket: TicketId,
    ) -> Result<AggregateCleanupStep, Diagnostic> {
        match self.aggregate_cleanup_transfer_inner(ticket, false)? {
            AggregateCleanupTransfer::Field { ticket, .. } => {
                Ok(AggregateCleanupStep::Field(ticket))
            }
            AggregateCleanupTransfer::Skipped => Ok(AggregateCleanupStep::Skipped),
            AggregateCleanupTransfer::Complete => Ok(AggregateCleanupStep::Complete),
        }
    }
    pub(crate) fn admit_cleanup_cell(&self, cell: CellId) -> Result<(), Diagnostic> {
        self.admit_live_value(cell).map(|_| ())
    }
    pub(crate) fn admit_cleanup_root(&self, cell: CellId) -> Result<ValueRepr, Diagnostic> {
        self.admit_live_value(cell)
    }
    pub(crate) fn cleanup_ticket_inventory(
        &self,
        ticket: TicketId,
    ) -> Result<CleanupInventory, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.validate_value(value)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        match value.kind {
            1..=7 | 9 | 11 | 12 => Ok(CleanupInventory::Leaf),
            8 if self.types.is_string(TypeId(value.type_id)) => Ok(CleanupInventory::Leaf),
            8 => match self.types.descriptor(TypeId(value.type_id))?.key.as_ref() {
                TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. } => {
                    Ok(CleanupInventory::Fields)
                }
                // No user Drop impl: drop glue drops the fields in order.
                TypeKey::Nominal { identity, .. }
                    if self
                        .user_drop_types
                        .as_ref()
                        .is_some_and(|types| !types.contains(identity)) =>
                {
                    Ok(CleanupInventory::Fields)
                }
                _ => Ok(CleanupInventory::NeedWitness(TypeId(value.type_id))),
            },
            _ => Err(invalid()),
        }
    }
    pub(crate) fn cleanup_next_field(
        &self,
        ticket: TicketId,
    ) -> Result<CleanupFieldAdmission, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let ObjectStorage::Aggregate(storage) = &self
            .objects
            .get(ObjectId::from_bits(value.payload))?
            .storage
        else {
            return Err(invalid());
        };
        if storage.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let Some(field) = storage.fields.get(storage.next_cleanup) else {
            return Ok(CleanupFieldAdmission::Complete);
        };
        match self.state(*field)? {
            CellState::Live(_) => Ok(CleanupFieldAdmission::Live),
            CellState::Uninit | CellState::Moved | CellState::Dropped => {
                Ok(CleanupFieldAdmission::Skip)
            }
            CellState::Dropping => Err(ownership()),
        }
    }
    pub fn aggregate_cleanup_transfer(
        &mut self,
        ticket: TicketId,
    ) -> Result<AggregateCleanupTransfer, Diagnostic> {
        self.aggregate_cleanup_transfer_inner(ticket, true)
    }
    fn aggregate_cleanup_transfer_inner(
        &mut self,
        ticket: TicketId,
        retain_location: bool,
    ) -> Result<AggregateCleanupTransfer, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let id = ObjectId::from_bits(value.payload);
        let ObjectStorage::Aggregate(storage) = &self.objects.get(id)?.storage else {
            return Err(invalid());
        };
        if storage.pending_cleanup.is_some() {
            return Err(ownership());
        }
        let Some(field) = storage.fields.get(storage.next_cleanup).copied() else {
            return Ok(AggregateCleanupTransfer::Complete);
        };
        let next_cleanup = storage.next_cleanup.checked_add(1).ok_or_else(resource)?;
        let step = match self.state(field)? {
            CellState::Live(_) => {
                let child = self.begin_drop(field)?;
                if !retain_location {
                    self.retire_cell(field)?;
                }
                AggregateCleanupTransfer::Field {
                    cell: field,
                    ticket: child,
                }
            }
            CellState::Uninit | CellState::Moved | CellState::Dropped => {
                if !self.cells.get(field)?.retired {
                    self.retire_cell(field)?;
                }
                AggregateCleanupTransfer::Skipped
            }
            CellState::Dropping => return Err(ownership()),
        };
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(id)?.storage else {
            return Err(invalid());
        };
        storage.next_cleanup = next_cleanup;
        if retain_location && let AggregateCleanupTransfer::Field { cell, ticket } = step {
            storage.pending_cleanup = Some((cell, ticket));
        }
        Ok(step)
    }
    /// Acknowledge only the exact consumed child ticket of the suspended field.
    /// Caller completion cannot retire a live ticket or let its parent proceed.
    pub fn finish_aggregate_cleanup_field(
        &mut self,
        parent: TicketId,
        field: CellId,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(parent)?;
        self.object_owner(value, ValueOwner::Ticket(parent))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let id = ObjectId::from_bits(value.payload);
        let ObjectStorage::Aggregate(storage) = &self.objects.get(id)?.storage else {
            return Err(invalid());
        };
        if storage.pending_cleanup != Some((field, ticket)) {
            return Err(ownership());
        }
        match self.tickets.get(ticket) {
            Ok(_) => return Err(ownership()),
            Err(error) if error.code == "LRR-E-STALE-HANDLE" => {}
            Err(error) => return Err(error),
        }
        if self.state(field)? != CellState::Dropping {
            return Err(ownership());
        }
        self.retire_cell(field)?;
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(id)?.storage else {
            return Err(invalid());
        };
        storage.pending_cleanup = None;
        Ok(())
    }
    /// Drop glue lends a cleanup ticket's nominal value to a fresh, writable
    /// frame location, so the selected `Drop::drop` receives an ordinary
    /// `&mut` place to it (its fields resolve through that location). The
    /// ticket record stays reserved for the same cleanup continuation; only
    /// `reclaim_cleanup_ticket` may use it again.
    pub(crate) fn lend_cleanup_ticket(
        &mut self,
        ticket: TicketId,
        receiver: CellId,
    ) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        self.validate_initialization_value(receiver, value)?;
        let cell = self.cells.get(receiver)?;
        if cell.state != CellState::Uninit
            || !cell.writable
            || !matches!(cell.owner, CellOwner::FrameSlot { .. })
        {
            return Err(ownership());
        }
        let declared = self.contract(cell, value)?;
        self.set_owner(value, ValueOwner::Cell(receiver))?;
        let cell = self.cells.get_mut(receiver)?;
        cell.state = CellState::Live(value);
        cell.declared = declared;
        self.commit_initialization_change(receiver, true)
    }
    /// After `Drop::drop` returned (or unwound), move the lent location's
    /// value (possibly replaced through `&mut self`, same type) back into its
    /// reserved cleanup ticket and retire the location.
    pub(crate) fn reclaim_cleanup_ticket(
        &mut self,
        ticket: TicketId,
        receiver: CellId,
    ) -> Result<(), Diagnostic> {
        let lent = self.tickets.get(ticket)?.value;
        let value = self.admit_live_value(receiver)?;
        if value.kind != 8 || value.type_id != lent.type_id {
            return Err(ownership());
        }
        self.prepare_initialization_change(receiver, false)?;
        self.tickets.get_mut(ticket)?.value = value;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        let cell = self.cells.get_mut(receiver)?;
        cell.state = CellState::Moved;
        cell.location_epoch = cell.location_epoch.saturating_add(1);
        self.commit_initialization_change(receiver, false)?;
        self.retire_cell(receiver)
    }
    /// Legacy test convenience only. Production cleanup consumes one explicit
    /// transition at a time, including skips, through aggregate_cleanup_step.
    #[cfg(test)]
    pub fn aggregate_cleanup_next(
        &mut self,
        ticket: TicketId,
    ) -> Result<Option<TicketId>, Diagnostic> {
        loop {
            match self.aggregate_cleanup_step(ticket)? {
                AggregateCleanupStep::Field(child) => return Ok(Some(child)),
                AggregateCleanupStep::Skipped => {}
                AggregateCleanupStep::Complete => return Ok(None),
            }
        }
    }
    pub(crate) fn require_frame(&self, frame: FrameId) -> Result<(), Diagnostic> {
        self.frames.get(frame).map(|_| ())
    }
    pub(crate) fn argument_types(
        &self,
        batch: ArgumentBatchId,
        expected: &[TypeId],
    ) -> Result<(), Diagnostic> {
        let batch = self.argument_batches.get(batch)?;
        if batch.types.len() != expected.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "parameter count differs from sealed argument batch".into(),
            ));
        }
        if batch.types != expected {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARG-TYPE",
                "parameter descriptors differ from the sealed argument batch".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn new_parameter_cells(
        &mut self,
        frame: FrameId,
        base: u32,
        contracts: &[(TypeId, bool)],
    ) -> Result<Vec<CellId>, Diagnostic> {
        self.require_frame(frame)?;
        let count = u32::try_from(contracts.len()).ok().ok_or_else(resource)?;
        base.checked_add(count).ok_or_else(resource)?;
        for (ty, _) in contracts {
            // This allocates empty physical cells. Frame source admission and
            // sealed argument types precede it; packet delivery still checks
            // values, reference targets and ownership. A registered nominal
            // referent must not be reclassified as a primitive here.
            self.types.descriptor(*ty)?;
        }
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(contracts.len())
            .map_err(|_| resource())?;
        self.cells.reserve(contracts.len(), &mut self.budget)?;
        for (ordinal, (ty, writable)) in contracts.iter().enumerate() {
            cells.push(self.new_cell(
                StorageContract::Exact(*ty),
                *writable,
                CellOwner::FrameSlot {
                    frame,
                    slot: base + ordinal as u32,
                },
            )?);
        }
        Ok(cells)
    }
    pub fn new_frame_anchor(&mut self) -> Result<FrameId, Diagnostic> {
        self.frames
            .allocate(FrameAnchor, 0, &mut self.budget)
            .map(FrameId::from)
    }
    pub fn new_cell(
        &mut self,
        declared: StorageContract,
        writable: bool,
        owner: CellOwner,
    ) -> Result<CellId, Diagnostic> {
        match owner {
            CellOwner::FrameSlot { frame, .. } => {
                self.frames.get(frame)?;
            }
            CellOwner::Field { object, .. } | CellOwner::Buffer { object, .. } => {
                self.objects.get(object)?;
            }
            CellOwner::Transfer(ticket) => {
                self.tickets.get(ticket)?;
            }
        }
        self.cells
            .allocate(
                Cell {
                    state: CellState::Uninit,
                    declared,
                    writable,
                    location_epoch: 1,
                    owner,
                    retired: false,
                    numeric_record: None,
                },
                0,
                &mut self.budget,
            )
            .map(CellId::from)
    }
    pub fn state(&self, id: CellId) -> Result<CellState, Diagnostic> {
        Ok(self.cells.get(id)?.state)
    }
    pub fn location_epoch(&self, id: CellId) -> Result<u32, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        Ok(cell.location_epoch)
    }
    pub fn declared(&self, id: CellId) -> Result<StorageContract, Diagnostic> {
        Ok(self.cells.get(id)?.declared)
    }
    fn validate_value(&self, value: ValueRepr) -> Result<(), Diagnostic> {
        match value.kind {
            1..=7 => value.validate_scalar(),
            8 => {
                let object = self.objects.get(ObjectId::from_bits(value.payload))?;
                if object.type_id.0 != value.type_id {
                    return Err(invalid());
                }
                match &object.storage {
                    ObjectStorage::String(_) if self.types.is_string(object.type_id) => Ok(()),
                    ObjectStorage::Aggregate(storage)
                        if match self.types.descriptor(object.type_id)?.key.as_ref() {
                            TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. } => {
                                storage.variant.is_none()
                            }
                            TypeKey::Nominal { .. } => storage.variant.is_some(),
                            _ => false,
                        } =>
                    {
                        Ok(())
                    }
                    _ => Err(invalid()),
                }
            }
            9 => {
                let record = self.references.get(RefId::from_bits(value.payload))?;
                if record.access == Access::Shared && record.owner.is_some() {
                    return Err(ownership());
                }
                match self.types.descriptor(TypeId(value.type_id))?.key.as_ref() {
                    TypeKey::Ref { mutable, target }
                        if *mutable == (record.access == Access::Mutable)
                            && *target == record.target =>
                    {
                        Ok(())
                    }
                    _ => Err(invalid()),
                }
            }
            11 | 12 => {
                if value.type_id != 0 {
                    return Err(invalid());
                }
                let literal = self.literals.get(LiteralId::from_bits(value.payload))?;
                if matches!(
                    (&literal.number, value.kind),
                    (LiteralNumber::Integer { .. }, 11) | (LiteralNumber::Float, 12)
                ) {
                    Ok(())
                } else {
                    Err(invalid())
                }
            }
            _ => Err(invalid()),
        }
    }
    pub fn ticket(&mut self, value: ValueRepr) -> Result<TicketId, Diagnostic> {
        self.validate_value(value)?;
        if value.kind == 8
            || (value.kind == 9
                && self.references.get(RefId::from_bits(value.payload))?.access == Access::Mutable)
        {
            return Err(ownership());
        }
        self.tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)
    }
    pub fn peek_ticket(&self, ticket: TicketId) -> Result<ValueRepr, Diagnostic> {
        Ok(self.tickets.get(ticket)?.value)
    }
    fn object_owner(&self, value: ValueRepr, owner: ValueOwner) -> Result<(), Diagnostic> {
        self.validate_value(value)?;
        if value.kind == 8
            && self.objects.get(ObjectId::from_bits(value.payload))?.owner != Some(owner)
        {
            return Err(ownership());
        }
        if value.kind == 9 {
            let record = self.references.get(RefId::from_bits(value.payload))?;
            if record.access == Access::Mutable && record.owner != Some(owner) {
                return Err(ownership());
            }
        }
        Ok(())
    }
    fn set_owner(&mut self, value: ValueRepr, owner: ValueOwner) -> Result<(), Diagnostic> {
        if value.kind == 8 {
            self.objects
                .get_mut(ObjectId::from_bits(value.payload))?
                .owner = Some(owner);
        }
        if value.kind == 9 {
            let record = self.references.get_mut(RefId::from_bits(value.payload))?;
            if record.access == Access::Mutable {
                record.owner = Some(owner);
            }
        }
        Ok(())
    }
    pub fn string(&mut self, text: &str) -> Result<TicketId, Diagnostic> {
        let ty = self.types.intern_string()?;
        self.objects.reserve(1, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(text.len())
            .map_err(|_| resource())?;
        bytes.extend_from_slice(text.as_bytes());
        let owned = bytes.capacity();
        let object = self.objects.allocate(
            ObjectRecord {
                type_id: ty,
                storage: ObjectStorage::String(bytes),
                owner: None,
            },
            owned,
            &mut self.budget,
        )?;
        let value = ValueRepr {
            kind: 8,
            type_id: ty.0,
            payload: object.bits(),
        };
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
    }
    pub(crate) fn integer_parts(
        &self,
        value: ValueRepr,
    ) -> Result<Option<(bool, u128)>, Diagnostic> {
        if value.kind != 11 {
            return Ok(None);
        }
        self.validate_value(value)?;
        match self
            .literals
            .get(LiteralId::from_bits(value.payload))?
            .number
        {
            LiteralNumber::Integer {
                negative,
                magnitude,
            } => Ok(Some((negative, magnitude))),
            _ => Err(invalid()),
        }
    }
    pub(crate) fn effective_type(&self, value: ValueRepr) -> Result<Option<TypeId>, Diagnostic> {
        if value.kind == 11 || value.kind == 12 {
            Ok(self
                .literals
                .get(LiteralId::from_bits(value.payload))?
                .binding)
        } else {
            Ok(Some(TypeId(value.type_id)))
        }
    }
    fn contract(&self, cell: &Cell, value: ValueRepr) -> Result<StorageContract, Diagnostic> {
        let incoming = self.effective_type(value)?;
        if cell.declared == StorageContract::NumericLiteral {
            if value.kind != 11 && value.kind != 12 && !matches!(incoming, Some(TypeId(4..=15))) {
                return Err(ownership());
            }
            if let Some(record) = cell.numeric_record
                && let Some(expected) = self.literals.get(record)?.binding
            {
                return if incoming == Some(expected) {
                    Ok(StorageContract::Exact(expected))
                } else {
                    Err(ownership())
                };
            }
            return Ok(incoming.map_or(StorageContract::NumericLiteral, StorageContract::Exact));
        }
        match cell.declared {
            StorageContract::Unbound => {
                Ok(incoming.map_or(StorageContract::NumericLiteral, StorageContract::Exact))
            }
            StorageContract::Exact(expected) if incoming == Some(expected) => Ok(cell.declared),
            _ => Err(ownership()),
        }
    }
    /// Pure destination admission before a VM transfer moves its source.
    pub(crate) fn validate_initialization_value(
        &self,
        id: CellId,
        value: ValueRepr,
    ) -> Result<(), Diagnostic> {
        self.validate_value(value)?;
        self.require_initialized_value(value)?;
        let cell = self.cells.get(id)?;
        if cell.retired
            || !matches!(cell.state, CellState::Uninit | CellState::Moved)
            || matches!(cell.state, CellState::Moved) && !cell.writable
        {
            return Err(ownership());
        }
        self.contract(cell, value)?;
        self.prepare_initialization_change(id, true)?;
        Ok(())
    }
    /// Pure admission for an already-bound constructor result, before commit
    /// consumes its guard. Aggregate/unit results cannot satisfy literal cells.
    pub(crate) fn validate_constructor_destination(
        &self,
        id: CellId,
        ty: TypeId,
    ) -> Result<(), Diagnostic> {
        self.types.descriptor(ty)?;
        let cell = self.cells.get(id)?;
        if cell.retired
            || !matches!(cell.state, CellState::Uninit | CellState::Moved)
            || matches!(cell.state, CellState::Moved) && !cell.writable
            || !matches!(cell.declared, StorageContract::Unbound)
                && !matches!(cell.declared, StorageContract::Exact(expected) if expected == ty)
        {
            return Err(ownership());
        }
        self.prepare_initialization_change(id, true)?;
        Ok(())
    }
    /// An observed result has not interned its descriptor yet. Its destination
    /// must accept first initialization without guessing a storage contract.
    pub(crate) fn validate_observed_constructor_destination(
        &self,
        id: CellId,
    ) -> Result<(), Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired
            || !matches!(cell.state, CellState::Uninit | CellState::Moved)
            || matches!(cell.state, CellState::Moved) && !cell.writable
            || cell.declared != StorageContract::Unbound
        {
            return Err(ownership());
        }
        self.prepare_initialization_change(id, true)?;
        Ok(())
    }
    /// The detached old value of replacement may have moved fields. Ordinary
    /// initialization must keep rejecting it; only an unpublished FrameSlot
    /// cleanup root may adopt it, and ordinary reads still require completeness.
    pub(crate) fn validate_replacement_cleanup_root(
        &self,
        id: CellId,
        value: ValueRepr,
    ) -> Result<(), Diagnostic> {
        self.validate_value(value)?;
        // A replaced nominal value is cleaned up by the same machine as a
        // structural one (drop glue, or its demanded Drop witness).
        if value.kind != 8
            || !matches!(
                self.types.descriptor(TypeId(value.type_id))?.key.as_ref(),
                TypeKey::Tuple(_)
                    | TypeKey::Array { .. }
                    | TypeKey::Nominal { .. }
                    | TypeKey::Callable { .. }
            )
        {
            return Err(ownership());
        }
        let cell = self.cells.get(id)?;
        if cell.retired
            || cell.state != CellState::Uninit
            || !matches!(cell.owner, CellOwner::FrameSlot { .. })
        {
            return Err(ownership());
        }
        self.contract(cell, value)?;
        self.prepare_initialization_change(id, false)?;
        Ok(())
    }
    pub(crate) fn initialize_replacement_cleanup_root(
        &mut self,
        id: CellId,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        self.validate_replacement_cleanup_root(id, value)?;
        let declared = self.contract(self.cells.get(id)?, value)?;
        self.tickets.take(ticket, &mut self.budget)?;
        self.set_owner(value, ValueOwner::Cell(id))?;
        let cell = self.cells.get_mut(id)?;
        cell.state = CellState::Live(value);
        cell.declared = declared;
        // This root has no aggregate parent. Its value's actual child summary
        // remains unchanged; no partially moved value becomes ordinarily usable.
        self.commit_initialization_change(id, false)
    }
    pub fn initialize(&mut self, id: CellId, ticket: TicketId) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        self.validate_initialization_value(id, value)?;
        let cell = self.cells.get(id)?;
        if cell.retired
            || !matches!(cell.state, CellState::Uninit | CellState::Moved)
            || matches!(cell.state, CellState::Moved) && !cell.writable
        {
            return Err(ownership());
        }
        let declared = self.contract(cell, value)?;
        self.tickets.take(ticket, &mut self.budget)?;
        self.set_owner(value, ValueOwner::Cell(id))?;
        let cell = self.cells.get_mut(id)?;
        cell.state = CellState::Live(value);
        cell.declared = declared;
        cell.numeric_record = if value.kind == 11 || value.kind == 12 {
            Some(LiteralId::from_bits(value.payload))
        } else {
            None
        };
        self.commit_initialization_change(id, true)?;
        Ok(())
    }
    pub(crate) fn preview_scalar(
        &self,
        id: CellId,
        expected: Option<TypeId>,
    ) -> Result<ValueRepr, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        let CellState::Live(value) = cell.state else {
            return Err(ownership());
        };
        self.object_owner(value, ValueOwner::Cell(id))?;
        if matches!(value.kind, 11 | 12) {
            let ty = self
                .effective_type(value)?
                .or(expected)
                .ok_or(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "numeric literal requires reached concrete context".into(),
                ))?;
            return self.literal_value(value, ty);
        }
        value.validate_scalar()?;
        Ok(value)
    }
    pub fn read_scalar(&self, id: CellId) -> Result<ValueRepr, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        let CellState::Live(value) = cell.state else {
            return Err(ownership());
        };
        self.object_owner(value, ValueOwner::Cell(id))?;
        if value.kind == 11 || value.kind == 12 {
            return self.bound_literal(value);
        }
        if value.kind == 9 {
            if self.references.get(RefId::from_bits(value.payload))?.access != Access::Shared {
                return Err(ownership());
            }
        } else {
            value.validate_scalar()?;
        }
        Ok(value)
    }
    fn admit_live_value(&self, id: CellId) -> Result<ValueRepr, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        let CellState::Live(value) = cell.state else {
            return Err(ownership());
        };
        self.validate_value(value)?;
        self.object_owner(value, ValueOwner::Cell(id))?;
        Ok(value)
    }
    pub(crate) fn admit_move(&self, id: CellId) -> Result<ValueRepr, Diagnostic> {
        let value = self.admit_live_value(id)?;
        self.require_initialized_value(value)?;
        Ok(value)
    }
    fn value_is_initialized(&self, value: ValueRepr) -> Result<bool, Diagnostic> {
        self.validate_value(value)?;
        if value.kind != 8 {
            return Ok(true);
        }
        match &self
            .objects
            .get(ObjectId::from_bits(value.payload))?
            .storage
        {
            ObjectStorage::String(_) => Ok(true),
            ObjectStorage::Aggregate(storage) => Ok(storage.initialized.complete_value()),
            ObjectStorage::Constructing => Err(ownership()),
        }
    }
    fn require_initialized_value(&self, value: ValueRepr) -> Result<(), Diagnostic> {
        if !self.value_is_initialized(value)? {
            return Err(Diagnostic::new(
                "LRR-E-USE-AFTER-MOVE",
                "whole owning value has an uninitialized or moved field".into(),
            ));
        }
        Ok(())
    }
    fn cell_is_initialized(&self, id: CellId) -> Result<bool, Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Ok(false);
        }
        match cell.state {
            CellState::Live(value) => self.value_is_initialized(value),
            _ => Ok(false),
        }
    }
    /// Check one owning-parent edge using logical identities only. Partial
    /// parents stay Live; their cached completeness is independent of leases.
    fn initialization_parent(
        &self,
        id: CellId,
    ) -> Result<Option<(ObjectId, usize, Option<CellId>)>, Diagnostic> {
        match self.cells.get(id)?.owner {
            CellOwner::FrameSlot { frame, .. } => {
                self.frames.get(frame)?;
                Ok(None)
            }
            CellOwner::Transfer(ticket) => {
                self.tickets.get(ticket)?;
                Ok(None)
            }
            CellOwner::Buffer { .. } => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "buffer initialization propagation requires its buffer integration".into(),
            )),
            CellOwner::Field { object, index } => {
                let record = self.objects.get(object)?;
                let ObjectStorage::Aggregate(storage) = &record.storage else {
                    return Err(ownership());
                };
                let index = index as usize;
                if storage.fields.get(index) != Some(&id) {
                    return Err(ownership());
                }
                storage.initialized.field(index)?;
                let parent = match record.owner {
                    Some(ValueOwner::Cell(parent)) => {
                        let value = self.admit_live_value(parent)?;
                        if value.kind != 8 || value.payload != object.bits() {
                            return Err(ownership());
                        }
                        Some(parent)
                    }
                    Some(ValueOwner::Ticket(ticket)) => {
                        let value = self.peek_ticket(ticket)?;
                        if value.kind != 8 || value.payload != object.bits() {
                            return Err(ownership());
                        }
                        None
                    }
                    Some(ValueOwner::Construction { guard, field }) => {
                        let value = self
                            .constructions
                            .get(guard)?
                            .fields
                            .get(field as usize)
                            .copied()
                            .flatten()
                            .ok_or_else(ownership)?;
                        if value.kind != 8 || value.payload != object.bits() {
                            return Err(ownership());
                        }
                        None
                    }
                    None => return Err(ownership()),
                };
                Ok(Some((object, index, parent)))
            }
        }
    }
    /// Validate every update before changing a cell. No allocation, field-tree
    /// scan, Copy/Drop selection, or guest callback is needed for this chain.
    fn prepare_initialization_change(
        &self,
        id: CellId,
        initialized: bool,
    ) -> Result<(), Diagnostic> {
        let mut current = id;
        let mut initialized = initialized;
        let mut slow = Some(id);
        let mut fast = Some(id);
        // Floyd's slow/fast walk below rejects cycles; no depth ceiling is needed.
        while let Some((object, index, parent)) = self.initialization_parent(current)? {
            let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
                return Err(ownership());
            };
            if storage.initialized.field(index)? != self.cell_is_initialized(current)? {
                return Err(ownership());
            }
            initialized = storage.initialized.preview(index, initialized)?;
            let Some(parent) = parent else {
                break;
            };
            current = parent;
            slow = match slow {
                Some(cell) => self.initialization_parent(cell)?.and_then(|link| link.2),
                None => None,
            };
            for _ in 0..2 {
                fast = match fast {
                    Some(cell) => self.initialization_parent(cell)?.and_then(|link| link.2),
                    None => None,
                };
            }
            if slow.is_some() && slow == fast {
                return Err(ownership());
            }
        }
        Ok(())
    }
    /// The prevalidated chain is unchanged by ticket allocation or the leaf
    /// transaction. Each parent summary is updated before visiting its parent.
    fn commit_initialization_change(
        &mut self,
        id: CellId,
        initialized: bool,
    ) -> Result<(), Diagnostic> {
        let mut current = id;
        let mut initialized = initialized;
        while let Some((object, index, parent)) = self.initialization_parent(current)? {
            let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(object)?.storage
            else {
                return Err(ownership());
            };
            storage.initialized.set(index, initialized)?;
            initialized = storage.initialized.complete_value();
            let Some(parent) = parent else {
                break;
            };
            current = parent;
        }
        Ok(())
    }
    pub(crate) fn admit_copy(&self, id: CellId) -> Result<ValueRepr, Diagnostic> {
        let value = self.admit_move(id)?;
        if value.kind == 8
            || (value.kind == 9
                && self.references.get(RefId::from_bits(value.payload))?.access == Access::Mutable)
        {
            return Err(ownership());
        }
        Ok(value)
    }
    pub(crate) fn admit_new_contract(
        &self,
        expected: Option<TypeId>,
        value: ValueRepr,
    ) -> Result<(), Diagnostic> {
        self.validate_value(value)?;
        if expected.is_some() && self.effective_type(value)? != expected {
            return Err(ownership());
        }
        Ok(())
    }
    pub(crate) fn admit_replacement(&self, id: CellId, value: ValueRepr) -> Result<(), Diagnostic> {
        self.validate_value(value)?;
        self.require_initialized_value(value)?;
        let cell = self.cells.get(id)?;
        if cell.retired || !cell.writable {
            return Err(ownership());
        }
        let CellState::Live(old) = cell.state else {
            return Err(ownership());
        };
        self.object_owner(old, ValueOwner::Cell(id))?;
        self.contract(cell, value)?;
        self.prepare_initialization_change(id, true)?;
        Ok(())
    }
    pub(crate) fn admit_discard(&self, id: CellId) -> Result<(), Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        match cell.state {
            CellState::Live(_) => {
                let value = self.admit_live_value(id)?;
                if value.kind == 8 && !self.types.is_string(TypeId(value.type_id)) {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "aggregate cleanup requires a checked ordinary destructor plan".into(),
                    ));
                }
            }
            CellState::Uninit | CellState::Moved => {}
            _ => return Err(ownership()),
        };
        Ok(())
    }
    /// Pure admission for current closed intrinsic owning reads. A moved
    /// location gets the prescribed value-use diagnostic; lease resolution keeps
    /// its separate stale-handle diagnostic and cleanup retains its own states.
    fn admit_read_location(&self, id: CellId) -> Result<(), Diagnostic> {
        let cell = self.cells.get(id)?;
        if cell.retired {
            return Err(stale());
        }
        if cell.state == CellState::Moved {
            return Err(Diagnostic::new(
                "LRR-E-USE-AFTER-MOVE",
                "owning read uses a moved guest location".into(),
            ));
        }
        Ok(())
    }
    fn intrinsic_read_mode(&self, id: CellId) -> Result<IntrinsicRead, Diagnostic> {
        self.admit_read_location(id)?;
        let value = self.admit_move(id)?;
        // Unresolved numeric literal records are shared immutable identities
        // until existing checked materialization binds their numeric context.
        if matches!(value.kind, 11 | 12) {
            return Ok(IntrinsicRead::CopyScalar);
        }
        let descriptor = self.types.descriptor(TypeId(value.type_id))?;
        match descriptor.key.as_ref() {
            TypeKey::Primitive(_) => {
                self.admit_copy(id)?;
                Ok(IntrinsicRead::CopyScalar)
            }
            TypeKey::Ref { mutable, .. } => Ok(if *mutable {
                IntrinsicRead::Move
            } else {
                IntrinsicRead::CopyScalar
            }),
            TypeKey::Facade { .. } if self.types.is_string(TypeId(value.type_id)) => {
                Ok(IntrinsicRead::Move)
            }
            // A structural NotCopy certificate is final even when another cold
            // member's nominal policy is Deferred. Transfer the whole existing
            // object; never duplicate its payload or force sibling Drop/Copy.
            TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. }
                if descriptor.copy == super::shape::CopyStatus::Copy =>
            {
                Ok(IntrinsicRead::CopyAggregate)
            }
            TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. }
                if descriptor.copy == super::shape::CopyStatus::NotCopy =>
            {
                Ok(IntrinsicRead::Move)
            }
            // A structural aggregate with nominal members is Copy exactly when
            // its reached fields are.
            TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. }
                if self.user_copy_types.is_some() =>
            {
                Ok(if self.fields_copy(id)? {
                    IntrinsicRead::CopyAggregate
                } else {
                    IntrinsicRead::Move
                })
            }
            // A nominal type without a Copy impl is moved as one whole object;
            // one with a Copy impl is copied field by field.
            TypeKey::Nominal { identity, .. }
                if self
                    .user_copy_types
                    .as_ref()
                    .is_some_and(|types| !types.contains(identity)) =>
            {
                Ok(IntrinsicRead::Move)
            }
            // A bounded `impl<T: Copy> Copy for N<T>` applies exactly when the
            // value's fields are all Copy (rustc requires that of every Copy
            // type's fields); otherwise the value moves.
            TypeKey::Nominal { identity, .. } if self.nominal_copy(*identity) => {
                Ok(if self.fields_copy(id)? {
                    IntrinsicRead::CopyAggregate
                } else {
                    IntrinsicRead::Move
                })
            }
            _ => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "owning read requires checked aggregate Copy or selected source policy".into(),
            )),
        }
    }
    /// Whether every field reached from a Copy-impl nominal value is itself
    /// Copy: scalars and shared references are; mutable references, strings
    /// and nominal values without a Copy impl are not.
    pub(super) fn fields_copy(&self, root: CellId) -> Result<bool, Diagnostic> {
        let mut pending = vec![root];
        while let Some(cell) = pending.pop() {
            let value = self.admit_move(cell)?;
            match value.kind {
                1..=7 | 11 | 12 => {}
                9 => {
                    if self.references.get(RefId::from_bits(value.payload))?.access
                        == Access::Mutable
                    {
                        return Ok(false);
                    }
                }
                8 => {
                    let descriptor = self.types.descriptor(TypeId(value.type_id))?;
                    match descriptor.key.as_ref() {
                        TypeKey::Nominal { identity, .. } => {
                            if !self.nominal_copy(*identity) {
                                return Ok(false);
                            }
                        }
                        TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. } => {
                            match descriptor.copy {
                                super::shape::CopyStatus::Copy => continue,
                                super::shape::CopyStatus::NotCopy => return Ok(false),
                                super::shape::CopyStatus::Deferred => {}
                            }
                        }
                        _ => return Ok(false),
                    }
                    let (_, storage) = self.place_object(cell)?;
                    pending.extend(storage.fields.iter().copied());
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
    pub(crate) fn admit_owning_read(&self, id: CellId) -> Result<IntrinsicRead, Diagnostic> {
        let mode = self.intrinsic_read_mode(id)?;
        let cell = self.cells.get(id)?;
        if mode == IntrinsicRead::Move
            && !matches!(
                cell.owner,
                CellOwner::FrameSlot { .. } | CellOwner::Transfer(_)
            )
        {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "field/buffer owning extraction requires its checked source move plan".into(),
            ));
        }
        Ok(mode)
    }
    /// Reserve the complete result cell/ticket batch before source ownership is
    /// consumed. A numeric materialization uses a second transfer ticket.
    pub(crate) fn reserve_owning_read_result(
        &mut self,
        needs_cell: bool,
        materializes_literal: bool,
    ) -> Result<(), Diagnostic> {
        let tickets = 1 + usize::from(materializes_literal);
        if needs_cell {
            self.cells.reserve(1, &mut self.budget)?;
        }
        self.tickets.reserve(tickets, &mut self.budget)?;
        Ok(())
    }
    /// Admit a pending pattern's whole cell/move-ticket batch against the u32
    /// handle index space (Handle encoding) before any move.
    pub(crate) fn preflight_pattern_bindings(&self, count: usize) -> Result<(), Diagnostic> {
        self.cells.admits(count)?;
        self.tickets.admits(count)
    }
    pub(crate) fn reserve_pattern_bindings(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.preflight_pattern_bindings(count)?;
        self.cells.reserve(count, &mut self.budget)?;
        self.tickets.reserve(count, &mut self.budget)?;
        Ok(())
    }
    /// Local assignment consumes an RHS ticket, optionally creates an old-value
    /// ticket, and publishes Unit. Handles are monotonic for a run, so consuming
    /// tickets does not replenish this batch's handle credit.
    pub(crate) fn reserve_local_assignment_result(
        &mut self,
        needs_cell: bool,
        replaces_live: bool,
    ) -> Result<(), Diagnostic> {
        let tickets = 2 + usize::from(replaces_live);
        if needs_cell {
            self.cells.reserve(1, &mut self.budget)?;
        }
        self.tickets.reserve(tickets, &mut self.budget)?;
        Ok(())
    }
    /// Bounded replacement consumes incoming/old tickets at commit and retains
    /// a Unit result plus the later root Drop ticket. One detached old-value
    /// cleanup cell and an optional result cell complete the reservation batch.
    /// Reserve before consuming either original owner.
    pub(crate) fn reserve_bounded_local_replacement(
        &mut self,
        needs_result_cell: bool,
    ) -> Result<(), Diagnostic> {
        let cells = 1 + usize::from(needs_result_cell);
        self.cells.reserve(cells, &mut self.budget)?;
        self.tickets.reserve(4, &mut self.budget)?;
        Ok(())
    }
    pub(crate) fn read_owning(&mut self, id: CellId) -> Result<TicketId, Diagnostic> {
        match self.admit_owning_read(id)? {
            IntrinsicRead::CopyScalar => self.copy_from(id),
            IntrinsicRead::CopyAggregate => Err(aggregate_copy::requires_activation()),
            IntrinsicRead::Move => self.move_from(id),
        }
    }
    pub fn copy_from(&mut self, id: CellId) -> Result<TicketId, Diagnostic> {
        let value = self.admit_copy(id)?;
        self.ticket(value)
    }
    pub fn move_from(&mut self, id: CellId) -> Result<TicketId, Diagnostic> {
        self.extract_from(id, false)
    }
    fn extract_from(&mut self, id: CellId, cleanup: bool) -> Result<TicketId, Diagnostic> {
        let value = if cleanup {
            self.admit_live_value(id)?
        } else {
            self.admit_move(id)?
        };
        self.prepare_initialization_change(id, false)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        let cell = self.cells.get_mut(id)?;
        cell.state = CellState::Moved;
        if let Some(epoch) = cell.location_epoch.checked_add(1) {
            cell.location_epoch = epoch;
        } else {
            cell.retired = true;
        }
        self.commit_initialization_change(id, false)?;
        Ok(ticket)
    }
    pub(crate) fn seal_arguments(
        &mut self,
        cells: &[CellId],
        types: &[TypeId],
    ) -> Result<ArgumentBatchId, Diagnostic> {
        if cells.len() != types.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "evaluated argument count does not match the prepared boundary".into(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for cell in cells {
            if !seen.insert(*cell) {
                return Err(ownership());
            }
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(cells.len())
            .map_err(|_| resource())?;
        for (cell, expected) in cells.iter().zip(types) {
            let value = self.admit_move(*cell)?;
            self.prepare_initialization_change(*cell, false)?;
            // An owning argument and one of its fields cannot enter the same
            // packet: both are live during admission, but the first transfer
            // would invalidate the other's complete-value contract.
            let mut ancestor = *cell;
            while let Some((_, _, Some(parent))) = self.initialization_parent(ancestor)? {
                if seen.contains(&parent) {
                    return Err(ownership());
                }
                ancestor = parent;
            }
            if self.effective_type(value)?.is_none() {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "call argument has no reached concrete descriptor".into(),
                ));
            }
            // A call argument is a coercion site: an array reference meets a
            // slice-reference parameter through the shared unsizing operation.
            let unsize = match self.admit_new_contract(Some(*expected), value) {
                Ok(()) => false,
                Err(error) if error.code == "LRR-E-OWNERSHIP" => {
                    if self.unsizing_coercion(value, *expected)?.is_none() {
                        return Err(Diagnostic::new(
                            "LRR-E-CALL-ARG-TYPE",
                            "evaluated argument descriptor does not match the prepared boundary"
                                .into(),
                        ));
                    }
                    true
                }
                Err(error) => return Err(error),
            };
            values.push((value, unsize.then_some(*expected)));
        }
        let mut tickets = Vec::new();
        tickets
            .try_reserve_exact(cells.len())
            .map_err(|_| resource())?;
        let mut contracts = Vec::new();
        contracts
            .try_reserve_exact(types.len())
            .map_err(|_| resource())?;
        contracts.extend_from_slice(types);
        let owned = tickets
            .capacity()
            .checked_mul(std::mem::size_of::<TicketId>())
            .and_then(|n| {
                contracts
                    .capacity()
                    .checked_mul(std::mem::size_of::<TypeId>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(resource)?;
        self.tickets.reserve(cells.len(), &mut self.budget)?;
        self.argument_batches.reserve(1, &mut self.budget)?;
        // No guest mutation until all storage and handle allowances are reserved.
        let mut sealed = Vec::new();
        sealed
            .try_reserve_exact(values.len())
            .map_err(|_| resource())?;
        for (value, unsize) in values {
            let value = match unsize {
                Some(expected) => self.coerce_unsized_admitted(value, expected)?,
                None => value,
            };
            sealed.push(value);
            tickets.push(
                self.tickets
                    .allocate(TransferTicket { value }, 0, &mut self.budget)?
                    .into(),
            );
        }
        let values = sealed;
        let batch = self.argument_batches.allocate(
            ArgumentBatch {
                tickets,
                types: contracts,
            },
            owned,
            &mut self.budget,
        )?;
        for (index, (source, value)) in cells.iter().zip(values).enumerate() {
            let ticket = self.argument_batches.get(batch)?.tickets[index];
            self.set_owner(value, ValueOwner::Ticket(ticket))?;
            let cell = self.cells.get_mut(*source)?;
            cell.state = CellState::Moved;
            if let Some(epoch) = cell.location_epoch.checked_add(1) {
                cell.location_epoch = epoch;
            } else {
                cell.retired = true;
            }
            self.commit_initialization_change(*source, false)?;
        }
        Ok(batch.into())
    }
    pub(crate) fn deliver_arguments(
        &mut self,
        batch: ArgumentBatchId,
        destinations: &[CellId],
    ) -> Result<(), Diagnostic> {
        let record = self.argument_batches.get(batch)?;
        if destinations.len() != record.tickets.len() {
            return Err(Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "callee argument layout does not match its sealed packet".into(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for ((destination, ticket), expected) in
            destinations.iter().zip(&record.tickets).zip(&record.types)
        {
            if !seen.insert(*destination) {
                return Err(ownership());
            }
            let value = self.peek_ticket(*ticket)?;
            self.object_owner(value, ValueOwner::Ticket(*ticket))?;
            self.admit_new_contract(Some(*expected), value)?;
            self.validate_initialization_value(*destination, value)?;
        }
        let record = self.argument_batches.take(batch, &mut self.budget)?;
        for (destination, ticket) in destinations.iter().zip(record.tickets) {
            self.initialize(*destination, ticket)?;
        }
        Ok(())
    }
    /// A closed facade intrinsic consumes its sealed packet itself; the
    /// caller owns every returned ticket and must consume or discard it.
    pub(crate) fn take_arguments(
        &mut self,
        batch: ArgumentBatchId,
    ) -> Result<Vec<TicketId>, Diagnostic> {
        let record = self.argument_batches.get(batch)?;
        for ticket in &record.tickets {
            let value = self.peek_ticket(*ticket)?;
            self.object_owner(value, ValueOwner::Ticket(*ticket))?;
        }
        let record = self.argument_batches.take(batch, &mut self.budget)?;
        Ok(record.tickets.into_iter().collect())
    }
    pub(crate) fn discard_arguments(&mut self, batch: ArgumentBatchId) -> Result<(), Diagnostic> {
        let record = self.argument_batches.get(batch)?;
        for ticket in &record.tickets {
            let value = self.peek_ticket(*ticket)?;
            self.object_owner(value, ValueOwner::Ticket(*ticket))?;
        }
        let record = self.argument_batches.take(batch, &mut self.budget)?;
        for ticket in record.tickets {
            self.discard(ticket)?;
        }
        Ok(())
    }
    pub fn replace(&mut self, id: CellId, incoming: TicketId) -> Result<TicketId, Diagnostic> {
        let value = self.peek_ticket(incoming)?;
        self.object_owner(value, ValueOwner::Ticket(incoming))?;
        self.admit_replacement(id, value)?;
        let cell = self.cells.get(id)?;
        let CellState::Live(old) = cell.state else {
            return Err(ownership());
        };
        let declared = self.contract(cell, value)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let old_ticket = self
            .tickets
            .allocate(TransferTicket { value: old }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.tickets.take(incoming, &mut self.budget)?;
        self.set_owner(old, ValueOwner::Ticket(old_ticket))?;
        self.set_owner(value, ValueOwner::Cell(id))?;
        let cell = self.cells.get_mut(id)?;
        cell.state = CellState::Live(value);
        cell.declared = declared;
        cell.numeric_record = if value.kind == 11 || value.kind == 12 {
            Some(LiteralId::from_bits(value.payload))
        } else {
            None
        };
        self.commit_initialization_change(id, true)?;
        Ok(old_ticket)
    }
    pub fn integer_literal(
        &mut self,
        negative: bool,
        magnitude: u128,
        spelling: String,
    ) -> Result<TicketId, Diagnostic> {
        self.literal(
            LiteralNumber::Integer {
                negative,
                magnitude,
            },
            spelling,
            11,
        )
    }
    pub fn float_literal(&mut self, spelling: String) -> Result<TicketId, Diagnostic> {
        self.literal(LiteralNumber::Float, spelling, 12)
    }
    fn literal(
        &mut self,
        number: LiteralNumber,
        spelling: String,
        kind: u32,
    ) -> Result<TicketId, Diagnostic> {
        self.literals.reserve(1, &mut self.budget)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let bytes = spelling.capacity();
        let literal = self.literals.allocate(
            LiteralRecord {
                number,
                spelling,
                binding: None,
            },
            bytes,
            &mut self.budget,
        )?;
        self.tickets
            .allocate(
                TransferTicket {
                    value: ValueRepr {
                        kind,
                        type_id: 0,
                        payload: literal.bits(),
                    },
                },
                0,
                &mut self.budget,
            )
            .map(TicketId::from)
    }
    pub(crate) fn preview_integer(
        negative: bool,
        magnitude: u128,
        ty: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        match super::shape::primitive(ty)? {
            super::shape::Primitive::Signed(width) => {
                let max = 1u128 << (width - 1);
                if negative {
                    if magnitude > max {
                        return Err(invalid());
                    }
                    ValueRepr::signed(
                        ty,
                        i64::try_from(-(magnitude as i128)).map_err(|_| invalid())?,
                    )
                } else {
                    ValueRepr::signed(ty, i64::try_from(magnitude).map_err(|_| invalid())?)
                }
            }
            super::shape::Primitive::Unsigned(_) => {
                // Folding a signed literal must preserve unary-negation admission:
                // unsigned operands cannot be negated, including zero. An explicit
                // cast of an already signed value uses the separate cast operation.
                if negative {
                    return Err(Diagnostic::new(
                        "LRR-E-OP-TYPE",
                        "unsigned integer cannot carry folded unary negation".into(),
                    ));
                }
                ValueRepr::unsigned(ty, u64::try_from(magnitude).map_err(|_| invalid())?)
            }
            _ => Err(invalid()),
        }
    }
    pub(crate) fn preview_float(decimal: &str, ty: TypeId) -> Result<ValueRepr, Diagnostic> {
        match ty.0 {
            14 => {
                let value = decimal.parse::<f32>().map_err(|_| invalid())?;
                if !value.is_finite() {
                    return Err(invalid());
                }
                Ok(ValueRepr::f32(value))
            }
            15 => {
                let value = decimal.parse::<f64>().map_err(|_| invalid())?;
                if !value.is_finite() {
                    return Err(invalid());
                }
                Ok(ValueRepr::f64(value))
            }
            _ => Err(invalid()),
        }
    }
    fn literal_value(&self, value: ValueRepr, ty: TypeId) -> Result<ValueRepr, Diagnostic> {
        let record = self.literals.get(LiteralId::from_bits(value.payload))?;
        match record.number {
            LiteralNumber::Integer {
                negative,
                magnitude,
            } => Self::preview_integer(negative, magnitude, ty),
            LiteralNumber::Float => Self::preview_float(&record.spelling, ty),
        }
    }
    /// A Copy read for closed formatting: an unbound numeric literal takes
    /// rustc's integer/float fallback type (i32 / f64).
    pub(crate) fn read_format_scalar(&self, id: CellId) -> Result<ValueRepr, Diagnostic> {
        let cell = self.cells.get(id)?;
        if let CellState::Live(value) = cell.state
            && (value.kind == 11 || value.kind == 12)
            && self
                .literals
                .get(LiteralId::from_bits(value.payload))?
                .binding
                .is_none()
        {
            self.object_owner(value, ValueOwner::Cell(id))?;
            return self.literal_value(value, TypeId(if value.kind == 11 { 6 } else { 15 }));
        }
        self.read_scalar(id)
    }
    fn bound_literal(&self, value: ValueRepr) -> Result<ValueRepr, Diagnostic> {
        let record = self.literals.get(LiteralId::from_bits(value.payload))?;
        let ty = record.binding.ok_or(Diagnostic::new(
            "LRR-E-TYPE-CONTEXT",
            "numeric literal needs an explicit suffix or reached type hint".into(),
        ))?;
        self.literal_value(value, ty)
    }
    /// Reserve the concrete result ticket before fixing a shared literal binding.
    pub fn materialize_ticket(
        &mut self,
        ticket: TicketId,
        ty: TypeId,
    ) -> Result<TicketId, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.validate_value(value)?;
        if value.kind != 11 && value.kind != 12 {
            return Err(invalid());
        }
        let literal = LiteralId::from_bits(value.payload);
        if self
            .literals
            .get(literal)?
            .binding
            .is_some_and(|bound| bound != ty)
        {
            return Err(ownership());
        }
        let concrete = self.literal_value(value, ty)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let result = self
            .tickets
            .allocate(TransferTicket { value: concrete }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.literals.get_mut(literal)?.binding = Some(ty);
        Ok(result)
    }
    /// Bind an unconstrained numeric literal held in `id` to rustc's
    /// fallback type (`i32` / `f64`); a bound literal or non-literal is
    /// unchanged. Used where inference leaves a literal with no context.
    pub(crate) fn bind_literal_fallback(&mut self, id: CellId) -> Result<(), Diagnostic> {
        let CellState::Live(value) = self.cells.get(id)?.state else {
            return Ok(());
        };
        if value.kind != 11 && value.kind != 12 {
            return Ok(());
        }
        let literal = LiteralId::from_bits(value.payload);
        let record = self.literals.get_mut(literal)?;
        if record.binding.is_none() {
            record.binding = Some(TypeId(if value.kind == 11 { 6 } else { 15 }));
        }
        Ok(())
    }
    pub fn materialize(&mut self, ticket: TicketId, ty: TypeId) -> Result<ValueRepr, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.validate_value(value)?;
        if value.kind != 11 && value.kind != 12 {
            return Err(invalid());
        }
        let literal = LiteralId::from_bits(value.payload);
        if self
            .literals
            .get(literal)?
            .binding
            .is_some_and(|bound| bound != ty)
        {
            return Err(ownership());
        }
        let concrete = self.literal_value(value, ty)?;
        self.literals.get_mut(literal)?.binding = Some(ty);
        Ok(concrete)
    }
    pub fn owner(&self, id: CellId) -> Result<CellOwner, Diagnostic> {
        Ok(self.cells.get(id)?.owner)
    }
    pub fn string_contents(&self, ticket: TicketId) -> Result<&str, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        let ObjectStorage::String(bytes) = &self
            .objects
            .get(ObjectId::from_bits(value.payload))?
            .storage
        else {
            return Err(invalid());
        };
        std::str::from_utf8(bytes).map_err(|_| invalid())
    }
    pub fn discard(&mut self, ticket: TicketId) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind == 8 {
            let object = self.objects.get(ObjectId::from_bits(value.payload))?;
            if let ObjectStorage::Aggregate(storage) = &object.storage
                && (storage.next_cleanup != storage.fields.len()
                    || storage.pending_cleanup.is_some())
            {
                return Err(ownership());
            }
        }
        self.tickets.take(ticket, &mut self.budget)?;
        if value.kind == 8 {
            self.objects
                .take(ObjectId::from_bits(value.payload), &mut self.budget)?;
        }
        if value.kind == 9
            && self.references.get(RefId::from_bits(value.payload))?.access == Access::Mutable
        {
            // Dropping the reference destroys only its lease, never the referent.
            self.references
                .take(RefId::from_bits(value.payload), &mut self.budget)?;
        }
        Ok(())
    }
    pub fn begin_drop(&mut self, id: CellId) -> Result<TicketId, Diagnostic> {
        let ticket = self.extract_from(id, true)?;
        self.cells.get_mut(id)?.state = CellState::Dropping;
        Ok(ticket)
    }
    pub fn retire_cell(&mut self, id: CellId) -> Result<(), Diagnostic> {
        self.prepare_initialization_change(id, false)?;
        let cell = self.cells.get_mut(id)?;
        if matches!(cell.state, CellState::Live(_)) {
            return Err(ownership());
        }
        cell.state = CellState::Dropped;
        cell.retired = true;
        cell.location_epoch = cell.location_epoch.saturating_add(1);
        self.commit_initialization_change(id, false)?;
        Ok(())
    }
    pub fn replacement_location(&mut self, id: CellId) -> Result<CellId, Diagnostic> {
        let cell = self.cells.get(id)?;
        if !cell.retired || matches!(cell.state, CellState::Live(_)) {
            return Err(ownership());
        }
        let declared = cell.declared;
        let writable = cell.writable;
        let owner = cell.owner;
        let field = match owner {
            CellOwner::Field { object, index } => {
                let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
                    return Err(ownership());
                };
                if storage.next_cleanup != 0 || storage.fields.get(index as usize) != Some(&id) {
                    return Err(ownership());
                }
                Some((object, index))
            }
            CellOwner::Buffer { .. } => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "buffer cell replacement requires its buffer lease integration".into(),
                ));
            }
            _ => None,
        };
        // Allocate before publishing a new field identity; failures keep the
        // retired location installed and cannot resurrect an older lease.
        let replacement = self.new_cell(declared, writable, owner)?;
        if let Some((object, index)) = field {
            let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(object)?.storage
            else {
                return Err(ownership());
            };
            storage.fields[index as usize] = replacement;
        }
        Ok(replacement)
    }
    pub fn reset(&mut self, new_epoch: u32) -> Result<(), Diagnostic> {
        if new_epoch <= self.budget.epoch() {
            return Err(stale());
        }
        self.cells.reset(new_epoch)?;
        self.objects.reset(new_epoch)?;
        self.tickets.reset(new_epoch)?;
        self.argument_batches.reset(new_epoch)?;
        self.literals.reset(new_epoch)?;
        self.frames.reset(new_epoch)?;
        self.constructions.reset(new_epoch)?;
        self.references.reset(new_epoch)?;
        self.budget = RunBudget::for_epoch(new_epoch)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "cell_folded_negative_unsigned_tests.rs"]
mod folded_negative_unsigned_tests;

#[cfg(test)]
#[path = "cell_initialization_tests.rs"]
mod initialization_tests;

#[cfg(test)]
#[path = "cell_structural_move_tests.rs"]
mod structural_move_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    const STOCK_STRING_MOVE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/use/use-after-move-based-on-type.rs"
    );
    pub(crate) fn stock_drop_record_storage() -> (GuestStore, TicketId, Vec<CellId>, Vec<String>) {
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/struct-field-drop-order.rs"
        );
        let members = crate::syntax::scope_fixture_facts::original_no_drop_record_members(original);
        assert!(!members.is_empty());
        let mut store = GuestStore::for_epoch(1).unwrap();
        let first = store.string(&members[0]).unwrap();
        let member_type = TypeId(store.peek_ticket(first).unwrap().type_id);
        let types = vec![member_type; members.len()];
        let tuple_type = store
            .intern_type(TypeKey::Tuple(types.clone().into_boxed_slice()))
            .unwrap();
        let guard = store.begin_construction(&types).unwrap();
        store.construction_push(guard, 0, first).unwrap();
        for (ordinal, member) in members.iter().enumerate().skip(1) {
            let ticket = store.string(member).unwrap();
            store
                .construction_push(guard, u32::try_from(ordinal).unwrap(), ticket)
                .unwrap();
        }
        let root = store.commit_construction(guard, tuple_type).unwrap();
        let original_locations: Vec<CellId> = (0..members.len())
            .map(|ordinal| {
                store
                    .aggregate_field(root, u32::try_from(ordinal).unwrap())
                    .unwrap()
            })
            .collect();
        (store, root, original_locations, members)
    }
    #[test]
    fn rustc_record_cleanup_retains_each_dropping_location_until_owned_completion() {
        let (mut store, root, original_locations, members) = stock_drop_record_storage();
        for (ordinal, member) in members.iter().enumerate() {
            let original_location = original_locations[ordinal];
            let AggregateCleanupTransfer::Field { cell, ticket } =
                store.aggregate_cleanup_transfer(root).unwrap()
            else {
                panic!("live stock-derived member was skipped")
            };
            assert_eq!(cell, original_location);
            assert_eq!(store.state(cell).unwrap(), CellState::Dropping);
            assert_eq!(store.string_contents(ticket).unwrap(), member);
            assert_eq!(store.begin_drop(cell).unwrap_err().code, "LRR-E-OWNERSHIP");
            assert!(
                matches!(store.aggregate_cleanup_transfer(root), Err(error) if error.code == "LRR-E-OWNERSHIP")
            );
            assert_eq!(store.discard(root).unwrap_err().code, "LRR-E-OWNERSHIP");
            assert_eq!(
                store
                    .finish_aggregate_cleanup_field(root, cell, ticket)
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            // Model a suspended dispatcher: owned ticket and location survive
            // arbitrary read-only inspection before the child continuation.
            assert_eq!(store.state(original_location).unwrap(), CellState::Dropping);
            assert_eq!(
                store
                    .aggregate_field(root, u32::try_from(ordinal).unwrap())
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            assert_eq!(store.string_contents(ticket).unwrap(), member);
            store.discard(ticket).unwrap();
            store
                .finish_aggregate_cleanup_field(root, cell, ticket)
                .unwrap();
            assert_eq!(
                store
                    .finish_aggregate_cleanup_field(root, cell, ticket)
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            assert_eq!(store.state(cell).unwrap(), CellState::Dropped);
            assert_eq!(
                store.peek_ticket(ticket).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
        }
        assert!(matches!(
            store.aggregate_cleanup_transfer(root).unwrap(),
            AggregateCleanupTransfer::Complete
        ));
        store.discard(root).unwrap();
        assert_eq!(
            store.peek_ticket(root).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        // Intrinsic ownership continuation only; no original guest destructor,
        // Rc/Cell state, println or complete rustc run-pass is exercised here.
    }

    fn stock_string_locations() -> (GuestStore, CellId, CellId, String) {
        let (text, source_writable, target_writable) =
            crate::syntax::place_fixture_facts::string_local_move(STOCK_STRING_MOVE);
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        // Disclosed component setup supplies original to_string's result;
        // this does not execute that method, println, or a whole stock fixture.
        let ticket = store.string(&text).unwrap();
        let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
        let source = store
            .new_cell(
                StorageContract::Exact(ty),
                source_writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let destination = store
            .new_cell(
                StorageContract::Exact(ty),
                target_writable,
                CellOwner::FrameSlot { frame, slot: 1 },
            )
            .unwrap();
        store.initialize(source, ticket).unwrap();
        (store, source, destination, text)
    }
    #[test]
    fn rustc_string_local_move_transfers_one_owner_then_reports_use_after_move() {
        let (mut store, source, destination, text) = stock_string_locations();
        let before = store.state(source).unwrap();
        let epoch = store.location_epoch(source).unwrap();
        assert_eq!(
            store.admit_owning_read(source).unwrap(),
            IntrinsicRead::Move
        );
        let ticket = store.read_owning(source).unwrap();
        assert_eq!(store.string_contents(ticket).unwrap(), text);
        store.initialize(destination, ticket).unwrap();
        assert_eq!(store.state(destination).unwrap(), before);
        assert_eq!(store.state(source).unwrap(), CellState::Moved);
        assert_eq!(store.location_epoch(source).unwrap(), epoch + 1);
        let handles = store.budget().handles();
        assert_eq!(
            store.admit_owning_read(source).unwrap_err().code,
            "LRR-E-USE-AFTER-MOVE"
        );
        assert_eq!(
            store.read_owning(source).unwrap_err().code,
            "LRR-E-USE-AFTER-MOVE"
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(store.state(destination).unwrap(), before);
        let CellState::Live(value) = before else {
            panic!("original owner absent")
        };
        assert_eq!(
            store
                .objects
                .get(ObjectId::from_bits(value.payload))
                .unwrap()
                .owner,
            Some(ValueOwner::Cell(destination))
        );
        // Fatal guest-run behavior/Drop remains later T045: this store-level
        // refusal retains the owner; reset physically reclaims without Drop.
        store.reset(2).unwrap();
        assert_eq!(
            store.state(destination).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }
    #[test]
    fn rustc_string_move_single_owner_property_rejects_the_bitcopy_model() {
        let (mut store, source, destination, _) = stock_string_locations();
        let frame = store.new_frame_anchor().unwrap();
        let CellState::Live(value) = store.state(source).unwrap() else {
            panic!("source owner absent")
        };
        let mut cells = vec![source, destination];
        let mut current = source;
        // Supplemental LRR transfer chain: original String payload, then
        // fresh owning locations. Test both ticket and cell publication states.
        for step in 0..32_u32 {
            let bogus_live_values = [value, value];
            let mut owners = std::collections::BTreeSet::new();
            assert!(!bogus_live_values.iter().all(|v| owners.insert(v.payload)));
            let handles = store.budget().handles();
            assert_eq!(store.ticket(value).unwrap_err().code, "LRR-E-OWNERSHIP");
            assert_eq!(store.budget().handles(), handles);
            let ticket = store.read_owning(current).unwrap();
            assert_eq!(store.ticket(value).unwrap_err().code, "LRR-E-OWNERSHIP");
            let next = if step == 0 {
                destination
            } else {
                let cell = store
                    .new_cell(
                        StorageContract::Exact(TypeId(value.type_id)),
                        false,
                        CellOwner::FrameSlot { frame, slot: step },
                    )
                    .unwrap();
                cells.push(cell);
                cell
            };
            store.initialize(next, ticket).unwrap();
            assert_eq!(store.state(current).unwrap(), CellState::Moved);
            current = next;
            let live = cells
                .iter()
                .filter_map(|cell| match store.state(*cell).unwrap() {
                    CellState::Live(value) => Some(value),
                    CellState::Moved | CellState::Uninit => None,
                    _ => panic!("transfer chain started cleanup"),
                })
                .collect::<Vec<_>>();
            assert_eq!(live, vec![value]);
            assert_eq!(
                store
                    .objects
                    .get(ObjectId::from_bits(value.payload))
                    .unwrap()
                    .owner,
                Some(ValueOwner::Cell(current))
            );
            assert_eq!(store.ticket(value).unwrap_err().code, "LRR-E-OWNERSHIP");
        }
        // No ordinary cleanup is claimed: a fatal run physically reclaims.
        store.reset(2).unwrap();
    }
    const STOCK_TUPLE_REFERENCE_MOVE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/tuple-field-move-with-drop-borrow.rs"
    );
    pub(crate) fn stock_tuple_reference_owner() -> (GuestStore, CellId, CellId, CellId, RefId) {
        use crate::frontend::types::NominalBinding;
        let (binding, _) = stock_variant_binding(
            STOCK_TUPLE_REFERENCE_MOVE,
            "tuple-field-move-with-drop-borrow.rs",
            "WithDrop",
            None,
        );
        let NominalBinding::Ready(bound) = binding else {
            panic!("original unit-like WithDrop schema did not bind")
        };
        let selector = crate::syntax::place_fixture_facts::tuple_reference_move_selector(
            STOCK_TUPLE_REFERENCE_MOVE,
        );
        assert_eq!(selector, 0);
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let referent = store
            .new_cell(
                StorageContract::Exact(TypeId(1)),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let unit = store.ticket(ValueRepr::unit()).unwrap();
        store.initialize(referent, unit).unwrap();
        let lease = store
            .borrow(&Place::cell(referent), Access::Mutable)
            .unwrap();
        let reference = store.reference_ticket(lease).unwrap();
        let reference_ty = TypeId(store.peek_ticket(reference).unwrap().type_id);
        // The sibling has the exact source-bound WithDrop identity, but its
        // actual destructor remains unselected. Tuple extraction must not force
        // that cold sibling's policy or pretend the tuple owns its destructor.
        let (sibling_ty, sibling_guard) = store.begin_nominal_construction(&bound).unwrap();
        let sibling = store
            .commit_construction(sibling_guard, sibling_ty)
            .unwrap();
        let tuple_ty = store
            .intern_type(TypeKey::Tuple(Box::new([reference_ty, sibling_ty])))
            .unwrap();
        let guard = store
            .begin_construction(&[reference_ty, sibling_ty])
            .unwrap();
        store.construction_push(guard, 0, reference).unwrap();
        store.construction_push(guard, 1, sibling).unwrap();
        let tuple = store.commit_construction(guard, tuple_ty).unwrap();
        let field = store.aggregate_field(tuple, selector).unwrap();
        let sibling = store.aggregate_field(tuple, 1).unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(tuple_ty),
                false,
                CellOwner::FrameSlot { frame, slot: 1 },
            )
            .unwrap();
        store.initialize(root, tuple).unwrap();
        (store, root, field, sibling, lease)
    }
    #[test]
    fn rustc_tuple_reference_field_move_preserves_cold_drop_sibling_and_referent() {
        let (mut store, root, field, sibling, lease) = stock_tuple_reference_owner();
        let epoch = store.location_epoch(root).unwrap();
        let sibling_state = store.state(sibling).unwrap();
        let place = store.project_tuple(&Place::cell(root), 0).unwrap();
        let (_, value, mode) = store.admit_owning_place_read(&place).unwrap();
        assert_eq!(mode, IntrinsicRead::Move);
        let ticket = store.read_owning_place(&place).unwrap();
        assert_eq!(store.peek_ticket(ticket).unwrap(), value);
        assert_eq!(store.state(field).unwrap(), CellState::Moved);
        assert_eq!(store.location_epoch(root).unwrap(), epoch);
        assert_eq!(store.state(sibling).unwrap(), sibling_state);
        assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
        assert_eq!(
            place_runtime_fault(store.admit_owning_place_read(&place).unwrap_err()).code,
            "LRR-E-USE-AFTER-MOVE"
        );
        assert_eq!(
            store.admit_owning_read(root).unwrap_err().code,
            "LRR-E-USE-AFTER-MOVE"
        );
        // No guest return or Drop transcript is claimed by this direct-place
        // adaptation of the original consume body. Fatal teardown is physical.
        store.reset(2).unwrap();
    }
    #[test]
    fn rustc_tuple_reference_field_move_rejects_deref_extraction_before_owner_change() {
        let (mut store, root, field, sibling, _) = stock_tuple_reference_owner();
        let alias = store.borrow(&Place::cell(root), Access::Shared).unwrap();
        let place = store.project_tuple(&Place::deref(alias), 0).unwrap();
        let before = (
            store.state(field).unwrap(),
            store.state(sibling).unwrap(),
            store.location_epoch(root).unwrap(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        assert_eq!(
            place_runtime_fault(store.read_owning_place(&place).unwrap_err()).code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(
            before,
            (
                store.state(field).unwrap(),
                store.state(sibling).unwrap(),
                store.location_epoch(root).unwrap(),
                store.budget().handles(),
                store.budget().bytes()
            )
        );
    }
    #[test]
    fn rustc_tuple_reference_field_move_rechecks_direct_child_owning_ancestry() {
        let (mut store, root, field, sibling, lease) = stock_tuple_reference_owner();
        // A direct child CellId must reconstruct every enclosing owner, just
        // like an ordinary projected place; immutable bindings allow moves.
        let ticket = store.read_owning_place(&Place::cell(field)).unwrap();
        assert_eq!(store.peek_ticket(ticket).unwrap().kind, 9);
        assert_eq!(store.state(field).unwrap(), CellState::Moved);
        assert!(matches!(store.state(root).unwrap(), CellState::Live(_)));
        assert!(matches!(store.state(sibling).unwrap(), CellState::Live(_)));
        assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
    }
    #[test]
    fn rustc_scalar_copy_owning_reads_preserve_original_location_and_epoch() {
        let source =
            include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/moves/move-arg.rs");
        let value = ValueRepr::signed(
            TypeId(8),
            crate::syntax::place_fixture_facts::scalar_copy_binding(source),
        )
        .unwrap();
        // The selected stock test parameter explicitly supplies isize context;
        // calls and the whole original run-pass remain separate observations.
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(TypeId(8)),
                false,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let ticket = store.ticket(value).unwrap();
        store.initialize(cell, ticket).unwrap();
        let epoch = store.location_epoch(cell).unwrap();
        for _ in 0..3 {
            assert_eq!(
                store.admit_owning_read(cell).unwrap(),
                IntrinsicRead::CopyScalar
            );
            let copy = store.read_owning(cell).unwrap();
            assert_eq!(store.peek_ticket(copy).unwrap(), value);
            assert_eq!(store.state(cell).unwrap(), CellState::Live(value));
            assert_eq!(store.location_epoch(cell).unwrap(), epoch);
            store.discard(copy).unwrap();
        }
    }
    fn place_runtime_fault(failure: PlaceFailure) -> Diagnostic {
        let PlaceFailure::RuntimeFault(error) = failure else {
            panic!("expected a runtime fault")
        };
        error
    }
    const STOCK_MUT_ALIASES: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/mut-borrow-outside-loop.rs"
    );
    const STOCK_IMMUTABLE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-mut-addr-of-imm-var.rs"
    );
    #[test]
    fn rustc_mutable_reference_binding_moves_its_lease_and_shared_reborrow_copies() {
        use crate::syntax::place_fixture_facts::{self as facts, InitializerFacts};
        let binding = facts::binding(STOCK_MUT_ALIASES, "first");
        let InitializerFacts::Borrow(borrow) = binding.initializer else {
            panic!("original mutable borrow absent")
        };
        assert!(borrow.mutable && !borrow.raw);
        assert_eq!(borrow.target, "void");
        // Transport exercises the original immutable binding of a mutable lease.
        // Move/copy/guard/call admission injections below are supplemental LRR
        // checks; the original Fake calls and loop are not executed.
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let target = store
            .new_cell(
                StorageContract::Exact(TypeId(1)),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let value = store.ticket(ValueRepr::unit()).unwrap();
        store.initialize(target, value).unwrap();
        let lease = store.borrow(&Place::cell(target), Access::Mutable).unwrap();
        let ticket = store.reference_ticket(lease).unwrap();
        let repr = store.peek_ticket(ticket).unwrap();
        assert_eq!(repr.kind, 9);
        let mutable_ty = TypeId(repr.type_id);
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            store.reference_ticket(lease).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(store.ticket(repr).unwrap_err().code, "LRR-E-OWNERSHIP");
        assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
        assert_eq!(store.peek_ticket(ticket).unwrap(), repr);
        let first = store
            .new_cell(
                StorageContract::Exact(mutable_ty),
                binding.writable,
                CellOwner::FrameSlot { frame, slot: 1 },
            )
            .unwrap();
        store.initialize(first, ticket).unwrap();
        assert_eq!(store.reference_from_cell(first).unwrap(), lease);
        let before = (
            store.state(first).unwrap(),
            store.location_epoch(first).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(store.copy_from(first).unwrap_err().code, "LRR-E-OWNERSHIP");
        assert_eq!(
            store.read_scalar(first).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            before,
            (
                store.state(first).unwrap(),
                store.location_epoch(first).unwrap(),
                store.budget().handles()
            )
        );
        // Immutable ref binding still grants mutation of the writable referent.
        let incoming = store.ticket(ValueRepr::unit()).unwrap();
        let old = store
            .replace_reference(store.reference_from_cell(first).unwrap(), incoming)
            .unwrap();
        store.discard(old).unwrap();
        let shared = store.borrow(&Place::deref(lease), Access::Shared).unwrap();
        let shared_ticket = store.reference_ticket(shared).unwrap();
        let shared_repr = store.peek_ticket(shared_ticket).unwrap();
        let shared_ty = TypeId(shared_repr.type_id);
        assert_ne!(mutable_ty, shared_ty);
        assert_eq!(
            store
                .ticket(ValueRepr {
                    type_id: shared_ty.0,
                    ..repr
                })
                .unwrap_err()
                .code,
            "LRR-E-VALUE-REPR"
        );
        assert_eq!(
            store
                .ticket(ValueRepr {
                    type_id: mutable_ty.0,
                    ..shared_repr
                })
                .unwrap_err()
                .code,
            "LRR-E-VALUE-REPR"
        );
        let wrong_target = store
            .types
            .intern(TypeKey::Ref {
                mutable: false,
                target: TypeId(8),
            })
            .unwrap();
        assert_eq!(
            store
                .ticket(ValueRepr {
                    type_id: wrong_target.0,
                    ..shared_repr
                })
                .unwrap_err()
                .code,
            "LRR-E-VALUE-REPR"
        );
        let shared_cell = store
            .new_cell(
                StorageContract::Exact(shared_ty),
                false,
                CellOwner::FrameSlot { frame, slot: 2 },
            )
            .unwrap();
        store.initialize(shared_cell, shared_ticket).unwrap();
        let shared_copy = store.copy_from(shared_cell).unwrap();
        assert_eq!(store.peek_ticket(shared_copy).unwrap(), shared_repr);
        store.discard(shared_copy).unwrap();
        assert_eq!(store.read_reference(shared).unwrap(), ValueRepr::unit());
        // A mutable reference itself may move without moving its referent.
        let moved = store.move_from(first).unwrap();
        assert_eq!(store.state(first).unwrap(), CellState::Moved);
        assert_eq!(
            store.reference_from_cell(first).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        let destination = store
            .new_cell(
                StorageContract::Exact(mutable_ty),
                false,
                CellOwner::FrameSlot { frame, slot: 3 },
            )
            .unwrap();
        store.initialize(destination, moved).unwrap();
        assert_eq!(store.reference_from_cell(destination).unwrap(), lease);
        assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
        // Contract refusal leaves every argument owner and epoch unchanged.
        let before = (
            store.state(destination).unwrap(),
            store.location_epoch(destination).unwrap(),
            store.budget().handles(),
        );
        assert_eq!(
            store
                .seal_arguments(&[destination], &[shared_ty])
                .unwrap_err()
                .code,
            "LRR-E-CALL-ARG-TYPE"
        );
        assert_eq!(
            before,
            (
                store.state(destination).unwrap(),
                store.location_epoch(destination).unwrap(),
                store.budget().handles()
            )
        );
        let batch = store.seal_arguments(&[destination], &[mutable_ty]).unwrap();
        let callee = store.new_frame_anchor().unwrap();
        let argument = store
            .new_cell(
                StorageContract::Exact(mutable_ty),
                false,
                CellOwner::FrameSlot {
                    frame: callee,
                    slot: 0,
                },
            )
            .unwrap();
        store.deliver_arguments(batch, &[argument]).unwrap();
        assert_eq!(store.reference_from_cell(argument).unwrap(), lease);
        assert_eq!(
            store.references.get(lease).unwrap().owner,
            Some(ValueOwner::Cell(argument))
        );
        // Owning aggregate field transfer must use the same single mutable owner.
        let tuple_ty = store
            .types
            .intern(TypeKey::Tuple(vec![mutable_ty].into_boxed_slice()))
            .unwrap();
        let guard = store.begin_construction(&[mutable_ty]).unwrap();
        store.construction_move_from(guard, 0, argument).unwrap();
        assert_eq!(
            store.references.get(lease).unwrap().owner,
            Some(ValueOwner::Construction { guard, field: 0 })
        );
        let tuple = store.commit_construction(guard, tuple_ty).unwrap();
        let field = store.aggregate_field(tuple, 0).unwrap();
        assert_eq!(
            store.references.get(lease).unwrap().owner,
            Some(ValueOwner::Cell(field))
        );
        assert_eq!(store.reference_from_cell(field).unwrap(), lease);
        assert_eq!(store.copy_from(field).unwrap_err().code, "LRR-E-OWNERSHIP");
        let cleanup = store.aggregate_cleanup_next(tuple).unwrap().unwrap();
        assert_eq!(
            store.references.get(lease).unwrap().owner,
            Some(ValueOwner::Ticket(cleanup))
        );
        store.discard(cleanup).unwrap();
        assert_eq!(
            store.read_reference(lease).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert!(store.aggregate_cleanup_next(tuple).unwrap().is_none());
        store.discard(tuple).unwrap();
        assert_eq!(store.read_scalar(target).unwrap(), ValueRepr::unit());
        assert_eq!(store.read_reference(shared).unwrap(), ValueRepr::unit());
        // Moving the referent invalidates the lease, but transporting an unused
        // shared reference is legal; its next dereference must fail precisely.
        let moved_target = store.move_from(target).unwrap();
        let stale_copy = store.copy_from(shared_cell).unwrap();
        assert_eq!(store.peek_ticket(stale_copy).unwrap(), shared_repr);
        store.discard(stale_copy).unwrap();
        assert_eq!(
            store.reference_from_cell(shared_cell).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            store.read_reference(shared).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        store.discard(moved_target).unwrap();
        let cleanup = store.move_from(shared_cell).unwrap();
        store.discard(cleanup).unwrap();
    }
    #[test]
    fn rustc_unit_mutable_alias_prefix_uses_two_checked_location_leases() {
        use crate::syntax::place_fixture_facts::{self as facts, InitializerFacts};
        let binding = facts::binding(STOCK_MUT_ALIASES, "void");
        assert!(matches!(binding.initializer, InitializerFacts::Unit(0)));
        let borrows = facts::borrows_of(STOCK_MUT_ALIASES, "void");
        assert_eq!(borrows.len(), 2);
        assert!(borrows.iter().all(|borrow| borrow.mutable && !borrow.raw));
        // Execute the location operations described by the original borrow prefix.
        // Fake trait calls and the loop are unexecuted, not a full stock pass.
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(TypeId(1)),
                binding.writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let unit = store.ticket(ValueRepr::unit()).unwrap();
        store.initialize(cell, unit).unwrap();
        let first = store.borrow(&Place::cell(cell), Access::Mutable).unwrap();
        let second = store.borrow(&Place::cell(cell), Access::Mutable).unwrap();
        assert_ne!(first, second);
        for reference in [first, second] {
            let incoming = store.ticket(ValueRepr::unit()).unwrap();
            let old = store.replace_reference(reference, incoming).unwrap();
            store.discard(old).unwrap();
            assert_eq!(store.read_reference(first).unwrap(), ValueRepr::unit());
            assert_eq!(store.read_reference(second).unwrap(), ValueRepr::unit());
        }
        let shared = store.borrow(&Place::deref(first), Access::Shared).unwrap();
        let handles = store.budget().handles();
        assert_eq!(
            place_runtime_fault(
                store
                    .borrow(&Place::deref(shared), Access::Mutable)
                    .unwrap_err()
            )
            .code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(store.budget().handles(), handles);
        let incoming = store.ticket(ValueRepr::unit()).unwrap();
        assert_eq!(
            store.replace_reference(shared, incoming).unwrap_err().code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(store.peek_ticket(incoming).unwrap(), ValueRepr::unit());
        store.discard(incoming).unwrap();
        let moved = store.move_from(cell).unwrap();
        assert_eq!(
            store.read_reference(first).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            store.read_reference(second).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        store.initialize(cell, moved).unwrap();
        assert_eq!(
            store.read_reference(shared).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        let fresh = store.borrow(&Place::cell(cell), Access::Shared).unwrap();
        assert_eq!(store.read_reference(fresh).unwrap(), ValueRepr::unit());
        store.reset(2).unwrap();
        assert_eq!(
            store.read_reference(fresh).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }
    #[test]
    fn rustc_immutable_isize_binding_rejects_original_mutable_borrow_before_effects() {
        use crate::syntax::place_fixture_facts::{self as facts, InitializerFacts};
        let binding = facts::binding(STOCK_IMMUTABLE, "x");
        assert_eq!(binding.annotation.as_deref(), Some("isize"));
        let InitializerFacts::Integer(actual) = binding.initializer else {
            panic!("stock integer absent")
        };
        let borrows = facts::borrows_of(STOCK_IMMUTABLE, "x");
        assert_eq!(borrows.len(), 1);
        assert!(borrows[0].mutable && !borrows[0].raw);
        assert_eq!(borrows[0].target, "x");
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(TypeId(8)),
                binding.writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let value = store
            .ticket(ValueRepr::signed(TypeId(8), actual).unwrap())
            .unwrap();
        store.initialize(cell, value).unwrap();
        let handles = store.budget().handles();
        let bytes = store.budget().bytes();
        assert_eq!(
            place_runtime_fault(
                store
                    .borrow(&Place::cell(cell), Access::Mutable)
                    .unwrap_err()
            )
            .code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(store.budget().handles(), handles);
        assert_eq!(store.budget().bytes(), bytes);
        assert_eq!(
            store.read_scalar(cell).unwrap(),
            ValueRepr::signed(TypeId(8), actual).unwrap()
        );
        // Later *y assignment/printing is deliberately unexecuted after refusal.
    }
    const STOCK_POINT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-comp.rs"
    );
    fn point_ticket(
        store: &mut GuestStore,
        bound: &crate::frontend::types::BoundNominalVariant,
        values: &[i64],
    ) -> (TypeId, TicketId) {
        let (ty, guard) = store.begin_nominal_construction(bound).unwrap();
        assert_eq!(bound.fields().len(), values.len());
        for (ordinal, value) in values.iter().enumerate() {
            let ticket = store
                .ticket(ValueRepr::signed(TypeId(8), *value).unwrap())
                .unwrap();
            store
                .construction_push(guard, u32::try_from(ordinal).unwrap(), ticket)
                .unwrap();
        }
        (ty, store.commit_construction(guard, ty).unwrap())
    }
    fn drain_point(store: &mut GuestStore, ticket: TicketId) {
        while let Some(field) = store.aggregate_cleanup_next(ticket).unwrap() {
            store.discard(field).unwrap();
        }
        store.discard(ticket).unwrap();
    }
    #[test]
    fn rustc_point_field_lease_preserves_scalar_assignment_and_rejects_replaced_enclosing_object() {
        use crate::syntax::place_fixture_facts as facts;
        let (writable, initial_values) = facts::point(STOCK_POINT, "a");
        let replacement_values = facts::point_replacement(STOCK_POINT, "c");
        let (lhs, next_value) = facts::assignment(STOCK_POINT, "d");
        assert_eq!(lhs, "p.y");
        let (binding, db) = stock_schema_binding(STOCK_POINT, "point.rs", "Point");
        let crate::frontend::types::NominalBinding::Ready(bound) = binding else {
            panic!("original Point schema must bind")
        };
        assert_eq!(
            bound
                .fields()
                .iter()
                .map(|field| field.primitive())
                .collect::<Vec<_>>(),
            [Some(crate::ir::op::PrimitiveType::Isize); 2]
        );
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let (ty, initial_ticket) = point_ticket(&mut store, &bound, &initial_values);
        let cell = store
            .new_cell(
                StorageContract::Exact(ty),
                writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(cell, initial_ticket).unwrap();
        let place = Place {
            root: PlaceRoot::Cell(cell),
            projections: Box::new([Projection::Field(1)]),
        };
        let shared = store.borrow(&place, Access::Shared).unwrap();
        let field = store
            .references
            .get(shared)
            .unwrap()
            .location()
            .unwrap()
            .final_cell
            .cell;
        let root_epoch = store.location_epoch(cell).unwrap();
        let field_epoch = store.location_epoch(field).unwrap();
        let incoming = store
            .ticket(ValueRepr::signed(TypeId(8), next_value).unwrap())
            .unwrap();
        let old = store.replace_place(&place, incoming).unwrap();
        store.discard(old).unwrap();
        assert_eq!(
            store.read_reference(shared).unwrap(),
            ValueRepr::signed(TypeId(8), next_value).unwrap()
        );
        assert_eq!(store.location_epoch(field).unwrap(), field_epoch);
        let direct = store.borrow(&Place::cell(field), Access::Shared).unwrap();
        assert_eq!(
            store
                .references
                .get(direct)
                .unwrap()
                .location()
                .unwrap()
                .root
                .cell,
            cell
        );
        let (_, replacement_ticket) = point_ticket(&mut store, &bound, &replacement_values);
        let old_record = store.replace(cell, replacement_ticket).unwrap();
        assert_eq!(store.location_epoch(cell).unwrap(), root_epoch);
        assert_eq!(store.location_epoch(field).unwrap(), field_epoch);
        assert_eq!(store.aggregate_field(old_record, 1).unwrap(), field);
        assert_eq!(
            store.read_scalar(field).unwrap(),
            ValueRepr::signed(TypeId(8), next_value).unwrap()
        );
        assert_eq!(
            store.read_reference(shared).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            store.read_reference(direct).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            place_runtime_fault(
                store
                    .borrow(&Place::cell(field), Access::Shared)
                    .unwrap_err()
            )
            .code,
            "LRR-E-STALE-HANDLE"
        );
        drain_point(&mut store, old_record);
        let current = store.borrow(&place, Access::Shared).unwrap();
        let old_field = store
            .references
            .get(current)
            .unwrap()
            .location()
            .unwrap()
            .final_cell
            .cell;
        // Supplemental LRR invalidation case, not a billion-step stock run:
        // force location-epoch exhaustion on the same original Point field.
        store.cells.get_mut(old_field).unwrap().location_epoch = u32::MAX;
        let exhausted = store
            .borrow(&Place::cell(old_field), Access::Shared)
            .unwrap();
        let value = store.move_from(old_field).unwrap();
        let new_field = store.replacement_location(old_field).unwrap();
        assert_ne!(old_field, new_field);
        store.initialize(new_field, value).unwrap();
        assert_eq!(
            store.read_reference(exhausted).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        let current = store.borrow(&place, Access::Shared).unwrap();
        assert_eq!(
            store
                .references
                .get(current)
                .unwrap()
                .location()
                .unwrap()
                .final_cell
                .cell,
            new_field
        );
        let moved = store.move_from(cell).unwrap();
        let new_cell = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot { frame, slot: 1 },
            )
            .unwrap();
        store.initialize(new_cell, moved).unwrap();
        assert_eq!(
            store.read_reference(current).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        let readonly_field = Place {
            root: PlaceRoot::Cell(new_cell),
            projections: Box::new([Projection::Field(1)]),
        };
        assert_eq!(
            place_runtime_fault(store.borrow(&readonly_field, Access::Mutable).unwrap_err()).code,
            "LRR-E-REFERENCE-ACCESS"
        );
        let incoming = store
            .ticket(ValueRepr::signed(TypeId(8), next_value).unwrap())
            .unwrap();
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            place_runtime_fault(store.replace_place(&readonly_field, incoming).unwrap_err()).code,
            "LRR-E-REFERENCE-ACCESS"
        );
        assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
        assert_eq!(
            store.peek_ticket(incoming).unwrap(),
            ValueRepr::signed(TypeId(8), next_value).unwrap()
        );
        store.discard(incoming).unwrap();
        assert_eq!(
            store.read_place(&readonly_field).unwrap(),
            ValueRepr::signed(TypeId(8), replacement_values[1]).unwrap()
        );
        let record = store.move_from(new_cell).unwrap();
        drain_point(&mut store, record);
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        // Original field schemas/operands supply this backend transaction;
        // no full function execution, guest Drop or native compile-fail pass.
    }
    fn stock_unit_binding(
        original: &str,
        logical_path: &str,
    ) -> (
        crate::frontend::types::NominalBinding,
        crate::db::memo::Database,
    ) {
        stock_schema_binding(original, logical_path, "Foo")
    }
    const STOCK_NESTED_TUPLE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/index-float.rs");
    pub(super) fn nested_unit_cells(
        store: &mut GuestStore,
        depth: usize,
        writable: bool,
    ) -> (CellId, CellId) {
        let frame = store.new_frame_anchor().unwrap();
        let mut ty = TypeId(1);
        let mut ticket = store.ticket(ValueRepr::unit()).unwrap();
        for _ in 0..depth {
            let guard = store.begin_construction(&[ty]).unwrap();
            store.construction_push(guard, 0, ticket).unwrap();
            ty = store.intern_type(TypeKey::Tuple(Box::new([ty]))).unwrap();
            ticket = store.commit_construction(guard, ty).unwrap();
        }
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let mut leaf = root;
        for _ in 0..depth {
            leaf = store.place_object(leaf).unwrap().1.fields[0];
        }
        (root, leaf)
    }
    #[test]
    fn rustc_nested_tuple_leaf_roots_retain_every_enclosing_object_and_permission() {
        let depth = crate::syntax::place_fixture_facts::nested_unit_tuple_depth(STOCK_NESTED_TUPLE);
        assert!(depth > 1);
        for (levels, writable) in [(depth, false), (depth * 256, true)] {
            // The first case has the exact stock tuple nesting. The second is a
            // disclosed LRR stress extension of that shape, not a stock pass.
            let mut store = GuestStore::for_epoch(1).unwrap();
            let (root, leaf) = nested_unit_cells(&mut store, levels, writable);
            let reference = store.borrow(&Place::cell(leaf), Access::Shared).unwrap();
            let record = store.references.get(reference).unwrap().location().unwrap();
            assert_eq!(record.root.cell, root);
            assert_eq!(record.final_cell.cell, leaf);
            assert_eq!(record.enclosing.len(), levels);
            assert_eq!(store.read_reference(reference).unwrap(), ValueRepr::unit());
            if !writable {
                let before = (store.budget().handles(), store.budget().bytes());
                assert_eq!(
                    place_runtime_fault(
                        store
                            .borrow(&Place::cell(leaf), Access::Mutable)
                            .unwrap_err()
                    )
                    .code,
                    "LRR-E-REFERENCE-ACCESS"
                );
                assert_eq!((store.budget().handles(), store.budget().bytes()), before);
            }
            let moved = store.move_from(root).unwrap();
            assert_eq!(
                store.read_reference(reference).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
            // Keep the complete aggregate in its transfer owner; physical arena
            // teardown does not run guest Drop or claim cleanup integration.
            assert_eq!(store.peek_ticket(moved).unwrap().kind, 8);
        }
    }
    #[test]
    fn rustc_nested_tuple_forged_owner_cycle_is_rejected_without_guest_mutation() {
        let depth = crate::syntax::place_fixture_facts::nested_unit_tuple_depth(STOCK_NESTED_TUPLE);
        let mut store = GuestStore::for_epoch(1).unwrap();
        let (root, leaf) = nested_unit_cells(&mut store, depth, true);
        let (object, _) = store.place_object(root).unwrap();
        // LRR-only corruption injection makes the root its own installed field.
        // It remains checked by typed arena identities; no host unsafe is used.
        let original_owner = store.cells.get(root).unwrap().owner;
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("tuple aggregate absent")
        };
        let original_child = storage.fields[0];
        storage.fields[0] = root;
        store.cells.get_mut(root).unwrap().owner = CellOwner::Field { object, index: 0 };
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            place_runtime_fault(
                store
                    .borrow(&Place::cell(root), Access::Shared)
                    .unwrap_err()
            )
            .code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);
        store.cells.get_mut(root).unwrap().owner = original_owner;
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("tuple aggregate absent")
        };
        storage.fields[0] = original_child;
        let valid = store.borrow(&Place::cell(leaf), Access::Shared).unwrap();
        assert_eq!(store.read_reference(valid).unwrap(), ValueRepr::unit());
    }
    #[test]
    fn rustc_enum_boolean_field_lease_rejects_active_variant_changes() {
        use crate::frontend::types::NominalBinding;
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/issue-61696.rs"
        );
        let (binding, _) = stock_variant_binding(ORIGINAL, "issue-61696.rs", "E1", Some("V1"));
        let NominalBinding::Ready(bound) = binding else {
            panic!("original boolean variant must bind")
        };
        let (other, _) = stock_variant_binding(ORIGINAL, "issue-61696.rs", "E1", Some("V3"));
        let NominalBinding::Ready(other) = other else {
            panic!("original unit variant must bind")
        };
        assert_ne!(bound.variant(), other.variant());
        assert_eq!(
            bound.fields()[0].primitive(),
            Some(crate::ir::op::PrimitiveType::Bool)
        );
        let value = ValueRepr::boolean(crate::syntax::place_fixture_facts::first_main_boolean(
            ORIGINAL,
        ));
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let (ty, guard) = store.begin_nominal_construction(&bound).unwrap();
        let field = store.ticket(value).unwrap();
        store.construction_push(guard, 0, field).unwrap();
        let ticket = store.commit_construction(guard, ty).unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let place = Place {
            root: PlaceRoot::Cell(root),
            projections: Box::new([Projection::Field(0)]),
        };
        let reference = store.borrow(&place, Access::Shared).unwrap();
        assert_eq!(store.read_reference(reference).unwrap(), value);
        let object = store.place_object(root).unwrap().0;
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("enum storage absent")
        };
        // Supplemental variant corruption preserves object/cell identities and
        // requires the saved active-variant edge itself to reject the lease.
        storage.variant = Some(other.variant());
        assert_eq!(
            store.read_reference(reference).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("enum storage absent")
        };
        storage.variant = Some(bound.variant());
        assert_eq!(store.read_reference(reference).unwrap(), value);
        // Original niche-layout/match assertions and the full run-pass are pending.
    }
    #[test]
    fn rustc_point_source_bound_field_projection_preserves_declared_member_identity() {
        use crate::{
            core::digest::StableEncode,
            db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
            frontend::types::NominalBinding,
            syntax::place_fixture_facts as facts,
        };
        let (binding, mut db) = stock_schema_binding(STOCK_POINT, "point.rs", "Point");
        let NominalBinding::Ready(bound) = binding else {
            panic!("Point must bind")
        };
        let original = db
            .fetch(
                &SyntaxProvider,
                QueryKey::TypeSchema(bound.definition().fingerprint()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::TypeSchema(schema) = original.value.as_ref() else {
            panic!("Point source schema absent")
        };
        let (lhs, _) = facts::assignment(STOCK_POINT, "d");
        let member = lhs.rsplit('.').next().unwrap();
        let field = u32::try_from(
            schema
                .variant(None)
                .unwrap()
                .fields
                .iter()
                .position(|field| field.name.as_deref() == Some(member))
                .unwrap(),
        )
        .unwrap();
        let (writable, values) = facts::point(STOCK_POINT, "a");
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let (ty, ticket) = point_ticket(&mut store, &bound, &values);
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let projected = store
            .project_nominal_field(&Place::cell(root), &bound, field)
            .unwrap();
        assert_eq!(
            store.read_place(&projected).unwrap(),
            ValueRepr::signed(TypeId(8), values[field as usize]).unwrap()
        );
        let (foreign, _) = stock_schema_binding(STOCK_POINT, "foreign-point.rs", "Point");
        let NominalBinding::Ready(foreign) = foreign else {
            panic!("same-shaped foreign Point absent")
        };
        assert_eq!(foreign.fields(), bound.fields());
        assert_ne!(foreign.definition(), bound.definition());
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            place_runtime_fault(
                store
                    .project_nominal_field(&Place::cell(root), &foreign, field)
                    .err()
                    .expect("projection must refuse")
            )
            .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            place_runtime_fault(
                store
                    .project_nominal_field(&Place::cell(root), &bound, u32::MAX)
                    .err()
                    .expect("projection must refuse")
            )
            .code,
            "LRR-E-PLACE-FIELD"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);
        assert_eq!(
            store.read_place(&projected).unwrap(),
            ValueRepr::signed(TypeId(8), values[field as usize]).unwrap()
        );
        // Original source supplies member name, declaration order and values;
        // foreign-unit identity is an LRR supplement, not a full stock run.
    }
    #[test]
    fn rustc_enum_source_bound_projection_rejects_wrong_active_variant() {
        use crate::frontend::types::NominalBinding;
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/issue-61696.rs"
        );
        let (binding, _) = stock_variant_binding(ORIGINAL, "issue-61696.rs", "E1", Some("V1"));
        let NominalBinding::Ready(bound) = binding else {
            panic!("original V1 absent")
        };
        let (other, _) = stock_variant_binding(ORIGINAL, "issue-61696.rs", "E1", Some("V3"));
        let NominalBinding::Ready(other) = other else {
            panic!("original V3 absent")
        };
        let value = ValueRepr::boolean(crate::syntax::place_fixture_facts::first_main_boolean(
            ORIGINAL,
        ));
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let (ty, guard) = store.begin_nominal_construction(&bound).unwrap();
        let field = store.ticket(value).unwrap();
        store.construction_push(guard, 0, field).unwrap();
        let ticket = store.commit_construction(guard, ty).unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let projected = store
            .project_nominal_field(&Place::cell(root), &bound, 0)
            .unwrap();
        assert_eq!(store.read_place(&projected).unwrap(), value);
        let object = store.place_object(root).unwrap().0;
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("enum storage absent")
        };
        // This LRR-only corruption changes the active variant while preserving
        // the old boolean field: forming a fresh place must still reject it.
        storage.variant = Some(other.variant());
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            place_runtime_fault(
                store
                    .project_nominal_field(&Place::cell(root), &bound, 0)
                    .err()
                    .expect("projection must refuse")
            )
            .code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            place_runtime_fault(
                store
                    .project_nominal_field(&Place::cell(root), &other, 0)
                    .err()
                    .expect("projection must refuse")
            )
            .code,
            "LRR-E-PLACE-FIELD"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);
        let ObjectStorage::Aggregate(storage) = &mut store.objects.get_mut(object).unwrap().storage
        else {
            panic!("enum storage absent")
        };
        storage.variant = Some(bound.variant());
        let valid = store
            .project_nominal_field(&Place::cell(root), &bound, 0)
            .unwrap();
        assert_eq!(store.read_place(&valid).unwrap(), value);
        // Full niche-layout/main/match oracles are not executed by this control.
    }
    #[test]
    fn rustc_selected_primitive_field_keeps_unrelated_string_field_unbound() {
        use crate::frontend::types::{NominalBindingDemand, NominalFieldBinding};
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-1.rs"
        );
        let ((selected, string, private), db) = stock_schema_selection(
            ORIGINAL,
            "struct-order-of-eval-1.rs",
            "S",
            |names, caller| {
                let selected = names.bind_named_field(None, "f1", caller)?;
                // Demand the other source member separately; a primitive field
                // witness must not imply the unresolved String field was bound.
                let string = names.bind_named_field(None, "f0", caller)?;
                let mut foreign = caller.clone();
                foreign.crate_key = crate::core::digest::Digest::of_bytes(b"foreign field caller");
                let private = names.bind_named_field(None, "f0", &foreign).unwrap_err();
                Ok((selected, string, private))
            },
        );
        let NominalFieldBinding::Ready(field) = selected else {
            panic!("the original isize member must bind independently")
        };
        assert_eq!(field.primitive(), Some(crate::ir::op::PrimitiveType::Isize));
        assert_ne!(field.field(), 0);
        assert!(matches!(
            string,
            NominalFieldBinding::Deferred {
                field: 0,
                demand: NominalBindingDemand::Name(_)
            }
        ));
        assert_eq!(private.code, "LRR-E-TYPE-SCHEMA");
        assert!(private.message.contains("inaccessible"));
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        assert!(
            db.trace
                .counts
                .keys()
                .all(|name| !name.starts_with("BodyIndex:")
                    && !name.starts_with("Thunk:")
                    && !name.starts_with("TypeGoal:")
                    && !name.starts_with("FunctionSignature:")),
            "unexpected executable query: {:?}",
            db.trace.counts
        );
        // TypeSchema reads its selected declaration header via Signature; this
        // is not a function body or executable signature demand. Verify its
        // source identity rather than forbidding the required header query.
        assert_eq!(db.trace.counts.get("Signature:ProviderRun"), Some(&1));
        assert_eq!(db.trace.truncated, 0);
        let schema_definition = db
            .trace
            .events
            .iter()
            .find_map(|event| {
                if let crate::db::key::QueryKey::TypeSchema(definition) = event.key {
                    Some(definition)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(db.trace.events.iter().all(|event| {
            !matches!(event.key, crate::db::key::QueryKey::Signature(definition)
                if definition != schema_definition)
        }));
        // Original record-update/string execution and output are still pending;
        // this is source selection/privacy evidence only, not a full run-pass.
    }
    #[test]
    fn rustc_point_selected_member_witness_projects_without_complete_variant_binding() {
        use crate::{core::digest::StableEncode, frontend::types::NominalFieldBinding};
        let (selected, _) =
            stock_schema_selection(STOCK_POINT, "point.rs", "Point", |names, caller| {
                let (lhs, _) = crate::syntax::place_fixture_facts::assignment(STOCK_POINT, "d");
                names.bind_named_field(None, lhs.rsplit('.').next().unwrap(), caller)
            });
        let NominalFieldBinding::Ready(selected) = selected else {
            panic!("original Point member absent")
        };
        let (foreign, _) =
            stock_schema_selection(STOCK_POINT, "foreign-point.rs", "Point", |names, caller| {
                names.bind_named_field(None, "y", caller)
            });
        let NominalFieldBinding::Ready(foreign) = foreign else {
            panic!("same-shaped source member absent")
        };
        assert_ne!(selected.fingerprint(), foreign.fingerprint());
        assert_eq!(selected.primitive(), foreign.primitive());
        let (binding, _) = stock_schema_binding(STOCK_POINT, "point.rs", "Point");
        let crate::frontend::types::NominalBinding::Ready(bound) = binding else {
            panic!("Point construction binding absent")
        };
        let (writable, values) = crate::syntax::place_fixture_facts::point(STOCK_POINT, "a");
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let (ty, ticket) = point_ticket(&mut store, &bound, &values);
        let root = store
            .new_cell(
                StorageContract::Exact(ty),
                writable,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, ticket).unwrap();
        let place = store
            .project_source_field(&Place::cell(root), &selected)
            .unwrap();
        assert_eq!(
            store.read_place(&place).unwrap(),
            ValueRepr::signed(TypeId(8), values[selected.field() as usize]).unwrap()
        );
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            place_runtime_fault(
                store
                    .project_source_field(&Place::cell(root), &foreign)
                    .err()
                    .expect("foreign member must refuse")
            )
            .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);
        // Member selection binds one source field; constructing this test value
        // still uses the existing complete-variant construction witness.
    }
    #[test]
    fn rustc_public_enum_field_member_inherits_parent_enum_visibility() {
        use crate::frontend::types::NominalFieldBinding;
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/issue-61696.rs"
        );
        let (selected, db) =
            stock_schema_selection(ORIGINAL, "issue-61696.rs", "E1", |names, caller| {
                let mut foreign = caller.clone();
                foreign.crate_key =
                    crate::core::digest::Digest::of_bytes(b"foreign enum field caller");
                names.bind_named_field(Some("V1"), "f", &foreign)
            });
        let NominalFieldBinding::Ready(selected) = selected else {
            panic!("the public enum's original boolean field must be visible")
        };
        assert_eq!(
            selected.primitive(),
            Some(crate::ir::op::PrimitiveType::Bool)
        );
        assert_eq!(
            selected.definition().item_kind,
            crate::source::manifest::ItemKind::Enum
        );
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        // Original pub enum/field source supplies visibility and type. The
        // foreign caller is an LRR supplement; no full niche-layout run occurs.
    }
    fn stock_schema_binding(
        original: &str,
        logical_path: &str,
        schema_name: &str,
    ) -> (
        crate::frontend::types::NominalBinding,
        crate::db::memo::Database,
    ) {
        stock_variant_binding(original, logical_path, schema_name, None)
    }
    fn stock_variant_binding(
        original: &str,
        logical_path: &str,
        schema_name: &str,
        variant: Option<&str>,
    ) -> (
        crate::frontend::types::NominalBinding,
        crate::db::memo::Database,
    ) {
        stock_schema_selection(original, logical_path, schema_name, |names, _| {
            names.bind_variant(variant)
        })
    }
    pub(super) fn stock_schema_selection<T>(
        original: &str,
        logical_path: &str,
        schema_name: &str,
        select: impl FnOnce(
            &crate::frontend::types::SchemaNames<'_>,
            &crate::syntax::outline::ScopeKey,
        ) -> Result<T, Diagnostic>,
    ) -> (T, crate::db::memo::Database) {
        use crate::{
            core::{
                digest::{Digest, StableEncode},
                profile::ProfileIdentity,
            },
            db::{
                key::{OutlineKey, QueryKey},
                memo::Database,
                provider::SyntaxProvider,
                value::QueryValue,
            },
            frontend::{names::NameNamespaces, types::SchemaNames},
            source::{
                changes::{
                    PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
                },
                snapshot::{CfgSet, SourceBlob, SourcePath},
            },
            syntax::parse::ParseKey,
        };
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(logical_path.as_bytes());
        let path = SourcePath::new(logical_path).unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("stock unit-like Drop source absent")
        };
        let definition = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == schema_name)
            .unwrap()
            .def_key
            .fingerprint();
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::TypeSchema(definition))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::TypeSchema(schema) = outcome.value.as_ref() else {
            panic!("stock unit schema absent")
        };
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let names = SchemaNames::from_database(&mut db, &graph, &view, schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let binding = select(&names, &schema.scope).unwrap();
        (binding, db)
    }
    #[test]
    fn rustc_threaded_unit_drop_schema_defers_on_original_import_before_allocation() {
        use crate::{
            db::key::QueryKey,
            frontend::{
                names::NameLookup,
                signature_names::SignatureName,
                types::{NominalBinding, NominalBindingDemand},
            },
        };
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/unit-like-struct-drop-run.rs"
        );
        let (binding, db) = stock_unit_binding(ORIGINAL, "unit-like-struct-drop-run.rs");
        assert!(matches!(
            binding,
            NominalBinding::Deferred {
                field: None,
                demand: NominalBindingDemand::Name(SignatureName::Unresolved(
                    NameLookup::NeedInput(_)
                )),
            }
        ));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::ExpansionInput(_)
        )));
        // The actual std::thread frontier is not discharged by this component.
        // No bound witness exists to allocate a guest constructor or run Drop.
    }
    #[test]
    fn rustc_unit_like_drop_schema_cannot_borrow_scalar_discard_without_drop_witness() {
        use crate::frontend::types::NominalBinding;
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/destructor-run-for-let-ignore-6892.rs"
        );
        let (binding, _) = stock_unit_binding(ORIGINAL, "destructor-run-for-let-ignore-6892.rs");
        let NominalBinding::Ready(bound) = binding else {
            panic!("import-free stock unit constructor schema must bind")
        };
        assert!(bound.fields().is_empty());
        let mut store = GuestStore::for_epoch(1).unwrap();
        let (ty, guard) = store.begin_nominal_construction(&bound).unwrap();
        assert_eq!(
            store.type_descriptor(ty).unwrap().copy,
            super::super::shape::CopyStatus::Deferred
        );
        assert!(store.type_descriptor(ty).unwrap().destructor.is_none());
        let ticket = store.commit_construction(guard, ty).unwrap();
        let value = store.peek_ticket(ticket).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(cell, ticket).unwrap();
        let handles = store.budget().handles();
        for _ in 0..2 {
            let error = store.admit_discard(cell).unwrap_err();
            assert_eq!(error.code, "LRR-E-UNSUPPORTED");
            assert_eq!(
                error.message,
                "aggregate cleanup requires a checked ordinary destructor plan"
            );
            assert_eq!(store.state(cell).unwrap(), CellState::Live(value));
            assert_eq!(store.budget().handles(), handles);
        }
        // No guest Drop, static mutation, or full upstream body was executed. Physical
        // teardown of the refused run may reclaim storage without callbacks.
        store.reset(2).unwrap();
        assert_eq!(store.state(cell).unwrap_err().code, "LRR-E-STALE-HANDLE");
    }
    #[test]
    fn construction_scalar_cleanup_refuses_unbound_aggregate_without_advancing_owner() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let ty = store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(2)])))
            .unwrap();
        let inner = store.begin_construction(&[TypeId(2)]).unwrap();
        let field = store.ticket(ValueRepr::boolean(true)).unwrap();
        store.construction_push(inner, 0, field).unwrap();
        let aggregate = store.commit_construction(inner, ty).unwrap();
        let outer = store.begin_construction(&[ty]).unwrap();
        store.construction_push(outer, 0, aggregate).unwrap();
        let handles = store.budget().handles();
        for _ in 0..2 {
            let error = store.admit_scalar_construction_cleanup(outer).unwrap_err();
            assert_eq!(error.code, "LRR-E-UNSUPPORTED");
            assert_eq!(
                error.message,
                "constructor field cleanup requires ordinary destructor dispatch"
            );
            assert_eq!(store.budget().handles(), handles);
            assert_eq!(store.constructions.get(outer).unwrap().completed, [0]);
            assert_eq!(
                store.constructions.get(outer).unwrap().state,
                ConstructionState::Building
            );
        }
        // Explicit harness cleanup proves the guarded value was retained.
        let aggregate = store.construction_cleanup_next(outer).unwrap().unwrap();
        assert!(store.construction_cleanup_next(outer).unwrap().is_none());
        let field = store.aggregate_cleanup_next(aggregate).unwrap().unwrap();
        assert_eq!(store.peek_ticket(field).unwrap(), ValueRepr::boolean(true));
        store.discard(field).unwrap();
        assert!(store.aggregate_cleanup_next(aggregate).unwrap().is_none());
        store.discard(aggregate).unwrap();
    }

    #[test]
    fn construction_rejection_keeps_ticket_and_cleanup_cursor_owned() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let guard = store.begin_construction(&[TypeId(2), TypeId(2)]).unwrap();
        let wrong = store.ticket(ValueRepr::unit()).unwrap();
        assert_eq!(
            store.construction_push(guard, 0, wrong).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(store.peek_ticket(wrong).unwrap(), ValueRepr::unit());
        store.discard(wrong).unwrap();
        let value = store.ticket(ValueRepr::boolean(true)).unwrap();
        store.construction_push(guard, 1, value).unwrap();
        let duplicate = store.ticket(ValueRepr::boolean(false)).unwrap();
        assert_eq!(
            store
                .construction_push(guard, 1, duplicate)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(
            store.peek_ticket(duplicate).unwrap(),
            ValueRepr::boolean(false)
        );
        store.discard(duplicate).unwrap();
        let cleanup = store.construction_cleanup_next(guard).unwrap().unwrap();
        assert_eq!(
            store.peek_ticket(cleanup).unwrap(),
            ValueRepr::boolean(true)
        );
        let late = store.ticket(ValueRepr::boolean(false)).unwrap();
        assert_eq!(
            store.construction_push(guard, 0, late).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        store.discard(late).unwrap();
        store.discard(cleanup).unwrap();
        assert!(store.construction_cleanup_next(guard).unwrap().is_none());
    }

    #[test]
    fn worker_type_identity_and_metadata_survive_guest_epoch_reset() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let tuple = TypeKey::Tuple(Box::new([TypeId(7), TypeId(8)]));
        let ty = store.intern_type(tuple.clone()).unwrap();
        let bytes = store.type_metadata_bytes();
        assert!(bytes > 0);
        assert_eq!(store.intern_type(tuple.clone()).unwrap(), ty);
        assert_eq!(store.type_metadata_bytes(), bytes);
        store.reset(2).unwrap();
        assert_eq!(store.intern_type(tuple).unwrap(), ty);
        assert_eq!(store.type_metadata_bytes(), bytes);
        assert_eq!(store.budget.bytes(), 0);
        assert_eq!(
            store.intern_type(TypeKey::Tuple(Box::new([]))).unwrap(),
            TypeId(1)
        );
        assert_eq!(
            store.type_descriptor(TypeId(16)).unwrap().shape,
            ShapeCategory::Unsized
        );
        assert_eq!(
            store.begin_construction(&[TypeId(16)]).unwrap_err().code,
            "LRR-E-VALUE-REPR"
        );
    }

    #[test]
    fn exhausted_location_epoch_is_retired_before_replacement_initialization() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Unbound,
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let ticket = store.ticket(ValueRepr::boolean(true)).unwrap();
        store.initialize(cell, ticket).unwrap();
        store.cells.get_mut(cell).unwrap().location_epoch = u32::MAX;
        let value = store.move_from(cell).unwrap();
        assert!(store.location_epoch(cell).is_err());
        assert!(store.initialize(cell, value).is_err());
        let replacement = store.replacement_location(cell).unwrap();
        assert_ne!(replacement, cell);
        store.initialize(replacement, value).unwrap();
        assert_eq!(
            store.read_scalar(replacement).unwrap(),
            ValueRepr::boolean(true)
        );
        assert!(store.location_epoch(cell).is_err());
    }
}
