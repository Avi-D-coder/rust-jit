use super::diagnostic::Diagnostic;

pub const RUNTIME_VERSION: &str = "0.2.0";
pub const SEMANTICS_PROFILE: &str = "lrr-2024-v2";
pub const DEFERRED_LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const GUEST_TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const GUEST_TARGET: &str = "x86_64-apple-darwin";
#[cfg(not(target_os = "macos"))]
pub const GUEST_TARGET: &str = DEFERRED_LINUX_TARGET;
#[cfg(all(
    target_os = "macos",
    not(any(target_arch = "aarch64", target_arch = "x86_64"))
))]
pub const GUEST_TARGET: &str = "unsupported-macos-architecture";
/// Default standalone edition; each selected unit retains its own edition.
pub const LIVE_EDITION: u16 = 2024;
pub fn is_supported_source_edition(edition: u16) -> bool {
    matches!(edition, 2015 | 2018 | 2021 | 2024)
}

/// Recognized preparation targets; recognition does not establish port coverage.
pub fn is_supported_preparation_target(target: &str) -> bool {
    matches!(
        target,
        "aarch64-apple-darwin" | "x86_64-apple-darwin" | "x86_64-unknown-linux-gnu"
    )
}

pub fn require_supported_host() -> Result<(), Diagnostic> {
    if cfg!(all(
        any(
            all(
                target_os = "macos",
                any(target_arch = "aarch64", target_arch = "x86_64")
            ),
            all(
                target_os = "linux",
                target_arch = "x86_64",
                target_env = "gnu"
            )
        ),
        target_endian = "little",
        target_pointer_width = "64"
    )) {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "LRR-E-HOST-TARGET",
            format!(
                "requires 64-bit little-endian macOS (ARM64/x86-64) or deferred Linux x86-64 GNU; detected {} {} with {}-bit pointers",
                std::env::consts::OS,
                std::env::consts::ARCH,
                usize::BITS
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_host_fails_closed() {
        assert_eq!(
            require_supported_host().is_ok(),
            cfg!(all(
                any(
                    all(
                        target_os = "macos",
                        any(target_arch = "aarch64", target_arch = "x86_64")
                    ),
                    all(
                        target_os = "linux",
                        target_arch = "x86_64",
                        target_env = "gnu"
                    )
                ),
                target_endian = "little",
                target_pointer_width = "64"
            ))
        );
        if let Err(error) = require_supported_host() {
            assert_eq!(error.code, "LRR-E-HOST-TARGET");
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ProfileIdentity {
    pub edition: u16,
    pub semantics: String,
}
impl ProfileIdentity {
    pub fn current() -> Self {
        Self {
            edition: LIVE_EDITION,
            semantics: SEMANTICS_PROFILE.into(),
        }
    }
    pub(crate) fn validate_source(&self) -> Result<(), Diagnostic> {
        if !is_supported_source_edition(self.edition) {
            return Err(Diagnostic::new(
                "LRR-E-TOOLCHAIN-CAPABILITY",
                "unknown source edition".into(),
            ));
        }
        if self.semantics != SEMANTICS_PROFILE {
            return Err(Diagnostic::new(
                "LRR-E-SEMANTICS-PROFILE",
                "source semantics profile is incompatible".into(),
            ));
        }
        Ok(())
    }
    pub fn validate_live(&self) -> Result<(), Diagnostic> {
        if !is_supported_source_edition(self.edition) {
            return Err(Diagnostic::new(
                "LRR-E-LIVE-EDITION",
                format!(
                    "live edition {} is unsupported; expected 2015, 2018, 2021 or 2024",
                    self.edition
                ),
            ));
        }
        if self.semantics != SEMANTICS_PROFILE {
            return Err(Diagnostic::new(
                "LRR-E-SEMANTICS-PROFILE",
                format!(
                    "profile {} is incompatible with {SEMANTICS_PROFILE}",
                    self.semantics
                ),
            ));
        }
        Ok(())
    }
}
impl crate::core::digest::StableEncode for ProfileIdentity {
    const DOMAIN: &'static str = "lrr/v2/semantic-profile";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.u16(self.edition);
        e.string(&self.semantics);
    }
}

#[cfg(test)]
mod edition_tests {
    use super::*;
    use crate::core::digest::StableEncode;
    #[test]
    fn live_edition_and_profile_are_checked_and_hashed() {
        let current = ProfileIdentity::current();
        current.validate_live().unwrap();
        let mut old = current.clone();
        old.edition = 2021;
        assert_ne!(current.fingerprint(), old.fingerprint());
        old.validate_live().unwrap();
        let mut identities = std::collections::BTreeSet::new();
        for edition in [2015, 2018, 2021, 2024] {
            let profile = ProfileIdentity {
                edition,
                ..current.clone()
            };
            profile.validate_live().unwrap();
            profile.validate_source().unwrap();
            assert!(identities.insert(profile.fingerprint()));
        }
        old.edition = 2099;
        assert_eq!(old.validate_live().unwrap_err().code, "LRR-E-LIVE-EDITION");
        old = current.clone();
        old.semantics = "lrr-2021-v1".into();
        assert_ne!(current.fingerprint(), old.fingerprint());
        assert_eq!(
            old.validate_live().unwrap_err().code,
            "LRR-E-SEMANTICS-PROFILE"
        );
    }

    #[test]
    fn revision_three_profile_is_rejected_and_has_distinct_identity() {
        let current = ProfileIdentity::current();
        let old = ProfileIdentity {
            edition: 2024,
            semantics: "lrr-2024-v1".into(),
        };
        assert_eq!(current.semantics, "lrr-2024-v2");
        assert_ne!(old.fingerprint(), current.fingerprint());
        assert_eq!(
            old.validate_live().unwrap_err().code,
            "LRR-E-SEMANTICS-PROFILE"
        );
    }
}
