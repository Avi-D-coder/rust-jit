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
use crate::runtime::object::MemoryRegion;
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
    /// Declared field names of the first variant (`None`: positional).
    pub field_names: Vec<Option<String>>,
    /// The last declared field is typed by a `?Sized` parameter, so the struct
    /// is laid out as `StructKind::MaybeUnsized` whatever it is instantiated
    /// with (rustc decides from the declared type).
    pub maybe_unsized_tail: bool,
    /// Declared field types per variant (a struct has one variant).
    pub variants: Vec<Vec<NominalFieldType>>,
    /// Discriminant value per variant (enums).
    pub discriminants: Vec<i128>,
    pub library: Option<LibraryLayout>,
    /// The declaration's field Types cannot be stated generically (a field
    /// projects through a parameter: `ManuallyDrop<T::Item>`). Every instance's
    /// field Types are then installed by the coordinator from the pinned
    /// owner's normalization (`install_layout_instance`); `variants` only
    /// carries the declaration's shape.
    pub instance_supplied: bool,
}

/// A nominal declaration whose facts are needed.
#[derive(Debug, Clone)]
pub(crate) struct LayoutDemand {
    pub identity: crate::core::digest::Digest,
    pub definition: DefKey,
    /// The nominal descriptor whose layout asked.
    pub instance: TypeId,
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

    /// Whether the declaration's facts are installed.
    pub(crate) fn layout_declaration_installed(&self, identity: crate::core::digest::Digest) -> bool {
        self.layouts.declarations.contains_key(&identity)
    }
    /// Install the instantiated field Types of one nominal descriptor whose
    /// declaration is `instance_supplied`.
    pub(crate) fn install_layout_instance(&mut self, ty: TypeId, variants: Vec<Vec<TypeId>>) {
        self.layouts.instances.insert(ty, Arc::new(variants));
    }

    /// Whether a pointer to `target` is a fat pointer.
    pub(super) fn unsized_pointee(&self, target: TypeId) -> bool {
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
                            instance: ty,
                        }));
                    };
                    // Library types stated directly never look at their fields.
                    if matches!(
                        declaration.library.map(|l| l.shape),
                        Some(LibraryShape::Pointer { .. } | LibraryShape::Fixed { .. })
                    ) {
                        continue;
                    }
                    if declaration.instance_supplied && !self.layouts.instances.contains_key(&ty) {
                        return Ok(Some(LayoutDemand {
                            identity: *identity,
                            definition: definition.clone(),
                            instance: ty,
                        }));
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
                Ok(algorithm::univariant(&members, &Repr::default(), StructKind::MaybeUnsized).layout)
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

    pub(super) fn layouts_of(&self, members: &[TypeId]) -> Result<Vec<Layout>, Diagnostic> {
        members.iter().map(|member| self.layout_of(*member)).collect()
    }

    pub(super) fn declaration_of(&self, ty: TypeId) -> Result<(Arc<DeclarationFacts>, Option<Arc<Vec<Vec<TypeId>>>>), Diagnostic> {
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
                if declaration.maybe_unsized_tail {
                    StructKind::MaybeUnsized
                } else {
                    StructKind::Sized
                },
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
        match self.state(cell)? {
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
    pub(crate) fn referent_discriminant(&mut self, value: ValueRepr) -> Result<ValueRepr, Diagnostic> {
        match self.referent_enum_variant(value)? {
            Some((enumeration, variant)) => self.enum_discriminant_value(enumeration, variant),
            None => ValueRepr::unsigned(TypeId(9), 0),
        }
    }
    pub(crate) fn pending_referent_discriminant(
        &self, value: ValueRepr,
    ) -> Result<Option<(TypeId, u32)>, Diagnostic> {
        match self.referent_enum_variant(value)? {
            Some((enumeration, variant)) => self.pending_discriminant(enumeration, variant),
            None => Ok(None),
        }
    }
    /// Shared checked place/variant selection; no scalar allocation or source demand.
    fn referent_enum_variant(&self, value: ValueRepr) -> Result<Option<(TypeId, u32)>, Diagnostic> {
        if value.kind != 9 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "discriminant requires a reference".into(),
            ));
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        let ReferenceLease::Location(location) = &record.lease else {
            return Ok(None);
        };
        let cell = location.final_cell.cell;
        let CellState::Live(held) = self.state(cell)? else {
            return Err(lease_fault());
        };
        if held.kind != 8 {
            return Ok(None);
        }
        let enumeration = TypeId(held.type_id);
        if !matches!(self.types.descriptor(enumeration)?.key.as_ref(),
            TypeKey::Nominal { definition, .. } if definition.item_kind == crate::source::manifest::ItemKind::Enum)
        {
            return Ok(None);
        }
        let (_, storage) = self.place_object(cell)?;
        let variant = storage.variant.ok_or_else(invalid)?;
        Ok(Some((enumeration, variant)))
    }
}

// ---- Plain data: `mem::zeroed` and `mem::transmute` ---------------------------
//
// The managed runtime has no byte-addressable memory. A *plain* type - a
// scalar, a thin pointer (raw, fn or reference), or an array, tuple, nominal
// struct or union of plain types - has a byte image under its layout, so a
// value converts to bytes and back without naming a host representation.
// `zeroed` builds the all-zero image; `transmute` is image-to-image between
// plain types of equal size. A pointer's image is its synthetic address
// (E-R5-008, `cell_pointers.rs`); reading it back as a raw or fn pointer is
// that address, and as a reference is the checked reborrow of the live cell
// of the pointee type that starts there (`pointer_from_address`), so an
// address that names nothing live never becomes a reference. Everything else
// (fat pointers, `Box`, enums, closures) has provenance or ownership the byte
// image cannot carry and is refused with the type pair.

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

    /// Whether `ty` has a byte image (see the section comment). A thin
    /// reference or raw pointer is its address; a union is its bytes.
    pub(super) fn plain_type(&self, ty: TypeId) -> bool {
        if let Ok(scalar) = primitive(ty) {
            return scalar != Primitive::Str;
        }
        match self.types.descriptor(ty).map(|d| Arc::clone(&d.key)) {
            Ok(key) => match key.as_ref() {
                TypeKey::RawPtr { target, .. } | TypeKey::Ref { target, .. } => {
                    !self.unsized_pointee(*target)
                }
                TypeKey::Function { .. } => true,
                TypeKey::Array { element, .. } => self.plain_type(*element),
                TypeKey::Tuple(members) => members.iter().all(|member| self.plain_type(*member)),
                TypeKey::Nominal { .. } => match self.union_image_members(ty) {
                    Some(members) => members.iter().all(|member| self.plain_type(*member)),
                    None => self.nominal_image_fields(ty)
                        .ok().flatten().is_some_and(|(members, _)|
                            members.iter().all(|member| self.plain_type(*member))),
                },
                _ => false,
            },
            Err(_) => false,
        }
    }

    /// The member types of a union whose declaration is installed.
    fn union_image_members(&self, ty: TypeId) -> Option<Vec<TypeId>> {
        let (declaration, instance) = self.declaration_of(ty).ok()?;
        if declaration.kind != NominalKind::Union {
            return None;
        }
        let instance = instance?;
        let [members] = instance.as_slice() else { return None; };
        Some(members.clone())
    }

    /// The byte offset of a dotted field path (`a.b.0`) from a tuple or struct
    /// (`mem::offset_of!`). Each step names a field of the current aggregate by
    /// declared name or position; the offsets are the layout algorithm's own,
    /// so they agree with `size_of`, field reads and byte images. A trailing
    /// unsized field (a slice, `str`, `dyn` or an aggregate ending in one) keeps
    /// the last position and starts at its alignment.
    pub(crate) fn offset_of_path(&self, ty: TypeId, path: &[&str]) -> Result<u64, Diagnostic> {
        let mut current = ty;
        let mut total = 0u64;
        for step in path {
            let (members, names, offsets) = self.field_placement(current)?;
            let index = match step.parse::<usize>() {
                Ok(position) if names.get(position).is_some_and(Option::is_none) => position,
                _ => names
                    .iter()
                    .position(|name| name.as_deref() == Some(*step))
                    .ok_or_else(|| {
                        layout_fault(format!("`offset_of!` field `{step}` is not a field of the type"))
                    })?,
            };
            total = total
                .checked_add(offsets[index])
                .ok_or_else(invalid)?;
            current = members[index];
        }
        Ok(total)
    }

    /// Members, field names and byte offsets of a tuple or struct.
    fn field_placement(
        &self,
        ty: TypeId,
    ) -> Result<(Vec<TypeId>, Vec<Option<String>>, Vec<u64>), Diagnostic> {
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        match key.as_ref() {
            TypeKey::Tuple(members) => {
                let offsets =
                    self.placement_offsets(members, &Repr::default(), StructKind::MaybeUnsized)?;
                Ok((members.to_vec(), vec![None; members.len()], offsets))
            }
            TypeKey::Nominal { .. } => {
                let (declaration, instance) = self.declaration_of(ty)?;
                if declaration.library.is_some_and(|library| {
                    !matches!(library.shape, LibraryShape::Fields { .. })
                }) || declaration.kind == NominalKind::Enum
                {
                    return Err(layout_fault(
                        "`offset_of!` needs a struct, union or tuple container".into(),
                    ));
                }
                let instance = instance.ok_or_else(invalid)?;
                let [members] = instance.as_slice() else {
                    return Err(invalid());
                };
                let names = if declaration.field_names.len() == members.len() {
                    declaration.field_names.clone()
                } else {
                    vec![None; members.len()]
                };
                let offsets = if declaration.kind == NominalKind::Union {
                    vec![0; members.len()]
                } else {
                    self.placement_offsets(
                        members,
                        &declaration.repr,
                        if declaration.maybe_unsized_tail {
                            StructKind::MaybeUnsized
                        } else {
                            StructKind::Sized
                        },
                    )?
                };
                Ok((members.clone(), names, offsets))
            }
            _ => Err(layout_fault(
                "`offset_of!` needs a struct, union or tuple container".into(),
            )),
        }
    }

    /// Offsets of `members` laid out as one aggregate, with an unsized last
    /// member kept last (rustc `StructKind::MaybeUnsized`).
    fn placement_offsets(
        &self,
        members: &[TypeId],
        repr: &Repr,
        kind: StructKind,
    ) -> Result<Vec<u64>, Diagnostic> {
        let unsized_tail = match members.split_last() {
            Some((last, rest)) => self.unsized_alignment(*last)?.map(|align| (rest, align)),
            None => None,
        };
        let Some((prefix, tail_align)) = unsized_tail else {
            let layouts = self.layouts_of(members)?;
            return Ok(algorithm::univariant(&layouts, repr, kind).offsets);
        };
        let layouts = self.layouts_of(prefix)?;
        let mut offsets = algorithm::univariant(&layouts, repr, StructKind::Sized).offsets;
        let end = offsets
            .iter()
            .zip(&layouts)
            .map(|(offset, layout)| offset.saturating_add(layout.size))
            .max()
            .unwrap_or(0);
        let align = repr.packed.map_or(tail_align, |pack| tail_align.min(pack));
        offsets.push(algorithm::align_up(end, align));
        Ok(offsets)
    }

    /// The alignment of a dynamically sized type (`None` for a sized one): a
    /// slice has its element's, `str` and `dyn` 1, and an aggregate ending in
    /// such a field the strictest of its sized prefix and that field.
    fn unsized_alignment(&self, ty: TypeId) -> Result<Option<u64>, Diagnostic> {
        if primitive(ty).is_ok_and(|scalar| matches!(scalar, Primitive::Str)) {
            return Ok(Some(1));
        }
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        let aggregate = |members: &[TypeId], repr: &Repr| -> Result<Option<u64>, Diagnostic> {
            let Some((last, rest)) = members.split_last() else {
                return Ok(None);
            };
            let Some(tail) = self.unsized_alignment(*last)? else {
                return Ok(None);
            };
            let prefix = algorithm::univariant(&self.layouts_of(rest)?, repr, StructKind::Sized);
            Ok(Some(prefix.layout.align.max(tail).max(repr.align.unwrap_or(1))))
        };
        match key.as_ref() {
            TypeKey::Slice { element } => Ok(Some(self.layout_of(*element)?.align)),
            TypeKey::Dyn { .. } => Ok(Some(1)),
            TypeKey::Tuple(members) => aggregate(members, &Repr::default()),
            TypeKey::Nominal { .. } => {
                let Ok((declaration, Some(instance))) = self.declaration_of(ty) else {
                    return Ok(None);
                };
                match instance.as_slice() {
                    [members] if declaration.kind == NominalKind::Struct => {
                        aggregate(members, &declaration.repr)
                    }
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// How a struct or tuple of this type is placed: a tuple, or a struct whose
    /// last declared field is a `?Sized` parameter, keeps that last field last.
    pub(super) fn struct_kind_of(&self, ty: TypeId) -> StructKind {
        match self.types.descriptor(ty).map(|descriptor| descriptor.key.as_ref()) {
            Ok(TypeKey::Tuple(_)) => StructKind::MaybeUnsized,
            Ok(TypeKey::Nominal { .. })
                if self
                    .declaration_of(ty)
                    .is_ok_and(|(declaration, _)| declaration.maybe_unsized_tail) =>
            {
                StructKind::MaybeUnsized
            }
            _ => StructKind::Sized,
        }
    }

    /// Byte offsets of the members of a plain tuple.
    fn tuple_offsets(&self, members: &[TypeId]) -> Result<Vec<u64>, Diagnostic> {
        let layouts = self.layouts_of(members)?;
        Ok(algorithm::univariant(&layouts, &Repr::default(), StructKind::MaybeUnsized).offsets)
    }

    /// Field contracts and placement from the same prepared declaration used
    /// by size/alignment queries. This grants no source constructor or Copy fact.
    pub(super) fn nominal_image_fields(&self, ty: TypeId) -> Result<Option<(Vec<TypeId>, Vec<u64>)>, Diagnostic> {
        let (declaration, instance) = self.declaration_of(ty)?;
        if declaration.kind != NominalKind::Struct
            || matches!(declaration.library.map(|library| library.shape),
                Some(LibraryShape::Pointer { .. } | LibraryShape::Fixed { .. }))
        {
            return Ok(None);
        }
        let instance = instance.ok_or_else(invalid)?;
        let [members] = instance.as_slice() else { return Err(invalid()); };
        let placement = algorithm::univariant(
            &self.layouts_of(members)?,
            &declaration.repr,
            if declaration.maybe_unsized_tail {
                StructKind::MaybeUnsized
            } else {
                StructKind::Sized
            },
        );
        Ok(Some((members.clone(), placement.offsets)))
    }

    pub(super) fn image_range(offset: u64, length: usize) -> Result<std::ops::Range<usize>, Diagnostic> {
        let start = usize::try_from(offset).map_err(|_| invalid())?;
        let end = start.checked_add(length).ok_or_else(invalid)?;
        Ok(start..end)
    }
    fn put_bytes(image: &mut [u8], offset: u64, bytes: &[u8]) -> Result<(), Diagnostic> {
        let range = Self::image_range(offset, bytes.len())?;
        image.get_mut(range).ok_or_else(invalid)?.copy_from_slice(bytes);
        Ok(())
    }

    /// Write the byte image of `value` (of plain type `ty`) at `offset`.
    pub(super) fn write_plain_image(
        &mut self,
        value: ValueRepr,
        ty: TypeId,
        offset: u64,
        image: &mut [u8],
    ) -> Result<(), Diagnostic> {
        let length = usize::try_from(self.layout_of(ty)?.size).map_err(|_| invalid())?;
        image.get(Self::image_range(offset, length)?).ok_or_else(invalid)?;
        let value = if matches!(value.kind, 11 | 12) {
            self.bound_literal(value)?
        } else {
            value
        };
        match value.kind {
            1 => {}
            2 => Self::put_bytes(image, offset, &[value.payload as u8])?,
            3 => Self::put_bytes(image, offset, &(value.payload as u32).to_le_bytes())?,
            17 => Self::put_bytes(image, offset, &self.wide_integer_bits(value)?.to_le_bytes())?,
            4 | 5 => {
                let Ok(Primitive::Signed(width) | Primitive::Unsigned(width)) =
                    primitive(TypeId(value.type_id))
                else {
                    return Err(invalid());
                };
                let bytes = value.payload.to_le_bytes();
                Self::put_bytes(image, offset, &bytes[..usize::from(width) / 8])?;
            }
            6 => Self::put_bytes(image, offset, &(value.payload as u32).to_le_bytes())?,
            7 => Self::put_bytes(image, offset, &value.payload.to_le_bytes())?,
            9 => {
                let address = self.pointer_address(value)?;
                Self::put_bytes(image, offset, &address.to_le_bytes())?;
            }
            8 if self.is_union_type(ty) => {
                // A union is its canonical bytes; none may be uninitialized
                // (the image carries no initialization mask).
                let ObjectStorage::Aggregate(storage) = &self.objects.get(ObjectId::from_bits(value.payload))?.storage else {
                    return Err(invalid());
                };
                let region = storage.memory.as_ref().ok_or_else(invalid)?;
                if region.bytes.len() != length {
                    return Err(invalid());
                }
                if !region.initialized.iter().all(|byte| *byte) {
                    return Err(Diagnostic::new(
                        "LRR-E-UNINITIALIZED",
                        "byte image of a union with uninitialized bytes".into(),
                    ));
                }
                let bytes = region.bytes.clone();
                Self::put_bytes(image, offset, &bytes)?;
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
                    TypeKey::Nominal { .. } => {
                        let (members, offsets) = self.nominal_image_fields(ty)?.ok_or_else(invalid)?;
                        let ObjectStorage::Aggregate(storage) = &self.objects.get(ObjectId::from_bits(value.payload))?.storage else { return Err(invalid()); };
                        if storage.variant != Some(0) || fields.len() != members.len() {
                            return Err(invalid());
                        }
                        for ((field, member), at) in fields.iter().zip(members).zip(offsets) {
                            let held = self.live_value(*field)?;
                            self.write_plain_image(held, member, offset + at, image)?;
                        }
                    }
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    fn image_word(image: &[u8], offset: u64, count: usize) -> Result<[u8; 8], Diagnostic> {
        let mut bytes = [0u8; 8];
        let source = image.get(Self::image_range(offset, count)?).ok_or_else(invalid)?;
        bytes.get_mut(..count).ok_or_else(invalid)?.copy_from_slice(source);
        Ok(bytes)
    }
    /// Pure scalar decoding shared by owning reconstruction and live memory views.
    pub(super) fn image_scalar(&self, ty: TypeId, offset: u64, image: &[u8]) -> Result<Option<ValueRepr>, Diagnostic> {
        if matches!(ty.0, 17 | 18) { return Ok(None); }
        let take = |count| Self::image_word(image, offset, count);
        if let Ok(scalar) = primitive(ty) {
            let value = match scalar {
                Primitive::Unit => ValueRepr::unit(),
                Primitive::Bool => {
                    let byte = take(1)?[0];
                    if byte > 1 {
                        return Err(plain_fault(format!(
                            "transmute produced the invalid bool value {byte}"
                        )));
                    }
                    ValueRepr::boolean(byte == 1)
                }
                Primitive::Char => {
                    let code = u32::from_le_bytes(take(4)?[..4].try_into().expect("four bytes"));
                    ValueRepr::character(char::from_u32(code).ok_or_else(|| {
                        plain_fault(format!("transmute produced the invalid char value {code:#x}"))
                    })?)
                }
                Primitive::F32 => ValueRepr::f32(f32::from_bits(u32::from_le_bytes(
                    take(4)?[..4].try_into().expect("four bytes"),
                ))),
                Primitive::F64 => ValueRepr::f64(f64::from_bits(u64::from_le_bytes(take(8)?))),
                Primitive::Unsigned(width) => {
                    let bytes = usize::from(width) / 8;
                    ValueRepr::unsigned(ty, u64::from_le_bytes(take(bytes)?))?
                }
                Primitive::Signed(width) => {
                    let bytes = usize::from(width) / 8;
                    let raw = u64::from_le_bytes(take(bytes)?);
                    let shift = 64 - 8 * bytes as u32;
                    ValueRepr::signed(ty, ((raw << shift) as i64) >> shift)?
                }
                Primitive::Str => return Err(invalid()),
            };
            return Ok(Some(value));
        }
        Ok(None)
    }

    /// The value of plain type `ty` whose byte image starts at `offset`.
    pub(super) fn read_plain_image(
        &mut self,
        ty: TypeId,
        offset: u64,
        image: &[u8],
    ) -> Result<TicketId, Diagnostic> {
        let length = usize::try_from(self.layout_of(ty)?.size).map_err(|_| invalid())?;
        image.get(Self::image_range(offset, length)?).ok_or_else(invalid)?;
        if let Some(value) = self.image_scalar(ty, offset, image)? {
            return self.ticket(value);
        }
        if matches!(ty.0, 17 | 18) {
            let start = usize::try_from(offset).map_err(|_| invalid())?;
            let end = start.checked_add(16).ok_or_else(invalid)?;
            let bytes: [u8; 16] = image.get(start..end).ok_or_else(invalid)?
                .try_into().map_err(|_| invalid())?;
            let value = self.wide_integer_value(ty, u128::from_le_bytes(bytes))?;
            return self.ticket(value);
        }
        let key = Arc::clone(&self.types.descriptor(ty)?.key);
        match key.as_ref() {
            TypeKey::RawPtr { .. } | TypeKey::Ref { .. } | TypeKey::Function { .. } => {
                let address = u64::from_le_bytes(Self::image_word(image, offset, 8)?);
                self.pointer_from_address(address, ty)
            }
            TypeKey::Nominal { .. } if self.is_union_type(ty) => {
                let range = Self::image_range(offset, length)?;
                let bytes = image.get(range).ok_or_else(invalid)?.to_vec();
                self.install_union_memory(
                    ty,
                    MemoryRegion {
                        initialized: vec![true; bytes.len()],
                        bytes,
                        pointers: Vec::new(),
                        leaves: Vec::new(),
                    },
                )
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
            TypeKey::Nominal { .. } => {
                let (members, offsets) = self.nominal_image_fields(ty)?.ok_or_else(invalid)?;
                let guard = self.begin_construction(&members)?;
                self.constructions.get_mut(guard)?.nominal = Some((ty, 0));
                for (index, (member, at)) in members.iter().zip(offsets).enumerate() {
                    let item = self.read_plain_image(*member, offset + at, image)?;
                    self.construction_push(guard, u32::try_from(index).map_err(|_| resource())?, item)?;
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
                "mem::zeroed::<{}> requires a supported plain byte image",
                self.describe_type(ty)
            )));
        }
        let size = self.layout_of(ty)?.size as usize;
        self.read_plain_image(ty, 0, &vec![0u8; size])
    }

    /// `mem::transmute::<Src, Dst>(value)` between plain types of equal size.
    pub(crate) fn transmute_value(
        &mut self,
        source_ticket: TicketId,
        source: TypeId,
        destination: TypeId,
    ) -> Result<TicketId, Diagnostic> {
        let value = self.peek_ticket(source_ticket)?;
        // A thin reference's byte image is its address; reading it back as a
        // reference re-borrows the live cell at that address
        // (`pointer_from_address`), so references transmute in both directions.
        if !self.plain_type(source) || !self.plain_type(destination) {
            return Err(plain_fault(format!(
                "mem::transmute::<{}, {}> requires supported byte images and pointer provenance",
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
        let result = self.read_plain_image(destination, 0, &image)?;
        // Transmute consumes bits without calling the source's Drop. Reuse the
        // physical owning-field transfer kernel, never leak an input ticket.
        self.consume_image_source(source_ticket)?;
        Ok(result)
    }
    pub(super) fn consume_image_source(&mut self, source_ticket: TicketId) -> Result<(), Diagnostic> {
        let mut sources = vec![source_ticket];
        while let Some(ticket) = sources.last().copied() {
            if self.peek_ticket(ticket)?.kind == 8 {
                match self.aggregate_cleanup_step(ticket)? {
                    AggregateCleanupStep::Field(child) => { sources.push(child); continue; }
                    AggregateCleanupStep::Skipped => continue,
                    AggregateCleanupStep::Complete => {}
                }
            }
            self.discard(ticket)?;
            sources.pop();
        }
        Ok(())
    }

}

#[cfg(test)]
mod image_storage_tests {
    use super::*;

    #[test]
    fn image_windows_fail_closed_and_transmute_consumes_its_owning_graph() {
        let mut store = GuestStore::new(1, u32::MAX, usize::MAX).unwrap();
        let byte = TypeId(9);
        let word = TypeId(11);
        let array = store.types.intern(TypeKey::Array { element: byte, len: 4 }).unwrap();
        let guard = store.begin_construction(&[byte; 4]).unwrap();
        for index in 0..4 {
            let ticket = store.ticket(ValueRepr::unsigned(byte, u64::from(index) + 1).unwrap()).unwrap();
            store.construction_push(guard, index, ticket).unwrap();
        }
        let source = store.commit_construction(guard, array).unwrap();
        let object = ObjectId::from_bits(store.peek_ticket(source).unwrap().payload);
        let result = store.transmute_value(source, array, word).unwrap();
        assert_eq!(store.peek_ticket(result).unwrap().payload, 0x04030201);
        assert!(store.peek_ticket(source).is_err());
        assert!(store.objects.get(object).is_err());
        let owners = store.owner_inventory_for_test();
        assert_eq!(owners.tickets, vec![result.bits()]);
        assert!(owners.objects.is_empty());
        assert!(owners.guards.is_empty());
        assert!(owners.cells.iter().all(|(_, state)| *state == CellState::Dropped));
        store.discard(result).unwrap();
        let before = store.owner_inventory_for_test();
        for offset in [0, 1, u64::MAX] {
            let mut image = [0x55; 3];
            assert!(store.write_plain_image(ValueRepr::unsigned(word, 7).unwrap(), word, offset, &mut image).is_err());
            assert_eq!(image, [0x55; 3]);
            assert!(store.read_plain_image(word, offset, &image).is_err());
            assert_eq!(store.owner_inventory_for_test(), before);
        }
        assert!(GuestStore::put_bytes(&mut [], 1, &[]).is_err());
        GuestStore::put_bytes(&mut [], 0, &[]).unwrap();
        let unit = store.read_plain_image(TypeId(1), 0, &[]).unwrap();
        store.discard(unit).unwrap();
        assert!(store.read_plain_image(TypeId(1), 1, &[]).is_err());
    }

    /// A writable `u8` local holding `value`.
    fn byte_local(store: &mut GuestStore, value: u64) -> CellId {
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(TypeId(9)),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        let ticket = store.ticket(ValueRepr::unsigned(TypeId(9), value).unwrap()).unwrap();
        store.initialize(cell, ticket).unwrap();
        cell
    }
    fn reference_type(store: &mut GuestStore, mutable: bool) -> TypeId {
        store.intern_type(TypeKey::Ref { mutable, target: TypeId(9) }).unwrap()
    }

    #[test]
    fn thin_references_round_trip_through_address_images_as_checked_reborrows() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let cell = byte_local(&mut store, 5);
        let shared = reference_type(&mut store, false);
        let ticket = store.borrow_ticket(&Place::cell(cell), Access::Shared).unwrap();
        let value = store.peek_ticket(ticket).unwrap();
        assert!(store.plain_type(shared));
        let mut image = [0u8; 8];
        store.write_plain_image(value, shared, 0, &mut image).unwrap();
        assert_eq!(u64::from_le_bytes(image), store.cell_address(cell).unwrap());
        // The image names the cell; reading it re-borrows that same location.
        let back = store.read_plain_image(shared, 0, &image).unwrap();
        let back_value = store.peek_ticket(back).unwrap();
        assert_eq!(TypeId(back_value.type_id), shared);
        let id = RefId::from_bits(back_value.payload);
        assert_eq!(store.read_place(&Place::deref(id)).unwrap().payload, 5);
        // The recovered reference is bound to the location, not to its bytes:
        // a dead location is reported, never silently resurrected.
        store.discard(back).unwrap();
        store.discard(ticket).unwrap();
        assert!(!store.plain_type(TypeId(16)));
        let dead = byte_local(&mut store, 6);
        let dead_address = store.cell_address(dead).unwrap();
        let moved = store.move_from(dead).unwrap();
        store.discard(moved).unwrap();
        store.retire_cell(dead).unwrap();
        let error = store
            .read_plain_image(shared, 0, &dead_address.to_le_bytes())
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-POINTER-DANGLING");
        for address in [0u64, 1, dead_address + 1] {
            assert_eq!(
                store.read_plain_image(shared, 0, &address.to_le_bytes()).unwrap_err().code,
                "LRR-E-POINTER-DANGLING"
            );
        }
        // A cell of another pointee type at that address is not a `&u8`.
        let wide = store.intern_type(TypeKey::Ref { mutable: false, target: TypeId(11) }).unwrap();
        let live = store.cell_address(cell).unwrap();
        assert_eq!(
            store.read_plain_image(wide, 0, &live.to_le_bytes()).unwrap_err().code,
            "LRR-E-POINTER-DANGLING"
        );
    }

    #[test]
    fn mutable_references_recover_one_owned_lease_from_an_address_image() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let cell = byte_local(&mut store, 1);
        let unique = reference_type(&mut store, true);
        let address = store.cell_address(cell).unwrap();
        let back = store.read_plain_image(unique, 0, &address.to_le_bytes()).unwrap();
        let value = store.peek_ticket(back).unwrap();
        assert_eq!(TypeId(value.type_id), unique);
        let id = RefId::from_bits(value.payload);
        let nine = store.ticket(ValueRepr::unsigned(TypeId(9), 9).unwrap()).unwrap();
        let old = store.replace_place(&Place::deref(id), nine).unwrap();
        store.discard(old).unwrap();
        assert_eq!(store.read_place(&Place::cell(cell)).unwrap().payload, 9);
        // Writing the lease back to bytes consumes it exactly once.
        let mut image = [0u8; 8];
        store.write_plain_image(value, unique, 0, &mut image).unwrap();
        assert_eq!(u64::from_le_bytes(image), address);
        store.discard(back).unwrap();
        assert!(store.read_place(&Place::deref(id)).is_err());
    }
}
