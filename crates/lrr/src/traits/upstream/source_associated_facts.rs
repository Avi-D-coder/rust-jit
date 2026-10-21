//! Pure representation of complete source type-member identities and actual
//! alias argument namespaces. Neither is full member coverage or root admission.
use super::{
    identity::DefinitionId,
    model::{AllocateList, List},
    prepared::Cx,
    query_facts::QueryFact,
    source_alias_layouts::{AliasLayoutsPending, SourceAliasLayoutKind},
    source_definition_headers::{DefinitionHeaderPending, DefinitionHeaderRole, SourceHeaderParent},
    source_definition_semantics::{DefinitionSemanticPending, SourceAliasDomain, SourceSemantic},
    source_fact_bundle::{SourceDefinitionFacts, SourceFactBundle},
};
use crate::{
    core::digest::Digest,
    frontend::trait_associated_types::TraitAssociatedTypePending,
    traits::term::BoundKind,
};
use rustc_type_ir::AliasTermKind;

#[derive(Debug)]
pub(super) enum SourceAssociatedFactsPending {
    Definition(Digest),
    Header { definition: Digest, pending: DefinitionHeaderPending },
    SourceWorld(Digest),
    QueryNotDemanded { definition: Digest, query: &'static str },
    QueryIdentity(Digest),
    AssociatedTypes { definition: Digest, pending: TraitAssociatedTypePending },
    MemberRole(Digest),
    MemberParent(Digest),
    DuplicateMember(Digest),
    AliasDomain { definition: Digest, pending: DefinitionSemanticPending },
    AliasLayout { definition: Digest, pending: AliasLayoutsPending },
}
pub(super) type AliasLayouts<'cx> = Vec<(AliasTermKind<Cx<'cx>>, Vec<BoundKind>)>;

fn definition<'a>(cx: Cx<'_>, bundle: &'a SourceFactBundle, source: Digest)
    -> Result<&'a SourceDefinitionFacts, SourceAssociatedFactsPending>
{
    let actual = bundle.definitions().get(&source)
        .ok_or(SourceAssociatedFactsPending::Definition(source))?;
    if actual.id().source() != source || actual.id().world() != cx.services().source_world() {
        return Err(SourceAssociatedFactsPending::SourceWorld(source));
    }
    if let Some(pending) = actual.header().pending() {
        return Err(SourceAssociatedFactsPending::Header { definition: source, pending });
    }
    Ok(actual)
}

/// Q77 certifies only associated type declarations. Known(empty) requires its
/// genuine complete producer; function/constant coverage is a separate query.
pub(super) fn associated_types<'cx>(cx: Cx<'cx>, bundle: &SourceFactBundle, source: Digest)
    -> Result<QueryFact<&'cx [DefinitionId]>, SourceAssociatedFactsPending>
{
    let owner = definition(cx, bundle, source)?;
    if owner.header().role() != DefinitionHeaderRole::Trait {
        return Ok(QueryFact::NotApplicable);
    }
    let plan = bundle.trait_associated_types(source).ok_or(
        SourceAssociatedFactsPending::QueryNotDemanded { definition: source, query: "associated_types" }
    )?;
    if plan.key().trait_definition != source || plan.key().inventory != owner.header().key().inventory {
        return Err(SourceAssociatedFactsPending::QueryIdentity(source));
    }
    let members = plan.definitions().map_err(|pending|
        SourceAssociatedFactsPending::AssociatedTypes { definition: source, pending })?;
    let mut ids = Vec::new();
    for member in members {
        let actual = definition(cx, bundle, member)?;
        if actual.header().role() != DefinitionHeaderRole::TypeAlias {
            return Err(SourceAssociatedFactsPending::MemberRole(member));
        }
        if actual.header().source_parent() != &SourceHeaderParent::Associated(source) {
            return Err(SourceAssociatedFactsPending::MemberParent(member));
        }
        ids.push(actual.id());
    }
    ids.sort_unstable();
    if let Some(duplicate) = ids.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(SourceAssociatedFactsPending::DuplicateMember(duplicate[0].source()));
    }
    let retained: List<'cx, DefinitionId> = cx.list(ids);
    Ok(QueryFact::Known(retained.items()))
}

/// Every mapping below corresponds to a source-authenticated Q87 domain and
/// actual upstream parameter-kind order; no absent layout becomes zero args.
pub(super) fn alias_layouts<'cx>(cx: Cx<'cx>, bundle: &SourceFactBundle, source: Digest)
    -> Result<QueryFact<AliasLayouts<'cx>>, SourceAssociatedFactsPending>
{
    let actual = definition(cx, bundle, source)?;
    let domain = match actual.semantics().alias_domain() {
        SourceSemantic::Known(domain) => domain,
        SourceSemantic::NotApplicable => return Ok(QueryFact::NotApplicable),
        SourceSemantic::Pending(pending) => return Err(SourceAssociatedFactsPending::AliasDomain {
            definition: source, pending,
        }),
    };
    let plan = bundle.alias_layouts(source).ok_or(
        SourceAssociatedFactsPending::QueryNotDemanded { definition: source, query: "alias_layouts" }
    )?;
    if plan.key() != actual.header().key() {
        return Err(SourceAssociatedFactsPending::QueryIdentity(source));
    }
    let id = actual.id();
    use SourceAliasDomain as D;
    use SourceAliasLayoutKind as K;
    let requested = match domain {
        D::FreeType => vec![(K::FreeType, AliasTermKind::FreeTy { def_id: id })],
        D::FreeConst => vec![(K::FreeConst, AliasTermKind::FreeConst { def_id: id })],
        D::ProjectionType | D::TraitImplType => vec![(K::ProjectionType, AliasTermKind::ProjectionTy { def_id: id })],
        D::ProjectionConst => vec![(K::ProjectionConst, AliasTermKind::ProjectionConst { def_id: id })],
        D::InherentType => vec![(K::InherentType, AliasTermKind::InherentTy { def_id: id })],
        D::InherentConst => vec![
            (K::InherentConstSelf, AliasTermKind::InherentConstSelf { def_id: id }),
            (K::InherentConstImpl, AliasTermKind::InherentConstImpl { def_id: id }),
        ],
    };
    let mut layouts = Vec::new();
    for (kind, alias) in requested {
        let kinds = plan.layout(kind).map_err(|pending|
            SourceAssociatedFactsPending::AliasLayout { definition: source, pending })?;
        layouts.push((alias, kinds.to_vec()));
    }
    Ok(QueryFact::Known(layouts))
}
