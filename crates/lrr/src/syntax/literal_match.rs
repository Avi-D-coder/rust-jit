//! Reached closed scalar match phases. No queries, descriptors or guest effects.
#[path = "scalar_match_ranges.rs"]
mod ranges;
use super::{draft, error, local_binding, parameter_at, primitive_cast_target, return_pc};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::{scalar_pattern_binding::ScalarPatternBindingAdmission, thunks},
    ir::{
        op::{
            DecodedLiteral, HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, Op,
            PrimitiveType,
        },
        pc::{NodeId, PcKey, Phase, SlotId},
        recipe::{Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        spans::BodySpan,
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{self, HasGenericArgs, HasName},
};

fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "closed scalar match preparation budget".into(),
    )
}
fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .filter(|n| *n <= 100_000)
        .ok_or_else(limit)?;
    Ok(())
}
fn primitive(raw: &str) -> Option<PrimitiveType> {
    Some(match raw {
        "bool" => PrimitiveType::Bool,
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "usize" => PrimitiveType::Usize,
        _ => return None,
    })
}
fn supported(ty: PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::Bool
            | PrimitiveType::Char
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::Isize
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Usize
            | PrimitiveType::F32
            | PrimitiveType::F64
    )
}
fn pattern_type(pat: &ast::Pat) -> Option<PrimitiveType> {
    if let ast::Pat::RangePat(range) = pat {
        return ranges::source_type(range);
    }
    let ast::Pat::LiteralPat(pat) = pat else {
        return None;
    };
    let literal = pat.literal()?;
    match literal.token().kind() {
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if pat.minus_token().is_none() => {
            Some(PrimitiveType::Bool)
        }
        SyntaxKind::CHAR if pat.minus_token().is_none() => Some(PrimitiveType::Char),
        SyntaxKind::INT_NUMBER => ast::IntNumber::cast(literal.token())?
            .suffix()
            .and_then(primitive),
        _ => None,
    }
}
/// The ordinal is an exact source relation, not a printed primitive proof.
/// Its scalar class is checked against the admitted parameter source by VM.
fn parameter_context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expression: &ast::Expr,
) -> Result<Option<HintRecipe>, Diagnostic> {
    if parsed.generated_origin.is_some() {
        return Ok(None);
    }
    let mut expression = expression.clone();
    for _ in 0..256 {
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PathExpr(path) => {
                let Some(segment) = path
                    .path()
                    .filter(|p| p.qualifier().is_none())
                    .and_then(|p| p.segment())
                else {
                    return Ok(None);
                };
                if segment.coloncolon_token().is_some()
                    || segment.generic_arg_list().is_some()
                    || segment.parenthesized_arg_list().is_some()
                    || segment.type_anchor().is_some()
                    || local_binding(parsed, index, body, &path)?.is_some()
                {
                    return Ok(None);
                }
                let Some(name) = segment.name_ref() else {
                    return Ok(None);
                };
                let name = MacroName::parse(name.text(), parsed.edition)?;
                return Ok(parameter_at(parsed, path.syntax(), &name)?
                    .map(HintRecipe::DefinitionParameter));
            }
            _ => return Ok(None),
        }
    }
    Err(limit())
}
fn context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expr: &ast::Expr,
    expected: Option<PrimitiveType>,
    depth: usize,
    work: &mut usize,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    charge(work)?;
    if depth >= 256 {
        return Err(limit());
    }
    if parsed.generated_origin.is_some()
        || expr
            .syntax()
            .children()
            .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return Ok(None);
    }
    Ok(match expr {
        ast::Expr::ParenExpr(x) => match x.expr() {
            Some(x) => context(parsed, index, body, &x, expected, depth + 1, work)?,
            None => None,
        },
        ast::Expr::PrefixExpr(x) if x.op_token().is_some_and(|t| matches!(t.text(), "-" | "!")) => {
            match x.expr() {
                Some(x) => context(parsed, index, body, &x, expected, depth + 1, work)?,
                None => None,
            }
        }
        ast::Expr::Literal(x) => match x.token().kind() {
            SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => Some(PrimitiveType::Bool),
            SyntaxKind::CHAR => Some(PrimitiveType::Char),
            SyntaxKind::FLOAT_NUMBER => ranges::floating_context(x, expected),
            SyntaxKind::INT_NUMBER => {
                if let Some(ty) = ranges::floating_context(x, expected) {
                    return Ok(Some(ty));
                }
                ast::IntNumber::cast(x.token()).and_then(|n| match n.suffix() {
                    Some(s) => primitive(s),
                    None => Some(
                        expected
                            .filter(|p| {
                                !matches!(
                                    *p,
                                    PrimitiveType::Bool
                                        | PrimitiveType::Char
                                        | PrimitiveType::F32
                                        | PrimitiveType::F64
                                )
                            })
                            .unwrap_or(PrimitiveType::I32),
                    ),
                })
            }
            _ => None,
        },
        ast::Expr::PathExpr(x) => {
            let Some(path) = x.path() else {
                return Ok(None);
            };
            let Some(segment) = path.segment() else {
                return Ok(None);
            };
            if path.qualifier().is_some()
                || segment.coloncolon_token().is_some()
                || segment.generic_arg_list().is_some()
                || segment.parenthesized_arg_list().is_some()
                || segment.type_anchor().is_some()
            {
                return Ok(None);
            }
            if let Some(binding) = local_binding(parsed, index, body, x)? {
                let node = index
                    .nodes()
                    .get(binding.0 as usize)
                    .ok_or_else(|| error("match local binding absent"))?;
                let start = body
                    .range
                    .start
                    .checked_add(node.range.start)
                    .ok_or_else(limit)?;
                let end = body
                    .range
                    .start
                    .checked_add(node.range.end)
                    .ok_or_else(limit)?;
                let root = parsed.entered_body_root(body)?;
                for syntax in root.descendants() {
                    charge(work)?;
                    let Some(statement) = ast::LetStmt::cast(syntax) else {
                        continue;
                    };
                    let Some(ast::Pat::IdentPat(binding)) = statement.pat() else {
                        continue;
                    };
                    let range = binding.syntax().text_range();
                    if u32::from(range.start()) != start || u32::from(range.end()) != end {
                        continue;
                    }
                    if binding.ref_token().is_some() || binding.at_token().is_some() {
                        return Ok(None);
                    }
                    return Ok(
                        primitive_cast_target::resolve(parsed, index, statement.ty())?
                            .filter(|p| supported(*p)),
                    );
                }
                None
            } else {
                let Some(name) = segment.name_ref() else {
                    return Ok(None);
                };
                let name = MacroName::parse(name.text(), parsed.edition)?;
                let Some(ordinal) = parameter_at(parsed, x.syntax(), &name)? else {
                    return Ok(None);
                };
                let Some(function) = x.syntax().ancestors().find_map(ast::Fn::cast) else {
                    return Ok(None);
                };
                let Some(parameters) = function.param_list() else {
                    return Ok(None);
                };
                let Some(ordinal) =
                    ordinal.checked_sub(u32::from(parameters.self_param().is_some()))
                else {
                    return Ok(None);
                };
                let Some(parameter) = parameters.params().nth(ordinal as usize) else {
                    return Ok(None);
                };
                primitive_cast_target::resolve(parsed, index, parameter.ty())?
                    .filter(|p| supported(*p))
            }
        }
        // Closed literal result shape is a structural proof only. Arm thunks,
        // names and values remain cold; a general nested arm is not inferred.
        ast::Expr::BlockExpr(x)
            if x.stmt_list()
                .is_some_and(|s| s.statements().next().is_none()) =>
        {
            match x.tail_expr() {
                Some(x) => context(parsed, index, body, &x, expected, depth + 1, work)?,
                None => None,
            }
        }
        ast::Expr::MatchExpr(x) => {
            let Some(list) = x.match_arm_list() else {
                return Ok(None);
            };
            let mut common = None;
            let mut any = false;
            for arm in list.arms() {
                charge(work)?;
                let (enabled, attributed) = super::statement_cfg(arm.syntax(), index.cfg())?;
                if !enabled {
                    continue;
                }
                if attributed || arm.guard().is_some() {
                    return Ok(None);
                }
                let Some(result) = arm.expr() else {
                    return Ok(None);
                };
                let Some(ty) = context(parsed, index, body, &result, expected, depth + 1, work)?
                else {
                    return Ok(None);
                };
                if common.is_some_and(|old| old != ty) {
                    return Ok(None);
                }
                common = Some(ty);
                any = true;
            }
            if any { common } else { None }
        }
        _ => None,
    })
}
struct Alternative {
    pattern: ast::Pat,
    id: NodeId,
}
struct Arm {
    syntax: ast::MatchArm,
    guard: Option<NodeId>,
    attributed: bool,
    id: NodeId,
    result: NodeId,
    alternatives: Vec<Alternative>,
}
struct Plan {
    expression: ast::MatchExpr,
    id: NodeId,
    source: NodeId,
    ty: HintRecipe,
    arms: Vec<Arm>,
}
type IndexRows = Vec<((u32, u32, u8), NodeId)>;
fn tag(role: NodeRole) -> u8 {
    match role {
        NodeRole::Expression(_) => 0,
        NodeRole::MatchArm => 1,
        NodeRole::Pattern => 2,
        _ => 3,
    }
}
fn identity(
    rows: &IndexRows,
    body: &BodyHandle,
    syntax: &SyntaxNode,
    kind: u8,
) -> Result<NodeId, Diagnostic> {
    let r = syntax.text_range();
    let key = (
        u32::from(r.start())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
        u32::from(r.end())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
        kind,
    );
    rows.binary_search_by_key(&key, |r| r.0)
        .ok()
        .map(|i| rows[i].1)
        .ok_or_else(|| error("match indexed source identity absent"))
}
fn alternatives(
    rows: &IndexRows,
    body: &BodyHandle,
    pattern: ast::Pat,
    work: &mut usize,
    total: &mut usize,
    bytes: &mut usize,
) -> Result<Option<Vec<Alternative>>, Diagnostic> {
    // Account a conservative four-times capacity bound for both transient
    // work vectors before their first allocation. No guest storage is involved.
    let mut nodes = 0usize;
    for syntax in pattern.syntax().descendants() {
        charge(work)?;
        if ast::Pat::can_cast(syntax.kind()) {
            nodes = nodes.checked_add(1).ok_or_else(limit)?;
        }
    }
    let transient = nodes
        .checked_mul(4)
        .and_then(|n| {
            n.checked_mul(std::mem::size_of::<(ast::Pat, usize)>())
                .and_then(|a| {
                    n.checked_mul(std::mem::size_of::<ast::Pat>())
                        .and_then(|b| a.checked_add(b))
                })
        })
        .ok_or_else(limit)?;
    let maximum_output = nodes
        .checked_mul(4)
        .and_then(|n| n.checked_mul(std::mem::size_of::<Alternative>()))
        .ok_or_else(limit)?;
    bytes
        .checked_add(transient)
        .and_then(|n| n.checked_add(maximum_output))
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(limit)?;
    let mut output = Vec::new();
    let mut stack = Vec::new();
    stack.try_reserve_exact(1).map_err(|_| limit())?;
    stack.push((pattern, 0usize));
    while let Some((pattern, depth)) = stack.pop() {
        charge(work)?;
        if depth >= 256 {
            return Err(limit());
        }
        match pattern {
            ast::Pat::OrPat(or) => {
                let mut children = Vec::new();
                for child in or.pats() {
                    charge(work)?;
                    if children.len() >= 10_000 {
                        return Err(limit());
                    }
                    children.try_reserve(1).map_err(|_| limit())?;
                    children.push(child);
                }
                if children.is_empty() {
                    return Ok(None);
                }
                let size = stack
                    .len()
                    .checked_add(children.len())
                    .and_then(|n| n.checked_mul(std::mem::size_of::<(ast::Pat, usize)>()))
                    .and_then(|n| n.checked_add(*bytes))
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(limit)?;
                let _ = size;
                stack.try_reserve(children.len()).map_err(|_| limit())?;
                stack.extend(children.into_iter().rev().map(|p| (p, depth + 1)));
            }
            ast::Pat::ParenPat(group) => {
                let Some(inner) = group.pat() else {
                    return Ok(None);
                };
                stack.try_reserve(1).map_err(|_| limit())?;
                stack.push((inner, depth + 1));
            }
            pattern => {
                // Shape and bounds only. Semantic checks belong to the reached
                // arm/alternative, so an unsupported cold leaf stays cold.
                *total = total
                    .checked_add(1)
                    .filter(|n| *n <= 10_000)
                    .ok_or_else(limit)?;
                *bytes = bytes
                    .checked_add(
                        std::mem::size_of::<Alternative>()
                            .checked_mul(4)
                            .ok_or_else(limit)?,
                    )
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(limit)?;
                let id = identity(rows, body, pattern.syntax(), 2)?;
                output.try_reserve(1).map_err(|_| limit())?;
                output.push(Alternative { pattern, id });
            }
        }
    }
    Ok(Some(output))
}
fn plan(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<Option<Plan>, Diagnostic> {
    let Some(expression) = syntax.ancestors().find_map(ast::MatchExpr::cast) else {
        return Err(error("match owning expression absent"));
    };
    let mut work = 0usize;
    let mut rows = Vec::new();
    let mut bytes = index
        .nodes()
        .len()
        .checked_mul(std::mem::size_of::<((u32, u32, u8), NodeId)>())
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(limit)?;
    rows.try_reserve_exact(index.nodes().len())
        .map_err(|_| limit())?;
    for node in index.nodes() {
        charge(&mut work)?;
        if tag(node.role) < 3 {
            rows.push(((node.range.start, node.range.end, tag(node.role)), node.id))
        }
    }
    rows.sort_unstable_by_key(|r| r.0);
    if rows.windows(2).any(|r| r[0].0 == r[1].0) {
        return Err(error("match source identity ambiguous"));
    }
    let id = identity(&rows, body, expression.syntax(), 0)?;
    if index.nodes()[id.0 as usize].role != NodeRole::Expression(ExpressionKind::Match) {
        return Err(error("match indexed expression role differs"));
    }
    let Some(source) = expression.expr() else {
        return Ok(None);
    };
    let source_id = identity(&rows, body, source.syntax(), 0)?;
    let Some(list) = expression.match_arm_list() else {
        return Ok(None);
    };
    let mut arms = Vec::new();
    let mut expected = None;
    let mut total = 0usize;
    for syntax in list.arms() {
        charge(&mut work)?;
        let (enabled, attributed) = super::statement_cfg(syntax.syntax(), index.cfg())?;
        if !enabled {
            continue;
        }
        if arms.len() >= 10_000 {
            return Err(limit());
        }
        bytes = bytes
            .checked_add(
                std::mem::size_of::<Arm>()
                    .checked_mul(4)
                    .ok_or_else(limit)?,
            )
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(limit)?;
        let Some(pattern) = syntax.pat() else {
            return Ok(None);
        };
        let Some(alternatives) =
            alternatives(&rows, body, pattern, &mut work, &mut total, &mut bytes)?
        else {
            return Ok(None);
        };
        if alternatives.is_empty() {
            return Ok(None);
        }
        if expected.is_none() {
            expected = alternatives.iter().find_map(|a| pattern_type(&a.pattern))
        }
        let Some(result) = syntax.expr() else {
            return Ok(None);
        };
        let guard = syntax
            .guard()
            .and_then(|guard| guard.condition())
            .map(|guard| identity(&rows, body, guard.syntax(), 0))
            .transpose()?;
        let arm = Arm {
            guard,
            attributed,
            id: identity(&rows, body, syntax.syntax(), 1)?,
            result: identity(&rows, body, result.syntax(), 0)?,
            alternatives,
            syntax,
        };
        arms.try_reserve(1).map_err(|_| limit())?;
        arms.push(arm);
    }
    let ty = if let Some(primitive) = context(parsed, index, body, &source, expected, 0, &mut work)?
    {
        HintRecipe::Primitive(primitive)
    } else if let Some(parameter) = parameter_context(parsed, index, body, &source)? {
        // A current primitive source is required before ReadParameter. This
        // never imposes a later pattern's type or moves a non-scalar scrutinee.
        parameter
    } else {
        return Ok(None);
    };
    Ok(Some(Plan {
        expression,
        id,
        source: source_id,
        ty,
        arms,
    }))
}
fn empty_binding_header(arm: &Arm) -> bool {
    arm.alternatives.iter().all(|a| {
        matches!(
            a.pattern,
            ast::Pat::LiteralPat(_)
                | ast::Pat::WildcardPat(_)
                | ast::Pat::RangePat(_)
                | ast::Pat::PathPat(_)
        )
    })
}
/// Shape only. A name becomes a binding only after reached namespace admission.
/// Do not flatten an or/parenthesized binding into this deliberately narrow lane.
fn plain_binding_header(arm: &Arm) -> Option<NodeId> {
    let ast::Pat::IdentPat(pattern) = arm.syntax.pat()? else {
        return None;
    };
    if arm.attributed
        || arm.alternatives.len() != 1
        || pattern.ref_token().is_some()
        || pattern.mut_token().is_some()
        || pattern.at_token().is_some()
        || pattern.name().is_none()
        || pattern
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
        || arm.alternatives[0].pattern.syntax() != pattern.syntax()
    {
        return None;
    }
    Some(arm.alternatives[0].id)
}
fn binding_draft(
    index: &BodyIndex,
    key: &PcKey,
    operations: Vec<Op>,
    exit: RecipeExit,
    admission: &ScalarPatternBindingAdmission,
) -> Result<VerifiedRecipe, Diagnostic> {
    let required = admission.dependencies();
    if required.len() > 4096 {
        return Err(limit());
    }
    let mut bytes = required
        .len()
        .checked_mul(std::mem::size_of::<crate::db::dependency::DependencyStamp>())
        .ok_or_else(limit)?;
    for stamp in required {
        bytes = bytes
            .checked_add(stamp.heap_bytes())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(limit)?;
    }
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(required.len())
        .map_err(|_| limit())?;
    dependencies.extend_from_slice(required);
    crate::ir::validate::verify(
        thunks::with_temporary_scope_entry(
            index,
            RecipeDraft {
                key: key.clone(),
                origin: Origin {
                    span: index.span(NodeId(key.node))?,
                    expansion_ancestry: vec![],
                },
                operations,
                exits: vec![exit],
                dependencies,
            },
        )?,
        index,
    )
}
fn value(index: &BodyIndex, id: NodeId) -> Result<SlotId, Diagnostic> {
    index
        .nodes()
        .get(id.0 as usize)
        .and_then(|n| n.value)
        .ok_or_else(|| error("match expression value slot absent"))
}
fn operand(index: &BodyIndex, owner: NodeId, child: NodeId) -> Result<Phase, Diagnostic> {
    let n = index.nodes()[owner.0 as usize]
        .operands
        .iter()
        .position(|n| *n == child)
        .ok_or_else(|| error("match result is not its structural operand"))?;
    Ok(Phase::Operand(u16::try_from(n).map_err(|_| limit())?))
}
pub(super) fn prepare(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    syntax: SyntaxNode,
) -> Result<VerifiedRecipe, Diagnostic> {
    prepare_inner(parsed, body, index, key, syntax, None)
}
pub(super) fn prepare_binding(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    syntax: SyntaxNode,
    admission: &ScalarPatternBindingAdmission,
) -> Result<VerifiedRecipe, Diagnostic> {
    if syntax.kind() != SyntaxKind::IDENT_PAT
        || !matches!(Phase::from_code(key.phase), Phase::Entry | Phase::Commit)
    {
        return Err(error(
            "binding admission requires its identifier pattern entry/commit",
        ));
    }
    prepare_inner(parsed, body, index, key, syntax, Some(admission))
}
fn prepare_inner(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    syntax: SyntaxNode,
    admission: Option<&ScalarPatternBindingAdmission>,
) -> Result<VerifiedRecipe, Diagnostic> {
    let fallback = index.pc(NodeId(key.node), Phase::Exit)?;
    let unsupported =
        |message: &str| thunks::unsupported(index, key.clone(), message.into(), fallback.clone());
    let Some(plan) = plan(parsed, index, body, &syntax)? else {
        return unsupported(
            "closed scalar match requires supported zero-binding patterns and source-proven primitive context",
        );
    };
    let node = &index.nodes()[plan.id.0 as usize];
    let source = value(index, plan.source)?;
    let destination = value(index, plan.id)?;
    let [Some(expected), Some(test)] = node.runtime_temporaries else {
        return Err(error("match temporary slots absent"));
    };
    let phase = Phase::from_code(key.phase);
    if key.node == plan.id.0 {
        if phase == Phase::Entry {
            return draft(
                index,
                key,
                vec![Op::SetHint {
                    child: source,
                    hint: plan.ty.clone(),
                }],
                RecipeExit::Next(index.pc(plan.source, Phase::Entry)?),
            );
        }
        if phase == Phase::Exit {
            return draft(index, key, vec![], RecipeExit::MatchNoArm);
        }
        if phase == operand(index, plan.id, plan.source)? {
            let next = plan.arms.first().map(|a| a.id).unwrap_or(plan.id);
            let next_phase = if plan.arms.is_empty() {
                Phase::Exit
            } else {
                Phase::Entry
            };
            return draft(
                index,
                key,
                vec![],
                RecipeExit::Next(index.pc(next, next_phase)?),
            );
        }
        for arm in &plan.arms {
            if let Some(guard) = arm.guard
                && phase == operand(index, plan.id, guard)?
            {
                if arm.alternatives.len() != 1 {
                    return unsupported(
                        "guarded alternative continuation requires typed progress state",
                    );
                }
                let guard_scope = index.nodes()[arm.id.0 as usize]
                    .guard_scope
                    .ok_or_else(|| error("indexed match guard scope absent"))?;
                let guard_value = value(index, guard)?;
                return draft(
                    index,
                    key,
                    vec![Op::ScopeLeave { scope: guard_scope }],
                    RecipeExit::Branch {
                        condition: guard_value,
                        yes: index.pc(arm.id, Phase::Commit)?,
                        no: index.pc(arm.id, Phase::Exit)?,
                    },
                );
            }
            if phase == operand(index, plan.id, arm.result)? {
                return draft(
                    index,
                    key,
                    vec![
                        Op::ResultTransfer {
                            source: value(index, arm.result)?,
                            destination,
                        },
                        Op::ScopeLeave {
                            scope: index.nodes()[arm.id.0 as usize].scope,
                        },
                        Op::Discard { source },
                    ],
                    RecipeExit::Next(return_pc(index, body, plan.expression.syntax())?),
                );
            }
        }
        return unsupported("invalid closed scalar match phase");
    }
    if let Some(arm) = plan.arms.iter().find(|a| a.id.0 == key.node) {
        if arm.attributed {
            return unsupported("reached match arm attributes require macro integration");
        }
        // Empty headers and one plain identifier may route structurally. Only
        // the reached pattern proof can establish that identifier is a binding;
        // range/constant semantics remain alternative-local.
        if !empty_binding_header(arm) && plain_binding_header(arm).is_none() {
            return unsupported(
                "reached alternative binding header requires transactional pattern integration",
            );
        }
        return match phase {
            Phase::Entry => draft(
                index,
                key,
                vec![],
                RecipeExit::Next(index.pc(arm.alternatives[0].id, Phase::Entry)?),
            ),
            Phase::Commit | Phase::Exit => {
                let Some(guard) = arm.guard else {
                    return unsupported("unguarded arm has no guard continuation phase");
                };
                if arm.alternatives.len() != 1 {
                    return unsupported(
                        "guarded alternative continuation requires typed progress state",
                    );
                }
                let guard_value = value(index, guard)?;
                let mut ops = vec![Op::Discard {
                    source: guard_value,
                }];
                let next = if phase == Phase::Commit {
                    ops.push(Op::NumericHint {
                        parent: destination,
                        child: value(index, arm.result)?,
                        left: None,
                        operator: None,
                    });
                    index.pc(arm.result, Phase::Entry)?
                } else {
                    ops.push(Op::ScopeLeave {
                        scope: index.nodes()[arm.id.0 as usize].scope,
                    });
                    let ordinal = plan
                        .arms
                        .iter()
                        .position(|a| a.id == arm.id)
                        .ok_or_else(|| error("guard arm ordinal absent"))?;
                    match plan.arms.get(ordinal + 1) {
                        Some(next) => index.pc(next.id, Phase::Entry)?,
                        None => index.pc(plan.id, Phase::Exit)?,
                    }
                };
                draft(index, key, ops, RecipeExit::Next(next))
            }
            _ => unsupported("invalid guarded arm continuation phase"),
        };
    }
    let Some((arm_ordinal, alternative_ordinal)) =
        plan.arms.iter().enumerate().find_map(|(i, a)| {
            a.alternatives
                .iter()
                .position(|p| p.id.0 == key.node)
                .map(|j| (i, j))
        })
    else {
        return unsupported("match alternative is disabled or outside selected source");
    };
    let arm = &plan.arms[arm_ordinal];
    let binding = plain_binding_header(arm);
    if arm.attributed || (!empty_binding_header(arm) && binding.is_none()) {
        return unsupported("reached alternative header requires macro/binding integration");
    }
    let alternative = &arm.alternatives[alternative_ordinal];
    let wildcard = matches!(alternative.pattern, ast::Pat::WildcardPat(_));
    let binding_admission = if let Some(binding) = binding {
        if binding != alternative.id || !matches!(phase, Phase::Entry | Phase::Commit) {
            return unsupported("plain binding pattern requires its exact entry/commit phase");
        }
        let Some(admission) = admission else {
            return unsupported(
                "reached plain binding pattern requires current namespace admission",
            );
        };
        admission.require(parsed, body, index, key, binding)?;
        Some(admission)
    } else {
        None
    };
    if alternative
        .pattern
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return unsupported("reached alternative attributes require macro integration");
    }
    let range = if let ast::Pat::RangePat(pattern) = &alternative.pattern {
        match ranges::ScalarRange::from_source(pattern) {
            Ok(range) => Some(range),
            Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                return unsupported(&error.message);
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if !wildcard && binding.is_none() && range.is_none() {
        let ast::Pat::LiteralPat(pattern) = &alternative.pattern else {
            return unsupported("reached alternative requires scalar literal or wildcard");
        };
        let Some(literal) = pattern.literal() else {
            return unsupported("reached alternative literal absent");
        };
        if !matches!(
            literal.token().kind(),
            SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW | SyntaxKind::INT_NUMBER | SyntaxKind::CHAR
        ) || (literal.token().kind() != SyntaxKind::INT_NUMBER
            && pattern.minus_token().is_some())
            || (literal.token().kind() == SyntaxKind::INT_NUMBER
                && ast::IntNumber::cast(literal.token())
                    .and_then(|n| n.suffix().map(|s| primitive(s).is_none()))
                    .unwrap_or(false))
        {
            return unsupported("reached literal alternative requires supported scalar type");
        }
    }
    match phase {
        Phase::Entry | Phase::Operand(0) if range.is_some() => {
            let range = range
                .as_ref()
                .ok_or_else(|| error("range source disappeared"))?;
            if !index.nodes()[alternative.id.0 as usize].scalar_range_phase {
                return Err(error(
                    "range continuation lacks its original structural marker",
                ));
            }
            let upper = phase == Phase::Operand(0);
            let literal = match if upper {
                range.upper(body, key)
            } else {
                range.lower(body, key)
            } {
                Ok(literal) => literal,
                Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                    return unsupported(&error.message);
                }
                Err(error) => return Err(error),
            };
            let hint = ranges::hint(&literal, &plan.ty);
            let mut ops = Vec::new();
            ops.try_reserve_exact(if upper { 4 } else { 3 })
                .map_err(|_| limit())?;
            if upper {
                ops.push(Op::Discard { source: test });
            }
            ops.extend([
                Op::Literal {
                    literal,
                    hint,
                    destination: expected,
                },
                Op::NumericBinary {
                    left: source,
                    right: expected,
                    destination: test,
                    operator: if upper {
                        range.upper_comparison()
                    } else {
                        NumericBinary::Ge
                    },
                },
                Op::Discard { source: expected },
            ]);
            draft(
                index,
                key,
                ops,
                RecipeExit::Branch {
                    condition: test,
                    yes: index.pc(
                        alternative.id,
                        if upper {
                            Phase::Commit
                        } else {
                            Phase::Operand(0)
                        },
                    )?,
                    no: index.pc(alternative.id, Phase::Exit)?,
                },
            )
        }
        Phase::Entry if binding.is_some() => binding_draft(
            index,
            key,
            vec![],
            RecipeExit::Next(index.pc(alternative.id, Phase::Commit)?),
            binding_admission.ok_or_else(|| error("binding admission disappeared"))?,
        ),
        Phase::Entry if wildcard => draft(
            index,
            key,
            vec![],
            RecipeExit::Next(index.pc(alternative.id, Phase::Commit)?),
        ),
        Phase::Entry => {
            let ast::Pat::LiteralPat(pattern) = &alternative.pattern else {
                return Err(error("validated scalar alternative shape differs"));
            };
            let Some(literal) = pattern.literal() else {
                return Err(error("validated scalar literal absent"));
            };
            let r = pattern.syntax().text_range();
            let span = BodySpan {
                body_key: key.body.clone(),
                relative_start: u32::from(r.start())
                    .checked_sub(body.range.start)
                    .ok_or_else(limit)?,
                relative_end: u32::from(r.end())
                    .checked_sub(body.range.start)
                    .ok_or_else(limit)?,
            };
            let recipe = if pattern.minus_token().is_some() {
                let number = ast::IntNumber::cast(literal.token())
                    .ok_or_else(|| error("match integer token absent"))?;
                LiteralRecipe {
                    kind: LiteralKind::NegativeInteger,
                    spelling: pattern.syntax().text().to_string(),
                    decoded: Some(crate::syntax::literal_decode::integer(&number, true)),
                    source: span,
                }
            } else {
                super::literal_recipe(&literal, span)?
            };
            let hint = match &recipe.decoded {
                Some(DecodedLiteral::Integer {
                    suffix: Some(ty), ..
                }) => HintRecipe::Primitive(*ty),
                _ if recipe.kind == LiteralKind::Bool => HintRecipe::Primitive(PrimitiveType::Bool),
                Some(DecodedLiteral::Char(_)) => HintRecipe::Primitive(PrimitiveType::Char),
                _ => plan.ty.clone(),
            };
            draft(
                index,
                key,
                vec![
                    Op::Literal {
                        literal: recipe,
                        hint,
                        destination: expected,
                    },
                    Op::NumericBinary {
                        left: source,
                        right: expected,
                        destination: test,
                        operator: NumericBinary::Eq,
                    },
                    Op::Discard { source: expected },
                ],
                RecipeExit::Branch {
                    condition: test,
                    yes: index.pc(alternative.id, Phase::Commit)?,
                    no: index.pc(alternative.id, Phase::Exit)?,
                },
            )
        }
        Phase::Commit => {
            if arm.syntax.guard().is_some() && arm.guard.is_none() {
                return unsupported("matched guard condition syntax absent");
            }
            if arm.guard.is_some() && arm.alternatives.len() != 1 {
                return unsupported(
                    "guarded alternative continuation requires typed progress state",
                );
            }
            let mut ops = Vec::new();
            if !wildcard && binding.is_none() {
                ops.push(Op::Discard { source: test })
            }
            ops.push(Op::ScopeEnter {
                scope: index.nodes()[arm.id.0 as usize].scope,
            });
            if let Some(binding) = binding {
                let place = SlotId(expected.0.checked_add(1).ok_or_else(limit)?);
                if index.slot_kind(place)? != crate::ir::pc::SlotKind::Place {
                    return Err(error("match binding scratch place is not paired storage"));
                }
                ops.extend([
                    Op::ValuePlace {
                        source,
                        destination: place,
                    },
                    Op::ReadPlace {
                        place,
                        destination: expected,
                    },
                    Op::BindLocal {
                        binding,
                        source: expected,
                        hint: HintRecipe::None,
                        writable: false,
                    },
                ]);
            }
            let next = if let Some(guard) = arm.guard {
                ops.push(Op::SetHint {
                    child: value(index, guard)?,
                    hint: HintRecipe::Primitive(PrimitiveType::Bool),
                });
                index.pc(guard, Phase::Entry)?
            } else {
                ops.push(Op::NumericHint {
                    parent: destination,
                    child: value(index, arm.result)?,
                    left: None,
                    operator: None,
                });
                index.pc(arm.result, Phase::Entry)?
            };
            if let Some(admission) = binding_admission {
                binding_draft(index, key, ops, RecipeExit::Next(next), admission)
            } else {
                draft(index, key, ops, RecipeExit::Next(next))
            }
        }
        Phase::Exit if !wildcard && binding.is_none() => {
            let next = if let Some(next) = arm.alternatives.get(alternative_ordinal + 1) {
                index.pc(next.id, Phase::Entry)?
            } else if let Some(next) = plan.arms.get(arm_ordinal + 1) {
                index.pc(next.id, Phase::Entry)?
            } else {
                index.pc(plan.id, Phase::Exit)?
            };
            draft(
                index,
                key,
                vec![Op::Discard { source: test }],
                RecipeExit::Next(next),
            )
        }
        _ => unsupported("invalid closed scalar alternative phase"),
    }
}

#[cfg(test)]
#[path = "scalar_match_ranges_tests.rs"]
mod scalar_match_ranges_tests;

#[cfg(test)]
mod binding_route_tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    fn entered(source: &str, edition: u16, function: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([229; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == function)
            .unwrap()
            .body
            .clone()
            .unwrap();
        let index = parsed.index_body(&body, Default::default()).unwrap();
        (parsed, body, index)
    }
    fn arm_id(index: &BodyIndex, body: &BodyHandle, arm: &ast::MatchArm) -> NodeId {
        let mut rows: IndexRows = index
            .nodes()
            .iter()
            .map(|node| ((node.range.start, node.range.end, tag(node.role)), node.id))
            .collect();
        rows.sort_by_key(|row| row.0);
        identity(&rows, body, arm.syntax(), 1).unwrap()
    }
    #[test]
    fn original_guarded_identifier_routes_to_reached_pattern_without_binding_effects() {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = entered(ORIGINAL, edition, "label_break_match");
            let root = parsed.entered_body_root(&body).unwrap();
            let matched = root.descendants().find_map(ast::MatchExpr::cast).unwrap();
            let arm = matched.match_arm_list().unwrap().arms().nth(1).unwrap();
            let arm_id = arm_id(&index, &body, &arm);
            let pattern = arm.pat().unwrap();
            assert!(matches!(pattern, ast::Pat::IdentPat(_)));
            let pattern_id = super::super::id_for(&index, &body, pattern.syntax()).unwrap();
            let recipe = prepare(
                &parsed,
                &body,
                &index,
                &index.pc(arm_id, Phase::Entry).unwrap(),
                arm.syntax().clone(),
            )
            .unwrap();
            assert_eq!(
                recipe.recipe().exit,
                RecipeExit::Next(index.pc(pattern_id, Phase::Entry).unwrap())
            );
            assert!(!recipe.recipe().operations.iter().any(|op| matches!(
                op,
                Op::ValuePlace { .. } | Op::ReadPlace { .. } | Op::BindLocal { .. }
            )));
            for phase in [Phase::Entry, Phase::Commit] {
                let refused = prepare(
                    &parsed,
                    &body,
                    &index,
                    &index.pc(pattern_id, phase).unwrap(),
                    pattern.syntax().clone(),
                )
                .unwrap();
                assert!(
                    refused
                        .recipe()
                        .operations
                        .iter()
                        .any(|op| matches!(op, Op::Unsupported(_)))
                );
                assert!(!refused.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::ValuePlace { .. } | Op::ReadPlace { .. } | Op::BindLocal { .. }
                )));
            }
            let first = matched
                .match_arm_list()
                .unwrap()
                .arms()
                .next()
                .unwrap()
                .pat()
                .unwrap();
            let first_id = super::super::id_for(&index, &body, first.syntax()).unwrap();
            let literal = prepare(
                &parsed,
                &body,
                &index,
                &index.pc(first_id, Phase::Entry).unwrap(),
                first.syntax().clone(),
            )
            .unwrap();
            assert!(
                literal
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::Literal { .. }))
            );
            assert!(literal.recipe().operations.iter().any(|op| matches!(
                op,
                Op::NumericBinary {
                    operator: NumericBinary::Eq,
                    ..
                }
            )));
        }
    }
    #[test]
    fn modifiers_and_alternative_bindings_stay_outside_plain_binding_route() {
        for pattern in ["mut v", "ref v", "v @ _", "(v)", "v | v"] {
            let source = format!("fn chosen(c:u8){{match c{{{pattern}=>{{}},_=>{{}}}}}}");
            let (parsed, body, index) = entered(&source, 2024, "chosen");
            let matched = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::MatchExpr::cast)
                .unwrap();
            let arm = matched.match_arm_list().unwrap().arms().next().unwrap();
            let id = arm_id(&index, &body, &arm);
            let refused = prepare(
                &parsed,
                &body,
                &index,
                &index.pc(id, Phase::Entry).unwrap(),
                arm.syntax().clone(),
            )
            .unwrap();
            assert!(
                refused
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::Unsupported(_))),
                "{pattern}"
            );
            assert!(
                !refused.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::ValuePlace { .. } | Op::ReadPlace { .. } | Op::BindLocal { .. }
                )),
                "{pattern}"
            );
        }
    }
}
