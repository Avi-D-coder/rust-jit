//! Initial pure recipe construction. Successor PCs are data, never fetched here.
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        op::{DiagnosticTemplate, HintRecipe, LiteralRecipe, Op},
        pc::{NodeId, PcKey, SlotId},
        recipe::{Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        validate::verify,
    },
    syntax::body_index::BodyIndex,
};
/// Enter the indexed temporary boundary before its reached entry operations.
/// Shared by source and compiler-runtime-macro builders; bindings remain lexical.
pub(crate) fn with_temporary_scope_entry(
    body: &BodyIndex,
    mut recipe: RecipeDraft,
) -> Result<RecipeDraft, Diagnostic> {
    let node = body.nodes().get(recipe.key.node as usize).ok_or_else(|| {
        Diagnostic::new(
            "LRR-E-INTERNAL-IR",
            "temporary scope entry node absent".into(),
        )
    })?;
    if recipe.key.phase == crate::ir::pc::Phase::Entry.code()?
        && let Some(scope) = node.boolean_scope.or(node.statement_scope)
    {
        recipe.operations.try_reserve(1).map_err(|_| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "temporary scope entry recipe allocation".into(),
            )
        })?;
        recipe.operations.insert(0, Op::ScopeEnter { scope });
    }
    Ok(recipe)
}
fn draft(
    body: &BodyIndex,
    key: PcKey,
    operations: Vec<Op>,
    exit: RecipeExit,
) -> Result<RecipeDraft, Diagnostic> {
    let span = body.span(NodeId(key.node))?;
    with_temporary_scope_entry(
        body,
        RecipeDraft {
            key,
            origin: Origin {
                span,
                expansion_ancestry: Vec::new(),
            },
            operations,
            exits: vec![exit],
            dependencies: Vec::new(),
        },
    )
}
pub fn literal(
    body: &BodyIndex,
    key: PcKey,
    literal: LiteralRecipe,
    hint: HintRecipe,
    destination: SlotId,
    next: PcKey,
) -> Result<VerifiedRecipe, Diagnostic> {
    verify(
        draft(
            body,
            key,
            vec![Op::Literal {
                literal,
                hint,
                destination,
            }],
            RecipeExit::Next(next),
        )?,
        body,
    )
}
pub fn unit(
    body: &BodyIndex,
    key: PcKey,
    destination: SlotId,
    next: PcKey,
) -> Result<VerifiedRecipe, Diagnostic> {
    verify(
        draft(
            body,
            key,
            vec![Op::Unit { destination }],
            RecipeExit::Next(next),
        )?,
        body,
    )
}
pub fn branch(
    body: &BodyIndex,
    key: PcKey,
    phase_unit: SlotId,
    condition: SlotId,
    yes: PcKey,
    no: PcKey,
) -> Result<VerifiedRecipe, Diagnostic> {
    if phase_unit == condition {
        return Err(Diagnostic::new(
            "LRR-E-INTERNAL-IR",
            "branch phase unit aliases its condition".into(),
        ));
    }
    verify(
        draft(
            body,
            key,
            vec![Op::Unit {
                destination: phase_unit,
            }],
            RecipeExit::Branch { condition, yes, no },
        )?,
        body,
    )
}
pub fn unsupported(
    body: &BodyIndex,
    key: PcKey,
    message: String,
    next: PcKey,
) -> Result<VerifiedRecipe, Diagnostic> {
    verify(
        draft(
            body,
            key,
            vec![Op::Unsupported(DiagnosticTemplate { message })],
            RecipeExit::Next(next),
        )?,
        body,
    )
}
