//! Bounded immutable attachment data from an actual issued request. No producer
//! body, evaluator, namespace authority or captured source is manufactured here.
use super::{SourceDeclarativeRequest, adapter, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::identity::{ExpansionKey, MacroIdentity},
};
use lrr_expand_adapter::token_wire::{Delimiter, Key, Spacing, TokenTree};
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SyntaxRequestReceipt {
    pub(super) key: ExpansionKey,
    pub(super) site: Key,
    pub(super) roots: Digest,
    pub(super) definition: Option<crate::expand::scopes::MacroDefinition>,
    input: SyntaxInput,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SyntaxInput {
    DelimitedCall,
    FunctionArguments,
    ItemTransform,
}
impl SyntaxRequestReceipt {
    pub(crate) fn capture(request: &SourceDeclarativeRequest) -> Result<Self, Diagnostic> {
        request.key().validate()?;
        let bytes = request.input.encode().map_err(adapter)?;
        let (input, identity) = if let Some(procedural) = request.procedural_request() {
            if bytes != procedural.input.encode().map_err(adapter)?
                || request.site != procedural.call_site
            {
                return Err(fault(
                    "issued procedural syntax request input or site differs",
                ));
            }
            use lrr_expand_adapter::proc_client::ProcForm;
            let input = match procedural.form {
                ProcForm::FunctionLike => SyntaxInput::FunctionArguments,
                ProcForm::Attribute | ProcForm::Derive => SyntaxInput::ItemTransform,
            };
            // The existing procedural schema also binds attribute tokens,
            // callback sites and captured source identities. A bare forest
            // digest is deliberately not the identity of this request kind.
            (
                input,
                crate::service::macro_session::procedural_input_fingerprint(procedural)?,
            )
        } else {
            (SyntaxInput::DelimitedCall, Digest::of_bytes(&bytes))
        };
        if identity != request.key().input_tokens {
            return Err(fault("issued syntax request input differs from its key"));
        }
        Ok(Self {
            key: request.key().clone(),
            site: request.site,
            roots: roots_identity(request.input.roots())?,
            definition: match request.key().macro_identity {
                MacroIdentity::Declaration { definition }
                | MacroIdentity::Builtin { definition } => {
                    Some(crate::expand::scopes::MacroDefinition {
                        identity: definition,
                        defining_unit: request.owner,
                        definition_edition: request.key().definition_edition,
                    })
                }
                MacroIdentity::Procedural { .. } => None,
            },
            input,
        })
    }
    pub(super) fn require_call_roots(
        &self,
        call: &lrr_expand_adapter::token_wire::TokenForest,
    ) -> Result<(), Diagnostic> {
        let roots = match self.input {
            SyntaxInput::DelimitedCall => call.roots(),
            SyntaxInput::FunctionArguments => {
                let [TokenTree::Group { children, .. }] = call.roots() else {
                    return Err(fault(
                        "issued function arguments require their actual call group",
                    ));
                };
                children
            }
            SyntaxInput::ItemTransform => {
                return Err(fault(
                    "issued item transform requires its actual item attachment",
                ));
            }
        };
        if roots_identity(roots)? != self.roots {
            return Err(fault("source syntax issued call tokens/site differ"));
        }
        Ok(())
    }
    pub(crate) fn key(&self) -> &ExpansionKey {
        &self.key
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(match &self.key.macro_identity {
                MacroIdentity::Procedural { export, .. } => export.capacity(),
                _ => 0,
            })
            .filter(|n| *n <= 128 * 1024)
            .ok_or_else(|| fault("issued syntax receipt byte budget"))
    }
}
/// Explicit token-root encoding preserves grouping, spelling and exact span
/// keys, independently of unrelated context/span table rows in either forest.
pub(super) fn roots_identity(roots: &[TokenTree]) -> Result<Digest, Diagnostic> {
    let mut e = StableEncoder::new("lrr/v1/issued-syntax-token-roots");
    e.u64(u64::try_from(roots.len()).map_err(|_| fault("syntax root count overflow"))?);
    let mut pending = Vec::new();
    if roots.len() > 2_000_000 {
        return Err(fault("syntax root count budget"));
    }
    pending
        .try_reserve_exact(roots.len())
        .map_err(|_| fault("syntax root work allocation"))?;
    pending.extend(roots.iter().rev());
    let mut work = 0usize;
    while let Some(token) = pending.pop() {
        work = work
            .checked_add(1)
            .filter(|n| *n <= 2_000_000)
            .ok_or_else(|| fault("syntax root work budget"))?;
        match token {
            TokenTree::Group {
                delimiter,
                open,
                close,
                children,
            } => {
                e.u8(0);
                e.u8(match delimiter {
                    Delimiter::Parenthesis => 0,
                    Delimiter::Brace => 1,
                    Delimiter::Bracket => 2,
                    Delimiter::Invisible => 3,
                });
                e.digest(Digest(open.0));
                e.digest(Digest(close.0));
                e.u64(
                    u64::try_from(children.len())
                        .map_err(|_| fault("syntax child count overflow"))?,
                );
                pending
                    .len()
                    .checked_add(children.len())
                    .filter(|n| *n <= 2_000_000)
                    .ok_or_else(|| fault("syntax pending work budget"))?;
                pending
                    .try_reserve_exact(children.len())
                    .map_err(|_| fault("syntax pending work allocation"))?;
                pending.extend(children.iter().rev());
            }
            TokenTree::Ident { text, raw, span } => {
                e.u8(1);
                e.string(text);
                e.bool(*raw);
                e.digest(Digest(span.0));
            }
            TokenTree::Punct {
                scalar,
                spacing,
                span,
            } => {
                e.u8(2);
                e.u32(u32::from(*scalar));
                e.u8(match spacing {
                    Spacing::Alone => 0,
                    Spacing::Joint => 1,
                    Spacing::JointHidden => 2,
                });
                e.digest(Digest(span.0));
            }
            TokenTree::Literal { token_text, span } => {
                e.u8(3);
                e.string(token_text);
                e.digest(Digest(span.0));
            }
        }
    }
    Ok(e.finish())
}
