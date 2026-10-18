//! Written spelling of a non-local value path for the dispatcher's ordinary
//! Value-namespace selection. Pure syntax: no name lookup or type demand.
//!
//! The spelling uses the same encoding as a written callee path in a call
//! recipe (`a::b`, a leading `::`, `crate`/`self`/`super`, and `$crate@<unit>`
//! for a generated `$crate` root), so value and callee paths resolve through
//! the one `CallResolver::source_function` route. A body-local item or `use`
//! that may bind the path's first name is lexical scope the module graph does
//! not see. A single-name expression path keeps its spelling: the dispatcher
//! selects it through the entered lexical Value goal (never a same-named
//! module item, `entered_value_requires_lexical_context`). A qualified or
//! pattern path with such a root is refused here.
use super::{ParsedFile, error, statement_cfg};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    ir::pc::{NodeId, PcKey, Phase},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasGenericParams, HasName},
};

pub(super) enum WrittenValuePath {
    Spelled(String),
    Refused(&'static str),
}

impl ParsedFile {
    /// The same spelling for the common Value goal's qualified-path
    /// selection; `None` where the dispatcher refuses the path.
    pub(crate) fn written_value_spelling(
        &self,
        path: &ast::PathExpr,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<Option<String>, Diagnostic> {
        Ok(match self.written_value_path(path, cfg)? {
            WrittenValuePath::Spelled(spelled) => Some(spelled),
            WrittenValuePath::Refused(_) => None,
        })
    }
    pub(super) fn written_value_path(
        &self,
        path: &ast::PathExpr,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<WrittenValuePath, Diagnostic> {
        let Some(source_path) = path.path() else {
            return Ok(WrittenValuePath::Refused("value path absent"));
        };
        self.written_path(source_path, path.syntax(), cfg, true)
    }
    /// The same spelling for a path written in a pattern (path, tuple-struct
    /// or record pattern), so pattern paths resolve through the one route.
    pub(super) fn written_pattern_path(
        &self,
        path: ast::Path,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<WrittenValuePath, Diagnostic> {
        let anchor = path.syntax().clone();
        self.written_path(path, &anchor, cfg, false)
    }
    /// The spelling of the Value written path; `lexical_value` admits a
    /// single-name expression path that a body-local item may bind.
    fn written_path(
        &self,
        source_path: ast::Path,
        anchor: &ra_ap_syntax::SyntaxNode,
        cfg: &crate::source::snapshot::CfgSet,
        lexical_value: bool,
    ) -> Result<WrittenValuePath, Diagnostic> {
        let mut components = Vec::new();
        let mut cursor = source_path;
        let absolute;
        loop {
            let Some(part) = cursor.segment() else {
                return Ok(WrittenValuePath::Refused("value path segment absent"));
            };
            // A pattern's written type arguments (`clam::a::<isize>(_)`) have
            // no runtime effect: the variant is selected by identity against
            // the scrutinee's own reached Type. An expression's arguments are
            // explicit type arguments of its value and are never dropped.
            if part.generic_arg_list().is_some()
                && (lexical_value || !matches!(part.kind(), Some(ast::PathSegmentKind::Name(_))))
            {
                return Ok(WrittenValuePath::Refused(
                    "generic value path requires generic context integration",
                ));
            }
            let text = match part.kind() {
                Some(ast::PathSegmentKind::Name(name)) if name.text() == "$crate" => {
                    match self.dollar_crate_root(crate::syntax::ast_adapter::range(
                        name.syntax().text_range(),
                    ))? {
                        Some((unit, _)) => crate::frontend::callables::dollar_crate_segment(unit),
                        None => return Err(error("`$crate` outside generated syntax")),
                    }
                }
                Some(ast::PathSegmentKind::Name(name)) => name.text().to_string(),
                Some(ast::PathSegmentKind::CrateKw) => "crate".into(),
                Some(ast::PathSegmentKind::SelfKw) => "self".into(),
                Some(ast::PathSegmentKind::SuperKw) => "super".into(),
                // `Self::NAME` in an impl: an associated item of its self
                // type, selected by the dispatcher's member lookup.
                Some(ast::PathSegmentKind::SelfTypeKw)
                    if lexical_value && cursor.qualifier().is_none() =>
                {
                    "Self".into()
                }
                // `<Type as Trait>::NAME` / `<Type>::NAME`.
                Some(ast::PathSegmentKind::Type {
                    type_ref,
                    trait_ref,
                }) if lexical_value && cursor.qualifier().is_none() => {
                    let Some(ty) = type_ref.and_then(|ty| match ty {
                        ast::Type::PathType(path) => path.path(),
                        _ => None,
                    }) else {
                        return Ok(WrittenValuePath::Refused(
                            "qualified value path anchor requires structural type integration",
                        ));
                    };
                    let Some(ty) = self.anchor_spelling(&ty, anchor, cfg, false)? else {
                        return Ok(WrittenValuePath::Refused(
                            "qualified value path anchor requires further type integration",
                        ));
                    };
                    match trait_ref.and_then(|tr| tr.path()) {
                        Some(tr) => match self.anchor_spelling(&tr, anchor, cfg, false)? {
                            Some(tr) => format!("<{ty} as {tr}>"),
                            None => {
                                return Ok(WrittenValuePath::Refused(
                                    "qualified value path trait requires further integration",
                                ));
                            }
                        },
                        None => format!("<{ty}>"),
                    }
                }
                _ => {
                    return Ok(WrittenValuePath::Refused(
                        "associated value path requires associated-item integration",
                    ));
                }
            };
            components.push(text);
            if let Some(parent) = cursor.qualifier() {
                cursor = parent;
            } else {
                absolute = part.coloncolon_token().is_some();
                break;
            }
        }
        components.reverse();
        if (components[0] == "Self" || components[0].starts_with('<')) && components.len() != 2 {
            return Ok(WrittenValuePath::Refused(
                "associated value path requires associated-item integration",
            ));
        }
        let relative = !absolute
            && !matches!(components[0].as_str(), "crate" | "self" | "super" | "Self")
            && !components[0].starts_with("$crate@")
            && !components[0].starts_with('<');
        if relative {
            let root = MacroName::parse(&components[0], self.edition)?;
            if components.len() > 2
                && (self.lexical_item_may_bind(anchor, &root, cfg)?
                    || self.enclosing_import_may_bind(anchor, &components[0], cfg)?)
            {
                return Ok(WrittenValuePath::Refused(
                    "lexical item value path requires body namespace integration",
                ));
            }
            if components.len() > 1 && generic_parameter_named(anchor, &root, self.edition) {
                return Ok(WrittenValuePath::Refused(
                    "generic source owner requires associated-item integration",
                ));
            }
        }
        let mut spelled = components.join("::");
        if absolute {
            spelled.insert_str(0, "::");
        }
        Ok(WrittenValuePath::Spelled(spelled))
    }
    /// The plain module-path spelling of a qualified anchor's type or trait
    /// (`m::Type`, `Self`); `None` for generic or otherwise structural
    /// spellings. A lexically bound root is refused like any value path's.
    /// The spelling of a type or trait path in a `<Type as Trait>` anchor.
    /// A call path admits an enclosing generic parameter (`<T as Trait>`):
    /// the dispatcher reads its reached binding before any module type.
    pub(super) fn anchor_spelling(
        &self,
        path: &ast::Path,
        anchor: &ra_ap_syntax::SyntaxNode,
        cfg: &crate::source::snapshot::CfgSet,
        generic: bool,
    ) -> Result<Option<String>, Diagnostic> {
        let mut parts = Vec::new();
        let mut cursor = Some(path.clone());
        let mut absolute = false;
        while let Some(current) = cursor {
            let Some(segment) = current.segment() else {
                return Ok(None);
            };
            if segment.generic_arg_list().is_some() {
                return Ok(None);
            }
            parts.push(match segment.kind() {
                Some(ast::PathSegmentKind::Name(name)) if name.text() != "$crate" => {
                    name.text().to_string()
                }
                Some(ast::PathSegmentKind::CrateKw) => "crate".into(),
                Some(ast::PathSegmentKind::SelfKw) => "self".into(),
                Some(ast::PathSegmentKind::SuperKw) => "super".into(),
                Some(ast::PathSegmentKind::SelfTypeKw) => "Self".into(),
                _ => return Ok(None),
            });
            cursor = current.qualifier();
            if cursor.is_none() {
                absolute = segment.coloncolon_token().is_some();
            }
        }
        parts.reverse();
        if parts.iter().skip(1).any(|part| part == "Self")
            || parts[0] == "Self" && parts.len() != 1
        {
            return Ok(None);
        }
        if !absolute && !matches!(parts[0].as_str(), "crate" | "self" | "super" | "Self") {
            let root = MacroName::parse(&parts[0], self.edition)?;
            if self.lexical_item_may_bind(anchor, &root, cfg)?
                || self.enclosing_import_may_bind(anchor, &parts[0], cfg)?
                || !generic && generic_parameter_named(anchor, &root, self.edition)
            {
                return Ok(None);
            }
        }
        let mut spelled = parts.join("::");
        if absolute {
            spelled.insert_str(0, "::");
        }
        Ok(Some(spelled))
    }
    /// Dispatcher probe for the `ValuePath` at `key`: whether a body-local
    /// item or `use` may bind its single written name. Such a path is
    /// selected only through the entered lexical Value goal; it must never
    /// fall back to a same-named module item.
    pub(crate) fn entered_value_requires_lexical_context(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<bool, Diagnostic> {
        if body.key() != *index.body() || key.body != body.key() {
            return Err(error("lexical value dispatch body identity differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("lexical value dispatch node absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Path)
            || index.pc(NodeId(key.node), Phase::from_code(key.phase))? != *key
        {
            return Err(error("lexical value dispatch requires an exact path entry"));
        }
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| error("lexical value dispatch source start overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| error("lexical value dispatch source end overflow"))?;
        let root = self.entered_body_root(body)?;
        let path = root
            .covering_element(ra_ap_syntax::TextRange::new(start.into(), end.into()))
            .ancestors()
            .find_map(|syntax| {
                let path = ast::PathExpr::cast(syntax)?;
                let range = path.syntax().text_range();
                (u32::from(range.start()) == start && u32::from(range.end()) == end)
                    .then_some(path)
            })
            .ok_or_else(|| error("lexical value dispatch exact path syntax absent"))?;
        let Some(written) = path.path() else {
            return Ok(false);
        };
        // A single name, or the root of `Root::member`: the entered lexical
        // Value goal decides whether a block item or import really binds it.
        let Some((name, _)) = Self::lexical_root_name(&written) else {
            return Ok(false);
        };
        let wanted = MacroName::parse(name.text(), self.edition)?;
        Ok(self.lexical_item_may_bind(path.syntax(), &wanted, index.cfg())?
            || self.enclosing_import_may_bind(path.syntax(), name.text(), index.cfg())?)
    }
    /// The name a block item or block-level `use` could bind in a written
    /// path: its only segment, or the first of exactly two (`Root::member`).
    /// Both must be plain identifiers without generic arguments and the path
    /// must not be absolute; any other shape is the module route's.
    pub(crate) fn lexical_root_name(
        path: &ast::Path,
    ) -> Option<(ast::NameRef, Option<ast::NameRef>)> {
        let plain = |segment: ast::PathSegment| {
            if segment.generic_arg_list().is_some() || segment.coloncolon_token().is_some() {
                return None;
            }
            // A path keyword (`Self`, `self`, `super`, `crate`, `$crate`) is
            // never the name of a block item or import.
            segment.name_ref().filter(|name| {
                let text = name.text();
                !matches!(&*text, "$crate" | "Self" | "self" | "super" | "crate")
            })
        };
        match path.qualifier() {
            None => path.segment().and_then(plain).map(|name| (name, None)),
            Some(root) => {
                if root.qualifier().is_some() {
                    return None;
                }
                let root = root.segment().and_then(plain)?;
                // The member segment may carry its own generic arguments
                // (`mem::size_of::<T>`); they belong to the member, not the root.
                let member = path.segment().and_then(|segment| segment.name_ref())?;
                Some((root, Some(member)))
            }
        }
    }
    /// Whether a block item or block-level `use` may bind the single name (or
    /// the root of a two-segment path) of a path, tuple-struct or record
    /// pattern's path. Such a pattern name is selected through the entered
    /// lexical Value goal of the pattern itself.
    pub(super) fn pattern_path_binder(
        &self,
        path: &ast::Path,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<bool, Diagnostic> {
        let Some((name, _)) = Self::lexical_root_name(path) else {
            return Ok(false);
        };
        let wanted = MacroName::parse(name.text(), self.edition)?;
        Ok(self.lexical_item_may_bind(path.syntax(), &wanted, cfg)?
            || self.enclosing_import_may_bind(path.syntax(), name.text(), cfg)?)
    }
    /// Whether a block item or block-level `use` of an enclosing statement
    /// list may bind the single name (or the root of a two-segment path) of
    /// this callee path. Such a
    /// callee is selected through the entered lexical Value goal, never by a
    /// module-graph lookup of its spelling.
    pub(super) fn lexical_binder_may_bind(
        &self,
        path: &ast::PathExpr,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<bool, Diagnostic> {
        let Some(written) = path.path() else {
            return Ok(false);
        };
        let Some((name, _)) = Self::lexical_root_name(&written) else {
            return Ok(false);
        };
        let wanted = MacroName::parse(name.text(), self.edition)?;
        Ok(self.lexical_item_may_bind(path.syntax(), &wanted, cfg)?
            || self.enclosing_import_may_bind(path.syntax(), name.text(), cfg)?)
    }
    /// Any enabled body-local item of the same name in an enclosing statement
    /// list. Its namespace is irrelevant: the module graph cannot see it.
    fn lexical_item_may_bind(
        &self,
        anchor: &ra_ap_syntax::SyntaxNode,
        wanted: &MacroName,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<bool, Diagnostic> {
        for list in anchor.ancestors().filter_map(ast::StmtList::cast) {
            for statement in list.statements() {
                let ast::Stmt::Item(item) = statement else {
                    continue;
                };
                if !statement_cfg(item.syntax(), cfg)?.0 {
                    continue;
                }
                let name = match &item {
                    ast::Item::Fn(item) => item.name(),
                    ast::Item::Const(item) => item.name(),
                    ast::Item::Static(item) => item.name(),
                    ast::Item::Struct(item) => item.name(),
                    ast::Item::Enum(item) => item.name(),
                    ast::Item::Union(item) => item.name(),
                    ast::Item::Module(item) => item.name(),
                    ast::Item::TypeAlias(item) => item.name(),
                    ast::Item::Trait(item) => item.name(),
                    _ => None,
                };
                if name.is_some_and(|name| {
                    MacroName::parse(name.text(), self.edition).is_ok_and(|name| &name == wanted)
                }) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

fn generic_parameter_named(
    anchor: &ra_ap_syntax::SyntaxNode,
    wanted: &MacroName,
    edition: u16,
) -> bool {
    anchor
        .ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|function| function.generic_param_list())
        .is_some_and(|parameters| {
            parameters.generic_params().any(|parameter| {
                matches!(parameter, ast::GenericParam::TypeParam(parameter)
                    if parameter.name().is_some_and(|name|
                        MacroName::parse(name.text(), edition).is_ok_and(|name| &name == wanted)))
            })
        })
}
