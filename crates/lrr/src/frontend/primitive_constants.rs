//! Closed primitive floating constants selected in an exact entered type namespace.
//! Planning is pure; the dispatcher must revalidate before any guest mutation.
use super::{
    call_contracts::{ArgumentType, primitive_runtime_id},
    names::{NameLookup, NameNamespaces},
    signature_names::{SignatureName, SignatureNames},
    source_locals::{OrdinaryNameSite, SourceLocalIndex},
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
    ir::op::PrimitiveType,
    runtime::{shape::TypeId, value::ValueRepr},
    source::{
        changes::PreparedSourceView,
        manifest::{BodyKey, Namespace},
    },
    syntax::{spans::BodySpan, token_wire::Key},
    traits::term::TyTerm,
};

#[derive(Clone, Copy)]
pub struct FloatingConstantSite<'a> {
    pub source: &'a PreparedSourceView,
    pub body: &'a BodyKey,
    pub at: u64,
    pub span: &'a BodySpan,
    pub qualifier: &'a str,
    pub member: &'a str,
    pub context: Key,
    pub expected: Option<&'a ArgumentType>,
}
#[derive(Debug)]
pub enum FloatingConstantOutcome {
    Ready(FloatingConstantPlan),
    NeedType(SignatureName),
    UnsupportedMember { primitive: PrimitiveType },
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Member {
    Nan,
    Infinity,
    NegInfinity,
    Min,
    Max,
    MinPositive,
    Epsilon,
}
impl Member {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "NAN" => Self::Nan,
            "INFINITY" => Self::Infinity,
            "NEG_INFINITY" => Self::NegInfinity,
            "MIN" => Self::Min,
            "MAX" => Self::Max,
            "MIN_POSITIVE" => Self::MinPositive,
            "EPSILON" => Self::Epsilon,
            _ => return None,
        })
    }
    fn tag(self) -> u32 {
        match self {
            Self::Nan => 0,
            Self::Infinity => 1,
            Self::NegInfinity => 2,
            Self::Min => 3,
            Self::Max => 4,
            Self::MinPositive => 5,
            Self::Epsilon => 6,
        }
    }
}
/// Owns only stable scalar/source metadata. No inference transaction or native symbol.
#[derive(Debug, Clone)]
pub struct FloatingConstantPlan {
    selection: Digest,
    signature: Digest,
    declaration_scope: Digest,
    source: BodySpan,
    at: u64,
    edition: u16,
    context: Key,
    primitive: PrimitiveType,
    member: Member,
    value: ValueRepr,
}
impl StableEncode for FloatingConstantPlan {
    const DOMAIN: &'static str = "lrr/v4/primitive-floating-constant-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.selection);
        e.digest(self.signature);
        e.digest(self.declaration_scope);
        e.record(&self.source.body_key);
        e.u32(self.source.relative_start);
        e.u32(self.source.relative_end);
        e.u64(self.at);
        e.u16(self.edition);
        e.digest(Digest(self.context.0));
        e.u32(self.primitive.tag());
        e.u32(self.member.tag());
        e.u32(self.value.kind);
        e.u32(self.value.type_id);
        e.u64(self.value.payload);
    }
}
impl FloatingConstantPlan {
    pub fn scalar(&self) -> ValueRepr {
        self.value
    }
    pub fn source_span(&self) -> &BodySpan {
        &self.source
    }
    pub fn edition(&self) -> u16 {
        self.edition
    }
    pub fn hygiene_context(&self) -> Key {
        self.context
    }
    pub fn source_type(&self) -> TyTerm {
        TyTerm::Primitive(self.primitive)
    }
    pub fn descriptor(&self) -> TypeId {
        TypeId(self.value.type_id)
    }
    /// Namespace, source and declaration checks remain pure and must precede commit.
    pub fn revalidate(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        locals: &SourceLocalIndex<'_>,
        site: FloatingConstantSite<'_>,
    ) -> QueryPoll<()> {
        match plan_floating_constant(db, graph, locals, site) {
            QueryPoll::Ready(Ok(FloatingConstantOutcome::Ready(current)))
                if current.fingerprint() == self.fingerprint() =>
            {
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Err(fault("floating constant source selection changed")))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}

/// Never infer a primitive from a matching runtime descriptor or printed qualifier.
/// Body-local type items/imports are checked before declaration/builtin fallback.
pub fn plan_floating_constant(
    db: &mut Database,
    graph: &NameNamespaces,
    locals: &SourceLocalIndex<'_>,
    site: FloatingConstantSite<'_>,
) -> QueryPoll<FloatingConstantOutcome> {
    match prepare(db, graph, locals, site) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
fn prepare(
    db: &mut Database,
    graph: &NameNamespaces,
    locals: &SourceLocalIndex<'_>,
    site: FloatingConstantSite<'_>,
) -> Result<QueryPoll<FloatingConstantOutcome>, Diagnostic> {
    if site.member.len() > 64 {
        return Err(Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "primitive constant name budget exceeded".into(),
        ));
    }
    graph.require_view(site.source)?;
    let owner = site
        .source
        .outline()
        .items
        .iter()
        .find(|item| {
            item.body
                .as_ref()
                .is_some_and(|body| body.key() == *site.body)
        })
        .ok_or_else(|| fault("floating constant body is absent from its exact source"))?;
    let body = owner
        .body
        .as_ref()
        .ok_or_else(|| fault("floating constant owner body absent"))?;
    if site.span.body_key != *site.body
        || site.span.relative_start > site.span.relative_end
        || site.span.relative_end > body.range.len()
        || body
            .range
            .start
            .checked_add(site.span.relative_start)
            .map(u64::from)
            != Some(site.at)
        || site.context != locals.context()
    {
        return Err(fault(
            "floating constant span or hygiene differs from its entered source",
        ));
    }
    if !matches!(db.input(&InputKey::DefinitionSource(site.body.definition)).as_deref(),
        Some(InputValue::DefinitionSource(parse)) if parse == &site.source.selection().outline.parse)
    {
        return Err(fault("floating constant source/database revision mismatch"));
    }
    let parsed = match db.fetch(
        &SyntaxProvider,
        QueryKey::ParseFile(site.source.selection().outline.parse.clone()),
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Err(fault("floating constant parse query variant differs"));
    };
    let end = body
        .range
        .start
        .checked_add(site.span.relative_end)
        .ok_or_else(|| fault("floating constant source end overflow"))?;
    let start =
        u32::try_from(site.at).map_err(|_| fault("floating constant source position overflow"))?;
    let path = crate::syntax::floating_paths::written_two_segment_path(parsed, body, start, end)?
        .ok_or_else(|| {
        fault("floating constant requires its exact written two-segment path")
    })?;
    if path.qualifier != site.qualifier || path.member != site.member {
        return Err(fault(
            "floating constant names differ from its exact written source path",
        ));
    }
    let signature = match db.fetch(
        &SyntaxProvider,
        QueryKey::FunctionSignature(site.body.definition),
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
        return Err(fault("floating constant declaration query variant differs"));
    };
    let names = SignatureNames::new(graph, site.source, signature)?;
    SignatureNames::from_database(db, graph, signature)?;
    let found = locals.lookup_ordinary(
        OrdinaryNameSite {
            source: site.source.selection(),
            body: site.body,
            at: site.at,
            name: site.qualifier,
            context: site.context,
            namespace: Namespace::Type,
        },
        graph,
        None,
    )?;
    let selected = match found {
        NameLookup::Absent => names.unqualified(site.qualifier, None)?,
        NameLookup::Defined(definition) => SignatureName::Source(definition),
        other => SignatureName::Unresolved(other),
    };
    let primitive = match selected {
        SignatureName::Primitive(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) => {
            primitive
        }
        other => {
            return Ok(QueryPoll::Ready(Ok(FloatingConstantOutcome::NeedType(
                other,
            ))));
        }
    };
    let Some(member) = Member::parse(site.member) else {
        return Ok(QueryPoll::Ready(Ok(
            FloatingConstantOutcome::UnsupportedMember { primitive },
        )));
    };
    let descriptor = primitive_runtime_id(primitive)?;
    if let Some(expected) = site.expected {
        if expected.descriptor != descriptor {
            return Err(fault(
                "floating constant descriptor differs from reached expression context",
            ));
        }
        if expected.source != TyTerm::Primitive(primitive) {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "floating constant source identity differs from reached expression context".into(),
            ));
        }
    }
    let value = scalar(primitive, member);
    value.validate_scalar()?;
    if value.type_id != descriptor.0 {
        return Err(fault(
            "primitive floating constant descriptor invariant failed",
        ));
    }
    let mut selection = StableEncoder::new("lrr/v4/primitive-constant-source-selection");
    let outline = &site.source.selection().outline;
    selection.record(&outline.parse);
    selection.digest(outline.crate_key);
    selection.sequence(&outline.module_path, |e, name| e.string(name));
    selection.digest(outline.cfg);
    selection.string(site.source.selection().path.as_str());
    Ok(QueryPoll::Ready(Ok(FloatingConstantOutcome::Ready(
        FloatingConstantPlan {
            selection: selection.finish(),
            signature: signature.fingerprint(),
            declaration_scope: signature.scope.fingerprint(),
            source: site.span.clone(),
            at: site.at,
            edition: signature.header.profile.edition,
            context: site.context,
            primitive,
            member,
            value,
        },
    ))))
}
fn scalar(primitive: PrimitiveType, member: Member) -> ValueRepr {
    match primitive {
        PrimitiveType::F32 => ValueRepr::f32(match member {
            Member::Nan => f32::NAN,
            Member::Infinity => f32::INFINITY,
            Member::NegInfinity => f32::NEG_INFINITY,
            Member::Min => f32::MIN,
            Member::Max => f32::MAX,
            Member::MinPositive => f32::MIN_POSITIVE,
            Member::Epsilon => f32::EPSILON,
        }),
        PrimitiveType::F64 => ValueRepr::f64(match member {
            Member::Nan => f64::NAN,
            Member::Infinity => f64::INFINITY,
            Member::NegInfinity => f64::NEG_INFINITY,
            Member::Min => f64::MIN,
            Member::Max => f64::MAX,
            Member::MinPositive => f64::MIN_POSITIVE,
            Member::Epsilon => f64::EPSILON,
        }),
        _ => unreachable!("closed floating primitive selected above"),
    }
}

#[cfg(test)]
#[path = "primitive_constants_tests.rs"]
mod tests;

#[path = "primitive_float_recipe.rs"]
mod recipe;
pub use recipe::FloatingConstantRecipe;

#[cfg(test)]
#[path = "primitive_float_recipe_tests.rs"]
mod recipe_tests;

#[cfg(test)]
#[path = "floating_syntax_boundary_tests.rs"]
mod syntax_boundary_tests;
