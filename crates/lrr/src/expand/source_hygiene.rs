//! Pinned adapter root-context preparation outside semantic query providers.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    syntax::token_wire::{
        ContextRecord, Key, NameResolutionContext, ResolutionContext, TokenForest,
    },
};
pub(crate) struct SourceHygiene {
    pub context: Key,
    pub contexts: Vec<ContextRecord>,
    pub rows: Vec<ResolutionContext>,
    pub names: Vec<NameResolutionContext>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-HYGIENE", message.into())
}

/// Read a captured unqualified path without exposing adapter token nodes to the frontend.
pub(crate) fn declaration_macro_name(
    path: &lrr_expand_adapter::token_wire::TokenForest,
    edition: u16,
) -> Result<(crate::expand::scopes::MacroName, Key), Diagnostic> {
    use lrr_expand_adapter::token_wire::TokenTree;
    let roots = match path.roots() {
        [TokenTree::Group { children, .. }] => children.as_slice(),
        roots => roots,
    };
    let [TokenTree::Ident { text, raw, span }] = roots else {
        return Err(error(
            "captured declaration macro path is not one identifier",
        ));
    };
    let name = crate::expand::scopes::MacroName::parse(
        &if *raw {
            format!("r#{text}")
        } else {
            text.clone()
        },
        edition,
    )?;
    let context = path
        .spans()
        .iter()
        .find(|s| s.key() == *span)
        .ok_or_else(|| error("captured declaration macro path span is absent"))?
        .context;
    Ok((name, Key(context.0)))
}
pub(crate) fn source_root(unit: Digest, edition: u16) -> Result<SourceHygiene, Diagnostic> {
    use lrr_expand_adapter::{hygiene::HygieneStore, token_wire as adapter};
    let edition = match edition {
        2015 => adapter::Edition::E2015,
        2018 => adapter::Edition::E2018,
        2021 => adapter::Edition::E2021,
        2024 => adapter::Edition::E2024,
        _ => return Err(error("unknown source root edition")),
    };
    let map = |e: lrr_expand_adapter::AdapterError| error(&e.message);
    let mut store = HygieneStore::default();
    let context = store.root(adapter::Key(unit.0), edition).map_err(map)?;
    let rows = store.resolution_contexts(&[context]).map_err(map)?;
    let names = store.name_contexts(&[context]).map_err(map)?;
    let bytes = adapter::TokenForest::new(vec![], store.records(), vec![])
        .map_err(map)?
        .encode()
        .map_err(map)?;
    let forest = TokenForest::decode(&bytes).map_err(|e| error(&e.message))?;
    let rows = serde_json::from_slice(
        &serde_json::to_vec(&rows).map_err(|_| error("source root row encoding failed"))?,
    )
    .map_err(|_| error("source root row decoding failed"))?;
    let names = serde_json::from_slice(
        &serde_json::to_vec(&names).map_err(|_| error("source name row encoding failed"))?,
    )
    .map_err(|_| error("source name row decoding failed"))?;
    Ok(SourceHygiene {
        context: Key(context.0),
        contexts: forest.contexts().to_vec(),
        rows,
        names,
    })
}

/// Normalize immutable output contexts through the pinned adapter only.
pub(crate) fn output_facts(
    tokens: &TokenForest,
) -> Result<crate::frontend::hygiene::HygieneFacts, Diagnostic> {
    let wire = tokens.encode().map_err(|e| error(&e.message))?;
    let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
        .map_err(|e| error(&e.message))?;
    let mut store = lrr_expand_adapter::hygiene::HygieneStore::default();
    store
        .import(tokens.contexts())
        .map_err(|e| error(&e.message))?;
    let demanded = tokens
        .contexts()
        .iter()
        .map(|c| c.key())
        .collect::<Vec<_>>();
    let rows = store
        .resolution_contexts(&demanded)
        .map_err(|e| error(&e.message))?;
    let names = store
        .name_contexts(&demanded)
        .map_err(|e| error(&e.message))?;
    let wire = lrr_expand_adapter::token_wire::TokenForest::new(vec![], store.records(), vec![])
        .and_then(|f| f.encode())
        .map_err(|e| error(&e.message))?;
    let contexts = TokenForest::decode(&wire).map_err(|e| error(&e.message))?;
    crate::frontend::hygiene::HygieneFacts::new(
        contexts.contexts().to_vec(),
        rows.into_iter()
            .map(|r| ResolutionContext {
                original: Key(r.original.0),
                item: Key(r.item.0),
                local: Key(r.local.0),
            })
            .collect(),
    )?
    .with_name_contexts(
        names
            .into_iter()
            .map(|r| NameResolutionContext {
                original: Key(r.original.0),
                item: crate::syntax::token_wire::NameContextKey(Key(r.item.0.0)),
                local: crate::syntax::token_wire::NameContextKey(Key(r.local.0.0)),
            })
            .collect(),
    )
}
