//! One memoized application of an actual entered named Type occurrence.
//! Representation metadata, not member selection or permission to execute.
use crate::{
 core::{diagnostic::Diagnostic,digest::{Digest,StableEncode,StableEncoder}},
 db::{memo::Database,key::QueryKey,poll::QueryPoll,provider::SyntaxProvider,value::QueryValue},
 frontend::{explicit_type_arguments::ExplicitType,source_locals::PreparedSourceLocalPlan,
  source_locals::owned_plan::{NamedTypeOutcome,type_declarations::BodyTypeDemand},
  source_types::NamedTypePreparation},
 ir::pc::{PcKey,NodeId},traits::term::TyTerm,
};
use std::sync::Arc;
fn fault(message:&str)->Diagnostic {Diagnostic::new("LRR-E-TYPE-CONTEXT",message.into())}
macro_rules! polled {($value:expr)=>{match $value {QueryPoll::Ready(Ok(value))=>value,QueryPoll::Ready(Err(error))=>return QueryPoll::Ready(Err(error)),QueryPoll::NeedInput(input)=>return QueryPoll::NeedInput(input)}};}
#[derive(Debug)]
pub struct PreparedBodyTypeApplication {key:PcKey,plan:Arc<PreparedSourceLocalPlan>,source:Option<ExplicitType>,trait_qualifier:Option<Digest>}
impl PreparedBodyTypeApplication {
 pub fn key(&self)->&PcKey {&self.key}
 pub(crate) fn retained_plan(&self)->&Arc<PreparedSourceLocalPlan>{&self.plan}
 pub(crate) fn source(&self)->Option<&ExplicitType>{self.source.as_ref()}
 pub(crate) fn source_for(&self,index:&crate::syntax::body_index::BodyIndex,site:&PcKey,range:crate::syntax::spans::SourceRange)->Result<Option<&ExplicitType>,Diagnostic>{
  if &self.key!=site{return Err(fault("Type application indexed source key differs"));}
  self.plan.context().source().require_type_site(index,site,range)?;
  Ok(self.source.as_ref())
 }
 pub(crate) fn trait_qualifier(&self)->Option<Digest>{self.trait_qualifier}
}
impl StableEncode for PreparedBodyTypeApplication {
 const DOMAIN:&'static str="lrr/v1/prepared-body-type-application";
 fn encode_fields(&self,e:&mut StableEncoder){e.record(&self.key);e.digest(self.plan.source_identity());e.option(&self.source,|e,source|e.record(source));e.option(&self.trait_qualifier,|e,id|e.digest(*id));}
}
pub(crate) fn prepare(db:&mut Database,key:&PcKey)->QueryPoll<PreparedBodyTypeApplication>{
 if key.phase!=0{return QueryPoll::Ready(Err(fault("Type application requires Entry")));}
 let selected=polled!(db.fetch(&SyntaxProvider,QueryKey::SourceLocalPlan(key.body.clone())));
 let QueryValue::SourceLocalPlan(plan)=selected.value.as_ref()else{return QueryPoll::Ready(Err(fault("Type application source plan variant differs")));};
 if !selected.cacheable || plan.context().source().index().body()!=&key.body {
  return QueryPoll::Ready(Err(fault("Type application actual source body differs")));
 }
 polled!(plan.require_current(db));
 let node=NodeId(key.node);
    let mut trait_qualifier = None;
    let outcome = polled!(plan.prepare_named_body_type_outcome(db, node));
    let prepared = match outcome {
        NamedTypeOutcome::Ready(value) => Some(value),
        NamedTypeOutcome::Pending { demand, .. } => {
            if let BodyTypeDemand::TypeRecipe(identity) = demand {
                if let Some(environment) = plan.environment() {
                    let members = match environment.macros().source_environment().member_index() {
                        Ok(members) => members,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    };
                    if members.is_trait(identity) {
                        trait_qualifier = Some(identity);
                    }
                }
            }
            None
        }
    };
    let source = match prepared {
        Some(NamedTypePreparation::Ready(actual)) => {
            let generic = match actual.source() {
                TyTerm::Nominal(identity, _) => {
                    let captured =
                        polled!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(*identity)));
                    let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                        return QueryPoll::Ready(Err(fault(
                            "associated qualifier schema variant differs",
                        )));
                    };
                    !schema.generics.is_empty()
                }
                _ => false,
            };
            (!generic).then(|| ExplicitType::Selected(actual.clone()))
        }
        Some(NamedTypePreparation::Parameter(parameter)) => {
            Some(ExplicitType::Parameter(TyTerm::Param(parameter)))
        }
        Some(NamedTypePreparation::Alias(crate::frontend::type_aliases::AliasTarget::Type(
            TyTerm::Primitive(primitive),
        ))) => match crate::frontend::source_types::SourceType::selected_primitive(primitive) {
            Ok(source) => Some(ExplicitType::Selected(source)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        },
        Some(NamedTypePreparation::Alias(crate::frontend::type_aliases::AliasTarget::Str)) => {
            match crate::frontend::source_types::SourceType::selected_str() {
                Ok(source) => Some(ExplicitType::Selected(source)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        }
        Some(NamedTypePreparation::Alias(crate::frontend::type_aliases::AliasTarget::Type(term))) => {
            polled!(crate::frontend::explicit_type_arguments::prepare_application(db,plan,term))
        }
        _ => None,
    };
    QueryPoll::Ready(Ok(PreparedBodyTypeApplication{key:key.clone(),plan:Arc::clone(plan),source,trait_qualifier}))
}
