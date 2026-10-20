//! The shared IR's immutable type family. No Rust execution or inference lives
//! here: folds and relations call the shared interfaces and prepared services.
use super::{
    context::Context, facts::SourceFacts, identity::DefinitionId, parameters::SourceParameter,
    storage::Interned,
};
use crate::core::digest::Digest;
use rustc_type_ir::relate::{Relate, RelateResult, TypeRelation};
use rustc_type_ir::{self as ty, inherent::*, *};
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

pub(crate) struct List<'cx, T>(pub(super) Option<Interned<'cx, Vec<T>>>);
impl<T> Copy for List<'_, T> {}
impl<T> Clone for List<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Default for List<'_, T> {
    fn default() -> Self {
        Self(None)
    }
}
impl<T> PartialEq for List<'_, T> {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0
    }
}
impl<T> Eq for List<'_, T> {}
impl<T> Hash for List<'_, T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}
impl<T: fmt::Debug> fmt::Debug for List<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.items().fmt(f)
    }
}
impl<'cx, T> List<'cx, T> {
    pub(crate) fn items(self) -> &'cx [T] {
        self.0.map_or(&[], |node| node.value().as_slice())
    }
}
impl<'cx, T: Copy> SliceLike for List<'cx, T> {
    type Item = T;
    type IntoIter = std::iter::Copied<std::slice::Iter<'cx, T>>;
    fn iter(self) -> Self::IntoIter {
        self.items().iter().copied()
    }
    fn as_slice(&self) -> &[T] {
        self.items()
    }
}
impl<'cx, T: Copy> IntoIterator for List<'cx, T> {
    type Item = T;
    type IntoIter = std::iter::Copied<std::slice::Iter<'cx, T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items().iter().copied()
    }
}
pub(super) trait AllocateList<'cx, T> {
    fn list(self, values: Vec<T>) -> List<'cx, T>;
}
impl<'cx, F: SourceFacts<'cx>, T: TypeVisitable<Context<'cx, F>>> TypeVisitable<Context<'cx, F>>
    for List<'cx, T>
{
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.items().visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>, T: Copy + TypeFoldable<Context<'cx, F>>>
    TypeFoldable<Context<'cx, F>> for List<'cx, T>
where
    Context<'cx, F>: AllocateList<'cx, T>,
{
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        let cx = d.cx();
        let values = self
            .iter()
            .map(|x| x.try_fold_with(d))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cx.list(values))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        let cx = d.cx();
        cx.list(self.iter().map(|x| x.fold_with(d)).collect())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Symbol<'cx>(pub(crate) &'cx str);
impl<'cx, I: Interner<Symbol = Self>> ty::inherent::Symbol<I> for Symbol<'cx> {
    const KW_UNDERSCORE_LIFETIME: Self = Self("'_");
    const KW_STATIC_LIFETIME: Self = Self("'static");
    const SYM_ANON: Self = Self("<anonymous>");
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Span {
    pub(crate) origin: Digest,
    pub(crate) start: u32,
    pub(crate) end: u32,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ErrorToken(pub(crate) Digest);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum Safety {
    Safe,
    Unsafe,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ScalarInt {
    pub(crate) bits: u128,
    pub(crate) bytes: u8,
}
macro_rules! atomic {
 ($($t:ty),*)=>{$(impl<I:Interner> TypeVisitable<I> for $t {fn visit_with<V:TypeVisitor<I>>(&self,_:&mut V)->V::Result{V::Result::output()}}
 impl<I:Interner> TypeFoldable<I> for $t {
 fn try_fold_with<D:FallibleTypeFolder<I>>(self,_:&mut D)->Result<Self,D::Error>{Ok(self)}
 fn fold_with<D:TypeFolder<I>>(self,_:&mut D)->Self{self}
 })*};
}
atomic!(Span, Safety, ScalarInt);
// The shared visitor must see the actual guarantee, including through Ty or
// Region error nodes. Treating this as an opaque atomic field loses taint.
impl<I: Interner<ErrorGuaranteed = Self>> TypeVisitable<I> for ErrorToken {
    fn visit_with<V: TypeVisitor<I>>(&self, visitor: &mut V) -> V::Result {
        visitor.visit_error(*self)
    }
}
impl<I: Interner<ErrorGuaranteed = Self>> TypeFoldable<I> for ErrorToken {
    fn try_fold_with<D: FallibleTypeFolder<I>>(self, _: &mut D) -> Result<Self, D::Error> {
        Ok(self)
    }
    fn fold_with<D: TypeFolder<I>>(self, _: &mut D) -> Self {
        self
    }
}
impl<I: Interner<Span = Self>> ty::inherent::Span<I> for Span {
    fn dummy() -> Self {
        Self {
            origin: Digest::default(),
            start: 0,
            end: 0,
        }
    }
}
impl<I: Interner<Safety = Self>> ty::inherent::Safety<I> for Safety {
    fn safe() -> Self {
        Self::Safe
    }
    fn unsafe_mode() -> Self {
        Self::Unsafe
    }
    fn is_safe(self) -> bool {
        self == Self::Safe
    }
    fn prefix_str(self) -> &'static str {
        if matches!(self, Self::Safe) {
            ""
        } else {
            "unsafe "
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct EarlyRegion<'cx> {
    pub(crate) parameter: SourceParameter,
    pub(crate) name: Option<Symbol<'cx>>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct LateRegion<'cx> {
    pub(crate) owner: DefinitionId,
    pub(crate) name: Option<Symbol<'cx>>,
}
impl ParamLike for EarlyRegion<'_> {
    fn index(self) -> u32 {
        self.parameter.index()
    }
}
impl<'cx, I: Interner<Symbol = Symbol<'cx>>> RegionName<I> for EarlyRegion<'cx> {
    fn get_name(&self, _: I) -> Option<I::Symbol> {
        self.name
    }
    fn is_named(&self, _: I) -> bool {
        self.name.is_some()
    }
}
impl<'cx, I: Interner<Symbol = Symbol<'cx>>> RegionName<I> for LateRegion<'cx> {
    fn get_name(&self, _: I) -> Option<I::Symbol> {
        self.name
    }
    fn is_named(&self, _: I) -> bool {
        self.name.is_some()
    }
}
impl<I: Interner<DefId = DefinitionId>> DefIdGetter<I> for LateRegion<'_> {
    fn get_def_id(self) -> Option<DefinitionId> {
        Some(self.owner)
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct FeatureSet<'cx> {
    pub(crate) generic_const_exprs: bool,
    pub(crate) gca_const_items: bool,
    pub(crate) coroutine_clone: bool,
    pub(crate) enabled: &'cx [Symbol<'cx>],
}
impl<'cx, F: SourceFacts<'cx>> Features<Context<'cx, F>> for FeatureSet<'cx> {
    fn generic_const_exprs(self) -> bool {
        self.generic_const_exprs
    }
    fn gca_const_items(self) -> bool {
        self.gca_const_items
    }
    fn coroutine_clone(self) -> bool {
        self.coroutine_clone
    }
    fn feature_bound_holds_in_crate(self, s: Symbol<'cx>) -> bool {
        self.enabled.contains(&s)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Ty<'cx, F: SourceFacts<'cx>>(
    pub(super) Interned<'cx, WithCachedTypeInfo<TyKind<Context<'cx, F>>>>,
);
impl<'cx, F: SourceFacts<'cx>> Ty<'cx, F> {
    pub(crate) fn new(cx: Context<'cx, F>, kind: TyKind<Context<'cx, F>>) -> Self {
        cx.ty(kind)
    }
}
impl<'cx, F: SourceFacts<'cx>> IntoKind for Ty<'cx, F> {
    type Kind = TyKind<Context<'cx, F>>;
    fn kind(self) -> Self::Kind {
        self.0.internee
    }
}
impl<'cx, F: SourceFacts<'cx>> Flags for Ty<'cx, F> {
    fn flags(&self) -> TypeFlags {
        self.0.flags
    }
    fn outer_exclusive_binder(&self) -> DebruijnIndex {
        self.0.outer_exclusive_binder
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for Ty<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        v.visit_ty(*self)
    }
}

impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for Ty<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        d.try_fold_ty(self)
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        d.fold_ty(self)
    }
}

impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Ty<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        r: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        r.tys(a, b)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum Arg<'cx, F: SourceFacts<'cx>> {
    Type(Ty<'cx, F>),
    Region(Region<Context<'cx, F>>),
    Const(Const<Context<'cx, F>>),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum Term<'cx, F: SourceFacts<'cx>> {
    Type(Ty<'cx, F>),
    Const(Const<Context<'cx, F>>),
}
pub(crate) type Args<'cx, F> = List<'cx, Arg<'cx, F>>;
pub(crate) type Types<'cx, F> = List<'cx, Ty<'cx, F>>;
pub(crate) type Regions<'cx, F> = List<'cx, ty::OutlivesClause<Context<'cx, F>, Arg<'cx, F>>>;
impl<'cx, F: SourceFacts<'cx>> From<Ty<'cx, F>> for Arg<'cx, F> {
    fn from(x: Ty<'cx, F>) -> Self {
        Self::Type(x)
    }
}
impl<'cx, F: SourceFacts<'cx>> From<Region<Context<'cx, F>>> for Arg<'cx, F> {
    fn from(x: Region<Context<'cx, F>>) -> Self {
        Self::Region(x)
    }
}
impl<'cx, F: SourceFacts<'cx>> From<Const<Context<'cx, F>>> for Arg<'cx, F> {
    fn from(x: Const<Context<'cx, F>>) -> Self {
        Self::Const(x)
    }
}
impl<'cx, F: SourceFacts<'cx>> From<Term<'cx, F>> for Arg<'cx, F> {
    fn from(x: Term<'cx, F>) -> Self {
        match x {
            Term::Type(t) => Self::Type(t),
            Term::Const(c) => Self::Const(c),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> From<Ty<'cx, F>> for Term<'cx, F> {
    fn from(x: Ty<'cx, F>) -> Self {
        Self::Type(x)
    }
}
impl<'cx, F: SourceFacts<'cx>> From<Const<Context<'cx, F>>> for Term<'cx, F> {
    fn from(x: Const<Context<'cx, F>>) -> Self {
        Self::Const(x)
    }
}
impl<'cx, F: SourceFacts<'cx>> IntoKind for Arg<'cx, F> {
    type Kind = ty::GenericArgKind<Context<'cx, F>>;
    fn kind(self) -> Self::Kind {
        match self {
            Self::Type(t) => GenericArgKind::Type(t),
            Self::Const(c) => GenericArgKind::Const(c),
            Self::Region(r) => GenericArgKind::Lifetime(r),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> IntoKind for Term<'cx, F> {
    type Kind = ty::TermKind<Context<'cx, F>>;
    fn kind(self) -> Self::Kind {
        match self {
            Self::Type(t) => TermKind::Ty(t),
            Self::Const(c) => TermKind::Const(c),
        }
    }
}
macro_rules! sum_fold {
 ($name:ident,$($variant:ident),+)=>{
 impl<'cx,F:SourceFacts<'cx>> TypeVisitable<Context<'cx,F>> for $name<'cx,F>{fn visit_with<V:TypeVisitor<Context<'cx,F>>>(&self,v:&mut V)->V::Result{match self{$(Self::$variant(x)=>x.visit_with(v)),+}}}
 impl<'cx,F:SourceFacts<'cx>> TypeFoldable<Context<'cx,F>> for $name<'cx,F>{
 fn try_fold_with<D:FallibleTypeFolder<Context<'cx,F>>>(self,d:&mut D)->Result<Self,D::Error>{match self{$(Self::$variant(x)=>Ok(Self::$variant(x.try_fold_with(d)?))),+}}
 fn fold_with<D:TypeFolder<Context<'cx,F>>>(self,d:&mut D)->Self{match self{$(Self::$variant(x)=>Self::$variant(x.fold_with(d))),+}}
 }};
}
sum_fold!(Arg, Type, Region, Const);
sum_fold!(Term, Type, Const);
impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Arg<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        r: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        match (a, b) {
            (Self::Type(a), Self::Type(b)) => Ok(Self::Type(r.relate(a, b)?)),
            (Self::Region(a), Self::Region(b)) => Ok(Self::Region(r.relate(a, b)?)),
            (Self::Const(a), Self::Const(b)) => Ok(Self::Const(r.relate(a, b)?)),
            _ => Err(ty::error::TypeError::Mismatch),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Term<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        r: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        match (a, b) {
            (Self::Type(a), Self::Type(b)) => Ok(Self::Type(r.relate(a, b)?)),
            (Self::Const(a), Self::Const(b)) => Ok(Self::Const(r.relate(a, b)?)),
            _ => Err(ty::error::TypeError::Mismatch),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> GenericArg<Context<'cx, F>> for Arg<'cx, F> {}
impl<'cx, F: SourceFacts<'cx>> ty::inherent::Term<Context<'cx, F>> for Term<'cx, F> {}
impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Args<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        r: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        if a.items().len() != b.items().len() {
            return Err(ty::error::TypeError::ArgCount);
        }
        ty::relate::relate_args_invariantly(r, a, b)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Pattern<'cx, F: SourceFacts<'cx>>(
    pub(super) Interned<'cx, WithCachedTypeInfo<PatternKind<Context<'cx, F>>>>,
);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ValueConst<'cx, F: SourceFacts<'cx>> {
    pub(crate) ty: Ty<'cx, F>,
    pub(crate) tree: ValTree<'cx, F>,
}
pub(crate) use super::constant_expression::ExprConst;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ValTree<'cx, F: SourceFacts<'cx>>(
    pub(super) Interned<'cx, ValTreeKind<Context<'cx, F>>>,
);
impl<'cx, F: SourceFacts<'cx>> IntoKind for ValTree<'cx, F> {
    type Kind = ValTreeKind<Context<'cx, F>>;
    fn kind(self) -> Self::Kind {
        *self.0
    }
}
impl<'cx, F: SourceFacts<'cx>> ty::inherent::ValueConst<Context<'cx, F>> for ValueConst<'cx, F> {
    fn ty(self) -> Ty<'cx, F> {
        self.ty
    }
    fn valtree(self) -> ValTree<'cx, F> {
        self.tree
    }
}
impl<'cx, F: SourceFacts<'cx>> fmt::Display for ValueConst<'cx, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.tree)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for ValueConst<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        ty::try_visit!(self.ty.visit_with(v));
        self.tree.kind().visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for ValueConst<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        let cx = d.cx();
        Ok(Self {
            ty: self.ty.try_fold_with(d)?,
            tree: cx.valtree(self.tree.kind().try_fold_with(d)?),
        })
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        let cx = d.cx();
        Self {
            ty: self.ty.fold_with(d),
            tree: cx.valtree(self.tree.kind().fold_with(d)),
        }
    }
}

impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for ExprConst<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.args().visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for ExprConst<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        Ok(Self::new(self.kind, self.args().try_fold_with(d)?))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        Self::new(self.kind, self.args().fold_with(d))
    }
}

impl<'cx, F: SourceFacts<'cx>> IntoKind for Pattern<'cx, F> {
    type Kind = PatternKind<Context<'cx, F>>;
    fn kind(self) -> Self::Kind {
        self.0.internee
    }
}
impl<'cx, F: SourceFacts<'cx>> Flags for Pattern<'cx, F> {
    fn flags(&self) -> TypeFlags {
        self.0.flags
    }
    fn outer_exclusive_binder(&self) -> DebruijnIndex {
        self.0.outer_exclusive_binder
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for Pattern<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.kind().visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for Pattern<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        let cx = d.cx();
        Ok(cx.pattern(self.kind().try_fold_with(d)?))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        let cx = d.cx();
        cx.pattern(self.kind().fold_with(d))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ParamEnv<'cx, F: SourceFacts<'cx>>(pub(crate) Clauses<'cx, F>);
impl<'cx, F: SourceFacts<'cx>> ty::inherent::ParamEnv<Context<'cx, F>> for ParamEnv<'cx, F> {
    fn caller_bounds(self) -> impl Iterator<Item = Clause<'cx, F>> {
        self.0.iter()
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for ParamEnv<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.0.visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for ParamEnv<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        Ok(Self(self.0.try_fold_with(d)?))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        Self(self.0.fold_with(d))
    }
}
pub(super) type PredicateData<'cx, F> =
    WithCachedTypeInfo<Binder<Context<'cx, F>, PredicateKind<Context<'cx, F>>>>;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Predicate<'cx, F: SourceFacts<'cx>>(
    pub(super) Interned<'cx, PredicateData<'cx, F>>,
);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Clause<'cx, F: SourceFacts<'cx>>(pub(super) Predicate<'cx, F>);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Clauses<'cx, F: SourceFacts<'cx>>(
    pub(super) Interned<'cx, WithCachedTypeInfo<List<'cx, Clause<'cx, F>>>>,
);
impl<'cx, F: SourceFacts<'cx>> IntoKind for Predicate<'cx, F> {
    type Kind = Binder<Context<'cx, F>, PredicateKind<Context<'cx, F>>>;
    fn kind(self) -> Self::Kind {
        self.0.internee
    }
}
impl<'cx, F: SourceFacts<'cx>> IntoKind for Clause<'cx, F> {
    type Kind = Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>;
    fn kind(self) -> Self::Kind {
        self.0.kind().map_bound(|k| match k {
            PredicateKind::Clause(c) => c,
            _ => unreachable!("only real clause predicates construct Clause"),
        })
    }
}
impl<'cx, F: SourceFacts<'cx>> Clause<'cx, F> {
    pub(crate) fn new(
        cx: Context<'cx, F>,
        kind: Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>,
    ) -> Self {
        Self(cx.predicate(kind.map_bound(PredicateKind::Clause)))
    }
}
impl<'cx, F: SourceFacts<'cx>> ty::inherent::Predicate<Context<'cx, F>> for Predicate<'cx, F> {
    fn as_clause(self) -> Option<Clause<'cx, F>> {
        matches!(self.kind().skip_binder(), PredicateKind::Clause(_)).then_some(Clause(self))
    }
}
impl<'cx, F: SourceFacts<'cx>> ty::inherent::Clause<Context<'cx, F>> for Clause<'cx, F> {
    fn as_predicate(self) -> Predicate<'cx, F> {
        self.0
    }
    fn instantiate_supertrait(
        self,
        cx: Context<'cx, F>,
        r: Binder<Context<'cx, F>, TraitRef<Context<'cx, F>>>,
    ) -> Self {
        self.instantiate_supertrait_upstream(cx, r)
    }
}
impl<'cx, F: SourceFacts<'cx>> SliceLike for Clauses<'cx, F> {
    type Item = Clause<'cx, F>;
    type IntoIter = <List<'cx, Clause<'cx, F>> as SliceLike>::IntoIter;
    fn iter(self) -> Self::IntoIter {
        self.0.internee.iter()
    }
    fn as_slice(&self) -> &[Self::Item] {
        self.0.internee.items()
    }
}
impl<'cx, F: SourceFacts<'cx>> ty::inherent::Clauses<Context<'cx, F>> for Clauses<'cx, F> {}
impl<'cx, F: SourceFacts<'cx>> Flags for Predicate<'cx, F> {
    fn flags(&self) -> TypeFlags {
        self.0.flags
    }
    fn outer_exclusive_binder(&self) -> DebruijnIndex {
        self.0.outer_exclusive_binder
    }
}
impl<'cx, F: SourceFacts<'cx>> Flags for Clause<'cx, F> {
    fn flags(&self) -> TypeFlags {
        self.0.flags()
    }
    fn outer_exclusive_binder(&self) -> DebruijnIndex {
        self.0.outer_exclusive_binder()
    }
}
impl<'cx, F: SourceFacts<'cx>> Flags for Clauses<'cx, F> {
    fn flags(&self) -> TypeFlags {
        self.0.flags
    }
    fn outer_exclusive_binder(&self) -> DebruijnIndex {
        self.0.outer_exclusive_binder
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Existentials<'cx, F: SourceFacts<'cx>>(
    pub(crate) List<'cx, Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>>,
);
impl<'cx, F: SourceFacts<'cx>> SliceLike for Existentials<'cx, F> {
    type Item = Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>;
    type IntoIter = <List<'cx, Self::Item> as SliceLike>::IntoIter;
    fn iter(self) -> Self::IntoIter {
        self.0.iter()
    }
    fn as_slice(&self) -> &[Self::Item] {
        self.0.items()
    }
}
impl<'cx, F: SourceFacts<'cx>> BoundExistentialPredicates<Context<'cx, F>>
    for Existentials<'cx, F>
{
    fn principal_def_id(self) -> Option<DefinitionId> {
        self.principal().map(|p| p.skip_binder().def_id)
    }
    fn principal(self) -> Option<Binder<Context<'cx, F>, ExistentialTraitRef<Context<'cx, F>>>> {
        self.iter().find_map(|p| {
            p.map_bound(|p| match p {
                ExistentialPredicate::Trait(t) => Some(t),
                _ => None,
            })
            .transpose()
        })
    }
    fn auto_traits(self) -> impl IntoIterator<Item = DefinitionId> {
        self.iter().filter_map(|p| match p.skip_binder() {
            ExistentialPredicate::AutoTrait(id) => Some(id),
            _ => None,
        })
    }
    fn projection_bounds(
        self,
    ) -> impl IntoIterator<Item = Binder<Context<'cx, F>, ExistentialProjection<Context<'cx, F>>>>
    {
        self.iter().filter_map(|p| {
            p.map_bound(|p| match p {
                ExistentialPredicate::Projection(t) => Some(t),
                _ => None,
            })
            .transpose()
        })
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for Existentials<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.0.visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for Existentials<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        Ok(Self(self.0.try_fold_with(d)?))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        Self(self.0.fold_with(d))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum GenericParameter<'cx, F: SourceFacts<'cx>> {
    Type(SourceParameter),
    Region(EarlyRegion<'cx>),
    Const(SourceParameter, Ty<'cx, F>),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct Generics<'cx, F: SourceFacts<'cx>> {
    pub(crate) parameters: List<'cx, GenericParameter<'cx, F>>,
    pub(crate) parent_count: usize,
}
impl<'cx, F: SourceFacts<'cx>> GenericsOf<Context<'cx, F>> for Generics<'cx, F> {
    fn count(&self) -> usize {
        self.parameters.items().len()
    }
    fn param_region_def_id(self, cx: Context<'cx, F>, r: EarlyRegion<'cx>) -> DefinitionId {
        cx.services().parameter_definition(cx, r.parameter)
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct AdtDefinition<'cx, F: SourceFacts<'cx>> {
    pub(crate) context: Context<'cx, F>,
    pub(crate) definition: DefinitionId,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AdtKind {
    Struct,
    Enum,
    Union,
}
/// Declaration-order metadata shared by ordinary source and expanded source.
/// Variant and field identity is its position in the retained owner schema;
/// neither needs a fabricated standalone source DefinitionId.
pub(crate) struct AdtVariant<'cx, F: SourceFacts<'cx>> {
    pub(crate) name: Symbol<'cx>,
    pub(crate) fields: &'cx [AdtField<'cx, F>],
}
pub(crate) struct AdtField<'cx, F: SourceFacts<'cx>> {
    pub(crate) name: Symbol<'cx>,
    pub(crate) ty: EarlyBinder<Context<'cx, F>, Ty<'cx, F>>,
}
impl<'cx, F: SourceFacts<'cx>> AdtField<'cx, F> {
    /// Pinned rustc_middle::ty::FieldDef::ty: preserve the unnormalized result.
    pub(super) fn ty(
        &self,
        cx: Context<'cx, F>,
        args: Args<'cx, F>,
    ) -> Unnormalized<Context<'cx, F>, Ty<'cx, F>> {
        self.ty.instantiate(cx, args)
    }
}
pub(crate) struct AdtInfo<'cx, F: SourceFacts<'cx>> {
    pub(crate) kind: AdtKind,
    /// Actual enum source repr, not a caller-supplied integer Type.
    pub(crate) discriminant: Option<super::enum_repr::PreparedEnumDiscriminant>,
    pub(crate) packed: bool,
    pub(crate) phantom_data: bool,
    pub(crate) manually_drop: bool,
    pub(crate) unsafe_fields: bool,
    pub(crate) fundamental: bool,
    pub(crate) variants: &'cx [AdtVariant<'cx, F>],
    /// Same declaration-order fields as `variants`, flattened for upstream's
    /// AdtDef interface; prepared publication verifies the two views agree.
    pub(crate) fields: Types<'cx, F>,
    pub(crate) tail: Option<Ty<'cx, F>>,
    pub(crate) sized: Option<Ty<'cx, F>>,
    pub(crate) meta_sized: Option<Ty<'cx, F>>,
    pub(crate) destructor: Option<bool>,
}
impl<'cx, F: SourceFacts<'cx>> AdtDefinition<'cx, F> {
    pub(super) fn variants(self) -> &'cx [AdtVariant<'cx, F>] {
        self.info().variants
    }
    pub(super) fn variant(self, index: rustc_abi::VariantIdx) -> &'cx AdtVariant<'cx, F> {
        &self.variants()[index.as_usize()]
    }
    pub(super) fn is_enum(self) -> bool { self.info().kind == AdtKind::Enum }
    pub(super) fn enum_discriminant_type(self, cx: Context<'cx,F>) -> Ty<'cx,F> {
        assert_eq!(cx, self.context, "ADT discriminant belongs to another context");
        self.info().discriminant.as_ref().expect("actual enum repr was not prepared").ty(cx,self.definition)
    }
    fn info(self) -> &'cx AdtInfo<'cx, F> {
        self.context
            .services()
            .adt_info(self.context, self.definition)
    }
}
impl<'cx, F: SourceFacts<'cx>> AdtDef<Context<'cx, F>> for AdtDefinition<'cx, F> {
    fn def_id(self) -> DefinitionId {
        self.definition
    }
    fn is_struct(self) -> bool {
        self.info().kind == AdtKind::Struct
    }
    fn is_packed(self) -> bool {
        self.info().packed
    }
    fn is_phantom_data(self) -> bool {
        self.info().phantom_data
    }
    fn is_manually_drop(self) -> bool {
        self.info().manually_drop
    }
    fn struct_tail_ty(
        self,
        _: Context<'cx, F>,
    ) -> Option<EarlyBinder<Context<'cx, F>, Ty<'cx, F>>> {
        assert!(self.is_struct());
        self.info().tail.map(|t| EarlyBinder::bind(self.context, t))
    }
    fn field_representing_type_info(
        self,
        cx: Context<'cx, F>,
        args: Args<'cx, F>,
    ) -> Option<FieldInfo<Context<'cx, F>>> {
        cx.services()
            .field_representing_type_info(cx, self.definition, args)
    }
    fn all_field_tys(
        self,
        _: Context<'cx, F>,
    ) -> EarlyBinder<Context<'cx, F>, impl IntoIterator<Item = Ty<'cx, F>>> {
        EarlyBinder::bind(self.context, self.info().fields)
    }
    fn sizedness_constraint(
        self,
        _: Context<'cx, F>,
        kind: ty::solve::SizedTraitKind,
    ) -> Option<EarlyBinder<Context<'cx, F>, Ty<'cx, F>>> {
        match kind {
            ty::solve::SizedTraitKind::Sized => self.info().sized,
            ty::solve::SizedTraitKind::MetaSized => self.info().meta_sized,
        }
        .map(|t| EarlyBinder::bind(self.context, t))
    }
    fn is_fundamental(self) -> bool {
        self.info().fundamental
    }
    fn destructor(self, _: Context<'cx, F>) -> Option<ty::solve::AdtDestructorKind> {
        self.info().destructor.map(|c| {
            if c {
                ty::solve::AdtDestructorKind::Const
            } else {
                ty::solve::AdtDestructorKind::NotConst
            }
        })
    }
}

pub(super) trait AllocateValue<'cx, T> {
    fn value(self, value: T) -> Interned<'cx, T>;
}
impl<'cx, F: SourceFacts<'cx>, T: TypeVisitable<Context<'cx, F>>> TypeVisitable<Context<'cx, F>>
    for Interned<'cx, T>
{
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.value().visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>, T: TypeFoldable<Context<'cx, F>>> TypeFoldable<Context<'cx, F>>
    for Interned<'cx, T>
where
    Context<'cx, F>: AllocateValue<'cx, T>,
{
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        let cx = d.cx();
        Ok(cx.value(self.value().clone().try_fold_with(d)?))
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        let cx = d.cx();
        cx.value(self.value().clone().fold_with(d))
    }
}

impl<'cx, F: SourceFacts<'cx>> std::ops::Deref for Pattern<'cx, F> {
    type Target = PatternKind<Context<'cx, F>>;
    fn deref(&self) -> &Self::Target {
        &self.0.internee
    }
}
impl<'cx, F: SourceFacts<'cx>> IntoIterator for Existentials<'cx, F> {
    type Item = Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>;
    type IntoIter = <List<'cx, Self::Item> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
