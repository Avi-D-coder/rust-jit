//! Portable source identities for the upstream context. These identify facts;
//! they do not certify a declaration, current source world or executable call.
use crate::core::digest::{Digest, StableEncode, StableEncoder};
use rustc_type_ir::{
    FallibleTypeFolder, Interner, TypeFoldable, TypeFolder, TypeVisitable, TypeVisitor,
    VisitorResult,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct DefinitionId {
    world: Digest,
    source: Digest,
    local: bool,
}
impl DefinitionId {
    pub(crate) fn new(world: Digest, source: Digest, local: bool) -> Self {
        Self {
            world,
            source,
            local,
        }
    }
    pub(crate) fn world(self) -> Digest {
        self.world
    }
    pub(crate) fn source(self) -> Digest {
        self.source
    }
}
impl StableEncode for DefinitionId {
    const DOMAIN: &'static str = "lrr/v1/upstream-source-definition";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.world);
        e.digest(self.source);
        e.bool(self.local);
    }
}
impl<I: Interner> TypeVisitable<I> for DefinitionId {
    fn visit_with<V: TypeVisitor<I>>(&self, _: &mut V) -> V::Result {
        V::Result::output()
    }
}
impl<I: Interner> TypeFoldable<I> for DefinitionId {
    fn try_fold_with<F: FallibleTypeFolder<I>>(self, _: &mut F) -> Result<Self, F::Error> {
        Ok(self)
    }
    fn fold_with<F: TypeFolder<I>>(self, _: &mut F) -> Self {
        self
    }
}
impl<I: Interner<DefId = Self, LocalDefId = Self>> rustc_type_ir::inherent::DefId<I>
    for DefinitionId
{
    fn is_local(self) -> bool {
        self.local
    }
    fn as_local(self) -> Option<Self> {
        self.local.then_some(self)
    }
}
