//! Cold selection of a `let` statement's composite or non-primitive
//! annotation through the entered Type service: each written named leaf is its own indexed Type
//! occurrence's `PreparedTypePlan`, so a local, imported, qualified or
//! macro-emitted Type name resolves exactly as at any other Type use. The
//! installed recipe retains the joined input closure of every selected leaf.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::{Database, PreparedInputGuard},
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{
    core::diagnostic::Diagnostic,
    ir::{pc::PcKey, written_type::WrittenType},
    syntax::{
        body_index::BodyIndex,
        outline::BodyHandle,
        parse::ParsedFile,
        thunks_let_annotations::{AnnotationShape, LetAnnotation},
    },
    traits::term::TyTerm,
};
use std::sync::Arc;
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
pub(super) fn thunk(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    let environment = db.input(&InputKey::BodyEnvironment(pc.body.clone()));
    let Some(InputValue::BodyEnvironment(_)) = environment.as_deref() else {
        return QueryPoll::Ready(Ok(None));
    };
    let site = match parsed.let_annotation_site(body, index, pc) {
        Ok(Some(site)) => site,
        Ok(None) => return QueryPoll::Ready(Ok(None)),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let mut guards = Vec::new();
    let written = ready!(select(db, pc, &site.shape, &mut guards));
    let guard = {
        let borrowed: Vec<&PreparedInputGuard> = guards.iter().map(Arc::as_ref).collect();
        match PreparedInputGuard::join(&borrowed) {
            Ok(guard) => guard,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
    let annotation = LetAnnotation {
        range: site.range,
        written: Arc::new(written),
    };
    let recipe = match parsed.initial_thunk_with_let_annotation(body, index, pc, &annotation) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(Arc::new(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
pub(crate) fn select(
    db: &mut Database,
    pc: &PcKey,
    shape: &AnnotationShape,
    guards: &mut Vec<Arc<PreparedInputGuard>>,
) -> QueryPoll<WrittenType> {
    QueryPoll::Ready(Ok(match shape {
        AnnotationShape::Leaf(node) => {
            let key = PcKey {
                body: pc.body.clone(),
                node: node.0,
                phase: 0,
            };
            let query=QueryKey::PreparedBodyTypeApplication(key.clone());
            let prepared=ready!(db.fetch(&SyntaxProvider,query.clone()));
            let QueryValue::PreparedBodyTypeApplication(application)=prepared.value.as_ref()else{
                return QueryPoll::Ready(Err(fault("annotation Type application variant differs")));
            };
            if !prepared.cacheable || application.key()!=&key {
                return QueryPoll::Ready(Err(fault("annotation Type application is not retained")));
            }
            let owner=application.retained_plan().context().source();
            let node=owner.index().nodes().get(key.node as usize)
                .ok_or_else(||fault("annotation actual Type index absent"));
            let node=match node{Ok(node)=>node,Err(error)=>return QueryPoll::Ready(Err(error))};
            let (Some(start),Some(end))=(owner.body().range.start.checked_add(node.range.start),owner.body().range.start.checked_add(node.range.end))else{
                return QueryPoll::Ready(Err(fault("annotation Type range overflow")));
            };
            let source=match application.source_for(owner.index(),&key,crate::syntax::spans::SourceRange{start,end}){
                Ok(Some(source))=>source,Ok(None)=>return QueryPoll::Ready(Err(unsupported("annotation Type application requires actual source preparation"))),
                Err(error)=>return QueryPoll::Ready(Err(error)),
            };
            let guard=Arc::new(ready!(db.prepare_input_guard(&SyntaxProvider,&query,prepared.fingerprint)));
            if let Err(error)=guard.require_current(db){return QueryPoll::Ready(Err(error));}
            guards.push(guard);
            use crate::frontend::explicit_type_arguments::ExplicitType;
            match source {
                ExplicitType::Selected(source)=>{
                    if let Some(primitive)=source.primitive(){WrittenType::Primitive(primitive)}
                    else if matches!(source.source(),TyTerm::Str){WrittenType::Str}
                    else if matches!(source.source(),TyTerm::Nominal(_,args)if args.is_empty()){
                        WrittenType::Nominal(Box::new(source.clone()))
                    }else{return QueryPoll::Ready(Err(unsupported("local annotation Type requires further source integration")));}
                }
                ExplicitType::Written{written,..}=>written.as_ref().clone(),
                ExplicitType::Parameter(_)|ExplicitType::FrameTemplate{..}=>return QueryPoll::Ready(Err(unsupported("annotation Type parameter requires its owning frame hint"))),
            }
        }
        // `()` is the unit primitive, the one canonical spelling.
        AnnotationShape::Tuple(members) if members.is_empty() => {
            WrittenType::Primitive(crate::ir::op::PrimitiveType::Unit)
        }
        AnnotationShape::Tuple(members) => {
            let mut written = Vec::new();
            for member in members {
                written.push(ready!(select(db, pc, member, guards)));
            }
            WrittenType::Tuple(written)
        }
        AnnotationShape::Array(element, len) => {
            WrittenType::Array(Box::new(ready!(select(db, pc, element, guards))), *len)
        }
        AnnotationShape::Slice(element) => {
            WrittenType::Slice(Box::new(ready!(select(db, pc, element, guards))))
        }
        AnnotationShape::Ref {
            region,
            mutable,
            target,
        } => WrittenType::Ref {
            region: region.clone(),
            mutable: *mutable,
            target: Box::new(ready!(select(db, pc, target, guards))),
        },
        AnnotationShape::Infer => WrittenType::Infer,
        AnnotationShape::Ptr { mutable, target } => WrittenType::Ptr {
            mutable: *mutable,
            target: Box::new(ready!(select(db, pc, target, guards))),
        },
        AnnotationShape::FnPtr { params, result } => {
            let mut written = Vec::new();
            for param in params {
                written.push(ready!(select(db, pc, param, guards)));
            }
            WrittenType::FnPtr {
                params: written,
                result: Box::new(ready!(select(db, pc, result, guards))),
            }
        }
        AnnotationShape::Dyn(node) => {
            let principal = ready!(
                crate::frontend::nominal_constructors::dyn_annotation_principal(
                    db, &pc.body, *node
                )
            );
            guards.extend(principal.guards);
            WrittenType::Dyn(crate::traits::term::DynSchemaKey(principal.identity))
        }
        AnnotationShape::Generic(node, arguments) => {
            let base = ready!(
                crate::frontend::nominal_constructors::generic_annotation_base(db, &pc.body, *node)
            );
            if base.type_parameters != arguments.len() {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "written type argument count differs from the declaration".into(),
                )));
            }
            guards.extend(base.guards);
            let mut args = Vec::new();
            for argument in arguments {
                args.push(ready!(select(db, pc, argument, guards)));
            }
            WrittenType::Generic {
                identity: base.identity,
                definition: base.definition,
                schema: base.schema,
                args,
            }
        }
    }))
}
