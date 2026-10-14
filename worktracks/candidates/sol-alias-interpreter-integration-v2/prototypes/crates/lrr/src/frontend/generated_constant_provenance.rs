//! Readonly generated constant provenance. Never a constant query/effect permission.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::BucketKey,
        memo::Database,
        seals::{DiscoveryInput, SealDomain, SealKey},
    },
    expand::{
        generated_items::{GeneratedHeaderKind, GeneratedItems},
        generated_syntax::GeneratedSyntaxInput,
        identity::GeneratedItemKey,
        record::ExpansionRecord,
        source_declarative::SourceDeclarative,
    },
    frontend::{generated_macros::GeneratedMacroCatalog, names::NameLookup},
    source::{
        changes::{PreparedSourceView, crate_policy::PreparedCratePreludePolicy},
        manifest::{ItemKind, Namespace},
    },
    syntax::{
        ast_adapter::range,
        outline::{BodyHandle, DeclarationContext, ScopeKey},
        parse::ParsedFile,
        spans::SourceRange,
        token_wire::{Key, SpanRecord},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
use std::{collections::BTreeMap, sync::Arc};
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_INPUTS: usize = 100_000;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERATED-CONSTANT-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated constant provenance budget".into(),
    )
}
fn scalar_syntax(ty: &ast::Type) -> bool {
    match ty {
        ast::Type::PathType(_) => true,
        ast::Type::TupleType(tuple) => tuple.fields().next().is_none(),
        _ => false,
    }
}
fn charge(bytes: &mut usize, amount: usize, maximum: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= maximum)
        .ok_or_else(limit)?;
    Ok(())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GeneratedConstantPrerequisite {
    CurrentNamespace,
    AssociatedMember,
    HygienicSourceType,
    GeneratedScopePolicy,
    ArrayLengthQuery,
    ConstantQueryWorld,
    RuntimeLayout,
}
/// Its offsets belong only to the generated syntax arena. The spans remain
/// separate real token locations, contexts, editions and origin chains.
pub(crate) struct GeneratedSyntaxSite {
    range: SourceRange,
    spans: Vec<SpanRecord>,
}
impl GeneratedSyntaxSite {
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn spans(&self) -> &[SpanRecord] {
        &self.spans
    }
    fn capture(
        items: &GeneratedItems,
        range: SourceRange,
        bytes: &mut usize,
        maximum: usize,
    ) -> Result<Self, Diagnostic> {
        // Existing projection has an explicit 64-span bound, checked allocation
        // and actual span-key membership. Reserve that transient before calling.
        charge(
            bytes,
            64usize
                .checked_mul(std::mem::size_of::<&SpanRecord>())
                .ok_or_else(limit)?,
            maximum,
        )?;
        let projected = items.project_source_spans(range)?;
        if projected.is_empty() {
            return Err(fault("generated written site has no retained token origin"));
        }
        let allocated = projected
            .len()
            .checked_mul(std::mem::size_of::<SpanRecord>())
            .ok_or_else(limit)?;
        charge(bytes, allocated, maximum)?;
        for span in &projected {
            charge(bytes, span.source.path.capacity(), maximum)?;
        }
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(projected.len())
            .map_err(|_| limit())?;
        for span in projected {
            spans.push(span.clone());
        }
        Ok(Self { range, spans })
    }
    fn encode(&self, e: &mut StableEncoder) {
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.sequence(&self.spans, |e, s| e.digest(Digest(s.key().0)));
    }
}
/// Syntactic shape only. Neither an expected descriptor nor primitive authority.
pub(crate) enum GeneratedConstantForm {
    Scalar {
        written_type: GeneratedSyntaxSite,
    },
    Array {
        element: GeneratedSyntaxSite,
        length: GeneratedSyntaxSite,
    },
}
pub(crate) struct RetainedGeneratedConstant {
    owner: PreparedSourceView,
    policy: PreparedCratePreludePolicy,
    record: Arc<ExpansionRecord>,
    member: GeneratedItemKey,
    scope: ScopeKey,
    declaration: SourceRange,
    written_type: GeneratedSyntaxSite,
    initializer: GeneratedSyntaxSite,
    form: GeneratedConstantForm,
    body: BodyHandle,
    header: Digest,
    generated_definition: Digest,
    name: String,
    epoch: u64,
    bytes: usize,
}
impl RetainedGeneratedConstant {
    pub(crate) fn capture(
        owner: &SourceDeclarative,
        generated: &GeneratedMacroCatalog,
        definition: Digest,
        epoch: u64,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        let (items, scope, root) = generated.captured_owner();
        let actual = owner.macro_index().hygiene_facts().context(root.key())?;
        if actual != root || generated.cfg() != owner.module_source().captured_macro_cfg()? {
            return Err(fault(
                "generated source catalogue owner/hygiene/cfg differs",
            ));
        }
        Self::capture_items(owner, items, scope, definition, epoch, byte_limit)
    }
    fn capture_items(
        owner: &SourceDeclarative,
        items: &GeneratedItems,
        scope: &ScopeKey,
        definition: Digest,
        epoch: u64,
        maximum: usize,
    ) -> Result<Self, Diagnostic> {
        if maximum > MAX_BYTES || maximum < std::mem::size_of::<Self>() {
            return Err(limit());
        }
        let source = owner.module_source();
        source.require_declaration_syntax()?;
        items.record().key().validate()?;
        if epoch == 0
            || items.epoch() != epoch
            || items
                .record()
                .key()
                .procedural_epoch
                .is_some_and(|e| e != epoch)
        {
            return Err(fault("generated source retained/current epoch differs"));
        }
        // A nested/generated/attribute producer needs its own semantic lineage
        // join; diagnostic MacroOrigin parents are not that authority.
        let call = source
            .outline()
            .item_invocations
            .iter()
            .find(|c| c.item_input == items.record().key().invocation)
            .ok_or_else(|| fault("generated constant original invocation lineage prerequisite"))?;
        if call.attribute_input.is_some()
            || call.scope != *scope
            || scope.owner.is_some()
            || scope.crate_key != source.selection().outline.crate_key
            || !scope
                .module_path
                .starts_with(&source.selection().outline.module_path)
            || items.record().key().call_edition != source.selection().outline.parse.profile.edition
        {
            return Err(fault(
                "generated constant exact module/source/call edition prerequisite",
            ));
        }
        let context = owner.macro_index().hygiene_facts().context(Key(items
            .record()
            .key()
            .parent_context
            .0))?;
        if context.parent.is_some() || context.defining_unit.0 != scope.crate_key.0 {
            return Err(fault("generated constant original call context differs"));
        }
        let header = items
            .items()
            .iter()
            .find(|h| h.key.fingerprint() == definition && h.kind == GeneratedHeaderKind::Const)
            .ok_or_else(|| fault("generated constant structural member absent"))?;
        header.key.validate()?;
        let mut bytes = std::mem::size_of::<Self>();
        // Reserve borrowed record lease, owned source selection copies and a
        // bounded reconstruction transient before replaying pinned syntax events.
        charge(
            &mut bytes,
            source
                .selection()
                .metadata_bytes()?
                .checked_mul(2)
                .ok_or_else(limit)?,
            maximum,
        )?;
        charge(&mut bytes, scope.heap_bytes(), maximum)?;
        charge(
            &mut bytes,
            header
                .key
                .structural_path
                .len()
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(limit)?,
            maximum,
        )?;
        charge(&mut bytes, items.metadata_bytes()?, maximum)?;
        // Conservative peak reservation for syntax-event reconstruction and
        // its derived structural outline, before either allocation occurs.
        let transient = items
            .text()
            .len()
            .checked_mul(128)
            .and_then(|n| n.checked_add(items.syntax_events().len().checked_mul(128)?))
            .and_then(|n| n.checked_add(items.items().len().checked_mul(4096)?))
            .ok_or_else(limit)?;
        charge(&mut bytes, transient, maximum)?;
        let residual = maximum.checked_sub(bytes).ok_or_else(limit)?;
        let policy = PreparedCratePreludePolicy::capture_current(source, epoch, residual)?;
        charge(&mut bytes, policy.metadata_bytes(), maximum)?;
        let parsed = ParsedFile::from_generated_items(items)?;
        let outline = parsed.outline(
            scope.crate_key,
            scope.module_path.clone(),
            source.captured_macro_cfg()?,
        )?;
        if parsed
            .estimated_bytes()
            .checked_add(outline.estimated_bytes())
            .is_none_or(|n| n > transient)
        {
            return Err(limit());
        }
        let item = outline
            .items
            .iter()
            .find(|i| {
                i.def_key.item_kind == ItemKind::Constant
                    && i.declaration_span.start == header.start
                    && i.declaration_span.end == header.end
            })
            .ok_or_else(|| fault("generated constant exact declaration boundary absent"))?;
        if item.scope.crate_key != scope.crate_key
            || !item.scope.module_path.starts_with(&scope.module_path)
        {
            return Err(fault("generated constant escaped captured owner"));
        }
        let node = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Const::cast)
            .find(|c| range(c.syntax().text_range()) == item.declaration_span)
            .ok_or_else(|| fault("generated constant owned syntax boundary absent"))?;
        let ty = node
            .ty()
            .ok_or_else(|| fault("generated constant written type prerequisite"))?;
        let expression = node
            .body()
            .ok_or_else(|| fault("generated constant initializer absent"))?;
        let written_type = GeneratedSyntaxSite::capture(
            items,
            range(ty.syntax().text_range()),
            &mut bytes,
            maximum,
        )?;
        let initializer = GeneratedSyntaxSite::capture(
            items,
            range(expression.syntax().text_range()),
            &mut bytes,
            maximum,
        )?;
        let form = match ty {
            ast::Type::ArrayType(a) => {
                let element = a
                    .ty()
                    .ok_or_else(|| fault("generated array element type absent"))?;
                if !scalar_syntax(&element) {
                    return Err(fault(
                        "generated array source element normalization prerequisite",
                    ));
                }
                let length = a
                    .expr()
                    .ok_or_else(|| fault("generated array exact written length absent"))?;
                GeneratedConstantForm::Array {
                    element: GeneratedSyntaxSite::capture(
                        items,
                        range(element.syntax().text_range()),
                        &mut bytes,
                        maximum,
                    )?,
                    length: GeneratedSyntaxSite::capture(
                        items,
                        range(length.syntax().text_range()),
                        &mut bytes,
                        maximum,
                    )?,
                }
            }
            scalar if scalar_syntax(&scalar) => GeneratedConstantForm::Scalar {
                written_type: GeneratedSyntaxSite::capture(
                    items,
                    written_type.range,
                    &mut bytes,
                    maximum,
                )?,
            },
            _ => {
                return Err(fault(
                    "generated scalar source type normalization prerequisite",
                ));
            }
        };
        let mut body = item
            .body
            .clone()
            .ok_or_else(|| fault("generated constant body identity absent"))?;
        body.require_entry()?;
        if body.range != initializer.range
            || !item.declaration_span.contains(written_type.range)
            || !item.declaration_span.contains(initializer.range)
        {
            return Err(fault(
                "generated constant written type/initializer/body source extent differs",
            ));
        }
        body.def_key = definition;
        let name = node
            .name()
            .ok_or_else(|| fault("generated anonymous constant selection prerequisite"))?
            .text()
            .to_string();
        charge(&mut bytes, name.capacity(), maximum)?;
        charge(&mut bytes, item.scope.heap_bytes(), maximum)?;
        Ok(Self {
            owner: source.share(),
            policy,
            record: items.record_input(),
            member: header.key.clone(),
            scope: item.scope.clone(),
            declaration: item.declaration_span,
            written_type,
            initializer,
            form,
            body,
            header: item.header.fingerprint(),
            generated_definition: item.def_key.fingerprint(),
            name,
            epoch,
            bytes,
        })
    }
    pub(crate) fn member(&self) -> &GeneratedItemKey {
        &self.member
    }
    pub(crate) fn owner_source(&self) -> &PreparedSourceView {
        &self.owner
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn record(&self) -> &ExpansionRecord {
        &self.record
    }
    pub(crate) fn written_type(&self) -> &GeneratedSyntaxSite {
        &self.written_type
    }
    pub(crate) fn initializer(&self) -> &GeneratedSyntaxSite {
        &self.initializer
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn form(&self) -> &GeneratedConstantForm {
        &self.form
    }
    pub(crate) fn prerequisite(&self) -> GeneratedConstantPrerequisite {
        if self.scope.owner.is_some() {
            GeneratedConstantPrerequisite::AssociatedMember
        } else {
            GeneratedConstantPrerequisite::CurrentNamespace
        }
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn revalidate(
        &self,
        owner: &SourceDeclarative,
        generated: &GeneratedMacroCatalog,
        epoch: u64,
    ) -> Result<(), Diagnostic> {
        if !self.owner.same_publication(owner.module_source())
            || self.owner.selection() != owner.module_source().selection()
        {
            return Err(fault("generated constant source publication changed"));
        }
        if self.policy.metadata_bytes() > MAX_BYTES.checked_sub(self.bytes).ok_or_else(limit)? {
            return Err(limit());
        }
        self.policy
            .revalidate_current(owner.module_source(), epoch)?;
        let replay_budget = MAX_BYTES.checked_sub(self.bytes).ok_or_else(limit)?;
        let fresh = Self::capture(
            owner,
            generated,
            self.member.fingerprint(),
            epoch,
            replay_budget,
        )?;
        if self.identity() != fresh.identity() || !self.record.same_record(&fresh.record) {
            return Err(fault("generated constant retained plan changed"));
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/generated-constant-provenance");
        e.string(self.owner.selection().path.as_str());
        e.record(&self.owner.selection().outline);
        e.digest(self.policy.identity());
        e.record(&self.record.descriptor());
        e.record(&self.member);
        e.record(&self.scope);
        e.u32(self.declaration.start);
        e.u32(self.declaration.end);
        e.digest(self.header);
        e.digest(self.generated_definition);
        e.record(&self.body);
        self.written_type.encode(&mut e);
        self.initializer.encode(&mut e);
        match &self.form {
            GeneratedConstantForm::Scalar { written_type } => {
                e.u8(0);
                written_type.encode(&mut e);
            }
            GeneratedConstantForm::Array { element, length } => {
                e.u8(1);
                element.encode(&mut e);
                length.encode(&mut e);
            }
        }
        e.string(&self.name);
        e.u64(self.epoch);
        e.finish()
    }
}
/// Current source/namespace provenance, deliberately lacking generated source
/// Type admission, child length query, constant query universe and runtime layout.
pub(crate) struct CurrentGeneratedConstant {
    retained: RetainedGeneratedConstant,
    unit: Arc<GeneratedSyntaxInput>,
    stamps: BTreeMap<InputKey, Digest>,
    value_seal: SealKey,
    type_seal: SealKey,
    publication_revision: u64,
    bytes: usize,
}
fn stamp(
    db: &Database,
    stamps: &mut BTreeMap<InputKey, Digest>,
    key: &InputKey,
    bytes: &mut usize,
    maximum: usize,
) -> Result<(), Diagnostic> {
    let (_, fingerprint) = db
        .published_input(key)
        .ok_or_else(|| fault("generated constant current dependency absent"))?;
    if let Some(old) = stamps.get(key) {
        return if *old == fingerprint {
            Ok(())
        } else {
            Err(fault("generated constant current dependency changed"))
        };
    }
    if stamps.len() == MAX_INPUTS {
        return Err(limit());
    }
    charge(
        bytes,
        std::mem::size_of::<(InputKey, Digest)>()
            .checked_add(128)
            .and_then(|n| n.checked_add(key.heap_bytes()))
            .ok_or_else(limit)?,
        maximum,
    )?;
    stamps.insert(key.clone(), fingerprint);
    Ok(())
}
fn sealed<'a>(
    db: &'a Database,
    key: &SealKey,
    unit: &GeneratedSyntaxInput,
    retained: &RetainedGeneratedConstant,
) -> Result<&'a DiscoveryInput, Diagnostic> {
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(key.clone()))
    else {
        return Err(fault(
            "generated constant current namespace seal prerequisite",
        ));
    };
    let certified = input
        .scope
        .as_ref()
        .ok_or_else(|| fault("generated constant scoped namespace absent"))?;
    if input.seal.key() != *key
        || input.seal.unresolved_frontiers() != 0
        || !matches!(key.domain, SealDomain::Scope { id, namespace } if id == retained.scope.fingerprint() && namespace == certified.namespace)
        || certified.scope != retained.scope
        || certified.edition != retained.owner.selection().outline.parse.profile.edition
        || !input.sources.contains(retained.owner.selection())
        || input
            .records
            .iter()
            .filter(|r| r.fingerprint() == retained.record.descriptor().fingerprint())
            .count()
            != 1
        || input
            .seal
            .input_dependencies()
            .binary_search(&(
                retained.record.key().fingerprint(),
                retained.record.descriptor().fingerprint(),
            ))
            .is_err()
        || unit.seal.unit_graph != key.unit_graph
        || unit.seal.epoch != key.epoch
    {
        return Err(fault(
            "generated constant namespace/source/record/world differs",
        ));
    }
    if !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph), _)) if graph.fingerprint() == input.source_dependencies)
    {
        return Err(fault("generated constant namespace source graph changed"));
    }
    Ok(input)
}
impl CurrentGeneratedConstant {
    pub(crate) fn capture(
        db: &Database,
        owner: &SourceDeclarative,
        definition: Digest,
        maximum: usize,
    ) -> Result<Self, Diagnostic> {
        if maximum > MAX_BYTES || maximum < std::mem::size_of::<Self>() {
            return Err(limit());
        }
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(definition))
        else {
            return Err(fault("generated constant published source prerequisite"));
        };
        let Some((InputValue::GeneratedSyntax(unit), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
        else {
            return Err(fault(
                "generated constant actual generated syntax prerequisite",
            ));
        };
        let Some((InputValue::ExpansionEpoch(epoch), _)) =
            db.published_input(&InputKey::ExpansionEpoch)
        else {
            return Err(fault("generated constant current expansion epoch absent"));
        };
        if unit.key != *parse
            || unit.key.profile != owner.module_source().selection().outline.parse.profile
            || unit.seal.epoch != *epoch
            || unit.items.epoch() != *epoch
            || unit.cfg != *owner.module_source().captured_macro_cfg()?
        {
            return Err(fault("generated constant parse/cfg/epoch differs"));
        }
        let retained = RetainedGeneratedConstant::capture_items(
            owner,
            &unit.items,
            &unit.scope,
            definition,
            *epoch,
            maximum,
        )?;
        let mut bytes = retained
            .metadata_bytes()
            .checked_add(std::mem::size_of::<Self>())
            .filter(|n| *n <= maximum)
            .ok_or_else(limit)?;
        // Reserve transient context/bucket copies and lexical name lookup before
        // cloning their dynamic metadata or borrowing namespace certificates.
        charge(
            &mut bytes,
            retained
                .scope
                .heap_bytes()
                .checked_mul(2)
                .and_then(|n| n.checked_add(retained.name.capacity().checked_mul(2)?))
                .and_then(|n| n.checked_add(256))
                .ok_or_else(limit)?,
            maximum,
        )?;
        if retained.scope.owner.is_some() {
            return Err(fault(
                "generated associated constant source admission prerequisite",
            ));
        }
        if !unit.definitions.get(&definition).is_some_and(|d| {
            d.fingerprint() == retained.generated_definition
                && d.item_kind == ItemKind::Constant
                && d.crate_key == retained.scope.crate_key
                && d.module_path == retained.scope.module_path
        }) {
            return Err(fault("generated constant registered member/owner differs"));
        }
        let context = DeclarationContext {
            scope: retained.scope.clone(),
            range: retained.declaration,
        };
        if !matches!(db.published_input(&InputKey::DeclarationContext(definition)), Some((InputValue::DeclarationContext(actual), _)) if actual == &context)
            || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)), Some((InputValue::DefinitionHeader(actual), _)) if actual.fingerprint() == retained.header)
            || !matches!(db.published_input(&InputKey::DefinitionBody(definition)), Some((InputValue::DefinitionBody(actual), _)) if actual == &retained.body)
            || !matches!(db.published_input(&InputKey::ExpansionRecord(retained.record.key().fingerprint())), Some((InputValue::ExpansionRecord(actual), _)) if actual.same_record(&retained.record))
        {
            return Err(fault(
                "generated constant published member/header/body/record differs",
            ));
        }
        // The unit's actual publication seal must remain present as well as the
        // member's scoped Value/Type pair; caller supplies no seal/world/list.
        if !matches!(db.published_input(&InputKey::DiscoverySeal(unit.seal.clone())), Some((InputValue::DiscoverySeal(actual), _)) if actual.seal.key() == unit.seal)
        {
            return Err(fault("generated constant syntax publication seal absent"));
        }
        let value_seal = SealKey {
            domain: SealDomain::Scope {
                id: retained.scope.fingerprint(),
                namespace: Namespace::Value,
            },
            unit_graph: unit.seal.unit_graph,
            epoch: *epoch,
        };
        let type_seal = SealKey {
            domain: SealDomain::Scope {
                id: retained.scope.fingerprint(),
                namespace: Namespace::Type,
            },
            unit_graph: unit.seal.unit_graph,
            epoch: *epoch,
        };
        // This existing pure accessor also requires coordinator ownership and
        // verifies actual epoch/compiler/graph inputs; it runs no provider.
        let certificate =
            db.source_view_scope_certificate(retained.owner.selection(), &retained.scope)?;
        if !matches!(certificate.as_ref(), InputValue::DiscoverySeal(actual) if actual.seal.key() == value_seal)
        {
            return Err(fault(
                "generated constant selected current Value certificate differs",
            ));
        }
        let value_input = sealed(db, &value_seal, unit, &retained)?;
        let type_input = sealed(db, &type_seal, unit, &retained)?;
        if value_input.sources != type_input.sources
            || (value_input.records.len() != type_input.records.len()
                || !value_input
                    .records
                    .iter()
                    .zip(&type_input.records)
                    .all(|(a, b)| a.fingerprint() == b.fingerprint()))
            || value_input.source_dependencies != type_input.source_dependencies
        {
            return Err(fault("generated constant Type/Value source sets differ"));
        }
        let bucket = BucketKey {
            scope: retained.scope.clone(),
            namespace: Namespace::Value,
            name: retained.name.clone(),
        };
        if !matches!(value_input.scope.as_ref().ok_or_else(|| fault("generated Value scope absent"))?.lookup(&bucket)?, NameLookup::Defined(found) if found.namespace == Namespace::Value && found.identity == definition && found.defining_unit == retained.scope.crate_key && found.definition_edition == retained.record.key().call_edition)
        {
            return Err(fault(
                "generated constant name does not select exact member",
            ));
        }
        let mut stamps = BTreeMap::new();
        for key in [
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DefinitionBody(definition),
            InputKey::DeclarationContext(definition),
            InputKey::GeneratedSyntax(parse.content),
            InputKey::ExpansionEpoch,
            InputKey::SourceDependencies,
            InputKey::DiscoverySeal(unit.seal.clone()),
            InputKey::DiscoverySeal(value_seal.clone()),
            InputKey::DiscoverySeal(type_seal.clone()),
        ] {
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        for (key, expected) in retained.owner.prepared_inputs() {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != expected.fingerprint())
            {
                return Err(fault(
                    "generated constant owning source/compile publication changed",
                ));
            }
            stamp(db, &mut stamps, key, &mut bytes, maximum)?;
        }
        for input in [value_input, type_input] {
            if input.compiler_inputs.len() > MAX_INPUTS
                || input.records.len() > MAX_INPUTS
                || input.sources.len() > MAX_INPUTS
            {
                return Err(limit());
            }
            for (key, expected) in &input.compiler_inputs {
                if db
                    .published_input(key)
                    .is_none_or(|(_, actual)| actual != *expected)
                {
                    return Err(fault("generated constant namespace compiler input changed"));
                }
                stamp(db, &mut stamps, key, &mut bytes, maximum)?;
            }
            for selected in &input.sources {
                let captured = InputKey::CapturedSource(selected.location_identity());
                if !matches!(db.published_input(&captured), Some((InputValue::CapturedSource(actual), _)) if actual == selected)
                {
                    return Err(fault(
                        "generated constant complete selected source membership changed",
                    ));
                }
                let blob = InputKey::SourceBlob(selected.outline.parse.content);
                if !matches!(db.published_input(&blob), Some((InputValue::SourceBlob(actual), _)) if actual.content() == selected.outline.parse.content)
                {
                    return Err(fault("generated constant selected source content absent"));
                }
                let cfg = InputKey::CrateCfg(selected.outline.crate_key);
                if !matches!(db.published_input(&cfg), Some((InputValue::CrateCfg(actual), _)) if actual.fingerprint() == selected.outline.cfg)
                {
                    return Err(fault("generated constant selected source cfg differs"));
                }
                for key in [&captured, &blob, &cfg] {
                    stamp(db, &mut stamps, key, &mut bytes, maximum)?;
                }
            }
            for descriptor in &input.records {
                let key = InputKey::ExpansionRecord(descriptor.key.fingerprint());
                if !matches!(db.published_input(&key), Some((InputValue::ExpansionRecord(actual), _)) if actual.descriptor().fingerprint() == descriptor.fingerprint())
                {
                    return Err(fault("generated constant retained source record changed"));
                }
                stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
            }
        }
        Ok(Self {
            retained,
            unit: unit.clone(),
            stamps,
            value_seal,
            type_seal,
            publication_revision: db.revision(),
            bytes,
        })
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn seal_keys(&self) -> (&SealKey, &SealKey) {
        (&self.value_seal, &self.type_seal)
    }
    pub(crate) fn retained(&self) -> &RetainedGeneratedConstant {
        &self.retained
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn prerequisites(&self) -> &'static [GeneratedConstantPrerequisite] {
        // A new inline module is not the physical original caller. Retaining
        // the whole output does not admit its module-local prelude/attribute policy.
        let generated_scope = self.retained.scope.module_path != self.unit.scope.module_path;
        match (&self.retained.form, generated_scope) {
            (GeneratedConstantForm::Scalar { .. }, false) => &[
                GeneratedConstantPrerequisite::HygienicSourceType,
                GeneratedConstantPrerequisite::ConstantQueryWorld,
                GeneratedConstantPrerequisite::RuntimeLayout,
            ],
            (GeneratedConstantForm::Array { .. }, false) => &[
                GeneratedConstantPrerequisite::HygienicSourceType,
                GeneratedConstantPrerequisite::ArrayLengthQuery,
                GeneratedConstantPrerequisite::ConstantQueryWorld,
                GeneratedConstantPrerequisite::RuntimeLayout,
            ],
            (GeneratedConstantForm::Scalar { .. }, true) => &[
                GeneratedConstantPrerequisite::GeneratedScopePolicy,
                GeneratedConstantPrerequisite::HygienicSourceType,
                GeneratedConstantPrerequisite::ConstantQueryWorld,
                GeneratedConstantPrerequisite::RuntimeLayout,
            ],
            (GeneratedConstantForm::Array { .. }, true) => &[
                GeneratedConstantPrerequisite::GeneratedScopePolicy,
                GeneratedConstantPrerequisite::HygienicSourceType,
                GeneratedConstantPrerequisite::ArrayLengthQuery,
                GeneratedConstantPrerequisite::ConstantQueryWorld,
                GeneratedConstantPrerequisite::RuntimeLayout,
            ],
        }
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        owner: &SourceDeclarative,
    ) -> Result<(), Diagnostic> {
        if db.revision() != self.publication_revision
            || !self.retained.owner.same_publication(owner.module_source())
        {
            return Err(fault(
                "generated constant publication barrier changed; remint required",
            ));
        }
        for (key, expected) in &self.stamps {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != *expected)
            {
                return Err(fault("generated constant input dependency changed"));
            }
        }
        if self.retained.policy.metadata_bytes()
            > MAX_BYTES.checked_sub(self.bytes).ok_or_else(limit)?
        {
            return Err(limit());
        }
        self.retained
            .policy
            .revalidate_current(owner.module_source(), self.retained.epoch)?;
        let fresh = Self::capture(
            db,
            owner,
            self.retained.member.fingerprint(),
            MAX_BYTES.checked_sub(self.bytes).ok_or_else(limit)?,
        )?;
        if !self.unit.same_unit(&fresh.unit) || self.identity() != fresh.identity() {
            return Err(fault("generated constant current source proof changed"));
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/current-generated-constant-provenance");
        e.digest(self.retained.identity());
        e.record(&self.value_seal);
        e.record(&self.type_seal);
        e.u64(self.stamps.len() as u64);
        for (key, fingerprint) in &self.stamps {
            e.record(key);
            e.digest(*fingerprint);
        }
        e.finish()
    }
}
#[cfg(test)]
#[path = "generated_constant_provenance_tests.rs"]
mod tests;
