//! Pinned variance inference algorithm; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_variance.py; source-context adapters only.
use self::VarianceTerm::*;
use super::{DefinitionId, VarianceSource};
use rustc_type_ir as ty;
use std::{collections::BTreeMap, fmt};
use tracing::debug;
use typed_arena::Arena;
pub(crate) type VarianceTermPtr<'a> = &'a VarianceTerm<'a>;

#[derive(Copy, Clone, Debug)]
pub(crate) struct InferredIndex(pub usize);

#[derive(Copy, Clone)]
pub(crate) enum VarianceTerm<'a> {
    ConstantTerm(ty::Variance),
    TransformTerm(VarianceTermPtr<'a>, VarianceTermPtr<'a>),
    InferredTerm(InferredIndex),
}

impl<'a> fmt::Debug for VarianceTerm<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ConstantTerm(c1) => write!(f, "{c1:?}"),
            TransformTerm(v1, v2) => write!(f, "({v1:?} \u{00D7} {v2:?})"),
            InferredTerm(id) => write!(f, "[{}]", {
                let InferredIndex(i) = id;
                i
            }),
        }
    }
}

// The first pass over the crate simply builds up the set of inferreds.

pub(crate) struct TermsContext<'a, 'cx> {
    pub tcx: VarianceSource<'a, 'cx>,
    pub arena: &'a Arena<VarianceTerm<'a>>,

    /// For marker types, `UnsafeCell`, and other lang items where
    /// variance is hardcoded, records the item-id and the hardcoded
    /// variance.
    pub lang_items: Vec<(DefinitionId, Vec<ty::Variance>)>,

    /// Maps from the node id of an item to the first inferred index
    /// used for its type & region parameters.
    pub inferred_starts: BTreeMap<DefinitionId, InferredIndex>,

    /// Maps from an InferredIndex to the term for that variable.
    pub inferred_terms: Vec<VarianceTermPtr<'a>>,
}

pub(crate) fn determine_parameters_to_be_inferred<'a, 'cx>(
    tcx: VarianceSource<'a, 'cx>,
    arena: &'a Arena<VarianceTerm<'a>>,
) -> TermsContext<'a, 'cx> {
    let mut terms_cx = TermsContext {
        tcx,
        arena,
        inferred_starts: Default::default(),
        inferred_terms: vec![],
        lang_items: lang_items(tcx),
    };
    for def_id in tcx.inference_items() {
        terms_cx.add_inferreds_for_item(def_id);
    }
    terms_cx
}

fn lang_items(tcx: VarianceSource<'_, '_>) -> Vec<(DefinitionId, Vec<ty::Variance>)> {
    let lang_items = tcx.lang_items();
    let all = [
        (lang_items.phantom_data(), vec![ty::Covariant]),
        (lang_items.unsafe_cell_type(), vec![ty::Invariant]),
        (lang_items.covariant_unsafe_cell_type(), vec![ty::Covariant]),
    ];

    all.into_iter() // iterating over (Option<DefinitionId>, Variance)
        .filter_map(|(d, v)| {
            let def_id = d?; // authenticated local inference item
            Some((def_id, v))
        })
        .collect()
}

impl<'a, 'cx> TermsContext<'a, 'cx> {
    fn add_inferreds_for_item(&mut self, def_id: DefinitionId) {
        let tcx = self.tcx;
        let count = tcx.generics_of(def_id).count();

        if count == 0 {
            return;
        }

        // Record the start of this item's inferreds.
        let start = self.inferred_terms.len();
        let newly_added = self
            .inferred_starts
            .insert(def_id, InferredIndex(start))
            .is_none();
        assert!(newly_added);

        // N.B., in the code below for writing the results back into the
        // `CrateVariancesMap`, we rely on the fact that all inferreds
        // for a particular item are assigned continuous indices.

        let arena = self.arena;
        self.inferred_terms.extend(
            (start..(start + count)).map(|i| &*arena.alloc(InferredTerm(InferredIndex(i)))),
        );
    }
}
