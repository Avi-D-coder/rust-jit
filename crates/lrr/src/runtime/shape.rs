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
        /// Actual source declaration identity, including generated item identity.
        /// The structured declaration key is retained independently below.
        identity: Digest,
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
    /// A function item or closure value: its callee identity (the source
    /// function definition, or the closure expression's body/node identity)
    /// and its captured fields in capture order (none for a function item).
    /// Its fields are structural like a tuple's, so copy, move, projection
    /// and drop glue follow the captured field types. `borrowed[i]` marks a
    /// by-reference capture: its field is the borrow and the closure body
    /// names the referent (a by-value capture of a reference is unmarked).
    Callable {
        identity: Digest,
        captures: Box<[TypeId]>,
        borrowed: Box<[bool]>,
    },
    Facade {
        family: FacadeType,
        args: Box<[TypeId]>,
    },
    /// `*const T` / `*mut T` (E-R5-008). The value is a `Copy` pointer over a
    /// raw reference record; `target` is the written pointee, which may differ
    /// from the type of the cell the pointer designates after a pointer cast.
    RawPtr {
        mutable: bool,
        target: TypeId,
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
    Pointer,
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
    /// Nominal enums only: the source-bound declaration-order discriminant
    /// table, attached by the first source-checked variant construction.
    pub discriminants: Option<Arc<[Option<i128>]>>,
}
/// Worker type table. E-R5-006: no descriptor count or metadata budget; the
/// byte figure is an observation only.
#[derive(Default)]
pub struct TypeRegistry {
    descriptors: Vec<TypeDescriptor>,
    by_key: BTreeMap<Arc<TypeKey>, TypeId>,
    /// Owned keys and index nodes; vector allocation is measured separately.
    owned_bytes: usize,
    /// The run's growable-array lang item: its `Vec<T>` Types have the same
    /// `[T]` element-window views as arrays.
    vec_lang: Option<Digest>,
}
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
                discriminants: None,
            }
        })
    })
}
/// Deterministic metadata estimate, including ordered-index node overhead.
const TYPE_ENTRY_OVERHEAD: usize = 128;
impl TypeRegistry {
    /// Install the growable-array lang item before any `Vec<T>` is interned.
    pub(crate) fn set_vec_lang(&mut self, identity: Option<Digest>) {
        self.vec_lang = identity;
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
            TypeKey::Tuple(members) => (
                ShapeCategory::Tuple,
                Some(DeferredFieldRecipe::Structural),
                self.structural_copy(members)?,
            ),
            TypeKey::Callable {
                captures, borrowed, ..
            } => {
                if captures.len() != borrowed.len() {
                    return Err(super::value::invalid());
                }
                (
                    ShapeCategory::Tuple,
                    Some(DeferredFieldRecipe::Structural),
                    self.structural_copy(captures)?,
                )
            }
            TypeKey::Nominal {
                identity,
                definition,
                schema,
                args,
                const_args,
            } => {
                if definition.namespace != Namespace::Type
                    || !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum)
                    || args.len().checked_add(const_args.len()).is_none()
                {
                    return Err(super::value::invalid());
                }
                for argument in args {
                    self.require(*argument)?;
                }
                (
                    ShapeCategory::Nominal,
                    Some(DeferredFieldRecipe::Nominal {
                        definition: *identity,
                        schema: *schema,
                    }),
                    CopyStatus::Deferred,
                )
            }
            TypeKey::Function { params, result } => {
                for parameter in params {
                    self.require(*parameter)?;
                }
                self.require(*result)?;
                (ShapeCategory::Function, None, CopyStatus::Copy)
            }
            TypeKey::RawPtr { target, .. } => {
                self.require(*target)?;
                (ShapeCategory::Pointer, None, CopyStatus::Copy)
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
        self.descriptors.try_reserve(1).map_err(|_| resource())?;
        let id = TypeId(
            u32::try_from(self.descriptors.len())
                .ok()
                .and_then(|n| n.checked_add(17))
                .ok_or_else(resource)?,
        );
        self.owned_bytes = self.owned_bytes.checked_add(bytes).ok_or_else(resource)?;
        let key = Arc::new(key);
        self.by_key.insert(key.clone(), id);
        let array_element = match key.as_ref() {
            TypeKey::Array { element, .. } => Some(*element),
            TypeKey::Nominal { identity, args, .. }
                if self.vec_lang == Some(*identity) && args.len() == 1 =>
            {
                Some(args[0])
            }
            _ => None,
        };
        let reference_target = match key.as_ref() {
            TypeKey::Ref { target, .. } => Some(*target),
            _ => None,
        };
        self.descriptors.push(TypeDescriptor {
            key,
            shape,
            fields,
            copy,
            destructor: None,
            representation_version: 1,
            discriminants: None,
        });
        // Every reference Type has its raw-pointer views, so call contracts and
        // pointer coercions can name them without interning (E-R5-008).
        if let Some(target) = reference_target {
            for mutable in [false, true] {
                self.intern(TypeKey::RawPtr { mutable, target })?;
            }
        }
        // Every array Type has its unsized `[T]` element-window view and the
        // reference Types an unsizing coercion produces, so read-only place
        // resolution and call contracts can name them without interning.
        if let Some(element) = array_element {
            let slice = self.intern(TypeKey::Slice { element })?;
            for mutable in [false, true] {
                self.intern(TypeKey::Ref {
                    mutable,
                    target: slice,
                })?;
            }
        }
        Ok(id)
    }
    /// Attach a nominal enum's source-bound discriminant table. The table is a
    /// property of the declaration schema, so a second attachment must agree.
    pub(crate) fn attach_discriminants(
        &mut self,
        id: TypeId,
        table: &Arc<[Option<i128>]>,
    ) -> Result<(), Diagnostic> {
        let index = id.0.checked_sub(17).ok_or_else(super::value::invalid)?;
        let descriptor = self
            .descriptors
            .get_mut(index as usize)
            .ok_or_else(super::value::invalid)?;
        if !matches!(descriptor.key.as_ref(), TypeKey::Nominal { definition, .. } if definition.item_kind == ItemKind::Enum)
        {
            return Err(super::value::invalid());
        }
        match &descriptor.discriminants {
            Some(existing) if existing != table => Err(super::value::invalid()),
            Some(_) => Ok(()),
            None => {
                descriptor.discriminants = Some(table.clone());
                Ok(())
            }
        }
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
impl TypeKey {
    /// The fn-pointer descriptor with its signature erased: runtime fn
    /// pointer values are interchangeable (E-R5-008), so a parameter, field
    /// or `let` of any fn-pointer type names this one descriptor when the
    /// signature's own descriptors are not at hand.
    pub fn erased_function_pointer() -> Self {
        Self::Function {
            params: Box::new([]),
            result: TypeId(1),
        }
    }
    fn heap_bytes(&self) -> Option<usize> {
        let ids = |members: &[TypeId]| members.len().checked_mul(std::mem::size_of::<TypeId>());
        match self {
            Self::Primitive(_)
            | Self::Ref { .. }
            | Self::RawPtr { .. }
            | Self::Slice { .. }
            | Self::Array { .. } => Some(0),
            Self::Tuple(members) => ids(members),
            Self::Callable {
                captures, borrowed, ..
            } => ids(captures)?.checked_add(borrowed.len()),
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
    const DOMAIN: &'static str = "lrr/v3/runtime-type-key";
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
                identity,
                definition,
                schema,
                args,
                const_args,
            } => {
                e.u32(5);
                e.digest(*identity);
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
            Self::Callable {
                identity,
                captures,
                borrowed,
            } => {
                e.u32(8);
                e.digest(*identity);
                e.sequence(captures, |e, id| e.u32(id.0));
                e.sequence(borrowed, |e, flag| e.bool(*flag));
            }
            Self::RawPtr { mutable, target } => {
                e.u32(9);
                e.bool(*mutable);
                e.u32(target.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_admission_checks_all_members_and_reserves_primitive_ids() {
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
        let mut fresh = TypeRegistry::default();
        assert!(!fresh.contains(TypeId(17)));
        assert_eq!(
            fresh
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
