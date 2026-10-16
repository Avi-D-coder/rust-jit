//! Final source and generated test metadata, gated by actual test discovery seals.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::{
        changes::PreparedSourceView,
        manifest::{BodyKey, ItemKind},
    },
    syntax::outline::AttributeClass,
};
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub definition: Digest,
    pub expansion_ancestry: Vec<Digest>,
    pub body: Option<BodyKey>,
    pub ignored: bool,
    pub should_panic: bool,
    pub panic_contains: Option<String>,
    pub unsupported_signature: bool,
    pub attribute_error: Option<Diagnostic>,
}
/// Ephemeral final-metadata selection policy, not a query or execution mode.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IgnorePolicy {
    Exclude,
    Only,
    Include,
}
/// A final value declaration and optional earlier compiler Test registration.
/// Borrow registration headers; do not clone or alter generated token metadata.
pub(crate) struct TestDeclaration<'a> {
    pub item: crate::syntax::outline::ItemOutline,
    pub identity: Digest,
    pub ancestry: Vec<Digest>,
    pub registration: Option<&'a crate::syntax::outline::HeaderRecipe>,
}
pub struct TestCatalog {
    cases: Vec<TestCase>,
    seal: Option<crate::db::seals::Seal>,
    source_revision: u64,
    environment: Option<std::sync::Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TEST-DISCOVERY", message.into())
}
impl TestCatalog {
    pub fn original(sources: &[PreparedSourceView]) -> Result<Self, Diagnostic> {
        for source in sources {
            if source
                .outline()
                .scopes
                .values()
                .any(|scope| !scope.blockers.is_empty())
                || source.outline().items.iter().any(|item| {
                    item.header.attributes.iter().any(|attr| {
                        matches!(
                            attr.class,
                            AttributeClass::TransformUnsupported | AttributeClass::Derive
                        )
                    })
                })
            {
                return Err(Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "test discovery requires authorized expansion/catalog integration".into(),
                ));
            }
        }
        Self::collect(
            sources
                .iter()
                .flat_map(|source| source.outline().items.iter()),
        )
    }
    fn collect<'a>(
        items: impl Iterator<Item = &'a crate::syntax::outline::ItemOutline>,
    ) -> Result<Self, Diagnostic> {
        Self::collect_registered(items.map(|item| (item, &item.header)))
    }
    fn collect_registered<'a>(
        items: impl Iterator<
            Item = (
                &'a crate::syntax::outline::ItemOutline,
                &'a crate::syntax::outline::HeaderRecipe,
            ),
        >,
    ) -> Result<Self, Diagnostic> {
        let mut cases = Vec::new();
        let mut bytes = 0usize;
        for (item, metadata) in items {
            if item.def_key.item_kind != ItemKind::Function
                || !metadata
                    .attributes
                    .iter()
                    .any(|a| a.name == "test" && a.class == AttributeClass::Test)
            {
                continue;
            }
            let mut ignored = false;
            let mut should_panic = false;
            let mut panic_contains = None;
            let attribute_error = (|| -> Result<(), Diagnostic> {
                for attr in &metadata.attributes {
                    match attr.name.as_str() {
                        "test" if attr.tokens.len() != 1 => {
                            return Err(fault("test attribute must have no arguments"));
                        }
                        "ignore" => {
                            ignored = true;
                            if attr.tokens.len() != 1
                                && (attr.tokens.len() != 3
                                    || attr.tokens[1].text != "="
                                    || crate::syntax::ast_adapter::string_value(
                                        &attr.tokens[2].text,
                                        metadata.profile.edition,
                                    )
                                    .is_none())
                            {
                                return Err(fault("invalid ignore attribute"));
                            }
                            ignored = true;
                        }
                        "should_panic" => {
                            should_panic = true;
                            if attr.tokens.len() != 1 {
                                let tokens = &attr.tokens;
                                let literal = if tokens.len() == 3 && tokens[1].text == "=" {
                                    &tokens[2].text
                                } else if matches!(tokens.len(), 6 | 7)
                                    && tokens[1].text == "("
                                    && tokens[2].text == "expected"
                                    && tokens[3].text == "="
                                    && tokens.last().is_some_and(|t| t.text == ")")
                                    && (tokens.len() == 6 || tokens[5].text == ",")
                                {
                                    &tokens[4].text
                                } else {
                                    return Err(fault("invalid expected-panic attribute"));
                                };
                                panic_contains = Some(
                                    crate::syntax::ast_adapter::string_value(
                                        literal,
                                        metadata.profile.edition,
                                    )
                                    .ok_or_else(|| {
                                        fault("expected panic must be a string literal")
                                    })?,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            })()
            .err();
            let name = item
                .scope
                .module_path
                .iter()
                .chain(std::iter::once(&item.def_key.declared_name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::");
            bytes = bytes
                .checked_add(name.capacity())
                .and_then(|n| n.checked_add(panic_contains.as_ref().map_or(0, String::capacity)))
                .and_then(|n| n.checked_add(256))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("test metadata budget exceeded"))?;
            if cases.len() >= 100_000 {
                return Err(fault("test count limit exceeded"));
            }
            cases.push(TestCase {
                name,
                definition: item.def_key.fingerprint(),
                expansion_ancestry: Vec::new(),
                body: item.body.as_ref().map(|b| b.key()),
                ignored,
                should_panic,
                panic_contains,
                attribute_error,
                unsupported_signature: item.scope.owner.is_some()
                    || item
                        .header
                        .tokens
                        .iter()
                        .take_while(|t| t.text != "fn")
                        .any(|t| t.text == "async"),
            });
        }
        cases.sort_by(|a, b| a.name.cmp(&b.name).then(a.definition.cmp(&b.definition)));
        if cases.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(fault("test names are ambiguous"));
        }
        Ok(Self {
            cases,
            seal: None,
            source_revision: 0,
            environment: None,
        })
    }
    pub(crate) fn from_declarations(
        entries: Vec<TestDeclaration<'_>>,
        seal: crate::db::seals::Seal,
        source_revision: u64,
        environment: std::sync::Arc<crate::expand::source_discovery::CompletedSourceEnvironment>,
    ) -> Result<Self, Diagnostic> {
        if !matches!(
            seal.domain(),
            crate::db::seals::SealDomain::TestCatalog { .. }
        ) {
            return Err(fault("test metadata requires a test catalog certificate"));
        }
        let identities = entries
            .iter()
            .map(|entry| {
                (
                    entry.item.def_key.fingerprint(),
                    (entry.identity, &entry.ancestry),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut catalog = Self::collect_registered(entries.iter().map(|entry| {
            (
                &entry.item,
                entry.registration.unwrap_or(&entry.item.header),
            )
        }))?;
        for case in &mut catalog.cases {
            let (definition, ancestry) = identities
                .get(&case.definition)
                .ok_or_else(|| fault("test declaration identity absent"))?;
            case.definition = *definition;
            case.expansion_ancestry
                .try_reserve_exact(ancestry.len())
                .map_err(|_| fault("test ancestry allocation"))?;
            case.expansion_ancestry.extend_from_slice(ancestry);
        }
        catalog.seal = Some(seal);
        catalog.source_revision = source_revision;
        catalog.environment = Some(environment);
        Ok(catalog)
    }
    pub fn seal(&self) -> Option<&crate::db::seals::Seal> {
        self.seal.as_ref()
    }
    /// Refuse stale source or expansion epochs before selection or guest entry.
    pub fn require_current(&self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        let Some(seal) = &self.seal else {
            return Ok(());
        };
        if self.source_revision != db.source_revision() {
            return Err(fault("test catalog source revision changed"));
        }
        match db.fetch(
            &crate::db::provider::SyntaxProvider,
            crate::db::key::QueryKey::DiscoverySeal(seal.key()),
        ) {
            crate::db::poll::QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(),crate::db::value::QueryValue::DiscoverySeal(actual) if actual.as_ref() == seal) => {
                Ok(())
            }
            _ => Err(fault(
                "test catalog certificate is absent, stale or incomplete",
            )),
        }
    }
    pub(crate) fn runtime_environment(
        &self,
    ) -> Option<&std::sync::Arc<crate::expand::source_discovery::CompletedSourceEnvironment>> {
        self.environment.as_ref()
    }
    pub fn cases(&self) -> &[TestCase] {
        &self.cases
    }
    pub fn select(&self, filter: Option<&str>, exact: bool) -> Vec<&TestCase> {
        self.cases
            .iter()
            .filter(|case| {
                filter.is_none_or(|filter| {
                    if exact {
                        case.name == filter
                    } else {
                        case.name.contains(filter)
                    }
                })
            })
            .collect()
    }
    /// Pure composition of name selection and final ignored metadata. Reuses
    /// the existing bounded selection vector, without another allocation.
    /// The caller retains `require_current` and its existing CLI flag mapping:
    /// only-ignored takes precedence over include-ignored for list and execution.
    pub fn select_with_ignore_policy(
        &self,
        filter: Option<&str>,
        exact: bool,
        policy: IgnorePolicy,
    ) -> Vec<&TestCase> {
        let mut selected = self.select(filter, exact);
        selected.retain(|case| match policy {
            IgnorePolicy::Exclude => !case.ignored,
            IgnorePolicy::Only => case.ignored,
            IgnorePolicy::Include => true,
        });
        selected
    }
}

impl std::fmt::Debug for TestCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestCatalog")
            .field("case_count", &self.cases.len())
            .field("seal", &self.seal)
            .finish()
    }
}

#[cfg(test)]
mod ignore_policy_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        source::snapshot::{CfgSet, SourceBlob},
        syntax::parse::ParsedFile,
    };

    const SOURCE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/tests-listing-format-default.rs"
    );
    const OUTPUT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/tests-listing-format-default.run.stdout"
    );

    // Pure metadata collector/selection component. The licensed input and
    // output stay unchanged; LRR uses its fixed Rust2024 profile and test cfg.
    // No expansion, seal, guest execution or full stock fixture pass is claimed.
    fn catalog(source: &str) -> TestCatalog {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let mut cfg = CfgSet::default();
        cfg.atoms.insert("test".into());
        let outline = parsed.outline(Digest([227; 32]), vec![], &cfg).unwrap();
        TestCatalog::collect(outline.items.iter()).unwrap()
    }
    fn names(cases: Vec<&TestCase>) -> Vec<&str> {
        cases.into_iter().map(|case| case.name.as_str()).collect()
    }
    #[test]
    fn upstream_listing_keeps_all_names_and_both_positive_and_negative_final_flags() {
        let catalog = catalog(SOURCE);
        let expected_names = OUTPUT
            .lines()
            .filter_map(|line| line.strip_suffix(": test"))
            .collect::<Vec<_>>();
        assert_eq!(expected_names, ["a_test", "m_test", "z_test"]);
        assert_eq!(names(catalog.select(None, false)), expected_names);
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|case| (
                    case.name.as_str(),
                    case.ignored,
                    case.should_panic,
                    case.panic_contains.as_deref(),
                    case.unsupported_signature,
                    case.attribute_error.is_some(),
                ))
                .collect::<Vec<_>>(),
            [
                ("a_test", false, false, None, false, false),
                ("m_test", false, false, None, false, false),
                ("z_test", true, false, None, false, false),
            ]
        );
    }
    #[test]
    fn source_backed_final_panic_flags_do_not_leak_between_catalog_entries() {
        // Owned metadata-only adaptation, outside the licensed fixture. Empty
        // bodies are not run: these controls concern final attribute metadata.
        assert_eq!(SOURCE.matches("fn a_test() {}").count(), 1);
        assert_eq!(SOURCE.matches("fn m_test() {}").count(), 1);
        let adapted = SOURCE
            .replacen("fn a_test() {}", "#[should_panic]\nfn a_test() {}", 1)
            .replacen(
                "fn m_test() {}",
                "#[should_panic(expected=\"needle\")]\nfn m_test() {}",
                1,
            );
        let catalog = catalog(&adapted);
        assert_eq!(
            catalog
                .cases()
                .iter()
                .map(|case| (
                    case.name.as_str(),
                    case.ignored,
                    case.should_panic,
                    case.panic_contains.as_deref(),
                    case.attribute_error.is_some(),
                ))
                .collect::<Vec<_>>(),
            [
                ("a_test", false, true, None, false),
                ("m_test", false, true, Some("needle"), false),
                ("z_test", true, false, None, false),
            ]
        );
    }
    #[test]
    fn expected_panic_string_forms_decode_the_same_final_message() {
        for attribute in [
            r##"#[should_panic = "needle\n\u{3bb}"]"##,
            "#[should_panic = r#\"needle\nλ\"#]",
            r##"#[should_panic(expected = "needle\n\u{3bb}")]"##,
            r##"#[should_panic(expected = "needle\n\u{3bb}",)]"##,
        ] {
            let adapted = SOURCE.replacen(
                "fn a_test() {}",
                &format!("{attribute}\nfn a_test() {{}}"),
                1,
            );
            let catalog = catalog(&adapted);
            let case = &catalog.cases()[0];
            assert_eq!(case.name, "a_test");
            assert!(case.should_panic);
            assert_eq!(case.panic_contains.as_deref(), Some("needle\nλ"));
            assert!(case.attribute_error.is_none());
            for other in &catalog.cases()[1..] {
                assert!(!other.should_panic);
                assert!(other.panic_contains.is_none());
            }
        }
    }
    #[test]
    fn name_value_expected_panic_rejects_non_string_without_faking_a_match() {
        let adapted = SOURCE.replacen("fn a_test() {}", "#[should_panic = 123]\nfn a_test() {}", 1);
        let catalog = catalog(&adapted);
        let case = &catalog.cases()[0];
        assert!(case.should_panic);
        assert!(case.panic_contains.is_none());
        let error = case.attribute_error.as_ref().unwrap();
        assert_eq!(error.code, "LRR-E-TEST-DISCOVERY");
        assert_eq!(error.message, "expected panic must be a string literal");
    }
    #[test]
    fn source_backed_ignore_policy_preserves_exact_filter_and_unfiltered_listing_meanings() {
        let catalog = catalog(SOURCE);
        assert_eq!(
            names(catalog.select(None, false)),
            ["a_test", "m_test", "z_test"]
        );
        assert!(catalog.select(Some("_test"), true).is_empty());
        assert_eq!(
            names(catalog.select(Some("_test"), false)),
            ["a_test", "m_test", "z_test"]
        );
        for (policy, expected) in [
            (IgnorePolicy::Exclude, vec!["a_test", "m_test"]),
            (IgnorePolicy::Only, vec!["z_test"]),
            (IgnorePolicy::Include, vec!["a_test", "m_test", "z_test"]),
        ] {
            assert_eq!(
                names(catalog.select_with_ignore_policy(None, false, policy)),
                expected
            );
            assert_eq!(
                names(catalog.select_with_ignore_policy(Some("_test"), false, policy)),
                expected
            );
            assert!(
                catalog
                    .select_with_ignore_policy(Some("_test"), true, policy)
                    .is_empty()
            );
            assert!(
                catalog
                    .select_with_ignore_policy(Some("absent"), false, policy)
                    .is_empty()
            );
        }
        assert!(
            catalog
                .select_with_ignore_policy(Some("z_test"), true, IgnorePolicy::Exclude)
                .is_empty()
        );
        assert_eq!(
            names(catalog.select_with_ignore_policy(Some("z_test"), true, IgnorePolicy::Only)),
            ["z_test"]
        );
        assert_eq!(
            names(catalog.select_with_ignore_policy(Some("z_test"), true, IgnorePolicy::Include)),
            ["z_test"]
        );
        assert!(
            catalog
                .select_with_ignore_policy(Some("a_test"), true, IgnorePolicy::Only)
                .is_empty()
        );
        assert_eq!(
            names(catalog.select_with_ignore_policy(Some("a_test"), true, IgnorePolicy::Exclude)),
            ["a_test"]
        );
    }
}
