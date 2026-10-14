//! Source-bound primitive recipe construction. No guest state or VM execution.
use super::{FloatingConstantPlan, FloatingConstantSite, fault};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{DependencyStamp, InputKey},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{names::NameNamespaces, source_locals::SourceLocalIndex},
    ir::{
        op::{FloatingConstantBinding, Op, PrimitiveFloatingConstant},
        pc::{PcKey, Phase},
        recipe::{Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        validate::verify,
    },
    syntax::body_index::{BodyIndex, ExpressionKind, NodeRole},
};

/// Retain this alongside an installed recipe. Revalidate before each new opcode
/// entry, including a PC cache hit or continuation after input publication.
#[derive(Debug)]
pub struct FloatingConstantRecipe {
    plan: FloatingConstantPlan,
    recipe: VerifiedRecipe,
}
impl FloatingConstantRecipe {
    pub fn recipe(&self) -> &VerifiedRecipe {
        &self.recipe
    }
    pub fn revalidate(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        locals: &SourceLocalIndex<'_>,
        site: FloatingConstantSite<'_>,
        body: &BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<()> {
        match self.plan.lower_recipe(db, graph, locals, site, body, pc) {
            QueryPoll::Ready(Ok(current))
                if current.recipe.recipe().fingerprint() == self.recipe.recipe().fingerprint() =>
            {
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Err(fault("floating recipe source proof changed")))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
impl FloatingConstantPlan {
    pub fn lower_recipe(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        locals: &SourceLocalIndex<'_>,
        site: FloatingConstantSite<'_>,
        body: &BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<FloatingConstantRecipe> {
        match self.revalidate(db, graph, locals, site) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        match lower(self, db, site, body, pc) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
fn lower(
    plan: &FloatingConstantPlan,
    db: &mut Database,
    site: FloatingConstantSite<'_>,
    body: &BodyIndex,
    pc: &PcKey,
) -> Result<QueryPoll<FloatingConstantRecipe>, Diagnostic> {
    if body.body() != &plan.source.body_key
        || &pc.body != body.body()
        || pc.phase != Phase::Entry.code()?
    {
        return Err(fault(
            "floating recipe body or entry PC differs from its source",
        ));
    }
    let node = body
        .nodes()
        .get(pc.node as usize)
        .ok_or_else(|| fault("floating recipe path node absent"))?;
    if node.role != NodeRole::Expression(ExpressionKind::Path) || body.span(node.id)? != plan.source
    {
        return Err(fault("floating recipe PC is not its exact written path"));
    }
    let destination = node
        .value
        .ok_or_else(|| fault("floating recipe value destination absent"))?;
    let owner = site
        .source
        .outline()
        .items
        .iter()
        .filter_map(|item| item.body.as_ref())
        .find(|owner| owner.key() == *body.body())
        .ok_or_else(|| fault("floating recipe source body absent"))?;
    let parse_key = QueryKey::ParseFile(site.source.selection().outline.parse.clone());
    let parsed = match db.fetch(&SyntaxProvider, parse_key.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    let QueryValue::ParseFile(parsed_file) = parsed.value.as_ref() else {
        return Err(fault("floating recipe parse query variant differs"));
    };
    let body_key = QueryKey::BodyStructure(body.body().clone());
    let indexed = match db.fetch(&SyntaxProvider, body_key.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    if !matches!(indexed.value.as_ref(), QueryValue::BodyStructure(current) if current.fingerprint() == body.fingerprint())
    {
        return Err(fault(
            "floating recipe index differs from the canonical source query",
        ));
    }
    let signature_key = QueryKey::FunctionSignature(body.body().definition);
    let signature = match db.fetch(&SyntaxProvider, signature_key.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    if !matches!(signature.value.as_ref(), QueryValue::FunctionSignature(current) if current.fingerprint() == plan.signature)
    {
        return Err(fault("floating recipe canonical signature changed"));
    }
    let end = owner
        .range
        .start
        .checked_add(plan.source.relative_end)
        .ok_or_else(|| fault("floating recipe source end overflow"))?;
    let start =
        u32::try_from(plan.at).map_err(|_| fault("floating recipe source start overflow"))?;
    let next = crate::syntax::floating_paths::floating_path_successor(
        parsed_file,
        owner,
        body,
        start,
        end,
    )?;
    let mut dependencies = Vec::new();
    dependencies.try_reserve_exact(10).map_err(|_| {
        Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "floating recipe dependency allocation".into(),
        )
    })?;
    for (key, fingerprint) in [
        (parse_key, parsed.fingerprint),
        (body_key, indexed.fingerprint),
        (signature_key, signature.fingerprint),
    ] {
        dependencies.push(DependencyStamp::Query { key, fingerprint });
    }
    for key in [
        InputKey::DefinitionSource(body.body().definition),
        InputKey::DefinitionHeader(body.body().definition),
        InputKey::DefinitionBody(body.body().definition),
        InputKey::DeclarationContext(body.body().definition),
        InputKey::CrateCfg(site.source.selection().outline.crate_key),
        InputKey::CapturedSource(site.source.selection().location_identity()),
        InputKey::SourceDependencies,
    ] {
        let input = db
            .input(&key)
            .ok_or_else(|| fault("floating recipe immutable source input absent"))?;
        dependencies.push(DependencyStamp::Input {
            key,
            fingerprint: input.fingerprint(),
        });
    }
    let constant = PrimitiveFloatingConstant::bind(FloatingConstantBinding {
        plan_identity: plan.fingerprint(),
        body_structure: body.fingerprint(),
        key: pc.clone(),
        next: next.clone(),
        source: plan.source.clone(),
        destination,
        primitive: plan.primitive,
        scalar: plan.value,
        edition: plan.edition,
        hygiene: plan.context,
        dependencies,
    })?;
    let dependencies = constant.dependencies().to_vec();
    let draft = super::super::thunks::with_temporary_scope_entry(
        body,
        RecipeDraft {
            key: pc.clone(),
            origin: Origin {
                span: plan.source.clone(),
                expansion_ancestry: vec![],
            },
            operations: vec![Op::PrimitiveFloatingConstant {
                constant: Box::new(constant),
                destination,
            }],
            exits: vec![RecipeExit::Next(next)],
            dependencies,
        },
    )?;
    let recipe = verify(draft, body)?;
    Ok(QueryPoll::Ready(Ok(FloatingConstantRecipe {
        plan: plan.clone(),
        recipe,
    })))
}
