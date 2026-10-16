//! Current generated primitive-name admission from exact source syntax.
use super::*;
use crate::{
    source::manifest::Namespace,
    syntax::{
        closed_reference_signatures::{ClosedReferenceSyntax, ClosedReferenceTypeSyntax},
        parse::ParseKey,
        signatures::FunctionSignature,
    },
};

pub(super) fn require(
    db: &Database,
    signature: &FunctionSignature,
    parse: &ParseKey,
) -> Result<(), Diagnostic> {
    if signature.result.len() != 1 || signature.result_symbolic.is_some() {
        return Err(fault(
            "LRR-E-CALL-CONTEXT",
            "primitive result requires its exact current generated Type namespace",
        ));
    }
    require_name(db, signature, parse, &signature.result[0].text, None)
}

/// Parameter authority is supplied by the ordinary parser's opaque RefType
/// proof. No caller-selected spelling or range can masquerade as that role.
pub(super) fn require_reference(
    db: &Database,
    signature: &FunctionSignature,
    syntax: &ClosedReferenceSyntax,
) -> Result<(), Diagnostic> {
    syntax.require(signature)?;
    require_reference_type(db, signature, syntax.reference())
}
pub(super) fn require_reference_type(
    db: &Database,
    signature: &FunctionSignature,
    syntax: &ClosedReferenceTypeSyntax,
) -> Result<(), Diagnostic> {
    syntax.require(signature)?;
    require_name(db, signature, syntax.parse(), syntax.target(), Some(syntax))
}

fn require_name(
    db: &Database,
    signature: &FunctionSignature,
    parse: &ParseKey,
    written_name: &str,
    reference: Option<&ClosedReferenceTypeSyntax>,
) -> Result<(), Diagnostic> {
    let missing = || {
        fault(
            "LRR-E-CALL-CONTEXT",
            "primitive result requires its exact current generated Type namespace",
        )
    };
    let Some((InputValue::GeneratedSyntax(unit), _)) =
        db.published_input(&InputKey::GeneratedSyntax(parse.content))
    else {
        return Err(missing());
    };
    let Some(definition) = unit.definitions.get(&signature.definition) else {
        return Err(missing());
    };
    let Some((InputValue::DeclarationContext(context), _)) =
        db.published_input(&InputKey::DeclarationContext(signature.definition))
    else {
        return Err(missing());
    };
    let Some((InputValue::DefinitionBody(body), _)) =
        db.published_input(&InputKey::DefinitionBody(signature.definition))
    else {
        return Err(missing());
    };
    if unit.key != *parse
        || !matches!(db.published_input(&InputKey::DefinitionSource(signature.definition)), Some((InputValue::DefinitionSource(current), _)) if current == parse)
        || signature.header.profile != unit.key.profile
        || context.scope != signature.scope
        || signature.scope.owner.is_some()
        || definition.crate_key != signature.scope.crate_key
        || definition.module_path != signature.scope.module_path
        || signature.scope.crate_key != unit.scope.crate_key
        || !signature
            .scope
            .module_path
            .starts_with(&unit.scope.module_path)
        || !matches!(db.published_input(&InputKey::DefinitionHeader(signature.definition)), Some((InputValue::DefinitionHeader(header), _)) if header == &signature.header)
        || !unit.items.items().iter().any(|header| {
            header.key.fingerprint() == signature.definition
                && header.start == context.range.start
                && header.end == context.range.end
        })
    {
        return Err(missing());
    }
    let header = unit
        .items
        .items()
        .iter()
        .find(|header| header.key.fingerprint() == signature.definition)
        .ok_or_else(missing)?;
    let header_range =
        crate::syntax::spans::SourceRange::checked(header.header_start, header.header_end)?;
    let selected_range = reference.map_or(header_range, ClosedReferenceTypeSyntax::target_range);
    if !header_range.contains(selected_range) {
        return Err(missing());
    }
    let origins = unit.items.project_source_spans(selected_range)?;
    if origins.is_empty() {
        return Err(missing());
    }
    // The reference proof already came from the common ParseFile query and
    // exact current signature. Results preserve their historical full-signature
    // replay check; no additional replay is needed for a captured parameter.
    if reference.is_none() {
        // Reuse exact retained token syntax and the normal pinned signature capture.
        // No body-structure query, preparation, macro execution or source reparse.
        let transient = unit
            .items
            .syntax_events()
            .len()
            .checked_mul(128)
            .and_then(|n| {
                unit.items
                    .text()
                    .len()
                    .checked_mul(128)
                    .and_then(|m| n.checked_add(m))
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)?;
        if unit.metadata_bytes()? > 32 * 1024 * 1024 {
            return Err(crate::runtime::arena::resource());
        }
        let parsed = crate::syntax::parse::ParsedFile::from_generated_items(&unit.items)?;
        if parsed.estimated_bytes() > transient {
            return Err(crate::runtime::arena::resource());
        }
        if parsed.key() != *parse
            || parsed.function_signature(
                signature.definition,
                &signature.header,
                context,
                Some(body),
            )? != *signature
        {
            return Err(missing());
        }
        drop(parsed);
    }
    let Some((InputValue::DiscoverySeal(publication), _)) =
        db.published_input(&InputKey::DiscoverySeal(unit.seal.clone()))
    else {
        return Err(missing());
    };
    let record = unit.items.record();
    if publication.seal.key() != unit.seal
        || unit.items.epoch() != unit.seal.epoch
        || !publication
            .records
            .iter()
            .any(|descriptor| descriptor.fingerprint() == record.descriptor().fingerprint())
        || !matches!(db.published_input(&InputKey::ExpansionRecord(record.key().fingerprint())), Some((InputValue::ExpansionRecord(current), _)) if current.same_record(record))
    {
        return Err(missing());
    }
    let mut selected = None;
    for receipt in &publication.type_scopes {
        if receipt.scope().scope == signature.scope
            && receipt.scope().edition == signature.header.profile.edition
        {
            if selected.is_some() {
                return Err(missing());
            }
            selected = Some(receipt);
        }
    }
    let receipt = selected.ok_or_else(missing)?;
    receipt.require_owner(&signature.scope, &signature.header.profile)?;
    receipt.require_current(db, publication)?;
    // Source-authority roots are not name-comparison classes. A foreign root
    // needs its own current original declaration and independently sealed Type
    // lookup; absence in the insertion scope alone cannot authorize it.
    require_original_origins(db, signature, unit, publication, &origins, written_name)?;
    let name =
        crate::expand::scopes::MacroName::parse(written_name, signature.header.profile.edition)?;
    if receipt.scope().namespace != Namespace::Type
        || receipt.scope().names.contains_key(name.as_str())
    {
        return Err(missing());
    }
    Ok(())
}

fn edition_year(edition: crate::syntax::token_wire::Edition) -> u16 {
    use crate::syntax::token_wire::Edition;
    match edition {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    }
}

/// Conservative intersection of the actual insertion and original definer Type
/// namespaces. This does not implement general mixed-site/opaque Type lookup.
fn require_original_origins(
    db: &Database,
    signature: &FunctionSignature,
    unit: &crate::expand::generated_syntax::GeneratedSyntaxInput,
    publication: &crate::db::seals::DiscoveryInput,
    origins: &[&crate::syntax::token_wire::SpanRecord],
    written_name: &str,
) -> Result<(), Diagnostic> {
    use crate::{expand::identity::MacroIdentity, syntax::declaration_scan::DeclarationScanLimits};
    let missing = || {
        fault(
            "LRR-E-CALL-CONTEXT",
            "primitive result requires its exact current generated Type namespace",
        )
    };
    if generated_parent::is_generated_definition(db, unit.items.record()) {
        return generated_parent::require(db, unit, publication, origins, written_name);
    }
    let hygiene = unit.items.hygiene_facts();
    // Bounded by project_source_spans' 64-row limit. Retain only borrowed source
    // selections and the exact replayed macro body range, never caller proofs.
    let mut checked: Vec<(
        &crate::source::changes::SourceSelection,
        crate::syntax::spans::SourceRange,
    )> = Vec::new();
    checked
        .try_reserve_exact(origins.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    let mut work = 0usize;
    for origin in origins {
        let item = hygiene.context(hygiene.item(origin.context)?)?;
        if item.parent.is_some() {
            return Err(missing());
        }
        if item.defining_unit.0 == signature.scope.crate_key.0 {
            continue;
        }
        // Only actual original-root macro_rules template tokens are supported.
        // Generated/opaque sources and captured tokens of a different owner
        // require an exact hygiene lookup producer, not a spelling exception.
        if item.defining_unit != origin.source.unit {
            return Err(missing());
        }
        let mut sources = publication.sources.iter().filter(|source| {
            source.outline.crate_key.0 == origin.source.unit.0
                && source.outline.parse.content.0 == origin.source.content.0
                && source.path.as_str() == origin.source.path
        });
        let source = sources.next().ok_or_else(missing)?;
        if sources.next().is_some()
            || edition_year(item.edition) != source.outline.parse.profile.edition
            || edition_year(origin.edition) != source.outline.parse.profile.edition
            || unit.items.record().key().definition_edition != source.outline.parse.profile.edition
        {
            return Err(missing());
        }
        let range = if let Some((_, range)) =
            checked.iter().find(|(selection, _)| *selection == source)
        {
            *range
        } else {
            let MacroIdentity::Declaration { definition } =
                &unit.items.record().key().macro_identity
            else {
                return Err(missing());
            };
            let view = db.prepared_source(source)?.ok_or_else(missing)?;
            if view.selection() != source {
                return Err(missing());
            }
            let scope = &view.outline().root_scope;
            let scan = view.declaration_inventory(DeclarationScanLimits::default())?;
            work = work
                .checked_add(scan.retained_bytes())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(crate::runtime::arena::resource)?;
            // Ordinary source item discovery selects its published DefKey;
            // cold declaration discovery selects the scan identity. These
            // are distinct domains joined only by the actual owning source
            // declaration, never by a printed name.
            let original = view.outline().items.iter().find(|item| {
                item.def_key.fingerprint() == *definition
                    && item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                    && item.scope == *scope
            });
            if let Some(item) = original
                && (!matches!(db.published_input(&InputKey::DefinitionSource(*definition)), Some((InputValue::DefinitionSource(parse), _)) if parse == &source.outline.parse)
                    || !matches!(db.published_input(&InputKey::DefinitionHeader(*definition)), Some((InputValue::DefinitionHeader(header), _)) if header == &item.header)
                    || !matches!(db.published_input(&InputKey::DeclarationContext(*definition)), Some((InputValue::DeclarationContext(context), _)) if context.scope == item.scope && context.range == item.declaration_span))
            {
                return Err(missing());
            }
            let mut declarations = scan.macro_definitions().iter().filter(|header| {
                header.identity == *definition
                    || original.is_some_and(|item| item.declaration_span == header.range)
            });
            let header = declarations.next().ok_or_else(missing)?;
            if declarations.next().is_some()
                || !scan.lexical_scopes().iter().any(|lexical| {
                    lexical.parent.is_none() && lexical.identity == header.lexical_scope
                })
            {
                return Err(missing());
            }
            let mut receipts = publication.type_scopes.iter().filter(|proof| {
                proof.scope().scope == *scope
                    && proof.scope().edition == source.outline.parse.profile.edition
            });
            let proof = receipts.next().ok_or_else(missing)?;
            if receipts.next().is_some() {
                return Err(missing());
            }
            proof.require_owner(scope, &source.outline.parse.profile)?;
            proof.require_current(db, publication)?;
            let name = crate::expand::scopes::MacroName::parse(
                written_name,
                source.outline.parse.profile.edition,
            )?;
            if proof.scope().namespace != Namespace::Type
                || proof.scope().names.contains_key(name.as_str())
            {
                return Err(missing());
            }
            checked.push((source, header.body));
            header.body
        };
        if origin.start < range.start || origin.end > range.end || origin.start > origin.end {
            return Err(missing());
        }
    }
    Ok(())
}

#[path = "generated_return_parent.rs"]
mod generated_parent;
#[cfg(test)]
#[path = "generated_return_parent_profile_tests.rs"]
mod parent_profile_tests;

#[cfg(test)]
#[path = "generated_result_cross_source_tests.rs"]
mod cross_source_tests;

#[cfg(test)]
#[path = "generated_reference_source_tests.rs"]
mod reference_source_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::compile_input_catalog::CompileInputCatalog,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
    };
    use std::sync::Arc;
    fn prepared(
        text: &str,
        edition: u16,
    ) -> (
        Database,
        FunctionSignature,
        ParseKey,
        crate::db::seals::DiscoveryInput,
    ) {
        prepared_with_epoch(text, edition, true)
    }
    fn prepared_with_epoch(
        text: &str,
        edition: u16,
        publish_epoch: bool,
    ) -> (
        Database,
        FunctionSignature,
        ParseKey,
        crate::db::seals::DiscoveryInput,
    ) {
        prepared_config(text, edition, publish_epoch, false)
    }
    fn prepared_config(
        text: &str,
        edition: u16,
        publish_epoch: bool,
        compile_environment: bool,
    ) -> (
        Database,
        FunctionSignature,
        ParseKey,
        crate::db::seals::DiscoveryInput,
    ) {
        let (db, signature, parse, publication, _) =
            prepared_config_retained(text, edition, publish_epoch, compile_environment);
        (db, signature, parse, publication)
    }
    pub(super) fn prepared_config_retained(
        text: &str,
        edition: u16,
        publish_epoch: bool,
        compile_environment: bool,
    ) -> (
        Database,
        FunctionSignature,
        ParseKey,
        crate::db::seals::DiscoveryInput,
        PreparedSourceView,
    ) {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let unit = SourceUnitInput {
            path: SourcePath::new("generated-result.rs").unwrap(),
            crate_key: Digest([141; 32]),
            module_path: vec![],
            profile: profile.clone(),
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        };
        let selection = SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: vec![],
                cfg: unit.cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh_with_dependencies(
            None,
            vec![unit],
            Default::default(),
            vec![],
        )
        .unwrap();
        let inputs = if compile_environment {
            let unit = selection.outline.crate_key;
            let environment = crate::expand::builtins::FrozenCompileEnvironment::new(
                unit,
                &crate::project::tool_manifest::CompileEnv {
                    unit,
                    host: crate::core::profile::GUEST_TARGET.into(),
                    target: crate::core::profile::GUEST_TARGET.into(),
                    cfg: CfgSet::guest_default(),
                    cargo: BTreeMap::new(),
                    rustc_env: BTreeMap::new(),
                    declared: BTreeMap::new(),
                    out_dir: None,
                },
            )
            .unwrap();
            inputs
                .with_compile_environment(Arc::new(environment))
                .unwrap()
        } else {
            inputs
        };
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        if publish_epoch {
            db.begin_expansion_epoch(1).unwrap();
        }
        let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
            panic!()
        };
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest([142; 32]),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest([143; 32]),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
        )
        .unwrap();
        let seal = loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(seal) => break seal,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                _ => panic!("genuine preparation remains incomplete"),
            }
        };
        discovery.publish(&mut db).unwrap();
        let publication = match db
            .published_input(&InputKey::DiscoverySeal(seal.key()))
            .unwrap()
            .0
        {
            InputValue::DiscoverySeal(publication) => publication.as_ref().clone(),
            _ => panic!(),
        };
        let stamps = db.lexical_namespace_stamps().unwrap();
        let generated = stamps
            .keys()
            .find_map(|key| match db.published_input(key) {
                Some((InputValue::GeneratedSyntax(unit), _))
                    if unit
                        .definitions
                        .values()
                        .any(|definition| definition.declared_name == "answer") =>
                {
                    Some(unit.clone())
                }
                _ => None,
            })
            .unwrap();
        let id = *generated
            .definitions
            .iter()
            .find(|(_, definition)| definition.declared_name == "answer")
            .unwrap()
            .0;
        let InputValue::DefinitionHeader(header) = db
            .published_input(&InputKey::DefinitionHeader(id))
            .unwrap()
            .0
        else {
            panic!()
        };
        let InputValue::DeclarationContext(context) = db
            .published_input(&InputKey::DeclarationContext(id))
            .unwrap()
            .0
        else {
            panic!()
        };
        let InputValue::DefinitionBody(body) =
            db.published_input(&InputKey::DefinitionBody(id)).unwrap().0
        else {
            panic!()
        };
        let parsed =
            crate::syntax::parse::ParsedFile::from_generated_items(&generated.items).unwrap();
        let signature = parsed
            .function_signature(id, header, context, Some(body))
            .unwrap();
        (db, signature, generated.key.clone(), publication, source)
    }
    const SOURCE: &str = "macro_rules! emit{()=>{pub fn answer()->i64{41_i64}}}emit!();fn main()->i64{answer()}fn cold<T:Missing>(){unknown();}";
    #[test]
    fn real_value_publication_carries_exact_generated_type_receipt_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (db, signature, parse, publication) = prepared(SOURCE, edition);
            assert!(matches!(
                publication.seal.key().domain,
                crate::db::seals::SealDomain::Scope {
                    namespace: Namespace::Value,
                    ..
                }
            ));
            assert!(!publication.type_scopes.is_empty());
            require(&db, &signature, &parse).unwrap();
        }
    }
    #[test]
    fn generated_primitive_spelling_shadow_is_not_primitive_authority() {
        for edition in [2015, 2018, 2021, 2024] {
            let (db, signature, parse, _) = prepared(
                "type i64=bool;macro_rules! emit{()=>{fn answer()->i64{true}}}emit!();fn main(){}",
                edition,
            );
            assert_eq!(
                require(&db, &signature, &parse).unwrap_err().code,
                "LRR-E-CALL-CONTEXT"
            );
        }
    }
    #[test]
    fn exact_receipt_rejects_missing_record_foreign_scope_and_changed_epoch() {
        let (mut db, signature, _, publication) = prepared(SOURCE, 2024);
        let receipt = publication
            .type_scopes
            .iter()
            .find(|proof| proof.scope().scope == signature.scope)
            .unwrap();
        receipt.require_current(&db, &publication).unwrap();
        let mut altered = publication.clone();
        altered.type_scopes.clear();
        assert_eq!(
            receipt.require_current(&db, &altered).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        let mut altered = publication.clone();
        altered.records.clear();
        assert_eq!(
            receipt.require_current(&db, &altered).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        let mut altered = publication.clone();
        altered.sources.clear();
        assert_eq!(
            receipt.require_current(&db, &altered).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        let mut altered = publication.clone();
        altered.compiler_inputs.insert(
            InputKey::UnitCompileEnv(Digest([145; 32])),
            Digest([146; 32]),
        );
        assert_eq!(
            receipt.require_current(&db, &altered).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        let mut foreign = signature.scope.clone();
        foreign.crate_key = Digest([144; 32]);
        assert_eq!(
            receipt
                .require_owner(&foreign, &signature.header.profile)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            receipt.require_current(&db, &publication).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
    }
    #[test]
    fn original_check_uses_actual_nested_type_receipt_with_generated_answer_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, _, _, publication, source) = prepared_config_retained(
                "macro_rules! emit{()=>{pub fn answer()->i64{41_i64}}}emit!();fn check()->bool{answer()==41_i64}fn main(){assert!(check());}",
                edition,
                true,
                false,
            );
            assert_eq!(source.selection(), &publication.sources[0]);
            let replay = db.prepared_source(source.selection()).unwrap().unwrap();
            assert_eq!(replay.selection(), source.selection());
            let item = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "check")
                .unwrap();
            let id = item.def_key.fingerprint();
            let InputValue::DeclarationContext(context) = db
                .published_input(&InputKey::DeclarationContext(id))
                .unwrap()
                .0
            else {
                panic!()
            };
            let parsed = db
                .prepared_parse(&source.selection().outline.parse)
                .unwrap();
            let signature = parsed
                .function_signature(id, &item.header, context, item.body.as_ref())
                .unwrap();
            let stamps = db.source_view_namespace_stamps().unwrap();
            NormalizedDeferredReturn::require_primitive_namespace(
                &mut db,
                &signature,
                &[&source],
                &stamps,
            )
            .unwrap();
        }
    }
    #[test]
    fn real_value_publication_without_epoch_row_is_a_precise_preparation_prerequisite() {
        let (db, signature, parse, _) = prepared_with_epoch(SOURCE, 2024, false);
        let error = require(&db, &signature, &parse).unwrap_err();
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
        assert_eq!(
            error.message,
            "Type receipt requires its published matching ExpansionEpoch"
        );
        assert!(db.published_input(&InputKey::ExpansionEpoch).is_none());
    }
    #[test]
    fn genuine_cli_prepared_unused_compile_environment_is_bound_without_claiming_consumption_all_editions()
     {
        for edition in [2015, 2018, 2021, 2024] {
            let (db, signature, parse, publication) = prepared_config(SOURCE, edition, true, true);
            let key = InputKey::UnitCompileEnv(signature.scope.crate_key);
            assert!(matches!(
                db.published_input(&key),
                Some((InputValue::UnitCompileEnv(_), _))
            ));
            assert!(!publication.compiler_inputs.contains_key(&key));
            require(&db, &signature, &parse).unwrap();
        }
    }
    #[test]
    fn actual_new_compile_world_invalidates_old_receipt_without_changing_source_or_header() {
        let (_, old_signature, old_parse, old) = prepared_config(SOURCE, 2024, true, false);
        let (db, signature, parse, publication) = prepared_config(SOURCE, 2024, true, true);
        assert_eq!(old_signature, signature);
        assert_eq!(old_parse, parse);
        let old_receipt = old
            .type_scopes
            .iter()
            .find(|proof| proof.scope().scope == signature.scope)
            .unwrap();
        let current = publication
            .type_scopes
            .iter()
            .find(|proof| proof.scope().scope == signature.scope)
            .unwrap();
        assert_eq!(
            old_receipt.scope().fingerprint(),
            current.scope().fingerprint()
        );
        assert_ne!(old_receipt.fingerprint(), current.fingerprint());
        assert_eq!(
            old_receipt
                .require_current(&db, &publication)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        current.require_current(&db, &publication).unwrap();
    }
}
