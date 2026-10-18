//! Structural queries over the retained tree that need neither an owned
//! snapshot nor a replayed parse: captures inside one edit and the body that
//! owns a call. They read the live tree, so their cost follows the answer.
use super::*;
use crate::declaration_output::{CallCapture, Captures, DefinitionCapture, capture_nodes};

/// Definitions and calls captured inside one range of the current tree, in
/// preorder, with the coordinates of that tree.
pub struct RangeCaptures {
    pub definitions: Vec<DefinitionCapture>,
    pub calls: Vec<CallCapture>,
}

/// Which declarations own a call's body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyOwners {
    /// Functions only (ordinary function-body expansion).
    Functions,
    /// Functions, constants and statics that have a body (entered bodies).
    Entered,
}

/// A function's body handle range: its block, or when it has a pattern
/// parameter (anything but `x`, `mut x` and `_`) its parameter list through
/// its block, as `lrr`'s `syntax::fn_parameters::body_range` defines it.
fn function_body(function: &ast::Fn) -> Option<TextRange> {
    let block = function.body()?.syntax().text_range();
    let patterns = function.param_list().filter(|list| {
        list.params().any(|parameter| match parameter.pat() {
            None | Some(ast::Pat::WildcardPat(_)) => false,
            Some(ast::Pat::IdentPat(pattern)) => {
                pattern.ref_token().is_some() || pattern.at_token().is_some()
            }
            Some(_) => true,
        })
    });
    Some(match patterns {
        Some(list) => TextRange::new(list.syntax().text_range().start(), block.end()),
        None => block,
    })
}

/// The body of a function, constant or static (`Some(None)` when it has no
/// body); `None` for any other node.
fn entered_body(node: &SyntaxNode) -> Option<Option<TextRange>> {
    if let Some(function) = ast::Fn::cast(node.clone()) {
        Some(function_body(&function))
    } else if let Some(constant) = ast::Const::cast(node.clone()) {
        Some(constant.body().map(|body| body.syntax().text_range()))
    } else if let Some(value) = ast::Static::cast(node.clone()) {
        Some(value.body().map(|body| body.syntax().text_range()))
    } else {
        None
    }
}

fn pair(range: TextRange) -> (u32, u32) {
    (range.start().into(), range.end().into())
}

impl SyntaxAttachment {
    /// The captures wholly inside `[start, end)` of the current tree. A
    /// successful edit's inserted range yields exactly the definitions and
    /// calls that edit introduced, without exporting the rest of the tree.
    pub fn captures_in(&self, start: u32, end: u32) -> Result<RangeCaptures, AdapterError> {
        if start > end || end > u32::from(self.root.text_range().len()) {
            return Err(error("capture range is outside the current syntax"));
        }
        let store = self
            .store
            .lock()
            .map_err(|_| error("attachment span store lock poisoned"))?;
        let Captures { definitions, calls } = capture_nodes(
            &self.root,
            &self.origins,
            &store,
            Some(TextRange::new(start.into(), end.into())),
        )?;
        Ok(RangeCaptures { definitions, calls })
    }

    /// The body that owns the macro call at exactly `[start, end)`: the body of
    /// the innermost enclosing function (`BodyOwners::Functions`, which is
    /// absent when that function has no body) or of the innermost enclosing
    /// function/constant/static that has one (`BodyOwners::Entered`).
    /// `Ok(None)` means no such owner.
    pub fn call_owner_body(
        &self,
        start: u32,
        end: u32,
        owners: BodyOwners,
    ) -> Result<Option<(u32, u32)>, AdapterError> {
        let call = self.select_call(start, end)?;
        Ok(match owners {
            BodyOwners::Functions => call
                .ancestors()
                .find_map(ast::Fn::cast)
                .and_then(|function| function_body(&function))
                .map(pair),
            BodyOwners::Entered => call
                .ancestors()
                .find_map(|node| entered_body(&node).flatten())
                .map(pair),
        })
    }

    /// The body of the outermost function/constant/static whose declaration
    /// occupies exactly `[start, end)`: `None` when no such declaration is
    /// there, `Some(None)` when it has no body.
    pub fn declaration_body_at(
        &self,
        start: u32,
        end: u32,
    ) -> Result<Option<Option<(u32, u32)>>, AdapterError> {
        if start > end || end > u32::from(self.root.text_range().len()) {
            return Err(error("declaration range is outside the current syntax"));
        }
        let range = TextRange::new(start.into(), end.into());
        Ok(outermost_at(&self.root, range, |node| {
            entered_body(node).is_some()
        })
        .and_then(|node| entered_body(&node))
        .map(|body| body.map(pair)))
    }
}
