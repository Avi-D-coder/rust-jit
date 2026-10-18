//! Source predicate requirements, not canonical goals or fulfillment witnesses.
//! A Sized default still needs the configured language item and relaxed bounds.
use super::term::{ParamKey, RegionTerm, TraitRef, TyTerm};
use crate::{
    core::digest::{StableEncode, StableEncoder},
    syntax::spans::SourceRange,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PredicatePending {
    Subject,
    TraitName,
    SymbolicType,
    HigherRanked,
    AssociatedIdentity,
    QualifiedBound,
    LifetimeName,
    CallableArguments,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PredicateResult {
    Implemented(TraitRef),
    /// A relaxation is metadata, never a negative trait requirement.
    Relaxed {
        trait_ref: TraitRef,
        parameter: Option<ParamKey>,
    },
    RegionOutlives {
        longer: RegionTerm,
        shorter: RegionTerm,
    },
    TypeOutlives {
        ty: TyTerm,
        region: RegionTerm,
    },
    Pending(PredicatePending),
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourcePredicateRecipe {
    Explicit {
        source: SourceRange,
        result: PredicateResult,
    },
    /// No trait ID is guessed from the spelling `Sized`. Explicit relaxed
    /// bounds (including where clauses) remain independent source recipes.
    ImplicitSizedDefault(ParamKey),
    ResolvedSizedDefault(TraitRef),
}
impl StableEncode for SourcePredicateRecipe {
    const DOMAIN: &'static str = "lrr/v2/source-predicate-requirement";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::ImplicitSizedDefault(key) => {
                e.u32(0);
                e.record(key);
            }
            Self::ResolvedSizedDefault(trait_ref) => {
                e.u32(2);
                e.record(trait_ref);
            }
            Self::Explicit { source, result } => {
                e.u32(1);
                e.u32(source.start);
                e.u32(source.end);
                match result {
                    PredicateResult::Implemented(value) => {
                        e.u32(0);
                        e.record(value);
                    }
                    PredicateResult::RegionOutlives { longer, shorter } => {
                        e.u32(1);
                        e.record(longer);
                        e.record(shorter);
                    }
                    PredicateResult::TypeOutlives { ty, region } => {
                        e.u32(2);
                        e.record(ty);
                        e.record(region);
                    }
                    PredicateResult::Relaxed {
                        trait_ref,
                        parameter,
                    } => {
                        e.u32(4);
                        e.record(trait_ref);
                        e.option(parameter, |e, key| e.record(key));
                    }
                    PredicateResult::Pending(why) => {
                        e.u32(3);
                        e.u32(match why {
                            PredicatePending::Subject => 0,
                            PredicatePending::TraitName => 1,
                            PredicatePending::SymbolicType => 2,
                            PredicatePending::HigherRanked => 3,
                            PredicatePending::AssociatedIdentity => 4,
                            PredicatePending::QualifiedBound => 6,
                            PredicatePending::LifetimeName => 7,
                            PredicatePending::CallableArguments => 8,
                        });
                    }
                }
            }
        }
    }
}
