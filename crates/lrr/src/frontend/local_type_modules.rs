//! Cold lexical module proof for an actual entered Type use. Pinned item
//! normalization chooses lexical versus definition-site lookup; token locations
//! and ownerless namespace membership never make that choice.
use super::error;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::InputValue,
        key::{CurrentModuleNameKey, OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{hygiene::HygieneFacts, source_body_context::PreparedBodyContext},
    source::{changes::SourceSelection, manifest::Namespace},
    syntax::{
        outline::{Outline, ScopeKey},
        token_wire::{Key, NameContextKey},
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct LexicalTypeModule {
    key: OutlineKey,
    outline: Arc<Outline>,
    scope: ScopeKey,
    // This is the actual owning source lease retained by PreparedBodyContext,
    // not a path recovered from a token's diagnostic location.
    source: Option<SourceSelection>,
    fingerprint: Digest,
    extra_bytes: usize,
}

/// A source use has exhausted its actual local/import prefix and selected a
/// lexical module. Current namespace evidence still comes from the ordinary
/// CurrentModuleName query and the owning environment publication.
pub(crate) struct LocalTypeModuleSelector {
    normalized: Key,
    comparison: NameContextKey,
}
pub(super) enum ModuleRoute {
    Lexical(LocalTypeModuleSelector),
    Definition(Key),
}
impl LocalTypeModuleSelector {
    pub(crate) fn normalized_context(&self) -> Key {
        self.normalized
    }
    pub(crate) fn comparison_context(&self) -> NameContextKey {
        self.comparison
    }
    pub(super) fn encode(&self, e: &mut StableEncoder) {
        e.digest(Digest(self.normalized.0));
        e.digest(Digest(self.comparison.0.0));
    }
}
impl LexicalTypeModule {
    pub(super) fn prepare(db: &mut Database, context: &PreparedBodyContext) -> QueryPoll<Self> {
        let source = context.source();
        if let Some(parent) = source.local_parent() {
            match parent.require_current(db) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            // capture_local proved the exact proper nested declaration. Its
            // module is the actual parent's module proof, never a local rib
            // stripped into a guessed module or another outline search.
            if parent.context().source().parsed().key() != source.parsed().key()
                || parent.context().source().scope().crate_key != source.scope().crate_key
                || parent.context().source().scope().module_path != source.scope().module_path
            {
                return QueryPoll::Ready(Err(error("local body lexical module parent differs")));
            }
            return QueryPoll::Ready(Ok(parent.lexical_module.clone()));
        }
        let mut key = None;
        for input in source.retained_inputs() {
            let candidate = match input.as_ref() {
                InputValue::RetainedSyntax(unit) => Some(OutlineKey {
                    parse: unit.key().clone(),
                    ..unit.prepared().source().outline.clone()
                }),
                InputValue::GeneratedSyntax(unit) => Some(OutlineKey {
                    parse: unit.key.clone(),
                    crate_key: unit.scope.crate_key,
                    module_path: unit.scope.module_path.clone(),
                    cfg: unit.cfg.fingerprint(),
                }),
                _ => None,
            };
            if let Some(candidate) = candidate {
                if key.replace(candidate).is_some() {
                    return QueryPoll::Ready(Err(error(
                        "entered lexical outline source is ambiguous",
                    )));
                }
            }
        }
        let key = match key.or_else(|| {
            source
                .original_source()
                .map(|view| view.selection().outline.clone())
        }) {
            Some(key)
                if key.parse == source.parsed().key()
                    && key.crate_key == source.scope().crate_key =>
            {
                key
            }
            _ => return QueryPoll::Ready(Err(error("entered lexical outline source differs"))),
        };
        let result = match db.fetch(&SyntaxProvider, QueryKey::Outline(key.clone())) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::Outline(outline) = result.value.as_ref() else {
            return QueryPoll::Ready(Err(error("entered lexical outline outcome differs")));
        };
        QueryPoll::Ready(Self::from_outline(context, key, Arc::clone(outline)))
    }
    fn from_outline(
        context: &PreparedBodyContext,
        key: OutlineKey,
        outline: Arc<Outline>,
    ) -> Result<Self, Diagnostic> {
        let admitted = context.source();
        if outline.content != key.parse.content
            || outline.profile != key.parse.profile
            || outline.cfg != key.cfg
        {
            return Err(error("entered lexical outline identity differs"));
        }
        // One cold join to the actual admitted declaration. Canonical generated
        // identity was checked against GeneratedSyntaxInput at source capture;
        // an original-shaped outline DefKey must not replace it here.
        let mut rows = outline
            .items
            .iter()
            .filter(|item| item.declaration_span == admitted.context().range);
        let row = rows
            .next()
            .ok_or_else(|| error("entered lexical declaration is absent"))?;
        if rows.next().is_some()
            || row.scope != admitted.context().scope
            || row.header.fingerprint() != admitted.header().fingerprint()
            || !row.body.as_ref().is_some_and(|body| {
                body.range == admitted.body().range
                    && body.body_hash == admitted.body().body_hash
                    && body.entry_blocked == admitted.body().entry_blocked
            })
        {
            return Err(error("entered lexical declaration/header/body differs"));
        }
        let goal = ScopeKey {
            crate_key: row.scope.crate_key,
            module_path: row.scope.module_path.clone(),
            owner: None,
        };
        let scope = outline
            .scopes
            .get_key_value(&goal)
            .map(|(scope, _)| scope.clone())
            .ok_or_else(|| error("entered lexical module is absent from actual outline"))?;
        let selected = context
            .module_source()
            .filter(|view| {
                view.selection().outline.crate_key == scope.crate_key
                    && view.outline().scopes.contains_key(&scope)
            })
            .map(|view| view.selection().clone());
        let key_bytes = key
            .parse
            .profile
            .semantics
            .capacity()
            .checked_add(
                key.module_path
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .and_then(|n| {
                key.module_path
                    .iter()
                    .try_fold(n, |n, part| n.checked_add(part.capacity()))
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        let scope_bytes = scope
            .module_path
            .capacity()
            .checked_mul(std::mem::size_of::<String>())
            .and_then(|n| {
                scope
                    .module_path
                    .iter()
                    .try_fold(n, |n, part| n.checked_add(part.capacity()))
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        let extra_bytes = key_bytes
            .checked_add(scope_bytes)
            .and_then(|n| {
                n.checked_add(selected.as_ref().map_or(Some(0), |source| {
                    source
                        .metadata_bytes()
                        .ok()?
                        .checked_sub(std::mem::size_of::<SourceSelection>())
                })?)
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut e = StableEncoder::new("lrr/v1/entered-type-lexical-module");
        e.digest(admitted.identity());
        e.record(&key);
        e.record(&scope);
        e.option(&selected, |e, source| {
            e.string(source.path.as_str());
            e.record(&source.outline);
        });
        Ok(Self {
            key,
            outline,
            scope,
            source: selected,
            fingerprint: e.finish(),
            extra_bytes,
        })
    }
    pub(super) fn route(&self, facts: &HygieneFacts, raw: Key) -> Result<ModuleRoute, Diagnostic> {
        let comparison = facts.name_item(raw)?;
        let normalized = facts.item(raw)?;
        let record = facts.context(normalized)?;
        if record.parent.is_none() {
            // Root class deliberately carries no module owner. The actual
            // admitted declaration/outline above supplies that separate proof.
            Ok(ModuleRoute::Lexical(LocalTypeModuleSelector {
                normalized,
                comparison,
            }))
        } else {
            // A surviving opaque mark requires its real selected-definition
            // declaration module, including inherited opaque outer marks.
            Ok(ModuleRoute::Definition(record.expansion.ok_or_else(
                || error("normalized item context lacks selected expansion"),
            )?))
        }
    }
    pub(super) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(super) fn key(&self) -> &OutlineKey {
        &self.key
    }
    pub(super) fn outline_root(&self) -> &Arc<Outline> {
        &self.outline
    }
    pub(super) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(super) fn extra_bytes(&self) -> usize {
        self.extra_bytes
    }
    pub(super) fn has_module_source(&self) -> bool {
        self.source.is_some()
    }
    pub(super) fn request(
        &self,
        name: &str,
        edition: u16,
    ) -> Result<Option<CurrentModuleNameKey>, Diagnostic> {
        // Normalize the actual use spelling in its own token edition before
        // handing its semantic name to the ordinary module query.
        let name = crate::frontend::names::owned_name(name, edition)?;
        Ok(self.source.as_ref().map(|source| CurrentModuleNameKey {
            source: source.outline.clone(),
            path: source.path.as_str().to_owned(),
            bucket: crate::db::key::BucketKey {
                scope: self.scope.clone(),
                namespace: Namespace::Type,
                name: name.as_str().to_owned(),
            },
        }))
    }
}
