//! Closed synthetic-core inherent declarations, never a host/sysroot selection.
//! The pinned f32/f64 is_nan contract is `const fn(self) -> bool { self != self }`.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::op::PrimitiveType,
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PRIMITIVE-FLOAT-METHOD", message.into())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PrimitiveFloatReceiverKind {
    Value,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PrimitiveFloatMethodBody {
    SelfNotEqual,
}
/// An immutable declaration receipt. It does not prove a caller's receiver type,
/// current source scope, physical value, or permission to evaluate an expression.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PrimitiveFloatMethod {
    profile: Arc<ProfileIdentity>,
    core: Digest,
    receiver: PrimitiveType,
    signature: Digest,
    body: Digest,
    identity: Digest,
}
impl PrimitiveFloatMethod {
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn receiver(&self) -> PrimitiveType {
        self.receiver
    }
    pub(crate) fn receiver_kind(&self) -> PrimitiveFloatReceiverKind {
        PrimitiveFloatReceiverKind::Value
    }
    pub(crate) fn result(&self) -> PrimitiveType {
        PrimitiveType::Bool
    }
    pub(crate) fn body(&self) -> PrimitiveFloatMethodBody {
        PrimitiveFloatMethodBody::SelfNotEqual
    }
    pub(crate) fn require_profile(&self, profile: &ProfileIdentity) -> Result<(), Diagnostic> {
        profile.validate_source()?;
        if self.profile.as_ref() != profile || self.core != super::unit(profile, "core") {
            return Err(fault("primitive method owning profile/core differs"));
        }
        Ok(())
    }
    /// A retained clone shares its immutable profile; charge conservatively even
    /// when another receipt already retains that allocation.
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<ProfileIdentity>())
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(self.profile.semantics.capacity()))
            .ok_or_else(|| fault("primitive method receipt metadata overflow"))
    }
}
/// Fixed compiler-owned inventory. Only actual standard-facade preparation
/// installs it in a runtime catalogue; macro-only catalogues retain None.
#[derive(Debug, Clone)]
pub(crate) struct PrimitiveFloatMethods {
    profile: Arc<ProfileIdentity>,
    members: [PrimitiveFloatMethod; 2],
    identity: Digest,
}
impl PrimitiveFloatMethods {
    pub(crate) fn for_profile(profile: &ProfileIdentity) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        let profile = Arc::new(profile.clone());
        let core = super::unit(&profile, "core");
        let members = [PrimitiveType::F32, PrimitiveType::F64].map(|receiver| {
            let mut signature = StableEncoder::new("lrr/v1/primitive-float-method-signature");
            signature.digest(core);
            signature.record(profile.as_ref());
            signature.u32(receiver.tag());
            signature.string("is_nan");
            signature.u8(0); // By-value self, evaluated once; no explicit arguments.
            signature.u32(0);
            signature.u32(PrimitiveType::Bool.tag());
            signature.bool(true); // const, no generic or region parameters.
            let signature = signature.finish();
            let mut body = StableEncoder::new("lrr/v1/primitive-float-method-body");
            body.digest(signature);
            body.u8(0); // Immutable receiver != same receiver, no external effects.
            let body = body.finish();
            let mut identity = StableEncoder::new("lrr/v1/primitive-float-method-member");
            identity.digest(core);
            identity.record(profile.as_ref());
            identity.u32(receiver.tag());
            identity.string("is_nan");
            identity.digest(signature);
            identity.digest(body);
            PrimitiveFloatMethod {
                profile: profile.clone(),
                core,
                receiver,
                signature,
                body,
                identity: identity.finish(),
            }
        });
        let mut identity = StableEncoder::new("lrr/v1/primitive-float-method-facet");
        identity.record(profile.as_ref());
        identity.digest(core);
        for member in &members {
            identity.digest(member.identity());
        }
        Ok(Self {
            profile,
            members,
            identity: identity.finish(),
        })
    }
    pub(crate) fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn lookup(
        &self,
        receiver: PrimitiveType,
        name: &str,
    ) -> Option<&PrimitiveFloatMethod> {
        if name != "is_nan" {
            return None;
        }
        self.members
            .iter()
            .find(|member| member.receiver == receiver)
    }
    pub(crate) fn require_member(&self, member: &PrimitiveFloatMethod) -> Result<(), Diagnostic> {
        member.require_profile(&self.profile)?;
        if self.lookup(member.receiver, "is_nan") != Some(member) {
            return Err(fault(
                "primitive method signature/body/member identity differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<ProfileIdentity>())
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(self.profile.semantics.capacity()))
            .ok_or_else(|| fault("primitive method facet metadata overflow"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forged_signature_body_core_receiver_or_profile_cannot_reuse_member_identity() {
        let facet = PrimitiveFloatMethods::for_profile(&ProfileIdentity::current()).unwrap();
        let actual = facet.lookup(PrimitiveType::F64, "is_nan").unwrap();
        let mut forged = actual.clone();
        forged.signature = Digest([202; 32]);
        assert!(facet.require_member(&forged).is_err());
        forged = actual.clone();
        forged.body = Digest([203; 32]);
        assert!(facet.require_member(&forged).is_err());
        forged = actual.clone();
        forged.core = Digest([204; 32]);
        assert!(facet.require_member(&forged).is_err());
        forged = actual.clone();
        forged.receiver = PrimitiveType::F32;
        assert!(facet.require_member(&forged).is_err());
        forged = actual.clone();
        forged.profile = Arc::new(ProfileIdentity {
            edition: 2015,
            ..ProfileIdentity::current()
        });
        assert!(facet.require_member(&forged).is_err());
        facet.require_member(actual).unwrap();
    }
}
