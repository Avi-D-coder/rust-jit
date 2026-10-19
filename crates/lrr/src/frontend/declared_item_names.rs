//! Actual declaration-name occurrences retained by common namespace preparation.
//! These are source facts, never a current use/name/scope certificate on their own.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        generated_items::{GeneratedHeader, GeneratedItems},
        identity::GeneratedItemKey,
    },
    frontend::{macro_names::NameBindingClass, names::NameDefinition},
    source::manifest::Namespace,
    syntax::{
        outline::ScopeKey,
        parse::ParseKey,
        spans::SourceRange,
        token_wire::{ContextRecord, Edition, Key, NameContextKey, SpanRecord},
        visibility::VisibilityScope,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct DeclaredItemName {
    source: GeneratedItemKey,
    // Direct locations in the canonical retained outline. A variant remains
    // owned by its enum header; it is never a fabricated generated item.
    member: Option<(u32, u32)>,
    parse: ParseKey,
    record: Digest,
    epoch: u64,
    scope: ScopeKey,
    definition: NameDefinition,
    name: String,
    range: SourceRange,
    span: Key,
    raw: Key,
    comparison: NameContextKey,
    class: NameBindingClass,
    visibility: VisibilityScope,
    fingerprint: Digest,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
impl DeclaredItemName {
    /// Only the enclosing admitted header plan can mint this occurrence fact.
    pub(super) fn capture(
        items: &GeneratedItems,
        header: &GeneratedHeader,
        parse: &ParseKey,
        span: &SpanRecord,
        scope: &ScopeKey,
        namespace: Namespace,
        visibility: VisibilityScope,
        owner: &ContextRecord,
    ) -> Result<Self, Diagnostic> {
        let name = header
            .name
            .as_ref()
            .ok_or_else(|| error("declared item name absent"))?;
        // Header and span were selected through the enclosing plan's exact
        // retained range/key indexes, not a whole-output scan per occurrence.
        if header.key.expansion != items.record().key().fingerprint()
            || span.key().0 != name.span.0
            || name.start < header.start
            || name.end > header.end
            || name.start >= name.end
        {
            return Err(error("declared item name/header attachment differs"));
        }
        let facts = items.hygiene_facts();
        let raw = span.context;
        let normalized = facts.item(raw)?;
        let comparison = facts.name_item(raw)?;
        if comparison != items.item_class(raw)? {
            return Err(error("declared item pinned comparison class differs"));
        }
        let owner_root = owner.key();
        if owner.parent.is_some() || owner.defining_unit.0 != scope.crate_key.0 {
            return Err(error("declared item owning root differs"));
        }
        // Physical root identity, not equality with a possibly opaque relative
        // invocation context. Pinned normal forms supply every adjustment.
        let class = if normalized == owner_root {
            NameBindingClass::Root
        } else {
            NameBindingClass::Item(comparison)
        };
        let edition = match span.edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        };
        let semantic = super::super::names::owned_name(&name.text, edition)?;
        let mut result = Self {
            source: header.key.clone(),
            member: None,
            parse: parse.clone(),
            record: items.record().descriptor().fingerprint(),
            epoch: items.epoch(),
            scope: scope.clone(),
            definition: NameDefinition {
                namespace,
                identity: header.key.fingerprint(),
                defining_unit: scope.crate_key,
                definition_edition: edition,
            },
            name: semantic.as_str().to_owned(),
            range: SourceRange {
                start: name.start,
                end: name.end,
            },
            span: Key(name.span.0),
            raw,
            comparison,
            class,
            visibility,
            fingerprint: Digest([0; 32]),
        };
        result.metadata_bytes()?;
        result.fingerprint = StableEncode::fingerprint(&result);
        Ok(result)
    }
    /// Capture a variant from the same retained enum outline used to prepare
    /// its header. The per-member span supplies its own edition and hygiene.
    pub(super) fn capture_variant(
        items: &GeneratedItems,
        parent: &Self,
        item_index: u32,
        variant_index: u32,
        variant: &crate::syntax::outline::VariantOutline,
        span: &SpanRecord,
        namespace: Namespace,
        owner: &ContextRecord,
    ) -> Result<Self, Diagnostic> {
        if parent.member.is_some()
            || parent.definition.namespace != Namespace::Type
            || parent.source.expansion != items.record().key().fingerprint()
            || parent.record != items.record().descriptor().fingerprint()
            || parent.epoch != items.epoch()
            || owner.parent.is_some()
            || owner.defining_unit.0 != parent.scope.crate_key.0
            || variant.name_range.start >= variant.name_range.end
            || !(namespace == Namespace::Type
                || (namespace == Namespace::Value && variant.has_value_constructor()))
        {
            return Err(error("declared enum member owner differs"));
        }
        let origin = items
            .origins()
            .partition_point(|o| o.end <= variant.name_range.start);
        if !items.origins().get(origin).is_some_and(|origin| {
            origin.start == variant.name_range.start
                && origin.end == variant.name_range.end
                && origin.span.0 == span.key().0
        }) {
            return Err(error("declared enum member token occurrence differs"));
        }
        let facts = items.hygiene_facts();
        let raw = span.context;
        let normalized = facts.item(raw)?;
        let comparison = facts.name_item(raw)?;
        if comparison != items.item_class(raw)? {
            return Err(error("declared enum member comparison class differs"));
        }
        let edition = match span.edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        };
        let name = super::super::names::owned_name(&variant.name, edition)?;
        let mut result = parent.clone();
        result.member = Some((item_index, variant_index));
        result.scope.module_path.push(parent.name.clone());
        result.definition.namespace = namespace;
        result.definition.identity =
            crate::syntax::outline::variant_identity(parent.definition.identity, name.as_str());
        result.definition.definition_edition = edition;
        result.name = name.as_str().to_owned();
        result.range = variant.name_range;
        result.span = span.key();
        result.raw = raw;
        result.comparison = comparison;
        result.class = if normalized == owner.key() {
            NameBindingClass::Root
        } else {
            NameBindingClass::Item(comparison)
        };
        result.metadata_bytes()?;
        result.fingerprint = StableEncode::fingerprint(&result);
        Ok(result)
    }
    pub(crate) fn enum_owner(&self) -> Option<Digest> {
        self.member.map(|_| self.source.fingerprint())
    }
    pub(crate) fn definition(&self) -> &NameDefinition {
        &self.definition
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn class(&self) -> NameBindingClass {
        self.class
    }
    pub(crate) fn comparison_context(&self) -> NameContextKey {
        self.comparison
    }
    pub(crate) fn raw_context(&self) -> Key {
        self.raw
    }
    pub(crate) fn source(&self) -> &GeneratedItemKey {
        &self.source
    }
    pub(crate) fn source_parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn record(&self) -> Digest {
        self.record
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn visibility(&self) -> &VisibilityScope {
        &self.visibility
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    /// Cold coordinator join to an actual admitted source input. The caller
    /// separately proves that this input and its seal/epoch are current in DB.
    /// No graph/parser reconstruction or full source census occurs here.
    pub(crate) fn require_generated_source(
        &self,
        unit: &crate::expand::generated_syntax::GeneratedSyntaxInput,
    ) -> Result<(), Diagnostic> {
        let definition = unit
            .definitions
            .get(&self.source.fingerprint())
            .ok_or_else(|| error("declared name canonical source member absent"))?;
        if unit.key != self.parse
            || self.source.expansion != unit.items.record().key().fingerprint()
            || self.record != unit.items.record().descriptor().fingerprint()
            || self.epoch != unit.seal.epoch
            || self.epoch != unit.items.epoch()
            || self.scope.owner.is_some()
            || self.scope.crate_key != unit.scope.crate_key
            || !self.scope.module_path.starts_with(&unit.scope.module_path)
            || definition.crate_key != self.scope.crate_key
        {
            return Err(error(
                "declared name actual source/member publication differs",
            ));
        }
        if let Some((item, variant)) = self.member {
            let outline = unit.outline()?;
            let row = outline
                .items
                .get(item as usize)
                .ok_or_else(|| error("declared enum member canonical owner absent"))?;
            let member = row
                .variants
                .get(variant as usize)
                .ok_or_else(|| error("declared enum member canonical variant absent"))?;
            let mut scope = row.scope.clone();
            scope.module_path.push(definition.declared_name.clone());
            let name = crate::syntax::ast_adapter::semantic_name(&member.name)
                .ok_or_else(|| error("declared enum member canonical name differs"))?;
            if row.def_key != *definition
                || definition.item_kind != crate::source::manifest::ItemKind::Enum
                || scope != self.scope
                || name != self.name
                || member.name_range != self.range
                || self.definition.identity
                    != crate::syntax::outline::variant_identity(self.source.fingerprint(), &name)
                || !(self.definition.namespace == Namespace::Type
                    || (self.definition.namespace == Namespace::Value
                        && member.has_value_constructor()))
            {
                return Err(error(
                    "declared enum member actual source publication differs",
                ));
            }
            return Ok(());
        }
        if self.source.fingerprint() != self.definition.identity
            || definition.module_path != self.scope.module_path
            || definition.declared_name != self.name
        {
            return Err(error("declared item actual source identity differs"));
        }
        if definition.namespace != self.definition.namespace
            && !(self.definition.namespace == Namespace::Value
                && definition.namespace == Namespace::Type
                && definition.item_kind == crate::source::manifest::ItemKind::Struct)
        {
            return Err(error("declared name actual source namespace differs"));
        }
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.name.capacity())
            .and_then(|n| n.checked_add(self.parse.profile.semantics.capacity()))
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .and_then(|n| n.checked_add(self.source.structural_path.capacity().checked_mul(4)?))
            .ok_or_else(crate::runtime::arena::resource)?;
        if let VisibilityScope::Within { module_path, .. } = &self.visibility {
            bytes = module_path
                .capacity()
                .checked_mul(std::mem::size_of::<String>())
                .and_then(|n| n.checked_add(bytes))
                .and_then(|bytes| {
                    module_path
                        .iter()
                        .try_fold(bytes, |n, p| n.checked_add(p.capacity()))
                })
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        Ok(bytes)
    }
}
impl StableEncode for DeclaredItemName {
    const DOMAIN: &'static str = "lrr/v2/declared-item-name-occurrence";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.option(&self.member, |e, (item, variant)| {
            e.u32(*item);
            e.u32(*variant);
        });
        e.record(&self.parse);
        e.digest(self.record);
        e.u64(self.epoch);
        e.record(&self.scope);
        e.record(&self.definition);
        e.string(&self.name);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.digest(Digest(self.span.0));
        e.digest(Digest(self.raw.0));
        e.digest(Digest(self.comparison.0.0));
        e.record(&self.class);
        e.record(&self.visibility);
    }
}

#[cfg(test)]
#[path = "contextual_declaration_tests.rs"]
mod tests;
