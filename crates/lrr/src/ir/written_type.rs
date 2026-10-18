//! A written local Type annotation selected by the entered Type service.
//!
//! Every named leaf was selected at its exact indexed Type occurrence by the
//! ordinary body Type service (`PreparedTypePlan`); tuples, arrays, slices,
//! references and parentheses are the written structure around them. The
//! tree holds source identities only: run-local descriptors are interned
//! from it on use.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    frontend::source_types::SourceType,
    ir::op::PrimitiveType,
    runtime::{
        cell::GuestStore,
        shape::{PrimitiveIdentity, TypeId, TypeKey},
    },
    traits::term::{ConstTerm, DynSchemaKey, RegionTerm, TyTerm},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WrittenType {
    Primitive(PrimitiveType),
    /// A selected argument-free nominal declaration.
    Nominal(Box<SourceType>),
    Tuple(Vec<WrittenType>),
    Array(Box<WrittenType>, u32),
    /// A generic nominal declaration applied to written type arguments.
    Generic {
        identity: crate::core::digest::Digest,
        definition: crate::source::manifest::DefKey,
        schema: crate::core::digest::Digest,
        args: Vec<WrittenType>,
    },
    /// The builtin unsized `str` pointee.
    Str,
    /// An unsized `[T]` pointee.
    Slice(Box<WrittenType>),
    /// `&'r T` or `&'r mut T`. An elided (or `'_`) region is this
    /// annotation's own local region identity; regions are not runtime data.
    Ref {
        region: RegionTerm,
        mutable: bool,
        target: Box<WrittenType>,
    },
    /// `_`: the written structure around it is known; this leaf is left to
    /// inference from the initializer.
    Infer,
    /// `dyn Trait`: the principal trait's object schema. A trait-object
    /// place keeps its value's concrete runtime type.
    Dyn(DynSchemaKey),
    /// A declaration field's own type parameter (index among the
    /// declaration's type parameters), instantiated by the value's
    /// `TypeKey::Nominal` args.
    Parameter(u32),
    /// `*const T` / `*mut T` (E-R5-008). Like a slice or trait-object pointee
    /// it is reached through a coercion or an `as` cast, so it names no single
    /// exact descriptor (`exact()` is false).
    Ptr {
        mutable: bool,
        target: Box<WrittenType>,
    },
    /// `fn(A, B) -> R`, `unsafe fn` or `extern "C" fn`: the ABI and `unsafe`
    /// qualifiers are not part of a runtime fn pointer.
    FnPtr {
        params: Vec<WrittenType>,
        result: Box<WrittenType>,
    },
}
impl WrittenType {
    /// Whether this annotation names one exact runtime descriptor. An
    /// inference hole names only a structure; a slice or trait-object pointee
    /// is reached from a sized value through a coercion.
    pub(crate) fn exact(&self) -> bool {
        match self {
            Self::Primitive(_) | Self::Nominal(_) | Self::Str | Self::Dyn(_) => true,
            Self::Tuple(members) => members.iter().all(Self::exact),
            Self::Array(element, _) => element.exact(),
            Self::Ref { target, .. } => target.exact(),
            Self::Generic { args, .. } => args.iter().all(Self::exact),
            Self::Slice(_)
            | Self::Infer
            | Self::Parameter(_)
            | Self::Ptr { .. }
            | Self::FnPtr { .. } => false,
        }
    }
    /// Whether this is a raw pointer or fn pointer annotation, whose values
    /// arrive by pointer coercion or cast (E-R5-008).
    pub(crate) fn is_pointer(&self) -> bool {
        matches!(self, Self::Ptr { .. } | Self::FnPtr { .. })
    }
    /// Whether no part of this Type is left to inference or reached through a
    /// coercion (an inference hole, a slice or a trait object); declaration
    /// parameters are allowed.
    pub(crate) fn coercion_free(&self) -> bool {
        match self {
            Self::Primitive(_) | Self::Nominal(_) | Self::Str | Self::Parameter(_) => true,
            Self::Tuple(members) => members.iter().all(Self::coercion_free),
            Self::Array(element, _) => element.coercion_free(),
            Self::Ref { target, .. } => target.coercion_free(),
            Self::Generic { args, .. } => args.iter().all(Self::coercion_free),
            Self::Slice(_) | Self::Infer | Self::Dyn(_) | Self::Ptr { .. } | Self::FnPtr { .. } => {
                false
            }
        }
    }
    /// Whether a declaration type parameter occurs in this Type.
    pub(crate) fn has_parameters(&self) -> bool {
        match self {
            Self::Parameter(_) => true,
            Self::Primitive(_) | Self::Nominal(_) | Self::Str | Self::Infer | Self::Dyn(_) => false,
            Self::Tuple(members) => members.iter().any(Self::has_parameters),
            Self::Generic { args, .. } => args.iter().any(Self::has_parameters),
            Self::Array(element, _)
            | Self::Slice(element)
            | Self::Ref {
                target: element, ..
            }
            | Self::Ptr {
                target: element, ..
            } => element.has_parameters(),
            Self::FnPtr { params, result } => {
                params.iter().any(Self::has_parameters) || result.has_parameters()
            }
        }
    }
    /// The source Type this annotation names; `None` while a leaf is left to
    /// inference.
    pub(crate) fn source(&self) -> Option<TyTerm> {
        self.source_in(None, &[])
    }
    /// The source Type under the instantiating type arguments; a parameter's
    /// argument is a closed runtime descriptor's source term.
    pub(crate) fn source_in(&self, store: Option<&GuestStore>, args: &[TypeId]) -> Option<TyTerm> {
        Some(match self {
            Self::Primitive(primitive) => TyTerm::Primitive(*primitive),
            Self::Nominal(source) => source.source().clone(),
            Self::Tuple(members) if members.is_empty() => {
                TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)
            }
            Self::Tuple(members) => TyTerm::Tuple(
                members
                    .iter()
                    .map(|member| member.source_in(store, args))
                    .collect::<Option<Vec<_>>>()?,
            ),
            Self::Array(element, len) => TyTerm::Array(
                Box::new(element.source_in(store, args)?),
                ConstTerm::Value(u64::from(*len)),
            ),
            Self::Generic {
                identity,
                args: written,
                ..
            } => TyTerm::Nominal(
                *identity,
                written
                    .iter()
                    .map(|arg| {
                        arg.source_in(store, args)
                            .map(crate::traits::term::GenericArg::Type)
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            Self::Str => TyTerm::Str,
            Self::Slice(element) => TyTerm::Slice(Box::new(element.source_in(store, args)?)),
            Self::Ref {
                region,
                mutable,
                target,
            } => TyTerm::Ref(
                region.clone(),
                *mutable,
                Box::new(target.source_in(store, args)?),
            ),
            Self::Infer => return None,
            Self::Dyn(schema) => TyTerm::Dyn(*schema),
            Self::Parameter(index) => store?.descriptor_source_term(*args.get(*index as usize)?)?,
            Self::Ptr { mutable, target } => {
                crate::traits::pointer_terms::raw_pointer(*mutable, target.source_in(store, args)?)
            }
            Self::FnPtr { params, result } => {
                crate::traits::pointer_terms::fn_pointer(
                    params
                        .iter()
                        .map(|param| param.source_in(store, args))
                        .collect::<Option<Vec<_>>>()?,
                    result.source_in(store, args)?,
                )
            }
        })
    }
    /// Every written region, for recipe verification.
    pub(crate) fn regions_satisfy(&self, check: &mut dyn FnMut(&RegionTerm) -> bool) -> bool {
        match self {
            Self::Primitive(_)
            | Self::Nominal(_)
            | Self::Str
            | Self::Infer
            | Self::Dyn(_)
            | Self::Parameter(_) => true,
            Self::Tuple(members) => members.iter().all(|member| member.regions_satisfy(check)),
            Self::Generic { args, .. } => args.iter().all(|arg| arg.regions_satisfy(check)),
            Self::Array(element, _) | Self::Slice(element) => element.regions_satisfy(check),
            Self::Ptr { target, .. } => target.regions_satisfy(check),
            Self::FnPtr { params, result } => {
                params.iter().all(|param| param.regions_satisfy(check))
                    && result.regions_satisfy(check)
            }
            Self::Ref { region, target, .. } => check(region) && target.regions_satisfy(check),
        }
    }
    fn generic_key(
        identity: &crate::core::digest::Digest,
        definition: &crate::source::manifest::DefKey,
        schema: &crate::core::digest::Digest,
        args: Vec<TypeId>,
    ) -> TypeKey {
        TypeKey::Nominal {
            identity: *identity,
            definition: definition.clone(),
            schema: *schema,
            args: args.into_boxed_slice(),
            const_args: Box::new([]),
        }
    }
    /// The already interned descriptor of an exact annotation, if any; never
    /// interns.
    pub(crate) fn find(&self, store: &GuestStore) -> Result<Option<TypeId>, Diagnostic> {
        // A written pointer names one descriptor when its parts are closed.
        if !self.exact() && !self.is_pointer() {
            return Ok(None);
        }
        self.find_in(store, &[])
    }
    /// The already interned descriptor of this Type under the instantiating
    /// type arguments; `None` when it names no single descriptor or is not
    /// yet interned. Never interns.
    pub(crate) fn find_in(
        &self,
        store: &GuestStore,
        args: &[TypeId],
    ) -> Result<Option<TypeId>, Diagnostic> {
        Ok(match self {
            Self::Primitive(primitive) => Some(PrimitiveIdentity::from_recipe(*primitive)?.id()),
            Self::Nominal(source) => source.find_descriptor(store),
            Self::Str => Some(TypeId(16)),
            Self::Parameter(index) => args.get(*index as usize).copied(),
            Self::Tuple(members) if members.is_empty() => Some(TypeId(1)),
            Self::Tuple(members) => {
                let mut ids = Vec::new();
                ids.try_reserve_exact(members.len())
                    .map_err(|_| crate::runtime::arena::resource())?;
                for member in members {
                    let Some(id) = member.find_in(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                store.find_type(&TypeKey::Tuple(ids.into_boxed_slice()))
            }
            Self::Array(element, len) => {
                let Some(element) = element.find_in(store, args)? else {
                    return Ok(None);
                };
                store.find_type(&TypeKey::Array { element, len: *len })
            }
            Self::Slice(element) => {
                let Some(element) = element.find_in(store, args)? else {
                    return Ok(None);
                };
                store.find_type(&TypeKey::Slice { element })
            }
            Self::Ref {
                mutable, target, ..
            } => {
                let Some(target) = target.find_in(store, args)? else {
                    return Ok(None);
                };
                store.find_type(&TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })
            }
            Self::Generic {
                identity,
                definition,
                schema,
                args: written,
            } => {
                let mut ids = Vec::new();
                for arg in written {
                    let Some(id) = arg.find_in(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                store.find_type(&Self::generic_key(identity, definition, schema, ids))
            }
            Self::Ptr { mutable, target } => {
                let Some(target) = target.find_in(store, args)? else {
                    return Ok(None);
                };
                store.find_type(&TypeKey::RawPtr {
                    mutable: *mutable,
                    target,
                })
            }
            Self::FnPtr { params, result } => {
                let mut ids = Vec::new();
                for param in params {
                    let Some(id) = param.find_in(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                let Some(result) = result.find_in(store, args)? else {
                    return Ok(None);
                };
                store.find_type(&TypeKey::Function {
                    params: ids.into_boxed_slice(),
                    result,
                })
            }
            Self::Dyn(schema) => store.find_type(&TypeKey::Dyn { schema: schema.0 }),
            Self::Infer => None,
        })
    }
    /// Intern the run-local descriptor of an exact annotation; `None` for a
    /// structure that names no single descriptor.
    pub(crate) fn materialize(&self, store: &mut GuestStore) -> Result<Option<TypeId>, Diagnostic> {
        // A written pointer is interned even though it is reached through a
        // coercion: the slot it hints then coerces its value to it (E-R5-008).
        if !self.exact() && !self.is_pointer() {
            return Ok(None);
        }
        self.materialize_in(store, &[])
    }
    /// Intern this Type's descriptor under the instantiating type arguments;
    /// `None` for an inference hole, a trait object or a missing argument.
    pub(crate) fn materialize_in(
        &self,
        store: &mut GuestStore,
        args: &[TypeId],
    ) -> Result<Option<TypeId>, Diagnostic> {
        self.materialize_with(store, &|index| args.get(index as usize).copied())
    }
    /// As `materialize_in`, with type arguments that may not yet be fixed:
    /// `None` wherever a needed parameter is still open.
    pub(crate) fn materialize_with(
        &self,
        store: &mut GuestStore,
        args: &dyn Fn(u32) -> Option<TypeId>,
    ) -> Result<Option<TypeId>, Diagnostic> {
        Ok(Some(match self {
            Self::Primitive(primitive) => PrimitiveIdentity::from_recipe(*primitive)?.id(),
            Self::Nominal(source) => source.materialize(store)?,
            Self::Str => TypeId(16),
            Self::Parameter(index) => match args(*index) {
                Some(id) => id,
                None => return Ok(None),
            },
            Self::Tuple(members) if members.is_empty() => TypeId(1),
            Self::Tuple(members) => {
                let mut ids = Vec::new();
                ids.try_reserve_exact(members.len())
                    .map_err(|_| crate::runtime::arena::resource())?;
                for member in members {
                    let Some(id) = member.materialize_with(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                store.intern_type(TypeKey::Tuple(ids.into_boxed_slice()))?
            }
            Self::Array(element, len) => {
                let Some(element) = element.materialize_with(store, args)? else {
                    return Ok(None);
                };
                store.intern_type(TypeKey::Array { element, len: *len })?
            }
            Self::Slice(element) => {
                let Some(element) = element.materialize_with(store, args)? else {
                    return Ok(None);
                };
                store.intern_type(TypeKey::Slice { element })?
            }
            Self::Ref {
                mutable, target, ..
            } => {
                let Some(target) = target.materialize_with(store, args)? else {
                    return Ok(None);
                };
                store.intern_type(TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })?
            }
            Self::Generic {
                identity,
                definition,
                schema,
                args: written,
            } => {
                let mut ids = Vec::new();
                for arg in written {
                    let Some(id) = arg.materialize_with(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                store.intern_type(Self::generic_key(identity, definition, schema, ids))?
            }
            Self::Ptr { mutable, target } => {
                let Some(target) = target.materialize_with(store, args)? else {
                    return Ok(None);
                };
                store.intern_type(TypeKey::RawPtr {
                    mutable: *mutable,
                    target,
                })?
            }
            Self::FnPtr { params, result } => {
                let mut ids = Vec::new();
                for param in params {
                    let Some(id) = param.materialize_with(store, args)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                let Some(result) = result.materialize_with(store, args)? else {
                    return Ok(None);
                };
                store.intern_type(TypeKey::Function {
                    params: ids.into_boxed_slice(),
                    result,
                })?
            }
            Self::Dyn(schema) => store.intern_type(TypeKey::Dyn { schema: schema.0 })?,
            Self::Infer => return Ok(None),
        }))
    }
    /// Whether a reached value of descriptor `ty` has this annotation's
    /// written structure. An inference hole admits any Type; a trait-object
    /// pointee admits its concrete value's Type.
    pub(crate) fn admits(&self, ty: TypeId, store: &GuestStore) -> Result<bool, Diagnostic> {
        self.bind(ty, store, &mut [])
    }
    /// Match a reached descriptor against this Type, fixing each declaration
    /// type parameter it determines (`args[i]`); an already fixed parameter
    /// must agree. `false` when the structure differs.
    pub(crate) fn bind(
        &self,
        ty: TypeId,
        store: &GuestStore,
        args: &mut [Option<TypeId>],
    ) -> Result<bool, Diagnostic> {
        let key = std::sync::Arc::clone(&store.type_descriptor(ty)?.key);
        Ok(match (self, key.as_ref()) {
            (Self::Infer | Self::Dyn(_), _) => true,
            (Self::Parameter(index), _) => match args.get_mut(*index as usize) {
                Some(Some(fixed)) => {
                    *fixed == ty || store.dyn_admits(*fixed, ty) || store.dyn_admits(ty, *fixed)
                }
                Some(slot) => {
                    *slot = Some(ty);
                    true
                }
                None => false,
            },
            (Self::Primitive(primitive), _) => {
                PrimitiveIdentity::from_recipe(*primitive)?.id() == ty
            }
            (Self::Str, _) => ty == TypeId(16),
            (Self::Nominal(source), _) => source.find_descriptor(store) == Some(ty),
            (Self::Tuple(members), _) if members.is_empty() => ty == TypeId(1),
            (Self::Tuple(members), TypeKey::Tuple(ids)) => {
                if members.len() != ids.len() {
                    return Ok(false);
                }
                for (member, id) in members.iter().zip(ids.iter()) {
                    if !member.bind(*id, store, args)? {
                        return Ok(false);
                    }
                }
                true
            }
            (
                Self::Array(element, len),
                TypeKey::Array {
                    element: id,
                    len: actual,
                },
            ) => len == actual && element.bind(*id, store, args)?,
            (Self::Slice(element), TypeKey::Slice { element: id }) => {
                element.bind(*id, store, args)?
            }
            (
                Self::Ref {
                    mutable, target, ..
                },
                TypeKey::Ref {
                    mutable: actual,
                    target: id,
                },
            ) => mutable == actual && target.bind(*id, store, args)?,
            (
                Self::Ptr {
                    mutable, target, ..
                },
                TypeKey::RawPtr {
                    mutable: actual,
                    target: id,
                },
            ) => mutable == actual && target.bind(*id, store, args)?,
            (
                Self::FnPtr { params, result },
                TypeKey::Function {
                    params: ids,
                    result: actual,
                },
            ) => {
                if params.len() != ids.len() {
                    return Ok(false);
                }
                for (param, id) in params.iter().zip(ids.iter()) {
                    if !param.bind(*id, store, args)? {
                        return Ok(false);
                    }
                }
                result.bind(*actual, store, args)?
            }
            (
                Self::Generic {
                    identity,
                    args: written,
                    ..
                },
                TypeKey::Nominal {
                    identity: actual,
                    args: ids,
                    ..
                },
            ) => {
                if identity != actual || written.len() != ids.len() {
                    return Ok(false);
                }
                for (arg, id) in written.iter().zip(ids.iter()) {
                    if !arg.bind(*id, store, args)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => false,
        })
    }
    pub(crate) fn heap_bytes(&self) -> usize {
        let boxed = |inner: &Self| std::mem::size_of::<Self>().saturating_add(inner.heap_bytes());
        match self {
            Self::Primitive(_) | Self::Str | Self::Infer | Self::Dyn(_) | Self::Parameter(_) => 0,
            Self::Nominal(source) => source.allocated_bytes().unwrap_or(usize::MAX),
            Self::Tuple(members) => members.iter().fold(
                members
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Self>()),
                |bytes, member| bytes.saturating_add(member.heap_bytes()),
            ),
            // A region term is inline; only the boxed pointee allocates.
            Self::Array(element, _)
            | Self::Slice(element)
            | Self::Ref {
                target: element, ..
            }
            | Self::Ptr {
                target: element, ..
            } => boxed(element),
            Self::FnPtr { params, result } => params.iter().fold(
                params
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Self>())
                    .saturating_add(boxed(result)),
                |bytes, param| bytes.saturating_add(param.heap_bytes()),
            ),
            Self::Generic { args, .. } => args.iter().fold(
                args.capacity().saturating_mul(std::mem::size_of::<Self>()),
                |bytes, arg| bytes.saturating_add(arg.heap_bytes()),
            ),
        }
    }
}
impl StableEncode for WrittenType {
    const DOMAIN: &'static str = "lrr/v1/written-local-type";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Primitive(primitive) => {
                e.u32(0);
                e.u32(primitive.tag());
            }
            Self::Nominal(source) => {
                e.u32(1);
                e.record(source.as_ref());
            }
            Self::Tuple(members) => {
                e.u32(2);
                e.sequence(members, |e, member| e.record(member));
            }
            Self::Array(element, len) => {
                e.u32(3);
                e.record(element.as_ref());
                e.u32(*len);
            }
            Self::Generic {
                identity,
                definition,
                schema,
                args,
            } => {
                e.u32(4);
                e.digest(*identity);
                e.record(definition);
                e.digest(*schema);
                e.sequence(args, |e, arg| e.record(arg));
            }
            Self::Str => e.u32(5),
            Self::Slice(element) => {
                e.u32(6);
                e.record(element.as_ref());
            }
            Self::Ref {
                region,
                mutable,
                target,
            } => {
                e.u32(7);
                e.record(region);
                e.bool(*mutable);
                e.record(target.as_ref());
            }
            Self::Infer => e.u32(8),
            Self::Dyn(schema) => {
                e.u32(9);
                e.record(schema);
            }
            Self::Parameter(index) => {
                e.u32(10);
                e.u32(*index);
            }
            Self::Ptr { mutable, target } => {
                e.u32(11);
                e.bool(*mutable);
                e.record(target.as_ref());
            }
            Self::FnPtr { params, result } => {
                e.u32(12);
                e.sequence(params, |e, param| e.record(param));
                e.record(result.as_ref());
            }
        }
    }
}
