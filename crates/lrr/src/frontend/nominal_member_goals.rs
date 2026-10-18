//! Pure specialization of one source-selected nominal member under actual
//! admitted Type arguments. Requests contain source terms, never guest IDs.
use super::types::{
    BoundNominalField, BoundNominalVariant, NominalBinding, NominalFieldBinding, SchemaNames,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    source::manifest::{ItemKind, PcKey},
    traits::term::TyTerm,
};
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum NominalMemberSelection {
    Variant(Option<String>),
    Named,
    Positional(u32),
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct NominalMemberKey {
    pub(crate) pc: PcKey,
    pub(crate) definition: Digest,
    pub(crate) arguments: Vec<TyTerm>,
    pub(crate) selection: NominalMemberSelection,
}
impl NominalMemberKey {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .arguments
            .capacity()
            .checked_mul(std::mem::size_of::<TyTerm>())?;
        for term in &self.arguments {
            bytes = bytes.checked_add(term.heap_bytes()?)?;
        }
        if let NominalMemberSelection::Variant(Some(name)) = &self.selection {
            bytes = bytes.checked_add(name.capacity())?;
        }
        Some(bytes)
    }
}
impl StableEncode for NominalMemberKey {
    const DOMAIN: &'static str = "lrr/v1/nominal-member-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.pc);
        e.digest(self.definition);
        e.sequence(&self.arguments, |e, term| e.record(term));
        match &self.selection {
            NominalMemberSelection::Variant(name) => {
                e.u32(0);
                e.option(name, |e, name| e.string(name));
            }
            NominalMemberSelection::Named => e.u32(1),
            NominalMemberSelection::Positional(field) => {
                e.u32(2);
                e.u32(*field);
            }
        }
    }
}
#[derive(Debug, Clone)]
enum BoundMember {
    Variant(Arc<BoundNominalVariant>),
    Field(BoundNominalField),
}
#[derive(Debug, Clone)]
pub struct PreparedNominalMember {
    key: NominalMemberKey,
    owner: Arc<super::source_locals::PreparedSourceLocalPlan>,
    bound: BoundMember,
    shape: super::types::FieldShape,
}
impl PreparedNominalMember {
    pub(crate) fn shape(&self) -> super::types::FieldShape {
        self.shape
    }
    pub(crate) fn key(&self) -> &NominalMemberKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<super::source_locals::PreparedSourceLocalPlan> {
        &self.owner
    }
    pub(crate) fn variant(&self) -> Option<&Arc<BoundNominalVariant>> {
        match &self.bound {
            BoundMember::Variant(variant) => Some(variant),
            _ => None,
        }
    }
    pub(crate) fn field(&self) -> Option<&BoundNominalField> {
        match &self.bound {
            BoundMember::Field(field) => Some(field),
            _ => None,
        }
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.key.heap_bytes()?)?
            .checked_add(match &self.bound {
                BoundMember::Variant(value) => value.estimated_bytes()?,
                BoundMember::Field(value) => value.estimated_bytes()?,
            })
    }
}
impl StableEncode for PreparedNominalMember {
    const DOMAIN: &'static str = "lrr/v1/prepared-nominal-member";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.owner.source_identity());
        e.u32(match self.shape {
            super::types::FieldShape::Unit => 0,
            super::types::FieldShape::Tuple => 1,
            super::types::FieldShape::Record => 2,
        });
        match &self.bound {
            BoundMember::Variant(value) => {
                e.u32(0);
                e.record(value.as_ref());
            }
            BoundMember::Field(value) => {
                e.u32(1);
                e.record(value);
            }
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
fn pending() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "nominal member requires further source Type argument/projection binding".into(),
    )
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
    key: &NominalMemberKey,
) -> QueryPoll<PreparedNominalMember> {
    if key.pc.phase != 0 {
        return QueryPoll::Ready(Err(fault("nominal member requires source entry phase")));
    }
    checked!(key.heap_bytes().ok_or_else(crate::runtime::arena::resource));
    let owner = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceLocalPlan(key.pc.body.clone())
    ));
    let QueryValue::SourceLocalPlan(owner) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("nominal member owner outcome differs")));
    };
    ready!(owner.require_current(db));
    let Some(environment) = owner.environment() else {
        return QueryPoll::Ready(Err(fault("nominal member source environment absent")));
    };
    let completed = environment.macros().source_environment();
    let sources: Vec<_> = completed.sources().iter().collect();
    let schema = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(key.definition)));
    let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("nominal member schema outcome differs")));
    };
    let names = ready!(SchemaNames::from_current_database(
        db,
        completed.names(),
        &sources,
        schema
    ));
    let members = checked!(completed.member_index());
    let arguments: Vec<_> = key.arguments.iter().cloned().map(Some).collect();
    let shape = checked!(schema.variant(match &key.selection {
        NominalMemberSelection::Variant(name) => name.as_deref(),
        _ => None,
    }))
    .shape;
    let bound = match &key.selection {
        NominalMemberSelection::Variant(name) => {
            match ready!(names.bind_variant_current_with_arguments(
                db,
                &sources,
                name.as_deref(),
                &arguments,
                &members
            )) {
                NominalBinding::Ready(bound) => BoundMember::Variant(Arc::new(bound)),
                NominalBinding::Deferred { .. } => return QueryPoll::Ready(Err(pending())),
            }
        }
        NominalMemberSelection::Named => {
            if names.definition().item_kind != ItemKind::Struct {
                return QueryPoll::Ready(Err(pending()));
            }
            let source = owner.context().source();
            let site = checked!(source.parsed().named_field_site(
                source.body(),
                source.index(),
                crate::ir::pc::NodeId(key.pc.node)
            ));
            checked!(source.require_identifier_origin(db, &sources, site.range(), site.member()));
            match ready!(names.bind_named_field_current_with_arguments(
                db,
                &sources,
                None,
                site.member(),
                source.scope(),
                &arguments,
                &members
            )) {
                NominalFieldBinding::Ready(bound) => BoundMember::Field(bound),
                NominalFieldBinding::Deferred { .. } => return QueryPoll::Ready(Err(pending())),
            }
        }
        NominalMemberSelection::Positional(field) => {
            use ra_ap_syntax::{AstNode, ast};
            if names.definition().item_kind != ItemKind::Struct {
                return QueryPoll::Ready(Err(pending()));
            }
            let source = owner.context().source();
            let expression = checked!(source.parsed().entered_node_syntax(
                source.body(),
                source.index(),
                crate::ir::pc::NodeId(key.pc.node)
            ));
            let selected = expression
                .and_then(ast::FieldExpr::cast)
                .and_then(|field| field.name_ref())
                .and_then(|name| name.text().parse::<u32>().ok());
            if selected != Some(*field) {
                return QueryPoll::Ready(Err(fault(
                    "positional field differs from indexed source selector",
                )));
            }
            match ready!(names.bind_positional_field_current_with_arguments(
                db,
                &sources,
                None,
                *field,
                source.scope(),
                &arguments,
                &members
            )) {
                NominalFieldBinding::Ready(bound) => BoundMember::Field(bound),
                NominalFieldBinding::Deferred { .. } => return QueryPoll::Ready(Err(pending())),
            }
        }
    };
    QueryPoll::Ready(Ok(PreparedNominalMember {
        key: key.clone(),
        owner: Arc::clone(owner),
        bound,
        shape,
    }))
}
