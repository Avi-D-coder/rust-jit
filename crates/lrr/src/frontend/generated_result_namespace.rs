//! Current generated function result admission; no original-source substitution.
use super::*;
use crate::{
    source::manifest::Namespace,
    syntax::{parse::ParseKey, signatures::FunctionSignature},
};

pub(super) fn require(
    db: &Database,
    signature: &FunctionSignature,
    parse: &ParseKey,
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
    if signature.result.len() != 1
        || signature.result_symbolic.is_some()
        || unit.key != *parse
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
    let origins = unit
        .items
        .project_source_spans(crate::syntax::spans::SourceRange::checked(
            header.header_start,
            header.header_end,
        )?)?;
    if origins.is_empty() {
        return Err(missing());
    }
    let hygiene = unit.items.hygiene_facts();
    for origin in origins {
        let item = hygiene.context(hygiene.item(origin.context)?)?;
        // Ordinary item lookup must actually normalize to this owning source
        // root. Opaque/foreign definition-site lookup needs its own producer.
        if item.parent.is_some() || item.defining_unit.0 != signature.scope.crate_key.0 {
            return Err(missing());
        }
    }
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
    let name = crate::expand::scopes::MacroName::parse(
        &signature.result[0].text,
        signature.header.profile.edition,
    )?;
    if receipt.scope().namespace != Namespace::Type
        || receipt.scope().names.contains_key(name.as_str())
    {
        return Err(missing());
    }
    Ok(())
}

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
    fn prepared_config_retained(
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
            let (db, _, _, publication, source) = prepared_config_retained(
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
                &db,
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
