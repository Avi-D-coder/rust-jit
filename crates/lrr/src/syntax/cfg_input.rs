//! Owned cfg facts from the pinned Rust syntax frontend, not expression evaluation.
use crate::{core::diagnostic::Diagnostic, source::snapshot::CfgSet};
use ra_ap_syntax::{AstNode, AstToken, Edition, ast};
fn fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TOOLCHAIN-CAPABILITY",
        "invalid captured cfg atom/key-value".into(),
    )
}
pub fn add(cfg: &mut CfgSet, input: &str) -> Result<(), Diagnostic> {
    if input.len() > 64 * 1024 {
        return Err(fault());
    }
    let attribute = format!("#[cfg({input})]");
    let parsed = ast::SourceFile::parse(
        &format!("{attribute} fn __lrr_cfg_capture() {{}}"),
        Edition::Edition2024,
    );
    if !parsed.errors().is_empty() {
        return Err(fault());
    }
    let attrs = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Attr::cast)
        .collect::<Vec<_>>();
    if attrs.len() != 1 || attrs[0].syntax().text().to_string() != attribute {
        return Err(fault());
    }
    let Some(ast::Meta::CfgMeta(meta)) = attrs[0].meta() else {
        return Err(fault());
    };
    if meta.comma_token().is_some() {
        return Err(fault());
    }
    let Some(ast::CfgPredicate::CfgAtom(atom)) = meta.cfg_predicate() else {
        return Err(fault());
    };
    let identifier = atom.ident_token().ok_or_else(fault)?;
    let name = identifier
        .text()
        .strip_prefix("r#")
        .unwrap_or(identifier.text())
        .to_owned();
    match (atom.eq_token(), atom.string_token()) {
        (None, None) => {
            cfg.atoms.insert(name);
        }
        (Some(_), Some(token)) => {
            let literal = ast::String::cast(token).ok_or_else(fault)?;
            let value = literal.value().map_err(|_| fault())?.into_owned();
            cfg.values.entry(name).or_default().insert(value);
        }
        _ => return Err(fault()),
    }
    Ok(())
}
