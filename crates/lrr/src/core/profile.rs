use super::diagnostic::Diagnostic;

pub const RUNTIME_VERSION: &str = "0.1.0";
pub const SEMANTICS_PROFILE: &str = "lrr-2024-v1";
pub const GUEST_TARGET: &str = "x86_64-unknown-linux-gnu";
pub const LIVE_EDITION: u16 = 2024;

pub fn require_supported_host() -> Result<(), Diagnostic> {
    if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        target_endian = "little",
        target_pointer_width = "64"
    )) {
        Ok(())
    } else {
        Err(Diagnostic {
            code: "LRR-E-HOST-TARGET",
            message: format!(
                "requires {GUEST_TARGET}; detected {} {} with {}-bit pointers",
                std::env::consts::OS,
                std::env::consts::ARCH,
                usize::BITS
            ),
        })
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
                target_os = "linux",
                target_arch = "x86_64",
                target_env = "gnu",
                target_endian = "little",
                target_pointer_width = "64"
            ))
        );
        if let Err(error) = require_supported_host() {
            assert_eq!(error.code, "LRR-E-HOST-TARGET");
        }
    }
}
