//! Immutable original-source lexical function facts for a reached direct call.
//! Namespace selection and admission remain dispatcher/frontend responsibilities.
use super::{
    body_index::BodyIndex,
    outline::{BodyHandle, DeclarationContext, HeaderRecipe, ItemOutline},
    parse::{ParseKey, ParsedFile},
    signatures::{FunctionSignature, GenericKind},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::MacroName,
    ir::{
        op::PrimitiveType,
        pc::{PcKey, Phase},
    },
    source::manifest::ItemKind,
    traits::term::TyTerm,
};
use ra_ap_syntax::{
    AstNode, TextRange, TextSize,
    ast::{self, HasArgList},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
/// Private fields prohibit callers from replacing a selected header/body pair.
/// Root admission must additionally verify current parent inputs/namespace facts.
#[derive(Debug, Clone)]
pub(crate) struct LexicalFunctionPlan {
    source: ParseKey,
    caller: BodyHandle,
    caller_header: HeaderRecipe,
    caller_context: DeclarationContext,
    site: PcKey,
    path_range: SourceRange,
    scope: u32,
    item: ItemOutline,
    signature: FunctionSignature,
    primitive_names: Vec<&'static str>,
    primitive_parameters: Box<[PrimitiveType]>,
    primitive_result: PrimitiveType,
    source_revision: u64,
    query_revision: u64,
    metadata_bytes: usize,
}
// This captures syntax candidates only. Exact owning Type-namespace absence is
// required separately before any candidate becomes a source contract.
fn primitive_candidate(
    tokens: &[super::ast_adapter::TokenRecipe],
    edition: u16,
) -> Result<(PrimitiveType, &'static str), Diagnostic> {
    if tokens.len() != 1 {
        return Err(fault(
            "lexical numeric type requires one relative primitive path",
        ));
    }
    let name = MacroName::parse(&tokens[0].text, edition)?;
    Ok(match name.as_str() {
        "i8" => (PrimitiveType::I8, "i8"),
        "i16" => (PrimitiveType::I16, "i16"),
        "i32" => (PrimitiveType::I32, "i32"),
        "i64" => (PrimitiveType::I64, "i64"),
        "isize" => (PrimitiveType::Isize, "isize"),
        "u8" => (PrimitiveType::U8, "u8"),
        "u16" => (PrimitiveType::U16, "u16"),
        "u32" => (PrimitiveType::U32, "u32"),
        "u64" => (PrimitiveType::U64, "u64"),
        "usize" => (PrimitiveType::Usize, "usize"),
        "f32" => (PrimitiveType::F32, "f32"),
        "f64" => (PrimitiveType::F64, "f64"),
        _ => {
            return Err(fault(
                "lexical named type awaits source namespace integration",
            ));
        }
    })
}
impl LexicalFunctionPlan {
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
    }
    pub(crate) fn caller(&self) -> &BodyHandle {
        &self.caller
    }
    pub(crate) fn caller_header(&self) -> &HeaderRecipe {
        &self.caller_header
    }
    pub(crate) fn caller_context(&self) -> &DeclarationContext {
        &self.caller_context
    }
    pub(crate) fn site(&self) -> &PcKey {
        &self.site
    }
    pub(crate) fn path_range(&self) -> SourceRange {
        self.path_range
    }
    pub(crate) fn scope(&self) -> u32 {
        self.scope
    }
    pub(crate) fn item(&self) -> &ItemOutline {
        &self.item
    }
    pub(crate) fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    pub(crate) fn primitive_parameters(&self) -> &[PrimitiveType] {
        &self.primitive_parameters
    }
    pub(crate) fn primitive_result(&self) -> PrimitiveType {
        self.primitive_result
    }
    pub(crate) fn primitive_names(&self) -> &[&'static str] {
        &self.primitive_names
    }
    pub(crate) fn source_revision(&self) -> u64 {
        self.source_revision
    }
    pub(crate) fn query_revision(&self) -> u64 {
        self.query_revision
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/reached-lexical-numeric-function-plan");
        e.record(&self.source);
        e.record(&self.caller.key());
        e.record(&self.caller_header);
        e.record(&self.caller_context.scope);
        e.u32(self.caller_context.range.start);
        e.u32(self.caller_context.range.end);
        e.record(&self.site);
        e.u32(self.path_range.start);
        e.u32(self.path_range.end);
        e.u32(self.scope);
        e.record(&self.item.def_key);
        e.record(&self.item.header);
        e.record(&self.item.scope);
        e.record(&self.signature);
        e.sequence(&self.primitive_parameters, |e, p| e.u32(p.tag()));
        e.u32(self.primitive_result.tag());
        e.u32(self.item.declaration_span.start);
        e.u32(self.item.declaration_span.end);
        e.record(&self.item.body.as_ref().expect("checked lexical body").key());
        e.u64(self.source_revision);
        e.u64(self.query_revision);
        e.finish()
    }
}
impl ParsedFile {
    /// Rebuild exact immutable declaration facts from this captured caller only.
    /// No callee body index/thunk, future call or provider is demanded.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lexical_numeric_function_plan(
        &self,
        caller: &BodyHandle,
        index: &BodyIndex,
        site: &PcKey,
        caller_header: &HeaderRecipe,
        caller_context: &DeclarationContext,
        selected: Digest,
        expected_name: &str,
        source_revision: u64,
        query_revision: u64,
    ) -> Result<LexicalFunctionPlan, Diagnostic> {
        if self.generated_origin.is_some()
            || caller.key() != *index.body()
            || site.body != caller.key()
            || site.phase != Phase::Entry.code()?
        {
            return Err(fault(
                "lexical numeric call requires original exact caller entry",
            ));
        }
        index.pc(crate::ir::pc::NodeId(site.node), Phase::Entry)?;
        let node = &index.nodes()[site.node as usize];
        let start = caller
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| fault("lexical call source start overflow"))?;
        let end = caller
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| fault("lexical call source end overflow"))?;
        let root = self.entered_body_root(caller)?;
        let call = root
            .covering_element(TextRange::new(TextSize::from(start), TextSize::from(end)))
            .ancestors()
            .find_map(|node| {
                let call = ast::CallExpr::cast(node)?;
                let range = call.syntax().text_range();
                (u32::from(range.start()) == start && u32::from(range.end()) == end).then_some(call)
            })
            .ok_or_else(|| fault("lexical call exact syntax absent"))?;
        if call.arg_list().is_none() {
            return Err(fault("lexical call arguments absent"));
        }
        let Some(ast::Expr::PathExpr(path)) = call.expr() else {
            return Err(fault("lexical call is not a direct source path"));
        };
        let p = path
            .path()
            .ok_or_else(|| fault("lexical call path absent"))?;
        let segment = p
            .segment()
            .ok_or_else(|| fault("lexical call segment absent"))?;
        if p.qualifier().is_some() || segment.coloncolon_token().is_some() {
            return Err(fault(
                "lexical numeric callee requires one relative source name",
            ));
        }
        let name = segment
            .name_ref()
            .ok_or_else(|| fault("lexical call name absent"))?;
        let wanted = MacroName::parse(expected_name, self.edition)?;
        if MacroName::parse(name.text(), self.edition)? != wanted {
            return Err(fault(
                "lexical call recipe name differs from exact source path",
            ));
        }
        let caller_signature =
            self.function_signature(caller.def_key, caller_header, caller_context, Some(caller))?;
        let facts = self.entered_macro_facts(caller, index.cfg(), &caller_context.scope)?;
        if facts.items.len() > 100_000 {
            return Err(fault("lexical declaration source work budget"));
        }
        let mut found = facts
            .items
            .iter()
            .filter(|row| row.item.def_key.fingerprint() == selected);
        let row = found
            .next()
            .ok_or_else(|| fault("selected lexical declaration absent from exact caller"))?;
        if found.next().is_some()
            || row.item.def_key.item_kind != ItemKind::Function
            || MacroName::parse(&row.item.def_key.declared_name, self.edition)? != wanted
            || row.attribute_input.is_some()
            || row.item.header.profile != self.profile
            || row.item.scope.owner.is_none()
        {
            return Err(fault(
                "lexical function source declaration identity differs",
            ));
        }
        let body = row
            .item
            .body
            .as_ref()
            .ok_or_else(|| fault("lexical function body handle absent"))?;
        body.require_entry()?;
        let function = self
            .parsed
            .tree()
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(row.item.declaration_span.start),
                TextSize::from(row.item.declaration_span.end),
            ))
            .ancestors()
            .find_map(ast::Fn::cast)
            .ok_or_else(|| fault("lexical function exact syntax absent"))?;
        if function.async_token().is_some()
            || function.gen_token().is_some()
            || function.abi().is_some()
            || function.unsafe_token().is_some()
            || function.param_list().is_some_and(|list| {
                list.params().any(|parameter| {
                    parameter
                        .syntax()
                        .children()
                        .any(|node| ast::Attr::can_cast(node.kind()))
                })
            })
        {
            return Err(fault(
                "lexical function modifiers or parameter attributes require later integration",
            ));
        }

        let context = DeclarationContext {
            scope: row.item.scope.clone(),
            range: row.item.declaration_span,
        };
        let signature =
            self.function_signature(selected, &row.item.header, &context, Some(body))?;
        if !signature.generics.is_empty()
            || !signature.predicates.is_empty()
            || signature.parameters.len() > 256
        {
            return Err(fault(
                "lexical generic source signature awaits owning context integration",
            ));
        }
        let mut names = Vec::new();
        names
            .try_reserve_exact(signature.parameters.len() + 1)
            .map_err(|_| fault("lexical primitive source proof allocation"))?;
        let mut primitive_parameters = Vec::new();
        primitive_parameters
            .try_reserve_exact(signature.parameters.len())
            .map_err(|_| fault("lexical primitive parameter allocation"))?;
        for parameter in &signature.parameters {
            if parameter.receiver || parameter.variadic {
                return Err(fault(
                    "lexical scalar parameter shape requires later integration",
                ));
            }
            let (primitive, name) = primitive_candidate(&parameter.ty, self.edition)?;
            if parameter
                .symbolic
                .as_ref()
                .is_some_and(|term| *term != TyTerm::Primitive(primitive))
            {
                return Err(fault(
                    "lexical source parameter differs from its primitive syntax candidate",
                ));
            }
            primitive_parameters.push(primitive);
            names.push(name);
        }
        let (primitive_result, result_name) = primitive_candidate(&signature.result, self.edition)?;
        if signature
            .result_symbolic
            .as_ref()
            .is_some_and(|term| *term != TyTerm::Primitive(primitive_result))
        {
            return Err(fault(
                "lexical source return differs from its primitive syntax candidate",
            ));
        }
        names.push(result_name);
        names.sort_unstable();
        names.dedup();
        for parameter in &caller_signature.generics {
            if parameter.kind == GenericKind::Type
                && names.iter().any(|name| {
                    MacroName::parse(&parameter.name, self.edition).ok()
                        == MacroName::parse(name, self.edition).ok()
                })
            {
                return Err(fault(
                    "lexical primitive spelling is a caller type parameter",
                ));
            }
        }
        let charge = |bytes: usize, n: usize| {
            bytes
                .checked_add(n)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("lexical function plan metadata budget"))
        };
        let mut metadata_bytes = std::mem::size_of::<LexicalFunctionPlan>();
        for n in [
            primitive_parameters
                .capacity()
                .checked_mul(std::mem::size_of::<PrimitiveType>())
                .ok_or_else(|| fault("lexical parameter size overflow"))?,
            caller_header.estimated_bytes(),
            row.item.header.estimated_bytes(),
            signature.estimated_bytes(),
            row.item.def_key.declared_name.len(),
            names
                .capacity()
                .checked_mul(std::mem::size_of::<&str>())
                .ok_or_else(|| fault("lexical name proof size overflow"))?,
        ] {
            metadata_bytes = charge(metadata_bytes, n)?;
        }
        for path in [
            &caller_context.scope.module_path,
            &row.item.scope.module_path,
            &row.item.def_key.module_path,
            &signature.scope.module_path,
        ] {
            metadata_bytes = charge(
                metadata_bytes,
                path.len()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(|| fault("lexical plan path size overflow"))?,
            )?;
            for component in path {
                metadata_bytes = charge(metadata_bytes, component.len())?;
            }
        }
        // Parse/header/signature profiles each retain their owned semantics string.
        metadata_bytes = charge(
            metadata_bytes,
            self.profile
                .semantics
                .len()
                .checked_mul(4)
                .ok_or_else(|| fault("lexical plan profile size overflow"))?,
        )?;
        let r = path.syntax().text_range();
        Ok(LexicalFunctionPlan {
            source: self.key(),
            caller: caller.clone(),
            caller_header: caller_header.clone(),
            caller_context: caller_context.clone(),
            site: site.clone(),
            path_range: SourceRange {
                start: u32::from(r.start()),
                end: u32::from(r.end()),
            },
            scope: row.scope,
            item: row.item.clone(),
            signature,
            primitive_names: names,
            primitive_parameters: primitive_parameters.into_boxed_slice(),
            primitive_result,
            source_revision,
            query_revision,
            metadata_bytes,
        })
    }
}

#[cfg(test)]
#[path = "lexical_numeric_callees_tests.rs"]
mod tests;
