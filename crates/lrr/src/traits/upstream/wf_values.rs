//! Pinned rustc value-tree helpers; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_wf_values.py.
use super::{
    context::Context,
    facts::SourceFacts,
    model::{ScalarInt, ValueConst},
};
use rustc_abi::{FIRST_VARIANT, Size, VariantIdx};
use rustc_type_ir::{self as ty, ValTreeKind, inherent::IntoKind};

/// Pinned compiler ConstExt operations for the shared foreign Const type.
pub(crate) trait ConstValueExt<'cx, F: SourceFacts<'cx>> {
    fn to_value(self) -> ValueConst<'cx, F>;
    fn to_leaf(self) -> ScalarInt;
}

pub(crate) struct DestructuredAdtConst<'cx, F: SourceFacts<'cx>> {
    pub variant: VariantIdx,
    pub fields: &'cx [ty::Const<Context<'cx, F>>],
}
impl<'cx, F: SourceFacts<'cx>> ConstValueExt<'cx, F> for ty::Const<Context<'cx, F>> {
    fn to_value(self) -> ValueConst<'cx, F> {
        match self.kind() {
            ty::ConstKind::Value(cv) => cv,
            _ => panic!("expected ConstKind::Value, got {:?}", self.kind()),
        }
    }
    fn to_leaf(self) -> ScalarInt {
        self.to_value().to_leaf()
    }
}
impl<'cx, F: SourceFacts<'cx>> ValueConst<'cx, F> {
    pub(crate) fn to_leaf(self) -> ScalarInt {
        match self.tree.kind() {
            ValTreeKind::Leaf(s) => s,
            ValTreeKind::Branch(..) => panic!("expected leaf, got {:?}", self),
        }
    }
    pub(crate) fn to_branch(self) -> &'cx [ty::Const<Context<'cx, F>>] {
        match self.tree.kind() {
            ValTreeKind::Branch(branch) => branch.items(),
            ValTreeKind::Leaf(..) => panic!("expected branch, got {:?}", self),
        }
    }
    pub(crate) fn destructure_adt_const(&self) -> DestructuredAdtConst<'cx, F> {
        let fields = self.to_branch();

        let (variant, fields) = match self.ty.kind() {
            ty::TyKind::Adt(def, _) if def.variants().is_empty() => {
                panic!("unreachable")
            }
            ty::TyKind::Adt(def, _) if def.is_enum() => {
                let (head, rest) = fields.split_first().unwrap();
                (VariantIdx::from_u32(head.to_leaf().to_u32()), rest)
            }
            ty::TyKind::Adt(_, _) => (FIRST_VARIANT, fields),
            _ => panic!(
                "destructure_adt_const called on non-ADT type: {:?}",
                self.ty
            ),
        };

        DestructuredAdtConst { variant, fields }
    }
}
impl ScalarInt {
    pub(crate) fn size(self) -> Size {
        Size::from_bytes(self.bytes)
    }
    pub(crate) fn check_data(self) {
        // Using a block `{self.bits}` here to force a copy instead of using `self.bits`
        // directly, because `debug_assert_eq` takes references to its arguments and formatting
        // arguments and would thus borrow `self.bits`. Since `Self`
        // is a packed struct, that would create a possibly unaligned reference, which
        // is UB.
        debug_assert_eq!(
            self.size().truncate(self.bits),
            { self.bits },
            "Scalar value {:#x} exceeds size of {} bytes",
            { self.bits },
            self.bytes
        );
    }
    pub(crate) fn to_bits(self, expected_size: Size) -> u128 {
        let self_size = u64::from(self.bytes);
        if expected_size.bytes() != self_size {
            #[cold]
            fn invalid(expected_size: u64, self_size: u64) -> ! {
                panic!("ScalarInt has size {self_size} but expected {expected_size}")
            }

            invalid(expected_size.bytes(), self_size);
        }

        self.check_data();
        self.bits
    }
    pub(crate) fn to_uint(self, size: Size) -> u128 {
        self.to_bits(size)
    }
    pub(crate) fn to_u32(self) -> u32 {
        self.to_uint(Size::from_bits(32)).try_into().unwrap()
    }
}
