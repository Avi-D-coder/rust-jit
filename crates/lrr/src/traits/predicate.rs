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
    RelaxedTrait,
    QualifiedBound,
    LifetimeName,
    CallableArguments,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PredicateResult {
    Implemented(TraitRef),
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
}
impl StableEncode for SourcePredicateRecipe {
    const DOMAIN: &'static str = "lrr/v1/source-predicate-requirement";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::ImplicitSizedDefault(key) => {
                e.u32(0);
                e.record(key);
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
                    PredicateResult::Pending(why) => {
                        e.u32(3);
                        e.u32(match why {
                            PredicatePending::Subject => 0,
                            PredicatePending::TraitName => 1,
                            PredicatePending::SymbolicType => 2,
                            PredicatePending::HigherRanked => 3,
                            PredicatePending::AssociatedIdentity => 4,
                            PredicatePending::RelaxedTrait => 5,
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
