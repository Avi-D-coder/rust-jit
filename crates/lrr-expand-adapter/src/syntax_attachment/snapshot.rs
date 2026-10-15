//! Immutable owned syntax plus the closed source/hygiene records it references.
//! This transports syntax; it grants no expansion, publication or VM authority.
use super::*;
use std::collections::BTreeSet;

pub struct SyntaxSnapshot {
    syntax: DeclarationSyntax,
    origins: TokenForest,
    identity: Key,
    grammar: DeclarationGrammar,
}
impl SyntaxSnapshot {
    pub fn syntax(&self) -> &DeclarationSyntax {
        &self.syntax
    }
    /// This forest has no syntax roots. Its records resolve the origin keys of
    /// the owned syntax events, including carried caller semicolons.
    pub fn origin_records(&self) -> &TokenForest {
        &self.origins
    }
    pub fn identity(&self) -> Key {
        self.identity
    }
    pub fn grammar(&self) -> DeclarationGrammar {
        self.grammar
    }
}
impl SyntaxAttachment {
    pub fn snapshot(&self) -> Result<SyntaxSnapshot, AdapterError> {
        let mut used = BTreeSet::new();
        for (_, origin) in self.origins.iter() {
            used.insert(self.store.key(origin)?);
        }
        let origins = self.store.export_records(vec![], used)?;
        if origins.allocated_bytes()? > MAX_RETAINED_INPUT {
            return Err(error("syntax snapshot origin retention budget"));
        }
        Ok(SyntaxSnapshot {
            syntax: self.materialize()?,
            origins,
            identity: self.syntax_identity()?,
            grammar: self.grammar,
        })
    }
}
