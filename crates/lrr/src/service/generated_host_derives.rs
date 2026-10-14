//! Pure macro-host registration facets; bodies remain opaque and no tool is invoked.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    expand::{
        declarative::{DeclarativeSession, ExpansionLimits},
        derive_declaration::SourceDeriveDeclaration,
        generated_items::{GeneratedHeaderKind, GeneratedItems},
        source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
    },
    project::tool_manifest::ToolManifest,
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{SourceBlob, SourcePath},
    },
};
use std::collections::BTreeSet;

pub(crate) struct GeneratedHostDerives {
    pub(crate) producer: Digest,
    pub(crate) source: Digest,
    pub(crate) definition_blob: SourceBlob,
    pub(crate) cfg: Digest,
    pub(crate) edition: u16,
    pub(crate) declarations: Vec<SourceDeriveDeclaration>,
    pub(crate) origin: Digest,
    pub(crate) expansions: u64,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
/// None is an unresolved producer, never an empty certified helper list.
pub(crate) fn prepare(
    manifest: &ToolManifest,
    consumer: Digest,
    producer: Digest,
    blob: &SourceBlob,
) -> Result<Option<GeneratedHostDerives>, Diagnostic> {
    let Some(edge) = manifest
        .graph
        .units
        .get(&consumer)
        .and_then(|u| u.dependencies.iter().find(|e| e.target == producer))
    else {
        return Ok(None);
    };
    let unit = &manifest.graph.units[&producer];
    let env = &manifest.compile_env[&producer];
    let parsed = crate::syntax::parse::ParsedFile::parse_macro_facet(blob, unit.key.edition)?;
    let original = parsed.source_derive_catalog(&env.cfg)?;
    if original.pending().is_empty() {
        return Ok(None);
    }
    let consumer_blob = super::derive_declarations::captured_source_root(manifest, consumer)?;
    let mut host_profile = ProfileIdentity::current();
    host_profile.edition = unit.key.edition;
    let units = vec![
        SourceUnitInput {
            path: SourcePath::new("macro-host.rs")?,
            crate_key: producer,
            module_path: vec![],
            profile: host_profile.clone(),
            cfg: env.cfg.clone(),
            blob: blob.clone(),
        },
        SourceUnitInput {
            path: SourcePath::new("consumer.rs")?,
            crate_key: consumer,
            module_path: vec![],
            profile: ProfileIdentity::current(),
            cfg: manifest.compile_env[&consumer].cfg.clone(),
            blob: consumer_blob,
        },
    ];
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![SourceDependency {
            consumer,
            alias: edge.alias.clone(),
            provider: producer,
        }],
    )?;
    let selection = crate::source::changes::SourceSelection {
        path: SourcePath::new("macro-host.rs")?,
        outline: crate::db::key::OutlineKey {
            parse: crate::syntax::parse::ParseKey {
                content: blob.content(),
                profile: host_profile,
            },
            crate_key: producer,
            module_path: vec![],
            cfg: env.cfg.fingerprint(),
        },
    };
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection)? else {
        return Err(fault("macro-host source view absent"));
    };
    let calls = view.outline().item_invocations.clone();
    let mut provider = SourceDeclarative::new(view.share())?;
    let mut declarations: Vec<_> = original
        .declarations()
        .iter()
        .map(|(_, d)| d.clone())
        .collect();
    let mut names: BTreeSet<_> = declarations
        .iter()
        .map(|d| d.names().export().to_owned())
        .collect();
    let mut bytes = 0usize;
    let mut session = DeclarativeSession::new(ExpansionLimits::default())?;
    let mut origin = StableEncoder::new("lrr/v2/generated-macro-host-derive-facet");
    origin.digest(producer);
    origin.digest(blob.content());
    origin.record(&env.cfg);
    origin.u16(unit.key.edition);
    for pending in original.pending() {
        let matching: Vec<_> = calls
            .iter()
            .filter(|c| {
                c.attribute_input.is_none()
                    && c.range.start == pending.start
                    && c.range.end <= pending.end
                    && pending.end - c.range.end <= 1
            })
            .collect();
        let [call] = matching.as_slice() else {
            return Ok(None);
        };
        let SourceDeclarativeDemand::Ready(request) = provider.demand(
            call.item_input,
            env.fingerprint(),
            Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
        )?
        else {
            return Ok(None);
        };
        let frame = session.begin(None, request.key().fingerprint())?;
        let record = std::sync::Arc::new(request.expand()?);
        let tokens = record.tokens()?;
        session.adopt(frame, &tokens)?;
        let items = GeneratedItems::admit(request.key(), record.clone(), 1)?;
        if items
            .items()
            .iter()
            .any(|i| i.kind == GeneratedHeaderKind::MacroRules)
        {
            return Ok(None);
        }
        // Replay the pinned owned syntax events: the bridge text deliberately
        // contains no inserted whitespace and must never be re-lexed as source.
        let expanded = crate::syntax::parse::ParsedFile::from_generated_items(&items)?;
        let generated = expanded.source_derive_catalog(&env.cfg)?;
        if !generated.pending().is_empty() {
            return Ok(None);
        }
        origin.digest(record.descriptor().fingerprint());
        for (_, decl) in generated.declarations {
            if !names.insert(decl.names().export().to_owned()) {
                return Err(fault("duplicate generated macro-host derive export"));
            }
            bytes = bytes
                .checked_add(decl.names().allocated_bytes()?)
                .and_then(|n| n.checked_add(512))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("generated macro-host declaration metadata limit"))?;
            if declarations.len() >= 4096 {
                return Err(fault("generated macro-host declaration count limit"));
            }
            origin.record(&decl);
            declarations.push(decl);
        }
        session.finish(frame)?;
        provider.complete_without_macro_bindings(call.item_input)?;
    }
    Ok(Some(GeneratedHostDerives {
        producer,
        source: blob.content(),
        definition_blob: blob.clone(),
        cfg: env.cfg.fingerprint(),
        edition: unit.key.edition,
        declarations,
        origin: origin.finish(),
        expansions: session.counts().invocations,
    }))
}
