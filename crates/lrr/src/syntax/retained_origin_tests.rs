//! Origin projection is preparation evidence, never guest execution.
use super::*;
use crate::syntax::{ast_adapter::range, token_wire as owned};
use lrr_expand_adapter::token_wire::Edition;

#[test]
fn retained_expression_preserves_caller_and_definition_contexts_in_every_edition() {
    for edition in [
        Edition::E2015,
        Edition::E2018,
        Edition::E2021,
        Edition::E2024,
    ] {
        let snapshot = tests::captured(
            "fn main(){let captured=3_i32;let _=selected!(captured);}",
            "($value:expr)=>{$value+1_i32}",
            edition,
        );
        let parsed =
            ParsedFile::from_syntax_snapshot(snapshot.clone(), ProfileIdentity::current()).unwrap();
        let binary = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::BinExpr::cast)
            .unwrap();
        let spans = parsed
            .retained_source_spans(range(binary.syntax().text_range()))
            .unwrap()
            .unwrap();
        assert!(
            spans
                .iter()
                .any(|span| span.source.unit == owned::Key([91; 32]))
        );
        assert!(
            spans
                .iter()
                .any(|span| span.source.unit == owned::Key([1; 32]))
        );
        assert!(spans.iter().all(|span| {
            snapshot
                .origin_records()
                .spans()
                .iter()
                .any(|original| original.key().0 == span.key().0)
        }));
        let definition = spans
            .iter()
            .find(|span| span.source.unit == owned::Key([1; 32]))
            .unwrap();
        assert_eq!(
            definition.edition,
            match edition {
                Edition::E2015 => owned::Edition::E2015,
                Edition::E2018 => owned::Edition::E2018,
                Edition::E2021 => owned::Edition::E2021,
                Edition::E2024 => owned::Edition::E2024,
            }
        );
    }
}
#[test]
fn repeated_captured_origins_are_deduplicated_without_combining_locations() {
    let snapshot = tests::captured(
        "fn main(){selected!(captured);}",
        "($value:expr)=>{$value+$value}",
        Edition::E2024,
    );
    let parsed = ParsedFile::from_syntax_snapshot(snapshot, ProfileIdentity::current()).unwrap();
    let binary = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::BinExpr::cast)
        .unwrap();
    let spans = parsed
        .retained_source_spans(range(binary.syntax().text_range()))
        .unwrap()
        .unwrap();
    assert_eq!(
        spans
            .iter()
            .filter(|span| span.source.unit == owned::Key([91; 32]))
            .count(),
        1
    );
    assert!(
        spans
            .iter()
            .any(|span| span.source.unit == owned::Key([1; 32]))
    );
}
#[test]
fn unchanged_stock_generated_names_point_back_to_their_actual_definition_tokens() {
    const STOCK: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    use crate::{
        expand::source_declarative::{
            self,
            assembly::{self, SourceSyntaxAssembly},
        },
        frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index, _) = assembly::tests::world(STOCK, edition);
        let outer = assembly::tests::request(&provider, &index);
        let generated = assembly::tests::output(&provider, index.scan(), &outer, 1);
        let syntax = SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .attach(&outer, generated.clone())
            .unwrap();
        let nested = GeneratedDeclarationMacroIndex::new(
            generated.clone(),
            DeclarationMacroParent::Source(index),
        )
        .unwrap();
        let invocation = generated
            .scan()
            .producers()
            .iter()
            .find(|p| p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
            .unwrap()
            .identity;
        let source_declarative::SourceDeclarativeDemand::Ready(inner) = provider
            .generated_declaration_demand(&nested, invocation, None)
            .unwrap()
        else {
            panic!("stock nested expansion absent")
        };
        let child = assembly::tests::output(&provider, generated.scan(), &inner, 1);
        let prepared = syntax.attach(&inner, child).unwrap().finish().unwrap();
        let parsed = ParsedFile::from_syntax_snapshot(
            prepared.syntax().clone(),
            provider
                .module_source()
                .selection()
                .outline
                .parse
                .profile
                .clone(),
        )
        .unwrap();
        let name = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Name::cast)
            .find(|name| name.text() == "Bar")
            .unwrap();
        let spans = parsed
            .retained_source_spans(range(name.syntax().text_range()))
            .unwrap()
            .unwrap();
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.source.unit, owned::Key([241; 32]));
        assert_eq!(&STOCK[span.start as usize..span.end as usize], "Bar");
        assert_eq!(span.source.content.0, Digest::of_bytes(STOCK.as_bytes()).0);
    }
}
#[test]
fn retained_offsets_reject_reversed_outside_and_split_utf8_ranges() {
    let snapshot = tests::captured("fn main(){selected!();}", "()=>{\"é\"}", Edition::E2024);
    let text = snapshot.syntax().text.clone();
    let parsed = ParsedFile::from_syntax_snapshot(snapshot, ProfileIdentity::current()).unwrap();
    let start = u32::try_from(text.find('é').unwrap()).unwrap();
    for at in [
        SourceRange { start: 2, end: 1 },
        SourceRange {
            start: 0,
            end: u32::MAX,
        },
        SourceRange {
            start,
            end: start + 1,
        },
    ] {
        assert_eq!(
            parsed.retained_source_spans(at).unwrap_err().code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
    }
}
