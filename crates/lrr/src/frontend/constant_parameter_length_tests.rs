//! Current-source length controls; no evaluator/provider/guest execution.
use super::*;
use crate::syntax::{
    pure_constants::{ConstantSource, fixture_length_sites as length_sites},
    spans::SourceRange,
};
const IRREFUTABLE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const_let_irrefutable.rs"
);
const CAST_FN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/min_const_fn/cast_fn.rs"
);
const NRVO: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/nrvo.rs");
const RETURN_ARRAY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/mir/mir_codegen_array_2.rs"
);
const STATIC_ROOT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-70773-mir-typeck-lt-norm.rs"
);

fn admit_length(
    text: &str,
    name: &str,
    edition: u16,
    site: SourceRange,
) -> Result<ConstantSourceAdmission, Diagnostic> {
    let (db, view, definition) = world(text, name, edition);
    let before = db.trace.counts.clone();
    let admitted = source_admit(&db, &view, definition, ConstantRequest::ArrayLength(site))?;
    assert_eq!(admitted.plan().owner_definition(), definition);
    assert_eq!(admitted.plan().range(), site);
    admitted.revalidate(&db, &view)?;
    let ready =
        CurrentConstantResultType::capture_current(&db, &view, admitted.clone(), MAX_BYTES)?;
    assert_eq!(ready.scalar().unwrap().primitive(), PrimitiveType::Usize);
    assert_eq!(db.trace.counts, before);
    Ok(admitted)
}
#[test]
fn unchanged_parameter_array_headers_admit_exact_static_lengths_all_editions() {
    for (source, name, spelling) in [(IRREFUTABLE, "array", "2"), (CAST_FN, "unsize", "3")] {
        for edition in [2015, 2018, 2021, 2024] {
            let sites = length_sites(source, name, edition);
            assert_eq!(sites.len(), 1);
            let site = sites[0];
            assert_eq!(&source[site.start as usize..site.end as usize], spelling);
            admit_length(source, name, edition, site).unwrap();
        }
    }
}
#[test]
fn unchanged_array_return_and_repeat_only_read_the_selected_length_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let sites = length_sites(RETURN_ARRAY, "into_inner", edition);
        assert_eq!(sites.len(), 2);
        let mut identities = Vec::new();
        for (site, spelling) in sites.into_iter().zip(["1024", "2*4*8*16"]) {
            assert_eq!(
                &RETURN_ARRAY[site.start as usize..site.end as usize],
                spelling
            );
            identities.push(
                admit_length(RETURN_ARRAY, "into_inner", edition, site)
                    .unwrap()
                    .plan()
                    .identity(),
            );
        }
        assert_ne!(identities[0], identities[1]);
    }
}
#[test]
fn unchanged_nrvo_retains_inline_attribute_frontier_without_skipping_it() {
    let sites = length_sites(NRVO, "init", 2024);
    assert_eq!(sites.len(), 1);
    let err = admit_length(NRVO, "init", 2024, sites[0]).unwrap_err();
    assert_eq!(err.code, "LRR-E-CONST-SOURCE");
    assert!(err.message.contains("attribute"));
    // This zero-parameter header is the existing admitted control. Mutation,
    // borrowed buffers, inline preparation and full original remain unexecuted.
    let site = length_sites(NRVO, "nrvo", 2024)[0];
    admit_length(NRVO, "nrvo", 2024, site).unwrap();
}
#[test]
fn actual_module_static_root_is_selected_without_entering_its_body() {
    let source = "const N:usize=42;const fn hidden()->usize{loop{}}const fn f(x:u32)->[u8;N+hidden()]{loop{}}fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let site = length_sites(source, "f", edition)[0];
        admit_length(source, "f", edition, site).unwrap();
    }
}
#[test]
fn potential_parameter_shadows_never_fall_back_to_same_named_global() {
    for pattern in ["N:u32", "r#N:u32", "(N,_):(u32,u32)", "[N,_]:[u32;2]"] {
        let source =
            format!("const N:usize=42;const fn f({pattern})->[u8;N]{{loop{{}}}}fn main(){{}}");
        let sites = length_sites(&source, "f", 2024);
        let site = *sites.last().unwrap();
        let err = admit_length(&source, "f", 2024, site).unwrap_err();
        assert_eq!(err.code, "LRR-E-CONST-LENGTH-SOURCE");
        assert!(err.message.contains("parameter"));
    }
}
#[test]
fn local_import_qualified_generic_and_unproven_pattern_context_stays_closed() {
    for (source, name) in [
        ("const N:usize=42;fn f(x:u32){let N=1;let _=[0;N];}", "f"),
        (
            "mod provider{pub const N:usize=42;}use provider::N;fn f(x:u32)->[u8;N]{loop{}}",
            "f",
        ),
        (
            "mod provider{pub const N:usize=42;}fn f(x:u32)->[u8;provider::N]{loop{}}",
            "f",
        ),
        ("fn f<T>(x:T)->[u8;3]{loop{}}", "f"),
        ("fn f(x:u32)where u32:Copy{let _=[0;3];}", "f"),
        ("fn f(Some(x):Option<u32>)->[u8;3]{loop{}}", "f"),
        ("fn f(x:u32)->[u8;Missing]{loop{}}", "f"),
        ("type usize=bool;fn f(x:u32)->[u8;42 as usize]{loop{}}", "f"),
        ("fn f(x:u32){fn cold()->usize{3}let _=[0;cold()];}", "f"),
        ("fn outer(){fn f(x:u32)->[u8;3]{loop{}}}", "f"),
    ] {
        let site = length_sites(source, name, 2024)[0];
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let captured = ConstantSource::parse(
            &blob,
            ProfileIdentity::current(),
            Digest::of_bytes(b"length negative scope"),
            CfgSet::guest_default(),
        )
        .unwrap();
        assert!(captured.array_length(site).is_err(), "{source}");
    }
}
#[test]
fn current_header_changed_source_and_epoch_refuse_without_provider_work() {
    let source = "const fn f(x:u32)->[u8;42]{loop{}}fn main(){}";
    let site = length_sites(source, "f", 2024)[0];
    let (mut db, view, definition) = world(source, "f", 2024);
    let admitted =
        source_admit(&db, &view, definition, ConstantRequest::ArrayLength(site)).unwrap();
    let (_, changed, _) = world("const fn f(y:u32)->[u8;42]{loop{}}fn main(){}", "f", 2024);
    assert!(admitted.revalidate(&db, &changed).is_err());
    let before = db.trace.counts.clone();
    db.begin_expansion_epoch(2).unwrap();
    assert!(admitted.revalidate(&db, &view).is_err());
    assert_eq!(db.trace.counts, before);
}
#[test]
fn work_header_metadata_and_current_admission_quotas_refuse_before_evaluation() {
    let parameters = (0..257)
        .map(|n| format!("p{n}:u8"))
        .collect::<Vec<_>>()
        .join(",");
    let long_header = format!("fn f(x:u8/*{}*/)->[u8;3]{{loop{{}}}}", "a".repeat(65_536));
    for source in [
        format!("fn f({parameters})->[u8;3]{{loop{{}}}}"),
        long_header,
    ] {
        let site = length_sites(&source, "f", 2024)[0];
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let captured = ConstantSource::parse(
            &blob,
            ProfileIdentity::current(),
            Digest::of_bytes(b"length budget"),
            CfgSet::guest_default(),
        )
        .unwrap();
        assert_eq!(
            captured.array_length(site).unwrap_err().code,
            "LRR-E-METADATA-LIMIT"
        );
    }
    let source = "const fn f(x:u32)->[u8;42]{loop{}}fn main(){}";
    let site = length_sites(source, "f", 2024)[0];
    let (db, view, definition) = world(source, "f", 2024);
    let admitted =
        source_admit(&db, &view, definition, ConstantRequest::ArrayLength(site)).unwrap();
    let before = db.trace.counts.clone();
    assert_eq!(
        ConstantSourceAdmission::capture_current(
            &db,
            &view,
            definition,
            ConstantRequest::ArrayLength(site),
            admitted.metadata_bytes() - 1
        )
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(db.trace.counts, before);
}

#[test]
fn foreign_owner_or_nonlength_range_cannot_reuse_a_current_header_witness() {
    let source =
        "const fn f(x:u32)->[u8;42]{loop{}} const fn other(y:u32)->[u8;42]{loop{}}fn main(){}";
    let (db, view, definition) = world(source, "f", 2024);
    let site = length_sites(source, "f", 2024)[0];
    let before = db.trace.counts.clone();
    let foreign = length_sites(source, "other", 2024)[0];
    assert!(
        source_admit(
            &db,
            &view,
            definition,
            ConstantRequest::ArrayLength(foreign)
        )
        .is_err()
    );
    let changed = SourceRange::checked(site.start, site.end - 1).unwrap();
    assert!(
        source_admit(
            &db,
            &view,
            definition,
            ConstantRequest::ArrayLength(changed)
        )
        .is_err()
    );
    assert_eq!(db.trace.counts, before);
}

#[test]
fn unchanged_wildcard_parameter_header_selects_exact_current_module_constant() {
    for edition in [2015, 2018, 2021, 2024] {
        let sites = length_sites(STATIC_ROOT, "init_hash", edition);
        assert_eq!(sites.len(), 1);
        let site = sites[0];
        assert_eq!(
            &STATIC_ROOT[site.start as usize..site.end as usize],
            "HASH_LEN"
        );
        admit_length(STATIC_ROOT, "init_hash", edition, site).unwrap();
    }
}
