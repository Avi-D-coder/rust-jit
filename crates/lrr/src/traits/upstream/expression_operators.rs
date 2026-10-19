//! Pinned compiler operation metadata, without MIR execution; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; unchanged originals/licenses: vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_relations.py.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BinOp {
    /// The `+` operator (addition)
    Add,
    /// Like `Add`, but with UB on overflow.  (Integers only.)
    AddUnchecked,
    /// Like `Add`, but returns `(T, bool)` of both the wrapped result
    /// and a bool indicating whether it overflowed.
    AddWithOverflow,
    /// The `-` operator (subtraction)
    Sub,
    /// Like `Sub`, but with UB on overflow.  (Integers only.)
    SubUnchecked,
    /// Like `Sub`, but returns `(T, bool)` of both the wrapped result
    /// and a bool indicating whether it overflowed.
    SubWithOverflow,
    /// The `*` operator (multiplication)
    Mul,
    /// Like `Mul`, but with UB on overflow.  (Integers only.)
    MulUnchecked,
    /// Like `Mul`, but returns `(T, bool)` of both the wrapped result
    /// and a bool indicating whether it overflowed.
    MulWithOverflow,
    /// The `/` operator (division)
    ///
    /// For integer types, division by zero is UB, as is `MIN / -1` for signed.
    /// The compiler should have inserted checks prior to this.
    ///
    /// Floating-point division by zero is safe, and does not need guards.
    Div,
    /// The `%` operator (modulus)
    ///
    /// For integer types, using zero as the modulus (second operand) is UB,
    /// as is `MIN % -1` for signed.
    /// The compiler should have inserted checks prior to this.
    ///
    /// Floating-point remainder by zero is safe, and does not need guards.
    Rem,
    /// The `^` operator (bitwise xor)
    BitXor,
    /// The `&` operator (bitwise and)
    BitAnd,
    /// The `|` operator (bitwise or)
    BitOr,
    /// The `<<` operator (shift left)
    ///
    /// The offset is given by `RHS.rem_euclid(LHS::BITS)`.
    /// In other words, it is (uniquely) determined as follows:
    /// - it is "equal modulo LHS::BITS" to the RHS
    /// - it is in the range `0..LHS::BITS`
    Shl,
    /// Like `Shl`, but is UB if the RHS >= LHS::BITS or RHS < 0
    ShlUnchecked,
    /// The `>>` operator (shift right)
    ///
    /// The offset is given by `RHS.rem_euclid(LHS::BITS)`.
    /// In other words, it is (uniquely) determined as follows:
    /// - it is "equal modulo LHS::BITS" to the RHS
    /// - it is in the range `0..LHS::BITS`
    ///
    /// This is an arithmetic shift if the LHS is signed
    /// and a logical shift if the LHS is unsigned.
    Shr,
    /// Like `Shl`, but is UB if the RHS >= LHS::BITS or RHS < 0
    ShrUnchecked,
    /// The `==` operator (equality)
    Eq,
    /// The `<` operator (less than)
    Lt,
    /// The `<=` operator (less than or equal to)
    Le,
    /// The `!=` operator (not equal to)
    Ne,
    /// The `>=` operator (greater than or equal to)
    Ge,
    /// The `>` operator (greater than)
    Gt,
    /// The `<=>` operator (three-way comparison, like `Ord::cmp`)
    ///
    /// This is supported only on the integer types and `char`, always returning
    /// [`LangItem::OrderingEnum`] (aka [`std::cmp::Ordering`]).
    ///
    /// [`Rvalue::BinaryOp`]`(BinOp::Cmp, A, B)` returns
    /// - `Ordering::Less` (`-1_i8`, as a Scalar) if `A < B`
    /// - `Ordering::Equal` (`0_i8`, as a Scalar) if `A == B`
    /// - `Ordering::Greater` (`+1_i8`, as a Scalar) if `A > B`
    ///
    /// [`LangItem::OrderingEnum`]: rustc_attr_ir::lang_items::LangItem
    Cmp,
    /// The `ptr.offset` operator
    Offset,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UnOp {
    /// The `!` operator for logical inversion
    Not,
    /// The `-` operator for negation
    Neg,
    /// Gets the metadata `M` from a `*const`/`*mut`/`&`/`&mut` to
    /// `impl Pointee<Metadata = M>`.
    ///
    /// For example, this will give a `()` from `*const i32`, a `usize` from
    /// `&mut [u8]`, or a `ptr::DynMetadata<dyn Foo>` (internally a pointer)
    /// from a `*mut dyn Foo`.
    ///
    /// Allowed only in [`MirPhase::Runtime`]; earlier it's an intrinsic.
    PtrMetadata,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CastKind {
    /// `thir::ExprKind::As`
    As,
    /// `thir::ExprKind::ValueExpr`
    Use,
}
