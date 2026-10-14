//! Worker-local descriptors. Interning a nominal identity does not force its fields.
use crate::core::diagnostic::Diagnostic;
use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    source::manifest::{DefKey, ItemKind, Namespace},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Primitive {
    Unit,
    Bool,
    Char,
    Signed(u8),
    Unsigned(u8),
    F32,
    F64,
    Str,
}
pub fn primitive(id: TypeId) -> Result<Primitive, Diagnostic> {
    match id.0 {
        1 => Ok(Primitive::Unit),
        2 => Ok(Primitive::Bool),
        3 => Ok(Primitive::Char),
        4 => Ok(Primitive::Signed(8)),
        5 => Ok(Primitive::Signed(16)),
        6 => Ok(Primitive::Signed(32)),
        7 | 8 => Ok(Primitive::Signed(64)),
        9 => Ok(Primitive::Unsigned(8)),
        10 => Ok(Primitive::Unsigned(16)),
        11 => Ok(Primitive::Unsigned(32)),
        12 | 13 => Ok(Primitive::Unsigned(64)),
        14 => Ok(Primitive::F32),
        15 => Ok(Primitive::F64),
        16 => Ok(Primitive::Str),
        _ => Err(super::value::invalid()),
    }
}
/// Width is insufficient for identity: i64/isize and u64/usize have distinct IDs.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct PrimitiveIdentity(TypeId);
impl PrimitiveIdentity {
    pub fn from_recipe(primitive: crate::ir::op::PrimitiveType) -> Result<Self, Diagnostic> {
        use crate::ir::op::PrimitiveType as P;
        let id = match primitive {
            P::Unit => 1,
            P::Bool => 2,
            P::Char => 3,
            P::I8 => 4,
            P::I16 => 5,
            P::I32 => 6,
            P::I64 => 7,
            P::Isize => 8,
            P::U8 => 9,
            P::U16 => 10,
            P::U32 => 11,
            P::U64 => 12,
            P::Usize => 13,
            P::F32 => 14,
            P::F64 => 15,
            P::I128 | P::U128 => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "128-bit guest integers are outside the live profile".into(),
                ));
            }
        };
        Self::new(TypeId(id))
    }
    pub fn new(id: TypeId) -> Result<Self, Diagnostic> {
        primitive(id)?;
        Ok(Self(id))
    }
    pub fn id(self) -> TypeId {
        self.0
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum FacadeType {
    String,
    Vec,
    Option,
    Result,
    Range,
    RangeInclusive,
    Box,
    PhantomData,
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum TypeKey {
    Primitive(PrimitiveIdentity),
    Ref {
        mutable: bool,
        target: TypeId,
    },
    Slice {
        element: TypeId,
    },
    Array {
        element: TypeId,
        len: u32,
    },
    Tuple(Box<[TypeId]>),
    Nominal {
        definition: DefKey,
        schema: Digest,
        args: Box<[TypeId]>,
        /// Declared const parameter types are retained by the schema identity.
        const_args: Box<[u64]>,
    },
    Function {
        params: Box<[TypeId]>,
        result: TypeId,
    },
    Facade {
        family: FacadeType,
        args: Box<[TypeId]>,
    },
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShapeCategory {
    Scalar,
    Unsized,
    Reference,
    Array,
    Tuple,
    Nominal,
    Function,
    Facade,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CopyStatus {
    Copy,
    NotCopy,
    Deferred,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeferredFieldRecipe {
    /// Immediate child IDs live in the key; no recursive schema expansion.
    Structural,
    Nominal {
        definition: Digest,
        schema: Digest,
    },
}
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Shared structural key; the ordered index compares values, never pointers.
    pub key: Arc<TypeKey>,
    pub shape: ShapeCategory,
    pub fields: Option<DeferredFieldRecipe>,
    pub copy: CopyStatus,
    /// A missing cached witness does not prove the absence of a user destructor.
    pub destructor: Option<Digest>,
    pub representation_version: u32,
}
pub struct TypeRegistry {
    descriptors: Vec<TypeDescriptor>,
    by_key: BTreeMap<Arc<TypeKey>, TypeId>,
    /// Owned keys and index nodes; vector allocation is measured separately.
    owned_bytes: usize,
    max_metadata_bytes: usize,
}
impl Default for TypeRegistry {
    fn default() -> Self {
        Self::with_metadata_limit(MAX_TYPE_METADATA_BYTES)
    }
}
const MAX_TYPE_METADATA_BYTES: usize = 32 * 1024 * 1024;
/// Reserved primitive descriptors have no source or worker-dependent state.
fn primitive_descriptors() -> &'static [TypeDescriptor; 16] {
    static DESCRIPTORS: OnceLock<[TypeDescriptor; 16]> = OnceLock::new();
    DESCRIPTORS.get_or_init(|| {
        std::array::from_fn(|index| {
            let id = TypeId(index as u32 + 1);
            TypeDescriptor {
                key: Arc::new(TypeKey::Primitive(PrimitiveIdentity(id))),
                shape: if id.0 == 16 {
                    ShapeCategory::Unsized
                } else {
                    ShapeCategory::Scalar
                },
                fields: None,
                copy: if id.0 == 16 {
                    CopyStatus::NotCopy
                } else {
                    CopyStatus::Copy
                },
                destructor: None,
                representation_version: 1,
            }
        })
    })
}
const MAX_TYPES: usize = 100_000;
const MAX_TYPE_MEMBERS: usize = 100_000;
/// Deterministic metadata estimate, including ordered-index node overhead.
const TYPE_ENTRY_OVERHEAD: usize = 128;
impl TypeRegistry {
    /// This budget belongs to the worker table, independently of guest epochs.
    /// Exhaustion requires retiring the worker between runs; IDs are never reused.
    pub fn with_metadata_limit(max_metadata_bytes: usize) -> Self {
        Self {
            descriptors: Vec::new(),
            by_key: BTreeMap::new(),
            owned_bytes: 0,
            max_metadata_bytes: max_metadata_bytes.min(MAX_TYPE_METADATA_BYTES),
        }
    }
    pub fn metadata_bytes(&self) -> usize {
        self.descriptors
            .capacity()
            .checked_mul(std::mem::size_of::<TypeDescriptor>())
            .and_then(|bytes| bytes.checked_add(self.owned_bytes))
            .unwrap_or(usize::MAX)
    }
    pub fn contains(&self, id: TypeId) -> bool {
        (1..=16).contains(&id.0)
            || id
                .0
                .checked_sub(17)
                .is_some_and(|index| (index as usize) < self.descriptors.len())
    }
    pub fn descriptor(&self, id: TypeId) -> Result<&TypeDescriptor, Diagnostic> {
        if (1..=16).contains(&id.0) {
            return Ok(&primitive_descriptors()[id.0 as usize - 1]);
        }
        let index = id.0.checked_sub(17).ok_or_else(super::value::invalid)?;
        self.descriptors
            .get(index as usize)
            .ok_or_else(super::value::invalid)
    }
    fn require(&self, id: TypeId) -> Result<(), Diagnostic> {
        if self.contains(id) {
            Ok(())
        } else {
            Err(super::value::invalid())
        }
    }
    fn copy_status(&self, id: TypeId) -> Result<CopyStatus, Diagnostic> {
        if id.0 <= 16 {
            primitive(id)?;
            return Ok(if id.0 == 16 {
                CopyStatus::NotCopy
            } else {
                CopyStatus::Copy
            });
        }
        Ok(self.descriptor(id)?.copy)
    }
    fn structural_copy(&self, members: &[TypeId]) -> Result<CopyStatus, Diagnostic> {
        let mut status = CopyStatus::Copy;
        for member in members {
            match self.copy_status(*member)? {
                CopyStatus::NotCopy => status = CopyStatus::NotCopy,
                CopyStatus::Deferred if status == CopyStatus::Copy => status = CopyStatus::Deferred,
                CopyStatus::Deferred | CopyStatus::Copy => {}
            }
        }
        Ok(status)
    }
    pub(crate) fn find(&self, key: &TypeKey) -> Option<TypeId> {
        match key {
            TypeKey::Primitive(primitive) => Some(primitive.id()),
            TypeKey::Tuple(members) if members.is_empty() => Some(TypeId(1)),
            _ => self.by_key.get(key).copied(),
        }
    }
    pub fn intern(&mut self, key: TypeKey) -> Result<TypeId, Diagnostic> {
        if let TypeKey::Primitive(primitive) = &key {
            return Ok(primitive.id());
        }
        if matches!(&key, TypeKey::Tuple(members) if members.is_empty()) {
            return Ok(TypeId(1));
        }
        if let Some(id) = self.by_key.get(&key) {
            return Ok(*id);
        }
        let resource = super::arena::resource;
        if self.descriptors.len() >= MAX_TYPES {
            return Err(resource());
        }
        let (shape, fields, copy) = match &key {
            TypeKey::Primitive(_) => return Err(super::value::invalid()),
            TypeKey::Ref { mutable, target } => {
                self.require(*target)?;
                (
                    ShapeCategory::Reference,
                    None,
                    if *mutable {
                        CopyStatus::NotCopy
                    } else {
                        CopyStatus::Copy
                    },
                )
            }
            TypeKey::Slice { element } => {
                self.require(*element)?;
                (ShapeCategory::Unsized, None, CopyStatus::NotCopy)
            }
            TypeKey::Array { element, .. } => {
                self.require(*element)?;
                (
                    ShapeCategory::Array,
                    Some(DeferredFieldRecipe::Structural),
                    self.copy_status(*element)?,
                )
            }
            TypeKey::Tuple(members) => {
                if members.len() > MAX_TYPE_MEMBERS {
                    return Err(resource());
                }
                (
                    ShapeCategory::Tuple,
                    Some(DeferredFieldRecipe::Structural),
                    self.structural_copy(members)?,
                )
            }
            TypeKey::Nominal {
                definition,
                schema,
                args,
                const_args,
            } => {
                if definition.namespace != Namespace::Type
                    || !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum)
                    || args
                        .len()
                        .checked_add(const_args.len())
                        .is_none_or(|n| n > MAX_TYPE_MEMBERS)
                    || definition.module_path.len() > 256
                {
                    return Err(super::value::invalid());
                }
                for argument in args {
                    self.require(*argument)?;
                }
                (
                    ShapeCategory::Nominal,
                    Some(DeferredFieldRecipe::Nominal {
                        definition: definition.fingerprint(),
                        schema: *schema,
                    }),
                    CopyStatus::Deferred,
                )
            }
            TypeKey::Function { params, result } => {
                if params.len() > MAX_TYPE_MEMBERS {
                    return Err(resource());
                }
                for parameter in params {
                    self.require(*parameter)?;
                }
                self.require(*result)?;
                (ShapeCategory::Function, None, CopyStatus::Copy)
            }
            TypeKey::Facade { family, args } => {
                let arity = match family {
                    FacadeType::String => 0,
                    FacadeType::Result => 2,
                    _ => 1,
                };
                if args.len() != arity {
                    return Err(super::value::invalid());
                }
                for argument in args {
                    self.require(*argument)?;
                }
                (
                    ShapeCategory::Facade,
                    None,
                    if *family == FacadeType::String {
                        CopyStatus::NotCopy
                    } else {
                        CopyStatus::Deferred
                    },
                )
            }
        };
        let bytes = key
            .heap_bytes()
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<TypeKey>() + 2 * std::mem::size_of::<usize>())
            })
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<(Arc<TypeKey>, TypeId)>() + TYPE_ENTRY_OVERHEAD)
            })
            .ok_or_else(resource)?;
        let prospective = self
            .metadata_bytes()
            .checked_add(bytes)
            .and_then(|n| {
                n.checked_add(if self.descriptors.len() == self.descriptors.capacity() {
                    std::mem::size_of::<TypeDescriptor>()
                } else {
                    0
                })
            })
            .ok_or_else(resource)?;
        if prospective > self.max_metadata_bytes {
            return Err(metadata_limit());
        }
        self.descriptors
            .try_reserve_exact(1)
            .map_err(|_| resource())?;
        // Include any allocator capacity slack even if a reservation is rejected.
        if self
            .metadata_bytes()
            .checked_add(bytes)
            .is_none_or(|n| n > self.max_metadata_bytes)
        {
            return Err(metadata_limit());
        }
        let id = TypeId(
            u32::try_from(self.descriptors.len())
                .ok()
                .and_then(|n| n.checked_add(17))
                .ok_or_else(resource)?,
        );
        self.owned_bytes = self.owned_bytes.checked_add(bytes).ok_or_else(resource)?;
        let key = Arc::new(key);
        self.by_key.insert(key.clone(), id);
        self.descriptors.push(TypeDescriptor {
            key,
            shape,
            fields,
            copy,
            destructor: None,
            representation_version: 1,
        });
        Ok(id)
    }
    pub fn intern_string(&mut self) -> Result<TypeId, Diagnostic> {
        self.intern(TypeKey::Facade {
            family: FacadeType::String,
            args: Box::new([]),
        })
    }
    pub fn is_string(&self, id: TypeId) -> bool {
        self.descriptor(id).is_ok_and(|descriptor| matches!(descriptor.key.as_ref(), TypeKey::Facade { family: FacadeType::String, args } if args.is_empty()))
    }
}
fn metadata_limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "worker type descriptors exceed their metadata budget".into(),
    )
}
impl TypeKey {
    fn heap_bytes(&self) -> Option<usize> {
        let ids = |members: &[TypeId]| members.len().checked_mul(std::mem::size_of::<TypeId>());
        match self {
            Self::Primitive(_) | Self::Ref { .. } | Self::Slice { .. } | Self::Array { .. } => {
                Some(0)
            }
            Self::Tuple(members) => ids(members),
            Self::Function { params, .. } | Self::Facade { args: params, .. } => ids(params),
            Self::Nominal {
                definition,
                args,
                const_args,
                ..
            } => {
                let mut bytes = ids(args)?
                    .checked_add(const_args.len().checked_mul(std::mem::size_of::<u64>())?)?
                    .checked_add(
                        definition
                            .module_path
                            .capacity()
                            .checked_mul(std::mem::size_of::<String>())?,
                    )?
                    .checked_add(definition.declared_name.capacity())?;
                for module in &definition.module_path {
                    bytes = bytes.checked_add(module.capacity())?;
                }
                Some(bytes)
            }
        }
    }
}
/// Worker-local fingerprint only: child IDs must be remapped from stable source
/// keys before interning. This encoding is never a portable cache record.
impl StableEncode for TypeKey {
    const DOMAIN: &'static str = "lrr/v2/runtime-type-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Primitive(primitive) => {
                e.u32(0);
                e.u32(primitive.id().0);
            }
            Self::Ref { mutable, target } => {
                e.u32(1);
                e.bool(*mutable);
                e.u32(target.0);
            }
            Self::Slice { element } => {
                e.u32(2);
                e.u32(element.0);
            }
            Self::Array { element, len } => {
                e.u32(3);
                e.u32(element.0);
                e.u32(*len);
            }
            Self::Tuple(members) => {
                e.u32(4);
                e.sequence(members, |e, id| e.u32(id.0));
            }
            Self::Nominal {
                definition,
                schema,
                args,
                const_args,
            } => {
                e.u32(5);
                e.record(definition);
                e.digest(*schema);
                e.sequence(args, |e, id| e.u32(id.0));
                e.sequence(const_args, |e, value| e.u64(*value));
            }
            Self::Function { params, result } => {
                e.u32(6);
                e.sequence(params, |e, id| e.u32(id.0));
                e.u32(result.0);
            }
            Self::Facade { family, args } => {
                e.u32(7);
                e.u32(match family {
                    FacadeType::String => 0,
                    FacadeType::Vec => 1,
                    FacadeType::Option => 2,
                    FacadeType::Result => 3,
                    FacadeType::Range => 4,
                    FacadeType::RangeInclusive => 5,
                    FacadeType::Box => 6,
                    FacadeType::PhantomData => 7,
                });
                e.sequence(args, |e, id| e.u32(id.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_admission_checks_all_members_and_preserves_ids_on_exhaustion() {
        let mut types = TypeRegistry::default();
        let invalid = TypeKey::Tuple(Box::new([TypeId(16), TypeId(u32::MAX)]));
        assert_eq!(types.intern(invalid).unwrap_err().code, "LRR-E-VALUE-REPR");
        assert_eq!(types.metadata_bytes(), 0);
        let first = types
            .intern(TypeKey::Array {
                element: TypeId(2),
                len: 0,
            })
            .unwrap();
        assert_eq!(first, TypeId(17));
        assert_eq!(types.descriptor(first).unwrap().copy, CopyStatus::Copy);
        let mut limited = TypeRegistry::with_metadata_limit(0);
        assert_eq!(
            limited
                .intern(TypeKey::Tuple(Box::new([TypeId(2)])))
                .unwrap_err()
                .code,
            "LRR-E-METADATA-LIMIT"
        );
        assert!(!limited.contains(TypeId(17)));
        assert_eq!(limited.metadata_bytes(), 0);
        assert_eq!(
            limited
                .intern(TypeKey::Primitive(
                    PrimitiveIdentity::new(TypeId(8)).unwrap()
                ))
                .unwrap(),
            TypeId(8)
        );
        assert_ne!(
            TypeKey::Primitive(PrimitiveIdentity::new(TypeId(7)).unwrap()).fingerprint(),
            TypeKey::Primitive(PrimitiveIdentity::new(TypeId(8)).unwrap()).fingerprint()
        );
    }
}
