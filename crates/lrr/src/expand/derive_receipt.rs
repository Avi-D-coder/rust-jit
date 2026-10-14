//! Immutable helper metadata; grants no preparation, execution or publication.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    frontend::generated_derive_exports::DeriveDeclarationRequest,
};
#[derive(Debug)]
pub struct PreparedDeriveDeclaration {
    request: Digest,
    demand: Digest,
    source_declaration: Digest,
    helpers: Vec<String>,
}
impl PreparedDeriveDeclaration {
    pub(crate) fn estimated_bytes(&self) -> Result<usize, Diagnostic> {
        let fault = || {
            Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "helper receipt size overflow".into(),
            )
        };
        let bytes = self
            .helpers
            .capacity()
            .checked_mul(std::mem::size_of::<String>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(fault)?;
        self.helpers.iter().try_fold(bytes, |n, name| {
            n.checked_add(name.capacity()).ok_or_else(fault)
        })
    }
    pub(crate) fn new(
        request: &DeriveDeclarationRequest,
        source_declaration: Digest,
        helpers: Vec<String>,
    ) -> Self {
        Self {
            request: request.fingerprint(),
            demand: request.demand(),
            source_declaration,
            helpers,
        }
    }
    pub fn helpers(&self) -> &[String] {
        &self.helpers
    }
    pub fn source_declaration(&self) -> Digest {
        self.source_declaration
    }
    pub fn validate_request(&self, request: &DeriveDeclarationRequest) -> Result<(), Diagnostic> {
        if self.request != request.fingerprint() || self.demand != request.demand() {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "derive declaration receipt belongs to another demand".into(),
            ));
        }
        Ok(())
    }
}
impl StableEncode for PreparedDeriveDeclaration {
    const DOMAIN: &'static str = "lrr/v2/prepared-derive-declaration-receipt";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.request);
        e.digest(self.demand);
        e.digest(self.source_declaration);
        e.u64(u64::try_from(self.helpers.len()).expect("bounded helper declarations"));
        for name in &self.helpers {
            e.string(name);
        }
    }
}
