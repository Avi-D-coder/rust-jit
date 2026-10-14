//! Pinned-AST cfg preprocessing. Returns owned facts; never schedules an effect.
use super::{
    ast_adapter::{TokenRecipe, range, tokens},
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::snapshot::CfgSet,
};
use ra_ap_syntax::{
    AstNode, TextRange, TextSize,
    ast::{self, HasAttrs, HasModuleItem, HasName},
};
use std::collections::BTreeMap;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-MODULE", message.into())
}
/// Pure expression-list cfg selection. Enabled non-cfg attributes remain on the
/// original expression for reached lowering; selection grants no macro authority.
pub(crate) fn expression_enabled(expression: &ast::Expr, cfg: &CfgSet) -> Result<bool, Diagnostic> {
    let mut metas = Vec::new();
    for attribute in expression.syntax().children().filter_map(ast::Attr::cast) {
        if metas.len() >= 100_000 {
            return Err(fault("expression attribute inventory limit"));
        }
        let meta = attribute
            .meta()
            .ok_or_else(|| fault("expression attribute syntax absent"))?;
        metas
            .try_reserve(1)
            .map_err(|_| fault("expression attribute allocation"))?;
        metas.push(meta);
    }
    effective_metas(metas, cfg).map(|(enabled, _)| enabled)
}
/// Evaluate source-file inner cfg before enumerating any declaration or module.
/// This is pure pinned syntax preprocessing, with no authority or source effect.
pub(crate) fn source_root_enabled(
    tree: &ast::SourceFile,
    cfg: &CfgSet,
) -> Result<bool, Diagnostic> {
    let mut metas = Vec::new();
    for (index, attribute) in tree.attrs().enumerate() {
        if index >= 100_000 {
            return Err(fault("source root attribute inventory limit"));
        }
        if let Some(meta) = attribute.meta() {
            metas
                .try_reserve(1)
                .map_err(|_| fault("source root attribute allocation"))?;
            metas.push(meta);
        }
    }
    effective_metas(metas, cfg).map(|(enabled, _)| enabled)
}

pub(crate) fn effective_metas(
    metas: Vec<ast::Meta>,
    cfg: &CfgSet,
) -> Result<(bool, Vec<ast::Meta>), Diagnostic> {
    let mut pending: Vec<_> = metas.into_iter().rev().map(|m| (m, 0usize)).collect();
    let mut result = Vec::new();
    let mut count = 0usize;
    while let Some((meta, depth)) = pending.pop() {
        count = count
            .checked_add(1)
            .ok_or_else(|| fault("attribute inventory overflow"))?;
        if count > 100_000 || depth >= 128 {
            return Err(fault("attribute inventory/nesting limit"));
        }
        match &meta {
            ast::Meta::CfgMeta(c) => {
                let predicate = c
                    .cfg_predicate()
                    .ok_or_else(|| fault("missing cfg predicate"))?;
                if !super::modules::cfg(predicate, cfg, 0)? {
                    return Ok((false, Vec::new()));
                }
            }
            ast::Meta::CfgAttrMeta(c) => {
                let predicate = c
                    .cfg_predicate()
                    .ok_or_else(|| fault("missing cfg_attr predicate"))?;
                if super::modules::cfg(predicate, cfg, 0)? {
                    let children: Vec<_> = c.metas().collect();
                    pending.extend(children.into_iter().rev().map(|m| (m, depth + 1)));
                }
                continue;
            }
            _ => {}
        }
        result.push(meta);
    }
    Ok((true, result))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeInput {
    pub range: SourceRange,
    pub enclosing_attribute_range: SourceRange,
    pub original_attribute_range: Option<SourceRange>,
    pub argument_range: Option<SourceRange>,
    pub name: String,
    pub tokens: Vec<TokenRecipe>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAttributeInputs {
    pub parse: ParseKey,
    pub item: SourceRange,
    pub cfg: Digest,
    pub enabled: bool,
    pub requires_cfg_materialization: bool,
    pub original_attribute_ranges: Vec<SourceRange>,
    pub attributes: Vec<AttributeInput>,
}
impl ParsedFile {
    /// Pure exact attachment fact used for worklist ownership. This selects no
    /// macro facet and prepares no signature, parameters, generics or body.
    pub(crate) fn attribute_targets_free_function(
        &self,
        selected: SourceRange,
    ) -> Result<bool, Diagnostic> {
        let root = self.parsed.tree();
        if selected.is_empty() || !range(root.syntax().text_range()).contains(selected) {
            return Err(fault("compiler Test attribute range outside owned syntax"));
        }
        let element = root.syntax().covering_element(TextRange::new(
            TextSize::from(selected.start),
            TextSize::from(selected.end),
        ));
        let node = element
            .clone()
            .into_node()
            .or_else(|| element.parent())
            .ok_or_else(|| fault("compiler Test attribute node absent"))?;
        let meta = node
            .ancestors()
            .take(257)
            .find_map(|node| {
                ast::Meta::cast(node).filter(|meta| range(meta.syntax().text_range()) == selected)
            })
            .ok_or_else(|| fault("compiler Test attribute is not an exact owned meta"))?;
        let attribute = meta
            .syntax()
            .ancestors()
            .take(257)
            .find_map(ast::Attr::cast)
            .ok_or_else(|| fault("compiler Test attribute attachment absent"))?;
        let eligible = attribute
            .syntax()
            .parent()
            .and_then(ast::Fn::cast)
            .is_some_and(|function| {
                function
                    .syntax()
                    .parent()
                    .is_none_or(|parent| !ast::AssocItemList::can_cast(parent.kind()))
            });
        Ok(eligible)
    }
    pub(crate) fn attribute_item_range_at(
        &self,
        selected: SourceRange,
    ) -> Result<Option<SourceRange>, Diagnostic> {
        let root = self.parsed.tree();
        if selected.start >= selected.end || !range(root.syntax().text_range()).contains(selected) {
            return Err(fault("attribute producer outside owned syntax"));
        }
        let element = root.syntax().covering_element(TextRange::new(
            TextSize::from(selected.start),
            TextSize::from(selected.end),
        ));
        let node = element
            .clone()
            .into_node()
            .or_else(|| element.parent())
            .ok_or_else(|| fault("attribute producer node absent"))?;
        Ok(node
            .ancestors()
            .take(257)
            .find_map(ast::Item::cast)
            .map(|item| range(item.syntax().text_range())))
    }
    pub fn attribute_inputs_at(
        &self,
        selected: SourceRange,
        cfg: &CfgSet,
    ) -> Result<ItemAttributeInputs, Diagnostic> {
        let root = self.parsed.tree();
        if selected.start >= selected.end || !range(root.syntax().text_range()).contains(selected) {
            return Err(fault("selected attribute item range is outside source"));
        }
        let node = root
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(selected.start),
                TextSize::from(selected.end),
            ))
            .into_node()
            .ok_or_else(|| fault("selected attribute item is not a node"))?;
        let item = node
            .ancestors()
            .take(257)
            .find_map(|n| ast::Item::cast(n).filter(|i| range(i.syntax().text_range()) == selected))
            .ok_or_else(|| fault("selected attribute range is not an exact item"))?;
        inputs_from_attrs(self.key(), selected, cfg, item.attrs().collect())
    }
}

fn inputs_from_attrs(
    parse: ParseKey,
    selected: SourceRange,
    cfg: &CfgSet,
    attrs: Vec<ast::Attr>,
) -> Result<ItemAttributeInputs, Diagnostic> {
    let (enabled, metas) = effective_metas(attrs.iter().filter_map(|a| a.meta()).collect(), cfg)?;
    let original_attribute_ranges = attrs
        .iter()
        .map(|attr| range(attr.syntax().text_range()))
        .collect();
    let requires_cfg_materialization = attrs
        .iter()
        .filter_map(|a| a.meta())
        .any(|m| matches!(m, ast::Meta::CfgMeta(_) | ast::Meta::CfgAttrMeta(_)));
    let mut attributes = Vec::new();
    let mut bytes = 0usize;
    for meta in metas {
        let ts = tokens(meta.syntax(), None);
        let name = meta
            .path()
            .map(|p| {
                tokens(p.syntax(), None)
                    .into_iter()
                    .map(|t| t.text)
                    .collect::<String>()
            })
            .or_else(|| meta.simple_name().map(|n| n.to_string()))
            .unwrap_or_default();
        let added = ts
            .iter()
            .try_fold(
                name.len() + std::mem::size_of::<AttributeInput>(),
                |sum, t| sum.checked_add(t.text.len() + std::mem::size_of::<TokenRecipe>()),
            )
            .ok_or_else(|| fault("attribute metadata overflow"))?;
        bytes = bytes
            .checked_add(added)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("attribute metadata limit"))?;
        attributes.push(AttributeInput {
            range: range(meta.syntax().text_range()),
            enclosing_attribute_range: meta
                .syntax()
                .ancestors()
                .find_map(ast::Attr::cast)
                .map(|attr| range(attr.syntax().text_range()))
                .ok_or_else(|| fault("attribute meta has no original wrapper"))?,
            original_attribute_range: meta
                .syntax()
                .ancestors()
                .find_map(ast::Attr::cast)
                .filter(|attr| {
                    attr.meta().is_some_and(|original| {
                        original.syntax().text_range() == meta.syntax().text_range()
                    })
                })
                .map(|attr| range(attr.syntax().text_range())),
            argument_range: match &meta {
                ast::Meta::TokenTreeMeta(meta) => meta
                    .token_tree()
                    .map(|tree| range(tree.syntax().text_range())),
                _ => None,
            },
            name,
            tokens: ts,
        });
    }
    Ok(ItemAttributeInputs {
        parse,
        item: selected,
        cfg: cfg.fingerprint(),
        enabled,
        requires_cfg_materialization,
        original_attribute_ranges,
        attributes,
    })
}
#[derive(Debug, Clone)]
pub struct DeriveItemInputs {
    pub item: ItemAttributeInputs,
    pub members: Vec<ItemAttributeInputs>,
}
/// Recursive cfg preprocessing facts, separate from item-local helper ownership.
pub struct DeriveCfgInputs {
    pub item: ItemAttributeInputs,
    pub descendants: Vec<ItemAttributeInputs>,
}
impl ParsedFile {
    pub fn derive_cfg_inputs_at(
        &self,
        selected: SourceRange,
        cfg: &CfgSet,
    ) -> Result<DeriveCfgInputs, Diagnostic> {
        let item = self.derive_item_inputs_at(selected, cfg)?.item;
        let mut descendants = Vec::new();
        if !item.enabled {
            return Ok(DeriveCfgInputs { item, descendants });
        }
        let node = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(|n| ast::Item::cast(n).filter(|i| range(i.syntax().text_range()) == selected))
            .ok_or_else(|| fault("derive cfg item unavailable"))?;
        let mut disabled = BTreeMap::new();
        let mut bytes = item.allocated_bytes()?;
        let mut claimed = std::collections::BTreeSet::new();
        for owner in node
            .syntax()
            .descendants()
            .filter_map(ast::AnyHasAttrs::cast)
        {
            let selected_owner = range(owner.syntax().text_range());
            if selected_owner == selected
                || disabled
                    .range(..=selected_owner.start)
                    .next_back()
                    .is_some_and(|(_, r): (&u32, &SourceRange)| r.contains(selected_owner))
            {
                continue;
            }
            let attrs = ast::attrs_including_inner(&owner)
                .filter(|a| {
                    !claimed.contains(&(
                        u32::from(a.syntax().text_range().start()),
                        u32::from(a.syntax().text_range().end()),
                    ))
                })
                .collect::<Vec<_>>();
            if !attrs
                .iter()
                .filter_map(|a| a.meta())
                .any(|m| matches!(m, ast::Meta::CfgMeta(_) | ast::Meta::CfgAttrMeta(_)))
            {
                continue;
            }
            if descendants.len() >= 100_000 {
                return Err(fault("derive cfg owner count limit"));
            }
            for attr in &attrs {
                claimed.insert((
                    u32::from(attr.syntax().text_range().start()),
                    u32::from(attr.syntax().text_range().end()),
                ));
            }
            let facts = inputs_from_attrs(self.key(), selected_owner, cfg, attrs)?;
            if !facts.enabled {
                disabled.insert(selected_owner.start, selected_owner);
            }
            bytes = bytes
                .checked_add(facts.allocated_bytes()?)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("derive cfg fact metadata limit"))?;
            descendants
                .try_reserve(1)
                .map_err(|_| fault("derive cfg fact allocation"))?;
            descendants.push(facts);
        }
        Ok(DeriveCfgInputs { item, descendants })
    }
    /// Inspect only ADT fields/variants after outer cfg enables the derive.
    /// Disabled variants prevent demands on their nested fields.
    pub fn derive_item_inputs_at(
        &self,
        selected: SourceRange,
        cfg: &CfgSet,
    ) -> Result<DeriveItemInputs, Diagnostic> {
        let item = self.attribute_inputs_at(selected, cfg)?;
        let mut members = Vec::new();
        if !item.enabled {
            return Ok(DeriveItemInputs { item, members });
        }
        let node = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(|n| ast::Item::cast(n).filter(|i| range(i.syntax().text_range()) == selected))
            .ok_or_else(|| fault("derive item unavailable"))?;
        if !matches!(
            node,
            ast::Item::Struct(_) | ast::Item::Enum(_) | ast::Item::Union(_)
        ) {
            return Err(fault("derive requires a struct, enum or union"));
        }
        let mut disabled = BTreeMap::new();
        let mut bytes = 0usize;
        for member in node
            .syntax()
            .descendants()
            .filter_map(ast::AnyHasAttrs::cast)
        {
            if ast::RecordField::cast(member.syntax().clone()).is_none()
                && ast::TupleField::cast(member.syntax().clone()).is_none()
                && ast::Variant::cast(member.syntax().clone()).is_none()
            {
                continue;
            }
            let selected_member = range(member.syntax().text_range());
            if disabled
                .range(..=selected_member.start)
                .next_back()
                .is_some_and(|(_, r): (&u32, &SourceRange)| r.contains(selected_member))
            {
                continue;
            }
            let owning_item = member.syntax().ancestors().find_map(ast::Item::cast);
            if owning_item
                .as_ref()
                .is_none_or(|i| i.syntax() != node.syntax())
            {
                continue;
            }
            if members.len() >= 100_000 {
                return Err(fault("derive member count limit"));
            }
            let facts =
                inputs_from_attrs(self.key(), selected_member, cfg, member.attrs().collect())?;
            if !facts.enabled {
                disabled.insert(selected_member.start, selected_member);
            }
            bytes = bytes
                .checked_add(facts.allocated_bytes()?)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("derive member metadata limit"))?;
            members.push(facts);
        }
        Ok(DeriveItemInputs { item, members })
    }
}
impl ItemAttributeInputs {
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let add = |a: usize, b: usize| {
            a.checked_add(b)
                .ok_or_else(|| fault("attribute allocation overflow"))
        };
        let array = |n: usize, size: usize| {
            n.checked_mul(size)
                .ok_or_else(|| fault("attribute array overflow"))
        };
        let mut bytes = add(
            std::mem::size_of::<Self>(),
            self.parse.profile.semantics.capacity(),
        )?;
        bytes = add(
            bytes,
            array(
                self.original_attribute_ranges.capacity(),
                std::mem::size_of::<SourceRange>(),
            )?,
        )?;
        bytes = add(
            bytes,
            array(
                self.attributes.capacity(),
                std::mem::size_of::<AttributeInput>(),
            )?,
        )?;
        for attr in &self.attributes {
            bytes = add(bytes, attr.name.capacity())?;
            bytes = add(
                bytes,
                array(attr.tokens.capacity(), std::mem::size_of::<TokenRecipe>())?,
            )?;
            for token in &attr.tokens {
                bytes = add(bytes, token.text.capacity())?;
            }
        }
        Ok(bytes)
    }
}

impl ParsedFile {
    /// Source-facet declaration capture for an exact annotated function. Bodies stay cold.
    pub fn source_derive_declaration_at(
        &self,
        selected: SourceRange,
        cfg: &CfgSet,
    ) -> Result<crate::expand::derive_declaration::SourceDeriveDeclaration, Diagnostic> {
        let inputs = self.attribute_inputs_at(selected, cfg)?;
        let function = self
            .parsed
            .tree()
            .items()
            .find_map(|i| match i {
                ast::Item::Fn(f) if range(f.syntax().text_range()) == selected => Some(f),
                _ => None,
            })
            .ok_or_else(|| fault("proc macro declaration is not an exact function"))?;
        let names = crate::expand::derive_declaration::from_item_inputs(&inputs)?;
        let excluded = function.body().map(|b| b.syntax().text_range());
        let header = tokens(function.syntax(), excluded);
        let mut e = crate::core::digest::StableEncoder::new("lrr/v2/proc-export-source-header");
        e.sequence(&header, |e, t| e.record(t));
        Ok(crate::expand::derive_declaration::SourceDeriveDeclaration {
            source: self.content,
            cfg: cfg.fingerprint(),
            edition: self.edition,
            names,
            header: e.finish(),
        })
    }
}

impl ParsedFile {
    pub fn source_function_range(&self, name: &str) -> Result<SourceRange, Diagnostic> {
        let mut found = None;
        for item in self.parsed.tree().items() {
            if let ast::Item::Fn(f) = item
                && f.name().is_some_and(|n| n.text() == name)
                && found.replace(range(f.syntax().text_range())).is_some()
            {
                return Err(fault("ambiguous original source function name"));
            }
        }
        found.ok_or_else(|| fault("original source function absent"))
    }
}

impl ParsedFile {
    /// Read enabled root declarations; macro/function bodies remain opaque.
    pub fn source_derive_catalog(
        &self,
        cfg: &CfgSet,
    ) -> Result<crate::expand::derive_declaration::SourceDeriveCatalog, Diagnostic> {
        let root = self.parsed.tree();
        let mut catalog = crate::expand::derive_declaration::SourceDeriveCatalog {
            declarations: Vec::new(),
            pending: Vec::new(),
        };
        let root_range = range(root.syntax().text_range());
        let crate_attrs = inputs_from_attrs(self.key(), root_range, cfg, root.attrs().collect())?;
        if !crate_attrs.enabled {
            return Ok(catalog);
        }
        let harmless = |name: &str| {
            matches!(
                name,
                "cfg"
                    | "doc"
                    | "allow"
                    | "warn"
                    | "deny"
                    | "forbid"
                    | "deprecated"
                    | "inline"
                    | "cold"
                    | "must_use"
                    | "repr"
                    | "path"
                    | "macro_use"
                    | "macro_export"
                    | "feature"
                    | "no_std"
                    | "no_implicit_prelude"
                    | "no_main"
                    | "recursion_limit"
                    | "type_length_limit"
            )
        };
        if crate_attrs.attributes.iter().any(|a| !harmless(&a.name)) {
            catalog.pending.push(root_range);
            return Ok(catalog);
        }
        let mut exports = std::collections::BTreeSet::new();
        let mut bytes = 0usize;
        for (index, item) in root.items().enumerate() {
            if index >= 100_000 {
                return Err(fault("source derive catalog item limit"));
            }
            let selected = range(item.syntax().text_range());
            let inputs = inputs_from_attrs(self.key(), selected, cfg, item.attrs().collect())?;
            if !inputs.enabled {
                continue;
            }
            let producer = matches!(item, ast::Item::MacroCall(_))
                || inputs.attributes.iter().any(|a| {
                    !harmless(&a.name)
                        && !matches!(
                            a.name.as_str(),
                            "proc_macro" | "proc_macro_attribute" | "proc_macro_derive"
                        )
                });
            if producer {
                catalog.pending.push(selected);
                continue;
            }
            if inputs
                .attributes
                .iter()
                .any(|a| a.name == "proc_macro_derive")
            {
                let declaration = self.source_derive_declaration_at(selected, cfg)?;
                if !exports.insert(declaration.names().export().to_owned()) {
                    return Err(fault("duplicate enabled source derive export"));
                }
                let storage = declaration.names().allocated_bytes()?;
                bytes = bytes
                    .checked_add(512)
                    .and_then(|n| n.checked_add(storage))
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| fault("source derive catalog metadata limit"))?;
                if catalog.declarations.len() >= 4096 {
                    return Err(fault("source derive catalog declaration limit"));
                }
                catalog.declarations.push((selected, declaration));
            }
        }
        Ok(catalog)
    }
}

impl ParsedFile {
    /// Native macro-host source facet only. Read an enabled direct root export's
    /// actual fn/name tokens; transformed or absent declarations stay unattached.
    pub(crate) fn source_procedural_definition_origin(
        &self,
        export: &str,
        form: lrr_expand_adapter::proc_client::ProcForm,
        cfg: &CfgSet,
    ) -> Result<Option<SourceRange>, Diagnostic> {
        let root = self.parsed.tree();
        if !inputs_from_attrs(
            self.key(),
            range(root.syntax().text_range()),
            cfg,
            root.attrs().collect(),
        )?
        .enabled
        {
            return Ok(None);
        }
        let mut found = None;
        for (index, item) in root.items().enumerate() {
            if index >= 100_000 {
                return Err(fault("macro-host source item inventory limit"));
            }
            let ast::Item::Fn(function) = item else {
                continue;
            };
            let Some(name) = function.name() else {
                continue;
            };
            let facts = inputs_from_attrs(
                self.key(),
                range(function.syntax().text_range()),
                cfg,
                function.attrs().collect(),
            )?;
            use lrr_expand_adapter::proc_client::ProcForm;
            let marker = match form {
                ProcForm::FunctionLike => "proc_macro",
                ProcForm::Attribute => "proc_macro_attribute",
                ProcForm::Derive => "proc_macro_derive",
            };
            if !facts.enabled || !facts.attributes.iter().any(|a| a.name == marker) {
                continue;
            }
            // An unknown transforming annotation cannot certify the original
            // declaration as the loaded export's final source definition.
            if facts.attributes.iter().any(|a| {
                !matches!(
                    a.name.as_str(),
                    "proc_macro"
                        | "proc_macro_attribute"
                        | "proc_macro_derive"
                        | "cfg"
                        | "doc"
                        | "allow"
                        | "warn"
                        | "deny"
                        | "forbid"
                        | "deprecated"
                        | "inline"
                        | "cold"
                        | "must_use"
                )
            }) {
                continue;
            }
            let exported = match form {
                ProcForm::Derive => {
                    crate::expand::derive_declaration::from_item_inputs(&facts)?.export() == export
                }
                ProcForm::FunctionLike | ProcForm::Attribute => {
                    super::ast_adapter::macro_name(name.text(), self.edition).as_deref()
                        == Some(export)
                }
            };
            if !exported {
                continue;
            }
            let keyword = function
                .fn_token()
                .ok_or_else(|| fault("macro-host function keyword absent"))?;
            let start = range(keyword.text_range()).start;
            let end = range(name.syntax().text_range()).end;
            let selected = SourceRange::checked(start, end)?;
            if self
                .errors
                .iter()
                .any(|error| error.range.start < selected.end && selected.start < error.range.end)
            {
                return Err(fault("macro-host export header has unreliable syntax"));
            }
            if found.replace(selected).is_some() {
                return Err(fault(
                    "macro-host procedural export has multiple source definitions",
                ));
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod procedural_origin_tests {
    use super::*;
    use crate::source::snapshot::SourceBlob;
    use lrr_expand_adapter::proc_client::ProcForm;

    #[test]
    fn rustc_stock_procedural_forms_preserve_exact_export_definition_headers() {
        if let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") {
            let root = std::fs::canonicalize(expected).unwrap();
            let compiled = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            assert_eq!(std::fs::canonicalize(compiled).unwrap(), root);
            assert_eq!(
                include_bytes!("attributes.rs").as_slice(),
                std::fs::read(root.join("crates/lrr/src/syntax/attributes.rs")).unwrap()
            );
            for (relative, embedded) in [
                (
                    "crates/lrr/src/frontend/procedural_exports.rs",
                    include_bytes!("../frontend/procedural_exports.rs").as_slice(),
                ),
                (
                    "crates/lrr/src/service/macro_session.rs",
                    include_bytes!("../service/macro_session.rs").as_slice(),
                ),
                (
                    "crates/lrr/src/expand/generated_call_request.rs",
                    include_bytes!("../expand/generated_call_request.rs").as_slice(),
                ),
            ] {
                assert_eq!(
                    embedded,
                    std::fs::read(root.join(relative)).unwrap(),
                    "validation embedded source differs: {relative}"
                );
            }
        }
        let original = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
        );
        let exports = [
            (ProcForm::FunctionLike, "empty", "empty"),
            (ProcForm::FunctionLike, "panic_bang", "panic_bang"),
            (ProcForm::FunctionLike, "identity", "identity"),
            (ProcForm::FunctionLike, "recollect", "recollect"),
            (ProcForm::Attribute, "empty_attr", "empty_attr"),
            (ProcForm::Attribute, "panic_attr", "panic_attr"),
            (ProcForm::Attribute, "identity_attr", "identity_attr"),
            (ProcForm::Attribute, "recollect_attr", "recollect_attr"),
            (ProcForm::Derive, "Empty", "empty_derive"),
            (ProcForm::Derive, "Panic", "panic_derive"),
            (ProcForm::Derive, "Identity", "identity_derive"),
            (ProcForm::Derive, "Recollect", "recollect_derive"),
        ];
        let cfg = CfgSet::guest_default();
        // The stock helper's source edition is 2015. Other editions and the
        // Unicode prefix are explicit external LRR source-map adaptations.
        for edition in [2015, 2018, 2021, 2024] {
            for text in [original.to_owned(), format!("// α β\n{original}")] {
                let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
                let parsed = ParsedFile::parse_macro_facet(&blob, edition).unwrap();
                for (form, export, implementation) in exports {
                    let at = parsed
                        .source_procedural_definition_origin(export, form, &cfg)
                        .unwrap()
                        .unwrap();
                    let expected = format!("fn {implementation}");
                    assert_eq!(&text[at.start as usize..at.end as usize], expected);
                    assert_eq!(
                        text.match_indices(&expected).next().unwrap().0,
                        usize::try_from(at.start).unwrap()
                    );
                    for other in [
                        ProcForm::FunctionLike,
                        ProcForm::Attribute,
                        ProcForm::Derive,
                    ] {
                        if other != form {
                            assert!(
                                parsed
                                    .source_procedural_definition_origin(export, other, &cfg)
                                    .unwrap()
                                    .is_none()
                            );
                        }
                    }
                }
                for name in ["print_helper", "deep_recollect", "missing", "panic_derive"] {
                    assert!(
                        parsed
                            .source_procedural_definition_origin(name, ProcForm::Derive, &cfg)
                            .unwrap()
                            .is_none()
                    );
                }
            }
        }
        // Authority/cfg refusal controls adapt the retained stock source outside
        // the upstream directory; these are not full compiletest fixture passes.
        for text in [
            format!("#![cfg(lrr_disabled)]\n{original}"),
            original
                .replace("#[proc_macro]", "#[cfg(lrr_disabled)]\n#[proc_macro]")
                .replace(
                    "#[proc_macro_attribute]",
                    "#[cfg(lrr_disabled)]\n#[proc_macro_attribute]",
                )
                .replace(
                    "#[proc_macro_derive(",
                    "#[cfg(lrr_disabled)]\n#[proc_macro_derive(",
                ),
            original
                .replace("#[proc_macro]", "#[unknown_transform]\n#[proc_macro]")
                .replace(
                    "#[proc_macro_attribute]",
                    "#[unknown_transform]\n#[proc_macro_attribute]",
                )
                .replace(
                    "#[proc_macro_derive(",
                    "#[unknown_transform]\n#[proc_macro_derive(",
                ),
        ] {
            let parsed =
                ParsedFile::parse_macro_facet(&SourceBlob::new(text.into_bytes()).unwrap(), 2015)
                    .unwrap();
            for (form, export, _) in exports {
                assert!(
                    parsed
                        .source_procedural_definition_origin(export, form, &cfg)
                        .unwrap()
                        .is_none()
                );
            }
        }
        let duplicate = format!("{original}\n{original}");
        let parsed =
            ParsedFile::parse_macro_facet(&SourceBlob::new(duplicate.into_bytes()).unwrap(), 2015)
                .unwrap();
        for (form, export, _) in exports {
            let error = parsed
                .source_procedural_definition_origin(export, form, &cfg)
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-SOURCE-MODULE");
            assert_eq!(
                error.message,
                "macro-host procedural export has multiple source definitions"
            );
        }
    }
}

#[cfg(test)]
mod expression_cfg_tests {
    use super::*;

    #[test]
    fn captured_expression_cfg_attr_does_not_remove_enabled_attributes_or_mutate_syntax() {
        let parsed = ast::SourceFile::parse(
            "fn main() { let a = (#[cfg_attr(flag, cfg(false))] 1, #[allow(dead_code)] 2); }",
            ra_ap_syntax::Edition::Edition2024,
        );
        assert!(parsed.errors().is_empty());
        let expressions = parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Expr::cast)
            .filter(|expression| matches!(expression, ast::Expr::Literal(_)))
            .collect::<Vec<_>>();
        assert_eq!(expressions.len(), 2);
        let before = expressions
            .iter()
            .map(|expression| expression.syntax().text().to_string())
            .collect::<Vec<_>>();
        let mut cfg = CfgSet::guest_default();
        assert!(expression_enabled(&expressions[0], &cfg).unwrap());
        cfg.atoms.insert("flag".into());
        assert!(!expression_enabled(&expressions[0], &cfg).unwrap());
        assert!(expression_enabled(&expressions[1], &cfg).unwrap());
        assert_eq!(
            before,
            expressions
                .iter()
                .map(|expression| expression.syntax().text().to_string())
                .collect::<Vec<_>>()
        );
    }
}
