//! One exact original reached path expression, never name selection or evaluation.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{
        ast_adapter::range,
        imports::ImportSegment,
        outline::{BodyHandle, DeclarationContext},
        parse::ParsedFile,
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasGenericParams},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-PATH-SOURCE", message.into())
}
#[derive(Debug, Clone)]
pub(crate) struct ConstantPathSite {
    selection: SourceSelection,
    body: BodyHandle,
    context: DeclarationContext,
    range: SourceRange,
    segments: Vec<ImportSegment>,
    absolute: bool,
}
/// Structural routing data only. Generic or other missing context is retained
/// privately; this wrapper has no API that grants strict constant-site admission.
#[derive(Debug)]
pub(crate) struct ConstantPathRoute {
    site: ConstantPathSite,
    prerequisite: Option<&'static str>,
}
impl ConstantPathRoute {
    pub(crate) fn context(&self) -> &DeclarationContext {
        self.site.context()
    }
    pub(crate) fn segments(&self) -> &[ImportSegment] {
        self.site.segments()
    }
    pub(crate) fn absolute(&self) -> bool {
        self.site.absolute()
    }
}
impl ConstantPathSite {
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.selection
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn segments(&self) -> &[ImportSegment] {
        &self.segments
    }
    pub(crate) fn absolute(&self) -> bool {
        self.absolute
    }
    pub(crate) fn member(&self) -> Result<&str, Diagnostic> {
        match self.segments.last() {
            Some(ImportSegment::Name(name)) => Ok(name),
            _ => Err(fault("constant path has no source member")),
        }
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.selection.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.context.scope.heap_bytes()))
            .and_then(|n| {
                n.checked_add(
                    self.segments
                        .capacity()
                        .checked_mul(std::mem::size_of::<ImportSegment>())?,
                )
            })
            .ok_or_else(|| fault("constant path metadata overflow"))?;
        for segment in &self.segments {
            if let ImportSegment::Name(name) = segment {
                bytes = bytes
                    .checked_add(name.capacity())
                    .filter(|n| *n <= 64 * 1024)
                    .ok_or_else(|| fault("constant path metadata limit"))?;
            }
        }
        Ok(bytes)
    }
}
impl StableEncode for ConstantPathSite {
    const DOMAIN: &'static str = "lrr/v1/original-constant-path-site";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.selection.path.as_str());
        e.digest(self.selection.location_identity());
        e.record(&self.selection.outline.parse);
        e.digest(self.selection.outline.cfg);
        e.record(&self.body);
        e.record(&self.context);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.bool(self.absolute);
        e.sequence(&self.segments, |e, s| match s {
            ImportSegment::Name(name) => {
                e.u8(0);
                e.string(name)
            }
            ImportSegment::Crate => e.u8(1),
            ImportSegment::SelfModule => e.u8(2),
            ImportSegment::Super => e.u8(3),
        });
    }
}
impl ParsedFile {
    pub(crate) fn constant_path_site(
        &self,
        view: &PreparedSourceView,
        body: &BodyHandle,
        selected: SourceRange,
    ) -> Result<ConstantPathSite, Diagnostic> {
        let route = self
            .constant_path_route(view, body, selected)?
            .ok_or_else(|| fault("generic/associated constant path requires type selection"))?;
        if let Some(prerequisite) = route.prerequisite {
            return Err(fault(prerequisite));
        }
        Ok(route.site)
    }
    pub(crate) fn constant_path_route(
        &self,
        view: &PreparedSourceView,
        body: &BodyHandle,
        selected: SourceRange,
    ) -> Result<Option<ConstantPathRoute>, Diagnostic> {
        view.require_original_hygiene()?;
        view.require_declaration_syntax()?;
        if self.key() != view.selection().outline.parse
            || selected.is_empty()
            || !body.range.contains(selected)
        {
            return Err(fault("constant path parser/body/source range differs"));
        }
        let item = view
            .outline()
            .items
            .iter()
            .find(|i| i.body.as_ref() == Some(body))
            .ok_or_else(|| fault("constant path exact caller declaration absent"))?;
        body.require_entry()?;
        let mut prerequisite = item
            .scope
            .owner
            .is_some()
            .then_some("lexical/associated caller requires namespace admission");
        let expression = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .take(100_001)
            .filter_map(ast::PathExpr::cast)
            .find(|path| range(path.syntax().text_range()) == selected)
            .ok_or_else(|| fault("selected constant site is not an exact path expression"))?;
        for ancestor in expression.syntax().ancestors().take(257) {
            let metas = ancestor
                .children()
                .filter_map(ast::Attr::cast)
                .map(|attribute| {
                    attribute
                        .meta()
                        .ok_or_else(|| fault("constant path ancestor attribute is incomplete"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (enabled, _) =
                super::attributes::effective_metas(metas, view.captured_macro_cfg()?)?;
            if !enabled {
                return Err(fault("constant path ancestor is cfg-disabled"));
            }
            if ast::ClosureExpr::can_cast(ancestor.kind())
                || ast::Impl::can_cast(ancestor.kind())
                || ast::Trait::can_cast(ancestor.kind())
            {
                prerequisite
                    .get_or_insert("constant path requires closure/associated namespace admission");
            }
            if let Some(function) = ast::Fn::cast(ancestor)
                && function.generic_param_list().is_some()
            {
                prerequisite
                    .get_or_insert("generic caller needs exact source ParamEnv/name admission");
            }
        }
        let mut path = expression
            .path()
            .ok_or_else(|| fault("constant path syntax is incomplete"))?;
        let mut segments = Vec::new();
        let absolute;
        loop {
            if segments.len() >= 256 {
                return Err(fault("constant path depth limit"));
            }
            let segment = path
                .segment()
                .ok_or_else(|| fault("constant path segment absent"))?;
            if segment.generic_arg_list().is_some() {
                return Ok(None);
            }
            let owned = match segment
                .kind()
                .ok_or_else(|| fault("constant path segment kind absent"))?
            {
                ast::PathSegmentKind::Name(reference) => {
                    let spelling = reference.text().to_string();
                    crate::expand::scopes::MacroName::parse(
                        &spelling,
                        self.token_edition(range(reference.syntax().text_range()))?,
                    )?;
                    ImportSegment::Name(spelling)
                }
                ast::PathSegmentKind::CrateKw => ImportSegment::Crate,
                ast::PathSegmentKind::SelfKw => ImportSegment::SelfModule,
                ast::PathSegmentKind::SuperKw => ImportSegment::Super,
                _ => {
                    return Ok(None);
                }
            };
            segments
                .try_reserve(1)
                .map_err(|_| fault("constant path allocation"))?;
            segments.push(owned);
            if let Some(parent) = path.qualifier() {
                path = parent;
            } else {
                absolute = segment.coloncolon_token().is_some();
                break;
            }
        }
        segments.reverse();
        let site = ConstantPathSite {
            selection: view.selection().clone(),
            body: body.clone(),
            context: DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            range: selected,
            segments,
            absolute,
        };
        site.member()?;
        site.metadata_bytes()?;
        Ok(Some(ConstantPathRoute { site, prerequisite }))
    }
}
