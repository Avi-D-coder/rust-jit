//! Owned visibility ranges resolved from the pinned AST, without item lookup.
use super::{
    ast_adapter::{semantic_module_path, semantic_name},
    outline::{ScopeKey, Visibility},
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasModuleItem, HasVisibility},
};
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum VisibilityScope {
    Public,
    Within {
        unit: Digest,
        module_path: Vec<String>,
    },
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl StableEncode for VisibilityScope {
    const DOMAIN: &'static str = "lrr/v2/visibility-scope";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Public => e.u32(0),
            Self::Within { unit, module_path } => {
                e.u32(1);
                e.digest(*unit);
                e.sequence(module_path, |e, name| e.string(name));
            }
        }
    }
}
impl VisibilityScope {
    pub(crate) fn validate_for_declaration(&self, scope: &ScopeKey) -> Result<(), Diagnostic> {
        if let Self::Within { module_path, .. } = self {
            let canonical = semantic_module_path(module_path)
                .ok_or_else(|| error("invalid visibility range"))?;
            if &canonical != module_path {
                return Err(error("visibility range is not canonical"));
            }
        }
        if !self.allows(scope)? {
            return Err(error(
                "visibility range is not an ancestor of its declaration",
            ));
        }
        Ok(())
    }
    pub fn permits_reexport_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Public, _) => true,
            (
                Self::Within { unit, module_path },
                Self::Within {
                    unit: other_unit,
                    module_path: other_path,
                },
            ) => unit == other_unit && other_path.starts_with(module_path),
            _ => false,
        }
    }
    pub fn allows(&self, caller: &ScopeKey) -> Result<bool, Diagnostic> {
        let path = semantic_module_path(&caller.module_path)
            .ok_or_else(|| error("invalid visibility caller path"))?;
        Ok(match self {
            Self::Public => true,
            Self::Within { unit, module_path } => {
                *unit == caller.crate_key && path.starts_with(module_path)
            }
        })
    }
    pub fn for_declaration(
        visibility: &Visibility,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<Self, Diagnostic> {
        let original = semantic_module_path(&scope.module_path)
            .ok_or_else(|| error("invalid visibility declaration scope"))?;
        let mut path = original.clone();
        match visibility {
            Visibility::Public => return Ok(Self::Public),
            Visibility::Private => {}
            Visibility::Crate => path.clear(),
            Visibility::Super => {
                path.pop()
                    .ok_or_else(|| error("pub(super) escapes crate root"))?;
            }
            Visibility::Restricted(tokens) => {
                let mut text = String::new();
                for token in tokens {
                    text.len()
                        .checked_add(token.text.len())
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| error("visibility recipe text limit exceeded"))?;
                    text.try_reserve(token.text.len() + 1)
                        .map_err(|_| error("visibility recipe allocation failed"))?;
                    text.push_str(&token.text);
                    text.push(' ');
                }
                text.try_reserve(14)
                    .map_err(|_| error("visibility parser allocation failed"))?;
                text.push_str("fn value() {}");
                let edition = match edition {
                    2015 => ra_ap_syntax::Edition::Edition2015,
                    2018 => ra_ap_syntax::Edition::Edition2018,
                    2021 => ra_ap_syntax::Edition::Edition2021,
                    2024 => ra_ap_syntax::Edition::Edition2024,
                    _ => return Err(error("unsupported visibility edition")),
                };
                let parsed = ast::SourceFile::parse(&text, edition);
                if !parsed.errors().is_empty() {
                    return Err(error("invalid visibility recipe"));
                }
                let tree = parsed.tree();
                let mut items = tree.items();
                let Some(ast::Item::Fn(function)) = items.next() else {
                    return Err(error("visibility recipe has no declaration"));
                };
                if items.next().is_some() {
                    return Err(error("visibility recipe contains extra declarations"));
                }
                let visibility = function
                    .visibility()
                    .ok_or_else(|| error("visibility recipe is missing"))?;
                match visibility.kind() {
                    ast::VisibilityKind::Pub => return Ok(Self::Public),
                    ast::VisibilityKind::PubCrate => path.clear(),
                    ast::VisibilityKind::PubSelf => {}
                    ast::VisibilityKind::PubSuper => {
                        path.pop()
                            .ok_or_else(|| error("visibility restriction escapes crate root"))?;
                    }
                    ast::VisibilityKind::In(mut qualifier) => {
                        let mut segments = Vec::new();
                        loop {
                            let segment = qualifier
                                .segment()
                                .and_then(|s| s.kind())
                                .ok_or_else(|| error("visibility segment missing"))?;
                            segments
                                .try_reserve(1)
                                .map_err(|_| error("visibility path allocation failed"))?;
                            segments.push(segment);
                            if let Some(parent) = qualifier.qualifier() {
                                qualifier = parent;
                            } else {
                                break;
                            }
                        }
                        let mut segments = segments.into_iter().rev();
                        match segments.next() {
                            Some(ast::PathSegmentKind::CrateKw) => path.clear(),
                            Some(ast::PathSegmentKind::SelfKw) => {}
                            Some(ast::PathSegmentKind::SuperKw) => {
                                path.pop().ok_or_else(|| {
                                    error("visibility restriction escapes crate root")
                                })?;
                            }
                            _ => {
                                return Err(error(
                                    "visibility restriction must start with crate/self/super",
                                ));
                            }
                        }
                        let mut named = false;
                        for segment in segments {
                            match segment {
                                ast::PathSegmentKind::Name(name) => {
                                    named = true;
                                    let name = semantic_name(&name.syntax().text().to_string())
                                        .ok_or_else(|| error("invalid visibility name"))?;
                                    path.try_reserve(1)
                                        .map_err(|_| error("visibility scope allocation failed"))?;
                                    path.push(name);
                                }
                                ast::PathSegmentKind::SuperKw if !named => {
                                    path.pop().ok_or_else(|| {
                                        error("visibility restriction escapes crate root")
                                    })?;
                                }
                                _ => return Err(error("invalid visibility scope segment")),
                            }
                        }
                    }
                }
            }
        }
        if !original.starts_with(&path) {
            return Err(error("visibility restriction is not an ancestor scope"));
        }
        Ok(Self::Within {
            unit: scope.crate_key,
            module_path: path,
        })
    }
}
