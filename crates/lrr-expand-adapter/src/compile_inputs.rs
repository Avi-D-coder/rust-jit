//! Pinned compiler-known compile-input facets. Discovery grants no call-site identity.
use crate::{AdapterError, UPSTREAM_COMMIT, token_wire::Key};
use hir_expand::{
    builtin::{EagerExpander, find_builtin_macro},
    name::Name,
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompileInputKind {
    Env,
    OptionEnv,
    Concat,
    Include,
    IncludeStr,
    IncludeBytes,
}
impl CompileInputKind {
    pub fn tag(self) -> u8 {
        match self {
            Self::Env => 0,
            Self::OptionEnv => 1,
            Self::Concat => 2,
            Self::Include => 3,
            Self::IncludeStr => 4,
            Self::IncludeBytes => 5,
        }
    }
}
#[derive(Debug, Clone)]
pub struct CompileInputDescriptor {
    name: &'static str,
    kind: CompileInputKind,
}
impl CompileInputDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn kind(&self) -> CompileInputKind {
        self.kind
    }
    pub fn definition_identity(&self) -> Key {
        let mut h = blake3::Hasher::new();
        for field in [
            "lrr/compile-input-builtin/v1",
            UPSTREAM_COMMIT,
            "Rust-1.99.0",
            self.name,
        ] {
            h.update(&(field.len() as u64).to_le_bytes());
            h.update(field.as_bytes());
        }
        h.update(&[self.kind.tag()]);
        Key(*h.finalize().as_bytes())
    }
}
pub fn descriptor(name: &str) -> Result<Option<CompileInputDescriptor>, AdapterError> {
    if name.len() > 64 * 1024 {
        return Err(AdapterError {
            code: "LRR-E-COMPILE-INPUT",
            message: "compile-input builtin name limit".into(),
        });
    }
    let Some(expander) = find_builtin_macro(&Name::new_root(name)).and_then(|e| e.right()) else {
        return Ok(None);
    };
    let (name, kind) = match expander {
        EagerExpander::Env => ("env", CompileInputKind::Env),
        EagerExpander::OptionEnv => ("option_env", CompileInputKind::OptionEnv),
        EagerExpander::Concat => ("concat", CompileInputKind::Concat),
        EagerExpander::Include => ("include", CompileInputKind::Include),
        EagerExpander::IncludeStr => ("include_str", CompileInputKind::IncludeStr),
        EagerExpander::IncludeBytes => ("include_bytes", CompileInputKind::IncludeBytes),
        _ => return Ok(None),
    };
    Ok(Some(CompileInputDescriptor { name, kind }))
}
