//! Shared reached Type-name authority, independent of macro identities and values.
use super::type_name_namespace::{SourceTypeOwner, require_primitive_name};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{op::PrimitiveType, pc::PcKey},
    source::manifest::BodyKey,
    syntax::{
        body_index::BodyIndex,
        outline::BodyHandle,
        parse::{ParseKey, ParsedFile},
        primitive_type_names::PrimitiveTypeNameSite,
        spans::SourceRange,
        token_wire::{Edition, Key},
    },
};

/// Opaque source proof. It supplies no recipe, runtime value or macro behavior.
pub(crate) struct PrimitiveTypeNameAdmission {
    namespace: std::sync::Arc<std::collections::BTreeMap<InputKey, Digest>>,
    declaration_rows: [Digest; 4],
    parse: ParseKey,
    body: BodyKey,
    node: u32,
    range: SourceRange,
    primitive: PrimitiveType,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
impl PrimitiveTypeNameAdmission {
    pub(crate) fn capture(
        db: &mut Database,
        parsed: &ParsedFile,
        body: &BodyHandle,
        pc: &PcKey,
        site: PrimitiveTypeNameSite,
    ) -> QueryPoll<Self> {
        let result = (|| {
            site.require(parsed, body, pc)?;
            if pc.body != body.key() {
                return Err(invalid("Type name selected body differs"));
            }
            let Some(locator) = db.input(&InputKey::DefinitionSource(pc.body.definition)) else {
                return Err(invalid("Type name source absent"));
            };
            if !matches!(locator.as_ref(), InputValue::DefinitionSource(key) if key == &parsed.key())
            {
                return Err(invalid("Type name selected parse differs"));
            }
            let Some(header) = db.input(&InputKey::DefinitionHeader(pc.body.definition)) else {
                return Err(invalid("Type name declaration header absent"));
            };
            let Some(context) = db.input(&InputKey::DeclarationContext(pc.body.definition)) else {
                return Err(invalid("Type name declaration context absent"));
            };
            let (InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context)) =
                (header.as_ref(), context.as_ref())
            else {
                return Err(invalid("Type name declaration input variants differ"));
            };
            let stamps = db.lexical_namespace_stamps()?;
            let actual_body = db
                .input(&InputKey::DefinitionBody(pc.body.definition))
                .ok_or_else(|| invalid("Type name current body input absent"))?;
            if !matches!(actual_body.as_ref(), InputValue::DefinitionBody(current) if current == body)
            {
                return Err(invalid("Type name current body input differs"));
            }
            let declaration_rows = declaration_inputs(db, pc.body.definition)?;
            let mut sources = Vec::new();
            let mut bytes = 0usize;
            for key in stamps.keys() {
                let InputKey::CapturedSource(_) = key else {
                    continue;
                };
                let Some(input) = db.input(key) else {
                    return Err(invalid("Type name captured source disappeared"));
                };
                let InputValue::CapturedSource(selection) = input.as_ref() else {
                    return Err(invalid("Type name captured source variant differs"));
                };
                let source = db
                    .prepared_source(selection)?
                    .ok_or_else(|| invalid("Type name original source lease absent"))?;
                bytes = bytes
                    .checked_add(source.prepared_metadata_bytes()?)
                    .ok_or_else(|| invalid("Type name source metadata budget"))?;
                sources
                    .try_reserve(1)
                    .map_err(|_| crate::runtime::arena::resource())?;
                sources.push(source);
            }
            let mut owners = sources.iter().filter(|source| {
                source
                    .outline()
                    .items
                    .iter()
                    .any(|item| item.def_key.fingerprint() == pc.body.definition)
            });
            let owner = owners
                .next()
                .ok_or_else(|| invalid("Type name original declaration owner absent"))?;
            if owners.next().is_some() {
                return Err(invalid("Type name original declaration owner ambiguous"));
            }
            // This scope proof covers actual unchanged original tokens. Marked
            // definition-site names need the general hygienic scope join, never
            // a spelling-based fallback to the caller's primitive namespace.
            let [origin] = site.origins() else {
                return Err(invalid("Type name requires one original token origin"));
            };
            let original = owner.selection();
            let root = crate::expand::source_hygiene::source_root(
                original.outline.crate_key,
                original.outline.parse.profile.edition,
            )?;
            let edition = match original.outline.parse.profile.edition {
                2015 => Edition::E2015,
                2018 => Edition::E2018,
                2021 => Edition::E2021,
                2024 => Edition::E2024,
                _ => return Err(invalid("Type name original edition differs")),
            };
            if origin.source.unit != Key(original.outline.crate_key.0)
                || origin.source.content != Key(original.outline.parse.content.0)
                || origin.source.path != original.path.as_str()
                || origin.context != root.context
                || origin.edition != edition
            {
                return Err(invalid(
                    "Type name requires its actual original source scope and hygiene",
                ));
            }
            let source_range = SourceRange::checked(origin.start, origin.end)?;
            let original_body = owner
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.fingerprint() == pc.body.definition)
                .and_then(|item| item.body.as_ref())
                .ok_or_else(|| invalid("Type name original body absent"))?;
            let original_parse = match db.fetch(
                &SyntaxProvider,
                QueryKey::ParseFile(original.outline.parse.clone()),
            ) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
            };
            let QueryValue::ParseFile(original_parse) = original_parse.value.as_ref() else {
                return Err(invalid("Type name original parse result differs"));
            };
            original_parse.require_original_type_name(original_body, source_range, site.name())?;
            let mut refs = Vec::new();
            refs.try_reserve_exact(sources.len())
                .map_err(|_| crate::runtime::arena::resource())?;
            refs.extend(sources.iter());
            require_primitive_name(
                db,
                SourceTypeOwner {
                    definition: pc.body.definition,
                    scope: &context.scope,
                    header,
                },
                &refs,
                &stamps,
                site.name(),
            )?;
            Ok(QueryPoll::Ready(Ok(Self {
                namespace: stamps,
                declaration_rows,
                parse: parsed.key(),
                body: pc.body.clone(),
                node: pc.node,
                range: site.range(),
                primitive: site.primitive(),
            })))
        })();
        match result {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    pub(crate) fn lower_current(
        &self,
        db: &mut Database,
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<crate::ir::recipe::VerifiedRecipe, Diagnostic> {
        if db.lexical_namespace_stamps()? != self.namespace
            || declaration_inputs(db, self.body.definition)? != self.declaration_rows
        {
            return Err(invalid("Type name admission source or namespace changed"));
        }
        parsed
            .initial_thunk_with_primitive_name(body, index, pc, self)?
            .with_namespace_witness(db)
    }
    pub(crate) fn require(
        &self,
        parsed: &ParsedFile,
        index: &BodyIndex,
        pc: &PcKey,
        range: SourceRange,
        primitive: PrimitiveType,
    ) -> Result<(), Diagnostic> {
        if self.parse != parsed.key()
            || &self.body != index.body()
            || self.body != pc.body
            || self.node != pc.node
            || self.range != range
            || self.primitive != primitive
        {
            return Err(invalid(
                "Type name admission belongs to another reached source occurrence",
            ));
        }
        Ok(())
    }
}

fn declaration_inputs(db: &mut Database, definition: Digest) -> Result<[Digest; 4], Diagnostic> {
    let mut stamps = [Digest([0; 32]); 4];
    for (slot, key) in stamps.iter_mut().zip([
        InputKey::DefinitionSource(definition),
        InputKey::DefinitionHeader(definition),
        InputKey::DefinitionBody(definition),
        InputKey::DeclarationContext(definition),
    ]) {
        *slot = db
            .input(&key)
            .ok_or_else(|| invalid("Type name declaration input absent"))?
            .fingerprint();
    }
    Ok(stamps)
}
