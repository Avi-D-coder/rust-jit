use crate::core::{diagnostic::Diagnostic, profile};

/// Development entry point; missing execution functionality always fails.
pub fn entry(program: &str) -> i32 {
    if let Err(error) = profile::require_supported_host() {
        eprintln!("{error}");
        return 2;
    }
    let error = Diagnostic {
        code: "LRR-E-NOT-YET-IMPLEMENTED",
        message: format!("{program}: development bootstrap; execution is not implemented"),
    };
    eprintln!("{error}");
    2
}

#[cfg(test)]
mod tests {
    #[test]
    fn development_entry_fails_closed() {
        assert_eq!(super::entry("bootstrap-test"), 2);
    }
}
