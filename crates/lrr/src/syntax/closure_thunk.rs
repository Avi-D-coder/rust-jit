//! Ordinary lowering of closure expressions.
//!
//! A closure expression's Entry runs in the frame that evaluates it and forms
//! the closure value (`Op::MakeClosure`) from the enclosing bindings its body
//! names (rustc's capture inference over whole variables). Its body is part
//! of the same indexed body but executes only in the closure's own call
//! frame: that frame enters at the closure's Commit phase (entering the
//! closure scope), runs the body as ordinary code, and returns the body's
//! value from the closure's Operand(0) continuation. Closure parameters are
//! the frame's parameters, read like a function's; captured bindings are
//! bound by the call to the cells the closure value captured.
use super::{binding_name_matches, draft, error, id_for, return_pc};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    frontend::thunks,
    ir::{
        op::{CaptureMode, CaptureSource, ClosureCapture, ClosureParameter, Op},
        pc::{NodeId, PcKey, Phase},
        recipe::{ControlRecipe, RecipeExit, VerifiedRecipe},
    },
    syntax::{body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile},
};
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasName},
};

/// The nearest closure whose body contains `syntax` within the current
/// function, if any.
pub(super) fn enclosing_closure(syntax: &SyntaxNode) -> Option<ast::ClosureExpr> {
    for ancestor in syntax.ancestors().skip(1) {
        if ast::Fn::can_cast(ancestor.kind()) {
            return None;
        }
        if let Some(closure) = ast::ClosureExpr::cast(ancestor)
            && closure
                .body()
                .is_some_and(|body| body.syntax().text_range().contains_range(syntax.text_range()))
        {
            return Some(closure);
        }
    }
    None
}

/// The ordinal of the closure parameter `name` names, when `closure`'s
/// parameter list binds it (identifier patterns only).
pub(super) fn closure_parameter(
    parsed: &ParsedFile,
    closure: &ast::ClosureExpr,
    reference: &SyntaxNode,
    name: &MacroName,
) -> Result<Option<u32>, Diagnostic> {
    let Some(list) = closure.param_list() else {
        return Ok(None);
    };
    for (ordinal, param) in list.params().enumerate() {
        // Every identifier a parameter pattern binds (`|&(a, b)|`).
        let Some(pattern) = param.pat() else {
            continue;
        };
        for candidate in pattern
            .syntax()
            .descendants()
            .filter_map(ast::IdentPat::cast)
            .filter_map(|ident| ident.name())
        {
            if binding_name_matches(parsed, reference, name, &candidate)? {
                return Ok(Some(
                    u32::try_from(ordinal)
                        .map_err(|_| error("closure parameter ordinal overflow"))?,
                ));
            }
        }
    }
    Ok(None)
}

/// How a use of a captured variable at `path` uses it.
fn capture_use(path: &SyntaxNode) -> CaptureMode {
    // Climb the place expression the path roots (fields, indexes, parens).
    let mut current = path.clone();
    loop {
        let Some(parent) = current.parent() else {
            return CaptureMode::Consumed;
        };
        if ast::ParenExpr::can_cast(parent.kind()) {
            current = parent;
            continue;
        }
        if let Some(field) = ast::FieldExpr::cast(parent.clone())
            && field.expr().is_some_and(|base| base.syntax() == &current)
        {
            current = parent;
            continue;
        }
        if let Some(index) = ast::IndexExpr::cast(parent.clone())
            && index.base().is_some_and(|base| base.syntax() == &current)
        {
            current = parent;
            continue;
        }
        if let Some(reference) = ast::RefExpr::cast(parent.clone()) {
            return if reference.mut_token().is_some() {
                CaptureMode::Mutable
            } else {
                CaptureMode::Shared
            };
        }
        if let Some(binary) = ast::BinExpr::cast(parent.clone())
            && let Some(operator) = binary.op_token()
        {
            let left = binary.lhs().is_some_and(|left| left.syntax() == &current);
            return match operator.text() {
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
                    if left =>
                {
                    CaptureMode::Mutable
                }
                // Comparison operators borrow their operands.
                "==" | "!=" | "<" | "<=" | ">" | ">=" => CaptureMode::Shared,
                _ => CaptureMode::Consumed,
            };
        }
        // A method receiver is autoref'd (or read by value through it); a
        // called closure or function value is borrowed by its call.
        if let Some(call) = ast::MethodCallExpr::cast(parent.clone())
            && call.receiver().is_some_and(|receiver| receiver.syntax() == &current)
        {
            return CaptureMode::Shared;
        }
        if let Some(call) = ast::CallExpr::cast(parent.clone())
            && call.expr().is_some_and(|callee| callee.syntax() == &current)
        {
            return CaptureMode::Shared;
        }
        // A deref reads the reference to reach its referent.
        if let Some(prefix) = ast::PrefixExpr::cast(parent.clone())
            && prefix.op_token().is_some_and(|token| token.text() == "*")
        {
            return CaptureMode::Shared;
        }
        // `format_args!` borrows each argument.
        if ast::FormatArgsArg::can_cast(parent.kind()) {
            return CaptureMode::Shared;
        }
        // Any other use reads (copies or moves) the value or projected part.
        return CaptureMode::Consumed;
    }
}

fn merge(found: &mut Vec<ClosureCapture>, source: CaptureSource, mode: CaptureMode) {
    let rank = |mode: CaptureMode| match mode {
        CaptureMode::Shared => 0,
        CaptureMode::Mutable => 1,
        CaptureMode::Consumed => 2,
        CaptureMode::Value => 3,
    };
    if let Some(existing) = found.iter_mut().find(|capture| capture.source == source) {
        // A consuming use subsumes borrows; a unique borrow subsumes a
        // shared one. A consuming use that turns out Copy needs only a
        // shared borrow, so a mutation alongside it keeps the unique one.
        existing.mode = match (existing.mode, mode) {
            (CaptureMode::Mutable, CaptureMode::Consumed)
            | (CaptureMode::Consumed, CaptureMode::Mutable) => CaptureMode::Mutable,
            (a, b) if rank(b) > rank(a) => b,
            (a, _) => a,
        };
    } else {
        found.push(ClosureCapture { source, mode });
    }
}

/// Capture-use classification over actual ordinary indexed expressions. The
/// caller supplies identities selected by the shared hygienic lexical query.
pub(crate) fn capture_mode(path: &SyntaxNode, by_value: bool) -> CaptureMode {
    if by_value { CaptureMode::Value } else { capture_use(path) }
}
pub(crate) fn merge_capture(found: &mut Vec<ClosureCapture>, source: CaptureSource, mode: CaptureMode) {
    merge(found, source, mode);
}

impl ParsedFile {
    /// Identifier parameters only: their binding mutability and written
    /// primitive annotation.
    fn closure_parameters(
        &self,
        closure: &ast::ClosureExpr,
    ) -> Result<Result<Box<[ClosureParameter]>, &'static str>, Diagnostic> {
        let mut parameters = Vec::new();
        let Some(list) = closure.param_list() else {
            return Ok(Err("closure parameter list absent"));
        };
        for param in list.params() {
            // A pattern parameter matches its argument through the shared
            // pattern transaction (`pattern_plan`), the closure as its owner.
            let Some(ast::Pat::IdentPat(pattern)) = param.pat() else {
                parameters.push(ClosureParameter {
                    mutable: false,
                    hint: None,
                });
                continue;
            };
            if pattern.ref_token().is_some() || pattern.at_token().is_some() {
                parameters.push(ClosureParameter {
                    mutable: false,
                    hint: None,
                });
                continue;
            }
            // A written primitive annotation types the argument; another
            // annotation is not re-enforced (rustc checked it).
            let hint = match super::hint(param.ty()) {
                Some(crate::ir::op::HintRecipe::Primitive(primitive)) => Some(primitive),
                _ => None,
            };
            parameters.push(ClosureParameter {
                mutable: pattern.mut_token().is_some(),
                hint,
            });
        }
        Ok(Ok(parameters.into_boxed_slice()))
    }
    #[inline(never)]
    pub(super) fn closure_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
        captures: Option<&[ClosureCapture]>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        let closure = ast::ClosureExpr::cast(syntax).expect("checked closure");
        if closure.async_token().is_some()
            || closure.static_token().is_some()
            || closure.for_binder().is_some()
        {
            return unsupported("async, static and higher-ranked closures await integration");
        }
        let Some(closure_scope) = node.closure_scope else {
            return Err(error("closure scope absent"));
        };
        let Some(closure_body) = closure.body() else {
            return unsupported("closure body absent");
        };
        let body_id = id_for(index, body, closure_body.syntax())?;
        match Phase::from_code(key.phase) {
            Phase::Entry => {
                let parameters = match self.closure_parameters(&closure)? {
                    Ok(parameters) => parameters,
                    Err(reason) => return unsupported(reason),
                };
                let Some(captures) = captures else {
                    return unsupported("closure captures require admitted hygienic source selection");
                };
                let captures = captures.to_vec().into_boxed_slice();
                draft(
                    index,
                    key,
                    vec![Op::MakeClosure {
                        body: body_id,
                        captures,
                        parameters,
                        destination: node.value.ok_or_else(|| error("closure value absent"))?,
                    }],
                    RecipeExit::Next(return_pc(index, body, closure.syntax())?),
                )
            }
            // The closure's own call frame starts here: enter the closure
            // scope and bind each parameter from its call argument.
            Phase::Commit => {
                let mut operations = vec![Op::ScopeEnter {
                    scope: closure_scope,
                }];
                if let Some(parameters) = closure.param_list() {
                    for (ordinal, parameter) in parameters.params().enumerate() {
                        let Some(ast::Pat::IdentPat(pattern)) = parameter.pat() else {
                            return unsupported(
                                "closure parameter pattern awaits parameter pattern integration",
                            );
                        };
                        let hint = super::hint(parameter.ty())
                            .unwrap_or(crate::ir::op::HintRecipe::None);
                        let staging = *node
                            .runtime_scalar_slots
                            .get(ordinal)
                            .ok_or_else(|| error("closure parameter staging slot absent"))?;
                        operations.push(Op::ReadParameter {
                            ordinal: u32::try_from(ordinal)
                                .map_err(|_| error("closure parameter ordinal overflow"))?,
                            destination: staging,
                        });
                        operations.push(Op::BindLocal {
                            binding: id_for(index, body, pattern.syntax())?,
                            source: staging,
                            hint,
                            writable: pattern.mut_token().is_some(),
                        });
                    }
                }
                draft(
                    index,
                    key,
                    operations,
                    RecipeExit::Next(index.pc(body_id, Phase::Entry)?),
                )
            }
            // The body's value returns from the closure's call frame. Other
            // indexed operands (a parameter type's array length) are never run.
            Phase::Operand(n)
                if node
                    .operands
                    .iter()
                    .position(|operand| *operand == body_id)
                    .and_then(|position| u16::try_from(position).ok())
                    == Some(n) =>
            {
                draft(
                    index,
                    key,
                    Vec::new(),
                    RecipeExit::Control(ControlRecipe::Return {
                        value: index.nodes()[body_id.0 as usize]
                            .value
                            .ok_or_else(|| error("closure body value absent"))?,
                    }),
                )
            }
            _ => unsupported("invalid closure phase"),
        }
    }
}
