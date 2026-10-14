//! One explicitly selected original-source macro call. No body enumeration or expansion.
use super::{
    ast_adapter::{range, semantic_name},
    imports::ImportSegment,
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::core::diagnostic::Diagnostic;
use ra_ap_syntax::{
    AstNode, TextRange, TextSize,
    ast::{self, HasGenericArgs, HasName},
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceMacroCall {
    key: ParseKey,
    range: SourceRange,
    segments: Vec<ImportSegment>,
    absolute: bool,
    module_suffix: Vec<String>,
    executable: bool,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl SourceMacroCall {
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.key.profile.semantics.capacity())
            .ok_or_else(|| error("macro call metadata overflow"))?;
        bytes = bytes
            .checked_add(
                self.segments
                    .capacity()
                    .checked_mul(std::mem::size_of::<ImportSegment>())
                    .ok_or_else(|| error("macro segment size overflow"))?,
            )
            .and_then(|n| {
                n.checked_add(
                    self.module_suffix
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .ok_or_else(|| error("macro call metadata overflow"))?;
        for segment in &self.segments {
            if let ImportSegment::Name(name) = segment {
                bytes = bytes
                    .checked_add(name.capacity())
                    .ok_or_else(|| error("macro name metadata overflow"))?;
            }
        }
        for name in &self.module_suffix {
            bytes = bytes
                .checked_add(name.capacity())
                .ok_or_else(|| error("macro module metadata overflow"))?;
        }
        Ok(bytes)
    }
    pub fn key(&self) -> &ParseKey {
        &self.key
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub fn segments(&self) -> &[ImportSegment] {
        &self.segments
    }
    pub fn absolute(&self) -> bool {
        self.absolute
    }
    pub fn module_suffix(&self) -> &[String] {
        &self.module_suffix
    }
    pub fn requires_body_entry(&self) -> bool {
        self.executable
    }
}
impl ParsedFile {
    /// Exact captured definition label, separate from the item's leading trivia.
    /// This supplies syntax facts only, never a macro or source authority.
    pub fn macro_definition_header_at(
        &self,
        selected: SourceRange,
    ) -> Result<(SourceRange, String), Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty()
            || selected.start > selected.end
            || !range(root.syntax().text_range()).contains(selected)
        {
            return Err(error("selected macro definition range is outside source"));
        }
        let element = root.syntax().covering_element(TextRange::new(
            TextSize::from(selected.start),
            TextSize::from(selected.end),
        ));
        let node = element
            .into_node()
            .ok_or_else(|| error("selected macro definition is not a node"))?;
        let definition = node
            .ancestors()
            .take(257)
            .find_map(|node| {
                let definition = ast::MacroRules::cast(node)?;
                (range(definition.syntax().text_range()) == selected).then_some(definition)
            })
            .ok_or_else(|| error("selected range is not an exact macro definition"))?;
        if self.errors.iter().any(|diagnostic| {
            selected.contains(diagnostic.range) || diagnostic.range.contains(selected)
        }) {
            return Err(error("selected macro definition has parse errors"));
        }
        let keyword = definition
            .macro_rules_token()
            .ok_or_else(|| error("captured macro definition keyword absent"))?;
        let name = definition
            .name()
            .ok_or_else(|| error("captured macro definition name absent"))?;
        let label = SourceRange::checked(
            u32::from(keyword.text_range().start()),
            u32::from(name.syntax().text_range().end()),
        )?;
        if !selected.contains(label) || label.is_empty() {
            return Err(error("macro definition label exceeds its captured owner"));
        }
        let name = semantic_name(&name.syntax().text().to_string())
            .ok_or_else(|| error("invalid captured macro definition name"))?;
        Ok((label, name))
    }

    /// Attach a captured opening delimiter to its actual source macro call.
    /// A delimiter belonging to another token tree supplies no invocation label.
    pub(crate) fn macro_call_for_opening_at(
        &self,
        selected: SourceRange,
    ) -> Result<Option<SourceMacroCall>, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty()
            || selected.start > selected.end
            || !range(root.syntax().text_range()).contains(selected)
        {
            return Err(error("opening delimiter is outside captured source"));
        }
        let element = root.syntax().covering_element(TextRange::new(
            TextSize::from(selected.start),
            TextSize::from(selected.end),
        ));
        let node = element.into_node().or_else(|| {
            root.syntax()
                .token_at_offset(TextSize::from(selected.start))
                .right_biased()
                .and_then(|t| t.parent())
        });
        let Some(node) = node else {
            return Ok(None);
        };
        for node in node.ancestors().take(257) {
            let Some(call) = ast::MacroCall::cast(node) else {
                continue;
            };
            let Some(tree) = call.token_tree() else {
                continue;
            };
            if range(tree.syntax().text_range()).start == selected.start {
                return self
                    .macro_call_at(range(call.syntax().text_range()))
                    .map(Some);
            }
        }
        Ok(None)
    }
    pub fn macro_call_at(&self, selected: SourceRange) -> Result<SourceMacroCall, Diagnostic> {
        self.macro_call_at_using(selected, false)
    }
    pub(crate) fn generated_macro_call_at(
        &self,
        selected: SourceRange,
    ) -> Result<SourceMacroCall, Diagnostic> {
        self.macro_call_at_using(selected, true)
    }
    fn macro_call_at_using(
        &self,
        selected: SourceRange,
        definition_unit: bool,
    ) -> Result<SourceMacroCall, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty()
            || selected.start > selected.end
            || !range(root.syntax().text_range()).contains(selected)
        {
            return Err(error("selected macro call range is outside source"));
        }
        let node = root
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(selected.start),
                TextSize::from(selected.end),
            ))
            .into_node()
            .ok_or_else(|| error("selected macro call is not a node"))?;
        let call = node
            .ancestors()
            .take(257)
            .find_map(|n| {
                let call = ast::MacroCall::cast(n)?;
                (range(call.syntax().text_range()) == selected).then_some(call)
            })
            .ok_or_else(|| error("selected range is not an exact macro call"))?;
        if self
            .errors
            .iter()
            .any(|e| selected.contains(e.range) || e.range.contains(selected))
        {
            return Err(error("selected macro call has parse errors"));
        }
        let path = call
            .path()
            .ok_or_else(|| error("macro invocation path is missing"))?;
        let (segments, absolute, mut bytes) = owned_path(path, definition_unit)?;
        let mut modules = Vec::new();
        let mut executable = false;
        for (depth, parent) in call.syntax().ancestors().enumerate() {
            if depth > 256 {
                return Err(error("macro invocation ancestry depth exceeded"));
            }
            executable |= ast::Fn::can_cast(parent.kind())
                || ast::ClosureExpr::can_cast(parent.kind())
                || ast::BlockExpr::can_cast(parent.kind());
            if let Some(module) = ast::Module::cast(parent) {
                let name = module
                    .name()
                    .ok_or_else(|| error("macro invocation module name is missing"))?;
                let name = semantic_name(&name.syntax().text().to_string())
                    .ok_or_else(|| error("invalid macro invocation module name"))?;
                bytes = bytes
                    .checked_add(name.len())
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("macro invocation metadata exceeded"))?;
                modules
                    .try_reserve(1)
                    .map_err(|_| error("macro invocation module allocation failed"))?;
                modules.push(name);
            }
        }
        modules.reverse();
        Ok(SourceMacroCall {
            key: self.key(),
            range: selected,
            segments,
            absolute,
            module_suffix: modules,
            executable,
        })
    }
}

fn owned_path(
    mut path: ast::Path,
    definition_unit: bool,
) -> Result<(Vec<ImportSegment>, bool, usize), Diagnostic> {
    let mut segments = Vec::new();
    let mut bytes = 0usize;
    let absolute;
    loop {
        if segments.len() >= 256 {
            return Err(error("macro invocation path depth exceeded"));
        }
        let segment = path
            .segment()
            .ok_or_else(|| error("macro invocation path segment is missing"))?;
        if segment.generic_arg_list().is_some() {
            return Err(error("macro invocation path has generic arguments"));
        }
        let owned = match segment
            .kind()
            .ok_or_else(|| error("invalid macro invocation path"))?
        {
            ast::PathSegmentKind::Name(name) => {
                let text = name.syntax().text().to_string();
                if text == "$crate" && !definition_unit {
                    return Err(error("original macro path has no dollar-crate authority"));
                }
                bytes = bytes
                    .checked_add(text.len())
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("macro invocation path text exceeded"))?;
                ImportSegment::Name(text)
            }
            ast::PathSegmentKind::CrateKw => ImportSegment::Crate,
            ast::PathSegmentKind::SelfKw => ImportSegment::SelfModule,
            ast::PathSegmentKind::SuperKw => ImportSegment::Super,
            _ => return Err(error("macro invocation path is not a module path")),
        };
        segments
            .try_reserve(1)
            .map_err(|_| error("macro invocation path allocation failed"))?;
        segments.push(owned);
        match path.qualifier() {
            Some(parent) => path = parent,
            None => {
                absolute = segment.coloncolon_token().is_some();
                break;
            }
        }
    }
    segments.reverse();
    if !matches!(segments.last(), Some(ImportSegment::Name(_))) {
        return Err(error("macro invocation member name is missing"));
    }
    Ok((segments, absolute, bytes))
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceAttributePath {
    key: ParseKey,
    range: SourceRange,
    owner: SourceRange,
    segments: Vec<ImportSegment>,
    absolute: bool,
    module_suffix: Vec<String>,
    executable: bool,
    container: bool,
}
impl SourceAttributePath {
    pub fn key(&self) -> &ParseKey {
        &self.key
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub fn owner(&self) -> SourceRange {
        self.owner
    }
    pub fn segments(&self) -> &[ImportSegment] {
        &self.segments
    }
    pub fn absolute(&self) -> bool {
        self.absolute
    }
    pub fn module_suffix(&self) -> &[String] {
        &self.module_suffix
    }
    pub fn requires_body_entry(&self) -> bool {
        self.executable
    }
    pub fn requires_container_entry(&self) -> bool {
        self.container
    }
}
impl ParsedFile {
    /// Exact namespace path independent of item-target eligibility. Fragment
    /// root inert attributes have a scope owner, but no ast::Item owner.
    pub(crate) fn attribute_lookup_path_at(
        &self,
        selected: SourceRange,
    ) -> Result<(Vec<ImportSegment>, bool), Diagnostic> {
        let meta = self.attribute_meta_at(selected)?;
        let path = meta
            .path()
            .ok_or_else(|| error("attribute meta has no module path"))?;
        let (segments, absolute, _) = owned_path(path, false)?;
        Ok((segments, absolute))
    }
    fn attribute_meta_at(&self, selected: SourceRange) -> Result<ast::Meta, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty()
            || selected.start > selected.end
            || !range(root.syntax().text_range()).contains(selected)
        {
            return Err(error("attribute path range is outside source"));
        }
        let element = root.syntax().covering_element(TextRange::new(
            TextSize::from(selected.start),
            TextSize::from(selected.end),
        ));
        let node = element
            .as_node()
            .cloned()
            .or_else(|| element.as_token().and_then(|token| token.parent()))
            .ok_or_else(|| error("attribute path is not a node"))?;
        let meta = node
            .ancestors()
            .take(257)
            .find_map(|n| ast::Meta::cast(n).filter(|m| range(m.syntax().text_range()) == selected))
            .ok_or_else(|| error("attribute path range is not an exact meta"))?;
        if self
            .errors
            .iter()
            .any(|e| selected.contains(e.range) || e.range.contains(selected))
        {
            return Err(error("attribute meta has parse errors"));
        }
        Ok(meta)
    }
    /// Select one effective original-source meta range; never inspect item bodies.
    pub fn attribute_path_at(
        &self,
        selected: SourceRange,
    ) -> Result<SourceAttributePath, Diagnostic> {
        let meta = self.attribute_meta_at(selected)?;
        let path = meta
            .path()
            .ok_or_else(|| error("attribute meta has no module path"))?;
        let (segments, absolute, mut bytes) = owned_path(path, false)?;
        let mut modules = Vec::new();
        let mut owner_seen = false;
        let mut owner = None;
        let mut executable = false;
        let mut container = false;
        for (depth, parent) in meta.syntax().ancestors().enumerate() {
            if depth > 256 {
                return Err(error("attribute ancestry depth exceeded"));
            }
            if !owner_seen && ast::Item::can_cast(parent.kind()) {
                owner_seen = true;
                owner = Some(range(parent.text_range()));
                continue;
            }
            container |= ast::RecordField::can_cast(parent.kind())
                || ast::TupleField::can_cast(parent.kind())
                || ast::Variant::can_cast(parent.kind());
            executable |= ast::BlockExpr::can_cast(parent.kind())
                || ast::ClosureExpr::can_cast(parent.kind())
                || (owner_seen && ast::Fn::can_cast(parent.kind()));
            container |= owner_seen
                && (ast::Trait::can_cast(parent.kind()) || ast::Impl::can_cast(parent.kind()));
            if let Some(module) = ast::Module::cast(parent) {
                let name = module
                    .name()
                    .ok_or_else(|| error("attribute enclosing module name is missing"))?;
                let name = semantic_name(&name.syntax().text().to_string())
                    .ok_or_else(|| error("invalid attribute enclosing module"))?;
                bytes = bytes
                    .checked_add(name.len())
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("attribute path metadata limit"))?;
                modules
                    .try_reserve(1)
                    .map_err(|_| error("attribute module allocation failed"))?;
                modules.push(name);
            }
        }
        modules.reverse();
        Ok(SourceAttributePath {
            key: self.key(),
            range: selected,
            owner: owner.ok_or_else(|| error("attribute meta has no owning item"))?,
            segments,
            absolute,
            module_suffix: modules,
            executable,
            container,
        })
    }
}

impl ParsedFile {
    pub(crate) fn macro_token_group(
        &self,
        selected: SourceRange,
        definition: bool,
    ) -> Result<SourceRange, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty() || !range(root.syntax().text_range()).contains(selected) {
            return Err(error("macro token selection is outside captured source"));
        }
        let node = root
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(selected.start),
                TextSize::from(selected.end),
            ))
            .into_node()
            .ok_or_else(|| error("macro token selection is not a node"))?;
        for node in node.ancestors().take(257) {
            if range(node.text_range()) != selected {
                continue;
            }
            let tree = if definition {
                ast::MacroRules::cast(node).and_then(|m| m.token_tree())
            } else {
                ast::MacroCall::cast(node).and_then(|m| m.token_tree())
            };
            return tree
                .map(|tree| range(tree.syntax().text_range()))
                .ok_or_else(|| error("exact macro token group absent"));
        }
        Err(error("exact macro token selection absent"))
    }
}
