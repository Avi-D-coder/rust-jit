//! Retained source identity policy for compiler clause publication.
//! Pinned hir_ty_lowering/mod.rs:983–1002 lowers positive PointeeSized bounds
//! into a transient vector, then discards its output after ordinary lowering.
//! This sidecar performs no lowering, validation, inference or source query at
//! encoding time. The common encoder must supply its completed real output.
use super::{
    model::{Clause, Span},
    prepared::{self, PreparedProvider},
    source_definition_headers::DefinitionHeaderKey,
    source_lang_identity::LangIdentityPending,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    syntax::spans::SourceRange,
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::ParamKey,
    },
};
use rustc_type_ir::{self as ty, inherent::IntoKind};
use std::{collections::BTreeMap, collections::BTreeSet, sync::Arc};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PredicateEmissionPending {
    Language {
        definition: Digest,
        pending: LangIdentityPending,
    },
    Written {
        source: SourceRange,
        pending: PredicatePending,
    },
    UnresolvedSized(ParamKey),
    Unprepared(Digest),
    EncodedIdentity,
    EncodedPolarity,
    EncodedClause,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ClauseEmission {
    Emit,
    TransientPositivePointeeSized,
    Pending(PredicateEmissionPending),
}
#[derive(Debug)]
struct TraitEmission {
    language: Arc<QueryValue>,
    decision: ClauseEmission,
}

/// Retained by the enclosing ordinary source memo after SourceBindings has
/// prepared the actual source Types/generics. Never install this query inside
/// SourceBindings: Q76 itself reaches Q75 and ordinary source bindings.
#[derive(Debug)]
pub(crate) struct PreparedClauseEmission {
    catalog: Arc<InputValue>,
    traits: BTreeMap<Digest, TraitEmission>,
    written_pending: Vec<PredicateEmissionPending>,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl PreparedClauseEmission {
    pub(super) fn prepare<'a>(
        db: &mut Database,
        catalog: Arc<InputValue>,
        recipes: impl IntoIterator<Item = &'a SourcePredicateRecipe>,
    ) -> QueryPoll<Self> {
        let InputValue::DiscoverySeal(source) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("clause emission catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(source) {
            return QueryPoll::Ready(Err(error));
        }
        let inventory = source.seal.key();
        if !db
            .input(&InputKey::DiscoverySeal(inventory.clone()))
            .is_some_and(|current| Arc::ptr_eq(&current, &catalog))
        {
            return QueryPoll::Ready(Err(fault("clause emission catalog is not current")));
        }
        let mut definitions = BTreeSet::new();
        let mut written_pending = Vec::new();
        for recipe in recipes {
            match recipe {
                SourcePredicateRecipe::ResolvedSizedDefault(reference)
                | SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Implemented(reference),
                    ..
                } => {
                    definitions.insert(reference.trait_key);
                }
                SourcePredicateRecipe::ImplicitSizedDefault(parameter) => {
                    written_pending.push(PredicateEmissionPending::UnresolvedSized(*parameter));
                }
                SourcePredicateRecipe::Explicit {
                    source,
                    result: PredicateResult::Pending(pending),
                } => written_pending.push(PredicateEmissionPending::Written {
                    source: *source,
                    pending: *pending,
                }),
                // These contain no positive trait clause to classify. Their
                // original source recipes remain in the enclosing plan.
                SourcePredicateRecipe::Explicit {
                    result:
                        PredicateResult::Relaxed { .. }
                        | PredicateResult::RegionOutlives { .. }
                        | PredicateResult::TypeOutlives { .. },
                    ..
                } => {}
            }
        }
        let mut result = Self {
            catalog,
            traits: BTreeMap::new(),
            written_pending,
        };
        for definition in definitions {
            let key = DefinitionHeaderKey {
                inventory: inventory.clone(),
                definition,
            };
            let captured = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceLangIdentity(Box::new(key.clone()))
            ));
            let QueryValue::SourceLangIdentity(language) = captured.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("clause emission language query differs")));
            };
            if !captured.cacheable || language.key() != &key {
                return QueryPoll::Ready(Err(fault("clause emission language identity differs")));
            }
            let decision = match language.classification() {
                Ok(Some(("pointee_sized", _))) => ClauseEmission::TransientPositivePointeeSized,
                // This is an actual Q76 classification, not absence from a map
                // or a comparison with the declaration's displayed spelling.
                Ok(Some(_)) | Ok(None) => ClauseEmission::Emit,
                Err(pending) => ClauseEmission::Pending(PredicateEmissionPending::Language {
                    definition,
                    pending,
                }),
            };
            result.traits.insert(
                definition,
                TraitEmission {
                    language: captured.value,
                    decision,
                },
            );
        }
        QueryPoll::Ready(Ok(result))
    }

    pub(crate) fn pending(&self) -> Option<PredicateEmissionPending> {
        self.written_pending.first().copied().or_else(|| {
            self.traits.values().find_map(|row| match row.decision {
                ClauseEmission::Pending(pending) => Some(pending),
                ClauseEmission::Emit | ClauseEmission::TransientPositivePointeeSized => None,
            })
        })
    }
    /// A per-ID decision, not a completeness certificate for neighboring rows.
    /// Unknown IDs never inherit Emit. Whole-query consumers check pending().
    pub(crate) fn decision(&self, definition: Digest) -> ClauseEmission {
        self.traits.get(&definition).map_or(
            ClauseEmission::Pending(PredicateEmissionPending::Unprepared(definition)),
            |row| row.decision,
        )
    }
    pub(crate) fn require(
        &self,
        definition: Digest,
    ) -> Result<ClauseEmission, PredicateEmissionPending> {
        match self.decision(definition) {
            ClauseEmission::Pending(pending) => Err(pending),
            decision => Ok(decision),
        }
    }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> {
        &self.catalog
    }

    /// Call only after the common encoder completes the actual predicate and
    /// every argument. Unsupported arguments must propagate before this call.
    /// Negative or non-trait clauses cannot enter the positive transient rule.
    pub(super) fn apply_encoded<'cx>(
        &self,
        definition: Digest,
        encoded: (Clause<'cx, PreparedProvider<'cx>>, Span),
    ) -> Result<Option<(Clause<'cx, PreparedProvider<'cx>>, Span)>, PredicateEmissionPending> {
        let ty::ClauseKind::Trait(clause) = encoded.0.kind().skip_binder() else {
            return Err(PredicateEmissionPending::EncodedClause);
        };
        if clause.polarity != ty::ClausePolarity::Positive {
            return Err(PredicateEmissionPending::EncodedPolarity);
        }
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else {
            unreachable!("retained emission catalog")
        };
        if clause.trait_ref.def_id.source() != definition
            || clause.trait_ref.def_id.world() != catalog.seal.fingerprint()
        {
            return Err(PredicateEmissionPending::EncodedIdentity);
        }
        match self.require(definition)? {
            ClauseEmission::Emit => Ok(Some(encoded)),
            ClauseEmission::TransientPositivePointeeSized => Ok(None),
            ClauseEmission::Pending(_) => unreachable!("checked emission decision"),
        }
    }
}
impl StableEncode for PreparedClauseEmission {
    const DOMAIN: &'static str = "lrr/v1/source-clause-emission";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.catalog.fingerprint());
        e.map(&self.traits, |e, definition, row| {
            e.digest(*definition);
            e.digest(row.language.fingerprint());
            match row.decision {
                ClauseEmission::Emit => e.u8(0),
                ClauseEmission::TransientPositivePointeeSized => e.u8(1),
                ClauseEmission::Pending(why) => {
                    e.u8(2);
                    encode_pending(e, why);
                }
            }
        });
        e.sequence(&self.written_pending, |e, why| encode_pending(e, *why));
    }
}
pub(super) fn encode_pending(e: &mut StableEncoder, pending: PredicateEmissionPending) {
    match pending {
        PredicateEmissionPending::Language {
            definition,
            pending,
        } => {
            e.u8(0);
            e.digest(definition);
            e.u8(pending as u8);
        }
        PredicateEmissionPending::Written { source, pending } => {
            e.u8(1);
            e.u32(source.start);
            e.u32(source.end);
            e.u8(match pending {
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
        PredicateEmissionPending::UnresolvedSized(parameter) => {
            e.u8(2);
            e.record(&parameter);
        }
        PredicateEmissionPending::Unprepared(definition) => {
            e.u8(3);
            e.digest(definition);
        }
        PredicateEmissionPending::EncodedIdentity => e.u8(4),
        PredicateEmissionPending::EncodedPolarity => e.u8(5),
        PredicateEmissionPending::EncodedClause => e.u8(6),
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
