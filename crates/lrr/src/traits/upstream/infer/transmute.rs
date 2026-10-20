//! Pinned full transmutability compiler client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_transmute.py.
use super::const_eval::TypingEnv;
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Region, Ty};
use crate::traits::upstream::{
    facts::SourceFacts as _,
    identity::DefinitionId,
    model::{self, AdtDefinition, AdtKind, ErrorToken, ScalarInt},
};
use rustc_abi::{
    FIRST_VARIANT, FieldIdx, FieldsShape, LayoutData, Primitive, Size, TagEncoding,
    TargetDataLayout, VariantIdx, Variants,
};
use rustc_transmute::{
    Answer, Reason,
    layout::Reference,
    layout::tree::Tree as RawTree,
    maybe_transmutable::{MaybeTransmutableQuery, query_context::QueryContext},
};
use rustc_type_ir::solve::{Certainty, NoSolution};
use rustc_type_ir::{self as ty, Interner, TypeVisitableExt, ValTreeKind, inherent::*};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    ops::Deref,
};
type Tree<'db> = RawTree<Def<'db>, TransmuteRegion<'db>, TransmuteTy<'db>>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TransmuteRegion<'db>(pub(crate) Region<'db>);
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TransmuteTy<'db>(pub(crate) Ty<'db>);
impl rustc_transmute::layout::Region for TransmuteRegion<'_> {}
impl rustc_transmute::layout::Type for TransmuteTy<'_> {}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) enum Def<'db> {
    Adt(AdtDefinition<'db, PreparedProvider<'db>>),
    Variant {
        owner: AdtDefinition<'db, PreparedProvider<'db>>,
        index: VariantIdx,
    },
    Field {
        owner: AdtDefinition<'db, PreparedProvider<'db>>,
        variant: VariantIdx,
        index: FieldIdx,
    },
    Primitive,
}
impl rustc_transmute::layout::Def for Def<'_> {
    fn has_safety_invariants(&self) -> bool {
        // Conservatively treat every ADT, variant, and field definition as
        // potentially carrying safety invariants. This check does not
        // inspect `unsafe` field annotations.
        self != &Self::Primitive
    }
}
impl<'db> QueryContext for DbInterner<'db> {
    type Def = Def<'db>;
    type Region = TransmuteRegion<'db>;
    type Type = TransmuteTy<'db>;
}
/// Same interned immutable ABI LayoutData handle, without compiler-only traits.
#[derive(Debug, Copy, Clone)]
pub(crate) struct Layout<'db>(pub(crate) &'db LayoutData<FieldIdx, VariantIdx>);
impl PartialEq for Layout<'_> {
    fn eq(&self, b: &Self) -> bool {
        std::ptr::eq(self.0, b.0)
    }
}
impl Eq for Layout<'_> {}
impl Hash for Layout<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        std::ptr::from_ref(self.0).hash(h)
    }
}
impl<'db> Deref for Layout<'db> {
    type Target = LayoutData<FieldIdx, VariantIdx>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl<'db> Layout<'db> {
    fn fields(self) -> &'db FieldsShape<FieldIdx> {
        &self.0.fields
    }
    fn variants(self) -> &'db Variants<FieldIdx, VariantIdx> {
        &self.0.variants
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum LayoutError {
    Unknown,
    ReferencesError,
    TooGeneric,
    InvalidSimd,
    NormalizationFailure,
    SizeOverflow,
}
#[derive(Debug, Copy, Clone)]
enum Err<'db> {
    /// The layout of the type is not yet supported.
    NotYetSupported,
    /// This error will be surfaced elsewhere by rustc, so don't surface it.
    UnknownLayout,
    /// Overflow size
    SizeOverflow,
    TypeError(ErrorToken),
    Preparation(TransmuteDependency<'db>),
}
impl<'db> From<&LayoutError> for Err<'db> {
    fn from(err: &LayoutError) -> Self {
        match err {
            LayoutError::Unknown
            | LayoutError::ReferencesError
            | LayoutError::TooGeneric
            | LayoutError::InvalidSimd
            | LayoutError::NormalizationFailure => Self::UnknownLayout,
            LayoutError::SizeOverflow => Self::SizeOverflow,
        }
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct LayoutQuery<'db> {
    pub(crate) typing_env: TypingEnv<'db>,
    pub(crate) ty: Ty<'db>,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct VariantQuery<'db> {
    pub(crate) base: LayoutQuery<'db>,
    pub(crate) parent: Layout<'db>,
    pub(crate) index: VariantIdx,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TagQuery<'db> {
    pub(crate) base: LayoutQuery<'db>,
    pub(crate) index: VariantIdx,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) enum TransmuteDependency<'db> {
    Layout(LayoutQuery<'db>),
    Variant(VariantQuery<'db>),
    Tag(TagQuery<'db>),
    Primitive(Primitive),
    Discriminants(DefinitionId),
}
#[derive(Debug, Copy, Clone)]
pub(crate) struct Discriminant<'db> {
    pub(crate) index: VariantIdx,
    pub(crate) value: u128,
    pub(crate) ty: Ty<'db>,
}
/// Typed immutable query data only: no constructor grants complete-source/root readiness.
pub(crate) struct PreparedTransmuteInputs<'db> {
    pub(crate) target: &'db TargetDataLayout,
    pub(crate) transmute_opts: Option<DefinitionId>,
    pub(crate) layouts: HashMap<LayoutQuery<'db>, Result<Layout<'db>, LayoutError>>,
    pub(crate) variants: HashMap<VariantQuery<'db>, Layout<'db>>,
    pub(crate) tags: HashMap<TagQuery<'db>, Option<ScalarInt>>,
    pub(crate) primitives: HashMap<Primitive, Ty<'db>>,
    pub(crate) discriminants: HashMap<DefinitionId, &'db [Discriminant<'db>]>,
}
#[derive(Copy, Clone)]
struct LayoutCx<'db> {
    tcx: DbInterner<'db>,
    typing_env: TypingEnv<'db>,
}
impl<'db> LayoutCx<'db> {
    fn new(tcx: DbInterner<'db>, typing_env: TypingEnv<'db>) -> Self {
        Self { tcx, typing_env }
    }
    fn tcx(self) -> DbInterner<'db> {
        self.tcx
    }
    fn data_layout(self) -> &'db TargetDataLayout {
        self.tcx.services().transmute_inputs().target
    }
    fn query(self, ty: Ty<'db>) -> LayoutQuery<'db> {
        LayoutQuery {
            typing_env: self.typing_env,
            ty: self.tcx.erase_and_anonymize_regions(ty),
        }
    }
    fn variant_layout(
        self,
        ty: Ty<'db>,
        parent: Layout<'db>,
        index: VariantIdx,
    ) -> Result<Layout<'db>, Err<'db>> {
        let query = VariantQuery {
            base: self.query(ty),
            parent,
            index,
        };
        self.tcx
            .services()
            .transmute_inputs()
            .variants
            .get(&query)
            .copied()
            .ok_or(Err::Preparation(TransmuteDependency::Variant(query)))
    }
    fn tag_for_variant(
        self,
        ty: Ty<'db>,
        index: VariantIdx,
    ) -> Result<Option<ScalarInt>, Err<'db>> {
        let query = TagQuery {
            base: self.query(ty),
            index,
        };
        self.tcx
            .services()
            .transmute_inputs()
            .tags
            .get(&query)
            .copied()
            .ok_or(Err::Preparation(TransmuteDependency::Tag(query)))
    }
    fn primitive_type(self, primitive: Primitive) -> Result<Ty<'db>, Err<'db>> {
        self.tcx
            .services()
            .transmute_inputs()
            .primitives
            .get(&primitive)
            .copied()
            .ok_or(Err::Preparation(TransmuteDependency::Primitive(primitive)))
    }
    fn discriminants(
        self,
        adt: AdtDefinition<'db, PreparedProvider<'db>>,
    ) -> Result<&'db [Discriminant<'db>], Err<'db>> {
        self.tcx
            .services()
            .transmute_inputs()
            .discriminants
            .get(&adt.definition)
            .copied()
            .ok_or(Err::Preparation(TransmuteDependency::Discriminants(
                adt.definition,
            )))
    }
}
fn layout_of<'db>(cx: LayoutCx<'db>, ty: Ty<'db>) -> Result<Layout<'db>, Err<'db>> {
    let query = cx.query(ty);
    cx.tcx
        .services()
        .transmute_inputs()
        .layouts
        .get(&query)
        .copied()
        .ok_or(Err::Preparation(TransmuteDependency::Layout(query)))?
        .map_err(|error| Err::from(&error))
}
impl ScalarInt {
    const TRUE: ScalarInt = ScalarInt {
        bits: 1_u128,
        bytes: 1,
    };
}
trait AssumeFromConst<'db>: Sized {
    fn from_const(tcx: DbInterner<'db>, ct: Const<'db>) -> Option<Self>;
}
impl<'db> AssumeFromConst<'db> for rustc_transmute::Assume {
    fn from_const(tcx: DbInterner<'db>, ct: Const<'db>) -> Option<Self> {
        let cv = ct.try_to_value()?;
        let adt_def = cv.ty.ty_adt_def()?;

        if tcx.services().transmute_inputs().transmute_opts != Some(adt_def.definition) {
            tcx.delay_bug(format!(
                "The given `const` was not marked with the `{}` lang item.",
                "transmute_opts"
            ));
            return Some(Self {
                alignment: true,
                lifetimes: true,
                safety: true,
                validity: true,
            });
        }

        let variant = adt_def.non_enum_variant();
        let fields = cv.to_branch();

        let get_field = |name| {
            let (field_idx, _) = variant
                .fields
                .iter()
                .enumerate()
                .find(|(_, field_def)| name == field_def.name.0)
                .unwrap_or_else(|| panic!("There were no fields named `{name}`."));
            fields[field_idx]
                .try_to_leaf()
                .map(|leaf| leaf == ScalarInt::TRUE)
        };

        Some(Self {
            alignment: get_field("alignment")?,
            lifetimes: get_field("lifetimes")?,
            safety: get_field("safety")?,
            validity: get_field("validity")?,
        })
    }
}
trait ConstTryExt<'db> {
    fn try_to_value(self) -> Option<model::ValueConst<'db, PreparedProvider<'db>>>;
    fn try_to_leaf(self) -> Option<ScalarInt>;
}
impl<'db> ConstTryExt<'db> for Const<'db> {
    fn try_to_value(self) -> Option<model::ValueConst<'db, PreparedProvider<'db>>> {
        match self.kind() {
            ty::ConstKind::Value(cv) => Some(cv),
            _ => None,
        }
    }
    fn try_to_leaf(self) -> Option<ScalarInt> {
        self.try_to_value()?.try_to_leaf()
    }
}
impl<'db> model::ValueConst<'db, PreparedProvider<'db>> {
    pub fn try_to_leaf(self) -> Option<ScalarInt> {
        match self.tree.kind() {
            ValTreeKind::Leaf(s) => Some(s),
            ValTreeKind::Branch(_) => None,
        }
    }
}
impl<'db> Ty<'db> {
    pub fn ty_adt_def(self) -> Option<AdtDefinition<'db, PreparedProvider<'db>>> {
        match self.kind() {
            ty::TyKind::Adt(adt, _) => Some(adt),
            _ => None,
        }
    }
    pub fn is_box(self) -> bool {
        match self.kind() {
            ty::TyKind::Adt(def, _) => def.is_box(),
            _ => false,
        }
    }
}
impl<'db> AdtDefinition<'db, PreparedProvider<'db>> {
    pub fn non_enum_variant(self) -> &'db model::AdtVariant<'db, PreparedProvider<'db>> {
        assert!(self.is_struct() || self.is_union());
        self.variant(FIRST_VARIANT)
    }
    pub fn adt_kind(self) -> AdtKind {
        if self.is_enum() {
            AdtKind::Enum
        } else if self.is_union() {
            AdtKind::Union
        } else {
            AdtKind::Struct
        }
    }
    fn is_union(self) -> bool {
        self.context
            .services()
            .adt_info(self.context, self.definition)
            .kind
            == AdtKind::Union
    }
    fn is_box(self) -> bool {
        self.context
            .is_adt_lang_item(self.definition, ty::lang_items::SolverAdtLangItem::OwnedBox)
    }
}
trait CompilerTree<'db>: Sized {
    fn from_ty(ty: Ty<'db>, cx: LayoutCx<'db>) -> Result<Self, Err<'db>>;
    fn from_tuple(
        ty_and_layout: (Ty<'db>, Layout<'db>),
        members: model::Types<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>>;
    fn from_struct(
        ty_and_layout: (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>>;
    fn from_enum(
        ty_and_layout: (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>>;
    fn from_variant(
        def: Def<'db>,
        tag: Option<(ScalarInt, VariantIdx, TagEncoding<VariantIdx>)>,
        ty_and_layout: (Ty<'db>, Layout<'db>),
        total_size: Size,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>>;
    fn from_tag(tag: ScalarInt, tcx: DbInterner<'db>) -> Self;
    fn from_union(
        ty_and_layout: (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>>;
}
impl<'db> CompilerTree<'db> for Tree<'db> {
    fn from_ty(ty: Ty<'db>, cx: LayoutCx<'db>) -> Result<Self, Err<'db>> {
        let layout = layout_of(cx, ty)?;

        if let Err(e) = ty.error_reported() {
            return Err(Err::TypeError(e));
        }

        let target = cx.data_layout();
        let pointer_size = target.pointer_size();

        match ty.kind() {
            ty::TyKind::Bool => Ok(Self::bool()),

            ty::TyKind::Float(nty) => {
                let width = nty.bit_width() / 8;
                Ok(Self::number(width.try_into().unwrap()))
            }

            ty::TyKind::Int(nty) => {
                let width = nty.normalize(pointer_size.bits() as _).bit_width().unwrap() / 8;
                Ok(Self::number(width.try_into().unwrap()))
            }

            ty::TyKind::Uint(nty) => {
                let width = nty.normalize(pointer_size.bits() as _).bit_width().unwrap() / 8;
                Ok(Self::number(width.try_into().unwrap()))
            }

            ty::TyKind::Tuple(members) => Self::from_tuple((ty, layout), members, cx),

            ty::TyKind::Array(inner_ty, _len) => {
                let FieldsShape::Array { stride, count } = &layout.fields else {
                    return Err(Err::NotYetSupported);
                };
                let inner_layout = layout_of(cx, inner_ty)?;
                assert_eq!(*stride, inner_layout.size);
                let elt = Tree::from_ty(inner_ty, cx)?;
                Ok(std::iter::repeat_n(elt, *count as usize)
                    .fold(Tree::unit(), |tree, elt| tree.then(elt)))
            }

            ty::TyKind::Adt(adt_def, _args_ref) if !ty.is_box() => match adt_def.adt_kind() {
                AdtKind::Struct => Self::from_struct((ty, layout), adt_def, cx),
                AdtKind::Enum => Self::from_enum((ty, layout), adt_def, cx),
                AdtKind::Union => Self::from_union((ty, layout), adt_def, cx),
            },

            ty::TyKind::Ref(region, ty, mutability) => {
                let layout = layout_of(cx, ty)?;
                let referent_align = layout.align.bytes_usize();
                let referent_size = layout.size.bytes_usize();

                Ok(Tree::Ref(Reference {
                    region: TransmuteRegion(region),
                    is_mut: mutability.is_mut(),
                    referent: TransmuteTy(ty),
                    referent_align,
                    referent_size,
                }))
            }

            ty::TyKind::Char => Ok(Self::char(match cx.data_layout().endian {
                rustc_abi::Endian::Little => rustc_transmute::layout::tree::Endian::Little,
                rustc_abi::Endian::Big => rustc_transmute::layout::tree::Endian::Big,
            })),

            _ => Err(Err::NotYetSupported),
        }
    }
    fn from_tuple(
        (ty, layout): (Ty<'db>, Layout<'db>),
        members: model::Types<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>> {
        match &layout.fields {
            FieldsShape::Primitive => {
                assert_eq!(members.len(), 1);
                let inner_ty = members.items()[0];
                Self::from_ty(inner_ty, cx)
            }
            FieldsShape::Arbitrary { offsets, .. } => {
                assert_eq!(offsets.len(), members.len());
                Self::from_variant(Def::Primitive, None, (ty, layout), layout.size, cx)
            }
            FieldsShape::Array { .. } | FieldsShape::Union(_) => Err(Err::NotYetSupported),
        }
    }
    fn from_struct(
        (ty, layout): (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>> {
        assert!(def.is_struct());
        let def = Def::Adt(def);
        Self::from_variant(def, None, (ty, layout), layout.size, cx)
    }
    fn from_enum(
        (ty, layout): (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>> {
        assert!(def.is_enum());

        // Computes the layout of a variant.
        let layout_of_variant = |index, encoding: Option<_>| -> Result<Self, Err<'db>> {
            let variant_layout = ty_variant(cx, (ty, layout), index)?;
            if variant_layout.is_uninhabited() {
                return Ok(Self::uninhabited());
            }
            let tag = cx.tag_for_variant(ty, index)?;
            let variant_def = Def::Variant { owner: def, index };
            Self::from_variant(
                variant_def,
                tag.map(|tag| (tag, index, encoding.unwrap())),
                (ty, variant_layout),
                layout.size,
                cx,
            )
        };

        match *layout.variants() {
            Variants::Empty => Ok(Self::uninhabited()),
            Variants::Single { index } => {
                // `Variants::Single` on enums with variants denotes that
                // the enum delegates its layout to the variant at `index`.
                layout_of_variant(index, None)
            }
            Variants::Multiple {
                tag: _,
                tag_encoding,
                tag_field,
                ..
            } => {
                // `Variants::Multiple` denotes an enum with multiple
                // variants. The layout of such an enum is the disjunction
                // of the layouts of its tagged variants.

                // For enums (but not coroutines), the tag field is
                // currently always the first field of the layout.
                assert_eq!(tag_field, FieldIdx::ZERO);

                let variants = cx
                    .discriminants(def)?
                    .iter()
                    .map(|discr| (discr.index, discr.value))
                    .try_fold(Self::uninhabited(), |variants, (idx, _discriminant)| {
                        let variant = layout_of_variant(idx, Some(tag_encoding))?;
                        Result::<Self, Err<'db>>::Ok(variants.or(variant))
                    })?;

                Ok(Self::def(Def::Adt(def)).then(variants))
            }
        }
    }
    fn from_variant(
        def: Def<'db>,
        tag: Option<(ScalarInt, VariantIdx, TagEncoding<VariantIdx>)>,
        (ty, layout): (Ty<'db>, Layout<'db>),
        total_size: Size,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>> {
        // This constructor does not support non-`FieldsShape::Arbitrary`
        // layouts.
        let FieldsShape::Arbitrary {
            offsets,
            in_memory_order,
        } = layout.fields()
        else {
            return Err(Err::NotYetSupported);
        };

        // When this function is invoked with enum variants,
        // `layout.size` does not encompass the entire size of the
        // enum. We rely on `total_size` for this.
        assert!(layout.size <= total_size);

        let mut size = Size::ZERO;
        let mut struct_tree = Self::def(def);

        // If a `tag` is provided, place it at the start of the layout.
        if let Some((tag, index, encoding)) = &tag {
            match encoding {
                TagEncoding::Direct => {
                    size += tag.size();
                }
                TagEncoding::Niche { niche_variants, .. } => {
                    if !niche_variants.contains(index) {
                        size += tag.size();
                    }
                }
            }
            struct_tree = struct_tree.then(Self::from_tag(*tag, cx.tcx()));
        }

        // Append the fields, in memory order, to the layout.
        for &field_idx in in_memory_order.iter() {
            // Add interfield padding.
            let padding_needed = offsets[field_idx] - size;
            let padding = Self::padding(padding_needed.bytes_usize());

            let field_ty = ty_field(cx, (ty, layout), field_idx)?;
            let field_layout = layout_of(cx, field_ty)?;
            let field_tree = Self::from_ty(field_ty, cx)?;

            struct_tree = struct_tree.then(padding).then(field_tree);

            size += padding_needed + field_layout.size;
        }

        // Add trailing padding.
        let padding_needed = total_size - size;
        let trailing_padding = Self::padding(padding_needed.bytes_usize());

        Ok(struct_tree.then(trailing_padding))
    }
    fn from_tag(tag: ScalarInt, tcx: DbInterner<'db>) -> Self {
        use rustc_abi::Endian;
        let size = tag.size();
        let bits = tag.to_bits(size);
        let bytes: [u8; 16];
        let bytes = match tcx.services().transmute_inputs().target.endian {
            Endian::Little => {
                bytes = bits.to_le_bytes();
                &bytes[..size.bytes_usize()]
            }
            Endian::Big => {
                bytes = bits.to_be_bytes();
                &bytes[bytes.len() - size.bytes_usize()..]
            }
        };
        Self::Seq(bytes.iter().map(|&b| Self::byte(b)).collect())
    }
    fn from_union(
        (ty, layout): (Ty<'db>, Layout<'db>),
        def: AdtDefinition<'db, PreparedProvider<'db>>,
        cx: LayoutCx<'db>,
    ) -> Result<Self, Err<'db>> {
        assert!(def.is_union());

        // This constructor does not support non-`FieldsShape::Union`
        // layouts. Fields of this shape are all placed at offset 0.
        let FieldsShape::Union(_fields) = layout.fields() else {
            return Err(Err::NotYetSupported);
        };

        let fields = &def.non_enum_variant().fields;
        let fields = fields.iter().enumerate().try_fold(
            Self::uninhabited(),
            |fields, (idx, _field_def)| {
                let idx = FieldIdx::from_usize(idx);
                let field_ty = ty_field(cx, (ty, layout), idx)?;
                let field_layout = layout_of(cx, field_ty)?;
                let field = Self::from_ty(field_ty, cx)?;
                let trailing_padding_needed = layout.size - field_layout.size;
                let trailing_padding = Self::padding(trailing_padding_needed.bytes_usize());
                let field_and_padding = field.then(trailing_padding);
                Result::<Self, Err<'db>>::Ok(fields.or(field_and_padding))
            },
        )?;

        Ok(Self::def(Def::Adt(def)).then(fields))
    }
}
fn ty_field<'db>(
    cx: LayoutCx<'db>,
    (ty, layout): (Ty<'db>, Layout<'db>),
    i: FieldIdx,
) -> Result<Ty<'db>, Err<'db>> {
    // We cannot use `ty_and_layout_field` to retrieve the field type, since
    // `ty_and_layout_field` erases regions in the returned type. We must
    // not erase regions here, since we may need to ultimately emit outlives
    // obligations as a consequence of the transmutability analysis.
    Ok(match ty.kind() {
        ty::TyKind::Adt(def, args) => {
            match layout.variants {
                Variants::Single { index } => {
                    let field = &def.variant(index).fields[i.as_usize()];
                    field.ty(cx.tcx(), args).skip_norm_wip()
                }
                Variants::Empty => panic!("there is no field in Variants::Empty types"),
                // Discriminant field for enums (where applicable).
                Variants::Multiple { tag, .. } => {
                    assert_eq!(i.as_usize(), 0);
                    cx.primitive_type(tag.primitive())?
                }
            }
        }
        ty::TyKind::Tuple(fields) => fields.items()[i.as_usize()],
        kind => unimplemented!(
            "only a subset of `Ty::ty_and_layout_field`'s functionality is implemented. implementation needed for {:?}",
            kind
        ),
    })
}
fn ty_variant<'db>(
    cx: LayoutCx<'db>,
    (ty, layout): (Ty<'db>, Layout<'db>),
    i: VariantIdx,
) -> Result<Layout<'db>, Err<'db>> {
    let ty = cx.tcx().erase_and_anonymize_regions(ty);
    cx.variant_layout(ty, layout, i)
}
fn answer<'db>(
    src: Ty<'db>,
    dst: Ty<'db>,
    assume: rustc_transmute::Assume,
    context: DbInterner<'db>,
) -> Result<Answer<TransmuteRegion<'db>, TransmuteTy<'db>>, TransmuteDependency<'db>> {
    let layout_cx = LayoutCx::new(context, TypingEnv::fully_monomorphized(context));

    // Convert `src` and `dst` from their rustc representations, to `Tree`-based
    // representations.
    let src = Tree::from_ty(src, layout_cx);
    let dst = Tree::from_ty(dst, layout_cx);

    Ok(match (src, dst) {
        (Err(Err::Preparation(dep)), _) | (_, Err(Err::Preparation(dep))) => return Err(dep),
        (Err(Err::TypeError(_)), _) | (_, Err(Err::TypeError(_))) => Answer::No(Reason::TypeError),
        (Err(Err::UnknownLayout), _) => Answer::No(Reason::SrcLayoutUnknown),
        (_, Err(Err::UnknownLayout)) => Answer::No(Reason::DstLayoutUnknown),
        (Err(Err::NotYetSupported), _) => Answer::No(Reason::SrcIsNotYetSupported),
        (_, Err(Err::NotYetSupported)) => Answer::No(Reason::DstIsNotYetSupported),
        (Err(Err::SizeOverflow), _) => Answer::No(Reason::SrcSizeOverflow),
        (_, Err(Err::SizeOverflow)) => Answer::No(Reason::DstSizeOverflow),
        (Ok(src), Ok(dst)) => MaybeTransmutableQuery::new(src, dst, assume, context).answer(),
    })
}
pub(crate) fn is_transmutable<'db>(
    infcx: &InferenceContext<'db>,
    src: Ty<'db>,
    dst: Ty<'db>,
    assume: Const<'db>,
) -> Result<Result<Certainty, NoSolution>, TransmuteDependency<'db>> {
    // Erase regions because we compute layouts in `rustc_transmute`,
    // which will ICE for region vars.
    let (dst, src) = infcx.interner.erase_and_anonymize_regions((dst, src));

    let Some(assume) = rustc_transmute::Assume::from_const(infcx.interner, assume) else {
        return Ok(Err(NoSolution));
    };

    // FIXME(transmutability): This really should be returning nested goals for `Answer::If*`
    Ok(match answer(src, dst, assume, infcx.interner)? {
        rustc_transmute::Answer::Yes => Ok(Certainty::Yes),
        rustc_transmute::Answer::No(_) | rustc_transmute::Answer::If(_) => Err(NoSolution),
    })
}
