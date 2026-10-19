//! Symbolic PCs only. No installed code, guest frame, or interpreter state.
use crate::core::diagnostic::Diagnostic;
pub use crate::source::manifest::PcKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct SlotId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SlotKind {
    Value,
    Place,
    Packet,
    Guard,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Phase {
    Entry,
    Commit,
    Exit,
    Operand(u16),
}
impl Phase {
    pub fn code(self) -> Result<u16, Diagnostic> {
        match self {
            Self::Entry => Ok(0),
            Self::Commit => Ok(1),
            Self::Exit => Ok(2),
            Self::Operand(n) => {
                n.checked_add(3)
                    .filter(|code| *code < u16::MAX)
                    .ok_or(Diagnostic::new(
                        "LRR-E-BODY-INDEX",
                        "operand phase does not fit the PC encoding".into(),
                    ))
            }
        }
    }
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => Self::Entry,
            1 => Self::Commit,
            2 => Self::Exit,
            n => Self::Operand(n - 3),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_encoding_is_explicit_and_checked_at_the_u16_boundary() {
        for phase in [
            Phase::Entry,
            Phase::Commit,
            Phase::Exit,
            Phase::Operand(0),
            Phase::Operand(u16::MAX - 4),
        ] {
            assert_eq!(Phase::from_code(phase.code().unwrap()), phase);
        }
        assert!(Phase::Operand(u16::MAX - 3).code().is_err());
    }
}
