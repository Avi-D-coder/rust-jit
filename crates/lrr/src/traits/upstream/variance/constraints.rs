//! Pinned variance inference algorithm; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_variance.py; source-context adapters only.
use super::terms::{VarianceTerm::*, *};
use super::{Args, Cx, DefinitionId, Ty, VarianceSource};
use rustc_type_ir::{self as ty, GenericArgKind, inherent::*};
use tracing::{debug, instrument};
pub(crate) struct ConstraintContext<'a, 'cx> {
    pub terms_cx: TermsContext<'a, 'cx>,

    // These are pointers to common `ConstantTerm` instances
    covariant: VarianceTermPtr<'a>,
    contravariant: VarianceTermPtr<'a>,
    invariant: VarianceTermPtr<'a>,
    bivariant: VarianceTermPtr<'a>,

    pub constraints: Vec<Constraint<'a>>,
}

/// Declares that the variable `decl_id` appears in a location with
/// variance `variance`.
#[derive(Copy, Clone)]
pub(crate) struct Constraint<'a> {
    pub inferred: InferredIndex,
    pub variance: &'a VarianceTerm<'a>,
}

/// To build constraints, we visit one item (type, trait) at a time
/// and look at its contents. So e.g., if we have
/// ```ignore (illustrative)
/// struct Foo<T> {
///     b: Bar<T>
/// }
/// ```
/// then while we are visiting `Bar<T>`, the `CurrentItem` would have
/// the `DefinitionId` and the start of `Foo`'s inferreds.
struct CurrentItem {
    inferred_start: InferredIndex,
}

pub(crate) fn add_constraints_from_crate<'a, 'cx>(
    terms_cx: TermsContext<'a, 'cx>,
) -> ConstraintContext<'a, 'cx> {
    let tcx = terms_cx.tcx;
    let covariant = terms_cx.arena.alloc(ConstantTerm(ty::Covariant));
    let contravariant = terms_cx.arena.alloc(ConstantTerm(ty::Contravariant));
    let invariant = terms_cx.arena.alloc(ConstantTerm(ty::Invariant));
    let bivariant = terms_cx.arena.alloc(ConstantTerm(ty::Bivariant));
    let mut constraint_cx = ConstraintContext {
        terms_cx,
        covariant,
        contravariant,
        invariant,
        bivariant,
        constraints: Vec::new(),
    };

    for def_id in tcx.inference_items() {
        constraint_cx.build_constraints_for_item(def_id);
    }

    constraint_cx
}

impl<'a, 'cx> ConstraintContext<'a, 'cx> {
    fn tcx(&self) -> VarianceSource<'a, 'cx> {
        self.terms_cx.tcx
    }

    fn build_constraints_for_item(&mut self, def_id: DefinitionId) {
        let tcx = self.tcx();
        debug!("build_constraints_for_item({})", tcx.def_path_str(def_id));

        // Skip items with no generics - there's nothing to infer in them.
        if tcx.generics_of(def_id).is_empty() {
            return;
        }

        let inferred_start = self.terms_cx.inferred_starts[&def_id];
        let current_item = &CurrentItem { inferred_start };
        let ty = tcx.definition_type(def_id);

        match ty.kind() {
            ty::Adt(_, _) => {
                // Not entirely obvious: constraints on structs/enums do not
                // affect the variance of their type parameters. See discussion
                // in comment at top of module.
                //
                // self.add_constraints_from_generics(generics);

                for field_ty in tcx.field_types(def_id) {
                    self.add_constraints_from_ty(current_item, *field_ty, self.covariant);
                }
            }

            ty::FnDef(..) => {
                self.add_constraints_from_sig(current_item, tcx.fn_sig(def_id), self.covariant);
            }

            ty::Error(_) => {}

            _ => {
                panic!("`build_constraints_for_item` unsupported for this item");
            }
        }
    }

    fn add_constraint(&mut self, current: &CurrentItem, index: u32, variance: VarianceTermPtr<'a>) {
        debug!("add_constraint(index={}, variance={:?})", index, variance);
        self.constraints.push(Constraint {
            inferred: InferredIndex(current.inferred_start.0 + index as usize),
            variance,
        });
    }

    fn contravariant(&mut self, variance: VarianceTermPtr<'a>) -> VarianceTermPtr<'a> {
        self.xform(variance, self.contravariant)
    }

    fn invariant(&mut self, variance: VarianceTermPtr<'a>) -> VarianceTermPtr<'a> {
        self.xform(variance, self.invariant)
    }

    fn constant_term(&self, v: ty::Variance) -> VarianceTermPtr<'a> {
        match v {
            ty::Covariant => self.covariant,
            ty::Invariant => self.invariant,
            ty::Contravariant => self.contravariant,
            ty::Bivariant => self.bivariant,
        }
    }

    fn xform(&mut self, v1: VarianceTermPtr<'a>, v2: VarianceTermPtr<'a>) -> VarianceTermPtr<'a> {
        match (*v1, *v2) {
            (_, ConstantTerm(ty::Covariant)) => {
                // Applying a "covariant" transform is always a no-op
                v1
            }

            (ConstantTerm(c1), ConstantTerm(c2)) => self.constant_term(c1.xform(c2)),

            _ => &*self.terms_cx.arena.alloc(TransformTerm(v1, v2)),
        }
    }

    #[instrument(level = "debug", skip(self, current))]
    fn add_constraints_from_invariant_args(
        &mut self,
        current: &CurrentItem,
        args: Args<'cx>,
        variance: VarianceTermPtr<'a>,
    ) {
        // Trait are always invariant so we can take advantage of that.
        let variance_i = self.invariant(variance);

        for arg in args {
            match arg.kind() {
                GenericArgKind::Lifetime(lt) => {
                    self.add_constraints_from_region(current, lt, variance_i)
                }
                GenericArgKind::Type(ty) => self.add_constraints_from_ty(current, ty, variance_i),
                GenericArgKind::Const(val) => {
                    self.add_constraints_from_const(current, val, variance_i)
                }
            }
        }
    }

    /// Adds constraints appropriate for an instance of `ty` appearing
    /// in a context with the generics defined in `generics` and
    /// ambient variance `variance`
    #[instrument(level = "debug", skip(self, current))]
    fn add_constraints_from_ty(
        &mut self,
        current: &CurrentItem,
        ty: Ty<'cx>,
        variance: VarianceTermPtr<'a>,
    ) {
        match ty.kind() {
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Str
            | ty::Never
            | ty::Foreign(..) => {
                // leaf type -- noop
            }

            ty::FnDef(..) | ty::Coroutine(..) | ty::Closure(..) | ty::CoroutineClosure(..) => {
                panic!("Unexpected unnameable type in variance computation: {ty:?}");
            }

            ty::Ref(region, ty, mutbl) => {
                self.add_constraints_from_region(current, region, variance);
                self.add_constraints_from_mt(current, &ty::TypeAndMut { ty, mutbl }, variance);
            }

            ty::Array(typ, len) => {
                self.add_constraints_from_const(current, len, variance);
                self.add_constraints_from_ty(current, typ, variance);
            }

            ty::Pat(typ, pat) => {
                self.add_constraints_from_pat(current, variance, pat);
                self.add_constraints_from_ty(current, typ, variance);
            }

            ty::Slice(typ) => {
                self.add_constraints_from_ty(current, typ, variance);
            }

            ty::RawPtr(ty, mutbl) => {
                self.add_constraints_from_mt(current, &ty::TypeAndMut { ty, mutbl }, variance);
            }

            ty::Tuple(subtys) => {
                for subty in subtys {
                    self.add_constraints_from_ty(current, subty, variance);
                }
            }

            ty::Adt(def, args) => {
                self.add_constraints_from_args(current, def.definition, args, variance);
            }

            ty::Alias(
                _,
                ty::AliasTy {
                    kind: ty::Projection { .. } | ty::Inherent { .. } | ty::Opaque { .. },
                    args,
                    ..
                },
            ) => {
                self.add_constraints_from_invariant_args(current, args, variance);
            }

            ty::Alias(
                _,
                ty::AliasTy {
                    kind: ty::Free { .. },
                    ..
                },
            ) => {
                let ty = self.tcx().expand_free_alias_tys(ty);
                self.add_constraints_from_ty(current, ty, variance);
            }

            ty::Dynamic(data, r) => {
                // The type `dyn Trait<T> +'a` is covariant w/r/t `'a`:
                self.add_constraints_from_region(current, r, variance);

                if let Some(poly_trait_ref) = data.principal() {
                    self.add_constraints_from_invariant_args(
                        current,
                        poly_trait_ref.skip_binder().args,
                        variance,
                    );
                }

                for projection in data.projection_bounds() {
                    match projection.skip_binder().term.kind() {
                        ty::TermKind::Ty(ty) => {
                            self.add_constraints_from_ty(current, ty, self.invariant);
                        }
                        ty::TermKind::Const(c) => {
                            self.add_constraints_from_const(current, c, self.invariant)
                        }
                    }
                }
            }

            ty::Param(ref data) => {
                self.add_constraint(current, data.index(), variance);
            }

            ty::FnPtr(sig_tys, hdr) => {
                self.add_constraints_from_sig(current, sig_tys.with(hdr), variance);
            }

            ty::UnsafeBinder(ty) => {
                // FIXME(unsafe_binders): This is covariant, right?
                self.add_constraints_from_ty(current, ty.skip_binder(), variance);
            }

            ty::Error(_) => {
                // we encounter this when walking the trait references for object
                // types, where we use Error as the Self type
            }

            ty::Placeholder(..) | ty::CoroutineWitness(..) | ty::Bound(..) | ty::Infer(..) => {
                panic!(
                    "unexpected type encountered in variance inference: {:?}",
                    ty
                );
            }
        }
    }

    fn add_constraints_from_pat(
        &mut self,
        current: &CurrentItem,
        variance: VarianceTermPtr<'a>,
        pat: <Cx<'cx> as ty::Interner>::Pat,
    ) {
        match *pat {
            ty::PatternKind::Range { start, end } => {
                self.add_constraints_from_const(current, start, variance);
                self.add_constraints_from_const(current, end, variance);
            }
            ty::PatternKind::NotNull => {}
            ty::PatternKind::Or(patterns) => {
                for pat in patterns {
                    self.add_constraints_from_pat(current, variance, pat)
                }
            }
        }
    }

    /// Adds constraints appropriate for a nominal type (enum, struct,
    /// object, etc) appearing in a context with ambient variance `variance`
    fn add_constraints_from_args(
        &mut self,
        current: &CurrentItem,
        def_id: DefinitionId,
        args: Args<'cx>,
        variance: VarianceTermPtr<'a>,
    ) {
        debug!(
            "add_constraints_from_args(def_id={:?}, args={:?}, variance={:?})",
            def_id, args, variance
        );

        // We don't record `inferred_starts` entries for empty generics.
        if args.is_empty() {
            return;
        }

        let (local, remote) = if let Some(def_id) = self.tcx().local_definition(def_id) {
            (Some(self.terms_cx.inferred_starts[&def_id]), None)
        } else {
            (None, Some(self.tcx().variances_of(def_id)))
        };

        for (i, arg) in args.iter().enumerate() {
            let variance_decl = if let Some(InferredIndex(start)) = local {
                // Parameter on an item defined within current crate:
                // variance not yet inferred, so return a symbolic
                // variance.
                self.terms_cx.inferred_terms[start + i]
            } else {
                // Parameter on an item defined within another crate:
                // variance already inferred, just look it up.
                self.constant_term(remote.as_ref().unwrap()[i])
            };
            let variance_i = self.xform(variance, variance_decl);
            debug!(
                "add_constraints_from_args: variance_decl={:?} variance_i={:?}",
                variance_decl, variance_i
            );
            match arg.kind() {
                GenericArgKind::Lifetime(lt) => {
                    self.add_constraints_from_region(current, lt, variance_i)
                }
                GenericArgKind::Type(ty) => self.add_constraints_from_ty(current, ty, variance_i),
                GenericArgKind::Const(val) => {
                    self.add_constraints_from_const(current, val, variance)
                }
            }
        }
    }

    /// Adds constraints appropriate for a const expression `val`
    /// in a context with ambient variance `variance`
    fn add_constraints_from_const(
        &mut self,
        current: &CurrentItem,
        c: ty::Const<Cx<'cx>>,
        variance: VarianceTermPtr<'a>,
    ) {
        debug!(
            "add_constraints_from_const(c={:?}, variance={:?})",
            c, variance
        );

        match &c.kind() {
            ty::ConstKind::Alias(_, alias_const) => {
                self.add_constraints_from_invariant_args(current, alias_const.args, variance);
            }
            _ => {}
        }
    }

    /// Adds constraints appropriate for a function with signature
    /// `sig` appearing in a context with ambient variance `variance`
    fn add_constraints_from_sig(
        &mut self,
        current: &CurrentItem,
        sig: ty::Binder<Cx<'cx>, ty::FnSig<Cx<'cx>>>,
        variance: VarianceTermPtr<'a>,
    ) {
        let contra = self.contravariant(variance);
        for &input in sig.skip_binder().inputs() {
            self.add_constraints_from_ty(current, input, contra);
        }
        self.add_constraints_from_ty(current, sig.skip_binder().output(), variance);
    }

    /// Adds constraints appropriate for a region appearing in a
    /// context with ambient variance `variance`
    fn add_constraints_from_region(
        &mut self,
        current: &CurrentItem,
        region: ty::Region<Cx<'cx>>,
        variance: VarianceTermPtr<'a>,
    ) {
        match region.kind() {
            ty::ReEarlyParam(ref data) => {
                self.add_constraint(current, data.index(), variance);
            }

            ty::ReStatic => {}

            ty::ReBound(..) => {
                // Either a higher-ranked region inside of a type or a
                // late-bound function parameter.
                //
                // We do not compute constraints for either of these.
            }

            ty::ReError(_) => {}

            ty::ReLateParam(..) | ty::ReVar(..) | ty::RePlaceholder(..) | ty::ReErased => {
                // We don't expect to see anything but 'static or bound
                // regions when visiting member types or method types.
                panic!(
                    "unexpected region encountered in variance \
                      inference: {:?}",
                    region
                );
            }
        }
    }

    /// Adds constraints appropriate for a mutability-type pair
    /// appearing in a context with ambient variance `variance`
    fn add_constraints_from_mt(
        &mut self,
        current: &CurrentItem,
        mt: &ty::TypeAndMut<Cx<'cx>>,
        variance: VarianceTermPtr<'a>,
    ) {
        match mt.mutbl {
            ty::Mutability::Mut => {
                let invar = self.invariant(variance);
                self.add_constraints_from_ty(current, mt.ty, invar);
            }

            ty::Mutability::Not => {
                self.add_constraints_from_ty(current, mt.ty, variance);
            }
        }
    }
}
