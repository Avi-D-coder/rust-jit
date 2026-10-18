//! Layout facts and the `core::mem` layout queries of the guest store
//! (E-R5-012).
//!
//! `runtime/layout.rs` is the one layout algorithm. This module feeds it: the
//! members of a tuple, array, closure or pointer come from type descriptors,
//! and a nominal type's field types, `repr` and discriminants come from its
//! declaration, which the engine demands once per declaration
//! (`engine/run_layout.rs`) and installs here. A declaration's field types are
//! instantiated with a descriptor's type arguments the first time that
//! descriptor is demanded, so the pure queries below never touch the source
//! database.
use super::*;
use crate::{
    frontend::types::NominalFieldType,
    runtime::{
        layout::{self as algorithm, Layout, Niche, Repr, StructKind},
        shape::{FacadeType, Primitive, primitive},
    },
    source::manifest::DefKey,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum NominalKind {
    Struct,
    Enum,
    Union,
}

/// How a library type's layout relates to its facade declaration. The facade
/// mirrors std's behavior, not always its fields, so a few types state std's
/// real layout directly (`facade/library_layouts.rs`).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LibraryShape {
    /// Laid out from its fields like any declaration; `strip_niche` hides the
    /// niche (`UnsafeCell`), `nonzero` states a one-value niche (`NonZero`).
    Fields { strip_niche: bool, nonzero: bool },
    /// A pointer to its first type argument: thin, or fat when that argument
    /// is unsized (`Box`, `Rc`, `Arc`, `NonNull`).
    Pointer { nonnull: bool },
    /// std's real size, alignment and unused-pattern count.
    Fixed { size: u64, align: u64, niche: u128 },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct LibraryLayout {
    pub shape: LibraryShape,
    /// Overrides the drop glue the facade fields would give.
    pub needs_drop: Option<bool>,
}

/// What layout needs from one nominal declaration.
#[derive(Debug)]
pub(crate) struct DeclarationFacts {
    pub kind: NominalKind,
    pub repr: Repr,
    /// Declared field types per variant (a struct has one variant).
    pub variants: Vec<Vec<NominalFieldType>>,
    /// Discriminant value per variant (enums).
    pub discriminants: Vec<i128>,
    pub library: Option<LibraryLayout>,
}

/// A nominal declaration whose facts are needed.
#[derive(Debug, Clone)]
pub(crate) struct LayoutDemand {
    pub identity: crate::core::digest::Digest,
    pub definition: DefKey,
}

#[derive(Default)]
pub(super) struct LayoutTable {
    declarations: BTreeMap<crate::core::digest::Digest, Arc<DeclarationFacts>>,
    /// Instantiated field types per nominal descriptor, per variant.
    instances: BTreeMap<TypeId, Arc<Vec<Vec<TypeId>>>>,
}

fn layout_fault(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}
fn unsized_fault(what: &str) -> Diagnostic {
    Diagnostic::new(
        "LRR-E-OP-TYPE",
        format!("the size of an unsized {what} is only known through a value"),
    )
}

impl GuestStore {
    /// Install a declaration's facts.
    pub(crate) fn install_layout_declaration(
        &mut self,
        identity: crate::core::digest::Digest,
        facts: DeclarationFacts,
    ) {
        self.layouts.declarations.insert(identity, Arc::new(facts));
    }

    /// Whether a pointer to `target` is a fat pointer.
    fn unsized_pointee(&self, target: TypeId) -> bool {
        target.0 == 16
            || matches!(
                self.types.descriptor(target).map(|d| d.key.as_ref()),
                Ok(TypeKey::Slice { .. } | TypeKey::Dyn { .. })
            )
    }

    /// The first nominal declaration reachable by value from `ty` whose facts
    /// are not installed. Pointer pointees are not followed: a pointer has the
    /// same layout whatever it points to. Instantiates the field types of
    /// every nominal whose declaration is known.
    pub(crate) fn layout_demand(&mut self, ty: TypeId) -> Result<Option<LayoutDemand>, Diagnostic> {
        let mut work = vec![ty];
        let mut seen = BTreeSet::new();
        while let Some(ty) = work.pop() {
            if !seen.insert(ty) {
                continue;
            }
            let key = Arc::clone(&self.types.descriptor(ty)?.key);
            match key.as_ref() {
                TypeKey::Array { element, .. } | TypeKey::Slice { element } => work.push(*element),
                TypeKey::Tuple(members) => work.extend(members.iter().copied()),
                TypeKey::Callable { captures, .. } => work.extend(captures.iter().copied()),
                TypeKey::Nominal {
                    identity,
                    definition,
                    args,
                    ..
                } => {
                    let Some(declaration) = self.layouts.declarations.get(identity).cloned() else {
                        return Ok(Some(LayoutDemand {
                            identity: *identity,
                            definition: definition.clone(),
                        }));
                    };
                    // Library types stated directly never look at their fields.
                    if matches!(
                        declaration.library.map(|l| l.shape),
                        Some(LibraryShape::Pointer { .. } | LibraryShape::Fixed { .. })
                    ) {
                        continue;
                    }
                    if !self.layouts.instances.contains_key(&ty) {
                        let mut variants = Vec::with_capacity(declaration.variants.len());
                        for variant in &declaration.variants {
                            let mut fields = Vec::with_capacity(variant.len());
                            for field in variant {
                                fields.push(field.materialize(self, args)?);
                            }
                            variants.push(fields);
                        }
                        self.layouts.instances.insert(ty, Arc::new(variants));
                    }
                    for variant in self.layouts.instances[&ty].iter() {
                        work.extend(variant.iter().copied());
                    }
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// The layout of a sized type. Every declaration reachable by value must
    /// have been demanded (`layout_demand`).
    pub(crate) fn layout_of(&self, ty: TypeId) -> Result<Layout, Diagnostic> {
        if let Ok(scalar) = primitive(ty) {
            return match scalar {
                Primitive::Unit => Ok(Layout::ZERO_SIZED),
                Primitive::Bool => Ok(Layout::bool()),
                Primitive::Char => Ok(Layout::char()),
                Primitive::F32 => Ok(Layout::scalar(4)),
                Primitive::F64 => Ok(Layout::scalar(8)),
                Primitive::Signed(width) | Primitive::Unsigned(width) => {
                    Ok(Layout::scalar(u64::from(width) / 8))
                }
                Primitive::Str => Err(unsized_fault("`str`")),
            };
        }
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        match key.as_ref() {
            TypeKey::Primitive(_) => Err(invalid()),
            TypeKey::Ref { target, .. } => Ok(Layout::pointer(self.unsized_pointee(*target), true)),
            TypeKey::RawPtr { target, .. } => {
                Ok(Layout::pointer(self.unsized_pointee(*target), false))
            }
            TypeKey::Function { .. } => Ok(Layout::pointer(false, true)),
            TypeKey::Slice { .. } => Err(unsized_fault("slice")),
            TypeKey::Dyn { .. } => Err(unsized_fault("trait object")),
            TypeKey::Array { element, len } => {
                Ok(Layout::array(self.layout_of(*element)?, u64::from(*len)))
            }
            TypeKey::Tuple(members) => {
                let members = self.layouts_of(members)?;
                Ok(algorithm::univariant(&members, &Repr::default(), StructKind::Sized).layout)
            }
            TypeKey::Callable {
                captures, borrowed, ..
            } => {
                // A by-reference capture stores the borrow: one thin pointer.
                let mut members = Vec::with_capacity(captures.len());
                for (capture, by_reference) in captures.iter().zip(borrowed.iter()) {
                    let layout = self.layout_of(*capture)?;
                    let is_pointer = matches!(
                        self.types.descriptor(*capture).map(|d| d.key.as_ref()),
                        Ok(TypeKey::Ref { .. } | TypeKey::RawPtr { .. })
                    );
                    members.push(if *by_reference && !is_pointer {
                        Layout::pointer(false, true)
                    } else {
                        layout
                    });
                }
                Ok(algorithm::univariant(&members, &Repr::default(), StructKind::Sized).layout)
            }
            TypeKey::Nominal { identity, args, .. } => self.nominal_layout(ty, identity, args),
            TypeKey::Facade { family, args } => match family {
                FacadeType::String | FacadeType::Vec => Ok(Layout {
                    size: 24,
                    align: 8,
                    niche: Some(Niche {
                        offset: 0,
                        width: 8,
                        available: 1 << 63,
                    }),
                    uninhabited: false,
                }),
                FacadeType::Box => Ok(Layout::pointer(
                    args.first().is_some_and(|a| self.unsized_pointee(*a)),
                    true,
                )),
                FacadeType::PhantomData => Ok(Layout::ZERO_SIZED),
                _ => Err(layout_fault(
                    "the layout of this built-in library type is not modeled".into(),
                )),
            },
        }
    }

    fn layouts_of(&self, members: &[TypeId]) -> Result<Vec<Layout>, Diagnostic> {
        members.iter().map(|member| self.layout_of(*member)).collect()
    }

    fn declaration_of(&self, ty: TypeId) -> Result<(Arc<DeclarationFacts>, Option<Arc<Vec<Vec<TypeId>>>>), Diagnostic> {
        let TypeKey::Nominal { identity, .. } = self.types.descriptor(ty)?.key.as_ref() else {
            return Err(invalid());
        };
        let declaration = self.layouts.declarations.get(identity).cloned().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "layout of a nominal type was queried before its declaration was demanded".into(),
            )
        })?;
        Ok((declaration, self.layouts.instances.get(&ty).cloned()))
    }

    fn nominal_layout(
        &self,
        ty: TypeId,
        _identity: &crate::core::digest::Digest,
        args: &[TypeId],
    ) -> Result<Layout, Diagnostic> {
        let (declaration, instance) = self.declaration_of(ty)?;
        let mut strip_niche = false;
        let mut nonzero = false;
        if let Some(library) = declaration.library {
            match library.shape {
                LibraryShape::Pointer { nonnull } => {
                    return Ok(Layout::pointer(
                        args.first().is_some_and(|a| self.unsized_pointee(*a)),
                        nonnull,
                    ));
                }
                LibraryShape::Fixed { size, align, niche } => {
                    return Ok(Layout {
                        size,
                        align,
                        niche: (niche > 0).then_some(Niche {
                            offset: 0,
                            width: size.min(8),
                            available: niche,
                        }),
                        uninhabited: false,
                    });
                }
                LibraryShape::Fields {
                    strip_niche: strip,
                    nonzero: non_zero,
                } => {
                    strip_niche = strip;
                    nonzero = non_zero;
                }
            }
        }
        let instance = instance.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-INTERNAL-IR",
                "layout of a nominal type was queried before its fields were instantiated".into(),
            )
        })?;
        let mut variants = Vec::with_capacity(instance.len());
        for fields in instance.iter() {
            variants.push(self.layouts_of(fields)?);
        }
        let mut layout = match declaration.kind {
            NominalKind::Struct => algorithm::univariant(
                variants.first().map_or(&[][..], Vec::as_slice),
                &declaration.repr,
                StructKind::Sized,
            )
            .layout,
            NominalKind::Union => algorithm::union_layout(
                &variants.into_iter().flatten().collect::<Vec<_>>(),
                &declaration.repr,
            ),
            NominalKind::Enum => {
                algorithm::enum_layout(&variants, &declaration.discriminants, &declaration.repr)
            }
        };
        if strip_niche {
            layout.niche = None;
        }
        if nonzero {
            layout.niche = Some(Niche {
                offset: 0,
                width: layout.size.min(8),
                available: 1,
            });
        }
        Ok(layout)
    }

    /// Whether dropping a value of `ty` runs any code (`mem::needs_drop`).
    pub(crate) fn needs_drop_type(&self, ty: TypeId) -> Result<bool, Diagnostic> {
        if primitive(ty).is_ok() {
            return Ok(false);
        }
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        Ok(match key.as_ref() {
            TypeKey::Primitive(_)
            | TypeKey::Ref { .. }
            | TypeKey::RawPtr { .. }
            | TypeKey::Function { .. } => false,
            TypeKey::Dyn { .. } => true,
            TypeKey::Slice { element } => self.needs_drop_type(*element)?,
            TypeKey::Array { element, len } => *len != 0 && self.needs_drop_type(*element)?,
            TypeKey::Tuple(members) => self.any_needs_drop(members)?,
            TypeKey::Callable {
                captures, borrowed, ..
            } => {
                let mut any = false;
                for (capture, by_reference) in captures.iter().zip(borrowed.iter()) {
                    if !*by_reference && self.needs_drop_type(*capture)? {
                        any = true;
                    }
                }
                any
            }
            TypeKey::Facade { family, args } => match family {
                FacadeType::PhantomData => false,
                FacadeType::String | FacadeType::Vec | FacadeType::Box => true,
                _ => self.any_needs_drop(args)?,
            },
            TypeKey::Nominal { identity, .. } => {
                if self
                    .user_drop_types
                    .as_ref()
                    .is_some_and(|types| types.contains(identity))
                {
                    return Ok(true);
                }
                let (declaration, instance) = self.declaration_of(ty)?;
                if let Some(answer) = declaration.library.and_then(|l| l.needs_drop) {
                    return Ok(answer);
                }
                if declaration.kind == NominalKind::Union {
                    return Ok(false);
                }
                let instance = instance.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-INTERNAL-IR",
                        "drop glue of a nominal type was queried before its fields were instantiated"
                            .into(),
                    )
                })?;
                let mut any = false;
                for variant in instance.iter() {
                    if self.any_needs_drop(variant)? {
                        any = true;
                    }
                }
                any
            }
        })
    }

    fn any_needs_drop(&self, members: &[TypeId]) -> Result<bool, Diagnostic> {
        for member in members {
            if self.needs_drop_type(*member)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Number of variants of an enum type (`mem::variant_count`).
    pub(crate) fn variant_count_of(&self, ty: TypeId) -> Result<u64, Diagnostic> {
        let TypeKey::Nominal { definition, .. } = self.types.descriptor(ty)?.key.as_ref() else {
            return Err(layout_fault("variant_count requires an enum type".into()));
        };
        if definition.item_kind != crate::source::manifest::ItemKind::Enum {
            return Err(layout_fault("variant_count requires an enum type".into()));
        }
        let (declaration, _) = self.declaration_of(ty)?;
        Ok(declaration.variants.len() as u64)
    }

    /// The `repr` of a nominal type whose declaration facts are known; the
    /// default otherwise.
    pub(crate) fn declared_repr(&self, ty: TypeId) -> Repr {
        match self.types.descriptor(ty).map(|d| d.key.as_ref()) {
            Ok(TypeKey::Nominal { identity, .. }) => self
                .layouts
                .declarations
                .get(identity)
                .map_or_else(Repr::default, |declaration| declaration.repr),
            _ => Repr::default(),
        }
    }

    // ---- Values -------------------------------------------------------------

    /// The value a cell holds, without moving or copying it.
    pub(crate) fn live_value(&self, cell: CellId) -> Result<ValueRepr, Diagnostic> {
        match self.cells.get(cell)?.state {
            CellState::Live(value) => Ok(value),
            _ => Err(lease_fault()),
        }
    }

    /// The sized type whose layout `size_of_val`/`align_of_val` of the
    /// reference `value` needs: the pointee, a slice's element, or - behind a
    /// `dyn` pointee, which keeps its value's concrete type - the type the
    /// designated cell holds. `None` for `str` (bytes).
    pub(crate) fn referent_sized_type(&self, value: ValueRepr) -> Result<Option<TypeId>, Diagnostic> {
        if value.kind != 9 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "size_of_val/align_of_val require a reference".into(),
            ));
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        let target = record.target;
        if target.0 == 16 {
            return Ok(None);
        }
        let key = Arc::clone(&self.types.descriptor(target)?.key);
        match key.as_ref() {
            TypeKey::Slice { element } => Ok(Some(*element)),
            TypeKey::Dyn { .. } => match &record.lease {
                ReferenceLease::Location(location) => {
                    Ok(Some(self.address_type(location.final_cell.cell)?))
                }
                _ => Err(layout_fault(
                    "size_of_val of a trait object requires its location".into(),
                )),
            },
            _ => Ok(Some(target)),
        }
    }

    /// `(size, align)` of the value a reference designates.
    pub(crate) fn referent_layout(&self, value: ValueRepr) -> Result<(u64, u64), Diagnostic> {
        let record = self.references.get(RefId::from_bits(value.payload))?;
        let target = record.target;
        let Some(sized) = self.referent_sized_type(value)? else {
            return Ok((self.referent_len(value)?, 1));
        };
        let layout = self.layout_of(sized)?;
        if matches!(
            self.types.descriptor(target).map(|d| d.key.as_ref()),
            Ok(TypeKey::Slice { .. })
        ) {
            return Ok((
                layout.size.saturating_mul(self.referent_len(value)?),
                layout.align,
            ));
        }
        Ok((layout.size, layout.align))
    }

    /// `mem::discriminant`: the discriminant value of the enum value a
    /// reference designates. Any other value has discriminant 0.
    pub(crate) fn referent_discriminant(&self, value: ValueRepr) -> Result<i64, Diagnostic> {
        if value.kind != 9 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "discriminant requires a reference".into(),
            ));
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        let ReferenceLease::Location(location) = &record.lease else {
            return Ok(0);
        };
        let cell = location.final_cell.cell;
        let CellState::Live(held) = self.cells.get(cell)?.state else {
            return Err(lease_fault());
        };
        if held.kind != 8 {
            return Ok(0);
        }
        let (_, storage) = self.place_object(cell)?;
        let Some(variant) = storage.variant else {
            return Ok(0);
        };
        let table = self.types.descriptor(TypeId(held.type_id))?.discriminants.clone();
        let declared = table
            .as_ref()
            .and_then(|table| table.get(variant as usize).copied().flatten());
        Ok(declared.map_or(i64::from(variant), |value| value as i64))
    }
}

// ---- Plain data: `mem::zeroed` and `mem::transmute` ---------------------------
//
// The managed runtime has no byte-addressable memory. A *plain* type - a
// scalar, a raw pointer, or an array or tuple of plain types - has a byte
// image under its layout, so a value converts to bytes and back without
// naming a host representation. `zeroed` builds the all-zero image;
// `transmute` is image-to-image between plain types of equal size. Everything
// else (references, `Box`, nominal types, closures) has provenance or
// ownership the byte image cannot carry and is refused with the type pair.

fn plain_fault(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}

impl GuestStore {
    /// A short, stable spelling of a type for diagnostics.
    pub(crate) fn describe_type(&self, ty: TypeId) -> String {
        // `isize`/`usize` have their own descriptors beside `i64`/`u64`.
        match ty.0 {
            8 => return "isize".into(),
            13 => return "usize".into(),
            _ => {}
        }
        if let Ok(scalar) = primitive(ty) {
            return match scalar {
                Primitive::Unit => "()".into(),
                Primitive::Bool => "bool".into(),
                Primitive::Char => "char".into(),
                Primitive::F32 => "f32".into(),
                Primitive::F64 => "f64".into(),
                Primitive::Str => "str".into(),
                Primitive::Signed(width) => format!("i{width}"),
                Primitive::Unsigned(width) => format!("u{width}"),
            };
        }
        match self.types.descriptor(ty).map(|d| Arc::clone(&d.key)) {
            Ok(key) => match key.as_ref() {
                TypeKey::Ref { mutable, target } => format!(
                    "&{}{}",
                    if *mutable { "mut " } else { "" },
                    self.describe_type(*target)
                ),
                TypeKey::RawPtr { mutable, target } => format!(
                    "*{} {}",
                    if *mutable { "mut" } else { "const" },
                    self.describe_type(*target)
                ),
                TypeKey::Slice { element } => format!("[{}]", self.describe_type(*element)),
                TypeKey::Array { element, len } => {
                    format!("[{}; {len}]", self.describe_type(*element))
                }
                TypeKey::Tuple(members) => format!(
                    "({})",
                    members
                        .iter()
                        .map(|member| self.describe_type(*member))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TypeKey::Nominal {
                    definition, args, ..
                } => {
                    if args.is_empty() {
                        definition.declared_name.clone()
                    } else {
                        format!(
                            "{}<{}>",
                            definition.declared_name,
                            args.iter()
                                .map(|arg| self.describe_type(*arg))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                }
                TypeKey::Function { .. } => "fn pointer".into(),
                TypeKey::Callable { .. } => "closure or fn item".into(),
                TypeKey::Dyn { .. } => "dyn Trait".into(),
                _ => "library type".into(),
            },
            Err(_) => "unknown type".into(),
        }
    }

    /// Whether `ty` has a byte image (see the section comment).
    fn plain_type(&self, ty: TypeId) -> bool {
        if let Ok(scalar) = primitive(ty) {
            return scalar != Primitive::Str;
        }
        match self.types.descriptor(ty).map(|d| Arc::clone(&d.key)) {
            Ok(key) => match key.as_ref() {
                TypeKey::RawPtr { target, .. } => !self.unsized_pointee(*target),
                TypeKey::Array { element, .. } => self.plain_type(*element),
                TypeKey::Tuple(members) => members.iter().all(|member| self.plain_type(*member)),
                _ => false,
            },
            Err(_) => false,
        }
    }

    /// Byte offsets of the members of a plain tuple.
    fn tuple_offsets(&self, members: &[TypeId]) -> Result<Vec<u64>, Diagnostic> {
        let layouts = self.layouts_of(members)?;
        Ok(algorithm::univariant(&layouts, &Repr::default(), StructKind::Sized).offsets)
    }

    fn put_bytes(image: &mut [u8], offset: u64, bytes: &[u8]) {
        let start = offset as usize;
        if let Some(window) = image.get_mut(start..start + bytes.len()) {
            window.copy_from_slice(bytes);
        }
    }

    /// Write the byte image of `value` (of plain type `ty`) at `offset`.
    fn write_plain_image(
        &mut self,
        value: ValueRepr,
        ty: TypeId,
        offset: u64,
        image: &mut [u8],
    ) -> Result<(), Diagnostic> {
        let value = if matches!(value.kind, 11 | 12) {
            self.bound_literal(value)?
        } else {
            value
        };
        match value.kind {
            1 => {}
            2 => Self::put_bytes(image, offset, &[value.payload as u8]),
            3 => Self::put_bytes(image, offset, &(value.payload as u32).to_le_bytes()),
            17 => Self::put_bytes(image, offset, &self.wide_integer_bits(value)?.to_le_bytes()),
            4 | 5 => {
                let Ok(Primitive::Signed(width) | Primitive::Unsigned(width)) =
                    primitive(TypeId(value.type_id))
                else {
                    return Err(invalid());
                };
                let bytes = value.payload.to_le_bytes();
                Self::put_bytes(image, offset, &bytes[..usize::from(width) / 8]);
            }
            6 => Self::put_bytes(image, offset, &(value.payload as u32).to_le_bytes()),
            7 => Self::put_bytes(image, offset, &value.payload.to_le_bytes()),
            9 => {
                let address = self.pointer_address(value)?;
                Self::put_bytes(image, offset, &address.to_le_bytes());
            }
            8 => {
                let key = Arc::clone(&self.types.descriptor(ty)?.key);
                let fields: Vec<CellId> = match &self.objects.get(ObjectId::from_bits(value.payload))?.storage {
                    ObjectStorage::Aggregate(storage) => storage.fields.clone(),
                    _ => return Err(invalid()),
                };
                match key.as_ref() {
                    TypeKey::Array { element, .. } => {
                        let stride = self.layout_of(*element)?.size;
                        for (index, field) in fields.iter().enumerate() {
                            let held = self.live_value(*field)?;
                            self.write_plain_image(held, *element, offset + stride * index as u64, image)?;
                        }
                    }
                    TypeKey::Tuple(members) => {
                        let offsets = self.tuple_offsets(members)?;
                        for ((field, member), at) in fields.iter().zip(members.iter()).zip(offsets) {
                            let held = self.live_value(*field)?;
                            self.write_plain_image(held, *member, offset + at, image)?;
                        }
                    }
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// The value of plain type `ty` whose byte image starts at `offset`.
    fn read_plain_image(
        &mut self,
        ty: TypeId,
        offset: u64,
        image: &[u8],
    ) -> Result<TicketId, Diagnostic> {
        let take = |count: usize| -> [u8; 8] {
            let mut bytes = [0u8; 8];
            let start = offset as usize;
            if let Some(window) = image.get(start..start + count) {
                bytes[..count].copy_from_slice(window);
            }
            bytes
        };
        if matches!(ty.0, 17 | 18) {
            let start = usize::try_from(offset).map_err(|_| invalid())?;
            let end = start.checked_add(16).ok_or_else(invalid)?;
            let bytes: [u8; 16] = image.get(start..end).ok_or_else(invalid)?
                .try_into().map_err(|_| invalid())?;
            let value = self.wide_integer_value(ty, u128::from_le_bytes(bytes))?;
            return self.ticket(value);
        }
        if let Ok(scalar) = primitive(ty) {
            let value = match scalar {
                Primitive::Unit => ValueRepr::unit(),
                Primitive::Bool => {
                    let byte = take(1)[0];
                    if byte > 1 {
                        return Err(plain_fault(format!(
                            "transmute produced the invalid bool value {byte}"
                        )));
                    }
                    ValueRepr::boolean(byte == 1)
                }
                Primitive::Char => {
                    let code = u32::from_le_bytes(take(4)[..4].try_into().expect("four bytes"));
                    ValueRepr::character(char::from_u32(code).ok_or_else(|| {
                        plain_fault(format!("transmute produced the invalid char value {code:#x}"))
                    })?)
                }
                Primitive::F32 => ValueRepr::f32(f32::from_bits(u32::from_le_bytes(
                    take(4)[..4].try_into().expect("four bytes"),
                ))),
                Primitive::F64 => ValueRepr::f64(f64::from_bits(u64::from_le_bytes(take(8)))),
                Primitive::Unsigned(width) => {
                    let bytes = usize::from(width) / 8;
                    ValueRepr::unsigned(ty, u64::from_le_bytes(take(bytes)))?
                }
                Primitive::Signed(width) => {
                    let bytes = usize::from(width) / 8;
                    let raw = u64::from_le_bytes(take(bytes));
                    let shift = 64 - 8 * bytes as u32;
                    ValueRepr::signed(ty, ((raw << shift) as i64) >> shift)?
                }
                Primitive::Str => return Err(invalid()),
            };
            return self.ticket(value);
        }
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        match key.as_ref() {
            TypeKey::RawPtr { .. } => {
                let address = u64::from_le_bytes(take(8));
                let integer = ValueRepr::unsigned(TypeId(13), address)?;
                let pointer = self.pointer_cast(integer, ty)?;
                self.ticket(pointer)
            }
            TypeKey::Array { element, len } => {
                let stride = self.layout_of(*element)?.size;
                let types = vec![*element; *len as usize];
                let guard = self.begin_construction(&types)?;
                for index in 0..*len {
                    let item = self.read_plain_image(*element, offset + stride * u64::from(index), image)?;
                    self.construction_push(guard, index, item)?;
                }
                self.commit_construction(guard, ty)
            }
            TypeKey::Tuple(members) => {
                let offsets = self.tuple_offsets(members)?;
                let guard = self.begin_construction(members)?;
                for (index, (member, at)) in members.iter().zip(offsets).enumerate() {
                    let item = self.read_plain_image(*member, offset + at, image)?;
                    self.construction_push(guard, index as u32, item)?;
                }
                self.commit_construction(guard, ty)
            }
            _ => Err(invalid()),
        }
    }

    /// `mem::zeroed::<T>()` for a plain `T`.
    pub(crate) fn zeroed_value(&mut self, ty: TypeId) -> Result<TicketId, Diagnostic> {
        if !self.plain_type(ty) {
            return Err(plain_fault(format!(
                "mem::zeroed::<{}> is outside the managed runtime's value model (only scalars, raw pointers and arrays or tuples of them)",
                self.describe_type(ty)
            )));
        }
        let size = self.layout_of(ty)?.size as usize;
        self.read_plain_image(ty, 0, &vec![0u8; size])
    }

    /// `mem::transmute::<Src, Dst>(value)` between plain types of equal size.
    pub(crate) fn transmute_value(
        &mut self,
        value: ValueRepr,
        source: TypeId,
        destination: TypeId,
    ) -> Result<TicketId, Diagnostic> {
        // A thin reference is accepted as a source: its byte image is its
        // address (`transmute::<&T, usize>`). Rebuilding a reference from
        // bytes has no provenance to restore, so it is not a destination.
        let thin_reference = matches!(
            self.types.descriptor(source).map(|d| d.key.as_ref()),
            Ok(TypeKey::Ref { target, .. }) if !self.unsized_pointee(*target)
        );
        if !(self.plain_type(source) || thin_reference) || !self.plain_type(destination) {
            return Err(plain_fault(format!(
                "mem::transmute::<{}, {}> is outside the managed runtime's value model (only scalars, raw pointers and arrays or tuples of them transmute)",
                self.describe_type(source),
                self.describe_type(destination)
            )));
        }
        let (from, to) = (self.layout_of(source)?.size, self.layout_of(destination)?.size);
        if from != to {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                format!(
                    "cannot transmute between types of different sizes: {} is {from} bytes, {} is {to} bytes",
                    self.describe_type(source),
                    self.describe_type(destination)
                ),
            ));
        }
        let mut image = vec![0u8; from as usize];
        self.write_plain_image(value, source, 0, &mut image)?;
        self.read_plain_image(destination, 0, &image)
    }
}
