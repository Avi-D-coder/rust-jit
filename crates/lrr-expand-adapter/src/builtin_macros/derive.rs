//! rustc builtin derive expanders (`rustc_builtin_macros/src/deriving/`):
//! `Clone`, `Copy`, `Debug`, `Default`, `Hash`, `PartialEq`, `Eq`,
//! `PartialOrd` and `Ord`.
//!
//! The input is the derive's item (cfg-stripped, preceding derive groups
//! removed) exactly as a procedural derive receives it; the output is the
//! ordinary item token output the caller appends after that item. Nothing is
//! evaluated here: the impls are parsed, indexed, resolved and executed by the
//! ordinary pipeline.
//!
//! Spans follow `deriving/generic/mod.rs`: library paths (`$crate::clone::Clone`),
//! keywords, locals (`__self_0`, `other`, `f`, `state`, `__H`, `cmp`) and
//! method names use the def-site span (`cx.with_def_site_ctxt(span)`); the
//! user's type, variant, field and generic parameter identifiers, bounds,
//! where predicates, field types and discriminant expressions keep their own
//! span records.
//!
//! # Differences from rustc's token output (runtime-equivalent)
//!
//! - Enum discriminants are computed by an ordinary `match` over the variants
//!   (explicit discriminant expressions, implicit ones as the previous value
//!   plus one, typed by the `#[repr]` integer or `isize`) instead of
//!   `::core::intrinsics::discriminant_value`.
//! - `Debug` uses the public `Formatter::debug_struct`/`debug_tuple` builders
//!   instead of the hidden `debug_struct_fieldN_finish` helpers, which are
//!   defined by core as exactly that builder sequence.
//! - Compile-time-only items are omitted: `#[automatically_derived]`,
//!   `#[inline]`, `#[doc(hidden)]` attributes, `AssertParamIsClone`/
//!   `AssertParamIsEq` assertions, `StructuralPartialEq` and `TrivialClone`
//!   impls, and `T::Assoc: Trait` where predicates for projections.
//! - Unreachable default arms (`unsafe { ::core::intrinsics::unreachable() }`)
//!   are replaced by an ordinary value of the arm type.
//! - Variant patterns and `Default`'s variant are written `Type::Variant`
//!   (the user's identifiers) rather than `Self::Variant`; inside the impl
//!   both name the same variant.
use super::{BuiltinFailureKind, BuiltinSite, Outcome, Output, failure, group, ident, std_path};
use crate::token_wire::{Delimiter, Key, Spacing, TokenForest, TokenTree};

type T = TokenTree;

/// The stable builtin derives of the pinned rustc (`register_derive!`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiltinDeriveKind {
    Clone,
    Copy,
    Debug,
    Default,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
}

impl BuiltinDeriveKind {
    /// By the rustc builtin name (`#[rustc_builtin_macro(Name)]` or the item name).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Clone" => Self::Clone,
            "Copy" => Self::Copy,
            "Debug" => Self::Debug,
            "Default" => Self::Default,
            "Hash" => Self::Hash,
            "PartialEq" => Self::PartialEq,
            "Eq" => Self::Eq,
            "PartialOrd" => Self::PartialOrd,
            "Ord" => Self::Ord,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Clone => "Clone",
            Self::Copy => "Copy",
            Self::Debug => "Debug",
            Self::Default => "Default",
            Self::Hash => "Hash",
            Self::PartialEq => "PartialEq",
            Self::Eq => "Eq",
            Self::PartialOrd => "PartialOrd",
            Self::Ord => "Ord",
        }
    }
}

/// Facts rustc's resolver records for the whole container before expanding
/// any of its derives (`has_derive_copy`, `has_derive_ord`): whether any derive
/// on the item resolves to the builtin `Copy`/`Ord`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeriveContainerFacts {
    pub derives_copy: bool,
    pub derives_ord: bool,
}

/// Expand one builtin derive. `input` has one root group whose children are
/// the item tokens; `call_site` (the derive path span) must be one of its span
/// records.
pub fn expand_builtin_derive(
    kind: BuiltinDeriveKind,
    input: &TokenForest,
    call_site: Key,
    site: &BuiltinSite,
    facts: DeriveContainerFacts,
) -> Outcome {
    input.validate()?;
    let [TokenTree::Group { children, .. }] = input.roots() else {
        return Err(super::fault("builtin derive input requires one root group"));
    };
    let item = match parse_item(children) {
        Ok(item) => item,
        Err(message) => {
            return Ok(Err(failure(
                BuiltinFailureKind::Syntax,
                message,
                Some(call_site),
            )));
        }
    };
    let mut out = Output::new(input, call_site, *site)?;
    let d = out.def_site()?;
    let g = Gen { d, item: &item };
    let tokens = match g.expand(kind, facts) {
        Ok(tokens) => tokens,
        Err(message) => {
            return Ok(Err(failure(
                BuiltinFailureKind::Syntax,
                message,
                Some(call_site),
            )));
        }
    };
    out.finish(tokens)
}

// ---------------------------------------------------------------------------
// Item parsing (token level, as the derive input is a single item).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Struct,
    Enum,
    Union,
}

enum ParamKind {
    Lifetime,
    Type,
    Const,
}

struct Param<'a> {
    kind: ParamKind,
    /// `'a` (two tokens), `T` or `N`.
    name: &'a [T],
    /// Tokens after `:` up to a default (lifetimes/types), or the const type.
    bounds: &'a [T],
}

enum Fields<'a> {
    Named(Vec<Field<'a>>),
    Tuple(Vec<Field<'a>>),
    Unit,
}

struct Field<'a> {
    name: Option<&'a T>,
    ty: &'a [T],
    /// A user span of this field (rustc `struct_field.span` for tuple indexes).
    span: Key,
}

struct Variant<'a> {
    name: &'a T,
    fields: Fields<'a>,
    discriminant: Option<&'a [T]>,
    default: bool,
}

struct Item<'a> {
    kind: Kind,
    name: &'a T,
    params: Vec<Param<'a>>,
    where_clause: &'a [T],
    data: Data<'a>,
    repr: Option<String>,
}

enum Data<'a> {
    Struct(Fields<'a>),
    Enum(Vec<Variant<'a>>),
}

fn is_punct(t: &T, c: char) -> bool {
    matches!(t, T::Punct { scalar, .. } if *scalar == c)
}

fn is_word(t: &T, word: &str) -> bool {
    matches!(t, T::Ident { text, raw: false, .. } if text == word)
}

fn span_of(t: &T) -> Key {
    match t {
        T::Group { open, .. } => *open,
        T::Ident { span, .. } | T::Punct { span, .. } | T::Literal { span, .. } => *span,
    }
}

/// `>` closing an arrow (`->`, `=>`) is not an angle bracket.
fn closes_angle(tokens: &[T], i: usize) -> bool {
    !(i > 0
        && matches!(
            &tokens[i - 1],
            T::Punct {
                scalar: '-' | '=',
                spacing: Spacing::Joint,
                ..
            }
        ))
}

/// A standalone `=` (not part of `==`, `<=`, `>=`, `!=`, `=>`, `+=`, ...).
fn is_assign(tokens: &[T], i: usize) -> bool {
    let T::Punct {
        scalar: '=',
        spacing,
        ..
    } = &tokens[i]
    else {
        return false;
    };
    let joined_before = i > 0
        && matches!(
            &tokens[i - 1],
            T::Punct {
                spacing: Spacing::Joint,
                ..
            }
        );
    *spacing != Spacing::Joint && !joined_before
}

/// Split at top-level commas, tracking `<...>` nesting. In a variant list,
/// after a standalone `=` at angle depth zero (a discriminant), angle tracking
/// stops until the next comma: the rest is an expression (`1 << 2`).
fn split_commas(tokens: &[T], discriminants: bool) -> Vec<&[T]> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut expression = false;
    for (i, t) in tokens.iter().enumerate() {
        match t {
            T::Punct { scalar: ',', .. } if depth == 0 || expression => {
                parts.push(&tokens[start..i]);
                start = i + 1;
                depth = 0;
                expression = false;
            }
            T::Punct { scalar: '<', .. } if !expression => depth += 1,
            T::Punct { scalar: '>', .. } if !expression && closes_angle(tokens, i) => {
                depth = depth.saturating_sub(1)
            }
            T::Punct { scalar: '=', .. } if discriminants && depth == 0 && is_assign(tokens, i) => {
                expression = true
            }
            _ => {}
        }
    }
    if start < tokens.len() {
        parts.push(&tokens[start..]);
    }
    parts
}

/// Skip outer attributes; report whether one is `#[default]`, and the repr
/// integer type if one is `#[repr(..)]`.
fn skip_attributes<'a>(mut tokens: &'a [T], default: &mut bool, repr: &mut Option<String>) -> &'a [T] {
    loop {
        match tokens {
            [hash, T::Group {
                delimiter: Delimiter::Bracket,
                children,
                ..
            }, rest @ ..]
                if is_punct(hash, '#') =>
            {
                if let [word] = children.as_slice()
                    && is_word(word, "default")
                {
                    *default = true;
                }
                if let [word, T::Group { children: args, .. }] = children.as_slice()
                    && is_word(word, "repr")
                {
                    for arg in args {
                        if let T::Ident { text, .. } = arg
                            && matches!(
                                text.as_str(),
                                "i8" | "i16"
                                    | "i32"
                                    | "i64"
                                    | "i128"
                                    | "isize"
                                    | "u8"
                                    | "u16"
                                    | "u32"
                                    | "u64"
                                    | "u128"
                                    | "usize"
                            )
                        {
                            *repr = Some(text.clone());
                        }
                    }
                }
                tokens = rest;
            }
            _ => return tokens,
        }
    }
}

/// Skip `pub`, `pub(crate)`, `pub(self)`, `pub(super)`, `pub(in path)`.
fn skip_visibility(tokens: &[T]) -> &[T] {
    match tokens {
        [first, T::Group {
            delimiter: Delimiter::Parenthesis,
            children,
            ..
        }, rest @ ..]
            if is_word(first, "pub")
                && matches!(children.first(), Some(t) if is_word(t, "crate") || is_word(t, "self") || is_word(t, "super") || is_word(t, "in")) =>
        {
            rest
        }
        [first, rest @ ..] if is_word(first, "pub") => rest,
        _ => tokens,
    }
}

fn parse_fields<'a>(group: &'a T) -> Result<Fields<'a>, String> {
    let T::Group {
        delimiter,
        children,
        ..
    } = group
    else {
        return Err("expected a field list".into());
    };
    let named = match delimiter {
        Delimiter::Brace => true,
        Delimiter::Parenthesis => false,
        _ => return Err("unexpected field delimiter".into()),
    };
    let mut fields = Vec::new();
    for part in split_commas(children, false) {
        let mut ignored = false;
        let mut repr = None;
        let part = skip_attributes(part, &mut ignored, &mut repr);
        let part = skip_visibility(part);
        let Some(first) = part.first() else {
            continue;
        };
        let span = span_of(first);
        if named {
            let [name @ T::Ident { .. }, colon, ty @ ..] = part else {
                return Err("expected a named field".into());
            };
            if !is_punct(colon, ':') || ty.is_empty() {
                return Err("expected `:` and a field type".into());
            }
            // Unstable field default values (`x: T = value`) end the type.
            let end = (0..ty.len())
                .find(|&i| is_assign(ty, i))
                .unwrap_or(ty.len());
            fields.push(Field {
                name: Some(name),
                ty: &ty[..end],
                span,
            });
        } else {
            fields.push(Field {
                name: None,
                ty: part,
                span,
            });
        }
    }
    Ok(if named {
        Fields::Named(fields)
    } else {
        Fields::Tuple(fields)
    })
}

fn parse_generics(tokens: &[T]) -> Result<(Vec<Param<'_>>, &[T]), String> {
    let Some(first) = tokens.first() else {
        return Ok((Vec::new(), tokens));
    };
    if !is_punct(first, '<') {
        return Ok((Vec::new(), tokens));
    }
    let mut depth = 0usize;
    let mut end = None;
    for (i, t) in tokens.iter().enumerate() {
        if is_punct(t, '<') {
            depth += 1;
        } else if is_punct(t, '>') && closes_angle(tokens, i) {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
    }
    let end = end.ok_or_else(|| "unclosed generic parameter list".to_owned())?;
    let mut params = Vec::new();
    for part in split_commas(&tokens[1..end], false) {
        let mut ignored = false;
        let mut repr = None;
        let part = skip_attributes(part, &mut ignored, &mut repr);
        if part.is_empty() {
            continue;
        }
        // The bound/type region ends at a default `=` at angle depth zero.
        let default_at = |region: &[T]| {
            let mut depth = 0usize;
            for (i, t) in region.iter().enumerate() {
                if is_punct(t, '<') {
                    depth += 1;
                } else if is_punct(t, '>') && closes_angle(region, i) {
                    depth = depth.saturating_sub(1);
                } else if depth == 0 && is_assign(region, i) {
                    return i;
                }
            }
            region.len()
        };
        let param = if is_punct(&part[0], '\'') {
            let name = part
                .get(..2)
                .ok_or_else(|| "invalid lifetime parameter".to_owned())?;
            let bounds = match part.get(2) {
                Some(colon) if is_punct(colon, ':') => &part[3..],
                Some(_) => return Err("invalid lifetime parameter".into()),
                None => &part[2..],
            };
            Param {
                kind: ParamKind::Lifetime,
                name,
                bounds,
            }
        } else if is_word(&part[0], "const") {
            let [_, _name, colon, ..] = part else {
                return Err("invalid const parameter".into());
            };
            if !is_punct(colon, ':') {
                return Err("invalid const parameter".into());
            }
            let ty = &part[3..];
            Param {
                kind: ParamKind::Const,
                name: &part[1..2],
                bounds: &ty[..default_at(ty)],
            }
        } else {
            let bounds = match part.get(1) {
                Some(colon) if is_punct(colon, ':') => {
                    let region = &part[2..];
                    &region[..default_at(region)]
                }
                _ => &part[1..1],
            };
            Param {
                kind: ParamKind::Type,
                name: &part[..1],
                bounds,
            }
        };
        params.push(param);
    }
    Ok((params, &tokens[end + 1..]))
}

fn parse_item(tokens: &[T]) -> Result<Item<'_>, String> {
    let mut ignored = false;
    let mut repr = None;
    let tokens = skip_attributes(tokens, &mut ignored, &mut repr);
    let tokens = skip_visibility(tokens);
    let [keyword, name @ T::Ident { .. }, rest @ ..] = tokens else {
        return Err("builtin derives apply to structs, enums and unions".into());
    };
    let kind = if is_word(keyword, "struct") {
        Kind::Struct
    } else if is_word(keyword, "enum") {
        Kind::Enum
    } else if is_word(keyword, "union") {
        Kind::Union
    } else {
        return Err("builtin derives apply to structs, enums and unions".into());
    };
    let (params, rest) = parse_generics(rest)?;
    // `where` before a braced body, or after a tuple body.
    let (where_clause, body, after) = match rest {
        [w, ..] if is_word(w, "where") => {
            let at = rest
                .iter()
                .position(|t| {
                    matches!(
                        t,
                        T::Group {
                            delimiter: Delimiter::Brace,
                            ..
                        }
                    ) || is_punct(t, ';')
                })
                .ok_or_else(|| "item body is absent".to_owned())?;
            (&rest[1..at], rest.get(at), &rest[at + 1..])
        }
        [body @ T::Group {
            delimiter: Delimiter::Parenthesis,
            ..
        }, after @ ..] => {
            let clause = match after {
                [w, clause @ ..] if is_word(w, "where") => {
                    let end = clause
                        .iter()
                        .position(|t| is_punct(t, ';'))
                        .unwrap_or(clause.len());
                    &clause[..end]
                }
                _ => &after[..0],
            };
            (clause, Some(body), &after[..0])
        }
        [body, after @ ..] => (&rest[..0], Some(body), after),
        [] => return Err("item body is absent".into()),
    };
    let _ = after;
    let body = body.ok_or_else(|| "item body is absent".to_owned())?;
    let data = match kind {
        Kind::Struct | Kind::Union => {
            if is_punct(body, ';') {
                Data::Struct(Fields::Unit)
            } else {
                Data::Struct(parse_fields(body)?)
            }
        }
        Kind::Enum => {
            let T::Group {
                delimiter: Delimiter::Brace,
                children,
                ..
            } = body
            else {
                return Err("enum body is absent".into());
            };
            let mut variants = Vec::new();
            for part in split_commas(children, true) {
                let mut default = false;
                let mut ignored = None;
                let part = skip_attributes(part, &mut default, &mut ignored);
                let part = skip_visibility(part);
                let [name @ T::Ident { .. }, rest @ ..] = part else {
                    if part.is_empty() {
                        continue;
                    }
                    return Err("expected an enum variant".into());
                };
                let (fields, rest) = match rest {
                    [
                        group @ T::Group {
                            delimiter: Delimiter::Brace | Delimiter::Parenthesis,
                            ..
                        },
                        rest @ ..,
                    ] => (parse_fields(group)?, rest),
                    _ => (Fields::Unit, rest),
                };
                let discriminant = match rest {
                    [eq, expr @ ..] if is_punct(eq, '=') && !expr.is_empty() => Some(expr),
                    [] => None,
                    _ => return Err("unexpected tokens after an enum variant".into()),
                };
                variants.push(Variant {
                    name,
                    fields,
                    discriminant,
                    default,
                });
            }
            Data::Enum(variants)
        }
    };
    Ok(Item {
        kind,
        name,
        params,
        where_clause,
        data,
        repr,
    })
}

impl Fields<'_> {
    fn list(&self) -> &[Field<'_>] {
        match self {
            Fields::Named(fields) | Fields::Tuple(fields) => fields,
            Fields::Unit => &[],
        }
    }
    fn is_empty(&self) -> bool {
        self.list().is_empty()
    }
}

/// rustc `TyKind::maybe_scalar` on the field type with references peeled.
fn maybe_scalar(mut ty: &[T]) -> bool {
    while let [amp, rest @ ..] = ty
        && is_punct(amp, '&')
    {
        ty = rest;
        if let [lt, name, rest @ ..] = ty
            && is_punct(lt, '\'')
            && matches!(name, T::Ident { .. })
        {
            ty = rest;
        }
        if let [m, rest @ ..] = ty
            && is_word(m, "mut")
        {
            ty = rest;
        }
    }
    match ty {
        [T::Group {
            delimiter: Delimiter::Parenthesis,
            children,
            ..
        }] => children.is_empty(),
        [T::Ident { text, raw: false, .. }] => matches!(
            text.as_str(),
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "f16"
                | "f32"
                | "f64"
                | "f128"
                | "char"
                | "bool"
        ),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Token generation.

struct Gen<'a> {
    d: Key,
    item: &'a Item<'a>,
}

/// One self-like argument's view of a field: `&self.x` for structs, the
/// pattern binding `__self_0` (a reference) for enum arms.
struct FieldExpr<'f> {
    field: &'f Field<'f>,
    self_expr: Vec<T>,
    other_expr: Option<Vec<T>>,
}

impl Gen<'_> {
    fn id(&self, text: &str) -> T {
        ident(text, self.d)
    }
    fn p(&self, c: char) -> T {
        T::Punct {
            scalar: c,
            spacing: Spacing::Alone,
            span: self.d,
        }
    }
    fn pj(&self, c: char) -> T {
        T::Punct {
            scalar: c,
            spacing: Spacing::Joint,
            span: self.d,
        }
    }
    fn colons(&self) -> [T; 2] {
        [self.pj(':'), self.p(':')]
    }
    fn arrow(&self) -> [T; 2] {
        [self.pj('-'), self.p('>')]
    }
    fn fat_arrow(&self) -> [T; 2] {
        [self.pj('='), self.p('>')]
    }
    fn eqeq(&self) -> [T; 2] {
        [self.pj('='), self.p('=')]
    }
    fn andand(&self) -> [T; 2] {
        [self.pj('&'), self.p('&')]
    }
    fn group(&self, delimiter: Delimiter, children: Vec<T>) -> T {
        group(delimiter, self.d, children)
    }
    fn paren(&self, children: Vec<T>) -> T {
        self.group(Delimiter::Parenthesis, children)
    }
    fn brace(&self, children: Vec<T>) -> T {
        self.group(Delimiter::Brace, children)
    }
    fn lit(&self, text: String) -> T {
        T::Literal {
            token_text: text,
            span: self.d,
        }
    }
    fn str_lit(&self, name: &T) -> T {
        let text = match name {
            T::Ident { text, .. } => text.as_str(),
            _ => "",
        };
        self.lit(format!("\"{}\"", text.escape_default()))
    }
    /// `$crate::a::b::c`.
    fn path(&self, components: &[&str]) -> Vec<T> {
        std_path(self.d, components)
    }
    /// `$crate::a::b::f(args)`.
    fn call(&self, components: &[&str], args: Vec<Vec<T>>) -> Vec<T> {
        let mut out = self.path(components);
        out.push(self.paren(self.comma_join(args)));
        out
    }
    fn comma_join(&self, parts: Vec<Vec<T>>) -> Vec<T> {
        let mut out = Vec::new();
        for (i, part) in parts.into_iter().enumerate() {
            if i > 0 {
                out.push(self.p(','));
            }
            out.extend(part);
        }
        out
    }
    fn amp(&self, mut expr: Vec<T>) -> Vec<T> {
        expr.insert(0, self.p('&'));
        expr
    }
    fn deref(&self, mut expr: Vec<T>) -> Vec<T> {
        expr.insert(0, self.p('*'));
        expr
    }
    fn some(&self, expr: Vec<T>) -> Vec<T> {
        self.call(&["option", "Option", "Some"], vec![expr])
    }
    fn equal(&self) -> Vec<T> {
        self.path(&["cmp", "Ordering", "Equal"])
    }
    /// `Type::Variant` with the user's identifiers.
    fn type_variant(&self, variant: &Variant<'_>) -> Vec<T> {
        let mut out = vec![self.item.name.clone()];
        out.extend(self.colons());
        out.push(variant.name.clone());
        out
    }
    /// The variant path of patterns and of `Default`. rustc writes
    /// `Self::Variant`; inside the impl it names the same variant as the
    /// user's `Type::Variant`, which is what is emitted.
    fn self_variant(&self, variant: &Variant<'_>) -> Vec<T> {
        self.type_variant(variant)
    }
    fn binding(&self, prefix: &str, index: usize) -> T {
        self.id(&format!("{prefix}_{index}"))
    }
    /// `self.x` / `self.0` with the field's own identifier/span.
    fn field_access(&self, receiver: &str, index: usize, field: &Field<'_>) -> Vec<T> {
        vec![
            self.id(receiver),
            self.p('.'),
            match field.name {
                Some(name) => name.clone(),
                None => T::Literal {
                    token_text: index.to_string(),
                    span: field.span,
                },
            },
        ]
    }
    /// A constructor or pattern: `path { a: e0, b: e1 }`, `path(e0, e1)`, `path`.
    fn construct(&self, path: Vec<T>, fields: &Fields<'_>, mut part: impl FnMut(usize, &Field<'_>) -> Vec<T>) -> Vec<T> {
        let mut out = path;
        match fields {
            Fields::Named(list) => {
                let mut body = Vec::new();
                for (i, field) in list.iter().enumerate() {
                    if i > 0 {
                        body.push(self.p(','));
                    }
                    body.push(field.name.expect("named field").clone());
                    body.push(self.p(':'));
                    body.extend(part(i, field));
                }
                out.push(self.brace(body));
            }
            Fields::Tuple(list) => {
                let parts = list
                    .iter()
                    .enumerate()
                    .map(|(i, field)| part(i, field))
                    .collect();
                out.push(self.paren(self.comma_join(parts)));
            }
            Fields::Unit => {}
        }
        out
    }
    /// `Self::V(__self_0, ..)` for one self-like prefix.
    fn pattern(&self, variant: &Variant<'_>, prefix: &str) -> Vec<T> {
        self.construct(self.self_variant(variant), &variant.fields, |i, _| {
            vec![self.binding(prefix, i)]
        })
    }
    fn has_type_params(&self) -> bool {
        self.item
            .params
            .iter()
            .any(|p| matches!(p.kind, ParamKind::Type))
    }

    /// `impl<..> TRAIT for Name<..> where .. { items }` (`create_derived_impl`).
    fn impl_item(&self, trait_path: &[&str], bound_params: bool, items: Vec<T>) -> Vec<T> {
        let mut out = vec![self.id("impl")];
        if !self.item.params.is_empty() {
            out.push(self.p('<'));
            for (i, param) in self.item.params.iter().enumerate() {
                if i > 0 {
                    out.push(self.p(','));
                }
                match param.kind {
                    ParamKind::Lifetime => {
                        out.extend(param.name.iter().cloned());
                        if !param.bounds.is_empty() {
                            out.push(self.p(':'));
                            out.extend(param.bounds.iter().cloned());
                        }
                    }
                    ParamKind::Type => {
                        out.extend(param.name.iter().cloned());
                        let mut bounds = Vec::new();
                        if bound_params {
                            bounds.push(self.path(trait_path));
                        }
                        if !param.bounds.is_empty() {
                            bounds.push(param.bounds.to_vec());
                        }
                        if !bounds.is_empty() {
                            out.push(self.p(':'));
                            for (i, bound) in bounds.into_iter().enumerate() {
                                if i > 0 {
                                    out.push(self.p('+'));
                                }
                                out.extend(bound);
                            }
                        }
                    }
                    ParamKind::Const => {
                        out.push(self.id("const"));
                        out.extend(param.name.iter().cloned());
                        out.push(self.p(':'));
                        out.extend(param.bounds.iter().cloned());
                    }
                }
            }
            out.push(self.p('>'));
        }
        out.extend(self.path(trait_path));
        out.push(self.id("for"));
        out.push(self.item.name.clone());
        if !self.item.params.is_empty() {
            out.push(self.p('<'));
            for (i, param) in self.item.params.iter().enumerate() {
                if i > 0 {
                    out.push(self.p(','));
                }
                out.extend(param.name.iter().cloned());
            }
            out.push(self.p('>'));
        }
        if !self.item.where_clause.is_empty() {
            out.push(self.id("where"));
            out.extend(self.item.where_clause.iter().cloned());
        }
        out.push(self.brace(items));
        out
    }

    /// `fn name(&self, other: &Self) -> ret { body }`.
    fn method(&self, name: &str, generics: Vec<T>, self_arg: bool, args: Vec<Vec<T>>, ret: Option<Vec<T>>, body: Vec<T>) -> Vec<T> {
        let mut out = vec![self.id("fn"), self.id(name)];
        out.extend(generics);
        let mut params = Vec::new();
        if self_arg {
            params.push(vec![self.p('&'), self.id("self")]);
        }
        params.extend(args);
        out.push(self.paren(self.comma_join(params)));
        if let Some(ret) = ret {
            out.extend(self.arrow());
            out.extend(ret);
        }
        out.push(self.brace(body));
        out
    }

    fn other_arg(&self) -> Vec<T> {
        vec![self.id("other"), self.p(':'), self.p('&'), self.id("Self")]
    }

    /// The fields of a struct as `&self.x` (and `&other.x`).
    fn struct_fields<'f>(&self, fields: &'f Fields<'f>, other: bool) -> Vec<FieldExpr<'f>> {
        fields
            .list()
            .iter()
            .enumerate()
            .map(|(i, field)| FieldExpr {
                field,
                self_expr: self.amp(self.field_access("self", i, field)),
                other_expr: other.then(|| self.amp(self.field_access("other", i, field))),
            })
            .collect()
    }
    /// The fields of an enum arm as pattern bindings.
    fn arm_fields<'f>(&self, fields: &'f Fields<'f>, other: bool) -> Vec<FieldExpr<'f>> {
        fields
            .list()
            .iter()
            .enumerate()
            .map(|(i, field)| FieldExpr {
                field,
                self_expr: vec![self.binding("__self", i)],
                other_expr: other.then(|| vec![self.binding("__arg1", i)]),
            })
            .collect()
    }

    /// `match *self {}` for an enum without variants.
    fn empty_match(&self) -> Vec<T> {
        vec![
            self.id("match"),
            self.p('*'),
            self.id("self"),
            self.brace(Vec::new()),
        ]
    }

    /// `let __self_discr: R = match self { Self::A { .. } => 0, .. };`
    fn discriminant_let(&self, variants: &[Variant<'_>], name: &str, receiver: &str) -> Vec<T> {
        let repr = self.item.repr.as_deref().unwrap_or("isize");
        let mut arms = Vec::new();
        let mut base: Option<&[T]> = None;
        let mut offset = 0u128;
        for variant in variants {
            let value = match variant.discriminant {
                Some(expr) => {
                    base = Some(expr);
                    offset = 0;
                    vec![self.paren(expr.to_vec())]
                }
                None => {
                    let value = match base {
                        Some(expr) => {
                            offset += 1;
                            vec![
                                self.paren(expr.to_vec()),
                                self.p('+'),
                                self.lit(offset.to_string()),
                            ]
                        }
                        None => {
                            let value = vec![self.lit(offset.to_string())];
                            offset += 1;
                            value
                        }
                    };
                    value
                }
            };
            arms.extend(self.self_variant(variant));
            match variant.fields {
                Fields::Named(_) => arms.push(self.brace(vec![self.pj('.'), self.p('.')])),
                Fields::Tuple(_) => arms.push(self.paren(vec![self.pj('.'), self.p('.')])),
                Fields::Unit => {}
            }
            arms.extend(self.fat_arrow());
            arms.extend(value);
            arms.push(self.p(','));
        }
        let mut out = vec![
            self.id("let"),
            self.id(name),
            self.p(':'),
            self.id(repr),
            self.p('='),
            self.id("match"),
            self.id(receiver),
        ];
        out.push(self.brace(arms));
        out.push(self.p(';'));
        out
    }

    /// `match (self, other) { arms.. , _ => default }` or `match self { .. }`.
    fn variant_match(
        &self,
        variants: &[Variant<'_>],
        other: bool,
        skip_fieldless: bool,
        mut body: impl FnMut(&Variant<'_>, Vec<FieldExpr<'_>>) -> Vec<T>,
        default: Option<Vec<T>>,
    ) -> Vec<T> {
        let mut arms = Vec::new();
        for variant in variants {
            if skip_fieldless && variant.fields.is_empty() {
                continue;
            }
            if other {
                let mut pats = self.pattern(variant, "__self");
                pats.push(self.p(','));
                pats.extend(self.pattern(variant, "__arg1"));
                arms.push(self.paren(pats));
            } else {
                arms.extend(self.pattern(variant, "__self"));
            }
            arms.extend(self.fat_arrow());
            arms.extend(body(variant, self.arm_fields(&variant.fields, other)));
            arms.push(self.p(','));
        }
        if let Some(default) = default {
            arms.push(self.id("_"));
            arms.extend(self.fat_arrow());
            arms.extend(default);
            arms.push(self.p(','));
        }
        let scrutinee = if other {
            vec![self.paren(vec![self.id("self"), self.p(','), self.id("other")])]
        } else {
            vec![self.id("self")]
        };
        let mut out = vec![self.id("match")];
        out.extend(scrutinee);
        out.push(self.brace(arms));
        out
    }

    fn expand(&self, kind: BuiltinDeriveKind, facts: DeriveContainerFacts) -> Result<Vec<T>, String> {
        if self.item.kind == Kind::Union
            && !matches!(kind, BuiltinDeriveKind::Clone | BuiltinDeriveKind::Copy | BuiltinDeriveKind::Eq)
        {
            return Err(format!("this trait cannot be derived for unions: `{}`", kind.name()));
        }
        Ok(match kind {
            BuiltinDeriveKind::Copy => self.impl_item(&["marker", "Copy"], true, Vec::new()),
            BuiltinDeriveKind::Eq => self.impl_item(&["cmp", "Eq"], true, Vec::new()),
            BuiltinDeriveKind::Clone => self.clone_impl(facts),
            BuiltinDeriveKind::Debug => self.debug_impl(),
            BuiltinDeriveKind::Default => self.default_impl()?,
            BuiltinDeriveKind::Hash => self.hash_impl(),
            BuiltinDeriveKind::PartialEq => self.partial_eq_impl(),
            BuiltinDeriveKind::PartialOrd => self.ord_impl(true, facts),
            BuiltinDeriveKind::Ord => self.ord_impl(false, facts),
        })
    }

    // `clone.rs`
    fn clone_impl(&self, facts: DeriveContainerFacts) -> Vec<T> {
        let simple = self.item.kind == Kind::Union
            || (facts.derives_copy && !self.has_type_params());
        let body = if simple {
            self.deref(vec![self.id("self")])
        } else {
            let clone = |expr: Vec<T>| self.call(&["clone", "Clone", "clone"], vec![expr]);
            match &self.item.data {
                Data::Struct(fields) => {
                    let exprs = self.struct_fields(fields, false);
                    self.construct(vec![self.item.name.clone()], fields, |i, _| {
                        clone(exprs[i].self_expr.clone())
                    })
                }
                Data::Enum(variants) if variants.is_empty() => self.empty_match(),
                Data::Enum(variants) => self.variant_match(
                    variants,
                    false,
                    false,
                    |variant, fields| {
                        self.construct(self.type_variant(variant), &variant.fields, |i, _| {
                            clone(fields[i].self_expr.clone())
                        })
                    },
                    None,
                ),
            }
        };
        let method = self.method("clone", Vec::new(), true, Vec::new(), Some(vec![self.id("Self")]), body);
        self.impl_item(&["clone", "Clone"], true, method)
    }

    // `partial_eq.rs`
    fn partial_eq_impl(&self) -> Vec<T> {
        let fields_eq = |fields: Vec<FieldExpr<'_>>, by_ref: bool| -> Vec<T> {
            let ordered = fields
                .iter()
                .filter(|f| maybe_scalar(f.field.ty))
                .chain(fields.iter().filter(|f| !maybe_scalar(f.field.ty)));
            let mut out = Vec::new();
            for (i, field) in ordered.enumerate() {
                if i > 0 {
                    out.extend(self.andand());
                }
                let (lhs, rhs) = (
                    field.self_expr.clone(),
                    field.other_expr.clone().expect("two self-like arguments"),
                );
                // rustc peels the `&` of `&self.x`; enum bindings are references.
                let (lhs, rhs) = if by_ref {
                    (self.deref(lhs), self.deref(rhs))
                } else {
                    (lhs[1..].to_vec(), rhs[1..].to_vec())
                };
                out.extend(lhs);
                out.extend(self.eqeq());
                out.extend(rhs);
            }
            if out.is_empty() {
                out.push(self.id("true"));
            }
            out
        };
        let body = match &self.item.data {
            Data::Struct(fields) => fields_eq(self.struct_fields(fields, true), false),
            Data::Enum(variants) => self.enum_body(variants, true, |variant, fields| {
                let _ = variant;
                fields_eq(fields, true)
            }, |discr| {
                let mut out = vec![self.id("__self_discr")];
                out.extend(self.eqeq());
                out.push(self.id("__arg1_discr"));
                if let Some(rest) = discr {
                    out.extend(self.andand());
                    out.extend(rest);
                }
                out
            }, vec![self.id("true")], vec![self.id("true")]),
        };
        let method = self.method(
            "eq",
            Vec::new(),
            true,
            vec![self.other_arg()],
            Some(vec![self.id("bool")]),
            body,
        );
        self.impl_item(&["cmp", "PartialEq"], true, method)
    }

    /// The shared enum shape of the `Unify` traits (`expand_enum_method_body`):
    /// `discr(Some(match))` combines a discriminant check with the variant
    /// match; `fieldless` is the arm for unified fieldless variants;
    /// `single` is the result for a lone fieldless variant.
    fn enum_body(
        &self,
        variants: &[Variant<'_>],
        other: bool,
        arm: impl FnMut(&Variant<'_>, Vec<FieldExpr<'_>>) -> Vec<T>,
        discr: impl FnOnce(Option<Vec<T>>) -> Vec<T>,
        fieldless: Vec<T>,
        single: Vec<T>,
    ) -> Vec<T> {
        if variants.is_empty() {
            return self.empty_match();
        }
        let all_fieldless = variants.iter().all(|v| v.fields.is_empty());
        let mut lets = self.discriminant_let(variants, "__self_discr", "self");
        if other {
            lets.extend(self.discriminant_let(variants, "__arg1_discr", "other"));
        }
        if all_fieldless {
            if variants.len() > 1 {
                lets.extend(discr(None));
                return lets;
            }
            return single;
        }
        let has_fieldless = variants.iter().any(|v| v.fields.is_empty());
        let default = if has_fieldless || (variants.len() > 1 && other) {
            Some(fieldless)
        } else {
            None
        };
        let matched = self.variant_match(variants, other, true, arm, default);
        if variants.len() > 1 {
            lets.extend(discr(Some(matched)));
            lets
        } else {
            matched
        }
    }

    // `partial_ord.rs` / `ord.rs`
    fn ord_impl(&self, partial: bool, facts: DeriveContainerFacts) -> Vec<T> {
        let method: &[&str] = if partial {
            &["cmp", "PartialOrd", "partial_cmp"]
        } else {
            &["cmp", "Ord", "cmp"]
        };
        let equal_value = || {
            if partial {
                self.some(self.equal())
            } else {
                self.equal()
            }
        };
        // `match new { Some(Equal) => old, cmp => cmp }`.
        let combine = |old: Vec<T>, new: Vec<T>| {
            let mut arms = if partial {
                self.some(self.equal())
            } else {
                self.equal()
            };
            arms.extend(self.fat_arrow());
            arms.extend(old);
            arms.push(self.p(','));
            arms.push(self.id("cmp"));
            arms.extend(self.fat_arrow());
            arms.push(self.id("cmp"));
            arms.push(self.p(','));
            let mut out = vec![self.id("match")];
            out.extend(new);
            out.push(self.brace(arms));
            out
        };
        let fields_cmp = |fields: Vec<FieldExpr<'_>>| -> Vec<T> {
            let mut acc: Option<Vec<T>> = None;
            for field in fields.into_iter().rev() {
                let new = self.call(
                    method,
                    vec![field.self_expr, field.other_expr.expect("two self-like arguments")],
                );
                acc = Some(match acc {
                    Some(old) => combine(old, new),
                    None => new,
                });
            }
            acc.unwrap_or_else(equal_value)
        };
        let is_simple = partial
            && facts.derives_ord
            && !self.has_type_params()
            && match &self.item.data {
                Data::Struct(Fields::Unit) => false,
                Data::Enum(variants) if variants.is_empty() => false,
                Data::Enum(variants) if variants.len() == 1 && matches!(variants[0].fields, Fields::Unit) => false,
                _ => true,
            };
        let body = if is_simple {
            self.some(self.call(&["cmp", "Ord", "cmp"], vec![vec![self.id("self")], vec![self.id("other")]]))
        } else {
            match &self.item.data {
                Data::Struct(fields) => fields_cmp(self.struct_fields(fields, true)),
                Data::Enum(variants) => self.enum_body(
                    variants,
                    true,
                    |_, fields| fields_cmp(fields),
                    |matched| {
                        let check = self.call(
                            method,
                            vec![
                                self.amp(vec![self.id("__self_discr")]),
                                self.amp(vec![self.id("__arg1_discr")]),
                            ],
                        );
                        match matched {
                            Some(matched) => combine(matched, check),
                            None => check,
                        }
                    },
                    equal_value(),
                    equal_value(),
                ),
            }
        };
        let ret = if partial {
            let mut ret = self.path(&["option", "Option"]);
            ret.push(self.p('<'));
            ret.extend(self.path(&["cmp", "Ordering"]));
            ret.push(self.p('>'));
            ret
        } else {
            self.path(&["cmp", "Ordering"])
        };
        let name = if partial { "partial_cmp" } else { "cmp" };
        let item = self.method(name, Vec::new(), true, vec![self.other_arg()], Some(ret), body);
        if partial {
            self.impl_item(&["cmp", "PartialOrd"], true, item)
        } else {
            self.impl_item(&["cmp", "Ord"], true, item)
        }
    }

    // `hash.rs`
    fn hash_impl(&self) -> Vec<T> {
        let hash_stmt = |expr: Vec<T>| {
            let mut out = self.call(&["hash", "Hash", "hash"], vec![expr, vec![self.id("state")]]);
            out.push(self.p(';'));
            out
        };
        let fields_hash = |fields: Vec<FieldExpr<'_>>| -> Vec<T> {
            let mut out = Vec::new();
            for field in fields {
                out.extend(hash_stmt(field.self_expr));
            }
            out
        };
        let body = match &self.item.data {
            Data::Struct(fields) => fields_hash(self.struct_fields(fields, false)),
            Data::Enum(variants) => self.enum_body(
                variants,
                false,
                |_, fields| vec![self.brace(fields_hash(fields))],
                |matched| {
                    let mut out = hash_stmt(self.amp(vec![self.id("__self_discr")]));
                    if let Some(matched) = matched {
                        out.extend(matched);
                    }
                    out
                },
                vec![self.brace(Vec::new())],
                Vec::new(),
            ),
        };
        let mut generics = vec![self.p('<'), self.id("__H"), self.p(':')];
        generics.extend(self.path(&["hash", "Hasher"]));
        generics.push(self.p('>'));
        let state = vec![
            self.id("state"),
            self.p(':'),
            self.p('&'),
            self.id("mut"),
            self.id("__H"),
        ];
        let method = self.method(
            "hash",
            generics,
            true,
            vec![state],
            Some(vec![self.paren(Vec::new())]),
            body,
        );
        self.impl_item(&["hash", "Hash"], true, method)
    }

    // `default.rs`
    fn default_impl(&self) -> Result<Vec<T>, String> {
        let default_call = || self.call(&["default", "Default", "default"], Vec::new());
        let (body, bound) = match &self.item.data {
            Data::Struct(fields) => (
                self.construct(vec![self.item.name.clone()], fields, |_, _| default_call()),
                true,
            ),
            Data::Enum(variants) => {
                let defaults: Vec<_> = variants.iter().filter(|v| v.default).collect();
                let [variant] = defaults.as_slice() else {
                    return Err(if defaults.is_empty() {
                        "no default declared: `#[derive(Default)]` on enum with no `#[default]`".into()
                    } else {
                        "multiple declared defaults".into()
                    });
                };
                if !matches!(variant.fields, Fields::Unit) {
                    return Err("the `#[default]` attribute may only be used on unit enum variants".into());
                }
                (self.self_variant(variant), false)
            }
        };
        let method = self.method("default", Vec::new(), false, Vec::new(), Some(vec![self.id("Self")]), body);
        Ok(self.impl_item(&["default", "Default"], bound, method))
    }

    // `debug.rs`
    fn debug_impl(&self) -> Vec<T> {
        let write_str = |name: Vec<T>| {
            self.call(&["fmt", "Formatter", "write_str"], vec![vec![self.id("f")], name])
        };
        let show = |name: &T, fields: &Fields<'_>, exprs: Vec<FieldExpr<'_>>| -> Vec<T> {
            if exprs.is_empty() {
                return write_str(vec![self.str_lit(name)]);
            }
            let (builder, builder_type) = match fields {
                Fields::Named(_) => ("debug_struct", "DebugStruct"),
                _ => ("debug_tuple", "DebugTuple"),
            };
            let len = exprs.len();
            let mut block = vec![self.id("let"), self.id("mut"), self.id("__builder"), self.p('=')];
            block.extend(self.call(
                &["fmt", "Formatter", builder],
                vec![vec![self.id("f")], vec![self.str_lit(name)]],
            ));
            block.push(self.p(';'));
            for (i, field) in exprs.into_iter().enumerate() {
                // rustc passes `&&self.last` so an unsized last field coerces.
                let value = if i + 1 < len {
                    field.self_expr
                } else {
                    self.amp(field.self_expr)
                };
                let mut args = vec![vec![self.p('&'), self.id("mut"), self.id("__builder")]];
                if let Some(field_name) = field.field.name {
                    args.push(vec![self.str_lit(field_name)]);
                }
                args.push(value);
                block.extend(self.call(&["fmt", builder_type, "field"], args));
                block.push(self.p(';'));
            }
            block.extend(self.call(
                &["fmt", builder_type, "finish"],
                vec![vec![self.p('&'), self.id("mut"), self.id("__builder")]],
            ));
            vec![self.brace(block)]
        };
        let body = match &self.item.data {
            Data::Struct(fields) => show(self.item.name, fields, self.struct_fields(fields, false)),
            Data::Enum(variants) if variants.is_empty() => self.empty_match(),
            Data::Enum(variants) if variants.len() > 1 && variants.iter().all(|v| v.fields.is_empty()) => {
                // `show_fieldless_enum`.
                let mut arms = Vec::new();
                for variant in variants {
                    arms.extend(self.type_variant(variant));
                    match variant.fields {
                        Fields::Named(_) => arms.push(self.brace(Vec::new())),
                        Fields::Tuple(_) => arms.push(self.paren(Vec::new())),
                        Fields::Unit => {}
                    }
                    arms.extend(self.fat_arrow());
                    arms.push(self.str_lit(variant.name));
                    arms.push(self.p(','));
                }
                let mut name = vec![self.id("match"), self.id("self")];
                name.push(self.brace(arms));
                write_str(name)
            }
            Data::Enum(variants) => self.variant_match(
                variants,
                false,
                false,
                |variant, fields| show(variant.name, &variant.fields, fields),
                None,
            ),
        };
        let mut formatter = vec![self.p('&'), self.id("mut")];
        formatter.extend(self.path(&["fmt", "Formatter"]));
        let method = self.method(
            "fmt",
            Vec::new(),
            true,
            vec![{
                let mut arg = vec![self.id("f"), self.p(':')];
                arg.extend(formatter);
                arg
            }],
            Some(self.path(&["fmt", "Result"])),
            body,
        );
        self.impl_item(&["fmt", "Debug"], true, method)
    }
}
