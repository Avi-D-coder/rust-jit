//! One real declaration's lang role, not a complete world registry/absence proof.
use super::source_definition_headers::{
    DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::manifest::{DefKey, ItemKind, Namespace},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
pub type SourceLangIdentityKey = DefinitionHeaderKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum LangIdentityPending {
    Header,
    AttributeSyntax,
    UnknownCompilerName,
    ConflictingRoles,
    FacetSource,
    FacetIdentity,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum LangIdentityOrigin {
    SourceAttribute,
    EmbeddedFacet,
}
#[derive(Debug)]
pub struct PreparedSourceLangIdentity {
    key: SourceLangIdentityKey,
    header: Arc<QueryValue>,
    catalog: Arc<InputValue>,
    classification: Result<Option<(String, LangIdentityOrigin)>, LangIdentityPending>,
}
macro_rules! ready {
    ($e:expr) => {
        match $e {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
impl PreparedSourceLangIdentity {
    pub fn key(&self) -> &SourceLangIdentityKey {
        &self.key
    }
    pub fn classification(
        &self,
    ) -> Result<Option<(&str, LangIdentityOrigin)>, LangIdentityPending> {
        self.classification
            .as_ref()
            .map(|v| v.as_ref().map(|(n, o)| (n.as_str(), *o)))
            .map_err(|p| *p)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceLangIdentityKey) -> QueryPoll<Self> {
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))
        ))
        .value;
        let Some(catalog) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("lang identity catalog absent")));
        };
        let InputValue::DiscoverySeal(input) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("lang identity catalog differs")));
        };
        if let Err(e) = super::prepared::catalog_world(input) {
            return QueryPoll::Ready(Err(e));
        }
        let QueryValue::DefinitionHeaderPlan(plan) = header.as_ref() else {
            return QueryPoll::Ready(Err(fault("lang identity header differs")));
        };
        let classification = if plan.pending().is_some() {
            Err(LangIdentityPending::Header)
        } else {
            let mut source = None;
            let mut pending = None;
            for attr in plan.attributes().iter().filter(|a| a.name == "lang") {
                let name = match attr.tokens.as_slice() {
                    [name, eq, value] if name.text == "lang" && eq.text == "=" => {
                        crate::syntax::ast_adapter::string_value(&value.text, plan.edition())
                    }
                    _ => None,
                };
                match name {
                    Some(name) if super::lang_names::known_name(&name) => {
                        if source.as_ref().is_some_and(|old| old != &name) {
                            pending = Some(LangIdentityPending::ConflictingRoles)
                        }
                        source = Some(name);
                    }
                    Some(_) => pending = Some(LangIdentityPending::UnknownCompilerName),
                    None => pending = Some(LangIdentityPending::AttributeSyntax),
                }
            }
            if let Some(p) = pending {
                Err(p)
            } else if let Some(name) = source {
                Ok(Some((name, LangIdentityOrigin::SourceAttribute)))
            } else {
                match facet_role(db, plan, &key.inventory) {
                    QueryPoll::Ready(Ok(v)) => {
                        v.map(|n| n.map(|n| (n.to_owned(), LangIdentityOrigin::EmbeddedFacet)))
                    }
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                }
            }
        };
        QueryPoll::Ready(Ok(Self {
            key: key.clone(),
            header,
            catalog,
            classification,
        }))
    }
}
// Explicit Section17 facet contract: only the authenticated defining unit/module
// and actual declaration key below can supply a compiler role. A caller's path,
// alias or same-spelled declaration is never consulted.
fn contract_variant(
    krate: &str,
    module: &[String],
    name: &str,
    role: DefinitionHeaderRole,
) -> Option<&'static str> {
    let path = module.iter().map(String::as_str).collect::<Vec<_>>();
    let candidates: &[&'static str] = match (krate, path.as_slice(), role) {
        ("core", ["marker"], DefinitionHeaderRole::Trait) => &[
            "Sized",
            "Unsize",
            "Sync",
            "Unpin",
            "Copy",
            "Tuple",
            "Freeze",
            "ConstParamTy",
            "FnPtrTrait",
            "Destruct",
            "DiscriminantKind",
        ],
        ("core", ["clone"], DefinitionHeaderRole::Trait) => &["Clone"],
        ("core", ["ops"], DefinitionHeaderRole::Trait) => &[
            "Fn",
            "FnMut",
            "FnOnce",
            "Deref",
            "DerefMut",
            "Drop",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "Neg",
            "Not",
            "BitAnd",
            "BitOr",
            "BitXor",
            "Shl",
            "Shr",
            "AddAssign",
            "SubAssign",
            "MulAssign",
            "DivAssign",
            "RemAssign",
            "BitAndAssign",
            "BitOrAssign",
            "BitXorAssign",
            "ShlAssign",
            "ShrAssign",
            "Index",
            "IndexMut",
            "Try",
            "Receiver",
        ],
        ("core", ["cmp"], DefinitionHeaderRole::Trait) => &["PartialEq", "PartialOrd"],
        ("core", ["future"], DefinitionHeaderRole::Trait) => &["Future"],
        ("core", ["iter"], DefinitionHeaderRole::Trait) => &["Iterator"],
        ("core", ["marker"], DefinitionHeaderRole::Struct) => &["PhantomData"],
        ("core", ["cell"], DefinitionHeaderRole::Struct) => &["UnsafeCell"],
        ("core", ["mem"], DefinitionHeaderRole::Struct) => &["ManuallyDrop"],
        ("core", ["pin"], DefinitionHeaderRole::Struct) => &["Pin"],
        ("alloc", ["boxed"], DefinitionHeaderRole::Struct) => &["OwnedBox"],
        ("core", ["option"], DefinitionHeaderRole::Enum) => &["Option"],
        ("core", ["task"], DefinitionHeaderRole::Enum) => &["Poll"],
        _ => &[],
    };
    candidates.iter().copied().find(|variant| {
        (match *variant {
            "OwnedBox" => "Box",
            "FnPtrTrait" => "FnPtr",
            other => other,
        }) == name
    })
}
type FacetFiles = BTreeMap<(String, Vec<String>), (&'static str, Digest)>;
fn facet_files() -> &'static FacetFiles {
    static FILES: OnceLock<FacetFiles> = OnceLock::new();
    FILES.get_or_init(|| {
        crate::source::embedded_crates::standard_facade_crates(&ProfileIdentity::current())
            .into_iter()
            .flat_map(|c| {
                c.files.into_iter().map(move |f| {
                    (
                        (c.name.to_owned(), f.module_path),
                        (f.path, Digest::of_bytes(f.text.as_bytes())),
                    )
                })
            })
            .collect()
    })
}
fn facet_role(
    db: &mut Database,
    plan: &PreparedDefinitionHeader,
    inventory: &crate::db::seals::SealKey,
) -> QueryPoll<Result<Option<&'static str>, LangIdentityPending>> {
    let (parse, _, context, _) = plan
        .declaration()
        .source_parts()
        .expect("retained actual declaration");
    let Some(krate) = ["core", "alloc", "std", "test"]
        .into_iter()
        .find(|c| crate::facade::unit(&parse.profile, c) == context.scope.crate_key)
    else {
        return QueryPoll::Ready(Ok(Ok(None)));
    };
    let Some(name) = plan.name() else {
        return QueryPoll::Ready(Ok(Ok(None)));
    };
    let Some(variant) = contract_variant(krate, &context.scope.module_path, name, plan.role())
    else {
        return QueryPoll::Ready(Ok(Ok(None)));
    };
    let Some(compiler_name) = super::lang_names::compiler_name(variant) else {
        return QueryPoll::Ready(Ok(Err(LangIdentityPending::UnknownCompilerName)));
    };
    let pending = |p| QueryPoll::Ready(Ok(Err(p)));
    let Some((path, content)) =
        facet_files().get(&(krate.to_owned(), context.scope.module_path.clone()))
    else {
        return pending(LangIdentityPending::FacetSource);
    };
    if parse.content != *content || context.scope.owner.is_some() {
        return pending(LangIdentityPending::FacetSource);
    }
    let kind = match plan.role() {
        DefinitionHeaderRole::Trait => ItemKind::Trait,
        DefinitionHeaderRole::Struct => ItemKind::Struct,
        DefinitionHeaderRole::Enum => ItemKind::Enum,
        _ => return pending(LangIdentityPending::FacetIdentity),
    };
    let expected = DefKey {
        crate_key: context.scope.crate_key,
        module_path: context.scope.module_path.clone(),
        namespace: Namespace::Type,
        item_kind: kind,
        declared_name: name.to_owned(),
        enclosing_impl: None,
        duplicate_ordinal: 0,
    };
    if expected.fingerprint() != plan.key().definition {
        return pending(LangIdentityPending::FacetIdentity);
    }
    let headers = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedScopeImplHeaders(inventory.clone())
    ))
    .value;
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else {
        return QueryPoll::Ready(Err(fault("lang identity environment differs")));
    };
    let Some(env) = headers.inventory_root().environment() else {
        return pending(LangIdentityPending::FacetSource);
    };
    let Some(view) = env.sources().iter().find(|v| {
        v.selection().outline.parse == *parse
            && v.selection().outline.module_path == context.scope.module_path
            && v.selection().outline.crate_key == context.scope.crate_key
            && v.selection().path.as_str() == *path
    }) else {
        return pending(LangIdentityPending::FacetSource);
    };
    let selected = view.selection();
    if selected.path.as_str() != *path
        || !matches!(db.input(&InputKey::CapturedSource(selected.location_identity())).as_deref(),Some(InputValue::CapturedSource(actual)) if actual==selected)
    {
        return pending(LangIdentityPending::FacetSource);
    }
    QueryPoll::Ready(Ok(Ok(Some(compiler_name))))
}
#[derive(Debug)]
pub struct CurrentSourceLangIdentity {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceLangIdentity {
    pub(crate) fn capture(db: &mut Database, key: &SourceLangIdentityKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceLangIdentity(Box::new(key.clone()));
        let v = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !v.cacheable
            || !matches!(v.value.as_ref(),QueryValue::SourceLangIdentity(p) if p.key()==key)
        {
            return QueryPoll::Ready(Err(fault("lang identity memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, v.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: v.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceLangIdentity {
        let QueryValue::SourceLangIdentity(p) = self.value.as_ref() else {
            unreachable!()
        };
        p
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
impl StableEncode for PreparedSourceLangIdentity {
    const DOMAIN: &'static str = "lrr/v1/source-lang-identity";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        let QueryValue::DefinitionHeaderPlan(h) = self.header.as_ref() else {
            unreachable!()
        };
        e.record(h.as_ref());
        e.digest(self.catalog.fingerprint());
        match &self.classification {
            Ok(None) => e.u8(0),
            Ok(Some((name, origin))) => {
                e.u8(1);
                e.string(name);
                e.u8(*origin as u8)
            }
            Err(p) => {
                e.u8(2);
                e.u8(*p as u8)
            }
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
