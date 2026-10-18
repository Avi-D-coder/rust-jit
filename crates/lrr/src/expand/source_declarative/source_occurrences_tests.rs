//! Structural controls outside the complete unchanged official source.
use super::*;
use crate::expand::source_declarative::assembly::tests::{output, world};
use crate::{
    expand::source_declarative::SourceDeclarativeDemand,
    frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    syntax::parse::ParsedFile,
};
const AUTO: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
);
fn range(start: usize, len: usize) -> SourceRange {
    SourceRange::checked(
        u32::try_from(start).unwrap(),
        u32::try_from(start + len).unwrap(),
    )
    .unwrap()
}
fn request_named(
    provider: &SourceDeclarative,
    index: &DeclarationMacroIndex,
    name: &str,
) -> Box<SourceDeclarativeRequest> {
    let invocation = index
        .scan()
        .producers()
        .iter()
        .find(|row| {
            row.kind == DeclarationProducerKind::Macro
                && row.path.last().is_some_and(|token| token.text == name)
        })
        .unwrap()
        .identity;
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            index,
            invocation,
            Digest([17; 32]),
            super::super::super::engine_version(),
        )
        .unwrap()
    else {
        panic!("actual source occurrence request incomplete")
    };
    request
}
#[test]
fn unchanged_auto_type_position_survives_real_item_attachment_in_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index, _) = world(AUTO, edition);
        let issued = request_named(&provider, &index, "auto");
        let generated = output(&provider, index.scan(), &issued, 1);
        let assembly = SourceSyntaxAssembly::new(&provider, &index, 1).unwrap();
        let before = assembly.tree.materialize().unwrap();
        let initial_type = range(before.text.rfind("u8").unwrap(), 2);
        let initial = assembly.occurrences.occurrence(initial_type).unwrap();
        assert!(matches!(initial, SourceOccurrence::Original {range,..} if range == initial_type));
        let prepared = assembly
            .attach(&issued, generated.clone())
            .unwrap()
            .finish()
            .unwrap();
        let text = &prepared.syntax().syntax().text;
        let current_type = range(text.rfind("u8").unwrap(), 2);
        assert_eq!(prepared.occurrence(current_type).unwrap(), initial);
        assert_eq!(
            prepared.occurrence(range(0, text.len())).unwrap(),
            SourceOccurrence::Mixed
        );
        let output_range = prepared
            .occurrences
            .intervals
            .iter()
            .find(|row| matches!(row.origin, Origin::Generated(_)))
            .unwrap()
            .range;
        assert!(
            matches!(prepared.occurrence(output_range).unwrap(), SourceOccurrence::Generated {record,descriptor,invocation,parent_inventory,attachment:0,..}
            if record == generated.record().key().fingerprint() && descriptor == generated.record().descriptor().fingerprint()
            && invocation == generated.parent_invocation() && parent_inventory == generated.parent_inventory())
        );
        assert_eq!(
            prepared
                .occurrence(SourceRange {
                    start: current_type.start,
                    end: current_type.start
                })
                .unwrap(),
            SourceOccurrence::Pending
        );
        assert!(
            prepared
                .occurrence(SourceRange {
                    start: 0,
                    end: u32::try_from(text.len() + 1).unwrap()
                })
                .is_err()
        );
        assert!(prepared.occurrences.metadata_bytes().unwrap() <= usize::MAX);
    }
}
#[test]
fn real_attachment_moves_later_original_position_without_recovering_it_from_token_spans() {
    let text = format!(
        "{AUTO}\nmacro_rules! shift{{()=>{{let _structural_provenance_displacement = 1;}}}} fn provenance_motion(){{shift!(); let _ = 1 as u8;}}"
    );
    let (provider, index, _) = world(&text, 2024);
    let issued = request_named(&provider, &index, "shift");
    let generated = output(&provider, index.scan(), &issued, 1);
    let assembly = SourceSyntaxAssembly::new(&provider, &index, 1).unwrap();
    let before = assembly.tree.materialize().unwrap();
    let initial = range(before.text.rfind("u8").unwrap(), 2);
    let original = assembly.occurrences.occurrence(initial).unwrap();
    let prepared = assembly
        .attach(&issued, generated)
        .unwrap()
        .finish()
        .unwrap();
    let current = range(prepared.syntax().syntax().text.rfind("u8").unwrap(), 2);
    assert_ne!(
        current, initial,
        "long actual output did not move the retained source position"
    );
    assert_eq!(prepared.occurrence(current).unwrap(), original);
}
#[test]
fn copied_original_token_origins_never_restore_original_structural_position() {
    let text = format!(
        "{AUTO}\nmacro_rules! echo{{($item:item)=>{{$item}}}} echo!{{fn provenance_copied(){{}}}}"
    );
    let (provider, index, _) = world(&text, 2024);
    let issued = request_named(&provider, &index, "echo");
    let generated = output(&provider, index.scan(), &issued, 1);
    let assembly = SourceSyntaxAssembly::new(&provider, &index, 1).unwrap();
    let before = assembly.tree.materialize().unwrap();
    let name = "provenance_copied";
    let original_name = range(before.text.rfind(name).unwrap(), name.len());
    let original = assembly.occurrences.occurrence(original_name).unwrap();
    assert!(matches!(original, SourceOccurrence::Original { .. }));
    let before_parsed = ParsedFile::from_syntax_snapshot(
        assembly.tree.snapshot().unwrap(),
        provider.profile().clone(),
    )
    .unwrap();
    let origins = before_parsed
        .retained_source_spans(original_name)
        .unwrap()
        .unwrap();
    let prepared = assembly
        .attach(&issued, generated.clone())
        .unwrap()
        .finish()
        .unwrap();
    let current_name = range(
        prepared.syntax().syntax().text.rfind(name).unwrap(),
        name.len(),
    );
    let parsed =
        ParsedFile::from_syntax_snapshot(Arc::clone(prepared.syntax()), provider.profile().clone())
            .unwrap();
    let copied = parsed.retained_source_spans(current_name).unwrap().unwrap();
    assert_eq!(
        origins, copied,
        "control did not preserve identical copied origins"
    );
    assert!(
        matches!(prepared.occurrence(current_name).unwrap(), SourceOccurrence::Generated {record, ..} if record == generated.record().key().fingerprint())
    );
    assert_ne!(prepared.occurrence(current_name).unwrap(), original);
}
#[test]
fn repeated_copied_calls_and_nested_attachments_keep_distinct_actual_occurrences() {
    let text = format!(
        "{AUTO}\nmacro_rules! duplicate{{($call:stmt)=>{{$call $call}}}} macro_rules! leaf{{()=>{{let _provenance = 1;}}}} fn provenance_control(){{duplicate!(leaf!());}}"
    );
    let (provider, index, _) = world(&text, 2024);
    let issued = request_named(&provider, &index, "duplicate");
    let generated = output(&provider, index.scan(), &issued, 1);
    let mut assembly = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&issued, Arc::clone(&generated))
        .unwrap();
    let mut nested = GeneratedDeclarationMacroIndex::new(
        Arc::clone(&generated),
        DeclarationMacroParent::Source(index),
    )
    .unwrap();
    let ids = generated
        .scan()
        .producers()
        .iter()
        .filter(|row| row.kind == DeclarationProducerKind::Macro)
        .map(|row| row.identity)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    let mut sites = Vec::new();
    let mut children = Vec::new();
    for (ordinal, invocation) in ids.into_iter().enumerate() {
        let SourceDeclarativeDemand::Ready(inner) = provider
            .generated_declaration_demand(&nested, invocation, None)
            .unwrap()
        else {
            panic!("actual nested occurrence request incomplete")
        };
        sites.push(inner.site);
        let child = output(&provider, generated.scan(), &inner, 1);
        let child_index = GeneratedDeclarationMacroIndex::new(
            Arc::clone(&child),
            DeclarationMacroParent::Generated(Arc::new(nested.clone())),
        )
        .unwrap();
        assembly = assembly.attach(&inner, Arc::clone(&child)).unwrap();
        let interval = assembly.occurrences.intervals.iter().find(|row| matches!(row.origin, Origin::Generated(out) if out.attachment == ordinal as u32 +1)).unwrap();
        assert!(
            matches!(assembly.occurrences.occurrence(interval.range).unwrap(), SourceOccurrence::Generated {record, invocation: selected, attachment, ..}
            if record == child.record().key().fingerprint() && selected == invocation && attachment == ordinal as u32+1)
        );
        nested
            .adopt_completed_child(inner.key(), &child_index)
            .unwrap();
        children.push(child.record().key().fingerprint());
    }
    assert_eq!(
        sites[0], sites[1],
        "control did not repeat actual copied token origins"
    );
    let prepared = assembly.finish().unwrap();
    let mut found = BTreeSet::new();
    for interval in &prepared.occurrences.intervals {
        if let SourceOccurrence::Generated {
            record, attachment, ..
        } = prepared.occurrence(interval.range).unwrap()
        {
            if children.contains(&record) {
                found.insert(attachment);
            }
        }
    }
    assert_eq!(found, BTreeSet::from([1, 2]));
    assert_eq!(
        prepared
            .occurrence(range(0, prepared.syntax().syntax().text.len()))
            .unwrap(),
        SourceOccurrence::Mixed
    );
}
#[test]
fn malformed_ranges_and_overflowing_metadata_cannot_mint_occurrence_proof() {
    let map = SourceOccurrences::new(Digest([3; 32]), 8).unwrap();
    assert!(map.occurrence(SourceRange { start: 5, end: 4 }).is_err());
    assert!(map.occurrence(SourceRange { start: 0, end: 9 }).is_err());
    assert!(bytes(usize::MAX).is_err());
    assert!(map.validate(7).is_err());
}
