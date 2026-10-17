//! Source-only primitive cast target admission. No descriptors or provider actions.
use super::{hint, statement_cfg};
use crate::syntax::{
    body_index::BodyIndex,
    imports::{ImportBudget, may_bind_name},
    parse::ParsedFile,
};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    ir::op::{HintRecipe, PrimitiveType},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-BODY-INDEX-LIMIT",
                "primitive cast source namespace work limit".into(),
            )
        })?;
    Ok(())
}
pub(super) fn resolve(
    parsed: &ParsedFile,
    index: &BodyIndex,
    ty: Option<ast::Type>,
    admission: Option<(
        &crate::ir::pc::PcKey,
        &crate::frontend::primitive_type_names::PrimitiveTypeNameAdmission,
    )>,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    let Some(ty) = ty else { return Ok(None) };
    if u32::from(ty.syntax().text_range().len()) > 32 {
        return Ok(None);
    }
    let Some(HintRecipe::Primitive(primitive)) = hint(Some(ty.clone())) else {
        return Ok(None);
    };
    if parsed.generated_origin.is_some() {
        let Some((pc, admission)) = admission else {
            return Ok(None);
        };
        admission.require(
            parsed,
            index,
            pc,
            crate::syntax::ast_adapter::range(ty.syntax().text_range()),
            primitive,
        )?;
    }
    // Unit has no name to resolve; its cast legality remains VM preflight's job.
    if primitive == PrimitiveType::Unit {
        return Ok(Some(primitive));
    }
    let wanted = MacroName::parse(ty.syntax().text().to_string().trim(), parsed.edition)?;
    let mut work = 0usize;
    let mut imports = ImportBudget::default();
    for scope in ty.syntax().ancestors() {
        charge(&mut work)?;
        for child in scope.children() {
            charge(&mut work)?;
            let Some(parameters) = ast::GenericParamList::cast(child) else {
                continue;
            };
            for parameter in parameters.generic_params() {
                charge(&mut work)?;
                if let ast::GenericParam::TypeParam(parameter) = parameter
                    && parameter.name().is_some_and(|name| {
                        MacroName::parse(name.text(), parsed.edition)
                            .is_ok_and(|name| name == wanted)
                    })
                {
                    return Ok(None);
                }
            }
        }
        let module =
            ast::ItemList::can_cast(scope.kind()) || ast::SourceFile::can_cast(scope.kind());
        if !module && !ast::StmtList::can_cast(scope.kind()) {
            continue;
        }
        for child in scope.children() {
            charge(&mut work)?;
            let Some(item) = ast::Item::cast(child) else {
                continue;
            };
            let (enabled, attributed) = statement_cfg(item.syntax(), index.cfg())?;
            if !enabled {
                continue;
            }
            if attributed || matches!(item, ast::Item::MacroCall(_)) {
                return Ok(None);
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
                return Ok(None);
            }
            if let ast::Item::Use(item) = item
                && may_bind_name(&item, &wanted, parsed.edition, &mut imports)?
            {
                return Ok(None);
            }
        }
        if module {
            break;
        }
    }
    Ok(Some(primitive))
}
