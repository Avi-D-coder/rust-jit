//! Pinned variance inference algorithm; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_variance.py; source-context adapters only.
use super::constraints::*;
use super::terms::{VarianceTerm::*, *};
use super::{VarianceGenerics, VarianceMap, VarianceMapEntries, VarianceParameterKind};
use rustc_type_ir::{self as ty, inherent::*};
use std::collections::{BTreeMap, HashSet};
use tracing::debug;
fn glb(v1: ty::Variance, v2: ty::Variance) -> ty::Variance {
    // Greatest lower bound of the variance lattice as defined in The Paper:
    //
    //       *
    //    -     +
    //       o
    match (v1, v2) {
        (ty::Invariant, _) | (_, ty::Invariant) => ty::Invariant,

        (ty::Covariant, ty::Contravariant) => ty::Invariant,
        (ty::Contravariant, ty::Covariant) => ty::Invariant,

        (ty::Covariant, ty::Covariant) => ty::Covariant,

        (ty::Contravariant, ty::Contravariant) => ty::Contravariant,

        (x, ty::Bivariant) | (ty::Bivariant, x) => x,
    }
}
struct SolveContext<'a, 'cx> {
    terms_cx: TermsContext<'a, 'cx>,
    constraints: Vec<Constraint<'a>>,

    // Maps from an InferredIndex to the inferred value for that variable.
    solutions: Vec<ty::Variance>,
}

pub(crate) fn solve_constraints<'cx>(constraints_cx: ConstraintContext<'_, 'cx>) -> VarianceMap {
    let ConstraintContext {
        terms_cx,
        mut constraints,
        ..
    } = constraints_cx;

    let mut overridden_inferreds = HashSet::new();
    let mut solutions = vec![ty::Bivariant; terms_cx.inferred_terms.len()];
    // prime the solutions for certain lang items which have hard-coded variance
    for (id, variances) in &terms_cx.lang_items {
        let InferredIndex(start) = terms_cx.inferred_starts[id];
        for (i, &variance) in variances.iter().enumerate() {
            solutions[start + i] = variance;
            overridden_inferreds.insert(start + i);
        }
    }

    // ensure the solutions for overridden inferreds are never constrained by anything else
    if !overridden_inferreds.is_empty() {
        constraints.retain(
            |Constraint {
                 inferred: InferredIndex(inferred),
                 ..
             }| { !overridden_inferreds.contains(inferred) },
        );
    }

    let mut solutions_cx = SolveContext {
        terms_cx,
        constraints,
        solutions,
    };
    solutions_cx.solve();
    let variances = solutions_cx.create_map();

    VarianceMap { variances }
}

impl<'a, 'cx> SolveContext<'a, 'cx> {
    fn solve(&mut self) {
        // Propagate constraints until a fixed point is reached. Note
        // that the maximum number of iterations is 2C where C is the
        // number of constraints (each variable can change values at most
        // twice). Since number of constraints is linear in size of the
        // input, so is the inference process.
        let mut changed = true;
        while changed {
            changed = false;

            for constraint in &self.constraints {
                let Constraint {
                    inferred,
                    variance: term,
                } = *constraint;
                let InferredIndex(inferred) = inferred;
                let variance = self.evaluate(term);
                let old_value = self.solutions[inferred];
                let new_value = glb(variance, old_value);
                if old_value != new_value {
                    debug!(
                        "updating inferred {} \
                            from {:?} to {:?} due to {:?}",
                        inferred, old_value, new_value, term
                    );

                    self.solutions[inferred] = new_value;
                    changed = true;
                }
            }
        }
    }

    fn enforce_const_invariance(
        &self,
        generics: &VarianceGenerics,
        variances: &mut [ty::Variance],
    ) {
        let tcx = self.terms_cx.tcx;

        // Make all const parameters invariant.
        for param in generics.own_params.iter() {
            if let VarianceParameterKind::Const { .. } = param.kind {
                variances[param.index as usize] = ty::Invariant;
            }
        }

        // Make all the const parameters in the parent invariant (recursively).
        if let Some(def_id) = generics.parent {
            self.enforce_const_invariance(tcx.generics_of(def_id), variances);
        }
    }

    fn create_map(&self) -> VarianceMapEntries {
        let tcx = self.terms_cx.tcx;

        let solutions = &self.solutions;
        BTreeMap::from_iter(self.terms_cx.inferred_starts.iter().map(
            |(&def_id, &InferredIndex(start))| {
                let generics = tcx.generics_of(def_id);
                let count = generics.count();

                let mut variances = solutions[start..(start + count)].to_vec();

                // Const parameters are always invariant.
                self.enforce_const_invariance(generics, &mut variances);

                // Functions are permitted to have unused generic parameters: make those invariant.
                if let ty::FnDef(..) = tcx.definition_type(def_id).kind() {
                    for variance in variances.iter_mut() {
                        if *variance == ty::Bivariant {
                            *variance = ty::Invariant;
                        }
                    }
                }

                (def_id, variances)
            },
        ))
    }

    fn evaluate(&self, term: VarianceTermPtr<'a>) -> ty::Variance {
        match *term {
            ConstantTerm(v) => v,

            TransformTerm(t1, t2) => {
                let v1 = self.evaluate(t1);
                let v2 = self.evaluate(t2);
                v1.xform(v2)
            }

            InferredTerm(InferredIndex(index)) => self.solutions[index],
        }
    }
}
