//! One retained structural tree per written pattern root. Every pattern owner
//! (match arms, `let`, `let`-`else`, `if let`/`while let` clauses and patterns
//! emitted by macros) builds the same tree from its indexed syntax; the VM's
//! single transactional matcher interprets it. Building is pure syntax work:
//! no namespace, type, value or executable preparation happens here. A form
//! that cannot be interpreted yet becomes an `Unsupported` leaf that faults
//! only when the matcher actually reaches it.
use super::{error, id_for};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::{
        op::{DecodedLiteral, PrimitiveType},
        pc::NodeId,
    },
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind,
    ast::{self, HasName, RangeItem},
};
use std::collections::BTreeSet;

/// A decoded literal endpoint. Typing happens against the reached scrutinee
/// value, exactly as rustc types a literal pattern by its scrutinee.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PatternLiteral {
    Integer {
        negative: bool,
        magnitude: u128,
        suffix: Option<PrimitiveType>,
    },
    Float {
        negative: bool,
        decimal: String,
        suffix: Option<PrimitiveType>,
    },
    Bool(bool),
    Char(char),
    Byte(u8),
    Str(String),
    Bytes(std::sync::Arc<[u8]>),
}
impl Eq for PatternLiteral {}

/// One written bound of a range pattern: a literal, or a path/identifier that
/// the pattern's name slot resolves (through the ordinary Value namespace) to
/// a constant, exactly as a constant pattern's name does.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RangeEndpoint {
    Literal(PatternLiteral),
    Constant(u32),
}
impl RangeEndpoint {
    fn encode(&self, e: &mut StableEncoder) {
        match self {
            Self::Literal(literal) => {
                e.u8(0);
                literal.encode(e);
            }
            Self::Constant(slot) => {
                e.u8(2);
                e.u32(*slot);
            }
        }
    }
}
impl PatternLiteral {
    fn encode(&self, e: &mut StableEncoder) {
        match self {
            Self::Integer {
                negative,
                magnitude,
                suffix,
            } => {
                e.u8(0);
                e.bool(*negative);
                e.bytes(&magnitude.to_le_bytes());
                e.option(suffix, |e, ty| e.u32(ty.tag()));
            }
            Self::Float {
                negative,
                decimal,
                suffix,
            } => {
                e.u8(1);
                e.bool(*negative);
                e.string(decimal);
                e.option(suffix, |e, ty| e.u32(ty.tag()));
            }
            Self::Bool(value) => {
                e.u8(2);
                e.bool(*value);
            }
            Self::Char(value) => {
                e.u8(3);
                e.u32(u32::from(*value));
            }
            Self::Byte(value) => {
                e.u8(4);
                e.u8(*value);
            }
            Self::Str(value) => {
                e.u8(5);
                e.string(value);
            }
            Self::Bytes(value) => {
                e.u8(6);
                e.bytes(value);
            }
        }
    }
}

/// What a written pattern name must resolve to through the ordinary
/// namespaces: a plain identifier (a fresh binding unless it names a unit
/// constructor or constant), a path pattern, a tuple-struct/tuple-variant
/// pattern or a record pattern (whose written field names are listed).
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum NameForm {
    Identifier,
    Path,
    Tuple,
    Record(Box<[String]>),
}
/// One resolution request, in the tree's stable slot order.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PatternName {
    pub(crate) spelling: String,
    pub(crate) form: NameForm,
    /// A plain identifier, or the single-name path of a path, tuple-struct
    /// or record pattern, that a block-local item or `use` may bind: the
    /// dispatcher selects it through the entered lexical Value goal of this
    /// indexed pattern node (block items shadow module items), never by a
    /// module-level lookup of its spelling.
    pub(crate) lexical: Option<NodeId>,
    /// Actual path-pattern owner; arguments stay at indexed source locations.
    pub(crate) source: Option<NodeId>,
}

impl PatternName {
    pub(crate) fn encode(&self, e: &mut StableEncoder) {
        e.string(&self.spelling);
        match &self.form {
            NameForm::Identifier => e.u8(0), NameForm::Path => e.u8(1), NameForm::Tuple => e.u8(2),
            NameForm::Record(names) => { e.u8(3); e.sequence(names, |e,name| e.string(name)); }
        }
        e.option(&self.lexical, |e,node| e.u32(node.0));
        e.option(&self.source, |e,node| e.u32(node.0));
    }
}

/// The written shape of a nominal pattern.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ConstructorForm {
    /// `E::A`, `S` or a constant path.
    Path,
    /// `E::B(p, ..)` / `S(p, q)`.
    Tuple {
        items: Box<[PatternNode]>,
        rest: Option<u32>,
    },
    /// `E::C { x, y: p, .. }`; field patterns in written order (shorthand
    /// fields are binding patterns), matched to declaration ordinals by the
    /// dispatcher's resolution of the same slot.
    Record {
        fields: Box<[PatternNode]>,
        rest: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum PatternNode {
    Wildcard,
    /// `x`, `mut x`, `ref x`, `ref mut x` and `x @ sub`. `binding` is the
    /// canonical lexical binding shared by every or-alternative occurrence,
    /// if its binding facts are resolved. A plain name may instead select a
    /// constant/constructor without creating a local. `name` is present only for a plain identifier,
    /// which ordinary Value-namespace resolution must classify before use.
    Binding {
        occurrence: NodeId,
        binding: Option<NodeId>,
        by_reference: bool,
        mutable: bool,
        edition: u16,
        name: Option<(u32, String)>,
        subpattern: Option<Box<PatternNode>>,
    },
    Literal(PatternLiteral),
    /// Each endpoint is tested in order when reached; an endpoint the
    /// engine cannot interpret yet faults only if its comparison is needed.
    Range {
        start: Option<Result<RangeEndpoint, String>>,
        end: Option<Result<RangeEndpoint, String>>,
        inclusive: bool,
    },
    Tuple {
        items: Box<[PatternNode]>,
        rest: Option<u32>,
    },
    /// `[a, b, ..]` over an array or slice; `rest` is the position of the
    /// `..`, and `rest_binding` the binding of a subslice `ref xs @ ..`,
    /// matched against the element window between the fixed items.
    Array {
        items: Box<[PatternNode]>,
        rest: Option<u32>,
        rest_binding: Option<Box<PatternNode>>,
    },
    Reference {
        mutable: bool,
        edition: u16,
        inner: Box<PatternNode>,
    },
    Or(Box<[PatternNode]>),
    /// A path, tuple-struct or record pattern; `slot` indexes its ordinary
    /// namespace resolution.
    Constructor {
        slot: u32,
        form: ConstructorForm,
    },
    Unsupported(String),
}
impl PatternNode {
    fn encode(&self, e: &mut StableEncoder) {
        match self {
            Self::Wildcard => e.u8(0),
            Self::Binding {
                occurrence,
                binding,
                by_reference,
                mutable,
                edition,
                name,
                subpattern,
            } => {
                e.u8(1);
                e.u32(occurrence.0);
                e.option(binding, |e, binding| e.u32(binding.0));
                e.bool(*by_reference);
                e.bool(*mutable);
                e.u16(*edition);
                e.option(name, |e, (slot, name)| {
                    e.u32(*slot);
                    e.string(name);
                });
                e.option(subpattern, |e, node| node.encode(e));
            }
            Self::Literal(literal) => {
                e.u8(2);
                literal.encode(e);
            }
            Self::Range {
                start,
                end,
                inclusive,
            } => {
                e.u8(3);
                for endpoint in [start, end] {
                    e.option(endpoint, |e, endpoint| match endpoint {
                        Ok(endpoint) => endpoint.encode(e),
                        Err(message) => {
                            e.u8(1);
                            e.string(message);
                        }
                    });
                }
                e.bool(*inclusive);
            }
            Self::Tuple { items, rest } => {
                e.u8(4);
                e.sequence(items, |e, item| item.encode(e));
                e.option(rest, |e, rest| e.u32(*rest));
            }
            Self::Array {
                items,
                rest,
                rest_binding,
            } => {
                e.u8(8);
                e.sequence(items, |e, item| item.encode(e));
                e.option(rest, |e, rest| e.u32(*rest));
                e.option(rest_binding, |e, binding| binding.encode(e));
            }
            Self::Reference {
                mutable,
                edition,
                inner,
            } => {
                e.u8(5);
                e.bool(*mutable);
                e.u16(*edition);
                inner.encode(e);
            }
            Self::Or(alternatives) => {
                e.u8(6);
                e.sequence(alternatives, |e, item| item.encode(e));
            }
            Self::Unsupported(message) => {
                e.u8(7);
                e.string(message);
            }
            Self::Constructor { slot, form } => {
                e.u8(9);
                e.u32(*slot);
                match form {
                    ConstructorForm::Path => e.u8(0),
                    ConstructorForm::Tuple { items, rest } => {
                        e.u8(1);
                        e.sequence(items, |e, item| item.encode(e));
                        e.option(rest, |e, rest| e.u32(*rest));
                    }
                    ConstructorForm::Record { fields, rest } => {
                        e.u8(2);
                        e.sequence(fields, |e, item| item.encode(e));
                        e.bool(*rest);
                    }
                }
            }
        }
    }
    fn heap_bytes(&self) -> usize {
        let node = std::mem::size_of::<Self>();
        match self {
            Self::Binding {
                name, subpattern, ..
            } => name
                .as_ref()
                .map_or(0, |(_, name)| name.capacity())
                .saturating_add(subpattern.as_ref().map_or(0, |sub| node + sub.heap_bytes())),
            Self::Literal(PatternLiteral::Str(text)) => text.capacity(),
            Self::Literal(PatternLiteral::Bytes(bytes)) => bytes.len(),
            Self::Literal(PatternLiteral::Float { decimal, .. }) => decimal.capacity(),
            Self::Range { start, end, .. } => [start, end]
                .into_iter()
                .flatten()
                .map(|endpoint| match endpoint {
                    Ok(RangeEndpoint::Literal(
                        PatternLiteral::Str(text) | PatternLiteral::Float { decimal: text, .. },
                    )) => text.capacity(),
                    Ok(RangeEndpoint::Literal(PatternLiteral::Bytes(bytes))) => bytes.len(),
                    Ok(_) => 0,
                    Err(message) => message.capacity(),
                })
                .sum(),
            Self::Array {
                items,
                rest_binding: Some(binding),
                ..
            } => items
                .iter()
                .fold(items.len().saturating_mul(node), |bytes, item| {
                    bytes.saturating_add(item.heap_bytes())
                })
                .saturating_add(node.saturating_add(binding.heap_bytes())),
            Self::Tuple { items, .. } | Self::Array { items, .. } | Self::Or(items) => items
                .iter()
                .fold(items.len().saturating_mul(node), |bytes, item| {
                    bytes.saturating_add(item.heap_bytes())
                }),
            Self::Reference { inner, .. } => node.saturating_add(inner.heap_bytes()),
            Self::Unsupported(message) => message.capacity(),
            Self::Constructor { form, .. } => match form {
                ConstructorForm::Path => 0,
                ConstructorForm::Tuple { items, .. }
                | ConstructorForm::Record { fields: items, .. } => items
                    .iter()
                    .fold(items.len().saturating_mul(node), |bytes, item| {
                        bytes.saturating_add(item.heap_bytes())
                    }),
            },
            Self::Wildcard | Self::Literal(_) => 0,
        }
    }
}

/// Retained, immutable and shared by every recipe of its owner.
#[derive(Debug)]
pub struct PatternTree {
    pattern: NodeId,
    root: PatternNode,
    names: Box<[PatternName]>,
    identity: Digest,
    bytes: usize,
}
impl PatternTree {
    pub(crate) fn pattern(&self) -> NodeId {
        self.pattern
    }
    pub(crate) fn root(&self) -> &PatternNode {
        &self.root
    }
    /// Names that need ordinary namespace resolution, in slot order.
    pub(crate) fn names(&self) -> &[PatternName] {
        &self.names
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
}

struct Builder<'a> {
    parsed: &'a ParsedFile,
    body: &'a BodyHandle,
    index: &'a BodyIndex,
    local_items: &'a LocalItems,
    names: Vec<PatternName>,
}

/// Block-local Value items visible anywhere in the entered body. A plain
/// identifier with one of these spellings may name such an item rather than
/// bind; its name slot is selected through the entered lexical Value goal.
#[derive(Debug, Default)]
pub(crate) struct LocalItems {
    names: BTreeSet<String>,
    glob_import: bool,
}
impl LocalItems {
    pub(crate) fn collect(parsed: &ParsedFile, body: &BodyHandle) -> Result<Self, Diagnostic> {
        let mut items = Self::default();
        let root = parsed.entered_body_root(body)?;
        for node in root.descendants() {
            match node.kind() {
                SyntaxKind::CONST | SyntaxKind::STATIC | SyntaxKind::STRUCT => {
                    if let Some(name) = node.children().find_map(ast::Name::cast) {
                        items.names.insert(name.text().to_string());
                    }
                }
                SyntaxKind::USE_TREE => {
                    let tree = ast::UseTree::cast(node).expect("checked use tree");
                    if tree.star_token().is_some() {
                        items.glob_import = true;
                    } else if tree.use_tree_list().is_none() {
                        if let Some(rename) = tree.rename().and_then(|rename| rename.name()) {
                            items.names.insert(rename.text().to_string());
                        } else if let Some(segment) = tree.path().and_then(|path| path.segment())
                            && let Some(name) = segment.name_ref()
                        {
                            items.names.insert(name.text().to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(items)
    }
    fn may_name(&self, name: &str) -> bool {
        self.glob_import || self.names.contains(name)
    }
}

fn literal(pattern: &ast::LiteralPat) -> Result<PatternLiteral, String> {
    let negative = pattern.minus_token().is_some();
    let Some(literal) = pattern.literal() else {
        return Err("literal pattern token absent".into());
    };
    let token = literal.token();
    Ok(match token.kind() {
        SyntaxKind::INT_NUMBER => {
            let number = ast::IntNumber::cast(token).ok_or("integer token cast failed")?;
            if matches!(number.suffix(), Some("f32" | "f64"))
                && number.radix() == ast::Radix::Decimal
            {
                PatternLiteral::Float {
                    negative,
                    decimal: number.value_string(),
                    suffix: Some(if number.suffix() == Some("f32") {
                        PrimitiveType::F32
                    } else {
                        PrimitiveType::F64
                    }),
                }
            } else {
                match super::super::literal_decode::integer(&number, negative) {
                    DecodedLiteral::Integer {
                        negative,
                        magnitude,
                        suffix,
                        ..
                    } => PatternLiteral::Integer {
                        negative,
                        magnitude,
                        suffix,
                    },
                    _ => return Err("integer pattern literal is invalid".into()),
                }
            }
        }
        SyntaxKind::FLOAT_NUMBER => {
            let number = ast::FloatNumber::cast(token).ok_or("float token cast failed")?;
            PatternLiteral::Float {
                negative,
                decimal: number.value_string(),
                suffix: match number.suffix() {
                    None => None,
                    Some("f32") => Some(PrimitiveType::F32),
                    Some("f64") => Some(PrimitiveType::F64),
                    Some(_) => return Err("float pattern suffix is invalid".into()),
                },
            }
        }
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if !negative => {
            PatternLiteral::Bool(token.kind() == SyntaxKind::TRUE_KW)
        }
        SyntaxKind::CHAR if !negative => PatternLiteral::Char(
            ast::Char::cast(token)
                .ok_or("char token cast failed")?
                .value()
                .map_err(|_| "invalid char pattern escape".to_string())?,
        ),
        SyntaxKind::BYTE if !negative => PatternLiteral::Byte(
            ast::Byte::cast(token)
                .ok_or("byte token cast failed")?
                .value()
                .map_err(|_| "invalid byte pattern escape".to_string())?,
        ),
        SyntaxKind::STRING if !negative => PatternLiteral::Str(
            ast::String::cast(token)
                .ok_or("string token cast failed")?
                .value()
                .map_err(|_| "invalid string pattern escape".to_string())?
                .into_owned(),
        ),
        SyntaxKind::BYTE_STRING if !negative => PatternLiteral::Bytes(std::sync::Arc::from(
            ast::ByteString::cast(token)
                .ok_or("byte string token cast failed")?
                .value()
                .map_err(|_| "invalid byte string pattern escape".to_string())?
                .as_ref(),
        )),
        _ => return Err("literal pattern kind requires its reached semantics".into()),
    })
}

impl Builder<'_> {
    fn indexed(&self, pattern: &ast::Pat) -> Result<NodeId, Diagnostic> {
        let id = id_for(self.index, self.body, pattern.syntax())?;
        if self.index.nodes()[id.0 as usize].role != NodeRole::Pattern {
            return Err(error("pattern tree node is not an indexed pattern"));
        }
        Ok(id)
    }
    /// The owning edition of the pattern's first written token: macro output
    /// keeps each token's own edition, so never span several tokens.
    fn edition(&self, pattern: &ast::Pat) -> Result<u16, Diagnostic> {
        let token = pattern
            .syntax()
            .first_token()
            .ok_or_else(|| error("pattern has no written token"))?;
        self.parsed
            .token_edition(crate::syntax::ast_adapter::range(token.text_range()))
    }
    /// A range bound is a literal or a constant named by a path or a bare
    /// identifier; the name takes a slot resolved by the ordinary Value route
    /// (an identifier that names no constant faults only if the bound is
    /// reached, as for a constant pattern).
    fn endpoint(
        &mut self,
        pattern: Option<ast::Pat>,
    ) -> Result<Option<Result<RangeEndpoint, String>>, Diagnostic> {
        Ok(match pattern {
            None => None,
            Some(ast::Pat::LiteralPat(endpoint)) => {
                Some(literal(&endpoint).map(RangeEndpoint::Literal))
            }
            Some(owner @ ast::Pat::PathPat(_)) => {
                let ast::Pat::PathPat(path) = &owner else {
                    unreachable!("matched path pattern");
                };
                Some(
                    self.path_slot(&owner, path.path(), NameForm::Path)?
                        .map(RangeEndpoint::Constant),
                )
            }
            Some(owner @ ast::Pat::IdentPat(_)) => {
                let ast::Pat::IdentPat(ident) = &owner else {
                    unreachable!("matched identifier pattern");
                };
                match ident.name() {
                    Some(name)
                        if ident.ref_token().is_none()
                            && ident.mut_token().is_none()
                            && ident.pat().is_none() =>
                    {
                        let text = name.text().to_string();
                        let slot = self.slot(&text, NameForm::Identifier)?;
                        if self.local_items.may_name(text.trim_start_matches("r#")) {
                            let occurrence = self.indexed(&owner)?;
                            self.names[slot as usize].lexical = Some(occurrence);
                        }
                        Some(Ok(RangeEndpoint::Constant(slot)))
                    }
                    _ => Some(Err("range pattern endpoint is not a constant".into())),
                }
            }
            Some(_) => Some(Err(
                "range pattern endpoint requires constant path integration".into(),
            )),
        })
    }
    /// An identifier pattern. A plain identifier (no `ref`/`mut`/`@`) is a
    /// fresh binding unless ordinary resolution of its slot names a unit
    /// constructor or constant; a record shorthand field always binds.
    fn binding(
        &mut self,
        pattern: &ast::Pat,
        ident: &ast::IdentPat,
        shorthand: bool,
        depth: usize,
    ) -> Result<PatternNode, Diagnostic> {
        let occurrence = self.indexed(pattern)?;
        let Some(name) = ident.name() else {
            return Ok(PatternNode::Unsupported("binding name absent".into()));
        };
        let Some(group) = self.index.pattern_bindings().group_for(occurrence) else {
            return Ok(PatternNode::Unsupported(
                "pattern binding group is not indexed".into(),
            ));
        };
        let binding = group.canonical_candidate();
        let subpattern = match ident.pat() {
            Some(sub) if ident.at_token().is_some() => Some(Box::new(self.build(&sub, depth + 1)?)),
            Some(_) => {
                return Ok(PatternNode::Unsupported(
                    "binding subpattern requires `@`".into(),
                ));
            }
            None => None,
        };
        let by_reference = ident.ref_token().is_some();
        let mutable = ident.mut_token().is_some();
        let text = name.text().to_string();
        let plain = !shorthand && !by_reference && !mutable && subpattern.is_none();
        if binding.is_none() && !plain {
            return Ok(PatternNode::Unsupported(
                "pattern binding candidates have unresolved alternative, mode or hygiene facts"
                    .into(),
            ));
        }
        let name = if plain {
            let slot = self.slot(&text, NameForm::Identifier)?;
            if self.local_items.may_name(text.trim_start_matches("r#")) {
                self.names[slot as usize].lexical = Some(occurrence);
            }
            Some((slot, text))
        } else {
            None
        };
        Ok(PatternNode::Binding {
            occurrence,
            binding,
            by_reference,
            mutable,
            edition: self.edition(pattern)?,
            name,
            subpattern,
        })
    }
    fn slot(&mut self, spelling: &str, form: NameForm) -> Result<u32, Diagnostic> {
        let slot =
            u32::try_from(self.names.len()).map_err(|_| error("pattern name slot overflow"))?;
        self.names.push(PatternName {
            spelling: spelling.to_string(),
            form,
            lexical: None,
            source: None,
        });
        Ok(slot)
    }
    /// The written path of a nominal pattern, spelled exactly as an ordinary
    /// value path or callee so it resolves through the same route.
    fn path_slot(
        &mut self,
        owner: &ast::Pat,
        path: Option<ast::Path>,
        form: NameForm,
    ) -> Result<Result<u32, String>, Diagnostic> {
        let Some(path) = path else {
            return Ok(Err("pattern path absent".into()));
        };
        // A single name a block item or `use` may bind is selected through
        // the entered lexical Value goal of the pattern itself (block items
        // shadow module items), never by a module-level lookup.
        let lexical = self.parsed.pattern_path_binder(&path, self.index.cfg())?;
        match self.parsed.written_pattern_path(path, self.index.cfg())? {
            super::value_paths::WrittenValuePath::Spelled(spelling) => {
                let slot = self.slot(&spelling, form)?;
                self.names[slot as usize].source = Some(self.indexed(owner)?);
                if lexical {
                    let occurrence = self.indexed(owner)?;
                    self.names[slot as usize].lexical = Some(occurrence);
                }
                Ok(Ok(slot))
            }
            super::value_paths::WrittenValuePath::Refused(reason) => Ok(Err(reason.into())),
        }
    }
    fn build(&mut self, pattern: &ast::Pat, depth: usize) -> Result<PatternNode, Diagnostic> {
        if pattern
            .syntax()
            .children()
            .any(|child| ast::Attr::can_cast(child.kind()))
        {
            return Ok(PatternNode::Unsupported(
                "pattern attributes require cfg integration".into(),
            ));
        }
        Ok(match pattern {
            ast::Pat::WildcardPat(_) => PatternNode::Wildcard,
            ast::Pat::ParenPat(group) => match group.pat() {
                Some(inner) => self.build(&inner, depth + 1)?,
                None => PatternNode::Unsupported("parenthesized pattern is empty".into()),
            },
            ast::Pat::IdentPat(ident) => self.binding(pattern, ident, false, depth)?,
            ast::Pat::LiteralPat(written) => match literal(written) {
                Ok(literal) => PatternNode::Literal(literal),
                Err(message) => PatternNode::Unsupported(message),
            },
            ast::Pat::RangePat(range) => {
                let inclusive = match range.op_kind() {
                    Some(ast::RangeOp::Inclusive) => true,
                    Some(ast::RangeOp::Exclusive) => false,
                    None => {
                        return Ok(PatternNode::Unsupported(
                            "range pattern operator absent".into(),
                        ));
                    }
                };
                let start = self.endpoint(range.start())?;
                let end = self.endpoint(range.end())?;
                match (start, end) {
                    (None, None) => {
                        PatternNode::Unsupported("range pattern has no endpoint".into())
                    }
                    (start, end) => PatternNode::Range {
                        start,
                        end,
                        inclusive,
                    },
                }
            }
            ast::Pat::TuplePat(tuple) => {
                let mut items = Vec::new();
                let mut rest = None;
                for (ordinal, item) in tuple.fields().enumerate() {
                    if matches!(item, ast::Pat::RestPat(_)) {
                        if rest.is_some() {
                            return Ok(PatternNode::Unsupported(
                                "tuple pattern has more than one rest".into(),
                            ));
                        }
                        rest = Some(
                            u32::try_from(ordinal)
                                .map_err(|_| error("tuple pattern arity overflow"))?,
                        );
                        continue;
                    }
                    items.push(self.build(&item, depth + 1)?);
                }
                PatternNode::Tuple {
                    items: items.into_boxed_slice(),
                    rest,
                }
            }
            ast::Pat::RefPat(reference) => match reference.pat() {
                Some(inner) => PatternNode::Reference {
                    mutable: reference.mut_token().is_some(),
                    edition: self.edition(pattern)?,
                    inner: Box::new(self.build(&inner, depth + 1)?),
                },
                None => PatternNode::Unsupported("reference pattern operand absent".into()),
            },
            ast::Pat::OrPat(or) => {
                let mut alternatives = Vec::new();
                for alternative in or.pats() {
                    alternatives.push(self.build(&alternative, depth + 1)?);
                }
                if alternatives.is_empty() {
                    PatternNode::Unsupported("or-pattern has no alternatives".into())
                } else {
                    PatternNode::Or(alternatives.into_boxed_slice())
                }
            }
            ast::Pat::RestPat(_) => {
                PatternNode::Unsupported("rest pattern outside a tuple or slice".into())
            }
            ast::Pat::PathPat(path) => {
                match self.path_slot(pattern, path.path(), NameForm::Path)? {
                    Ok(slot) => PatternNode::Constructor {
                        slot,
                        form: ConstructorForm::Path,
                    },
                    Err(message) => PatternNode::Unsupported(message),
                }
            }
            ast::Pat::TupleStructPat(tuple) => {
                let mut items = Vec::new();
                let mut rest = None;
                for (ordinal, item) in tuple.fields().enumerate() {
                    if matches!(item, ast::Pat::RestPat(_)) {
                        if rest.is_some() {
                            return Ok(PatternNode::Unsupported(
                                "tuple-struct pattern has more than one rest".into(),
                            ));
                        }
                        rest = Some(
                            u32::try_from(ordinal)
                                .map_err(|_| error("tuple-struct pattern arity overflow"))?,
                        );
                        continue;
                    }
                    items.push(self.build(&item, depth + 1)?);
                }
                match self.path_slot(pattern, tuple.path(), NameForm::Tuple)? {
                    Ok(slot) => PatternNode::Constructor {
                        slot,
                        form: ConstructorForm::Tuple {
                            items: items.into_boxed_slice(),
                            rest,
                        },
                    },
                    Err(message) => PatternNode::Unsupported(message),
                }
            }
            ast::Pat::RecordPat(record) => {
                let Some(list) = record.record_pat_field_list() else {
                    return Ok(PatternNode::Unsupported(
                        "record pattern fields absent".into(),
                    ));
                };
                let mut names = Vec::new();
                let mut fields = Vec::new();
                for field in list.fields() {
                    if field
                        .syntax()
                        .children()
                        .any(|child| ast::Attr::can_cast(child.kind()))
                    {
                        return Ok(PatternNode::Unsupported(
                            "record pattern field attributes require cfg integration".into(),
                        ));
                    }
                    let Some(pattern) = field.pat() else {
                        return Ok(PatternNode::Unsupported(
                            "record pattern field pattern absent".into(),
                        ));
                    };
                    // `x` / `ref mut x` shorthand: its own binding is the field.
                    let name = match field.field_name() {
                        Some(name) => name.text().to_string(),
                        None => match &pattern {
                            ast::Pat::IdentPat(binding) => match binding.name() {
                                Some(name) => name.text().to_string(),
                                None => {
                                    return Ok(PatternNode::Unsupported(
                                        "record shorthand field name absent".into(),
                                    ));
                                }
                            },
                            _ => {
                                return Ok(PatternNode::Unsupported(
                                    "record pattern field name absent".into(),
                                ));
                            }
                        },
                    };
                    let shorthand = field.field_name().is_none();
                    let node = if shorthand && let ast::Pat::IdentPat(binding) = &pattern {
                        // A shorthand field always binds; it never names a constant.
                        self.binding(&pattern, binding, true, depth)?
                    } else {
                        self.build(&pattern, depth + 1)?
                    };
                    names.push(name.trim_start_matches("r#").to_string());
                    fields.push(node);
                }
                let rest = list.rest_pat().is_some();
                match self.path_slot(
                    pattern,
                    record.path(),
                    NameForm::Record(names.into_boxed_slice()),
                )? {
                    Ok(slot) => PatternNode::Constructor {
                        slot,
                        form: ConstructorForm::Record {
                            fields: fields.into_boxed_slice(),
                            rest,
                        },
                    },
                    Err(message) => PatternNode::Unsupported(message),
                }
            }
            ast::Pat::SlicePat(slice) => {
                let mut items = Vec::new();
                let mut rest = None;
                let mut rest_binding = None;
                for (ordinal, item) in slice.pats().enumerate() {
                    // `xs @ ..`: the binding of the rest's element window.
                    if let ast::Pat::IdentPat(ident) = &item
                        && matches!(ident.pat(), Some(ast::Pat::RestPat(_)))
                    {
                        if rest.is_some() {
                            return Ok(PatternNode::Unsupported(
                                "slice pattern has more than one rest".into(),
                            ));
                        }
                        rest = Some(
                            u32::try_from(ordinal)
                                .map_err(|_| error("slice pattern arity overflow"))?,
                        );
                        let binding = match self.binding(&item, ident, false, depth + 1)? {
                            PatternNode::Binding {
                                occurrence,
                                binding,
                                by_reference,
                                mutable,
                                edition,
                                name,
                                ..
                            } => PatternNode::Binding {
                                occurrence,
                                binding,
                                by_reference,
                                mutable,
                                edition,
                                name,
                                subpattern: None,
                            },
                            other => other,
                        };
                        rest_binding = Some(Box::new(binding));
                        continue;
                    }
                    if matches!(item, ast::Pat::RestPat(_)) {
                        if rest.is_some() {
                            return Ok(PatternNode::Unsupported(
                                "slice pattern has more than one rest".into(),
                            ));
                        }
                        rest = Some(
                            u32::try_from(ordinal)
                                .map_err(|_| error("slice pattern arity overflow"))?,
                        );
                        continue;
                    }
                    items.push(self.build(&item, depth + 1)?);
                }
                PatternNode::Array {
                    items: items.into_boxed_slice(),
                    rest,
                    rest_binding,
                }
            }
            ast::Pat::BoxPat(_) => PatternNode::Unsupported("box pattern is unstable".into()),
            ast::Pat::MacroPat(_) => PatternNode::Unsupported(
                "pattern macro requires its expanded pattern syntax".into(),
            ),
            _ => PatternNode::Unsupported("pattern form requires its reached semantics".into()),
        })
    }
}

impl PatternTree {
    /// Build the retained tree for one written root pattern of an owner.
    pub(crate) fn build(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        local_items: &LocalItems,
        pattern: &ast::Pat,
    ) -> Result<Self, Diagnostic> {
        let mut builder = Builder {
            parsed,
            body,
            index,
            local_items,
            names: Vec::new(),
        };
        let id = builder.indexed(pattern)?;
        let root = builder.build(pattern, 0)?;
        let mut encoder = StableEncoder::new("lrr/v7/pattern-tree");
        encoder.record(index.body());
        encoder.u32(id.0);
        root.encode(&mut encoder);
        encoder.sequence(&builder.names, |e, name| name.encode(e));
        let bytes = std::mem::size_of::<Self>()
            .saturating_add(root.heap_bytes())
            .saturating_add(builder.names.iter().fold(0usize, |bytes, name| {
                bytes
                    .saturating_add(std::mem::size_of::<PatternName>())
                    .saturating_add(name.spelling.capacity())
            }));
        Ok(Self {
            pattern: id,
            names: builder.names.into_boxed_slice(),
            root,
            identity: encoder.finish(),
            bytes,
        })
    }
}
impl PatternTree {
    /// The `Some(PAT)` arm pattern of rustc's `for` lowering, over each
    /// `Iterator::next` result. `Some` is the lang item `Option::Some`,
    /// spelled as its absolute core path (never the caller's `Some`).
    pub(crate) fn build_for(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        local_items: &LocalItems,
        pattern: &ast::Pat,
    ) -> Result<Self, Diagnostic> {
        let mut builder = Builder {
            parsed,
            body,
            index,
            local_items,
            names: Vec::new(),
        };
        let id = builder.indexed(pattern)?;
        let some = builder.slot(OPTION_SOME_LANG_PATH, NameForm::Tuple)?;
        let inner = builder.build(pattern, 1)?;
        let root = PatternNode::Constructor {
            slot: some,
            form: ConstructorForm::Tuple {
                items: Box::new([inner]),
                rest: None,
            },
        };
        let mut encoder = StableEncoder::new("lrr/v3/for-pattern-tree");
        encoder.record(index.body());
        encoder.u32(id.0);
        root.encode(&mut encoder);
        encoder.sequence(&builder.names, |e, name| name.encode(e));
        let bytes = std::mem::size_of::<Self>()
            .saturating_add(root.heap_bytes())
            .saturating_add(builder.names.iter().fold(0usize, |bytes, name| {
                bytes
                    .saturating_add(std::mem::size_of::<PatternName>())
                    .saturating_add(name.spelling.capacity())
            }));
        Ok(Self {
            pattern: id,
            names: builder.names.into_boxed_slice(),
            root,
            identity: encoder.finish(),
            bytes,
        })
    }
}
/// The lang item `Option::Some` of `for` lowering. Resolution falls back to
/// the same path through `std` where `core` is not nameable (Rust 2015
/// crates name only their injected `std` from the crate root).
pub(crate) const OPTION_SOME_LANG_PATH: &str = "::core::option::Option::Some";
impl PartialEq for PatternTree {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for PatternTree {}
impl StableEncode for PatternTree {
    const DOMAIN: &'static str = "lrr/v8/pattern-tree-identity";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
    }
}
