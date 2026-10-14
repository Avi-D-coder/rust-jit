//! Entered constructor syntax. Owned operands retain source order, never values.
use super::{
    ast_adapter::{TokenRecipe, range, semantic_name},
    body_index::{BodyIndex, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::pc::{NodeId, SlotId},
    source::snapshot::CfgSet,
    syntax::spans::{BodySpan, SourceRange},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasGenericArgs},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConstructorOperand {
    pub node: NodeId,
    pub value: SlotId,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WrittenField {
    pub name: String,
    pub edition: u16,
    pub operand: ConstructorOperand,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConstructorToken {
    pub token: TokenRecipe,
    pub range: SourceRange,
    pub edition: u16,
}
/// Path classification from the exact source AST. It contains no selected
/// definition, runtime type or macro authority.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecordPathSelection {
    Named(RecordConstructorPath),
    TypeContext,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordConstructorPath {
    segments: Vec<super::imports::ImportSegment>,
    absolute: bool,
    at: u32,
    name: String,
}
impl RecordConstructorPath {
    pub fn segments(&self) -> &[super::imports::ImportSegment] {
        &self.segments
    }
    pub fn absolute(&self) -> bool {
        self.absolute
    }
    pub fn at(&self) -> u32 {
        self.at
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConstructorKind {
    Tuple(Vec<ConstructorOperand>),
    Array(Vec<ConstructorOperand>),
    Repeat {
        element: ConstructorOperand,
        length: Vec<ConstructorToken>,
    },
    Record {
        path: Vec<ConstructorToken>,
        fields: Vec<WrittenField>,
        rest: Option<ConstructorOperand>,
    },
}
/// Sealed source/index pairing. Consumers resolve types and evaluate child PCs
/// later; capture performs neither operation, including for a zero-length repeat.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConstructorPlan {
    origin: BodySpan,
    node: NodeId,
    profile: ProfileIdentity,
    cfg: Digest,
    kind: ConstructorKind,
}
impl ConstructorPlan {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub fn cfg(&self) -> Digest {
        self.cfg
    }
    pub fn origin(&self) -> &BodySpan {
        &self.origin
    }
    pub fn kind(&self) -> &ConstructorKind {
        &self.kind
    }
    pub fn estimated_bytes(&self) -> Option<usize> {
        let operands = |n: usize| n.checked_mul(std::mem::size_of::<ConstructorOperand>());
        let extra = match &self.kind {
            ConstructorKind::Tuple(fields) | ConstructorKind::Array(fields) => {
                operands(fields.capacity())?
            }
            ConstructorKind::Repeat { length, .. } => token_bytes(length)?,
            ConstructorKind::Record { path, fields, .. } => {
                let mut bytes = token_bytes(path)?.checked_add(
                    fields
                        .capacity()
                        .checked_mul(std::mem::size_of::<WrittenField>())?,
                )?;
                for field in fields {
                    bytes = bytes.checked_add(field.name.capacity())?;
                }
                bytes
            }
        };
        std::mem::size_of::<Self>()
            .checked_add(extra)?
            .checked_add(self.profile.semantics.capacity())
    }
}
fn token_bytes(tokens: &Vec<ConstructorToken>) -> Option<usize> {
    let mut bytes = tokens
        .capacity()
        .checked_mul(std::mem::size_of::<ConstructorToken>())?;
    for token in tokens {
        bytes = bytes.checked_add(token.token.text.capacity())?;
    }
    Some(bytes)
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "entered constructor metadata limit".into(),
    )
}
fn push<T>(fields: &mut Vec<T>, field: T) -> Result<(), Diagnostic> {
    // Every written operand must fit a symbolic Operand phase.
    if fields.len() >= usize::from(u16::MAX - 3) {
        return Err(limit());
    }
    fields.try_reserve(1).map_err(|_| limit())?;
    fields.push(field);
    Ok(())
}
impl ParsedFile {
    pub(crate) fn record_constructor_path(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        plan: &ConstructorPlan,
        cfg: &CfgSet,
    ) -> Result<RecordPathSelection, Diagnostic> {
        use super::imports::ImportSegment;
        if self
            .constructor_plan(body, index, plan.node(), cfg)?
            .as_ref()
            != Some(plan)
        {
            return Err(fault(
                "record path differs from exact source constructor plan",
            ));
        }
        if self.generated_origin.is_some() {
            // Per-token generated name contexts must come from the adopted
            // expansion view; an original source root cannot replace them.
            return Ok(RecordPathSelection::TypeContext);
        }
        let ConstructorKind::Record { path: original, .. } = plan.kind() else {
            return Err(fault("record path requested for another constructor kind"));
        };
        let first = original
            .first()
            .ok_or_else(|| fault("record path tokens absent"))?;
        let last = original
            .last()
            .ok_or_else(|| fault("record path tokens absent"))?;
        let path = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Path::cast)
            .find(|path| {
                range(path.syntax().text_range())
                    == SourceRange {
                        start: first.range.start,
                        end: last.range.end,
                    }
            })
            .ok_or_else(|| fault("record path AST boundary absent"))?;
        let mut suffix = Vec::new();
        let mut current = path;
        let absolute;
        let mut name = None;
        loop {
            if suffix.len() >= 256 {
                return Err(limit());
            }
            let segment = current
                .segment()
                .ok_or_else(|| fault("record path segment absent"))?;
            if segment.generic_arg_list().is_some() {
                return Ok(RecordPathSelection::TypeContext);
            }
            let owned = match segment
                .kind()
                .ok_or_else(|| fault("record path segment kind absent"))?
            {
                ast::PathSegmentKind::Name(reference) => {
                    // Original/raw spelling is retained for the source name engine.
                    let edition = self.token_edition(range(reference.syntax().text_range()))?;
                    crate::expand::scopes::MacroName::parse(reference.text(), edition)?;
                    let text = reference.text().to_string();
                    if name.is_none() {
                        name = Some(text.clone());
                    }
                    ImportSegment::Name(text)
                }
                ast::PathSegmentKind::CrateKw => ImportSegment::Crate,
                ast::PathSegmentKind::SelfKw => ImportSegment::SelfModule,
                ast::PathSegmentKind::SuperKw => ImportSegment::Super,
                _ => return Ok(RecordPathSelection::TypeContext),
            };
            suffix.try_reserve(1).map_err(|_| limit())?;
            suffix.push(owned);
            if let Some(parent) = current.qualifier() {
                current = parent;
            } else {
                absolute = segment.coloncolon_token().is_some();
                break;
            }
        }
        suffix.reverse();
        Ok(RecordPathSelection::Named(RecordConstructorPath {
            segments: suffix,
            absolute,
            at: first.range.start,
            name: name.ok_or_else(|| fault("record path final name absent"))?,
        }))
    }
    fn constructor_tokens(
        &self,
        node: &ra_ap_syntax::SyntaxNode,
    ) -> Result<Vec<ConstructorToken>, Diagnostic> {
        let mut owned = Vec::new();
        for token in node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia())
        {
            let range = range(token.text_range());
            push(
                &mut owned,
                ConstructorToken {
                    token: TokenRecipe {
                        kind: token.kind() as u16,
                        text: token.text().to_owned(),
                    },
                    range,
                    edition: self.token_edition(range)?,
                },
            )?;
        }
        Ok(owned)
    }
    pub fn constructor_plan(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        node: NodeId,
        cfg: &CfgSet,
    ) -> Result<Option<ConstructorPlan>, Diagnostic> {
        body.require_entry()?;
        if index.body() != &body.key() || index.profile() != &self.profile {
            return Err(fault("constructor body/index mismatch"));
        }
        let root = self.entered_body_root(body)?;
        let origin = index.span(node)?;
        let selected = index
            .nodes()
            .get(node.0 as usize)
            .ok_or_else(|| fault("constructor node absent"))?;
        if !matches!(selected.role, NodeRole::Expression(_)) {
            return Ok(None);
        }
        let start = body
            .range
            .start
            .checked_add(origin.relative_start)
            .ok_or_else(limit)?;
        let end = body
            .range
            .start
            .checked_add(origin.relative_end)
            .ok_or_else(limit)?;
        let syntax = root
            .descendants()
            .find(|syntax| {
                let r = range(syntax.text_range());
                r.start == start && r.end == end && ast::Expr::can_cast(syntax.kind())
            })
            .ok_or_else(|| fault("constructor expression boundary absent"))?;
        let expression =
            ast::Expr::cast(syntax).ok_or_else(|| fault("constructor expression absent"))?;
        if !matches!(
            expression,
            ast::Expr::TupleExpr(_) | ast::Expr::ArrayExpr(_) | ast::Expr::RecordExpr(_)
        ) {
            return Ok(None);
        }
        // A sorted temporary index avoids an operand-count times body-size walk.
        if index.nodes().len() > 100_000 {
            return Err(limit());
        }
        let mut locations = Vec::new();
        locations
            .try_reserve_exact(index.nodes().len())
            .map_err(|_| limit())?;
        for indexed in index
            .nodes()
            .iter()
            .filter(|n| matches!(n.role, NodeRole::Expression(_)))
        {
            locations.push(((indexed.range.start, indexed.range.end), indexed.id));
        }
        locations.sort_unstable_by_key(|(r, id)| (*r, id.0));
        if locations.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(fault("constructor operand boundary is ambiguous"));
        }
        let operand = |expr: ast::Expr| -> Result<ConstructorOperand, Diagnostic> {
            let r = range(expr.syntax().text_range());
            let relative = (
                r.start.checked_sub(body.range.start).ok_or_else(limit)?,
                r.end.checked_sub(body.range.start).ok_or_else(limit)?,
            );
            let found = locations
                .binary_search_by_key(&relative, |(r, _)| *r)
                .map_err(|_| fault("constructor operand absent from index"))?;
            let node = locations[found].1;
            let value = index.nodes()[node.0 as usize]
                .value
                .ok_or_else(|| fault("constructor operand has no value slot"))?;
            Ok(ConstructorOperand { node, value })
        };
        let kind = match expression {
            ast::Expr::TupleExpr(tuple) => {
                let mut fields = Vec::new();
                for field in tuple.fields() {
                    if super::attributes::expression_enabled(&field, cfg)? {
                        push(&mut fields, operand(field)?)?;
                    }
                }
                ConstructorKind::Tuple(fields)
            }
            ast::Expr::ArrayExpr(array) => match array.kind() {
                ast::ArrayExprKind::ElementList(elements) => {
                    let mut fields = Vec::new();
                    for element in elements {
                        if super::attributes::expression_enabled(&element, cfg)? {
                            push(&mut fields, operand(element)?)?;
                        }
                    }
                    ConstructorKind::Array(fields)
                }
                ast::ArrayExprKind::Repeat {
                    initializer,
                    repeat,
                } => {
                    let initializer = initializer.ok_or_else(|| fault("repeat operand absent"))?;
                    let repeat = repeat.ok_or_else(|| fault("repeat length absent"))?;
                    ConstructorKind::Repeat {
                        element: operand(initializer)?,
                        length: self.constructor_tokens(repeat.syntax())?,
                    }
                }
            },
            ast::Expr::RecordExpr(record) => {
                let path = record
                    .path()
                    .ok_or_else(|| fault("constructor path absent"))?;
                let list = record
                    .record_expr_field_list()
                    .ok_or_else(|| fault("constructor field list absent"))?;
                let mut fields = Vec::new();
                for field in list.fields() {
                    let (enabled, _) = super::attributes::effective_metas(
                        field.attrs().filter_map(|attr| attr.meta()).collect(),
                        cfg,
                    )?;
                    if !enabled {
                        continue;
                    }
                    let source_name = field
                        .field_name()
                        .ok_or_else(|| fault("constructor field name absent"))?;
                    let name = semantic_name(source_name.text())
                        .ok_or_else(|| fault("constructor field name invalid"))?;
                    let edition = self.token_edition(range(source_name.syntax().text_range()))?;
                    let expr = field
                        .expr()
                        .ok_or_else(|| fault("constructor initializer absent"))?;
                    push(
                        &mut fields,
                        WrittenField {
                            name,
                            edition,
                            operand: operand(expr)?,
                        },
                    )?;
                }
                let rest = list.spread().map(operand).transpose()?;
                if rest.is_some() && fields.len() >= usize::from(u16::MAX - 3) {
                    return Err(limit());
                }
                ConstructorKind::Record {
                    path: self.constructor_tokens(path.syntax())?,
                    fields,
                    rest,
                }
            }
            _ => return Ok(None),
        };
        let plan = ConstructorPlan {
            origin,
            node,
            profile: self.profile.clone(),
            cfg: cfg.fingerprint(),
            kind,
        };
        if plan
            .estimated_bytes()
            .is_none_or(|bytes| bytes > 32 * 1024 * 1024)
        {
            return Err(limit());
        }
        Ok(Some(plan))
    }
}
impl StableEncode for ConstructorPlan {
    const DOMAIN: &'static str = "lrr/v2/entered-constructor-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.origin.body_key);
        e.u32(self.node.0);
        e.u32(self.origin.relative_start);
        e.u32(self.origin.relative_end);
        e.record(&self.profile);
        e.digest(self.cfg);
        let operand = |e: &mut StableEncoder, value: &ConstructorOperand| {
            e.u32(value.node.0);
            e.u32(value.value.0);
        };
        match &self.kind {
            ConstructorKind::Tuple(fields) => {
                e.u32(0);
                e.sequence(fields, operand);
            }
            ConstructorKind::Array(fields) => {
                e.u32(1);
                e.sequence(fields, operand);
            }
            ConstructorKind::Repeat { element, length } => {
                e.u32(2);
                operand(e, element);
                e.sequence(length, |e, token| e.record(token));
            }
            ConstructorKind::Record { path, fields, rest } => {
                e.u32(3);
                e.sequence(path, |e, token| e.record(token));
                e.sequence(fields, |e, field| {
                    e.string(&field.name);
                    e.u16(field.edition);
                    operand(e, &field.operand);
                });
                e.option(rest, operand);
            }
        }
    }
}
impl StableEncode for ConstructorToken {
    const DOMAIN: &'static str = "lrr/v2/constructor-syntax-token";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.token);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.u16(self.edition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::pc::Phase;
    use crate::source::snapshot::SourceBlob;
    use crate::syntax::body_index::ExpressionKind;
    #[test]
    fn rustc_const_record_initializer_indexes_original_order_without_constant_evaluation() {
        use crate::syntax::body_index::{ScopeId, ScopeKind};
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-struct.rs"
        );
        let cfg = CfgSet::guest_default();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed.outline(Digest([147; 32]), vec![], &cfg).unwrap();
        let constant = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "y")
            .unwrap();
        let body = constant.body.as_ref().unwrap();
        let index = parsed.index_body(body, Default::default()).unwrap();
        let lexical = parsed
            .entered_macro_facts(body, &cfg, &constant.scope)
            .unwrap();
        assert_eq!(lexical.scopes[0].range, body.range);
        assert!(lexical.bindings.is_empty());
        assert!(lexical.invocations.is_empty());
        assert!(lexical.imports.is_empty());
        assert_eq!(index.scopes()[0].kind, ScopeKind::Initializer);
        assert_eq!(index.scopes()[0].parent, None);
        assert_eq!(index.nodes()[0].scope, ScopeId(0));
        let plan = parsed
            .constructor_plan(body, &index, NodeId(0), &cfg)
            .unwrap()
            .unwrap();
        let ConstructorKind::Record {
            fields, rest: None, ..
        } = plan.kind()
        else {
            panic!("original const initializer is not a plain record")
        };
        let written: Vec<_> = fields.iter().map(|field| field.name.as_str()).collect();
        assert_eq!(written, ["b", "c", "a"]);
        assert!(fields.windows(2).all(|pair| {
            index.nodes()[pair[0].operand.node.0 as usize].range.start
                < index.nodes()[pair[1].operand.node.0 as usize].range.start
        }));
        let declaration = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "foo")
            .unwrap();
        let schema = parsed
            .nominal_schema(
                declaration.def_key.fingerprint(),
                &declaration.header,
                &crate::syntax::outline::DeclarationContext {
                    scope: declaration.scope.clone(),
                    range: declaration.declaration_span,
                },
                &cfg,
            )
            .unwrap();
        assert_eq!(
            schema
                .variant(None)
                .unwrap()
                .named_initializer_slots(&written)
                .unwrap(),
            [1, 2, 0]
        );
        let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
        let thunk = parsed
            .constructor_thunk(body, &index, &entry, &plan)
            .unwrap();
        assert!(matches!(
            thunk.recipe().operations.as_ref(),
            [crate::ir::op::Op::ConstructStart { .. }]
        ));
        assert_eq!(plan.origin(), &index.span(NodeId(0)).unwrap());
        let mut changed = body.clone();
        changed.body_hash = Digest::of_bytes(b"different initializer");
        assert_eq!(
            parsed
                .index_body(&changed, Default::default())
                .unwrap_err()
                .code,
            "LRR-E-BODY-INDEX"
        );
        let operand = &index.nodes()[fields[0].operand.node.0 as usize];
        let mut forged = body.clone();
        forged.range = SourceRange {
            start: body.range.start.checked_add(operand.range.start).unwrap(),
            end: body.range.start.checked_add(operand.range.end).unwrap(),
        };
        let bytes = &ORIGINAL.as_bytes()[forged.range.start as usize..forged.range.end as usize];
        forged.body_hash = Digest::of_bytes(bytes);
        assert_eq!(
            parsed
                .index_body(&forged, Default::default())
                .unwrap_err()
                .code,
            "LRR-E-BODY-INDEX"
        );
        // Pure pinned AST/plan/schema/entry lowering only. No constant evaluator,
        // import settlement, Debug derive, guest allocation or main() ran.
    }
    const ZERO_REPEAT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/eval-empty-array-expr.rs"
    );
    const ONE_TUPLE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/one-tuple.rs");
    const RECORD_ORDER: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-2.rs"
    );
    const RECORD_UPDATE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-3.rs"
    );
    fn source(source: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([147; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let index = parsed.index_body(&body, Default::default()).unwrap();
        (parsed, body, index)
    }
    fn plans(parsed: &ParsedFile, body: &BodyHandle, index: &BodyIndex) -> Vec<ConstructorPlan> {
        index
            .nodes()
            .iter()
            .filter(|node| matches!(node.role, NodeRole::Expression(ExpressionKind::Other)))
            .filter_map(|node| {
                parsed
                    .constructor_plan(body, index, node.id, &CfgSet::guest_default())
                    .unwrap()
            })
            .collect()
    }
    #[test]
    fn rustc_zero_repeat_constructor_plan_keeps_opaque_panic_operand() {
        let (parsed, body, index) = source(ZERO_REPEAT);
        let plans = plans(&parsed, &body, &index);
        assert_eq!(plans.len(), 1);
        let ConstructorKind::Repeat { element, length } = plans[0].kind() else {
            panic!("original repeat plan absent");
        };
        assert_eq!(
            length
                .iter()
                .map(|token| token.token.text.as_str())
                .collect::<Vec<_>>(),
            ["0"]
        );
        assert_eq!(
            index.nodes()[element.node.0 as usize].role,
            NodeRole::Expression(ExpressionKind::Macro)
        );
        assert_eq!(
            index.nodes()[element.node.0 as usize].value,
            Some(element.value)
        );
        assert_eq!(&plans[0].origin().body_key, &body.key());
        assert!(plans[0].estimated_bytes().unwrap() < 32 * 1024 * 1024);
        assert_eq!(
            plans[0].fingerprint(),
            parsed
                .constructor_plan(
                    &body,
                    &index,
                    index
                        .nodes()
                        .iter()
                        .find(|node| node.range.start == plans[0].origin().relative_start
                            && node.range.end == plans[0].origin().relative_end)
                        .unwrap()
                        .id,
                    &CfgSet::guest_default()
                )
                .unwrap()
                .unwrap()
                .fingerprint()
        );
        // No macro expansion, guest panic or run-fail execution is claimed here.
    }
    #[test]
    fn rustc_constructor_plans_keep_unary_tuple_written_fields_and_last_update_base() {
        let (parsed, body, index) = source(ONE_TUPLE);
        let tuples = plans(&parsed, &body, &index);
        assert_eq!(tuples.len(), 2);
        for tuple in tuples {
            let ConstructorKind::Tuple(fields) = tuple.kind() else {
                panic!("original tuple plan absent");
            };
            assert_eq!(fields.len(), 1);
            assert_eq!(
                index.nodes()[fields[0].node.0 as usize].role,
                NodeRole::Expression(ExpressionKind::Literal)
            );
        }
        let (parsed, body, index) = source(RECORD_ORDER);
        let records = plans(&parsed, &body, &index);
        assert_eq!(records.len(), 1);
        let ConstructorKind::Record { fields, rest, .. } = records[0].kind() else {
            panic!("original record plan absent");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["f1", "f0"]
        );
        assert!(rest.is_none());
        assert!(fields.iter().all(|field| field.edition == 2024));
        assert!(fields.windows(2).all(|pair| {
            index.nodes()[pair[0].operand.node.0 as usize].range.start
                < index.nodes()[pair[1].operand.node.0 as usize].range.start
        }));
        let (parsed, body, index) = source(RECORD_UPDATE);
        let updates = plans(&parsed, &body, &index);
        let update = updates
            .iter()
            .find(|plan| matches!(plan.kind(), ConstructorKind::Record { rest: Some(_), .. }))
            .unwrap();
        let ConstructorKind::Record {
            fields,
            rest: Some(rest),
            ..
        } = update.kind()
        else {
            panic!("original update plan absent");
        };
        assert!(fields.iter().all(
            |field| index.nodes()[field.operand.node.0 as usize].range.end
                < index.nodes()[rest.node.0 as usize].range.start
        ));
        // These are source/index plans; tuple patterns, to_string, event()
        // evaluation and complete stock assertions still require VM lowering.
    }
    #[test]
    fn rustc_constructor_thunk_queries_lower_written_fields_without_child_demands() {
        use crate::{
            db::{key::QueryKey, memo::Database, provider::SyntaxProvider, value::QueryValue},
            ir::{op::Op, pc::Phase, recipe::RecipeExit},
            source::{
                changes::{PreparedSourceInputs, SourceUnitInput},
                snapshot::SourcePath,
            },
        };
        let (parsed, body, index) = source(RECORD_ORDER);
        let plan = plans(&parsed, &body, &index).into_iter().next().unwrap();
        let ConstructorKind::Record {
            fields, rest: None, ..
        } = plan.kind()
        else {
            panic!("record absent")
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path: SourcePath::new("main.rs").unwrap(),
                crate_key: Digest([147; 32]),
                module_path: vec![],
                profile: ProfileIdentity::current(),
                cfg: CfgSet::guest_default(),
                blob: SourceBlob::new(RECORD_ORDER.as_bytes().to_vec()).unwrap(),
            }],
            std::collections::BTreeMap::new(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let parent = &index.nodes()[plan.node().0 as usize];
        let mut pcs = vec![index.pc(plan.node(), Phase::Entry).unwrap()];
        for field in fields {
            let raw = parent
                .operands
                .iter()
                .position(|node| *node == field.operand.node)
                .unwrap();
            pcs.push(
                index
                    .pc(plan.node(), Phase::Operand(u16::try_from(raw).unwrap()))
                    .unwrap(),
            );
        }
        pcs.push(index.pc(plan.node(), Phase::Commit).unwrap());
        for (written, pc) in pcs.iter().enumerate() {
            let result = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = result.value.as_ref() else {
                panic!("constructor thunk absent")
            };
            let expected = if written == 0 {
                Op::ConstructStart {
                    guard: parent.constructor_guard.unwrap(),
                }
            } else if written <= fields.len() {
                Op::ConstructField {
                    guard: parent.constructor_guard.unwrap(),
                    field: u32::try_from(written - 1).unwrap(),
                    source: fields[written - 1].operand.value,
                }
            } else {
                Op::ConstructFinish {
                    guard: parent.constructor_guard.unwrap(),
                    destination: parent.value.unwrap(),
                }
            };
            assert!(recipe.recipe().operations.contains(&expected));
            if written < fields.len() {
                assert_eq!(
                    recipe.recipe().exit,
                    RecipeExit::Next(
                        index
                            .pc(fields[written].operand.node, Phase::Entry)
                            .unwrap()
                    )
                );
            } else if written == fields.len() {
                assert_eq!(
                    recipe.recipe().exit,
                    RecipeExit::Next(index.pc(plan.node(), Phase::Commit).unwrap())
                );
            }
            let again = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            assert_eq!(again.fingerprint, result.fingerprint);
        }
        assert_eq!(db.trace.counts["ConstructorPlan:ProviderRun"], 1);
        assert!(!db.trace.events.iter().any(|event| matches!(&event.key,
            QueryKey::Thunk(pc) if pc.node != plan.node().0)));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::TypeSchema(_) | QueryKey::FunctionSignature(_) | QueryKey::Constant(_)
        )));
    }

    #[test]
    fn constructor_plan_refuses_foreign_body_bytes_and_profile() {
        let (parsed, body, index) = source(ZERO_REPEAT);
        let node = index
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Other))
            .unwrap()
            .id;
        let mut foreign = body.clone();
        foreign.body_hash = Digest([9; 32]);
        assert_eq!(
            parsed
                .constructor_plan(&foreign, &index, node, &CfgSet::guest_default())
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SNAPSHOT"
        );
        let mut profile = ProfileIdentity::current();
        profile.edition = 2021;
        let old = ParsedFile::parse_declarations(
            &SourceBlob::new(ZERO_REPEAT.as_bytes().to_vec()).unwrap(),
            profile,
        )
        .unwrap();
        let old_index = old.index_body(&body, Default::default()).unwrap();
        assert_eq!(
            parsed
                .constructor_plan(&body, &old_index, node, &CfgSet::guest_default())
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SNAPSHOT"
        );
    }

    #[test]
    fn rustc_constructor_query_is_typed_memoized_and_keeps_zero_repeat_operand_cold() {
        use crate::{
            db::{
                key::QueryKey, memo::Database, provider::SyntaxProvider, trace::Action,
                value::QueryValue,
            },
            ir::pc::Phase,
            source::{
                changes::{PreparedSourceInputs, SourceUnitInput},
                snapshot::SourcePath,
            },
        };
        let (parsed, body, index) = source(ZERO_REPEAT);
        let node = plans(&parsed, &body, &index)[0].node();
        let key = QueryKey::ConstructorPlan(index.pc(node, Phase::Entry).unwrap());
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path: SourcePath::new("main.rs").unwrap(),
                crate_key: Digest([147; 32]),
                module_path: vec![],
                profile: ProfileIdentity::current(),
                cfg: CfgSet::guest_default(),
                blob: SourceBlob::new(ZERO_REPEAT.as_bytes().to_vec()).unwrap(),
            }],
            std::collections::BTreeMap::new(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let first = db
            .fetch(&SyntaxProvider, key.clone())
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(first.value.matches(&key));
        let QueryValue::ConstructorPlan(plan) = first.value.as_ref() else {
            panic!("typed constructor outcome absent");
        };
        assert!(matches!(plan.kind(), ConstructorKind::Repeat { .. }));
        let QueryKey::ConstructorPlan(pc) = &key else {
            panic!("constructor key absent");
        };
        let mut wrong_phase = pc.clone();
        wrong_phase.phase = Phase::Commit.code().unwrap();
        assert!(
            !first
                .value
                .matches(&QueryKey::ConstructorPlan(wrong_phase.clone()))
        );
        let mut wrong_node = pc.clone();
        wrong_node.node = wrong_node.node.checked_add(1).unwrap();
        assert!(!first.value.matches(&QueryKey::ConstructorPlan(wrong_node)));
        let second = db
            .fetch(&SyntaxProvider, key.clone())
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        assert!(
            db.trace
                .events
                .iter()
                .any(|event| event.key == key && event.action == Action::MemoHit)
        );
        let invalid = db
            .fetch(&SyntaxProvider, QueryKey::ConstructorPlan(wrong_phase))
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(invalid.code, "LRR-E-QUERY-REQUEST");
        let mut cfg = CfgSet::guest_default();
        cfg.atoms.insert("lrr_constructor_probe".into());
        let refreshed = PreparedSourceInputs::refresh(
            Some(&prepared),
            vec![SourceUnitInput {
                path: SourcePath::new("main.rs").unwrap(),
                crate_key: Digest([147; 32]),
                module_path: vec![],
                profile: ProfileIdentity::current(),
                cfg,
                blob: SourceBlob::new(ZERO_REPEAT.as_bytes().to_vec()).unwrap(),
            }],
            std::collections::BTreeMap::new(),
        )
        .unwrap();
        refreshed.publish(&mut db).unwrap();
        let updated = db
            .fetch(&SyntaxProvider, key.clone())
            .into_ready()
            .unwrap()
            .unwrap();
        assert_ne!(first.fingerprint, updated.fingerprint);
        assert!(
            db.trace
                .events
                .iter()
                .any(|event| event.key == key && event.action == Action::Invalidated)
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_)
                | QueryKey::FunctionSignature(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::ExpansionInput(_)
                | QueryKey::CurrentExpansionInput(_)
        )));
        // The stock panic remains unevaluated; this is query acceptance only,
        // with no guest run-fail, subprocess or complete fixture claim.
    }
}
