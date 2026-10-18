//! Selected field occurrence consumes the shared current owning-body proof.
use crate::{
    core::diagnostic::Diagnostic,
    db::{memo::Database, poll::QueryPoll},
    frontend::source_body_context::{self, AdmittedBodyContext},
    ir::pc::{NodeId, PcKey},
    source::changes::PreparedSourceView,
    syntax::{body_index::BodyIndex, fields::NamedFieldSite, outline::ScopeKey},
};
pub(super) struct FieldSource {
    site: NamedFieldSite,
    owner: std::sync::Arc<AdmittedBodyContext>,
}
impl FieldSource {
    pub(super) fn member(&self) -> &str {
        self.site.member()
    }
    pub(super) fn scope(&self) -> Result<&ScopeKey, Diagnostic> {
        Ok(self.owner.scope())
    }
}
pub(super) fn capture(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<FieldSource> {
    let owner = match source_body_context::frame_context(db, sources, index, pc) {
        QueryPoll::Ready(Ok(owner)) => owner,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let site = match owner
        .parsed()
        .named_field_site(owner.body(), owner.index(), NodeId(pc.node))
    {
        Ok(site) => site,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if let Err(error) = owner.require_identifier_origin(db, sources, site.range(), site.member()) {
        return QueryPoll::Ready(Err(error));
    }
    QueryPoll::Ready(Ok(FieldSource { site, owner }))
}
