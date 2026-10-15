//! Reached closed scalar match phases. No queries, descriptors or guest effects.
use super::{draft, error, local_binding, parameter_at, primitive_cast_target, return_pc};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::thunks,
    ir::{
        op::{
            DecodedLiteral, HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, Op,
            PrimitiveType,
        },
        pc::{NodeId, PcKey, Phase, SlotId},
        recipe::{RecipeExit, VerifiedRecipe},
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
    ast::{self, HasGenericArgs},
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
    )
}
fn pattern_type(pat: &ast::Pat) -> Option<PrimitiveType> {
    let ast::Pat::LiteralPat(pat) = pat else {
        return None;
    };
    let literal = pat.literal()?;
    match literal.token().kind() {
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if pat.minus_token().is_none() => {
            Some(PrimitiveType::Bool)
        }
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
            SyntaxKind::INT_NUMBER => {
                ast::IntNumber::cast(x.token()).and_then(|n| match n.suffix() {
                    Some(s) => primitive(s),
                    None => Some(
                        expected
                            .filter(|p| *p != PrimitiveType::Bool)
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
        // This slice proves the empty binding header. Range/constant leaves
        // expose no bindings; their unsupported semantics stay alternative-local.
        if !empty_binding_header(arm) {
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
    if arm.attributed || !empty_binding_header(arm) {
        return unsupported("reached alternative header requires macro/binding integration");
    }
    let alternative = &arm.alternatives[alternative_ordinal];
    let wildcard = matches!(alternative.pattern, ast::Pat::WildcardPat(_));
    if alternative
        .pattern
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return unsupported("reached alternative attributes require macro integration");
    }
    if !wildcard {
        let ast::Pat::LiteralPat(pattern) = &alternative.pattern else {
            return unsupported("reached alternative requires scalar literal or wildcard");
        };
        let Some(literal) = pattern.literal() else {
            return unsupported("reached alternative literal absent");
        };
        if !matches!(
            literal.token().kind(),
            SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW | SyntaxKind::INT_NUMBER
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
            if !wildcard {
                ops.push(Op::Discard { source: test })
            }
            ops.push(Op::ScopeEnter {
                scope: index.nodes()[arm.id.0 as usize].scope,
            });
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
            draft(index, key, ops, RecipeExit::Next(next))
        }
        Phase::Exit if !wildcard => {
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
