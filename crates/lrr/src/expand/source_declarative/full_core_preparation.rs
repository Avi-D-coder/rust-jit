//! Required-positive preparation using unchanged pinned physical inputs. Every
//! candidate absence is independently observed, never inferred from an import.
use super::*;
use crate::{
    cli::macro_export_modules::{ConfiguredMacroExportSources, MacroExportModulePreparation},
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        source_module_declarations::SourceModuleDeclaration,
    },
    project::tool_manifest::{InputKind, InputPlan, InputRoot},
};
const ROOT: &[u8] =
    include_bytes!("../../../../../fixtures/upstream/rustc-checkout/library/core/src/lib.rs");
fn fault(message: &str) -> crate::core::diagnostic::Diagnostic {
    crate::core::diagnostic::Diagnostic::new("LRR-E-MACRO-EXPORT-PREPARATION", message.into())
}
/// The configured-unit record is the existing source authority test world, not
/// an adopted SDK/native toolchain. Source bytes/inventory are actual captures.
pub(super) fn configured() -> Result<
    (Database, SourceDiscovery, ConfiguredMacroExportSources),
    crate::core::diagnostic::Diagnostic,
> {
    let package = fixture_root().join("library/core");
    let root_path = package.join("src/lib.rs");
    let (base, unit) = manifest(&root_path, ROOT, 2024, &package);
    let mut candidates = BTreeSet::new();
    for round in 0..256 {
        let plan = InputPlan::new(
            vec![InputRoot::open(
                InputKind::Source,
                &package,
                candidates.clone(),
                BTreeSet::from([SourcePath::new("src")?]),
                false,
                BTreeSet::new(),
            )?],
            BTreeSet::new(),
        )?;
        let observed = plan.capture()?;
        let mut manifest = base.clone();
        manifest.inputs = observed.inventory().clone();
        manifest.validate()?;
        if manifest.inputs.source_files.get(
            root_path
                .to_str()
                .ok_or_else(|| fault("core source path is not UTF-8"))?,
        ) != Some(&Some(Digest::of_bytes(ROOT)))
        {
            return Err(fault(
                "actual core root differs from unchanged pinned source",
            ));
        }
        let (root_input, selection) = input(unit, "src/lib.rs", ROOT, 2024);
        // A new inventory changes the generation. All old module/source leases
        // are discarded; no previous publication is rebound into this one.
        let prepared = PreparedSourceInputs::refresh(None, vec![root_input], BTreeMap::new())?;
        let source = select(&prepared, &selection);
        let scope = SourceModuleDeclaration::from_captured(&source)?
            .into_iter()
            .find(|row| row.request().name == "macros")
            .ok_or_else(|| fault("unchanged core root lacks its actual macros declaration"))?
            .scope()
            .clone();
        let facet = Arc::new(SourceMacroExportFacet::capture(
            &manifest,
            unit,
            root_path
                .to_str()
                .ok_or_else(|| fault("configured core path must be UTF-8"))?,
            source.share(),
        )?);
        let environment = Arc::new(FrozenCompileEnvironment::new(
            unit,
            &manifest.compile_env[&unit],
        )?);
        let mut db = Database::default();
        db.publish_inputs(BTreeMap::from([
            (
                InputKey::SourceDependencies,
                InputValue::SourceDependencies(source.source_dependencies()?),
            ),
            (
                InputKey::UnitCompileEnv(unit),
                InputValue::UnitCompileEnv(environment),
            ),
        ]))?;
        db.begin_expansion_epoch(1)?;
        db.publish_macro_export_sources(&[Arc::clone(&facet)])?;
        let state =
            ConfiguredMacroExportSources::new(&manifest, prepared, vec![facet], vec![], scope, 1)?;
        let compiler = Arc::new(CompileInputCatalog::standalone(
            &selection.outline.parse.profile,
        )?);
        eprintln!(
            "required core preparation round {round}: manifest {} source inventory {}",
            manifest.fingerprint(),
            manifest.inputs.fingerprint()
        );
        let result = state.prepare(&mut db, &manifest, compiler);
        // The capture/adoption effects may not silently race the independently
        // observed preparation grant, including its real directory membership.
        observed.verify_unchanged(&plan.capture()?)?;
        match result? {
            MacroExportModulePreparation::Ready { discovery, sources } => {
                return Ok((db, discovery, sources));
            }
            MacroExportModulePreparation::NeedInput { input, .. } => {
                return Err(crate::core::diagnostic::Diagnostic::new(
                    "LRR-E-MACRO-EXPORT-PREPARATION",
                    format!(
                        "required core needs actual external preparation input {}",
                        input.fingerprint()
                    ),
                ));
            }
            MacroExportModulePreparation::NeedPreparation { sources, demand } => {
                if demand.preparation() != manifest.fingerprint() {
                    return Err(fault("required core candidate demand preparation changed"));
                }
                let before = candidates.len();
                for path in demand.missing_paths() {
                    let relative = path.strip_prefix(&package).map_err(|_| {
                        fault(
                            "required core candidate requires another configured source authority",
                        )
                    })?;
                    candidates.insert(SourcePath::new(relative)?);
                    eprintln!(
                        "required core actual candidate capture grant: {}",
                        path.display()
                    );
                }
                // Batch only real retained declaration requests from the current
                // genuine selected set; no filename-derived logical module path.
                let pending = SourceDiscovery::new(
                    sources
                        .facets()
                        .iter()
                        .map(|facet| facet.source().share())
                        .collect(),
                    sources
                        .facets()
                        .iter()
                        .find(|facet| facet.source().outline().root_scope.module_path.is_empty())
                        .ok_or_else(|| fault("required core actual root disappeared"))?
                        .source()
                        .outline()
                        .root_scope
                        .clone(),
                    Namespace::Macro,
                    manifest.graph.fingerprint(),
                    1,
                )?
                .with_macro_export_source_publications(&mut db)?
                .with_macro_export_module_adoptions(&mut db, sources.modules())?;
                for declaration in pending.unresolved_ordinary_module_sources() {
                    let parent = sources
                        .facets()
                        .iter()
                        .find(|facet| {
                            facet.source().selection() == declaration.source().selection()
                        })
                        .ok_or_else(|| fault("required core actual candidate parent is absent"))?;
                    let authorities = crate::cli::macro_export_modules::authorities(parent)?;
                    eprintln!(
                        "required core actual declaration request: {} {}..{} producer {}",
                        declaration.source().selection().path.as_str(),
                        declaration.range().start,
                        declaration.range().end,
                        declaration.producer()
                    );
                    let request = match pending.prepare_original_module_source(
                        &mut db,
                        declaration.producer(),
                        &authorities,
                    ) {
                        QueryPoll::Ready(Ok(request)) => request,
                        QueryPoll::Ready(Err(error)) => return Err(error),
                        QueryPoll::NeedInput(_) => {
                            return Err(fault(
                                "required core batch requires actual compile environment",
                            ));
                        }
                    };
                    let InputRequest::CaptureModule { request, .. } = request else {
                        return Err(fault("required core batch request kind changed"));
                    };
                    for candidate in request.candidates() {
                        let path = parent.package_root().join(candidate.path().as_str());
                        let text = path
                            .to_str()
                            .ok_or_else(|| fault("required core candidate path must be UTF-8"))?;
                        if !manifest.inputs.source_files.contains_key(text) {
                            candidates.insert(SourcePath::new(path.strip_prefix(&package).map_err(|_| fault("required core batch requires another configured source authority"))?)?);
                        }
                    }
                }
                if candidates.len() == before || candidates.len() > 100_000 {
                    return Err(fault(
                        "required core preparation candidate closure made no progress or exceeded limit",
                    ));
                }
            }
        }
    }
    Err(fault("required core actual preparation generation limit"))
}
