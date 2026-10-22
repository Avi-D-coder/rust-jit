//! The one type-layout algorithm of the interpreter (E-R5-012).
//!
//! Sizes, alignments and niches follow rustc's `LayoutCalculator`
//! (`rustc_abi::layout`) on a 64-bit target for the types the managed runtime
//! can express: scalars, thin and fat pointers, tuples and structs under
//! `repr(Rust)`, `repr(C)`, `repr(packed(N))` and `repr(align(N))`, arrays,
//! unions, and enums with tagged and niche-filling layouts. The functions here
//! are pure: they take the already computed layouts of the members and return
//! the layout of the aggregate. `GuestStore` (`cell_layout.rs`) supplies the
//! members from type descriptors and demanded declaration facts, and both the
//! `core::mem` layout intrinsics and the synthetic pointer addresses use this
//! module, so a type has exactly one size.
//!
//! Field offsets are not part of observable Rust semantics; they are computed
//! because niche placement and enum variant packing depend on them.

use std::cmp::Reverse;

/// A scalar whose valid range leaves some bit patterns unused. Only the count
/// of unused patterns and where they sit matter for layout.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Niche {
    /// Byte offset of the scalar inside the type.
    pub offset: u64,
    /// Width in bytes of the scalar holding the niche.
    pub width: u64,
    /// Number of unused bit patterns.
    pub available: u128,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
    pub niche: Option<Niche>,
    /// No value of the type exists (`!`, `enum Void {}`, a product with an
    /// uninhabited field).
    pub uninhabited: bool,
}

/// The integer type of an enum discriminant (`#[repr(u8)]`, `#[repr(i32)]`).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct IntRepr {
    pub bytes: u64,
    pub signed: bool,
}

/// Parsed `#[repr(..)]` hints.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct Repr {
    pub c: bool,
    pub packed: Option<u64>,
    pub align: Option<u64>,
    pub int: Option<IntRepr>,
    pub transparent: bool,
}

impl Repr {
    fn inhibit_struct_field_reordering(&self) -> bool {
        self.c || self.int.is_some()
    }
    fn inhibit_enum_layout_opt(&self) -> bool {
        self.c || self.int.is_some()
    }
}

pub(crate) fn align_up(value: u64, align: u64) -> u64 {
    let align = align.max(1);
    value.div_ceil(align).saturating_mul(align)
}

impl Layout {
    pub const ZERO_SIZED: Layout = Layout {
        size: 0,
        align: 1,
        niche: None,
        uninhabited: false,
    };
    pub const NEVER: Layout = Layout {
        size: 0,
        align: 1,
        niche: None,
        uninhabited: true,
    };
    /// A plain scalar of `bytes` bytes with no niche.
    pub fn scalar(bytes: u64) -> Layout {
        Layout {
            size: bytes,
            align: bytes.max(1),
            niche: None,
            uninhabited: false,
        }
    }
    /// A scalar of `bytes` bytes of which `valid` patterns are valid.
    pub fn scalar_with_valid_range(bytes: u64, valid: u128) -> Layout {
        let total = 1u128 << (8 * bytes);
        Layout {
            niche: Some(Niche {
                offset: 0,
                width: bytes,
                available: total.saturating_sub(valid),
            }),
            ..Layout::scalar(bytes)
        }
    }
    pub fn bool() -> Layout {
        Layout::scalar_with_valid_range(1, 2)
    }
    pub fn char() -> Layout {
        Layout::scalar_with_valid_range(4, 0x11_0000)
    }
    /// A thin or fat pointer: `fat` pointers are a data pointer and a length
    /// or vtable word. `nonnull` pointers (references, `Box`, `NonNull`, fn
    /// pointers) have the null pattern as a niche.
    pub fn pointer(fat: bool, nonnull: bool) -> Layout {
        Layout {
            size: if fat { 16 } else { 8 },
            align: 8,
            niche: nonnull.then_some(Niche {
                offset: 0,
                width: 8,
                available: 1,
            }),
            uninhabited: false,
        }
    }
    pub fn is_zst(&self) -> bool {
        self.size == 0
    }
    pub fn is_1zst(&self) -> bool {
        self.size == 0 && self.align == 1
    }
    fn niche_available(&self) -> u128 {
        self.niche.map_or(0, |niche| niche.available)
    }
    /// The same layout with its niche removed (`UnsafeCell`, unions).
    pub fn without_niche(mut self) -> Layout {
        self.niche = None;
        self
    }
    /// An array of `len` elements.
    pub fn array(element: Layout, len: u64) -> Layout {
        let size = element.size.saturating_mul(len);
        Layout {
            size,
            align: element.align,
            niche: if len == 0 { None } else { element.niche },
            uninhabited: len != 0 && element.uninhabited,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StructKind {
    /// Fields laid out from offset 0.
    Sized,
    /// As `Sized`, but the last field may be unsized and so stays last: only
    /// the fields before it are reordered (rustc `StructKind::MaybeUnsized`,
    /// the kind of every tuple type).
    MaybeUnsized,
    /// Fields laid out after an enum tag of `size` bytes and `align`.
    Prefixed { size: u64, align: u64 },
}

/// A laid out product type: its layout and the byte offset of every field
/// (indexed by declaration order).
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub layout: Layout,
    pub offsets: Vec<u64>,
}

/// A struct, tuple, closure environment or enum variant.
pub fn univariant(fields: &[Layout], repr: &Repr, kind: StructKind) -> StructLayout {
    let start = univariant_biased(fields, repr, kind, true);
    // Enums prefer niches close to the start or end of their variants. If the
    // default ordering did not put the niche at the front, try ordering
    // niches towards the end and keep that when it is nearer an edge.
    if let Some(niche) = start.layout.niche
        && fields.len() > 1
    {
        let head = niche.offset;
        let tail = start.layout.size.saturating_sub(head + niche.width);
        if head != 0 && tail > 0 {
            let alt = univariant_biased(fields, repr, kind, false);
            if let Some(alt_niche) = alt.layout.niche {
                let alt_head = alt_niche.offset;
                if alt_head > head && alt_head > tail {
                    return alt;
                }
            }
        }
    }
    start
}

fn univariant_biased(
    fields: &[Layout],
    repr: &Repr,
    kind: StructKind,
    bias_start: bool,
) -> StructLayout {
    let pack = repr.packed;
    let mut align = 1u64;
    let mut order: Vec<usize> = (0..fields.len()).collect();
    if !repr.inhibit_struct_field_reordering() && fields.len() > 1 {
        // An unsized-capable last field is excluded from the heuristics too.
        let considered = match kind {
            StructKind::MaybeUnsized => &fields[..fields.len() - 1],
            _ => fields,
        };
        let largest_niche_size = considered
            .iter()
            .map(Layout::niche_available)
            .max()
            .unwrap_or(0);
        let max_field_align = considered.iter().map(|f| f.align).max().unwrap_or(1);
        // Groups fields by alignment, or a size-derived pseudo-alignment.
        let group_key = |f: &Layout| -> u64 {
            if let Some(pack) = pack {
                return f.align.min(pack);
            }
            let size_as_align = u64::from(f.align.max(f.size).trailing_zeros());
            if largest_niche_size > 0 {
                if bias_start {
                    u64::from(max_field_align.trailing_zeros()).min(size_as_align)
                } else if f.niche_available() == largest_niche_size {
                    u64::from(f.align.trailing_zeros())
                } else {
                    size_as_align
                }
            } else {
                size_as_align
            }
        };
        let optimizing = match kind {
            StructKind::MaybeUnsized => order.len() - 1,
            _ => order.len(),
        };
        match kind {
            StructKind::Sized | StructKind::MaybeUnsized => order[..optimizing].sort_by_key(|&index| {
                let field = &fields[index];
                let niche_size = field.niche_available();
                let niche_key = if bias_start { !niche_size } else { niche_size };
                let inner_niche_offset_key = if bias_start {
                    field.niche.map_or(0, |niche| niche.offset)
                } else {
                    field.niche.map_or(0, |niche| {
                        !(field.size.saturating_sub(niche.width + niche.offset))
                    })
                };
                (Reverse(group_key(field)), niche_key, inner_niche_offset_key)
            }),
            StructKind::Prefixed { .. } => order.sort_by_key(|&index| {
                let field = &fields[index];
                (group_key(field), field.niche_available())
            }),
        }
    }
    let mut offset = 0u64;
    if let StructKind::Prefixed {
        size,
        align: prefix_align,
    } = kind
    {
        let prefix_align = pack.map_or(prefix_align, |pack| prefix_align.min(pack));
        align = align.max(prefix_align);
        offset = align_up(size, prefix_align);
    }
    let mut offsets = vec![0u64; fields.len()];
    let mut largest_niche: Option<Niche> = None;
    let mut largest_available = 0u128;
    for &index in &order {
        let field = &fields[index];
        let field_align = pack.map_or(field.align, |pack| field.align.min(pack));
        offset = align_up(offset, field_align);
        align = align.max(field_align);
        offsets[index] = offset;
        if let Some(mut niche) = field.niche {
            let prefer = if bias_start {
                niche.available > largest_available
            } else {
                niche.available >= largest_available
            };
            if prefer {
                largest_available = niche.available;
                niche.offset += offset;
                largest_niche = Some(niche);
            }
        }
        offset = offset.saturating_add(field.size);
    }
    if let Some(repr_align) = repr.align {
        align = align.max(repr_align);
    }
    let size = align_up(offset, align);
    StructLayout {
        layout: Layout {
            size,
            align,
            niche: largest_niche,
            uninhabited: fields.iter().any(|f| f.uninhabited),
        },
        offsets,
    }
}

/// A `union`: the largest member, aligned to the strictest one. No niche.
pub fn union_layout(fields: &[Layout], repr: &Repr) -> Layout {
    let pack = repr.packed;
    let mut align = 1u64;
    let mut size = 0u64;
    for field in fields {
        align = align.max(pack.map_or(field.align, |pack| field.align.min(pack)));
        size = size.max(field.size);
    }
    if let Some(repr_align) = repr.align {
        align = align.max(repr_align);
    }
    Layout {
        size: align_up(size, align),
        align,
        niche: None,
        uninhabited: false,
    }
}

fn tag_for_range(min: i128, max: i128, repr: &Repr) -> (u64, bool) {
    if let Some(int) = repr.int {
        return (int.bytes, int.signed);
    }
    let unsigned_fit = |value: u128| -> u64 {
        match value {
            0..=0xFF => 1,
            0x100..=0xFFFF => 2,
            0x1_0000..=0xFFFF_FFFF => 4,
            0x1_0000_0000..=0xFFFF_FFFF_FFFF_FFFF => 8,
            _ => 16,
        }
    };
    let signed_fit = |value: i128| -> u64 {
        if (i128::from(i8::MIN)..=i128::from(i8::MAX)).contains(&value) {
            1
        } else if (i128::from(i16::MIN)..=i128::from(i16::MAX)).contains(&value) {
            2
        } else if (i128::from(i32::MIN)..=i128::from(i32::MAX)).contains(&value) {
            4
        } else if (i128::from(i64::MIN)..=i128::from(i64::MAX)).contains(&value) {
            8
        } else {
            16
        }
    };
    // `repr(C)` enums use at least a C `int`; `repr(Rust)` enums as little as
    // a byte.
    let at_least = if repr.c { 4 } else { 1 };
    if min >= 0 {
        (unsigned_fit(max as u128).max(at_least), false)
    } else {
        (signed_fit(min).max(signed_fit(max)).max(at_least), true)
    }
}

/// An enum: `variants[i]` are the field layouts of variant `i` and
/// `discriminants[i]` its discriminant value (declaration order).
pub fn enum_layout(variants: &[Vec<Layout>], discriminants: &[i128], repr: &Repr) -> Layout {
    if variants.is_empty() {
        return Layout::NEVER;
    }
    // A variant is absent when it cannot be constructed and occupies no space.
    let absent = |fields: &[Layout]| {
        fields.iter().any(|f| f.uninhabited) && fields.iter().all(|f| f.is_zst())
    };
    let mut present = variants
        .iter()
        .enumerate()
        .filter(|(_, fields)| !absent(fields))
        .map(|(index, _)| index);
    let first = present.next();
    let second = present.next();
    let Some(first) = first else {
        return Layout::NEVER;
    };
    if second.is_none() && !repr.inhibit_enum_layout_opt() {
        // A single inhabited variant is laid out as a struct, with no tag.
        let mut layout = univariant(&variants[first], repr, StructKind::Sized).layout;
        layout.uninhabited = false;
        return layout;
    }
    let tagged = tagged_layout(variants, discriminants, repr);
    if repr.inhibit_enum_layout_opt() {
        return tagged;
    }
    match niche_filling_layout(variants, repr, &absent) {
        // Pick the smaller layout; on a tie the larger niche; else the tagged.
        Some(niche) => {
            if tagged.size > niche.size
                || (tagged.size == niche.size
                    && tagged.niche_available() < niche.niche_available())
            {
                niche
            } else {
                tagged
            }
        }
        None => tagged,
    }
}

fn niche_filling_layout(
    variants: &[Vec<Layout>],
    repr: &Repr,
    absent: &dyn Fn(&[Layout]) -> bool,
) -> Option<Layout> {
    let mut align = 1u64;
    let mut laid_out = Vec::with_capacity(variants.len());
    for fields in variants {
        let layout = univariant(fields, repr, StructKind::Sized);
        align = align.max(layout.layout.align);
        laid_out.push(layout);
    }
    // `max_by_key` keeps the last of equally large variants.
    let largest = laid_out
        .iter()
        .enumerate()
        .max_by_key(|(_, layout)| layout.layout.size)
        .map(|(index, _)| index)?;
    let needs_tag = |index: usize| index != largest && !absent(&variants[index]);
    let first = (0..variants.len()).find(|&index| needs_tag(index))?;
    let last = (0..variants.len()).rev().find(|&index| needs_tag(index))?;
    let count = (last - first + 1) as u128;
    let niche = laid_out[largest].layout.niche?;
    if niche.available < count {
        return None;
    }
    let size = align_up(laid_out[largest].layout.size, align);
    let all_fit = laid_out.iter().enumerate().all(|(index, variant)| {
        if index == largest {
            return true;
        }
        // The variant fits before the niche, or after it.
        if variant.layout.size <= niche.offset {
            return true;
        }
        let this_offset = align_up(niche.offset + niche.width, variant.layout.align);
        this_offset + variant.layout.size <= size
    });
    if !all_fit {
        return None;
    }
    Some(Layout {
        size,
        align,
        niche: Some(Niche {
            available: niche.available - count,
            ..niche
        }),
        uninhabited: laid_out.iter().all(|v| v.layout.uninhabited),
    })
}

fn tagged_layout(variants: &[Vec<Layout>], discriminants: &[i128], repr: &Repr) -> Layout {
    let (mut min, mut max) = (i128::MAX, i128::MIN);
    for (index, fields) in variants.iter().enumerate() {
        if !repr.c && fields.iter().any(|f| f.uninhabited) {
            continue;
        }
        let value = discriminants.get(index).copied().unwrap_or(index as i128);
        min = min.min(value);
        max = max.max(value);
    }
    if (min, max) == (i128::MAX, i128::MIN) {
        (min, max) = (0, 0);
    }
    let (min_bytes, _signed) = tag_for_range(min, max, repr);
    let mut align = 1u64;
    let mut size = 0u64;
    let mut start_align = 256u64;
    // `repr(C)` enums are a tag followed by a union of the variants.
    let mut prefix_align = min_bytes;
    if repr.c {
        for fields in variants {
            for field in fields {
                prefix_align = prefix_align.max(field.align);
            }
        }
    }
    let mut laid_out = Vec::with_capacity(variants.len());
    for fields in variants {
        let layout = univariant(
            fields,
            repr,
            StructKind::Prefixed {
                size: min_bytes,
                align: prefix_align,
            },
        );
        // The first field (by offset) that is not a 1-ZST bounds how far the
        // tag may be widened.
        let mut by_offset: Vec<usize> = (0..fields.len()).collect();
        by_offset.sort_by_key(|&index| layout.offsets[index]);
        if let Some(&index) = by_offset.iter().find(|&&index| !fields[index].is_1zst()) {
            start_align = start_align.min(fields[index].align);
        }
        size = size.max(layout.layout.size);
        align = align.max(layout.layout.align);
        laid_out.push(layout);
    }
    size = align_up(size, align);
    // Widen the tag to the alignment of the first field when that is a plain
    // integer width (LLVM then copies no padding).
    let mut tag_bytes = if repr.c || repr.int.is_some() {
        min_bytes
    } else if matches!(start_align, 1 | 2 | 4 | 8 | 16) {
        start_align
    } else {
        min_bytes
    };
    if tag_bytes <= min_bytes {
        tag_bytes = min_bytes;
    }
    let tag_bits = 8 * tag_bytes.min(16);
    let total = if tag_bits >= 128 { u128::MAX } else { 1u128 << tag_bits };
    let used = (max - min) as u128 + 1;
    Layout {
        size,
        align,
        niche: Some(Niche {
            offset: 0,
            width: tag_bytes,
            available: total.saturating_sub(used),
        }),
        uninhabited: laid_out.iter().all(|v| v.layout.uninhabited),
    }
}

#[cfg(test)]
mod tests {
    //! Every expected number below comes from `rustc +1.99.0 -O` on a 64-bit
    //! macOS host (`size_of`/`align_of` of the same declarations).
    use super::*;

    fn u8_() -> Layout {
        Layout::scalar(1)
    }
    fn u16_() -> Layout {
        Layout::scalar(2)
    }
    fn u32_() -> Layout {
        Layout::scalar(4)
    }
    fn u64_() -> Layout {
        Layout::scalar(8)
    }
    fn unit() -> Layout {
        Layout::ZERO_SIZED
    }
    fn reference() -> Layout {
        Layout::pointer(false, true)
    }
    fn rust(fields: &[Layout]) -> Layout {
        univariant(fields, &Repr::default(), StructKind::Sized).layout
    }
    fn c(fields: &[Layout]) -> Layout {
        let repr = Repr {
            c: true,
            ..Repr::default()
        };
        univariant(fields, &repr, StructKind::Sized).layout
    }
    fn dims(layout: Layout) -> (u64, u64) {
        (layout.size, layout.align)
    }
    /// `Option<T>`
    fn option(inner: Layout) -> Layout {
        enum_layout(&[vec![], vec![inner]], &[0, 1], &Repr::default())
    }
    fn fieldless(count: usize) -> Layout {
        let variants = vec![Vec::new(); count];
        let discriminants: Vec<i128> = (0..count as i128).collect();
        enum_layout(&variants, &discriminants, &Repr::default())
    }

    #[test]
    fn scalars_and_pointers() {
        assert_eq!(dims(Layout::bool()), (1, 1));
        assert_eq!(dims(Layout::char()), (4, 4));
        assert_eq!(dims(Layout::pointer(false, true)), (8, 8));
        assert_eq!(dims(Layout::pointer(true, true)), (16, 8));
        assert_eq!(dims(Layout::array(u32_(), 3)), (12, 4));
        assert_eq!(dims(Layout::array(u16_(), 5)), (10, 2));
        assert_eq!(dims(Layout::array(u8_(), 0)), (0, 1));
    }

    #[test]
    fn tuples_and_repr_rust_structs_reorder_fields() {
        assert_eq!(dims(rust(&[u8_(), u32_()])), (8, 4));
        assert_eq!(dims(rust(&[u8_(), u16_(), u8_()])), (4, 2));
        let inner = rust(&[u32_(), u8_()]);
        assert_eq!(dims(inner), (8, 4));
        assert_eq!(dims(rust(&[u8_(), inner])), (12, 4));
        // struct A { a: u8, b: u32, c: u16 }
        assert_eq!(dims(rust(&[u8_(), u32_(), u16_()])), (8, 4));
        // struct W(u8, bool), struct Xx(bool, bool)
        assert_eq!(dims(rust(&[u8_(), Layout::bool()])), (2, 1));
        assert_eq!(dims(rust(&[Layout::bool(), Layout::bool()])), (2, 1));
    }

    #[test]
    fn repr_c_packed_and_align() {
        // #[repr(C)] struct B { a: u8, b: u32, c: u16 }
        assert_eq!(dims(c(&[u8_(), u32_(), u16_()])), (12, 4));
        let fields = [u8_(), u32_(), u16_()];
        // #[repr(C, packed)]
        let packed_c = Repr {
            c: true,
            packed: Some(1),
            ..Repr::default()
        };
        assert_eq!(
            dims(univariant(&fields, &packed_c, StructKind::Sized).layout),
            (7, 1)
        );
        // #[repr(packed(2))]
        let packed_2 = Repr {
            packed: Some(2),
            ..Repr::default()
        };
        assert_eq!(
            dims(univariant(&fields, &packed_2, StructKind::Sized).layout),
            (8, 2)
        );
        // #[repr(align(16))] struct E { a: u8 }
        let aligned = Repr {
            align: Some(16),
            ..Repr::default()
        };
        assert_eq!(
            dims(univariant(&[u8_()], &aligned, StructKind::Sized).layout),
            (16, 16)
        );
        // #[repr(C, align(8))] struct F { a: u8, b: u16 }
        let c_aligned = Repr {
            c: true,
            align: Some(8),
            ..Repr::default()
        };
        assert_eq!(
            dims(univariant(&[u8_(), u16_()], &c_aligned, StructKind::Sized).layout),
            (8, 8)
        );
    }

    #[test]
    fn upstream_union_packed_struct_sizes() {
        // tests/ui/union/union-packed.rs
        let s = [u16_(), Layout::array(u8_(), 3)];
        assert_eq!(dims(rust(&s)), (6, 2));
        let p1 = Repr {
            packed: Some(1),
            ..Repr::default()
        };
        let p2 = Repr {
            packed: Some(2),
            ..Repr::default()
        };
        let c_p4 = Repr {
            c: true,
            packed: Some(4),
            ..Repr::default()
        };
        assert_eq!(dims(univariant(&s, &p1, StructKind::Sized).layout), (5, 1));
        assert_eq!(dims(univariant(&s, &p2, StructKind::Sized).layout), (6, 2));
        assert_eq!(dims(union_layout(&s, &Repr::default())), (4, 2));
        assert_eq!(dims(union_layout(&s, &p1)), (3, 1));
        assert_eq!(dims(union_layout(&s, &p2)), (4, 2));
        assert_eq!(dims(union_layout(&s, &c_p4)), (4, 2));
    }

    #[test]
    fn fieldless_enums() {
        assert_eq!(dims(fieldless(0)), (0, 1));
        assert_eq!(dims(fieldless(1)), (0, 1));
        assert_eq!(dims(fieldless(2)), (1, 1));
        assert_eq!(dims(fieldless(3)), (1, 1));
        // enum K { X = 300, Y }
        assert_eq!(
            dims(enum_layout(&[vec![], vec![]], &[300, 301], &Repr::default())),
            (2, 2)
        );
        // #[repr(u8)] / #[repr(i32)] / #[repr(C)]
        let r_u8 = Repr {
            int: Some(IntRepr {
                bytes: 1,
                signed: false,
            }),
            ..Repr::default()
        };
        let r_i32 = Repr {
            int: Some(IntRepr {
                bytes: 4,
                signed: true,
            }),
            ..Repr::default()
        };
        let r_c = Repr {
            c: true,
            ..Repr::default()
        };
        assert_eq!(dims(enum_layout(&[vec![], vec![]], &[0, 1], &r_u8)), (1, 1));
        assert_eq!(dims(enum_layout(&[vec![], vec![]], &[-1, 5], &r_i32)), (4, 4));
        assert_eq!(dims(enum_layout(&[vec![], vec![]], &[0, 1], &r_c)), (4, 4));
    }

    #[test]
    fn data_enums_tagged() {
        let default = Repr::default();
        // enum L { X(u32), Y }
        assert_eq!(
            dims(enum_layout(&[vec![u32_()], vec![]], &[0, 1], &default)),
            (8, 4)
        );
        // enum M { X(u8), Y(u16), Z }
        assert_eq!(
            dims(enum_layout(
                &[vec![u8_()], vec![u16_()], vec![]],
                &[0, 1, 2],
                &default
            )),
            (4, 2)
        );
        // enum Pq { X(u32, u8), Y(u8) }
        assert_eq!(
            dims(enum_layout(
                &[vec![u32_(), u8_()], vec![u8_()]],
                &[0, 1],
                &default
            )),
            (8, 4)
        );
        // enum Q { X(u64), Y(u8, u8) }
        assert_eq!(
            dims(enum_layout(
                &[vec![u64_()], vec![u8_(), u8_()]],
                &[0, 1],
                &default
            )),
            (16, 8)
        );
        // #[repr(u8)] enum T1 { X(u32), Y }, #[repr(C)] enum T2 { X(u32), Y(u8) },
        // #[repr(C, u8)] enum T3 { X(u64), Y(u8) }
        let r_u8 = Repr {
            int: Some(IntRepr {
                bytes: 1,
                signed: false,
            }),
            ..Repr::default()
        };
        let r_c = Repr {
            c: true,
            ..Repr::default()
        };
        let r_c_u8 = Repr {
            c: true,
            int: r_u8.int,
            ..Repr::default()
        };
        assert_eq!(
            dims(enum_layout(&[vec![u32_()], vec![]], &[0, 1], &r_u8)),
            (8, 4)
        );
        assert_eq!(
            dims(enum_layout(&[vec![u32_()], vec![u8_()]], &[0, 1], &r_c)),
            (8, 4)
        );
        assert_eq!(
            dims(enum_layout(&[vec![u64_()], vec![u8_()]], &[0, 1], &r_c_u8)),
            (16, 8)
        );
    }

    #[test]
    fn single_variant_and_empty_enums() {
        // enum R {}  enum S1 { X }  enum S2 { X(u32) }
        assert_eq!(dims(enum_layout(&[], &[], &Repr::default())), (0, 1));
        assert_eq!(dims(enum_layout(&[vec![]], &[0], &Repr::default())), (0, 1));
        assert_eq!(
            dims(enum_layout(&[vec![u32_()]], &[0], &Repr::default())),
            (4, 4)
        );
        // enum SingleVariant { V, Never(Void) }: the uninhabited variant is absent.
        let void = Layout::NEVER;
        assert_eq!(
            dims(enum_layout(&[vec![], vec![void]], &[0, 1], &Repr::default())),
            (0, 1)
        );
    }

    #[test]
    fn niche_filling() {
        // enum N { X(bool), Y }
        assert_eq!(
            dims(enum_layout(
                &[vec![Layout::bool()], vec![]],
                &[0, 1],
                &Repr::default()
            )),
            (1, 1)
        );
        // Option<u8> / Option<u32> / Option<usize> / Option<()> / Option<bool>
        assert_eq!(dims(option(u8_())), (2, 1));
        assert_eq!(dims(option(u32_())), (8, 4));
        assert_eq!(dims(option(u64_())), (16, 8));
        assert_eq!(dims(option(unit())), (1, 1));
        assert_eq!(dims(option(Layout::bool())), (1, 1));
        assert_eq!(dims(option(option(Layout::bool()))), (1, 1));
        assert_eq!(dims(option(option(option(Layout::bool())))), (1, 1));
        assert_eq!(dims(option(Layout::char())), (4, 4));
        // Option<&T>, Option<Box<T>>, Option<fn()>, Option<*const T>
        assert_eq!(dims(option(reference())), (8, 8));
        assert_eq!(dims(option(Layout::pointer(false, false))), (16, 8));
        assert_eq!(dims(option(Layout::pointer(true, true))), (16, 8));
        // Option<fieldless enum>
        assert_eq!(dims(option(fieldless(3))), (1, 1));
        assert_eq!(dims(option(option(fieldless(3)))), (1, 1));
        // Option<(u8, bool)> and Option<[bool; 2]>
        assert_eq!(dims(option(rust(&[u8_(), Layout::bool()]))), (2, 1));
        assert_eq!(dims(option(Layout::array(Layout::bool(), 2))), (2, 1));
        // Result<G, ()>, Result<u8, ()>, Result<(), ()>, Result<u32, u8>, Result<bool, bool>
        let result = |ok: Layout, err: Layout| {
            enum_layout(&[vec![ok], vec![err]], &[0, 1], &Repr::default())
        };
        assert_eq!(dims(result(fieldless(3), unit())), (1, 1));
        assert_eq!(dims(result(u8_(), unit())), (2, 1));
        assert_eq!(dims(result(unit(), unit())), (1, 1));
        assert_eq!(dims(result(u32_(), u8_())), (8, 4));
        assert_eq!(dims(result(u32_(), fieldless(3))), (8, 4));
        assert_eq!(dims(result(Layout::bool(), Layout::bool())), (2, 1));
        assert_eq!(dims(result(reference(), unit())), (8, 8));
        // enum O { X(Box<u8>), Y, Z } is 16: the single null niche cannot hold two variants.
        assert_eq!(
            dims(enum_layout(
                &[vec![reference()], vec![], vec![]],
                &[0, 1, 2],
                &Repr::default()
            )),
            (16, 8)
        );
    }

    #[test]
    fn niche_of_a_tagged_enum_is_reused() {
        // enum L { X(u32), Y } has 2^32 - 2 tag niches.
        let l = enum_layout(&[vec![u32_()], vec![]], &[0, 1], &Repr::default());
        assert_eq!(dims(option(l)), (8, 4));
        // enum M { X(u8), Y(u16), Z }
        let m = enum_layout(
            &[vec![u8_()], vec![u16_()], vec![]],
            &[0, 1, 2],
            &Repr::default(),
        );
        assert_eq!(dims(option(m)), (4, 2));
        // Option<Option<u32>> reuses the tag niche.
        assert_eq!(dims(option(option(u32_()))), (8, 4));
    }

    #[test]
    fn upstream_nonzero_enum() {
        // enum E { A = 1, B = 2, C = 3 }; struct S { a: u16, b: u8, e: E }
        let e = enum_layout(&[vec![], vec![], vec![]], &[1, 2, 3], &Repr::default());
        assert_eq!(dims(e), (1, 1));
        assert_eq!(dims(option(e)), (1, 1));
        let result = enum_layout(&[vec![e], vec![unit()]], &[0, 1], &Repr::default());
        assert_eq!(dims(result), (1, 1));
        let s = rust(&[u16_(), u8_(), e]);
        assert_eq!(dims(option(s)), dims(s));
    }

    #[test]
    fn upstream_packed_struct_optimized_enum() {
        // type PackedRefAndBool = (Packed<&()>, bool) with #[repr(packed)] Packed.
        let packed = Repr {
            packed: Some(1),
            ..Repr::default()
        };
        let packed_ref = univariant(&[reference()], &packed, StructKind::Sized).layout;
        assert_eq!(dims(packed_ref), (8, 1));
        let pair = rust(&[packed_ref, Layout::bool()]);
        assert_eq!(dims(pair), (9, 1));
        assert_eq!(dims(option(pair)), (9, 1));
    }

    #[test]
    fn library_shaped_layouts() {
        // Vec<u8> / String: three words, a huge capacity niche.
        let vec = Layout {
            size: 24,
            align: 8,
            niche: Some(Niche {
                offset: 0,
                width: 8,
                available: 1 << 63,
            }),
            uninhabited: false,
        };
        assert_eq!(dims(option(vec)), (24, 8));
        assert_eq!(dims(option(option(vec))), (24, 8));
        assert_eq!(dims(option(Layout::pointer(true, true))), (16, 8));
        // Option<Cell<bool>>: UnsafeCell hides the niche.
        assert_eq!(dims(option(Layout::bool().without_niche())), (2, 1));
    }
}
