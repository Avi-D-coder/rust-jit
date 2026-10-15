//! Owned generated declaration inputs; replay the pinned syntax events, never display text.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        seals::SealKey,
    },
    source::snapshot::CfgSet,
    syntax::{
        outline::{DeclarationContext, ScopeKey},
        parse::{ParseKey, ParsedFile},
    },
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug)]
pub struct GeneratedSyntaxInput {
    pub(crate) items: Arc<super::generated_items::GeneratedItems>,
    pub(crate) key: ParseKey,
    pub(crate) scope: ScopeKey,
    pub(crate) cfg: CfgSet,
    pub(crate) seal: SealKey,
    pub(crate) producer_origin: Option<Arc<crate::syntax::origin::MacroOrigin>>,
    pub(crate) included_source: Option<Arc<crate::source::include_view::IncludedSourceView>>,
    pub(crate) definitions: BTreeMap<Digest, crate::source::manifest::DefKey>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
}
impl GeneratedSyntaxInput {
    pub(crate) fn prepare(
        record: Arc<super::record::ExpansionRecord>,
        scope: ScopeKey,
        cfg: CfgSet,
        seal: SealKey,
        producer_origin: Option<Arc<crate::syntax::origin::MacroOrigin>>,
        included_source: Option<Arc<crate::source::include_view::IncludedSourceView>>,
    ) -> Result<(Arc<Self>, BTreeMap<InputKey, InputValue>), Diagnostic> {
        if producer_origin.as_ref().is_some_and(|origin| {
            origin.invocation_identity != record.key().invocation
                || origin.heap_bytes().is_none_or(|n| n > 64 * 1024)
        }) {
            return Err(fault("generated producer diagnostic origin differs"));
        }
        if let Some(view) = &included_source {
            view.require_record(&record)?;
            if view.location().unit != scope.crate_key
                || producer_origin.as_ref().map(|origin| origin.fingerprint())
                    != Some(view.diagnostic_origin().fingerprint())
                || view.estimated_bytes() > 32 * 1024 * 1024
            {
                return Err(fault("generated include source owner differs"));
            }
        }
        let items = Arc::new(super::generated_items::GeneratedItems::admit(
            record.key(),
            record.clone(),
            seal.epoch,
        )?);
        let parsed = ParsedFile::from_generated_items(&items)?;
        let key = parsed.key();
        let outline = parsed.outline(scope.crate_key, scope.module_path.clone(), &cfg)?;
        let headers = items
            .items()
            .iter()
            .map(|h| ((h.start, h.end), h))
            .collect::<BTreeMap<_, _>>();
        let mut owners = BTreeMap::<Digest, Vec<&crate::syntax::outline::ItemOutline>>::new();
        let mut owner_count = 0usize;
        for item in outline.items.iter().filter(|i| {
            matches!(
                i.def_key.item_kind,
                crate::source::manifest::ItemKind::Impl | crate::source::manifest::ItemKind::Trait
            )
        }) {
            owner_count = owner_count
                .checked_add(1)
                .filter(|n| *n <= 100_000)
                .ok_or_else(|| fault("generated declaration owner count limit"))?;
            if owners.len() >= 100_000 {
                return Err(fault("generated declaration owner count limit"));
            }
            let identity = if item.def_key.item_kind == crate::source::manifest::ItemKind::Impl {
                item.header.fingerprint()
            } else {
                item.def_key.fingerprint()
            };
            let rows = owners.entry(identity).or_default();
            rows.try_reserve_exact(1)
                .map_err(|_| fault("generated declaration owner allocation"))?;
            rows.push(item);
        }
        let mut declarations = BTreeMap::new();
        let mut definitions = BTreeMap::new();
        for item in &outline.items {
            if item.scope.crate_key != scope.crate_key
                || !item.scope.owner.is_none_or(|owner| {
                    owners.get(&owner).is_some_and(|parents| {
                        parents.iter().any(|parent| {
                            parent.scope.crate_key == item.scope.crate_key
                                && parent.scope.module_path == item.scope.module_path
                                && parent.declaration_span.start <= item.declaration_span.start
                                && item.declaration_span.end <= parent.declaration_span.end
                        })
                    })
                })
                || !item.scope.module_path.starts_with(&scope.module_path)
            {
                return Err(fault("generated query declaration escapes captured owner"));
            }
            let header = headers
                .get(&(item.declaration_span.start, item.declaration_span.end))
                .ok_or_else(|| fault("generated query structural header absent"))?;
            let id = header.key.fingerprint();
            definitions.insert(id, item.def_key.clone());
            declarations.insert(
                InputKey::DefinitionHeader(id),
                InputValue::DefinitionHeader(item.header.clone()),
            );
            declarations.insert(
                InputKey::DefinitionSource(id),
                InputValue::DefinitionSource(key.clone()),
            );
            declarations.insert(
                InputKey::DeclarationContext(id),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                }),
            );
            if let Some(mut body) = item.body.clone() {
                body.def_key = id;
                declarations.insert(
                    InputKey::DefinitionBody(id),
                    InputValue::DefinitionBody(body),
                );
            }
        }
        Ok((
            Arc::new(Self {
                items,
                key,
                scope,
                cfg,
                seal,
                definitions,
                producer_origin,
                included_source,
            }),
            declarations,
        ))
    }
    pub(crate) fn same_unit(&self, other: &Self) -> bool {
        self.key == other.key
            && self.scope == other.scope
            && self.cfg == other.cfg
            && self.seal.epoch == other.seal.epoch
            && self.seal.unit_graph == other.seal.unit_graph
            && self.items.record().same_record(other.items.record())
            && self.included_source.as_ref().map(|view| view.identity())
                == other.included_source.as_ref().map(|view| view.identity())
            && self
                .producer_origin
                .as_ref()
                .map(|origin| origin.fingerprint())
                == other
                    .producer_origin
                    .as_ref()
                    .map(|origin| origin.fingerprint())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut definitions = 0usize;
        for def in self.definitions.values() {
            definitions = definitions
                .checked_add(256)
                .and_then(|n| n.checked_add(def.declared_name.capacity()))
                .and_then(|n| {
                    def.module_path
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(|| fault("generated definition metadata overflow"))?;
            for part in &def.module_path {
                definitions = definitions
                    .checked_add(part.capacity())
                    .ok_or_else(|| fault("generated definition metadata overflow"))?;
            }
        }
        std::mem::size_of::<Self>()
            .checked_add(self.items.metadata_bytes()?)
            .and_then(|n| {
                n.checked_add(
                    self.producer_origin
                        .as_ref()
                        .map_or(Some(0), |origin| origin.heap_bytes())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.included_source
                        .as_ref()
                        .map_or(0, |view| view.estimated_bytes()),
                )
            })
            .and_then(|n| n.checked_add(definitions))
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .and_then(|n| n.checked_add(self.key.profile.semantics.capacity()))
            .and_then(|n| {
                self.cfg
                    .metadata_bytes()
                    .ok()
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "generated input size overflow".into(),
            ))
    }
}
impl StableEncode for GeneratedSyntaxInput {
    const DOMAIN: &'static str = "lrr/v3/generated-syntax-value";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.scope);
        e.record(&self.cfg);
        e.record(&self.seal);
        e.record(&self.items.record().descriptor());
        e.option(&self.producer_origin, |e, origin| e.record(origin.as_ref()));
        e.option(&self.included_source, |e, view| e.digest(view.identity()));
    }
}
