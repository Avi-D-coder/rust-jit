//! Stable expansion and generated-item identity; source byte offsets are not item identities.
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum GrammarSlot {
    Items,
    Expression,
    Pattern,
    Type,
    TraitItems,
    ImplItems,
    Statements,
}
impl GrammarSlot {
    fn tag(self) -> u8 {
        match self {
            Self::Items => 0,
            Self::Expression => 1,
            Self::Pattern => 2,
            Self::Type => 3,
            Self::TraitItems => 4,
            Self::ImplItems => 5,
            Self::Statements => 6,
        }
    }
}
impl StableEncode for GrammarSlot {
    const DOMAIN: &'static str = "lrr/v2/grammar-slot";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u8(self.tag());
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum MacroIdentity {
    Declaration {
        definition: Digest,
    },
    Builtin {
        definition: Digest,
    },
    Procedural {
        artifact: Digest,
        export: String,
        kind: ProceduralKind,
    },
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ProceduralKind {
    Function,
    Attribute,
    Derive,
}
impl ProceduralKind {
    fn tag(self) -> u8 {
        match self {
            Self::Function => 0,
            Self::Attribute => 1,
            Self::Derive => 2,
        }
    }
}
/// Exact query identity. Epoch is mandatory only for procedural expansion.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpansionKey {
    pub macro_identity: MacroIdentity,
    pub invocation: Digest,
    pub input_tokens: Digest,
    pub consumer_compile_env: Digest,
    pub parent_context: Digest,
    pub definition_edition: u16,
    pub call_edition: u16,
    pub grammar: GrammarSlot,
    pub engine_version: Digest,
    pub procedural_epoch: Option<u64>,
}
impl ExpansionKey {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        let edition = |v| matches!(v, 2015 | 2018 | 2021 | 2024);
        if !edition(self.definition_edition)
            || !edition(self.call_edition)
            || matches!(self.macro_identity, MacroIdentity::Procedural { .. })
                != self.procedural_epoch.is_some()
        {
            return Err(error("invalid expansion edition/epoch"));
        }
        if let MacroIdentity::Procedural { export, .. } = &self.macro_identity
            && (export.is_empty() || export.len() > 64 * 1024 || export.contains('\0'))
        {
            return Err(error("invalid macro export identity"));
        }
        Ok(())
    }
}
impl StableEncode for ExpansionKey {
    const DOMAIN: &'static str = "lrr/v2/expansion-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match &self.macro_identity {
            MacroIdentity::Declaration { definition } => {
                e.u8(0);
                e.digest(*definition)
            }
            MacroIdentity::Builtin { definition } => {
                e.u8(1);
                e.digest(*definition)
            }
            MacroIdentity::Procedural {
                artifact,
                export,
                kind,
            } => {
                e.u8(2);
                e.digest(*artifact);
                e.string(export);
                e.u8(kind.tag())
            }
        }
        e.digest(self.invocation);
        e.digest(self.input_tokens);
        e.digest(self.consumer_compile_env);
        e.digest(self.parent_context);
        e.u16(self.definition_edition);
        e.u16(self.call_edition);
        e.u8(self.grammar.tag());
        e.digest(self.engine_version);
        e.option(&self.procedural_epoch, |e, v| e.u64(*v));
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedItemKey {
    pub expansion: Digest,
    pub structural_path: Vec<u32>,
    pub header: Digest,
}
impl GeneratedItemKey {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if self.structural_path.is_empty() || self.structural_path.len() > 256 {
            Err(error("generated item requires a bounded structural path"))
        } else {
            Ok(())
        }
    }
}
impl StableEncode for GeneratedItemKey {
    const DOMAIN: &'static str = "lrr/v2/generated-item";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.expansion);
        e.sequence(&self.structural_path, |e, v| e.u32(*v));
        e.digest(self.header);
    }
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-TOKEN-WIRE", message.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key() -> ExpansionKey {
        ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: Digest([1; 32]),
            },
            invocation: Digest([2; 32]),
            input_tokens: Digest([3; 32]),
            consumer_compile_env: Digest([4; 32]),
            parent_context: Digest([5; 32]),
            definition_edition: 2021,
            call_edition: 2024,
            grammar: GrammarSlot::Items,
            engine_version: Digest([6; 32]),
            procedural_epoch: None,
        }
    }
    #[test]
    fn expansion_identity_covers_context_inputs_editions_slot_engine_and_proc_epoch() {
        let original = key();
        original.validate().unwrap();
        let hash = original.fingerprint();
        let variants = [
            ExpansionKey {
                invocation: Digest([9; 32]),
                ..original.clone()
            },
            ExpansionKey {
                input_tokens: Digest([9; 32]),
                ..original.clone()
            },
            ExpansionKey {
                consumer_compile_env: Digest([9; 32]),
                ..original.clone()
            },
            ExpansionKey {
                parent_context: Digest([9; 32]),
                ..original.clone()
            },
            ExpansionKey {
                definition_edition: 2024,
                ..original.clone()
            },
            ExpansionKey {
                call_edition: 2021,
                ..original.clone()
            },
            ExpansionKey {
                grammar: GrammarSlot::Expression,
                ..original.clone()
            },
            ExpansionKey {
                engine_version: Digest([9; 32]),
                ..original.clone()
            },
            ExpansionKey {
                macro_identity: MacroIdentity::Builtin {
                    definition: Digest([1; 32]),
                },
                ..original.clone()
            },
        ];
        for v in variants {
            v.validate().unwrap();
            assert_ne!(hash, v.fingerprint())
        }
        let mut proc = ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact: Digest([1; 32]),
                export: "derive_this".into(),
                kind: ProceduralKind::Derive,
            },
            ..original.clone()
        };
        assert!(proc.validate().is_err());
        proc.procedural_epoch = Some(1);
        proc.validate().unwrap();
        let first = proc.fingerprint();
        proc.procedural_epoch = Some(2);
        assert_ne!(first, proc.fingerprint());
        assert!(
            ExpansionKey {
                procedural_epoch: Some(1),
                ..original
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn generated_item_identity_uses_structural_path_and_header() {
        let item = GeneratedItemKey {
            expansion: key().fingerprint(),
            structural_path: vec![0, 2],
            header: Digest([7; 32]),
        };
        item.validate().unwrap();
        let copy: GeneratedItemKey =
            serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
        assert_eq!(item.fingerprint(), copy.fingerprint());
        assert_ne!(
            item.fingerprint(),
            GeneratedItemKey {
                structural_path: vec![0, 3],
                ..item.clone()
            }
            .fingerprint()
        );
        assert_ne!(
            item.fingerprint(),
            GeneratedItemKey {
                header: Digest([8; 32]),
                ..item.clone()
            }
            .fingerprint()
        );
        assert!(
            GeneratedItemKey {
                structural_path: vec![],
                ..item
            }
            .validate()
            .is_err()
        );
    }
}
