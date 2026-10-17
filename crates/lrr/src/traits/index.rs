//! Checked active source headers from an actual scope inventory. A scope inventory
//! is not global impl completeness and cannot answer Drop/NoDrop or applicability.
use super::header::{SourceImplHeader, same_requirements};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::{SealDomain, SealKey},
        value::QueryValue,
    },
    frontend::impl_index::ImplHeaderIndex,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct HeaderLocation {
    source: u32,
    header: u32,
}
/// Retains the genuine query inventory and locators, without cloning source trees,
/// header token buffers or associated bodies. No public constructor accepts an
/// inventory, caller-selected header, trait name or completeness assertion.
pub struct PreparedScopeImplHeaders {
    key: SealKey,
    inventory: Arc<ImplHeaderIndex>,
    locations: Vec<HeaderLocation>,
    inventory_fingerprint: Digest,
    fingerprint: Digest,
    exclusive_bytes: usize,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-HEADER", message.into())
}
fn resource() -> Diagnostic {
    crate::runtime::arena::resource()
}
impl PreparedScopeImplHeaders {
    /// Cold preparation through the existing pure query and its actual dependencies.
    pub fn prepare(db: &mut Database, key: &SealKey) -> QueryPoll<Self> {
        if !matches!(&key.domain, SealDomain::Scope { .. }) {
            return QueryPoll::Ready(Err(error("source headers require a scope inventory")));
        }
        match db.fetch(
            &SyntaxProvider,
            QueryKey::ScopeDeclarationInventory(key.clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => {
                let QueryValue::ScopeDeclarationInventory {
                    key: actual,
                    inventory,
                } = outcome.value.as_ref()
                else {
                    return QueryPoll::Ready(Err(error("source inventory query variant differs")));
                };
                if actual != key {
                    return QueryPoll::Ready(Err(error("source inventory query owner differs")));
                }
                QueryPoll::Ready(Self::from_current_inventory(
                    key.clone(),
                    Arc::clone(inventory),
                    outcome.fingerprint,
                ))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
        }
    }
    fn from_current_inventory(
        key: SealKey,
        inventory: Arc<ImplHeaderIndex>,
        inventory_fingerprint: Digest,
    ) -> Result<Self, Diagnostic> {
        let headers = inventory.headers();
        let scans = inventory.scan_handles();
        if headers
            .windows(2)
            .any(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(error("source inventory count/order/identity conflict"));
        }
        // Temporary Option rows and final rows coexist until publication. Charge
        // both before reserving; all indices are checked before narrowing to u32.
        let _preflight_metadata = std::mem::size_of::<Self>()
            .checked_add(
                headers
                    .len()
                    .checked_mul(
                        std::mem::size_of::<Option<HeaderLocation>>()
                            + std::mem::size_of::<HeaderLocation>(),
                    )
                    .ok_or_else(resource)?,
            )
            .ok_or_else(resource)?;
        let mut locations = Vec::new();
        locations
            .try_reserve_exact(headers.len())
            .map_err(|_| resource())?;
        let temporary_bytes = locations
            .capacity()
            .checked_mul(std::mem::size_of::<Option<HeaderLocation>>())
            .ok_or_else(resource)?;
        locations.resize(headers.len(), None);
        let mut inspected = 0usize;
        for (source_ordinal, scan) in scans.iter().enumerate() {
            inspected = inspected
                .checked_add(scan.impls().len())
                .ok_or_else(resource)?;
            for (header_ordinal, actual) in scan.impls().iter().enumerate() {
                let Ok(ordinal) = headers.binary_search_by_key(&actual.identity, |h| h.identity)
                else {
                    continue;
                }; // A replaced/inactive source row is not revived.
                if !same_requirements(&headers[ordinal], actual) {
                    return Err(error("active impl header differs from retained source"));
                }
                let location = HeaderLocation {
                    source: u32::try_from(source_ordinal).map_err(|_| resource())?,
                    header: u32::try_from(header_ordinal).map_err(|_| resource())?,
                };
                if locations[ordinal].replace(location).is_some() {
                    return Err(error("active impl header has duplicate source owners"));
                }
            }
        }
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(headers.len())
            .map_err(|_| resource())?;
        // Account actual reported capacities, including both buffers while they
        // coexist. Failed allocation/admission publishes no partial header set.
        let prepared_bytes = prepared
            .capacity()
            .checked_mul(std::mem::size_of::<HeaderLocation>())
            .ok_or_else(resource)?;
        std::mem::size_of::<Self>()
            .checked_add(temporary_bytes)
            .and_then(|n| n.checked_add(prepared_bytes))
            .ok_or_else(resource)?;
        for location in locations {
            prepared.push(location.ok_or_else(|| error("active impl header source absent"))?);
        }
        let exclusive_bytes = std::mem::size_of::<Self>()
            .checked_add(
                prepared
                    .capacity()
                    .checked_mul(std::mem::size_of::<HeaderLocation>())
                    .ok_or_else(resource)?,
            )
            .ok_or_else(resource)?;
        let mut encoded = StableEncoder::new("lrr/v1/prepared-scope-source-impl-headers");
        encoded.record(&key);
        encoded.digest(inventory_fingerprint);
        encoded.u64(prepared.len() as u64);
        for (header, location) in headers.iter().zip(&prepared) {
            encoded.digest(header.identity);
            encoded.digest(header.header_identity);
            encoded.u32(location.source);
            encoded.u32(location.header);
        }
        Ok(Self {
            key,
            inventory,
            locations: prepared,
            inventory_fingerprint,
            fingerprint: encoded.finish(),
            exclusive_bytes,
        })
    }
    pub fn key(&self) -> &SealKey {
        &self.key
    }
    pub fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    /// Exclusive rows only; the actual inventory/scan allocations remain shared
    /// roots and must be retained/accounted by the consumer's existing query pins.
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub fn inventory_root(&self) -> &Arc<ImplHeaderIndex> {
        &self.inventory
    }
    pub fn len(&self) -> usize {
        self.locations.len()
    }
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }
    pub fn header(&self, identity: Digest) -> Option<SourceImplHeader<'_>> {
        let ordinal = self
            .inventory
            .headers()
            .binary_search_by_key(&identity, |h| h.identity)
            .ok()?;
        let location = self.locations.get(ordinal)?;
        let source = self
            .inventory
            .scan_handles()
            .get(location.source as usize)?;
        let header = source.impls().get(location.header as usize)?;
        Some(SourceImplHeader::joined(source, header))
    }
    /// Replays the real preparation dependency. Not an installed-operation guard.
    pub fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match db.fetch(
            &SyntaxProvider,
            QueryKey::ScopeDeclarationInventory(self.key.clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::ScopeDeclarationInventory { key, .. }
                    if key == &self.key && outcome.fingerprint == self.inventory_fingerprint =>
                {
                    QueryPoll::Ready(Ok(()))
                }
                _ => QueryPoll::Ready(Err(error("prepared impl inventory changed"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
        }
    }
}
impl std::fmt::Debug for PreparedScopeImplHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedScopeImplHeaders")
            .field("key", &self.key)
            .field("headers", &self.len())
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::compile_input_catalog::CompileInputCatalog,
        source::manifest::Namespace,
    };
    const FIELDS: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
    );
    const DESTRUCTORS: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/destructor-run-for-let-ignore-6892.rs"
    );
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("source impl preparation: {error:?}"),
            QueryPoll::NeedInput(request) => panic!("source impl preparation demand: {request:?}"),
        }
    }
    fn prepared(text: &str, edition: u16) -> (Database, PreparedScopeImplHeaders) {
        let (provider, _, inputs) =
            crate::expand::source_declarative::assembly::tests::world(text, edition);
        let source = provider.module_source().share();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"source-impl-header-authority-control"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"source-impl-header-compiler-control"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    discovery.publish(&mut db).unwrap();
                    break;
                }
                _ => panic!("source impl header discovery requires an unprepared producer"),
            }
        }
        let headers = ready(PreparedScopeImplHeaders::prepare(&mut db, discovery.key()));
        (db, headers)
    }
    #[test]
    fn unchanged_cleanup_and_user_drop_headers_retain_exact_source_requirements_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, empty) = prepared(FIELDS, edition);
            assert!(empty.is_empty()); // Scope-local facts; deliberately no NoDrop API.
            ready(empty.require_current(&mut db));
            let (mut db, prepared) = prepared(DESTRUCTORS, edition);
            assert_eq!(prepared.len(), 4);
            for active in prepared.inventory_root().headers() {
                let header = prepared.header(active.identity).unwrap();
                assert!(same_requirements(header.requirements(), active));
                assert_eq!(header.source().parse().profile.edition, edition);
                assert_eq!(header.requirements().associated.len(), 1);
                assert!(!header.requirements().associated[0].header.is_empty());
                // Even a copied identity cannot excuse altered associated source.
                let mut altered = active.clone();
                altered.associated[0].range.end += 1;
                assert!(!same_requirements(active, &altered));
            }
            ready(prepared.require_current(&mut db));
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                prepared.require_current(&mut db),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
    #[test]
    fn scope_header_preparation_rejects_caller_catalogue_keys() {
        let mut db = Database::default();
        let key = SealKey {
            domain: SealDomain::ImplCatalog {
                id: Digest::of_bytes(b"not-a-source-catalogue"),
            },
            unit_graph: Digest::of_bytes(b"not-a-configured-graph"),
            epoch: 1,
        };
        match PreparedScopeImplHeaders::prepare(&mut db, &key) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-IMPL-HEADER"),
            other => panic!("scope inventory promoted to impl completeness: {other:?}"),
        }
    }
}
