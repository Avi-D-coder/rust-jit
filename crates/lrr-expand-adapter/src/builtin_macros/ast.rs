//! Printing-only rustc-shaped AST built from the pinned Rust-analyzer parse of
//! macro input tokens. It exists solely to reproduce rustc's
//! `pprust::expr_to_string` text for `assert!` messages; it carries no
//! semantics and is never lowered or evaluated.
//!
//! Every syntax token is aligned with the owned wire leaves it was built from,
//! so identifier editions, `$crate` authority and macro-call token trees (with
//! their original spacing) come from the actual input records.
use super::tts::{self, Delim, DollarCrate, SpanIndex, Tt};
use crate::{
    AdapterError,
    token_wire::{Delimiter, Edition, Key, TokenTree},
};
use std::collections::HashMap;
use syntax::{
    AstNode, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, T, TextSize,
    ast::{
        self, HasArgList, HasGenericArgs, HasGenericParams, HasLoopBody, HasName, HasTypeBounds,
        HasVisibility, RangeItem,
    },
};

pub(crate) fn unsupported(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-BUILTIN-UNSUPPORTED",
        message: message.into(),
    }
}
fn fault(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-BUILTIN",
        message: message.into(),
    }
}

// ----- model -------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Path {
    pub(crate) segments: Vec<PathSegment>,
}
#[derive(Clone, Debug)]
pub(crate) struct PathSegment {
    /// Printed identifier (rustc `IdentPrinter` result); empty for `{{root}}`.
    pub(crate) ident: String,
    pub(crate) args: Option<GenericArgs>,
}
#[derive(Clone, Debug)]
pub(crate) struct QSelf {
    pub(crate) ty: Box<Ty>,
    pub(crate) position: usize,
}
#[derive(Clone, Debug)]
pub(crate) enum GenericArgs {
    Angle(Vec<AngleArg>),
    Paren {
        inputs: Vec<Ty>,
        output: Option<Box<Ty>>,
    },
    ParenElided,
}
#[derive(Clone, Debug)]
pub(crate) enum AngleArg {
    Lifetime(String),
    Type(Ty),
    Const(Expr),
    Constraint {
        ident: String,
        args: Option<GenericArgs>,
        kind: ConstraintKind,
    },
}
#[derive(Clone, Debug)]
pub(crate) enum ConstraintKind {
    EqualityTy(Ty),
    EqualityConst(Expr),
    Bound(Vec<Bound>),
}
#[derive(Clone, Debug)]
pub(crate) enum Bound {
    Trait {
        parens: bool,
        for_params: Vec<GenericParam>,
        constness: Option<&'static str>,
        asyncness: bool,
        polarity: Option<&'static str>,
        path: Path,
    },
    Outlives(String),
    Use(Vec<UseArg>),
}
#[derive(Clone, Debug)]
pub(crate) enum UseArg {
    Path(Path),
    Lifetime(String),
}
#[derive(Clone, Debug)]
pub(crate) enum GenericParam {
    Lifetime {
        name: String,
        bounds: Vec<String>,
    },
    Type {
        ident: String,
        bounds: Vec<Bound>,
        default: Option<Ty>,
    },
    Const {
        ident: String,
        ty: Ty,
        default: Option<Expr>,
    },
}
#[derive(Clone, Debug)]
pub(crate) enum Ty {
    Slice(Box<Ty>),
    Array(Box<Ty>, Box<Expr>),
    Ptr(bool, Box<Ty>),
    Ref(Option<String>, bool, Box<Ty>),
    Never,
    Tup(Vec<Ty>),
    Paren(Box<Ty>),
    FnPtr {
        generic_params: Vec<GenericParam>,
        unsafety: bool,
        abi: Option<Option<String>>,
        params: Vec<Param>,
        output: Option<Box<Ty>>,
    },
    Path(Option<QSelf>, Path),
    TraitObject(bool, Vec<Bound>),
    ImplTrait(Vec<Bound>),
    Infer,
    MacCall(MacCall),
}
#[derive(Clone, Debug)]
pub(crate) struct Param {
    pub(crate) pat: Option<Pat>,
    pub(crate) ty: Option<Ty>,
}
#[derive(Clone, Debug)]
pub(crate) struct MacCall {
    pub(crate) path: Path,
    pub(crate) delim: Delim,
    pub(crate) tts: Vec<Tt>,
}
#[derive(Clone, Debug)]
pub(crate) struct PatField {
    pub(crate) ident: Option<String>,
    pub(crate) pat: Pat,
}
#[derive(Clone, Debug)]
pub(crate) enum Pat {
    Wild,
    Ident {
        by_ref: bool,
        mutbl: bool,
        ident: String,
        sub: Option<Box<Pat>>,
    },
    TupleStruct(Option<QSelf>, Path, Vec<Pat>),
    Or(Vec<Pat>),
    Path(Option<QSelf>, Path),
    Struct(Option<QSelf>, Path, Vec<PatField>, bool),
    Tuple(Vec<Pat>),
    Deref(Box<Pat>),
    Ref(Box<Pat>, bool),
    Expr(Box<Expr>),
    Range(Option<Box<Expr>>, Option<Box<Expr>>, &'static str),
    Slice(Vec<Pat>),
    Rest,
    Paren(Box<Pat>),
    MacCall(MacCall),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    BitXor,
    BitAnd,
    BitOr,
    Shl,
    Shr,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
}
impl BinOp {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitXor => "^",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Ne => "!=",
            BinOp::Ge => ">=",
            BinOp::Gt => ">",
        }
    }
    /// rustc `BinOpKind::precedence`.
    pub(crate) fn precedence(self) -> Prec {
        match self {
            BinOp::Mul | BinOp::Div | BinOp::Rem => Prec::Product,
            BinOp::Add | BinOp::Sub => Prec::Sum,
            BinOp::Shl | BinOp::Shr => Prec::Shift,
            BinOp::BitAnd => Prec::BitAnd,
            BinOp::BitXor => Prec::BitXor,
            BinOp::BitOr => Prec::BitOr,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::Eq | BinOp::Ne => Prec::Compare,
            BinOp::And => Prec::LAnd,
            BinOp::Or => Prec::LOr,
        }
    }
    pub(crate) fn is_comparison(self) -> bool {
        self.precedence() == Prec::Compare
    }
}
/// rustc `ExprPrecedence`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Prec {
    Jump,
    Assign,
    Range,
    LOr,
    LAnd,
    Compare,
    BitOr,
    BitXor,
    BitAnd,
    Shift,
    Sum,
    Product,
    Cast,
    Prefix,
    Unambiguous,
}
#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub(crate) unsafe_: bool,
    pub(crate) stmts: Vec<Stmt>,
}
#[derive(Clone, Debug)]
pub(crate) enum Stmt {
    Let {
        super_: bool,
        pat: Pat,
        ty: Option<Ty>,
        init: Option<Expr>,
        els: Option<Block>,
    },
    Expr(Expr),
    Semi(Expr),
    Empty,
    MacCall(MacCall, bool),
    Item(Item),
}
// Printing-only declaration structure shared by all statement item sites.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub(crate) visibility: Visibility,
    pub(crate) ident: String,
    pub(crate) generics: Vec<GenericParam>,
    pub(crate) predicates: Vec<WherePredicate>,
    pub(crate) kind: ItemKind,
}
#[derive(Clone, Debug)]
pub(crate) enum Visibility {
    Inherited,
    Public,
    Restricted { path: Path, shorthand: bool },
}
#[derive(Clone, Debug)]
pub(crate) struct WherePredicate {
    pub(crate) params: Vec<GenericParam>,
    pub(crate) ty: Option<Ty>,
    pub(crate) lifetime: Option<String>,
    pub(crate) bounds: Vec<Bound>,
}
#[derive(Clone, Debug)]
pub(crate) enum ItemKind {
    Enum(Vec<Variant>),
    Struct(Fields),
    Union(Fields),
}
#[derive(Clone, Debug)]
pub(crate) struct Variant {
    pub(crate) visibility: Visibility,
    pub(crate) ident: String,
    pub(crate) fields: Fields,
    pub(crate) discriminant: Option<Expr>,
}
#[derive(Clone, Debug)]
pub(crate) enum Fields {
    Unit,
    Tuple(Vec<Field>),
    Record(Vec<Field>),
}
#[derive(Clone, Debug)]
pub(crate) struct Field {
    pub(crate) visibility: Visibility,
    pub(crate) ident: Option<String>,
    pub(crate) ty: Ty,
}
#[derive(Clone, Debug)]
pub(crate) struct Arm {
    pub(crate) pat: Pat,
    pub(crate) guard: Option<Expr>,
    pub(crate) body: Expr,
}
#[derive(Clone, Debug)]
pub(crate) struct Closure {
    pub(crate) for_params: Vec<GenericParam>,
    pub(crate) constness: bool,
    pub(crate) static_: bool,
    pub(crate) coroutine: Option<&'static str>,
    pub(crate) move_: bool,
    pub(crate) params: Vec<Param>,
    pub(crate) output: Option<Ty>,
    pub(crate) body: Box<Expr>,
}
#[derive(Clone, Debug)]
pub(crate) struct StructExpr {
    pub(crate) qself: Option<QSelf>,
    pub(crate) path: Path,
    pub(crate) fields: Vec<(Option<String>, Expr)>,
    pub(crate) rest: StructRest,
}
#[derive(Clone, Debug)]
pub(crate) enum StructRest {
    None,
    Base(Box<Expr>),
    Rest,
}
#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Array(Vec<Expr>),
    ConstBlock(Block),
    Repeat(Box<Expr>, Box<Expr>),
    Struct(Box<StructExpr>),
    Tup(Vec<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    MethodCall {
        ident: String,
        args: Option<GenericArgs>,
        receiver: Box<Expr>,
        call_args: Vec<Expr>,
    },
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `*`, `!`, `-`.
    Unary(&'static str, Box<Expr>),
    /// `raw`: `&raw const|mut`.
    AddrOf {
        raw: bool,
        mutbl: bool,
        expr: Box<Expr>,
    },
    Lit(String),
    Cast(Box<Expr>, Box<Ty>),
    Let(Box<Pat>, Box<Expr>),
    If(Box<Expr>, Block, Option<Box<Expr>>),
    While(Box<Expr>, Block, Option<String>),
    ForLoop {
        pat: Box<Pat>,
        iter: Box<Expr>,
        body: Block,
        label: Option<String>,
    },
    Loop(Block, Option<String>),
    Match(Box<Expr>, Vec<Arm>),
    Closure(Box<Closure>),
    Block(Block, Option<String>),
    /// `async`/`gen`/`async gen` blocks.
    Gen(&'static str, bool, Block),
    Await(Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    AssignOp(BinOp, Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Range(Option<Box<Expr>>, Option<Box<Expr>>, bool),
    Underscore,
    Path(Option<QSelf>, Path),
    Break(Option<String>, Option<Box<Expr>>),
    Continue(Option<String>),
    Ret(Option<Box<Expr>>),
    Yeet(Option<Box<Expr>>),
    Become(Box<Expr>),
    MacCall(MacCall),
    Paren(Box<Expr>),
    Yield(Option<Box<Expr>>),
    Try(Box<Expr>),
    TryBlock(Block),
}

impl Expr {
    /// rustc `Expr::precedence` (no attributes are admitted here).
    pub(crate) fn precedence(&self) -> Prec {
        match self {
            Expr::Closure(c) => {
                if c.output.is_none() {
                    Prec::Jump
                } else {
                    Prec::Unambiguous
                }
            }
            Expr::Break(_, value) | Expr::Ret(value) | Expr::Yield(value) | Expr::Yeet(value) => {
                if value.is_some() {
                    Prec::Jump
                } else {
                    Prec::Unambiguous
                }
            }
            Expr::Become(_) => Prec::Jump,
            Expr::Range(..) => Prec::Range,
            Expr::Binary(op, ..) => op.precedence(),
            Expr::Cast(..) => Prec::Cast,
            Expr::Assign(..) | Expr::AssignOp(..) => Prec::Assign,
            Expr::AddrOf { .. } => Prec::Prefix,
            Expr::Let(..) | Expr::Unary(..) => Prec::Prefix,
            _ => Prec::Unambiguous,
        }
    }
}

// ----- token alignment ---------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct GroupRef<'w> {
    pub(crate) delimiter: Delimiter,
    pub(crate) open: Key,
    pub(crate) close: Key,
    pub(crate) children: &'w [TokenTree],
}

#[derive(Clone, Copy)]
struct TokInfo<'w> {
    first: Key,
    group: Option<GroupRef<'w>>,
}

struct Entry<'w> {
    text: String,
    key: Key,
    group: Option<GroupRef<'w>>,
}

fn flatten<'w>(trees: &'w [TokenTree], out: &mut Vec<Entry<'w>>) {
    for tree in trees {
        match tree {
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            } => flatten(children, out),
            TokenTree::Group {
                delimiter,
                open,
                close,
                children,
            } => {
                let (o, c) = match delimiter {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Invisible => ("", ""),
                };
                out.push(Entry {
                    text: o.into(),
                    key: *open,
                    group: Some(GroupRef {
                        delimiter: *delimiter,
                        open: *open,
                        close: *close,
                        children,
                    }),
                });
                flatten(children, out);
                out.push(Entry {
                    text: c.into(),
                    key: *close,
                    group: None,
                });
            }
            TokenTree::Ident { text, raw, span } => out.push(Entry {
                text: if *raw {
                    format!("r#{text}")
                } else {
                    text.clone()
                },
                key: *span,
                group: None,
            }),
            TokenTree::Punct { scalar, span, .. } => out.push(Entry {
                text: scalar.to_string(),
                key: *span,
                group: None,
            }),
            TokenTree::Literal { token_text, span } => out.push(Entry {
                text: token_text.clone(),
                key: *span,
                group: None,
            }),
        }
    }
}

fn align<'w>(
    root: &SyntaxNode,
    trees: &'w [TokenTree],
) -> Result<HashMap<TextSize, TokInfo<'w>>, AdapterError> {
    let mut entries = Vec::new();
    flatten(trees, &mut entries);
    let mut map = HashMap::new();
    let (mut index, mut offset) = (0usize, 0usize);
    for element in root.descendants_with_tokens() {
        let NodeOrToken::Token(token) = element else {
            continue;
        };
        if token.kind().is_trivia() {
            continue;
        }
        let mut remaining = token.text();
        let first = entries
            .get(index)
            .ok_or_else(|| fault("syntax token has no source leaf"))?;
        let info = TokInfo {
            first: first.key,
            group: if offset == 0 { first.group } else { None },
        };
        while !remaining.is_empty() {
            let entry = entries
                .get(index)
                .ok_or_else(|| fault("syntax token exceeds source leaves"))?;
            let rest = &entry.text[offset..];
            if let Some(after) = remaining.strip_prefix(rest) {
                remaining = after;
                index += 1;
                offset = 0;
            } else if rest.starts_with(remaining) {
                offset += remaining.len();
                remaining = "";
            } else {
                return Err(fault("syntax token text differs from source leaves"));
            }
        }
        map.insert(token.text_range().start(), info);
    }
    Ok(map)
}

// ----- conversion --------------------------------------------------------

pub(crate) struct Converter<'w, 'i> {
    tokens: HashMap<TextSize, TokInfo<'w>>,
    index: &'i SpanIndex<'i>,
    dollar_crate: DollarCrate<'i>,
}

type Res<T> = Result<T, AdapterError>;

fn kids<N: AstNode>(node: &SyntaxNode) -> impl Iterator<Item = N> + use<N> {
    node.children().filter_map(N::cast)
}
fn token_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .find(|t| t.kind() == kind)
}
fn missing(what: &str) -> AdapterError {
    fault(format!("parsed syntax lacks {what}"))
}

impl<'w, 'i> Converter<'w, 'i> {
    pub(crate) fn new(
        root: &SyntaxNode,
        trees: &'w [TokenTree],
        index: &'i SpanIndex<'i>,
        dollar_crate: DollarCrate<'i>,
    ) -> Res<Self> {
        Ok(Self {
            tokens: align(root, trees)?,
            index,
            dollar_crate,
        })
    }

    fn info(&self, token: &SyntaxToken) -> Res<TokInfo<'w>> {
        self.tokens
            .get(&token.text_range().start())
            .copied()
            .ok_or_else(|| fault("unaligned syntax token"))
    }

    fn edition(&self, token: &SyntaxToken) -> Res<Edition> {
        self.index.edition(self.info(token)?.first)
    }

    /// rustc `print_ident` on an AST identifier (`Ident::guess_print_mode`,
    /// `$crate` converted to its crate name).
    fn ident(&self, token: &SyntaxToken) -> Res<String> {
        let text = token.text();
        let name = text.strip_prefix("r#").unwrap_or(text);
        if name == "$crate" {
            let key = self.info(token)?.first;
            return match self.dollar_crate {
                Some(convert) => Ok(tts::converted_dollar_crate(convert(key)?)),
                None => Ok(name.to_owned()),
            };
        }
        let can_be_raw = name != "_" && !tts::is_path_segment_keyword(name);
        if can_be_raw && tts::is_reserved(name, self.edition(token)?) {
            Ok(format!("r#{name}"))
        } else {
            Ok(name.to_owned())
        }
    }

    fn name_ref(&self, name: &ast::NameRef) -> Res<String> {
        let token = name
            .syntax()
            .first_token()
            .ok_or_else(|| missing("name token"))?;
        self.ident(&token)
    }

    fn name(&self, name: &ast::Name) -> Res<String> {
        let token = name
            .syntax()
            .first_token()
            .ok_or_else(|| missing("name token"))?;
        self.ident(&token)
    }

    fn lifetime(&self, lifetime: &ast::Lifetime) -> Res<String> {
        Ok(lifetime.syntax().text().to_string())
    }

    fn label(&self, label: Option<ast::Label>) -> Res<Option<String>> {
        label
            .map(|label| {
                label
                    .lifetime()
                    .ok_or_else(|| missing("label lifetime"))
                    .and_then(|l| self.lifetime(&l))
            })
            .transpose()
    }

    // --- paths ---

    fn path_segments(&self, path: &ast::Path, out: &mut Vec<ast::PathSegment>) {
        if let Some(qualifier) = path.qualifier() {
            self.path_segments(&qualifier, out);
        }
        if let Some(segment) = path.segment() {
            out.push(segment);
        }
    }

    /// Convert a path, splitting a leading `<T as Trait>` anchor into `QSelf`.
    pub(crate) fn qpath(&self, path: &ast::Path) -> Res<(Option<QSelf>, Path)> {
        let mut raw = Vec::new();
        self.path_segments(path, &mut raw);
        let mut segments = Vec::new();
        let mut qself = None;
        for (i, segment) in raw.iter().enumerate() {
            if i == 0 && segment.coloncolon_token().is_some() {
                segments.push(PathSegment {
                    ident: String::new(),
                    args: None,
                });
            }
            if let Some(ast::PathSegmentKind::Type {
                type_ref,
                trait_ref,
            }) = segment.kind()
            {
                if i != 0 {
                    return Err(unsupported("qualified path anchor after the first segment"));
                }
                let ty = self.ty(&type_ref.ok_or_else(|| missing("qualified self type"))?)?;
                let position = match trait_ref.and_then(|p| p.path()) {
                    Some(trait_path) => {
                        let (inner_qself, trait_path) = self.qpath(&trait_path)?;
                        if inner_qself.is_some() {
                            return Err(unsupported("nested qualified trait path"));
                        }
                        let n = trait_path.segments.len();
                        segments.extend(trait_path.segments);
                        n
                    }
                    None => 0,
                };
                qself = Some(QSelf {
                    ty: Box::new(ty),
                    position,
                });
                continue;
            }
            segments.push(self.segment(segment)?);
        }
        Ok((qself, Path { segments }))
    }

    pub(crate) fn path(&self, path: &ast::Path) -> Res<Path> {
        match self.qpath(path)? {
            (None, path) => Ok(path),
            (Some(_), _) => Err(unsupported("qualified path in an unqualified position")),
        }
    }

    fn segment(&self, segment: &ast::PathSegment) -> Res<PathSegment> {
        let token = segment
            .syntax()
            .children_with_tokens()
            .filter_map(|e| match e {
                NodeOrToken::Node(n) if n.kind() == SyntaxKind::NAME_REF => n.first_token(),
                NodeOrToken::Token(t)
                    if matches!(
                        t.kind(),
                        T![self] | T![super] | T![crate] | T![Self] | SyntaxKind::IDENT
                    ) =>
                {
                    Some(t)
                }
                _ => None,
            })
            .next()
            .ok_or_else(|| missing("path segment name"))?;
        let ident = self.ident(&token)?;
        let args = if let Some(list) = segment.generic_arg_list() {
            Some(self.generic_args(&list)?)
        } else if let Some(list) = segment.parenthesized_arg_list() {
            let mut inputs = Vec::new();
            for arg in list.type_args() {
                inputs.push(self.ty(&arg.ty().ok_or_else(|| missing("parenthesized arg"))?)?);
            }
            let output = segment
                .ret_type()
                .and_then(|r| r.ty())
                .map(|t| self.ty(&t))
                .transpose()?
                .map(Box::new);
            Some(GenericArgs::Paren { inputs, output })
        } else if segment.return_type_syntax().is_some() {
            Some(GenericArgs::ParenElided)
        } else {
            None
        };
        Ok(PathSegment { ident, args })
    }

    fn generic_args(&self, list: &ast::GenericArgList) -> Res<GenericArgs> {
        let mut args = Vec::new();
        for arg in list.generic_args() {
            args.push(match arg {
                ast::GenericArg::LifetimeArg(l) => AngleArg::Lifetime(
                    self.lifetime(&l.lifetime().ok_or_else(|| missing("lifetime arg"))?)?,
                ),
                ast::GenericArg::TypeArg(t) => {
                    AngleArg::Type(self.ty(&t.ty().ok_or_else(|| missing("type arg"))?)?)
                }
                ast::GenericArg::ConstArg(c) => {
                    AngleArg::Const(self.expr(&c.expr().ok_or_else(|| missing("const arg"))?)?)
                }
                ast::GenericArg::AssocTypeArg(a) => {
                    let ident =
                        self.name_ref(&a.name_ref().ok_or_else(|| missing("assoc name"))?)?;
                    let args = a
                        .generic_arg_list()
                        .map(|l| self.generic_args(&l))
                        .transpose()?;
                    let kind = if let Some(ty) = a.ty() {
                        ConstraintKind::EqualityTy(self.ty(&ty)?)
                    } else if let Some(c) = a.const_arg() {
                        ConstraintKind::EqualityConst(
                            self.expr(&c.expr().ok_or_else(|| missing("assoc const"))?)?,
                        )
                    } else if let Some(bounds) = a.type_bound_list() {
                        ConstraintKind::Bound(self.bounds(&bounds)?)
                    } else {
                        ConstraintKind::Bound(Vec::new())
                    };
                    AngleArg::Constraint { ident, args, kind }
                }
            });
        }
        Ok(GenericArgs::Angle(args))
    }

    // --- types ---

    fn bounds(&self, list: &ast::TypeBoundList) -> Res<Vec<Bound>> {
        let mut bounds = Vec::new();
        for bound in list.bounds() {
            bounds.push(self.bound(&bound)?);
        }
        Ok(bounds)
    }

    fn bound(&self, bound: &ast::TypeBound) -> Res<Bound> {
        if let Some(lifetime) = bound.lifetime() {
            return Ok(Bound::Outlives(self.lifetime(&lifetime)?));
        }
        if let Some(args) = bound.use_bound_generic_args() {
            let mut out = Vec::new();
            for arg in args.use_bound_generic_args() {
                out.push(match arg {
                    ast::UseBoundGenericArg::Lifetime(l) => UseArg::Lifetime(self.lifetime(&l)?),
                    ast::UseBoundGenericArg::NameRef(n) => UseArg::Path(Path {
                        segments: vec![PathSegment {
                            ident: self.name_ref(&n)?,
                            args: None,
                        }],
                    }),
                });
            }
            return Ok(Bound::Use(out));
        }
        let syntax = bound.syntax();
        let parens = token_of(syntax, T!['(']).is_some();
        let for_params = match bound.for_binder().and_then(|b| b.generic_param_list()) {
            Some(list) => self.generic_params(&list)?,
            None => Vec::new(),
        };
        let has = |kind| {
            syntax
                .descendants_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .take_while(|t| t.kind() != SyntaxKind::IDENT)
                .any(|t| t.kind() == kind)
        };
        let constness = if token_of(syntax, T![~]).is_some() || has(T![~]) {
            Some("~const")
        } else if has(T![const]) {
            Some("const")
        } else {
            None
        };
        let asyncness = has(T![async]);
        let polarity = if has(T![?]) {
            Some("?")
        } else if has(T![!]) {
            Some("!")
        } else {
            None
        };
        let ty = bound.ty().ok_or_else(|| missing("trait bound type"))?;
        let ast::Type::PathType(path_type) = ty else {
            return Err(unsupported("non-path trait bound"));
        };
        let path = self.path(&path_type.path().ok_or_else(|| missing("bound path"))?)?;
        Ok(Bound::Trait {
            parens,
            for_params,
            constness,
            asyncness,
            polarity,
            path,
        })
    }

    fn generic_params(&self, list: &ast::GenericParamList) -> Res<Vec<GenericParam>> {
        let mut out = Vec::new();
        for param in list.generic_params() {
            out.push(match param {
                ast::GenericParam::LifetimeParam(l) => {
                    let name = self.lifetime(&l.lifetime().ok_or_else(|| missing("lifetime"))?)?;
                    let mut bounds = Vec::new();
                    if let Some(list) = l.type_bound_list() {
                        for bound in list.bounds() {
                            bounds.push(self.lifetime(
                                &bound.lifetime().ok_or_else(|| missing("lifetime bound"))?,
                            )?);
                        }
                    }
                    GenericParam::Lifetime { name, bounds }
                }
                ast::GenericParam::TypeParam(t) => GenericParam::Type {
                    ident: self.name(&t.name().ok_or_else(|| missing("type param"))?)?,
                    bounds: match t.type_bound_list() {
                        Some(list) => self.bounds(&list)?,
                        None => Vec::new(),
                    },
                    default: t.default_type().map(|d| self.ty(&d)).transpose()?,
                },
                ast::GenericParam::ConstParam(c) => GenericParam::Const {
                    ident: self.name(&c.name().ok_or_else(|| missing("const param"))?)?,
                    ty: self.ty(&c.ty().ok_or_else(|| missing("const param type"))?)?,
                    default: c
                        .default_val()
                        .and_then(|d| d.expr())
                        .map(|e| self.expr(&e))
                        .transpose()?,
                },
            });
        }
        Ok(out)
    }

    pub(crate) fn ty(&self, ty: &ast::Type) -> Res<Ty> {
        Ok(match ty {
            ast::Type::SliceType(t) => Ty::Slice(Box::new(
                self.ty(&t.ty().ok_or_else(|| missing("slice type"))?)?,
            )),
            ast::Type::ArrayType(t) => {
                let len = t
                    .const_arg()
                    .and_then(|c| c.expr())
                    .ok_or_else(|| missing("array length"))?;
                Ty::Array(
                    Box::new(self.ty(&t.ty().ok_or_else(|| missing("array type"))?)?),
                    Box::new(self.expr(&len)?),
                )
            }
            ast::Type::PtrType(t) => Ty::Ptr(
                t.mut_token().is_some(),
                Box::new(self.ty(&t.ty().ok_or_else(|| missing("pointer type"))?)?),
            ),
            ast::Type::RefType(t) => Ty::Ref(
                t.lifetime().map(|l| self.lifetime(&l)).transpose()?,
                t.mut_token().is_some(),
                Box::new(self.ty(&t.ty().ok_or_else(|| missing("reference type"))?)?),
            ),
            ast::Type::NeverType(_) => Ty::Never,
            ast::Type::TupleType(t) => {
                let mut fields = Vec::new();
                for field in t.fields() {
                    fields.push(self.ty(&field)?);
                }
                Ty::Tup(fields)
            }
            ast::Type::ParenType(t) => Ty::Paren(Box::new(
                self.ty(&t.ty().ok_or_else(|| missing("paren type"))?)?,
            )),
            ast::Type::PathType(t) => {
                let (qself, path) = self.qpath(&t.path().ok_or_else(|| missing("type path"))?)?;
                Ty::Path(qself, path)
            }
            ast::Type::DynTraitType(t) => Ty::TraitObject(
                t.dyn_token().is_some(),
                self.bounds(&t.type_bound_list().ok_or_else(|| missing("dyn bounds"))?)?,
            ),
            ast::Type::ImplTraitType(t) => Ty::ImplTrait(
                self.bounds(&t.type_bound_list().ok_or_else(|| missing("impl bounds"))?)?,
            ),
            ast::Type::InferType(_) => Ty::Infer,
            ast::Type::MacroType(t) => Ty::MacCall(
                self.mac_call(&t.macro_call().ok_or_else(|| missing("type macro call"))?)?,
            ),
            ast::Type::FnPtrType(t) => self.fn_ptr(t, Vec::new())?,
            ast::Type::ForType(t) => {
                let params = match t.for_binder().and_then(|b| b.generic_param_list()) {
                    Some(list) => self.generic_params(&list)?,
                    None => Vec::new(),
                };
                match t.ty() {
                    Some(ast::Type::FnPtrType(f)) => self.fn_ptr(&f, params)?,
                    _ => return Err(unsupported("higher-ranked non-function type")),
                }
            }
            ast::Type::PatternType(_) => return Err(unsupported("pattern type")),
        })
    }

    fn fn_ptr(&self, t: &ast::FnPtrType, generic_params: Vec<GenericParam>) -> Res<Ty> {
        let abi = t.abi().map(|abi| {
            abi.syntax()
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::STRING)
                .map(|t| t.text().to_owned())
        });
        let mut params = Vec::new();
        if let Some(list) = t.param_list() {
            if list.self_param().is_some() {
                return Err(unsupported("self parameter in a function pointer type"));
            }
            for param in list.params() {
                params.push(Param {
                    pat: param.pat().map(|p| self.pat(&p)).transpose()?,
                    ty: param.ty().map(|t| self.ty(&t)).transpose()?,
                });
            }
        }
        Ok(Ty::FnPtr {
            generic_params,
            unsafety: t.unsafe_token().is_some(),
            abi,
            params,
            output: t
                .ret_type()
                .and_then(|r| r.ty())
                .map(|t| self.ty(&t))
                .transpose()?
                .map(Box::new),
        })
    }

    // --- macro calls ---

    pub(crate) fn mac_call(&self, call: &ast::MacroCall) -> Res<MacCall> {
        let path = self.path(&call.path().ok_or_else(|| missing("macro path"))?)?;
        let tree = call
            .token_tree()
            .ok_or_else(|| missing("macro token tree"))?;
        let open = tree
            .syntax()
            .first_token()
            .ok_or_else(|| missing("macro delimiter"))?;
        let group = self
            .info(&open)?
            .group
            .ok_or_else(|| fault("macro delimiter is not aligned with a source group"))?;
        let delim = match group.delimiter {
            Delimiter::Parenthesis => Delim::Parenthesis,
            Delimiter::Bracket => Delim::Bracket,
            Delimiter::Brace => Delim::Brace,
            Delimiter::Invisible => Delim::Invisible,
        };
        let tts = tts::build(group.children, self.index, self.dollar_crate)?;
        Ok(MacCall { path, delim, tts })
    }

    // --- patterns ---

    fn pat_expr(&self, pat: &ast::Pat) -> Res<Expr> {
        Ok(match pat {
            ast::Pat::LiteralPat(l) => {
                let lit = Expr::Lit(
                    l.literal()
                        .ok_or_else(|| missing("literal"))?
                        .token()
                        .text()
                        .to_owned(),
                );
                if l.minus_token().is_some() {
                    Expr::Unary("-", Box::new(lit))
                } else {
                    lit
                }
            }
            ast::Pat::PathPat(p) => {
                let (qself, path) = self.qpath(&p.path().ok_or_else(|| missing("path"))?)?;
                Expr::Path(qself, path)
            }
            ast::Pat::IdentPat(p)
                if p.ref_token().is_none() && p.mut_token().is_none() && p.pat().is_none() =>
            {
                Expr::Path(
                    None,
                    Path {
                        segments: vec![PathSegment {
                            ident: self.name(&p.name().ok_or_else(|| missing("name"))?)?,
                            args: None,
                        }],
                    },
                )
            }
            ast::Pat::ConstBlockPat(c) => Expr::ConstBlock(
                self.block(&c.block_expr().ok_or_else(|| missing("const block"))?)?,
            ),
            _ => return Err(unsupported("range pattern bound")),
        })
    }

    pub(crate) fn pat(&self, pat: &ast::Pat) -> Res<Pat> {
        Ok(match pat {
            ast::Pat::WildcardPat(_) => Pat::Wild,
            ast::Pat::RestPat(_) => Pat::Rest,
            ast::Pat::IdentPat(p) => Pat::Ident {
                by_ref: p.ref_token().is_some(),
                mutbl: p.mut_token().is_some(),
                ident: self.name(&p.name().ok_or_else(|| missing("binding name"))?)?,
                sub: p.pat().map(|s| self.pat(&s)).transpose()?.map(Box::new),
            },
            ast::Pat::LiteralPat(_) | ast::Pat::ConstBlockPat(_) => {
                Pat::Expr(Box::new(self.pat_expr(pat)?))
            }
            ast::Pat::PathPat(p) => {
                let (qself, path) = self.qpath(&p.path().ok_or_else(|| missing("path"))?)?;
                Pat::Path(qself, path)
            }
            ast::Pat::TupleStructPat(p) => {
                let (qself, path) = self.qpath(&p.path().ok_or_else(|| missing("path"))?)?;
                let mut fields = Vec::new();
                for field in p.fields() {
                    fields.push(self.pat(&field)?);
                }
                Pat::TupleStruct(qself, path, fields)
            }
            ast::Pat::RecordPat(p) => {
                let (qself, path) = self.qpath(&p.path().ok_or_else(|| missing("path"))?)?;
                let list = p
                    .record_pat_field_list()
                    .ok_or_else(|| missing("record pattern fields"))?;
                let mut fields = Vec::new();
                for field in list.fields() {
                    let ident = field.name_ref().map(|n| self.name_ref(&n)).transpose()?;
                    let pat = self.pat(&field.pat().ok_or_else(|| missing("field pattern"))?)?;
                    fields.push(PatField { ident, pat });
                }
                Pat::Struct(qself, path, fields, list.rest_pat().is_some())
            }
            ast::Pat::TuplePat(p) => {
                let mut fields = Vec::new();
                for field in p.fields() {
                    fields.push(self.pat(&field)?);
                }
                Pat::Tuple(fields)
            }
            ast::Pat::SlicePat(p) => {
                let mut pats = Vec::new();
                for field in p.pats() {
                    pats.push(self.pat(&field)?);
                }
                Pat::Slice(pats)
            }
            ast::Pat::RefPat(p) => Pat::Ref(
                Box::new(self.pat(&p.pat().ok_or_else(|| missing("referenced pattern"))?)?),
                p.mut_token().is_some(),
            ),
            ast::Pat::OrPat(p) => {
                let mut pats = Vec::new();
                for alt in p.pats() {
                    pats.push(self.pat(&alt)?);
                }
                Pat::Or(pats)
            }
            ast::Pat::ParenPat(p) => Pat::Paren(Box::new(
                self.pat(&p.pat().ok_or_else(|| missing("pattern"))?)?,
            )),
            ast::Pat::MacroPat(p) => Pat::MacCall(
                self.mac_call(&p.macro_call().ok_or_else(|| missing("pattern macro"))?)?,
            ),
            ast::Pat::RangePat(p) => {
                let op = p
                    .syntax()
                    .children_with_tokens()
                    .filter_map(NodeOrToken::into_token)
                    .find_map(|t| match t.kind() {
                        T![..] => Some(".."),
                        T![..=] => Some("..="),
                        T![...] => Some("..."),
                        _ => None,
                    })
                    .ok_or_else(|| missing("range operator"))?;
                Pat::Range(
                    p.start()
                        .map(|s| self.pat_expr(&s))
                        .transpose()?
                        .map(Box::new),
                    p.end()
                        .map(|e| self.pat_expr(&e))
                        .transpose()?
                        .map(Box::new),
                    op,
                )
            }
            ast::Pat::DerefPat(p) => Pat::Deref(Box::new(
                self.pat(&p.pat().ok_or_else(|| missing("pattern"))?)?,
            )),
            ast::Pat::BoxPat(_) => return Err(unsupported("box pattern")),
            ast::Pat::NotNull(_) => return Err(unsupported("`!null` pattern")),
            ast::Pat::CfgPredPat(_) => return Err(unsupported("cfg predicate pattern")),
        })
    }

    // --- declaration statements (rustc_ast_pretty::state::item) ---

    fn item_attrs(&self, node: &SyntaxNode) -> Res<()> {
        if node
            .children()
            .any(|child| child.kind() == SyntaxKind::ATTR)
        {
            return Err(unsupported("attributes on a declaration statement"));
        }
        Ok(())
    }

    fn visibility(&self, visibility: Option<ast::Visibility>) -> Res<Visibility> {
        let Some(visibility) = visibility else {
            return Ok(Visibility::Inherited);
        };
        match visibility.visibility_inner() {
            None => Ok(Visibility::Public),
            Some(inner) => Ok(Visibility::Restricted {
                path: self.path(&inner.path().ok_or_else(|| missing("visibility path"))?)?,
                shorthand: inner.in_token().is_none(),
            }),
        }
    }

    fn fields(&self, list: Option<ast::FieldList>) -> Res<Fields> {
        Ok(match list {
            None => Fields::Unit,
            Some(ast::FieldList::TupleFieldList(list)) => {
                let mut fields = Vec::new();
                for field in list.fields() {
                    self.item_attrs(field.syntax())?;
                    if field.mut_restriction().is_some() {
                        return Err(unsupported("field mutation restriction"));
                    }
                    fields.push(Field {
                        visibility: self.visibility(field.visibility())?,
                        ident: None,
                        ty: self.ty(&field.ty().ok_or_else(|| missing("field type"))?)?,
                    });
                }
                Fields::Tuple(fields)
            }
            Some(ast::FieldList::RecordFieldList(list)) => {
                let mut fields = Vec::new();
                for field in list.fields() {
                    self.item_attrs(field.syntax())?;
                    if field.mut_restriction().is_some()
                        || field.unsafe_token().is_some()
                        || field.default_val().is_some()
                    {
                        return Err(unsupported("restricted or defaulted field"));
                    }
                    fields.push(Field {
                        visibility: self.visibility(field.visibility())?,
                        ident: Some(
                            self.name(&field.name().ok_or_else(|| missing("field name"))?)?,
                        ),
                        ty: self.ty(&field.ty().ok_or_else(|| missing("field type"))?)?,
                    });
                }
                Fields::Record(fields)
            }
        })
    }

    fn item(&self, item: &ast::Item) -> Res<Item> {
        self.item_attrs(item.syntax())?;
        let (visibility, ident, generics, predicates, kind) = match item {
            ast::Item::Enum(item) => {
                let mut variants = Vec::new();
                for variant in item
                    .variant_list()
                    .ok_or_else(|| missing("enum variants"))?
                    .variants()
                {
                    self.item_attrs(variant.syntax())?;
                    variants.push(Variant {
                        visibility: self.visibility(variant.visibility())?,
                        ident: self
                            .name(&variant.name().ok_or_else(|| missing("variant name"))?)?,
                        fields: self.fields(variant.field_list())?,
                        discriminant: variant
                            .const_arg()
                            .and_then(|arg| arg.expr())
                            .map(|expr| self.expr(&expr))
                            .transpose()?,
                    });
                }
                (
                    item.visibility(),
                    item.name(),
                    item.generic_param_list(),
                    item.where_clause(),
                    ItemKind::Enum(variants),
                )
            }
            ast::Item::Struct(item) => (
                item.visibility(),
                item.name(),
                item.generic_param_list(),
                item.where_clause(),
                ItemKind::Struct(self.fields(item.field_list())?),
            ),
            ast::Item::Union(item) => (
                item.visibility(),
                item.name(),
                item.generic_param_list(),
                item.where_clause(),
                ItemKind::Union(
                    self.fields(
                        item.record_field_list()
                            .map(ast::FieldList::RecordFieldList),
                    )?,
                ),
            ),
            _ => return Err(unsupported("non-nominal declaration statement")),
        };
        let mut where_predicates = Vec::new();
        if let Some(predicates) = predicates {
            for predicate in predicates.predicates() {
                where_predicates.push(WherePredicate {
                    params: predicate
                        .for_binder()
                        .and_then(|binder| binder.generic_param_list())
                        .map(|params| self.generic_params(&params))
                        .transpose()?
                        .unwrap_or_default(),
                    ty: predicate.ty().map(|ty| self.ty(&ty)).transpose()?,
                    lifetime: predicate
                        .lifetime()
                        .map(|lifetime| self.lifetime(&lifetime))
                        .transpose()?,
                    bounds: predicate
                        .type_bound_list()
                        .map(|bounds| self.bounds(&bounds))
                        .transpose()?
                        .unwrap_or_default(),
                });
            }
        }
        Ok(Item {
            visibility: self.visibility(visibility)?,
            ident: self.name(&ident.ok_or_else(|| missing("item name"))?)?,
            generics: generics
                .map(|params| self.generic_params(&params))
                .transpose()?
                .unwrap_or_default(),
            predicates: where_predicates,
            kind,
        })
    }

    // --- blocks ---

    pub(crate) fn block(&self, block: &ast::BlockExpr) -> Res<Block> {
        let list = block
            .stmt_list()
            .ok_or_else(|| missing("block statements"))?;
        if list
            .syntax()
            .children()
            .any(|n| n.kind() == SyntaxKind::ATTR)
        {
            return Err(unsupported("inner attributes in a block"));
        }
        let mut stmts = Vec::new();
        for stmt in list.statements() {
            stmts.push(self.stmt(&stmt)?);
        }
        if let Some(tail) = list.tail_expr() {
            stmts.push(match tail {
                ast::Expr::MacroExpr(m) => Stmt::MacCall(
                    self.mac_call(&m.macro_call().ok_or_else(|| missing("macro call"))?)?,
                    false,
                ),
                tail => Stmt::Expr(self.expr(&tail)?),
            });
        }
        Ok(Block {
            unsafe_: block.unsafe_token().is_some(),
            stmts,
        })
    }

    fn stmt(&self, stmt: &ast::Stmt) -> Res<Stmt> {
        Ok(match stmt {
            ast::Stmt::LetStmt(l) => {
                if l.syntax().children().any(|n| n.kind() == SyntaxKind::ATTR) {
                    return Err(unsupported("attributes on a let statement"));
                }
                Stmt::Let {
                    super_: l.super_token().is_some(),
                    pat: self.pat(&l.pat().ok_or_else(|| missing("let pattern"))?)?,
                    ty: l.ty().map(|t| self.ty(&t)).transpose()?,
                    init: l.initializer().map(|e| self.expr(&e)).transpose()?,
                    els: l
                        .let_else()
                        .and_then(|e| e.block_expr())
                        .map(|b| self.block(&b))
                        .transpose()?,
                }
            }
            ast::Stmt::ExprStmt(e) => {
                let semi = e.semicolon_token().is_some();
                match e.expr() {
                    Some(ast::Expr::MacroExpr(m)) => Stmt::MacCall(
                        self.mac_call(&m.macro_call().ok_or_else(|| missing("macro call"))?)?,
                        semi,
                    ),
                    Some(expr) => {
                        let expr = self.expr(&expr)?;
                        if semi {
                            Stmt::Semi(expr)
                        } else {
                            Stmt::Expr(expr)
                        }
                    }
                    None if semi => Stmt::Empty,
                    None => return Err(missing("statement expression")),
                }
            }
            ast::Stmt::Item(ast::Item::MacroCall(m)) => {
                let semi = m.semicolon_token().is_some();
                Stmt::MacCall(self.mac_call(m)?, semi)
            }
            ast::Stmt::Item(item) => Stmt::Item(self.item(item)?),
        })
    }

    // --- expressions ---

    fn exprs(&self, list: impl Iterator<Item = ast::Expr>) -> Res<Vec<Expr>> {
        let mut out = Vec::new();
        for expr in list {
            out.push(self.expr(&expr)?);
        }
        Ok(out)
    }

    fn boxed(&self, expr: Option<ast::Expr>, what: &str) -> Res<Box<Expr>> {
        Ok(Box::new(self.expr(&expr.ok_or_else(|| missing(what))?)?))
    }

    fn opt_boxed(&self, expr: Option<ast::Expr>) -> Res<Option<Box<Expr>>> {
        expr.map(|e| self.expr(&e).map(Box::new)).transpose()
    }

    pub(crate) fn expr(&self, expr: &ast::Expr) -> Res<Expr> {
        if expr
            .syntax()
            .children()
            .any(|n| n.kind() == SyntaxKind::ATTR)
        {
            return Err(unsupported("expression attributes"));
        }
        Ok(match expr {
            ast::Expr::ArrayExpr(a) => {
                if a.semicolon_token().is_some() {
                    let mut items = kids::<ast::Expr>(a.syntax());
                    let element = items.next().ok_or_else(|| missing("repeat element"))?;
                    let count = items.next().ok_or_else(|| missing("repeat count"))?;
                    Expr::Repeat(Box::new(self.expr(&element)?), Box::new(self.expr(&count)?))
                } else {
                    Expr::Array(self.exprs(kids::<ast::Expr>(a.syntax()))?)
                }
            }
            ast::Expr::AwaitExpr(a) => Expr::Await(self.boxed(a.expr(), "await operand")?),
            ast::Expr::BecomeExpr(b) => Expr::Become(self.boxed(b.expr(), "become operand")?),
            ast::Expr::BinExpr(b) => {
                let lhs = self.boxed(b.lhs(), "binary lhs")?;
                let rhs = self.boxed(b.rhs(), "binary rhs")?;
                let op = b.op_kind().ok_or_else(|| missing("binary operator"))?;
                use syntax::ast::{ArithOp, BinaryOp, CmpOp, LogicOp, Ordering};
                let arith = |op: ArithOp| match op {
                    ArithOp::Add => BinOp::Add,
                    ArithOp::Mul => BinOp::Mul,
                    ArithOp::Sub => BinOp::Sub,
                    ArithOp::Div => BinOp::Div,
                    ArithOp::Rem => BinOp::Rem,
                    ArithOp::Shl => BinOp::Shl,
                    ArithOp::Shr => BinOp::Shr,
                    ArithOp::BitXor => BinOp::BitXor,
                    ArithOp::BitOr => BinOp::BitOr,
                    ArithOp::BitAnd => BinOp::BitAnd,
                };
                match op {
                    BinaryOp::LogicOp(LogicOp::And) => Expr::Binary(BinOp::And, lhs, rhs),
                    BinaryOp::LogicOp(LogicOp::Or) => Expr::Binary(BinOp::Or, lhs, rhs),
                    BinaryOp::ArithOp(op) => Expr::Binary(arith(op), lhs, rhs),
                    BinaryOp::CmpOp(op) => Expr::Binary(
                        match op {
                            CmpOp::Eq { negated: false } => BinOp::Eq,
                            CmpOp::Eq { negated: true } => BinOp::Ne,
                            CmpOp::Ord {
                                ordering: Ordering::Less,
                                strict: true,
                            } => BinOp::Lt,
                            CmpOp::Ord {
                                ordering: Ordering::Less,
                                strict: false,
                            } => BinOp::Le,
                            CmpOp::Ord {
                                ordering: Ordering::Greater,
                                strict: true,
                            } => BinOp::Gt,
                            CmpOp::Ord {
                                ordering: Ordering::Greater,
                                strict: false,
                            } => BinOp::Ge,
                        },
                        lhs,
                        rhs,
                    ),
                    BinaryOp::Assignment { op: None } => Expr::Assign(lhs, rhs),
                    BinaryOp::Assignment { op: Some(op) } => Expr::AssignOp(arith(op), lhs, rhs),
                }
            }
            ast::Expr::BlockExpr(b) => self.block_expr(b)?,
            ast::Expr::BreakExpr(b) => Expr::Break(
                b.lifetime().map(|l| self.lifetime(&l)).transpose()?,
                self.opt_boxed(b.expr())?,
            ),
            ast::Expr::CallExpr(c) => {
                let args = c.arg_list().ok_or_else(|| missing("call arguments"))?;
                Expr::Call(self.boxed(c.expr(), "callee")?, self.exprs(args.args())?)
            }
            ast::Expr::CastExpr(c) => Expr::Cast(
                self.boxed(c.expr(), "cast operand")?,
                Box::new(self.ty(&c.ty().ok_or_else(|| missing("cast type"))?)?),
            ),
            ast::Expr::ClosureExpr(c) => {
                let for_params = match c.for_binder().and_then(|b| b.generic_param_list()) {
                    Some(list) => self.generic_params(&list)?,
                    None => Vec::new(),
                };
                let mut params = Vec::new();
                if let Some(list) = c.param_list() {
                    for param in list.params() {
                        params.push(Param {
                            pat: param.pat().map(|p| self.pat(&p)).transpose()?,
                            ty: param.ty().map(|t| self.ty(&t)).transpose()?,
                        });
                    }
                }
                let coroutine = match (c.async_token().is_some(), c.gen_token().is_some()) {
                    (true, true) => Some("async gen"),
                    (true, false) => Some("async"),
                    (false, true) => Some("gen"),
                    (false, false) => None,
                };
                Expr::Closure(Box::new(Closure {
                    for_params,
                    constness: c.const_token().is_some(),
                    static_: c.static_token().is_some(),
                    coroutine,
                    move_: c.move_token().is_some(),
                    params,
                    output: c
                        .ret_type()
                        .and_then(|r| r.ty())
                        .map(|t| self.ty(&t))
                        .transpose()?,
                    body: self.boxed(c.body(), "closure body")?,
                }))
            }
            ast::Expr::ContinueExpr(c) => {
                Expr::Continue(c.lifetime().map(|l| self.lifetime(&l)).transpose()?)
            }
            ast::Expr::FieldExpr(f) => {
                let name = f.name_ref().ok_or_else(|| missing("field name"))?;
                let token = name
                    .syntax()
                    .first_token()
                    .ok_or_else(|| missing("field token"))?;
                let field = if token.kind() == SyntaxKind::INT_NUMBER {
                    token.text().to_owned()
                } else {
                    self.ident(&token)?
                };
                Expr::Field(self.boxed(f.expr(), "field base")?, field)
            }
            ast::Expr::ForExpr(f) => Expr::ForLoop {
                pat: Box::new(self.pat(&f.pat().ok_or_else(|| missing("for pattern"))?)?),
                iter: self.boxed(f.iterable(), "for iterable")?,
                body: self.block(&f.loop_body().ok_or_else(|| missing("for body"))?)?,
                label: self.label(f.label())?,
            },
            ast::Expr::IfExpr(i) => self.if_expr(i)?,
            ast::Expr::IndexExpr(i) => Expr::Index(
                self.boxed(i.base(), "index base")?,
                self.boxed(i.index(), "index")?,
            ),
            ast::Expr::LetExpr(l) => Expr::Let(
                Box::new(self.pat(&l.pat().ok_or_else(|| missing("let pattern"))?)?),
                self.boxed(l.expr(), "let scrutinee")?,
            ),
            ast::Expr::Literal(l) => Expr::Lit(l.token().text().to_owned()),
            ast::Expr::LoopExpr(l) => Expr::Loop(
                self.block(&l.loop_body().ok_or_else(|| missing("loop body"))?)?,
                self.label(l.label())?,
            ),
            ast::Expr::MacroExpr(m) => {
                Expr::MacCall(self.mac_call(&m.macro_call().ok_or_else(|| missing("macro call"))?)?)
            }
            ast::Expr::MatchExpr(m) => {
                let scrutinee = self.boxed(m.expr(), "match scrutinee")?;
                let mut arms = Vec::new();
                if let Some(list) = m.match_arm_list() {
                    for arm in list.arms() {
                        if arm
                            .syntax()
                            .children()
                            .any(|n| n.kind() == SyntaxKind::ATTR)
                        {
                            return Err(unsupported("match arm attributes"));
                        }
                        arms.push(Arm {
                            pat: self.pat(&arm.pat().ok_or_else(|| missing("arm pattern"))?)?,
                            guard: arm
                                .guard()
                                .and_then(|g| g.syntax().children().find_map(ast::Expr::cast))
                                .map(|e| self.expr(&e))
                                .transpose()?,
                            body: self.expr(&arm.expr().ok_or_else(|| missing("arm body"))?)?,
                        });
                    }
                }
                Expr::Match(scrutinee, arms)
            }
            ast::Expr::MethodCallExpr(m) => {
                let args = m.arg_list().ok_or_else(|| missing("method arguments"))?;
                Expr::MethodCall {
                    ident: self.name_ref(&m.name_ref().ok_or_else(|| missing("method name"))?)?,
                    args: m
                        .generic_arg_list()
                        .map(|l| self.generic_args(&l))
                        .transpose()?,
                    receiver: self.boxed(m.receiver(), "method receiver")?,
                    call_args: self.exprs(args.args())?,
                }
            }
            ast::Expr::ParenExpr(p) => {
                let open = p.l_paren_token().ok_or_else(|| missing("paren"))?;
                let inner = self.expr(&p.expr().ok_or_else(|| missing("paren body"))?)?;
                match self.info(&open)?.group {
                    // Rust-analyzer wraps substituted `$e:expr` fragments in a
                    // parenthesis group whose delimiters share one span; rustc
                    // keeps the fragment as the expression itself.
                    Some(group) if group.open == group.close => inner,
                    _ => Expr::Paren(Box::new(inner)),
                }
            }
            ast::Expr::PathExpr(p) => {
                let (qself, path) = self.qpath(&p.path().ok_or_else(|| missing("path"))?)?;
                Expr::Path(qself, path)
            }
            ast::Expr::PrefixExpr(p) => {
                let op = match p.op_kind().ok_or_else(|| missing("prefix operator"))? {
                    ast::UnaryOp::Deref => "*",
                    ast::UnaryOp::Not => "!",
                    ast::UnaryOp::Neg => "-",
                };
                Expr::Unary(op, self.boxed(p.expr(), "prefix operand")?)
            }
            ast::Expr::RangeExpr(r) => Expr::Range(
                self.opt_boxed(r.start())?,
                self.opt_boxed(r.end())?,
                r.op_kind() == Some(ast::RangeOp::Inclusive),
            ),
            ast::Expr::RecordExpr(r) => {
                let (qself, path) = self.qpath(&r.path().ok_or_else(|| missing("struct path"))?)?;
                let list = r
                    .record_expr_field_list()
                    .ok_or_else(|| missing("struct fields"))?;
                let mut fields = Vec::new();
                for field in list.fields() {
                    if field
                        .syntax()
                        .children()
                        .any(|n| n.kind() == SyntaxKind::ATTR)
                    {
                        return Err(unsupported("struct field attributes"));
                    }
                    let expr = self.expr(&field.expr().ok_or_else(|| missing("field value"))?)?;
                    let ident = match field.name_ref() {
                        Some(name) if field.colon_token().is_some() => Some(self.name_ref(&name)?),
                        _ => None,
                    };
                    fields.push((ident, expr));
                }
                let rest = match (list.dotdot_token(), list.spread()) {
                    (_, Some(base)) => StructRest::Base(Box::new(self.expr(&base)?)),
                    (Some(_), None) => StructRest::Rest,
                    (None, None) => StructRest::None,
                };
                Expr::Struct(Box::new(StructExpr {
                    qself,
                    path,
                    fields,
                    rest,
                }))
            }
            ast::Expr::RefExpr(r) => Expr::AddrOf {
                raw: r.raw_token().is_some(),
                mutbl: r.mut_token().is_some(),
                expr: self.boxed(r.expr(), "reference operand")?,
            },
            ast::Expr::ReturnExpr(r) => Expr::Ret(self.opt_boxed(r.expr())?),
            ast::Expr::TryExpr(t) => Expr::Try(self.boxed(t.expr(), "try operand")?),
            ast::Expr::TupleExpr(t) => Expr::Tup(self.exprs(t.fields())?),
            ast::Expr::UnderscoreExpr(_) => Expr::Underscore,
            ast::Expr::WhileExpr(w) => Expr::While(
                self.boxed(w.condition(), "while condition")?,
                self.block(&w.loop_body().ok_or_else(|| missing("while body"))?)?,
                self.label(w.label())?,
            ),
            ast::Expr::YeetExpr(y) => Expr::Yeet(self.opt_boxed(y.expr())?),
            ast::Expr::YieldExpr(y) => Expr::Yield(self.opt_boxed(y.expr())?),
            ast::Expr::AsmExpr(_)
            | ast::Expr::CfgPredExpr(_)
            | ast::Expr::FormatArgsExpr(_)
            | ast::Expr::IncludeBytesExpr(_)
            | ast::Expr::OffsetOfExpr(_) => {
                return Err(unsupported("`builtin #` expression syntax"));
            }
        })
    }

    fn block_expr(&self, b: &ast::BlockExpr) -> Res<Expr> {
        let label = self.label(b.label())?;
        let block = self.block(b)?;
        let is_async = b.async_token().is_some();
        let is_gen = b.gen_token().is_some();
        if is_async || is_gen {
            let kind = match (is_async, is_gen) {
                (true, true) => "async gen",
                (true, false) => "async",
                _ => "gen",
            };
            return Ok(Expr::Gen(kind, b.move_token().is_some(), block));
        }
        if b.try_block_modifier().is_some() {
            return Ok(Expr::TryBlock(block));
        }
        if b.const_token().is_some() {
            return Ok(Expr::ConstBlock(block));
        }
        Ok(Expr::Block(block, label))
    }

    fn if_expr(&self, i: &ast::IfExpr) -> Res<Expr> {
        let cond = self.boxed(i.condition(), "if condition")?;
        let then = self.block(&i.then_branch().ok_or_else(|| missing("if body"))?)?;
        let els = match i.else_branch() {
            None => None,
            Some(ast::ElseBranch::Block(b)) => Some(Box::new(Expr::Block(self.block(&b)?, None))),
            Some(ast::ElseBranch::IfExpr(e)) => Some(Box::new(self.if_expr(&e)?)),
        };
        Ok(Expr::If(cond, then, els))
    }
}
