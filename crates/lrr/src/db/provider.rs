use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    value::{Bucket, QueryValue},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    syntax::parse::ParsedFile,
};
use std::sync::Arc;

pub struct Provided {
    pub value: QueryValue,
    pub cacheable: bool,
}
impl Provided {
    pub fn pure(value: QueryValue) -> QueryPoll<Self> {
        QueryPoll::Ready(Ok(Self {
            value,
            cacheable: true,
        }))
    }
    pub fn error(code: &'static str, message: impl Into<String>) -> QueryPoll<Self> {
        QueryPoll::Ready(Err(Diagnostic::new(code, message.into())))
    }
}
/// Providers can only observe immutable database inputs and other pure queries.
pub trait Provider {
    fn execute(&self, db: &mut Database, key: &QueryKey) -> QueryPoll<Provided>;
}
pub struct SyntaxProvider;
impl SyntaxProvider {
    /// Pure lookup of adopted candidate evidence; never probes or captures a file.
    pub fn module_capture(
        db: &mut Database,
        input: &super::poll::InputRequest,
    ) -> QueryPoll<std::sync::Arc<crate::source::module_capture::AdoptedModuleCapture>> {
        let mut work=||->Result<QueryPoll<std::sync::Arc<crate::source::module_capture::AdoptedModuleCapture>>,Diagnostic> {
            input.validate()?;
            let super::poll::InputRequest::CaptureModule {request,..}=input else {return Err(Diagnostic::new("LRR-E-COMPILE-INPUT", "module lookup requires a module capture demand".into()));};
            let Some(value)=db.input(&InputKey::ModuleRecord(input.fingerprint())) else {return Ok(QueryPoll::NeedInput(input.clone()));};
            let InputValue::ModuleRecord {input:identity,captured}=value.as_ref() else {return Err(Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", "module input variant differs".into()));};
            if *identity!=input.fingerprint() {return Err(Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", "module input identity differs".into()));}
            captured.validate(request)?;
            Ok(QueryPoll::Ready(Ok(std::sync::Arc::new(crate::source::module_capture::AdoptedModuleCapture::new(value)?))))
        };
        match work() {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    /// Exact prepared unit environment; publication is preparation's responsibility.
    pub fn compile_environment(
        db: &mut Database,
        unit: Digest,
    ) -> QueryPoll<Arc<crate::expand::builtins::FrozenCompileEnvironment>> {
        let Some(input) = db.input(&InputKey::UnitCompileEnv(unit)) else {
            return QueryPoll::NeedInput(super::poll::InputRequest::CaptureCompileEnvironment {
                unit,
            });
        };
        match input.as_ref() {
            InputValue::UnitCompileEnv(environment) if environment.unit() == unit => {
                QueryPoll::Ready(Ok(environment.clone()))
            }
            _ => QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-COMPILE-INPUT",
                "prepared compile environment unit/variant mismatch".into(),
            ))),
        }
    }

    /// Pure item-grammar include entry. Discovery owns later lexical integration.
    pub fn included_items(
        db: &mut Database,
        request: &super::poll::InputRequest,
        context: &crate::source::included::IncludeItemsContext,
    ) -> QueryPoll<crate::source::included::IncludedItems> {
        let mut parse = || -> Result<Option<crate::source::included::IncludedItems>, Diagnostic> {
            request.validate()?;
            context.profile.validate_source()?;
            let super::poll::InputRequest::CaptureInclude {
                request: include, ..
            } = request
            else {
                return Err(Diagnostic::new(
                    "LRR-E-COMPILE-INPUT",
                    "item include requires an authority-bound capture request".into(),
                ));
            };
            if include.unit() != context.unit
                || include.kind() != crate::source::include_paths::IncludeKind::Syntax
            {
                return Err(Diagnostic::new(
                    "LRR-E-COMPILE-INPUT",
                    "item include unit/kind mismatch".into(),
                ));
            }
            let Some(input) = db.input(&InputKey::IncludeRecord(request.fingerprint())) else {
                return Ok(None);
            };
            let InputValue::IncludeRecord {
                input: identity,
                captured,
            } = input.as_ref()
            else {
                return Err(Diagnostic::new(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "include input variant mismatch".into(),
                ));
            };
            if *identity != request.fingerprint()
                || captured.request() != include.identity()
                || captured.kind() != include.kind()
            {
                return Err(Diagnostic::new(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "include input identity mismatch".into(),
                ));
            }
            let parsed = if context.profile.edition == crate::core::profile::LIVE_EDITION {
                ParsedFile::parse(captured.blob(), context.profile.clone())?
            } else {
                ParsedFile::parse_dependency(captured.blob(), context.profile.clone())?
            };
            if let Some(error) = parsed.errors.first() {
                return Err(Diagnostic::new(
                    "LRR-E-INCLUDE-PARSE",
                    format!(
                        "included item syntax at {}:{}..{}: {}; caller origin={}; include origin={}",
                        captured.location().path.as_str(),
                        error.range.start,
                        error.range.end,
                        error.message.chars().take(256).collect::<String>(),
                        Digest(include.from().origin.0).hex(),
                        Digest(include.include_origin().0).hex()
                    ),
                ));
            }
            let outline =
                parsed.outline(context.crate_key, context.module_path.clone(), &context.cfg)?;
            Ok(Some(crate::source::included::IncludedItems {
                parsed: Arc::new(parsed),
                outline: Arc::new(outline),
                input,
            }))
        };
        match parse() {
            Ok(Some(items)) => QueryPoll::Ready(Ok(items)),
            Ok(None) => QueryPoll::NeedInput(request.clone()),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
impl Provider for SyntaxProvider {
    fn execute(&self, db: &mut Database, key: &QueryKey) -> QueryPoll<Provided> {
        let current_expansion = matches!(key, QueryKey::CurrentExpansionInput(_));
        let value = match key {
            QueryKey::SealedName(key) => {
                if key.seal.domain
                    != (super::seals::SealDomain::Scope {
                        id: key.bucket.scope.fingerprint(),
                        namespace: key.bucket.namespace,
                    })
                    || key.bucket.name.len() > 64 * 1024
                    || key.bucket.scope.owner.is_some()
                {
                    return Provided::error(
                        "LRR-E-QUERY-REQUEST",
                        "sealed name query domain/owner differs",
                    );
                }
                match db.fetch(self, QueryKey::DiscoverySeal(key.seal.clone())) {
                    QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(),QueryValue::DiscoverySeal(seal) if seal.key()==key.seal) =>
                        {}
                    QueryPoll::Ready(Ok(_)) => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "sealed name certificate variant differs",
                        );
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                }
                let input = db.input(&InputKey::DiscoverySeal(key.seal.clone()));
                let Some(InputValue::DiscoverySeal(input)) = input.as_deref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "sealed name input absent after verification",
                    );
                };
                let Some(scope) = &input.scope else {
                    return QueryPoll::NeedInput(super::poll::InputRequest::Discover {
                        key: key.seal.clone(),
                    });
                };
                let lookup = match scope.lookup(&key.bucket) {
                    Ok(lookup) => lookup,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                QueryValue::SealedName {
                    key: key.clone(),
                    lookup,
                }
            }
            QueryKey::ScopeDeclarationInventory(key) => {
                if !matches!(&key.domain, super::seals::SealDomain::Scope { .. }) {
                    return Provided::error(
                        "LRR-E-QUERY-REQUEST",
                        "declaration inventory requires a scope certificate",
                    );
                }
                match db.fetch(self, QueryKey::DiscoverySeal(key.clone())) {
                    QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(seal) if seal.key() == *key) =>
                        {}
                    QueryPoll::Ready(Ok(_)) => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "declaration inventory certificate variant differs",
                        );
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                }
                let input = db.input(&InputKey::DiscoverySeal(key.clone()));
                let Some(InputValue::DiscoverySeal(input)) = input.as_deref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "declaration inventory input absent after verification",
                    );
                };
                QueryValue::ScopeDeclarationInventory {
                    key: key.clone(),
                    inventory: input.declaration_inventory.clone(),
                }
            }
            QueryKey::DiscoverySeal(key) => {
                if let Err(error) = key.validate() {
                    return QueryPoll::Ready(Err(error));
                }
                if let Some(epoch) = db.input(&InputKey::ExpansionEpoch)
                    && !matches!(epoch.as_ref(),InputValue::ExpansionEpoch(actual) if *actual==key.epoch)
                {
                    return Provided::error(
                        "LRR-E-QUERY-REQUEST",
                        "discovery query has stale epoch",
                    );
                }
                let Some(input) = db.input(&InputKey::DiscoverySeal(key.clone())) else {
                    return QueryPoll::NeedInput(super::poll::InputRequest::Discover {
                        key: key.clone(),
                    });
                };
                let InputValue::DiscoverySeal(input) = input.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "discovery input variant differs",
                    );
                };
                if input.seal.key() != *key {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "discovery certificate key differs",
                    );
                }
                if !matches!(db.input(&InputKey::SourceDependencies).as_deref(), Some(InputValue::SourceDependencies(graph)) if graph.fingerprint() == input.source_dependencies)
                {
                    return QueryPoll::NeedInput(super::poll::InputRequest::Discover {
                        key: key.clone(),
                    });
                }
                for (dependency, fingerprint) in &input.compiler_inputs {
                    if db
                        .input(dependency)
                        .is_none_or(|value| value.fingerprint() != *fingerprint)
                    {
                        return QueryPoll::NeedInput(super::poll::InputRequest::Discover {
                            key: key.clone(),
                        });
                    }
                }
                for source in &input.sources {
                    if !matches!(db.input(&InputKey::CapturedSource(source.location_identity())).as_deref(),Some(InputValue::CapturedSource(actual)) if actual==source)
                    {
                        return QueryPoll::NeedInput(super::poll::InputRequest::Discover {
                            key: key.clone(),
                        });
                    }
                    let outline_key = QueryKey::Outline(source.outline.clone());
                    let outline = match db.fetch(self, outline_key.clone()) {
                        QueryPoll::Ready(Ok(outcome)) => outcome,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                    };
                    let QueryValue::Outline(outline) = outline.value.as_ref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "discovery source outline variant differs",
                        );
                    };
                    if input
                        .seal
                        .input_dependencies()
                        .binary_search(&(outline_key.fingerprint(), outline.fingerprint()))
                        .is_err()
                    {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "discovery source fingerprint differs from certificate",
                        );
                    }
                }
                for descriptor in &input.records {
                    let record = db.input(&InputKey::ExpansionRecord(descriptor.key.fingerprint()));
                    match record.as_deref() {
                        Some(InputValue::ExpansionRecord(record))
                            if record.key() == &descriptor.key
                                && record.descriptor().fingerprint()
                                    == descriptor.fingerprint() => {}
                        None => {
                            return QueryPoll::NeedInput(super::poll::InputRequest::Expand(
                                Box::new(super::poll::ExpansionRequest {
                                    key: descriptor.key.clone(),
                                }),
                            ));
                        }
                        _ => {
                            return Provided::error(
                                "LRR-E-INTERNAL-SNAPSHOT",
                                "discovery record differs from certificate",
                            );
                        }
                    }
                    if descriptor
                        .key
                        .procedural_epoch
                        .is_some_and(|epoch| epoch != key.epoch)
                        || (descriptor.key.procedural_epoch.is_some()
                            && !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(actual)) if *actual==key.epoch))
                    {
                        return Provided::error(
                            "LRR-E-QUERY-REQUEST",
                            "discovery procedural record has stale epoch",
                        );
                    }
                }
                QueryValue::DiscoverySeal(Box::new(input.seal.clone()))
            }
            QueryKey::ExpansionInput(key) | QueryKey::CurrentExpansionInput(key) => {
                if let Err(error) = key.validate() {
                    return QueryPoll::Ready(Err(error));
                }
                if current_expansion && let Some(expected) = key.procedural_epoch {
                    match db.input(&InputKey::ExpansionEpoch).as_deref() {
                        Some(InputValue::ExpansionEpoch(actual)) if *actual == expected => {}
                        _ => {
                            return Provided::error(
                                "LRR-E-QUERY-REQUEST",
                                "procedural expansion has no matching current epoch input",
                            );
                        }
                    }
                }
                match db
                    .input(&InputKey::ExpansionRecord(key.fingerprint()))
                    .as_deref()
                {
                    Some(InputValue::ExpansionRecord(record)) if record.key() == key.as_ref() => {
                        QueryValue::ExpansionInput(Box::new(record.descriptor()))
                    }
                    None => {
                        return QueryPoll::NeedInput(super::poll::InputRequest::Expand(Box::new(
                            super::poll::ExpansionRequest {
                                key: key.as_ref().clone(),
                            },
                        )));
                    }
                    _ => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "expansion record identity mismatch",
                        );
                    }
                }
            }
            QueryKey::ParseFile(k) => {
                if let Some(input) = db.input(&InputKey::GeneratedSyntax(k.content)) {
                    let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated syntax input variant differs",
                        );
                    };
                    if unit.key != *k {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "generated parse key/profile differs",
                        );
                    }
                    match db.fetch(self, QueryKey::DiscoverySeal(unit.seal.clone())) {
                        QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(),QueryValue::DiscoverySeal(seal) if seal.key()==unit.seal) =>
                            {}
                        QueryPoll::Ready(Ok(_)) => {
                            return Provided::error(
                                "LRR-E-INTERNAL-SNAPSHOT",
                                "generated syntax certificate differs",
                            );
                        }
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                    }
                    return match ParsedFile::from_generated_items(&unit.items) {
                        Ok(parsed) => Provided::pure(QueryValue::ParseFile(Arc::new(parsed))),
                        Err(error) => QueryPoll::Ready(Err(error)),
                    };
                }
                match db.input(&InputKey::SourceBlob(k.content)).as_deref() {
                    Some(InputValue::SourceBlob(blob)) => {
                        if let Some(parsed) = db.prepared_parse(k) {
                            return Provided::pure(QueryValue::ParseFile(parsed));
                        }
                        match ParsedFile::parse(blob, k.profile.clone()) {
                            Ok(p) => QueryValue::ParseFile(Arc::new(p)),
                            Err(e) => QueryValue::Error(e),
                        }
                    }
                    _ => return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "parse source absent"),
                }
            }
            QueryKey::Outline(k) => {
                let parse = match db.fetch(self, QueryKey::ParseFile(k.parse.clone())) {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let cfg = db.input(&InputKey::CrateCfg(k.crate_key));
                match (parse.value.as_ref(), cfg.as_deref()) {
                    (QueryValue::Error(e), _) => QueryValue::Error(e.clone()),
                    (QueryValue::ParseFile(p), Some(InputValue::CrateCfg(cfg))) => {
                        if cfg.fingerprint() != k.cfg {
                            return Provided::error(
                                "LRR-E-INTERNAL-SNAPSHOT",
                                "outline cfg does not match immutable key",
                            );
                        }
                        if let Some(outline) = db.prepared_outline(k) {
                            return Provided::pure(QueryValue::Outline(outline));
                        }
                        match p.outline(k.crate_key, k.module_path.clone(), cfg) {
                            Ok(o) => QueryValue::Outline(Arc::new(o)),
                            Err(e) => QueryValue::Error(e),
                        }
                    }
                    _ => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "outline input or result mismatch",
                        );
                    }
                }
            }
            QueryKey::ScopeBucket(k) => {
                let blockers = db.input(&InputKey::ScopeBlockers(k.scope.clone()));
                let Some(InputValue::ScopeBlockers(blockers)) = blockers.as_deref() else {
                    return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "scope not published");
                };
                let candidates = db.input(&InputKey::NameCandidates(k.clone()));
                let candidates = match candidates.as_deref() {
                    None => vec![],
                    Some(InputValue::NameCandidates(c)) => c.clone(),
                    _ => return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "name input mismatch"),
                };
                QueryValue::ScopeBucket(Bucket {
                    candidates,
                    blockers: blockers.clone(),
                })
            }
            QueryKey::Signature(def) => {
                match db.input(&InputKey::DefinitionHeader(*def)).as_deref() {
                    Some(InputValue::DefinitionHeader(h)) => {
                        QueryValue::Signature(Arc::new(h.clone()))
                    }
                    _ => {
                        return Provided::error(
                            "LRR-E-NAME-MISSING",
                            "definition header not found",
                        );
                    }
                }
            }
            QueryKey::FunctionSignature(definition)
            | QueryKey::TypeAlias(definition)
            | QueryKey::TypeSchema(definition) => {
                let is_alias = matches!(key, QueryKey::TypeAlias(_));
                let is_schema = matches!(key, QueryKey::TypeSchema(_));
                let header = match db.fetch(self, QueryKey::Signature(*definition)) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let header = match header.value.as_ref() {
                    QueryValue::Signature(header) => header,
                    QueryValue::Error(error) => return QueryPoll::Ready(Err(error.clone())),
                    _ => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "function header result mismatch",
                        );
                    }
                };
                if header.attributes.iter().any(|attribute| {
                    attribute.class == crate::syntax::outline::AttributeClass::TransformUnsupported
                }) {
                    return Provided::error(
                        "LRR-E-UNSUPPORTED",
                        "function signature requires pending attribute-macro integration and macro authority",
                    );
                }
                let Some(source) = db.input(&InputKey::DefinitionSource(*definition)) else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "function source not published",
                    );
                };
                let InputValue::DefinitionSource(key) = source.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "function source input mismatch",
                    );
                };
                let parsed = match db.fetch(self, QueryKey::ParseFile(key.clone())) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "function source parse result mismatch",
                    );
                };
                let body = if is_schema {
                    None
                } else {
                    db.input(&InputKey::DefinitionBody(*definition))
                };
                let body = match body.as_deref() {
                    Some(InputValue::DefinitionBody(body)) => Some(body),
                    None => None,
                    _ => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "function body locator input mismatch",
                        );
                    }
                };
                let Some(context) = db.input(&InputKey::DeclarationContext(*definition)) else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "function declaration context not published",
                    );
                };
                let InputValue::DeclarationContext(context) = context.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "function declaration context input mismatch",
                    );
                };
                if is_schema {
                    let Some(cfg) = db.input(&InputKey::CrateCfg(context.scope.crate_key)) else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "schema cfg input not published",
                        );
                    };
                    let InputValue::CrateCfg(cfg) = cfg.as_ref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "schema cfg input variant differs",
                        );
                    };
                    match parsed.nominal_schema(*definition, header, context, cfg) {
                        Ok(schema) => QueryValue::TypeSchema(Box::new(schema)),
                        Err(error) => QueryValue::Error(error),
                    }
                } else if is_alias {
                    match parsed.type_alias_recipe(*definition, header, context) {
                        Ok(alias) => QueryValue::TypeAlias(Box::new(alias)),
                        Err(error) => QueryValue::Error(error),
                    }
                } else {
                    match parsed.function_signature(*definition, header, context, body) {
                        Ok(signature) => QueryValue::FunctionSignature(Box::new(signature)),
                        Err(error) => QueryValue::Error(error),
                    }
                }
            }

            QueryKey::BodyStructure(body) => {
                let header = match db.fetch(self, QueryKey::Signature(body.definition)) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                if let QueryValue::Error(error) = header.value.as_ref() {
                    return QueryPoll::Ready(Err(error.clone()));
                }
                if let QueryValue::Signature(signature) = header.value.as_ref()
                    && signature.attributes.iter().any(|attribute| {
                        attribute.class
                            == crate::syntax::outline::AttributeClass::TransformUnsupported
                    })
                {
                    return Provided::error(
                        "LRR-E-UNSUPPORTED",
                        "entered function requires pending attribute-macro integration and macro authority",
                    );
                }
                let Some(handle) = db.input(&InputKey::DefinitionBody(body.definition)) else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "entered definition body not published",
                    );
                };
                let InputValue::DefinitionBody(handle) = handle.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "definition body input mismatch",
                    );
                };
                if handle.key() != *body {
                    return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "entered body key is stale");
                }
                let Some(source) = db.input(&InputKey::DefinitionSource(body.definition)) else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "entered definition source not published",
                    );
                };
                let InputValue::DefinitionSource(parse_key) = source.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "definition source input mismatch",
                    );
                };
                let QueryValue::Signature(signature) = header.value.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "owning signature result mismatch",
                    );
                };
                if signature.profile != parse_key.profile {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "body source and owning signature profiles differ",
                    );
                }
                let parsed = match db.fetch(self, QueryKey::ParseFile(parse_key.clone())) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "body parse result mismatch",
                    );
                };
                let Some(context) = db.input(&InputKey::DeclarationContext(body.definition)) else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "body declaration context absent",
                    );
                };
                let InputValue::DeclarationContext(context) = context.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "body declaration context differs",
                    );
                };
                let Some(cfg) = db.input(&InputKey::CrateCfg(context.scope.crate_key)) else {
                    return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "body owning cfg absent");
                };
                let InputValue::CrateCfg(cfg) = cfg.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "body owning cfg variant differs",
                    );
                };
                match parsed.index_body_with_cfg(handle, cfg, Default::default()) {
                    Ok(index) => QueryValue::BodyStructure(Box::new(index)),
                    Err(error) => QueryValue::Error(error),
                }
            }
            QueryKey::Thunk(pc) | QueryKey::ConstructorPlan(pc) => {
                let constructor = matches!(key, QueryKey::ConstructorPlan(_));
                if constructor && pc.phase != 0 {
                    return Provided::error(
                        "LRR-E-QUERY-REQUEST",
                        "constructor plan requires entry phase",
                    );
                }
                let body = match db.fetch(self, QueryKey::BodyStructure(pc.body.clone())) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let index = match body.value.as_ref() {
                    QueryValue::BodyStructure(index) => index,
                    QueryValue::Error(error) => return QueryPoll::Ready(Err(error.clone())),
                    _ => {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "body structure result mismatch",
                        );
                    }
                };
                let handle = db.input(&InputKey::DefinitionBody(pc.body.definition));
                let source = db.input(&InputKey::DefinitionSource(pc.body.definition));
                let (
                    Some(InputValue::DefinitionBody(handle)),
                    Some(InputValue::DefinitionSource(parse_key)),
                ) = (handle.as_deref(), source.as_deref())
                else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "thunk source inputs missing",
                    );
                };
                let parsed = match db.fetch(self, QueryKey::ParseFile(parse_key.clone())) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
                };
                let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
                    return Provided::error(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "thunk parse result mismatch",
                    );
                };
                if constructor {
                    let context = db.input(&InputKey::DeclarationContext(pc.body.definition));
                    let Some(InputValue::DeclarationContext(context)) = context.as_deref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "constructor declaration context absent",
                        );
                    };
                    let cfg = db.input(&InputKey::CrateCfg(context.scope.crate_key));
                    let Some(InputValue::CrateCfg(cfg)) = cfg.as_deref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "constructor cfg input absent",
                        );
                    };
                    match parsed.constructor_plan(
                        handle,
                        index,
                        crate::ir::pc::NodeId(pc.node),
                        cfg,
                    ) {
                        Ok(Some(plan)) => QueryValue::ConstructorPlan(Box::new(plan)),
                        Ok(None) => {
                            return Provided::error(
                                "LRR-E-QUERY-REQUEST",
                                "selected expression is not a constructor",
                            );
                        }
                        Err(error) => QueryValue::Error(error),
                    }
                } else if index
                    .nodes()
                    .get(pc.node as usize)
                    .is_some_and(|node| node.constructor_guard.is_some())
                {
                    let mut entry = pc.clone();
                    entry.phase = crate::ir::pc::Phase::Entry.code().expect("entry encoding");
                    let plan = match db.fetch(self, QueryKey::ConstructorPlan(entry)) {
                        QueryPoll::Ready(Ok(outcome)) => outcome,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    };
                    let QueryValue::ConstructorPlan(plan) = plan.value.as_ref() else {
                        return Provided::error(
                            "LRR-E-INTERNAL-SNAPSHOT",
                            "constructor query result mismatch",
                        );
                    };
                    match parsed.constructor_thunk(handle, index, pc, plan) {
                        Ok(recipe) => QueryValue::Thunk(Box::new(recipe)),
                        Err(error) => QueryValue::Error(error),
                    }
                } else {
                    match parsed.initial_thunk(handle, index, pc) {
                        Ok(recipe) => QueryValue::Thunk(Box::new(recipe)),
                        Err(error) => QueryValue::Error(error),
                    }
                }
            }
            QueryKey::Constant(_) => {
                return Provided::error(
                    "LRR-E-NOT-YET-IMPLEMENTED",
                    "constant evaluation requires T054",
                );
            }
        };
        Provided::pure(value)
    }
}
