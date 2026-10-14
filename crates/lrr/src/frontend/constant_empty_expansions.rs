//! Actual completed empty records are evidence, never blanket generated authority.
use super::*;
pub(super) fn validate(
    db: &Database,
    input: &DiscoveryInput,
    key: &SealKey,
    sources: &RetainedConstantSources,
) -> Result<(), Diagnostic> {
    if input.records.len() > 256 || input.declaration_fragments.len() > 256 {
        return Err(limit());
    }
    if input.records.len() != input.declaration_fragments.len() {
        return Err(fault(
            "original empty expansion descriptors/fragments are incomplete",
        ));
    }
    let views = sources.views();
    let mut seen = BTreeSet::new();
    let mut wire_bytes = 0usize;
    for fragment in &input.declaration_fragments {
        let scan = fragment.source_scan();
        let source = views
            .iter()
            .find(|view| {
                &view.selection().outline.parse == scan.parse()
                    && &view.selection().path == scan.path()
                    && &view.outline().root_scope == scan.scope()
                    && view.selection().outline.cfg == scan.cfg()
            })
            .ok_or_else(|| fault("original empty expansion source not retained"))?;
        let record = fragment.require_original_empty_record(source, key.epoch)?;
        let descriptor = record.descriptor();
        if descriptor.wire_bytes > 65_536 {
            return Err(limit());
        }
        wire_bytes = wire_bytes
            .checked_add(descriptor.wire_bytes)
            .filter(|bytes| *bytes <= 16 * 1024 * 1024)
            .ok_or_else(limit)?;
        let identity = descriptor.key.fingerprint();
        if !seen.insert(identity)
            || input
                .records
                .iter()
                .filter(|actual| {
                    actual.key == descriptor.key
                        && actual.output == descriptor.output
                        && actual.wire_bytes == descriptor.wire_bytes
                })
                .count()
                != 1
        {
            return Err(fault(
                "original empty expansion descriptor is missing or duplicated",
            ));
        }
        let Some((InputValue::ExpansionRecord(actual), _)) =
            db.published_input(&InputKey::ExpansionRecord(identity))
        else {
            return Err(fault("original empty expansion current publication absent"));
        };
        if !actual.same_record(record) {
            return Err(fault("original empty expansion current payload differs"));
        }
        let crate::expand::identity::MacroIdentity::Declaration { definition } =
            &descriptor.key.macro_identity
        else {
            return Err(fault(
                "original empty expansion definition is not declarative",
            ));
        };
        let mut owners = 0usize;
        for view in &views {
            for item in &view.outline().items {
                if item.def_key.fingerprint() == *definition
                    && item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                    && item.scope.owner.is_none()
                    && view.selection().outline.parse.profile.edition
                        == descriptor.key.definition_edition
                {
                    owners = owners.checked_add(1).ok_or_else(limit)?;
                }
            }
        }
        if owners != 1 {
            return Err(fault(
                "original empty expansion exact module definition owner absent or ambiguous",
            ));
        }
    }
    Ok(())
}
