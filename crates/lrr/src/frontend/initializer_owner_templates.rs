//! Actual declaration-owner templates for reached item applications. This is
//! source metadata, not an applicable-provider or initializer-entry proof.
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
    frontend::{
        source_locals::PreparedSourceLocalPlan,
        source_types::{WrittenTypeDeclaration, resolve_actual_type},
    },
    ir::pc::PcKey,
    syntax::{
        ast_adapter::{range, tokens},
        outline::DeclarationContext,
        parse::ParseKey,
        signatures::{FunctionSignature, GenericKind},
    },
    traits::term::{ParamKey, TyTerm},
};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct InitializerOwnerTemplateKey {
    pub caller_path: PcKey,
    pub definition: Digest,
}
impl StableEncode for InitializerOwnerTemplateKey {
    const DOMAIN: &'static str = "lrr/v1/initializer-owner-template-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.caller_path);
        e.digest(self.definition);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum InitializerOwnerDemand {
    ConfiguredSource,
    OwnerContext,
    OwnerType,
}
/// Real declaration ownership. `Impl.self_type` is expressed over the selected
/// constant signature's own ParamKeys, not impl/caller ordinals or descriptors.
#[derive(Debug)]
pub(crate) enum InitializerOwner {
    Unowned,
    Impl {
        identity: Digest,
        self_type: Option<TyTerm>,
    },
    Trait {
        identity: Digest,
        self_parameter: ParamKey,
    },
}

pub struct PreparedInitializerOwnerTemplate {
    key: InitializerOwnerTemplateKey,
    plan: Arc<PreparedSourceLocalPlan>,
    signature: Arc<QueryValue>,
    signature_fingerprint: Digest,
    source: ParseKey,
    context: DeclarationContext,
    cfg: Option<Digest>,
    owner: Option<InitializerOwner>,
    owner_syntax: Option<SyntaxNode>,
    demand: Option<InitializerOwnerDemand>,
}
impl std::fmt::Debug for PreparedInitializerOwnerTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedInitializerOwnerTemplate")
            .field("key", &self.key)
            .field("owner", &self.owner)
            .field("demand", &self.demand)
            .finish_non_exhaustive()
    }
}
impl PreparedInitializerOwnerTemplate {
    pub(crate) fn key(&self) -> &InitializerOwnerTemplateKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn signature(&self) -> &FunctionSignature {
        let QueryValue::FunctionSignature(signature) = self.signature.as_ref() else {
            unreachable!("private initializer-owner signature constructor checked the variant")
        };
        signature
    }
    pub(crate) fn owner(&self) -> Option<&InitializerOwner> {
        self.owner.as_ref()
    }
    pub(crate) fn demand(&self) -> Option<InitializerOwnerDemand> {
        self.demand
    }
}
impl StableEncode for PreparedInitializerOwnerTemplate {
    const DOMAIN: &'static str = "lrr/v1/prepared-initializer-owner-template";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        e.digest(self.signature_fingerprint);
        e.record(&self.source);
        e.record(&self.context.scope);
        e.u32(self.context.range.start);
        e.u32(self.context.range.end);
        e.option(&self.cfg, |e, cfg| e.digest(*cfg));
        e.option(&self.owner_syntax, |e, syntax| {
            let at = range(syntax.text_range());
            e.u32(at.start);
            e.u32(at.end);
        });
        e.option(&self.owner, |e, owner| match owner {
            InitializerOwner::Unowned => e.u8(0),
            InitializerOwner::Impl {
                identity,
                self_type,
            } => {
                e.u8(1);
                e.digest(*identity);
                e.option(self_type, |e, ty| e.record(ty));
            }
            InitializerOwner::Trait {
                identity,
                self_parameter,
            } => {
                e.u8(2);
                e.digest(*identity);
                e.record(self_parameter);
            }
        });
        e.option(&self.demand, |e, demand| {
            e.u8(match demand {
                InitializerOwnerDemand::ConfiguredSource => 0,
                InitializerOwnerDemand::OwnerContext => 1,
                InitializerOwnerDemand::OwnerType => 2,
            })
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ITEM-APPLICATION", message.into())
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
macro_rules! checked {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
}

pub(crate) fn prepare(
    db: &mut Database,
    key: &InitializerOwnerTemplateKey,
) -> QueryPoll<PreparedInitializerOwnerTemplate> {
    if key.caller_path.phase != 0 {
        return QueryPoll::Ready(Err(fault("initializer owner caller path phase differs")));
    }
    let caller = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::EnteredValue(key.caller_path.clone())
    ));
    let QueryValue::EnteredValue(caller_goal) = caller.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("initializer owner caller Value variant differs")));
    };
    if !caller.cacheable || caller_goal.key() != &key.caller_path {
        return QueryPoll::Ready(Err(fault(
            "initializer owner caller source is not retained",
        )));
    }
    let plan = Arc::clone(caller_goal.retained_plan());
    ready!(plan.require_current(db));
    let Some(environment) = plan.environment() else {
        return QueryPoll::Ready(Err(fault("initializer owner caller environment absent")));
    };
    let completed = Arc::clone(environment.macros().source_environment());

    let selected = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(key.definition)));
    let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("initializer owner signature variant differs")));
    };
    let source = db.input(&InputKey::DefinitionSource(key.definition));
    let header = db.input(&InputKey::DefinitionHeader(key.definition));
    let context = db.input(&InputKey::DeclarationContext(key.definition));
    let (
        Some(InputValue::DefinitionSource(source)),
        Some(InputValue::DefinitionHeader(header)),
        Some(InputValue::DeclarationContext(context)),
    ) = (source.as_deref(), header.as_deref(), context.as_deref())
    else {
        return QueryPoll::Ready(Err(fault(
            "initializer owner actual declaration inputs absent",
        )));
    };
    if !selected.cacheable
        || signature.definition != key.definition
        || signature.header != *header
        || signature.scope != context.scope
        || source.profile != signature.header.profile
    {
        return QueryPoll::Ready(Err(fault("initializer owner source signature differs")));
    }
    let source = source.clone();
    let context = context.clone();
    let configured = db.input(&InputKey::CrateCfg(context.scope.crate_key));
    let cfg = match configured.as_deref() {
        Some(InputValue::CrateCfg(cfg)) => Some(cfg.fingerprint()),
        None => None,
        _ => return QueryPoll::Ready(Err(fault("initializer owner cfg input variant differs"))),
    };
    let parsed = ready!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.clone())));
    if !parsed.cacheable {
        return QueryPoll::Ready(Err(fault("initializer owner parse is not retained")));
    }
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("initializer owner parse variant differs")));
    };
    if parsed.key() != source {
        return QueryPoll::Ready(Err(fault(
            "initializer owner parse source identity differs",
        )));
    }
    let syntax = checked!(parsed.syntax_at(context.range, |node| ast::Const::can_cast(
        node.kind()
    ) || ast::Static::can_cast(
        node.kind()
    )));
    let Some(syntax) = syntax else {
        return QueryPoll::Ready(Err(fault(
            "initializer owner actual const/static declaration absent",
        )));
    };
    let excluded = match ast::Const::cast(syntax.clone()) {
        Some(item) => item.body().map(|body| body.syntax().text_range()),
        None => ast::Static::cast(syntax.clone())
            .and_then(|item| item.body())
            .map(|body| body.syntax().text_range()),
    };
    if range(syntax.text_range()) != context.range || tokens(&syntax, excluded) != header.tokens {
        return QueryPoll::Ready(Err(fault(
            "initializer owner actual declaration/header differs",
        )));
    }
    let actual_owner = syntax
        .parent()
        .filter(|node| ast::AssocItemList::can_cast(node.kind()))
        .and_then(|items| items.parent())
        .filter(|node| ast::Impl::can_cast(node.kind()) || ast::Trait::can_cast(node.kind()));
    let mut output = PreparedInitializerOwnerTemplate {
        key: key.clone(),
        plan,
        signature: Arc::clone(&selected.value),
        signature_fingerprint: selected.fingerprint,
        source,
        context,
        cfg,
        owner: None,
        owner_syntax: actual_owner.clone(),
        demand: cfg
            .is_none()
            .then_some(InitializerOwnerDemand::ConfiguredSource),
    };
    if let Some(owner) = actual_owner {
        let items = owner
            .children()
            .find(|node| ast::AssocItemList::can_cast(node.kind()));
        if tokens(&owner, items.as_ref().map(|items| items.text_range()))
            != signature.header.context
        {
            return QueryPoll::Ready(Err(fault("initializer owner actual parent header differs")));
        }
        let Some(identity) = signature.scope.owner else {
            output.demand = Some(InitializerOwnerDemand::OwnerContext);
            return QueryPoll::Ready(Ok(output));
        };
        if let Some(implementation) = ast::Impl::cast(owner) {
            let mut self_type = None;
            if output.demand.is_none() {
                if let Some(ty) = implementation.self_ty() {
                    let declaration = WrittenTypeDeclaration::function_syntax(signature, &syntax);
                    let views: Vec<_> = completed.sources().iter().collect();
                    self_type = ready!(resolve_actual_type(
                        db,
                        completed.names(),
                        &views,
                        &declaration,
                        &ty
                    ));
                }
                if self_type.is_none() {
                    output.demand = Some(InitializerOwnerDemand::OwnerType);
                }
            }
            output.owner = Some(InitializerOwner::Impl {
                identity,
                self_type,
            });
        } else {
            // The existing genuine trait signature producer appends Self.
            let parameter = signature.generics.last().filter(|parameter| {
                parameter.kind == GenericKind::Type && parameter.name == "Self"
            });
            if let Some(parameter) = parameter {
                output.owner = Some(InitializerOwner::Trait {
                    identity,
                    self_parameter: ParamKey {
                        definition: signature.definition,
                        ordinal: parameter.ordinal,
                    },
                });
            } else {
                output.demand = Some(InitializerOwnerDemand::OwnerContext);
            }
        }
    } else {
        // A local item does not capture its enclosing function's parameters.
        // Nonempty header context still needs its actual enclosing envelope.
        if !signature.header.context.is_empty() {
            let enclosing = syntax.ancestors().skip(1).find_map(ast::Fn::cast);
            if enclosing.is_none_or(|function| {
                tokens(
                    function.syntax(),
                    function.body().map(|body| body.syntax().text_range()),
                ) != signature.header.context
            }) {
                output.demand = Some(InitializerOwnerDemand::OwnerContext);
                return QueryPoll::Ready(Ok(output));
            }
        }
        output.owner = Some(InitializerOwner::Unowned);
    }
    QueryPoll::Ready(Ok(output))
}
