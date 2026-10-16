//! Reached closed scalar match phases. No queries, descriptors or guest effects.
use super::pattern_tests as patterns;
use super::{draft, error, local_binding, parameter_at, primitive_cast_target, return_pc};
#[cfg(test)]
use crate::ir::op::{DecodedLiteral, LiteralKind, LiteralRecipe, NumericBinary};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::{scalar_pattern_binding::ScalarPatternBindingAdmission, thunks},
    ir::{
        op::{HintRecipe, Op, PatternOperation, PrimitiveType},
        pc::{NodeId, PcKey, Phase, SlotId},
        recipe::{Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
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
    patterns::source_type(pat)
}
/// The ordinal is an exact source relation, not a printed primitive proof.
/// Its scalar class is checked against the admitted parameter source by VM.
fn parameter_context(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expression: &ast::Expr,
) -> Result<Option<HintRecipe>, Diagnostic> {
    // Ordinary parameter lookup checks the actual token context in retained
    // syntax as well. The ordinal still grants no primitive/source authority.
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
    if expr
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return Ok(None);
    }
    // All ordinary control expressions forward the same known result
    // constraint into reached child slots. This is a source constraint, not
    // evidence about which branch executes or its runtime/source identity.
    // Do not inspect cold result shapes merely to rediscover this constraint.
    if matches!(
        expr,
        ast::Expr::MatchExpr(_) | ast::Expr::IfExpr(_) | ast::Expr::BlockExpr(_)
    ) && let Some(expected) = expected.filter(|ty| supported(*ty))
    {
        return Ok(Some(expected));
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
            SyntaxKind::BYTE => Some(PrimitiveType::U8),
            SyntaxKind::FLOAT_NUMBER => patterns::floating_context(x, expected),
            SyntaxKind::INT_NUMBER => {
                if let Some(ty) = patterns::floating_context(x, expected) {
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
            // Actual ordinary hygienic lookup chooses the source binding.
            // Primitive Type paths below still require their source admission;
            // provenance alone neither rejects nor proves a source constraint.
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
                let Some(statement) = parsed.binding_let_statement(body, index, binding)? else {
                    return Ok(None);
                };
                let Some(ast::Pat::IdentPat(binding)) = statement.pat() else {
                    return Ok(None);
                };
                if binding.ref_token().is_some() || binding.at_token().is_some() {
                    return Ok(None);
                }
                primitive_cast_target::resolve(parsed, index, statement.ty(), None)?
                    .filter(|p| supported(*p))
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
                primitive_cast_target::resolve(parsed, index, parameter.ty(), None)?
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
            // Without a concrete surrounding constraint, the existing partial
            // result-type collector still needs the general source goal engine.
            // Do not default an unresolved nonnumeric/call result or claim that
            // this fallback implements complete lazy result-type inference.
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
#[derive(Debug)]
struct Alternative {
    id: NodeId,
    source_type: Option<PrimitiveType>,
    empty_binding: bool,
    wildcard: bool,
    range: bool,
    attributed: bool,
    scalar: Result<patterns::ScalarTestPlan, Diagnostic>,
}
#[derive(Debug)]
struct Arm {
    guard: Option<NodeId>,
    missing_guard: bool,
    attributed: bool,
    id: NodeId,
    result: NodeId,
    binding: Option<NodeId>,
    empty_binding: bool,
    alternatives: Vec<Alternative>,
}
#[derive(Debug, Clone, Copy)]
enum Selection {
    Arm(usize),
    Alternative(usize, usize),
}
#[derive(Debug)]
pub(super) struct Plan {
    id: NodeId,
    source: NodeId,
    ty: HintRecipe,
    arms: Vec<Arm>,
    return_pc: PcKey,
    source_phase: u16,
    selections: Vec<(NodeId, Selection)>,
    continuations: Vec<(u16, usize, bool)>,
    bytes: usize,
}
impl Plan {
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn requires_binding_admission(&self, key: &PcKey) -> bool {
        matches!(Phase::from_code(key.phase), Phase::Entry | Phase::Commit)
            && matches!(self.selected(NodeId(key.node)), Some(Selection::Alternative(arm, alternative))
                if self.arms[arm].binding == Some(self.arms[arm].alternatives[alternative].id))
    }
    fn selected(&self, node: NodeId) -> Option<Selection> {
        self.selections
            .binary_search_by_key(&node, |row| row.0)
            .ok()
            .map(|i| self.selections[i].1)
    }
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
                output.push(Alternative {
                    id,
                    source_type: pattern_type(&pattern),
                    empty_binding: matches!(
                        pattern,
                        ast::Pat::LiteralPat(_)
                            | ast::Pat::WildcardPat(_)
                            | ast::Pat::RangePat(_)
                            | ast::Pat::PathPat(_)
                    ),
                    wildcard: matches!(pattern, ast::Pat::WildcardPat(_)),
                    range: matches!(pattern, ast::Pat::RangePat(_)),
                    attributed: pattern
                        .syntax()
                        .children()
                        .any(|n| ast::Attr::can_cast(n.kind())),
                    scalar: patterns::ScalarTestPlan::from_source(&pattern),
                });
            }
        }
    }
    Ok(Some(output))
}
pub(super) fn plan(
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
            expected = alternatives.iter().find_map(|a| a.source_type)
        }
        let Some(result) = syntax.expr() else {
            return Ok(None);
        };
        let guard = syntax
            .guard()
            .and_then(|guard| guard.condition())
            .map(|guard| identity(&rows, body, guard.syntax(), 0))
            .transpose()?;
        let binding = if !attributed && alternatives.len() == 1 {
            syntax.pat().and_then(|root| {
                let header =
                    patterns::IrrefutablePattern::from_source(&root, parsed.edition).ok()??;
                let binding = header.binding?;
                if header.writable || binding.name().is_none() {
                    return None;
                }
                let id = identity(&rows, body, binding.syntax(), 2).ok()?;
                (id == alternatives[0].id).then_some(id)
            })
        } else {
            None
        };
        let arm = Arm {
            guard,
            missing_guard: syntax.guard().is_some() && guard.is_none(),
            attributed,
            binding,
            empty_binding: alternatives
                .iter()
                .all(|alternative| alternative.empty_binding),
            id: identity(&rows, body, syntax.syntax(), 1)?,
            result: identity(&rows, body, result.syntax(), 0)?,
            alternatives,
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
    let count = arms.len().checked_add(total).ok_or_else(limit)?;
    let row_bytes = count
        .checked_mul(std::mem::size_of::<(NodeId, Selection)>())
        .and_then(|n| {
            arms.len()
                .checked_mul(2 * std::mem::size_of::<(u16, usize, bool)>())
                .and_then(|m| n.checked_add(m))
        })
        .ok_or_else(limit)?;
    bytes
        .checked_add(row_bytes)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(limit)?;
    let mut selections = Vec::new();
    selections.try_reserve_exact(count).map_err(|_| limit())?;
    let mut continuations = Vec::new();
    continuations
        .try_reserve_exact(arms.len().checked_mul(2).ok_or_else(limit)?)
        .map_err(|_| limit())?;
    for (i, arm) in arms.iter().enumerate() {
        selections.push((arm.id, Selection::Arm(i)));
        for (j, alternative) in arm.alternatives.iter().enumerate() {
            selections.push((alternative.id, Selection::Alternative(i, j)));
        }
        if let Some(guard) = arm.guard {
            continuations.push((operand(index, id, guard)?.code()?, i, true));
        }
        continuations.push((operand(index, id, arm.result)?.code()?, i, false));
    }
    selections.sort_unstable_by_key(|row| row.0);
    continuations.sort_unstable_by_key(|row| row.0);
    if selections.windows(2).any(|rows| rows[0].0 == rows[1].0)
        || continuations.windows(2).any(|rows| rows[0].0 == rows[1].0)
    {
        return Err(error("pattern retained phase identity ambiguous"));
    }
    let mut metadata = std::mem::size_of::<Plan>()
        .checked_add(
            arms.capacity()
                .checked_mul(std::mem::size_of::<Arm>())
                .ok_or_else(limit)?,
        )
        .and_then(|n| {
            n.checked_add(
                selections
                    .capacity()
                    .checked_mul(std::mem::size_of::<(NodeId, Selection)>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(continuations.capacity().checked_mul(std::mem::size_of::<(
                u16,
                usize,
                bool,
            )>())?)
        })
        .ok_or_else(limit)?;
    for arm in &arms {
        metadata = metadata
            .checked_add(
                arm.alternatives
                    .capacity()
                    .checked_mul(std::mem::size_of::<Alternative>())
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        for alternative in &arm.alternatives {
            if let Err(diagnostic) = &alternative.scalar {
                metadata = metadata
                    .checked_add(diagnostic.heap_bytes().ok_or_else(limit)?)
                    .ok_or_else(limit)?;
            }
        }
    }
    if metadata > 32 * 1024 * 1024 {
        return Err(limit());
    }
    Ok(Some(Plan {
        return_pc: return_pc(index, body, expression.syntax())?,
        bytes: metadata,
        id,
        source: source_id,
        ty,
        arms,
        selections,
        continuations,
        source_phase: operand(index, id, source_id)?.code()?,
    }))
}

fn empty_binding_header(arm: &Arm) -> bool {
    arm.empty_binding
}
/// Structural header only; the reached ordinary namespace proof remains required.
fn plain_binding_header(_parsed: &ParsedFile, arm: &Arm) -> Option<NodeId> {
    arm.binding
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
#[cfg(test)]
pub(super) fn prepare(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    syntax: SyntaxNode,
) -> Result<VerifiedRecipe, Diagnostic> {
    let retained = plan(parsed, index, body, &syntax)?;
    prepare_inner(parsed, body, index, key, retained.as_ref(), None)
}
pub(super) fn prepare_inner(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    retained: Option<&Plan>,
    admission: Option<&ScalarPatternBindingAdmission>,
) -> Result<VerifiedRecipe, Diagnostic> {
    let fallback = index.pc(NodeId(key.node), Phase::Exit)?;
    let unsupported =
        |message: &str| thunks::unsupported(index, key.clone(), message.into(), fallback.clone());
    let Some(plan) = retained else {
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
        if key.phase == plan.source_phase {
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
        if let Ok(row) = plan
            .continuations
            .binary_search_by_key(&key.phase, |row| row.0)
        {
            let (_, arm_ordinal, is_guard) = plan.continuations[row];
            let arm = &plan.arms[arm_ordinal];
            if is_guard {
                let guard = arm
                    .guard
                    .ok_or_else(|| error("prepared guard continuation absent"))?;
                let guard_scope = index.nodes()[arm.id.0 as usize]
                    .guard_scope
                    .ok_or_else(|| error("indexed match guard scope absent"))?;
                return draft(
                    index,
                    key,
                    vec![Op::ScopeLeave { scope: guard_scope }],
                    RecipeExit::Branch {
                        condition: value(index, guard)?,
                        yes: index.pc(arm.id, Phase::Commit)?,
                        no: index.pc(arm.id, Phase::Exit)?,
                    },
                );
            }
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
                RecipeExit::Next(plan.return_pc.clone()),
            );
        }
        return unsupported("invalid closed scalar match phase");
    }
    if let Some(Selection::Arm(ordinal)) = plan.selected(NodeId(key.node)) {
        let arm = &plan.arms[ordinal];
        if arm.attributed {
            return unsupported("reached match arm attributes require macro integration");
        }
        // Empty headers and one plain identifier may route structurally. Only
        // the reached pattern proof can establish that identifier is a binding;
        // range/constant semantics remain alternative-local.
        if !empty_binding_header(arm) && plain_binding_header(parsed, arm).is_none() {
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
                let guard_value = value(index, guard)?;
                let mut ops = vec![Op::Discard {
                    source: guard_value,
                }];
                let exit = if phase == Phase::Commit {
                    ops.push(Op::Pattern {
                        operation: PatternOperation::Finish {
                            owner: plan.id,
                            matched: None,
                        },
                    });
                    ops.push(Op::NumericHint {
                        parent: destination,
                        child: value(index, arm.result)?,
                        left: None,
                        operator: None,
                    });
                    RecipeExit::Next(index.pc(arm.result, Phase::Entry)?)
                } else {
                    ops.push(Op::ScopeLeave {
                        scope: index.nodes()[arm.id.0 as usize].scope,
                    });
                    RecipeExit::PatternRetry { owner: plan.id }
                };
                draft(index, key, ops, exit)
            }
            _ => unsupported("invalid guarded arm continuation phase"),
        };
    }
    let Some(Selection::Alternative(arm_ordinal, alternative_ordinal)) =
        plan.selected(NodeId(key.node))
    else {
        return unsupported("match alternative is disabled or outside selected source");
    };
    let arm = &plan.arms[arm_ordinal];
    let binding = plain_binding_header(parsed, arm);
    if arm.attributed || (!empty_binding_header(arm) && binding.is_none()) {
        return unsupported("reached alternative header requires macro/binding integration");
    }
    let alternative = &arm.alternatives[alternative_ordinal];
    let wildcard = alternative.wildcard;
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
    if alternative.attributed {
        return unsupported("reached alternative attributes require macro integration");
    }
    let retry = if let Some(next) = arm.alternatives.get(alternative_ordinal + 1) {
        index.pc(next.id, Phase::Entry)?
    } else if let Some(next) = plan.arms.get(arm_ordinal + 1) {
        index.pc(next.id, Phase::Entry)?
    } else {
        index.pc(plan.id, Phase::Exit)?
    };
    let begin = || Op::Pattern {
        operation: PatternOperation::Begin {
            owner: plan.id,
            alternative: alternative.id,
            source,
            retry: retry.clone(),
        },
    };
    let is_range = alternative.range;
    let scalar_test = if !wildcard && binding.is_none() {
        match &alternative.scalar {
            Ok(test) => Some(test),
            Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                return unsupported(&error.message);
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    match phase {
        Phase::Entry | Phase::Operand(0) if scalar_test.is_some() => {
            let scalar_test = scalar_test
                .as_ref()
                .ok_or_else(|| error("scalar test source disappeared"))?;
            if is_range && !index.nodes()[alternative.id.0 as usize].scalar_range_phase {
                return Err(error("range continuation lacks its structural marker"));
            }
            let upper = phase == Phase::Operand(0);
            if upper && !is_range {
                return unsupported("non-range scalar pattern has no upper phase");
            }
            let comparison = match if upper {
                scalar_test.upper(parsed, body, key)
            } else {
                scalar_test.first(parsed, body, key)
            } {
                Ok(comparison) => comparison,
                Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                    return unsupported(&error.message);
                }
                Err(error) => return Err(error),
            };
            let upper_pending = comparison.upper_pending;
            let mut ops = Vec::new();
            ops.try_reserve_exact(4).map_err(|_| limit())?;
            if upper {
                ops.push(Op::Discard { source: test });
            } else {
                ops.push(begin());
            }
            ops.extend(comparison.operations(source, expected, test, &plan.ty));
            draft(
                index,
                key,
                ops,
                RecipeExit::Branch {
                    condition: test,
                    yes: index.pc(
                        alternative.id,
                        if upper_pending {
                            Phase::Operand(0)
                        } else {
                            Phase::Commit
                        },
                    )?,
                    no: index.pc(alternative.id, Phase::Exit)?,
                },
            )
        }
        Phase::Entry if binding.is_some() => binding_draft(
            index,
            key,
            vec![begin()],
            RecipeExit::Next(index.pc(alternative.id, Phase::Commit)?),
            binding_admission.ok_or_else(|| error("binding admission disappeared"))?,
        ),
        Phase::Entry if wildcard => draft(
            index,
            key,
            vec![begin()],
            RecipeExit::Next(index.pc(alternative.id, Phase::Commit)?),
        ),
        Phase::Commit => {
            if arm.missing_guard {
                return unsupported("matched guard condition syntax absent");
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
                    Op::Pattern {
                        operation: PatternOperation::StageBinding {
                            owner: plan.id,
                            binding,
                            source: expected,
                            writable: false,
                        },
                    },
                ]);
            }
            let next = if let Some(guard) = arm.guard {
                ops.push(Op::Pattern {
                    operation: PatternOperation::Preview { owner: plan.id },
                });
                if binding.is_some() {
                    // The generic preview copied its pending input without
                    // consuming it. This separate ReadPlace scratch now retires
                    // normally; the actual scrutinee remains live for retry.
                    ops.push(Op::Discard { source: expected });
                }
                ops.push(Op::SetHint {
                    child: value(index, guard)?,
                    hint: HintRecipe::Primitive(PrimitiveType::Bool),
                });
                index.pc(guard, Phase::Entry)?
            } else {
                ops.push(Op::Pattern {
                    operation: PatternOperation::Finish {
                        owner: plan.id,
                        matched: None,
                    },
                });
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
        Phase::Exit if !wildcard && binding.is_none() => draft(
            index,
            key,
            vec![
                Op::Pattern {
                    operation: PatternOperation::Rollback { owner: plan.id },
                },
                Op::Discard { source: test },
            ],
            RecipeExit::Next(retry),
        ),
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

#[cfg(test)]
#[path = "byte_scalar_pattern_tests.rs"]
mod byte_scalar_pattern_tests;
