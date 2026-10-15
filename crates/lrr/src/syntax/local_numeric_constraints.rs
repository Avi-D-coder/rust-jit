//! Bounded direct-use source constraints for a reached closed numeric binding.
//! No later thunk, producer, macro expansion, guest value or descriptor is queried.
use super::super::{body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    ir::op::{HintRecipe, PrimitiveType},
};
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasGenericArgs, HasName},
};

const NON_DIRECT: &str = "closed numeric binding awaits non-direct local-use type inference";
fn frontier(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
#[derive(Default)]
struct Budget {
    work: usize,
    reads: usize,
    imports: super::super::imports::ImportBudget,
}
impl Budget {
    fn charge(&mut self) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(1)
            .filter(|n| *n <= 100_000)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-BODY-INDEX-LIMIT",
                    "local numeric constraint source work budget".into(),
                )
            })?;
        Ok(())
    }
    fn read(&mut self) -> Result<(), Diagnostic> {
        // Existing lexical resolution scans the captured statement list. Bound
        // its use separately so many repeated names cannot multiply that work.
        self.reads = self
            .reads
            .checked_add(1)
            .filter(|n| *n <= 256)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-BODY-INDEX-LIMIT",
                    "local numeric constraint binding read budget".into(),
                )
            })?;
        Ok(())
    }
}
fn path(expression: ast::Expr, budget: &mut Budget) -> Result<Option<ast::PathExpr>, Diagnostic> {
    let mut expression = expression;
    for _ in 0..256 {
        budget.charge()?;
        match expression {
            ast::Expr::ParenExpr(group) => {
                let Some(inner) = group.expr() else {
                    return Ok(None);
                };
                expression = inner;
            }
            ast::Expr::PathExpr(path) => return Ok(Some(path)),
            _ => return Ok(None),
        }
    }
    Err(Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "local numeric constraint parenthesis budget".into(),
    ))
}
fn named(path: &ast::PathExpr, wanted: &MacroName, edition: u16) -> bool {
    path.path()
        .filter(|path| path.qualifier().is_none())
        .and_then(|path| path.segment())
        .filter(|segment| {
            segment.generic_arg_list().is_none() && segment.coloncolon_token().is_none()
        })
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| {
            MacroName::parse(name.text(), edition).is_ok_and(|name| &name == wanted)
        })
}
fn family(primitive: PrimitiveType) -> Option<bool> {
    match primitive {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::Isize
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::Usize => Some(false),
        PrimitiveType::F32 | PrimitiveType::F64 => Some(true),
        _ => None,
    }
}
/// Reject unresolved symbolic annotations before interpreting a printed primitive.
// The same strict source/namespace proof used by existing direct-use inference.
// Exposed only to the entered local-context collector; unresolved source syntax
// remains a prerequisite, never a primitive spelling fallback.
pub(super) fn source_primitive_annotation(
    parsed: &ParsedFile,
    index: &BodyIndex,
    ty: ast::Type,
) -> Result<PrimitiveType, Diagnostic> {
    annotation(parsed, index, ty, &mut Budget::default())
}
fn annotation(
    parsed: &ParsedFile,
    index: &BodyIndex,
    ty: ast::Type,
    budget: &mut Budget,
) -> Result<PrimitiveType, Diagnostic> {
    let unresolved =
        || frontier("closed numeric binding annotation requires symbolic type resolution");
    let Some(HintRecipe::Primitive(primitive)) = super::hint(Some(ty.clone())) else {
        return Err(unresolved());
    };
    if family(primitive).is_none() {
        return Err(unresolved());
    }
    let wanted = MacroName::parse(&ty.syntax().text().to_string(), parsed.edition)?;
    for (depth, scope) in ty.syntax().ancestors().enumerate() {
        budget.charge()?;
        if depth >= 256 {
            return Err(Diagnostic::new(
                "LRR-E-BODY-INDEX-LIMIT",
                "local numeric constraint namespace depth budget".into(),
            ));
        }
        // Generic type parameters are symbolic, including enclosing impls.
        for parameters in scope.children().filter_map(ast::GenericParamList::cast) {
            for parameter in parameters.generic_params() {
                budget.charge()?;
                if let ast::GenericParam::TypeParam(parameter) = parameter
                    && parameter.name().is_some_and(|name| {
                        MacroName::parse(name.text(), parsed.edition)
                            .is_ok_and(|name| name == wanted)
                    })
                {
                    return Err(unresolved());
                }
            }
        }
        let module =
            ast::ItemList::can_cast(scope.kind()) || ast::SourceFile::can_cast(scope.kind());
        if !module && !ast::StmtList::can_cast(scope.kind()) {
            continue;
        }
        for item in scope.children().filter_map(ast::Item::cast) {
            budget.charge()?;
            let (enabled, attributed) = super::statement_cfg(item.syntax(), index.cfg())?;
            if !enabled {
                continue;
            }
            if attributed || matches!(item, ast::Item::MacroCall(_)) {
                // An unresolved item expansion may introduce a type name. Do
                // not expand it or inspect an attribute producer for this proof.
                return Err(unresolved());
            }
            let declared = match &item {
                ast::Item::TypeAlias(item) => item.name(),
                ast::Item::Struct(item) => item.name(),
                ast::Item::Enum(item) => item.name(),
                ast::Item::Union(item) => item.name(),
                ast::Item::Trait(item) => item.name(),
                _ => None,
            };
            if declared.is_some_and(|name| {
                MacroName::parse(name.text(), parsed.edition).is_ok_and(|name| name == wanted)
            }) {
                return Err(unresolved());
            }
            if let ast::Item::Use(item) = item
                && super::super::imports::may_bind_name(
                    &item,
                    &wanted,
                    parsed.edition,
                    &mut budget.imports,
                )?
            {
                return Err(unresolved());
            }
        }
        if module {
            break;
        }
    }
    Ok(primitive)
}
fn opaque_use(
    syntax: &SyntaxNode,
    wanted: &MacroName,
    edition: u16,
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    for node in syntax.descendants() {
        budget.charge()?;
        // Token trees cannot prove the absence of a local use. Do not parse or
        // expand a later macro just to choose an earlier numeric representation.
        if ast::MacroCall::can_cast(node.kind()) {
            return Ok(true);
        }
        if let Some(path) = ast::PathExpr::cast(node)
            && named(&path, wanted, edition)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn operator_type(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    statement: &ast::LetStmt,
    pattern: &ast::IdentPat,
    default: PrimitiveType,
) -> Result<PrimitiveType, Diagnostic> {
    let Some(list) = statement.syntax().parent().and_then(ast::StmtList::cast) else {
        return Ok(default);
    };
    let Some(name) = pattern.name() else {
        return Ok(default);
    };
    let wanted = MacroName::parse(name.text(), parsed.edition)?;
    let binding = super::id_for(index, body, pattern.syntax())?;
    let mut budget = Budget::default();
    // Do not inspect unrelated later producers or macros if no direct written
    // context even proposes constraining this exact indexed binding.
    let mut proposed = false;
    for syntax in list
        .syntax()
        .children()
        .filter(|syntax| syntax.text_range().start() >= statement.syntax().text_range().end())
    {
        budget.charge()?;
        let Some(later) = ast::LetStmt::cast(syntax) else {
            continue;
        };
        if later.ty().is_none() {
            continue;
        }
        let Some(initializer) = later.initializer() else {
            continue;
        };
        let Some(path) = path(initializer, &mut budget)? else {
            continue;
        };
        if named(&path, &wanted, parsed.edition) {
            budget.read()?;
            if super::local_binding(parsed, index, body, &path)? == Some(binding) {
                proposed = true;
                break;
            }
        }
    }
    if !proposed {
        return Ok(default);
    }
    let mut selected = None;
    let mut opaque = pattern.ref_token().is_some()
        || pattern.mut_token().is_some()
        || pattern.at_token().is_some();
    for syntax in list
        .syntax()
        .children()
        .filter(|syntax| syntax.text_range().start() >= statement.syntax().text_range().end())
    {
        budget.charge()?;
        let (enabled, attributed) = super::statement_cfg(&syntax, index.cfg())?;
        if !enabled {
            continue;
        }
        opaque |= attributed;
        if ast::Item::can_cast(syntax.kind()) {
            // Uncalled item bodies cannot capture this local. Their type/import
            // headers are checked only if a direct written annotation needs it.
            continue;
        }
        let Some(later) = ast::LetStmt::cast(syntax.clone()) else {
            opaque |= opaque_use(&syntax, &wanted, parsed.edition, &mut budget)?;
            continue;
        };
        // Even an irrefutable let-else block can contribute source constraints
        // in Rust. The direct initializer alone cannot certify its hidden uses;
        // inspect only captured syntax and refuse before running any operand.
        if later.let_else().is_some()
            && opaque_use(later.syntax(), &wanted, parsed.edition, &mut budget)?
        {
            return Err(frontier(NON_DIRECT));
        }
        match later.pat() {
            Some(ast::Pat::IdentPat(pattern))
                if pattern.at_token().is_none() && pattern.ref_token().is_none() =>
            {
                // Even a simple shadow is outside a direct-use proof if the old
                // local also has a concrete context. Its RHS still sees old x.
                opaque |= pattern.name().is_some_and(|name| {
                    MacroName::parse(name.text(), parsed.edition).is_ok_and(|name| name == wanted)
                });
            }
            Some(ast::Pat::WildcardPat(_)) => {}
            _ => opaque = true,
        }
        let Some(initializer) = later.initializer() else {
            continue;
        };
        let direct = path(initializer.clone(), &mut budget)?;
        if let Some(path) = direct
            && named(&path, &wanted, parsed.edition)
        {
            budget.read()?;
            // The existing source resolver checks lexical visibility and owning
            // indexed pattern identity. A later same-name local is not this one.
            if super::local_binding(parsed, index, body, &path)? != Some(binding) {
                continue;
            }
            if let Some(ty) = later.ty() {
                let primitive = annotation(parsed, index, ty, &mut budget)?;
                if family(primitive) != family(default)
                    || selected.is_some_and(|old| old != primitive)
                {
                    return Err(frontier(
                        "closed numeric binding has incompatible direct source constraints",
                    ));
                }
                selected = Some(primitive);
            } else if !matches!(later.pat(), Some(ast::Pat::WildcardPat(_))) {
                opaque = true;
            }
        } else {
            opaque |= opaque_use(initializer.syntax(), &wanted, parsed.edition, &mut budget)?;
        }
    }
    match selected {
        Some(_) if opaque => Err(frontier(NON_DIRECT)),
        Some(primitive) => Ok(primitive),
        // No direct context: preserve the existing closed-operator fallback.
        // This does not solve constraints through aliases, calls or macros.
        None => Ok(default),
    }
}
