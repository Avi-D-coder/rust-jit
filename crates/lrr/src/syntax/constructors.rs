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
    name_range: SourceRange,
    first_name_range: SourceRange,
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
    pub(crate) fn name_range(&self) -> SourceRange {
        self.name_range
    }
    pub(crate) fn first_name_range(&self) -> SourceRange {
        self.first_name_range
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
/// A range expression's lang-item struct (rustc's AST lowering of `a..b`,
/// `a..`, `..b`, `..` and `..=b` to `core::ops::Range*` struct literals).
/// The record plan names the item through the owning `core` crate, never
/// through the caller's scope, so user items cannot shadow it.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LangRecord {
    Range,
    RangeFrom,
    RangeTo,
    RangeFull,
    RangeToInclusive,
}
impl LangRecord {
    /// Path of the lang item inside the `core` crate.
    pub fn core_path(self) -> [&'static str; 2] {
        [
            "ops",
            match self {
                Self::Range => "Range",
                Self::RangeFrom => "RangeFrom",
                Self::RangeTo => "RangeTo",
                Self::RangeFull => "RangeFull",
                Self::RangeToInclusive => "RangeToInclusive",
            },
        ]
    }
    fn tag(self) -> u32 {
        match self {
            Self::Range => 0,
            Self::RangeFrom => 1,
            Self::RangeTo => 2,
            Self::RangeFull => 3,
            Self::RangeToInclusive => 4,
        }
    }
    /// The lang item a range expression constructs and its written fields.
    /// `a..=b` lowers to `RangeInclusive::new(a, b)` (private fields), which
    /// is not a record construction; it is `None` here.
    pub(crate) fn of(range: &ast::RangeExpr) -> Option<(Self, Option<ast::Expr>, Option<ast::Expr>)> {
        use ast::RangeItem;
        let inclusive = range.op_kind()? == ast::RangeOp::Inclusive;
        let (start, end) = (range.start(), range.end());
        let item = match (start.is_some(), end.is_some(), inclusive) {
            (true, true, false) => Self::Range,
            (true, false, false) => Self::RangeFrom,
            (false, true, false) => Self::RangeTo,
            (false, false, false) => Self::RangeFull,
            (false, true, true) => Self::RangeToInclusive,
            _ => return None,
        };
        Some((item, start, end))
    }
}
/// `a..=b` is rustc's `RangeInclusive::new(a, b)` (the type has private
/// fields, so it is not a record construction): the written bounds, when
/// both are present. Its callee is a language item resolved through the
/// profile's own `core` crate, never through the caller's names.
pub(crate) fn range_inclusive_bounds(range: &ast::RangeExpr) -> Option<(ast::Expr, ast::Expr)> {
    use ast::RangeItem;
    if range.op_kind()? != ast::RangeOp::Inclusive {
        return None;
    }
    Some((range.start()?, range.end()?))
}
/// The call path of `RangeInclusive::new` in the profile's `core` crate (a
/// `$crate`-rooted path, as a macro-emitted call names the crate defining it).
pub(crate) fn range_inclusive_new_path(profile: &ProfileIdentity) -> String {
    format!(
        "{}::ops::RangeInclusive::new",
        crate::frontend::callables::dollar_crate_segment(crate::facade::unit(profile, "core"))
    )
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
    /// `Some` for a range expression: a record of this `core` lang item.
    lang: Option<LangRecord>,
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
    /// The `core` lang-item struct a range expression's record constructs.
    pub fn lang_record(&self) -> Option<LangRecord> {
        self.lang
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
        if body.key() != *index.body()
            || plan.origin() != &index.span(plan.node())?
            || plan.profile() != index.profile()
            || plan.profile() != &self.profile
            || plan.cfg() != cfg.fingerprint()
        {
            return Err(fault(
                "record path differs from exact source constructor plan",
            ));
        }
        self.entered_body_root(body)?;
        let ConstructorKind::Record { path: original, .. } = plan.kind() else {
            return Err(fault("record path requested for another constructor kind"));
        };
        let first = original
            .first()
            .ok_or_else(|| fault("record path tokens absent"))?;
        let last = original
            .last()
            .ok_or_else(|| fault("record path tokens absent"))?;
        let expression = self
            .entered_node_syntax(body, index, plan.node())?
            .and_then(ast::RecordExpr::cast)
            .ok_or_else(|| fault("record constructor indexed expression absent"))?;
        let path = expression
            .path()
            .ok_or_else(|| fault("record path AST boundary absent"))?;
        if range(path.syntax().text_range())
            != (SourceRange {
                start: first.range.start,
                end: last.range.end,
            })
            || self.constructor_tokens(path.syntax())? != *original
        {
            return Err(fault("record path differs from exact source tokens"));
        }
        let mut suffix = Vec::new();
        let mut current = path;
        let absolute;
        let mut name = None;
        let mut name_range = None;
        let mut first_name_range = None;
        loop {
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
                    let actual_range = range(reference.syntax().text_range());
                    let edition = self.token_edition(actual_range)?;
                    // Generated and retained names require their actual token
                    // attachment; None remains the ordinary original context.
                    self.identifier_context(actual_range)?;
                    first_name_range = Some(actual_range);
                    crate::expand::scopes::MacroName::parse(reference.text(), edition)?;
                    let text = reference.text().to_string();
                    if name.is_none() {
                        name = Some(text.clone());
                        name_range = Some(actual_range);
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
            name_range: name_range.ok_or_else(|| fault("record final name occurrence absent"))?,
            first_name_range: first_name_range
                .ok_or_else(|| fault("record first name occurrence absent"))?,
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
        self.entered_body_root(body)?;
        let origin = index.span(node)?;
        let selected = index
            .nodes()
            .get(node.0 as usize)
            .ok_or_else(|| fault("constructor node absent"))?;
        if !matches!(selected.role, NodeRole::Expression(_)) {
            return Ok(None);
        }
        let syntax = self
            .entered_node_syntax(body, index, node)?
            .ok_or_else(|| fault("constructor expression boundary absent"))?;
        let expression =
            ast::Expr::cast(syntax).ok_or_else(|| fault("constructor expression absent"))?;
        if !matches!(
            expression,
            ast::Expr::TupleExpr(_) | ast::Expr::ArrayExpr(_) | ast::Expr::RecordExpr(_)
        ) && !matches!(&expression, ast::Expr::RangeExpr(range) if LangRecord::of(range).is_some())
        {
            return Ok(None);
        }
        let mut lang = None;
        let operand = |expr: ast::Expr| -> Result<ConstructorOperand, Diagnostic> {
            let r = range(expr.syntax().text_range());
            let relative = SourceRange {
                start: r.start.checked_sub(body.range.start).ok_or_else(limit)?,
                end: r.end.checked_sub(body.range.start).ok_or_else(limit)?,
            };
            let mut found = index
                .nodes_at_range(relative)
                .filter(|node| matches!(node.role, NodeRole::Expression(_)));
            let selected = found
                .next()
                .ok_or_else(|| fault("constructor operand absent from index"))?;
            if found.next().is_some() {
                return Err(fault("constructor operand boundary is ambiguous"));
            }
            let value = selected
                .value
                .ok_or_else(|| fault("constructor operand has no value slot"))?;
            Ok(ConstructorOperand {
                node: selected.id,
                value,
            })
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
            // `a..b` is the record `core::ops::Range { start: a, end: b }`.
            ast::Expr::RangeExpr(range) => {
                let (item, start, end) =
                    LangRecord::of(&range).ok_or_else(|| fault("range lang item absent"))?;
                let edition = self.profile.edition;
                let mut fields = Vec::new();
                for (name, expr) in [("start", start), ("end", end)] {
                    if let Some(expr) = expr {
                        push(
                            &mut fields,
                            WrittenField {
                                name: name.into(),
                                edition,
                                operand: operand(expr)?,
                            },
                        )?;
                    }
                }
                lang = Some(item);
                ConstructorKind::Record {
                    path: Vec::new(),
                    fields,
                    rest: None,
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
            lang,
        };
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
        e.option(&self.lang, |e, item| e.u32(item.tag()));
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
    /// rustc lowers `..`, `a..`, `..b` and `a..b` to `core::ops` lang-item
    /// struct literals; each reached range expression is that ordinary record
    /// plan with its written bounds as the named fields, in source order.
    #[test]
    fn rustc_range_expressions_plan_core_lang_item_records() {
        const ORIGINAL: &str =
            include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/slice.rs");
        let cfg = CfgSet::guest_default();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed.outline(Digest([149; 32]), vec![], &cfg).unwrap();
        let main = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let body = main.body.as_ref().unwrap();
        let index = parsed.index_body(body, Default::default()).unwrap();
        let plans: Vec<_> = index
            .nodes()
            .iter()
            .filter(|node| node.constructor_guard.is_some())
            .filter_map(|node| {
                parsed
                    .constructor_plan(body, &index, node.id, &cfg)
                    .unwrap()
            })
            .filter(|plan| plan.lang_record().is_some())
            .collect();
        let shapes: Vec<_> = plans
            .iter()
            .map(|plan| {
                let ConstructorKind::Record {
                    path,
                    fields,
                    rest: None,
                } = plan.kind()
                else {
                    panic!("range plan is not a plain record")
                };
                assert!(path.is_empty(), "a lang item is never named by caller tokens");
                (
                    plan.lang_record().unwrap(),
                    fields
                        .iter()
                        .map(|field| field.name.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let expected = [
            (LangRecord::RangeFull, vec![]),
            (LangRecord::RangeFrom, vec!["start"]),
            (LangRecord::RangeTo, vec!["end"]),
            (LangRecord::Range, vec!["start", "end"]),
        ];
        // `&x[..]` .. `&x[Foo..Foo]`, then the same four under `&mut`.
        assert_eq!(shapes.len(), 8);
        for (actual, expected) in shapes.iter().zip(expected.iter().chain(expected.iter())) {
            assert_eq!(actual.0, expected.0);
            assert_eq!(actual.1, expected.1);
        }
        assert_eq!(LangRecord::Range.core_path(), ["ops", "Range"]);
        // Distinct lang items keep distinct plan identities.
        assert_ne!(plans[0].fingerprint(), plans[4].fingerprint());
    }
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
