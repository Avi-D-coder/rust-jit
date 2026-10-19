//! Predicate wrappers retain shared binders and use upstream traversal hooks.
use super::{context::Context, facts::SourceFacts, model::*};
use rustc_type_ir::elaborate::Elaboratable;
use rustc_type_ir::{self as ty, inherent::*, *};

macro_rules! predicate_traversal {
    ($name:ident,$build:expr) => {
        impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for $name<'cx, F> {
            fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
                v.visit_predicate(*self)
            }
        }
        impl<'cx, F: SourceFacts<'cx>> TypeSuperVisitable<Context<'cx, F>> for $name<'cx, F> {
            fn super_visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
                self.kind().visit_with(v)
            }
        }
        impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for $name<'cx, F> {
            fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
                self,
                d: &mut D,
            ) -> Result<Self, D::Error> {
                d.try_fold_predicate(self)
            }
            fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
                d.fold_predicate(self)
            }
        }
        impl<'cx, F: SourceFacts<'cx>> TypeSuperFoldable<Context<'cx, F>> for $name<'cx, F> {
            fn try_super_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
                self,
                d: &mut D,
            ) -> Result<Self, D::Error> {
                let cx = d.cx();
                let kind = self.kind().try_fold_with(d)?;
                Ok(($build)(cx, kind))
            }
            fn super_fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
                let cx = d.cx();
                let kind = self.kind().fold_with(d);
                ($build)(cx, kind)
            }
        }
    };
}
predicate_traversal!(Predicate, |cx: Context<'cx, F>, k| cx.predicate(k));
predicate_traversal!(Clause, |cx: Context<'cx, F>, k| Clause::new(cx, k));
impl<'cx, F: SourceFacts<'cx>> PredicateProxy<Context<'cx, F>> for Predicate<'cx, F> {
    fn allow_normalization(&self) -> bool {
        ty::inherent::Predicate::allow_normalization(*self)
    }
    fn clause_kind_unchecked(
        &self,
    ) -> Option<Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>> {
        self.as_clause().map(|c| c.kind())
    }
    fn map_projection(
        self,
        cx: Context<'cx, F>,
        f: impl FnOnce(
            Binder<Context<'cx, F>, ProjectionClause<Context<'cx, F>>>,
        ) -> Binder<Context<'cx, F>, ProjectionClause<Context<'cx, F>>>,
    ) -> Option<Self> {
        self.as_clause()
            .and_then(|c| c.map_projection(cx, f))
            .map(|c| c.0)
    }
}
impl<'cx, F: SourceFacts<'cx>> PredicateProxy<Context<'cx, F>> for Clause<'cx, F> {
    fn allow_normalization(&self) -> bool {
        ty::inherent::Predicate::allow_normalization(self.0)
    }
    fn clause_kind_unchecked(
        &self,
    ) -> Option<Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>> {
        Some(self.kind())
    }
    fn map_projection(
        self,
        cx: Context<'cx, F>,
        f: impl FnOnce(
            Binder<Context<'cx, F>, ProjectionClause<Context<'cx, F>>>,
        ) -> Binder<Context<'cx, F>, ProjectionClause<Context<'cx, F>>>,
    ) -> Option<Self> {
        let projection = self
            .kind()
            .map_bound(|k| {
                if let ClauseKind::Projection(p) = k {
                    Some(p)
                } else {
                    None
                }
            })
            .transpose()?;
        Some(Clause::new(
            cx,
            f(projection).map_bound(ClauseKind::Projection),
        ))
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeVisitable<Context<'cx, F>> for Clauses<'cx, F> {
    fn visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        v.visit_clauses(*self)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeSuperVisitable<Context<'cx, F>> for Clauses<'cx, F> {
    fn super_visit_with<V: TypeVisitor<Context<'cx, F>>>(&self, v: &mut V) -> V::Result {
        self.0.internee.visit_with(v)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeFoldable<Context<'cx, F>> for Clauses<'cx, F> {
    fn try_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        d.try_fold_clauses(self)
    }
    fn fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        d.fold_clauses(self)
    }
}
impl<'cx, F: SourceFacts<'cx>> TypeSuperFoldable<Context<'cx, F>> for Clauses<'cx, F> {
    fn try_super_fold_with<D: FallibleTypeFolder<Context<'cx, F>>>(
        self,
        d: &mut D,
    ) -> Result<Self, D::Error> {
        let cx = d.cx();
        Ok(cx.clauses(self.0.internee.try_fold_with(d)?))
    }
    fn super_fold_with<D: TypeFolder<Context<'cx, F>>>(self, d: &mut D) -> Self {
        let cx = d.cx();
        cx.clauses(self.0.internee.fold_with(d))
    }
}
macro_rules! elaboratable {
    ($name:ident,$pred:expr,$child:expr) => {
        impl<'cx, F: SourceFacts<'cx>> Elaboratable<Context<'cx, F>> for $name<'cx, F> {
            fn predicate(&self) -> Predicate<'cx, F> {
                ($pred)(*self)
            }
            fn child(&self, c: Clause<'cx, F>) -> Self {
                ($child)(c)
            }
            fn child_with_derived_cause(
                &self,
                c: Clause<'cx, F>,
                _: Span,
                _: Binder<Context<'cx, F>, TraitClause<Context<'cx, F>>>,
                _: usize,
            ) -> Self {
                ($child)(c)
            }
        }
    };
}
elaboratable!(Predicate, |p| p, |c: Clause<'cx, F>| c.0);
elaboratable!(Clause, |c: Clause<'cx, F>| c.0, |c| c);
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, PredicateKind<Context<'cx, F>>>
    for Predicate<'cx, F>
{
    fn upcast_from(p: PredicateKind<Context<'cx, F>>, cx: Context<'cx, F>) -> Self {
        cx.predicate(Binder::dummy(p))
    }
}
impl<'cx, F: SourceFacts<'cx>>
    UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, PredicateKind<Context<'cx, F>>>>
    for Predicate<'cx, F>
{
    fn upcast_from(
        p: Binder<Context<'cx, F>, PredicateKind<Context<'cx, F>>>,
        cx: Context<'cx, F>,
    ) -> Self {
        cx.predicate(p)
    }
}
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, ClauseKind<Context<'cx, F>>>
    for Predicate<'cx, F>
{
    fn upcast_from(p: ClauseKind<Context<'cx, F>>, cx: Context<'cx, F>) -> Self {
        Clause::new(cx, Binder::dummy(p)).0
    }
}
impl<'cx, F: SourceFacts<'cx>>
    UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>>
    for Predicate<'cx, F>
{
    fn upcast_from(
        p: Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>,
        cx: Context<'cx, F>,
    ) -> Self {
        Clause::new(cx, p).0
    }
}
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, Clause<'cx, F>> for Predicate<'cx, F> {
    fn upcast_from(p: Clause<'cx, F>, _: Context<'cx, F>) -> Self {
        p.0
    }
}
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, NormalizesTo<Context<'cx, F>>>
    for Predicate<'cx, F>
{
    fn upcast_from(p: NormalizesTo<Context<'cx, F>>, cx: Context<'cx, F>) -> Self {
        cx.predicate(Binder::dummy(PredicateKind::NormalizesTo(p)))
    }
}
impl<'cx, F: SourceFacts<'cx>>
    UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>>
    for Clause<'cx, F>
{
    fn upcast_from(
        p: Binder<Context<'cx, F>, ClauseKind<Context<'cx, F>>>,
        cx: Context<'cx, F>,
    ) -> Self {
        Clause::new(cx, p)
    }
}
macro_rules! clause_casts {
    ($source:ty,$variant:ident) => {
        impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, $source> for Clause<'cx, F> {
            fn upcast_from(p: $source, cx: Context<'cx, F>) -> Self {
                Clause::new(cx, Binder::dummy(ClauseKind::$variant(p)))
            }
        }
        impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, $source>>
            for Clause<'cx, F>
        {
            fn upcast_from(p: Binder<Context<'cx, F>, $source>, cx: Context<'cx, F>) -> Self {
                Clause::new(cx, p.map_bound(ClauseKind::$variant))
            }
        }
        impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, $source> for Predicate<'cx, F> {
            fn upcast_from(p: $source, cx: Context<'cx, F>) -> Self {
                let c: Clause<'cx, F> = p.upcast(cx);
                c.0
            }
        }
        impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, $source>>
            for Predicate<'cx, F>
        {
            fn upcast_from(p: Binder<Context<'cx, F>, $source>, cx: Context<'cx, F>) -> Self {
                let c: Clause<'cx, F> = p.upcast(cx);
                c.0
            }
        }
    };
}
clause_casts!(TraitClause<Context<'cx, F>>, Trait);
clause_casts!(ProjectionClause<Context<'cx, F>>, Projection);
clause_casts!(OutlivesClause<Context<'cx, F>, Ty<'cx, F>>, TypeOutlives);
clause_casts!(
    OutlivesClause<Context<'cx, F>, Region<Context<'cx, F>>>,
    RegionOutlives
);
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, TraitRef<Context<'cx, F>>>
    for Clause<'cx, F>
{
    fn upcast_from(p: TraitRef<Context<'cx, F>>, cx: Context<'cx, F>) -> Self {
        let p: TraitClause<Context<'cx, F>> = p.upcast(cx);
        p.upcast(cx)
    }
}
impl<'cx, F: SourceFacts<'cx>>
    UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, TraitRef<Context<'cx, F>>>>
    for Clause<'cx, F>
{
    fn upcast_from(
        p: Binder<Context<'cx, F>, TraitRef<Context<'cx, F>>>,
        cx: Context<'cx, F>,
    ) -> Self {
        let p: Binder<Context<'cx, F>, TraitClause<Context<'cx, F>>> = p.upcast(cx);
        p.upcast(cx)
    }
}
impl<'cx, F: SourceFacts<'cx>> UpcastFrom<Context<'cx, F>, TraitRef<Context<'cx, F>>>
    for Predicate<'cx, F>
{
    fn upcast_from(p: TraitRef<Context<'cx, F>>, cx: Context<'cx, F>) -> Self {
        let c: Clause<'cx, F> = p.upcast(cx);
        c.0
    }
}
impl<'cx, F: SourceFacts<'cx>>
    UpcastFrom<Context<'cx, F>, Binder<Context<'cx, F>, TraitRef<Context<'cx, F>>>>
    for Predicate<'cx, F>
{
    fn upcast_from(
        p: Binder<Context<'cx, F>, TraitRef<Context<'cx, F>>>,
        cx: Context<'cx, F>,
    ) -> Self {
        let c: Clause<'cx, F> = p.upcast(cx);
        c.0
    }
}

use super::model::{
    Clause, Clauses, ExprConst, ParamEnv, Pattern, Predicate, Safety, Span, Symbol, Term, Ty,
    ValueConst,
};
use rustc_type_ir::inherent::{Clause as _, Predicate as _, Ty as _};
