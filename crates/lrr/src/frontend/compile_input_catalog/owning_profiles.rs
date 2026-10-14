//! Closed catalogue identity used by owning-source compiler-facet preparation.
use super::*;
use crate::core::profile::ProfileIdentity;
impl CompileInputCatalog {
    pub(crate) fn owning_profile_identity(&self) -> Result<Digest, Diagnostic> {
        let mut e = StableEncoder::new("lrr/v1/prepared-compiler-facet-membership");
        e.digest(self.unit);
        e.u16(self.edition);
        e.map(&self.facades, |e, name, unit| {
            e.string(name);
            e.digest(*unit)
        });
        e.option(&self.allocation, |e, unit| e.digest(*unit));
        e.u64(self.definitions.len() as u64);
        for (id, (definition, descriptor)) in &self.definitions {
            definition.validate()?;
            e.digest(*id);
            e.digest(definition.identity);
            e.digest(definition.defining_unit);
            e.u16(definition.definition_edition);
            e.digest(descriptor.definition_identity());
            e.string(descriptor.name());
        }
        if let Some(attributes) = &self.attributes {
            e.bool(true);
            e.u64(attributes.definitions().count() as u64);
            for definition in attributes.definitions() {
                let descriptor = attributes
                    .select(definition)?
                    .ok_or_else(|| fault("compiler Attribute catalogue member absent"))?;
                e.digest(definition.identity);
                e.digest(definition.defining_unit);
                e.u16(definition.definition_edition);
                e.digest(descriptor.definition_identity());
                e.string(descriptor.name());
                e.u8(match descriptor.action() {
                    crate::expand::builtin_attributes::BuiltinAttributeAction::Derive => 0,
                    crate::expand::builtin_attributes::BuiltinAttributeAction::Test => 1,
                });
            }
        } else {
            e.bool(false)
        }
        Ok(e.finish())
    }
    pub(crate) fn canonical_standalone_member(
        &self,
        profile: &ProfileIdentity,
    ) -> Result<Option<Digest>, Diagnostic> {
        if self.unit != crate::facade::unit(profile, "core")
            || self.edition != profile.edition
            || self.attributes.is_none()
            || self.facades.is_empty()
        {
            return Ok(None);
        }
        let expected = Self::standalone(profile)?;
        let identity = self.owning_profile_identity()?;
        Ok((identity == expected.owning_profile_identity()?).then_some(identity))
    }
    pub(crate) fn has_standard_facets(&self) -> bool {
        !self.facades.is_empty() || self.attributes.is_some()
    }
    pub(crate) fn owning_profile_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(256)
            .ok_or_else(|| fault("compiler facet metadata overflow"))?;
        for name in self.facades.keys() {
            bytes = bytes
                .checked_add(name.capacity())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("compiler facade metadata overflow"))?;
        }
        bytes = bytes
            .checked_add(
                self.definitions
                    .len()
                    .checked_mul(256)
                    .ok_or_else(|| fault("compiler definition metadata overflow"))?,
            )
            .ok_or_else(|| fault("compiler definition metadata overflow"))?;
        if let Some(attrs) = &self.attributes {
            bytes = bytes
                .checked_add(
                    attrs
                        .definitions()
                        .count()
                        .checked_mul(256)
                        .ok_or_else(|| fault("compiler Attribute metadata overflow"))?,
                )
                .ok_or_else(|| fault("compiler Attribute metadata overflow"))?;
        }
        if bytes > 64 * 1024 {
            return Err(fault("compiler facet retained metadata limit"));
        }
        Ok(bytes)
    }
}
