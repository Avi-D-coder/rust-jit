//! Owned token content. No upstream span, symbol, file, or arena ID crosses this boundary.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize)]
pub struct WireError {
    pub code: &'static str,
    pub message: String,
}
impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for WireError {}

use std::collections::BTreeMap;
const MAX_WIRE: usize = usize::MAX;
pub(super) fn error(message: &str) -> WireError {
    WireError {
        code: "LRR-E-MACRO-TOKEN-WIRE",
        message: message.into(),
    }
}
pub(super) fn limit(message: &str) -> WireError {
    WireError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
/// Content key, computed from explicitly encoded semantic records, never allocation order.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Key(pub [u8; 32]);
struct Encoder(blake3::Hasher);
impl Encoder {
    fn new(domain: &str) -> Self {
        let mut e = Self(blake3::Hasher::new());
        e.text(domain);
        e
    }
    fn byte(&mut self, v: u8) {
        self.0.update(&[v]);
    }
    fn u32(&mut self, v: u32) {
        self.0.update(&v.to_le_bytes());
    }
    fn count(&mut self, v: usize) {
        self.0.update(&(v as u64).to_le_bytes());
    }
    fn key(&mut self, v: Key) {
        self.0.update(&v.0);
    }
    fn optional(&mut self, v: Option<Key>) {
        match v {
            Some(v) => {
                self.byte(1);
                self.key(v)
            }
            None => self.byte(0),
        }
    }
    fn text(&mut self, v: &str) {
        self.count(v.len());
        self.0.update(v.as_bytes());
    }
    fn finish(self) -> Key {
        Key(*self.0.finalize().as_bytes())
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Edition {
    E2015,
    E2018,
    E2021,
    E2024,
}
impl Edition {
    fn tag(self) -> u8 {
        match self {
            Self::E2015 => 0,
            Self::E2018 => 1,
            Self::E2021 => 2,
            Self::E2024 => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Transparency {
    Opaque,
    SemiTransparent,
    Transparent,
}
impl Transparency {
    fn tag(self) -> u8 {
        match self {
            Self::Opaque => 0,
            Self::SemiTransparent => 1,
            Self::Transparent => 2,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRecord {
    pub parent: Option<Key>,
    pub expansion: Option<Key>,
    pub transparency: Transparency,
    pub defining_unit: Key,
    pub edition: Edition,
}
/// Owned normal forms computed by the pinned adapter. These are context keys,
/// never raw frontend/interner handles. Runtime consumes facts without Salsa.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionContext {
    pub original: Key,
    pub item: Key,
    pub local: Key,
}
/// Comparison-only identity; never a source/context authority or span key.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct NameContextKey(pub Key);
impl NameContextKey {
    pub fn from_canonical_context(context: Key) -> Self {
        let mut e = Encoder::new("lrr/v2/name-comparison-class");
        e.key(context);
        Self(e.finish())
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameResolutionContext {
    pub original: Key,
    pub item: NameContextKey,
    pub local: NameContextKey,
}
impl ContextRecord {
    pub fn key(&self) -> Key {
        let mut e = Encoder::new("lrr/v2/syntax-context");
        self.encode(&mut e);
        e.finish()
    }
    fn encode(&self, e: &mut Encoder) {
        e.optional(self.parent);
        e.optional(self.expansion);
        e.byte(self.transparency.tag());
        e.key(self.defining_unit);
        e.byte(self.edition.tag());
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub unit: Key,
    pub content: Key,
    pub path: String,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanRecord {
    pub source: SourceIdentity,
    pub start: u32,
    pub end: u32,
    pub context: Key,
    pub edition: Edition,
    pub origin: Key,
}
impl SpanRecord {
    pub fn key(&self) -> Key {
        let mut e = Encoder::new("lrr/v2/token-span");
        self.encode(&mut e);
        e.finish()
    }
    /// Change name-resolution context while retaining the original source location and origin.
    pub fn resolved_at(&self, other: &Self) -> Self {
        Self {
            context: other.context,
            ..self.clone()
        }
    }
    /// Change source location while retaining this span's resolution context and origin chain.
    pub fn located_at(&self, other: &Self) -> Self {
        Self {
            source: other.source.clone(),
            start: other.start,
            end: other.end,
            edition: other.edition,
            ..self.clone()
        }
    }
    fn encode(&self, e: &mut Encoder) {
        e.key(self.source.unit);
        e.key(self.source.content);
        e.text(&self.source.path);
        e.u32(self.start);
        e.u32(self.end);
        e.key(self.context);
        e.byte(self.edition.tag());
        e.key(self.origin);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    Invisible,
}
impl Delimiter {
    fn tag(self) -> u8 {
        match self {
            Self::Parenthesis => 0,
            Self::Brace => 1,
            Self::Bracket => 2,
            Self::Invisible => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Spacing {
    Alone,
    Joint,
    JointHidden,
}
impl Spacing {
    fn tag(self) -> u8 {
        match self {
            Self::Alone => 0,
            Self::Joint => 1,
            Self::JointHidden => 2,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TokenTree {
    Group {
        delimiter: Delimiter,
        open: Key,
        close: Key,
        children: Vec<TokenTree>,
    },
    Ident {
        text: String,
        raw: bool,
        span: Key,
    },
    Punct {
        scalar: char,
        spacing: Spacing,
        span: Key,
    },
    Literal {
        token_text: String,
        span: Key,
    },
}
impl TokenTree {
    fn encode(&self, e: &mut Encoder) {
        match self {
            Self::Group {
                delimiter,
                open,
                close,
                children,
            } => {
                e.byte(0);
                e.byte(delimiter.tag());
                e.key(*open);
                e.key(*close);
                e.count(children.len());
                for t in children {
                    t.encode(e)
                }
            }
            Self::Ident { text, raw, span } => {
                e.byte(1);
                e.text(text);
                e.byte(u8::from(*raw));
                e.key(*span)
            }
            Self::Punct {
                scalar,
                spacing,
                span,
            } => {
                e.byte(2);
                e.u32(u32::from(*scalar));
                e.byte(spacing.tag());
                e.key(*span)
            }
            Self::Literal { token_text, span } => {
                e.byte(3);
                e.text(token_text);
                e.key(*span)
            }
        }
    }
}
/// The stable key of every span record, in table order, computed once per
/// forest (each key is a hash of its record). A derived cache: it takes no part
/// in equality, hashing or identity.
#[derive(Default)]
struct SpanKeyIndex(std::sync::OnceLock<Vec<Key>>);
impl Clone for SpanKeyIndex {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl PartialEq for SpanKeyIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for SpanKeyIndex {}
impl std::hash::Hash for SpanKeyIndex {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}
impl std::fmt::Debug for SpanKeyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpanKeyIndex")
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TokenForest {
    schema: u32,
    roots: Vec<TokenTree>,
    contexts: Vec<ContextRecord>,
    spans: Vec<SpanRecord>,
    span_keys: SpanKeyIndex,
}
impl TokenForest {
    pub fn new(
        roots: Vec<TokenTree>,
        mut contexts: Vec<ContextRecord>,
        mut spans: Vec<SpanRecord>,
    ) -> Result<Self, WireError> {
        let mut source_text = 0;
        for span in &spans {
            add_text(&mut source_text, &span.source.path)?;
        }
        contexts.sort_by_cached_key(ContextRecord::key);
        spans.sort_by_cached_key(SpanRecord::key);
        let f = Self {
            schema: 2,
            roots,
            contexts,
            spans,
            span_keys: SpanKeyIndex::default(),
        };
        f.validate()?;
        Ok(f)
    }
    /// The stable key of each span record, in table order (computed once).
    pub fn span_keys(&self) -> &[Key] {
        self.span_keys
            .0
            .get_or_init(|| self.spans.iter().map(SpanRecord::key).collect())
    }
    /// The position of the span record with `key` in the key-ordered table.
    pub fn span_position(&self, key: &Key) -> Option<usize> {
        self.span_keys().binary_search(key).ok()
    }
    pub fn roots(&self) -> &[TokenTree] {
        &self.roots
    }
    pub fn contexts(&self) -> &[ContextRecord] {
        &self.contexts
    }
    pub fn spans(&self) -> &[SpanRecord] {
        &self.spans
    }
    /// Retained owned storage, excluding allocator bookkeeping and external Arc owners.
    /// Allocation capacities are accounting data, never semantic identities.
    pub fn allocated_bytes(&self) -> Result<usize, WireError> {
        self.validate()?;
        let add = |a: usize, b: usize| {
            a.checked_add(b)
                .ok_or_else(|| limit("owned token allocation size overflow"))
        };
        let array = |capacity: usize, size: usize| {
            capacity
                .checked_mul(size)
                .ok_or_else(|| limit("owned token allocation size overflow"))
        };
        let mut bytes = std::mem::size_of::<Self>();
        bytes = add(
            bytes,
            array(self.roots.capacity(), std::mem::size_of::<TokenTree>())?,
        )?;
        bytes = add(
            bytes,
            array(
                self.contexts.capacity(),
                std::mem::size_of::<ContextRecord>(),
            )?,
        )?;
        bytes = add(
            bytes,
            array(self.spans.capacity(), std::mem::size_of::<SpanRecord>())?,
        )?;
        for span in &self.spans {
            bytes = add(bytes, span.source.path.capacity())?;
        }
        let mut pending: Vec<_> = self.roots.iter().collect();
        while let Some(token) = pending.pop() {
            match token {
                TokenTree::Group { children, .. } => {
                    bytes = add(
                        bytes,
                        array(children.capacity(), std::mem::size_of::<TokenTree>())?,
                    )?;
                    pending.extend(children);
                }
                TokenTree::Ident { text, .. } => bytes = add(bytes, text.capacity())?,
                TokenTree::Literal { token_text, .. } => bytes = add(bytes, token_text.capacity())?,
                TokenTree::Punct { .. } => {}
            }
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<(), WireError> {
        if self.schema != 2 {
            return Err(error("unsupported token schema"));
        }
        // Each stable key is one hash; compute it once per record.
        let context_keys: Vec<Key> = self.contexts.iter().map(ContextRecord::key).collect();
        let span_keys = self.span_keys();
        let contexts: BTreeMap<_, _> = context_keys.iter().copied().zip(&self.contexts).collect();
        let spans: std::collections::BTreeSet<Key> = span_keys.iter().copied().collect();
        if contexts.len() != self.contexts.len()
            || spans.len() != self.spans.len()
            || !context_keys.windows(2).all(|w| w[0] < w[1])
            || !span_keys.windows(2).all(|w| w[0] < w[1])
        {
            return Err(error("duplicate or noncanonical stable table"));
        }
        for c in &self.contexts {
            if c.parent.is_some() != c.expansion.is_some() {
                return Err(error("context parent/expansion mismatch"));
            }
            let mut next = c.parent;
            while let Some(key) = next {
                next = contexts
                    .get(&key)
                    .ok_or_else(|| error("unknown parent context"))?
                    .parent;
            }
        }
        let mut text = 0usize;
        for s in &self.spans {
            if s.start > s.end
                || s.source.path.is_empty()
                || s.source.path.contains('\0')
                || !contexts.contains_key(&s.context)
            {
                return Err(error("invalid span source/range/context edition"));
            }
            add_text(&mut text, &s.source.path)?;
        }
        let mut stack: Vec<_> = self.roots.iter().map(|t| (t, 0usize)).collect();
        let mut count = 0usize;
        while let Some((t, depth)) = stack.pop() {
            count = count
                .checked_add(1)
                .ok_or_else(|| error("token count overflow"))?;
            let exists = |key: &Key| {
                if spans.contains(key) {
                    Ok(())
                } else {
                    Err(error("token references unknown span"))
                }
            };
            match t {
                TokenTree::Group {
                    open,
                    close,
                    children,
                    ..
                } => {
                    exists(open)?;
                    exists(close)?;
                    stack.extend(children.iter().map(|t| (t, depth + 1)));
                }
                TokenTree::Ident { text: t, span, .. }
                | TokenTree::Literal {
                    token_text: t,
                    span,
                } => {
                    exists(span)?;
                    if t.is_empty() || t.contains('\0') {
                        return Err(error("empty or NUL token spelling"));
                    }
                    add_text(&mut text, t)?;
                }
                TokenTree::Punct { scalar, span, .. } => {
                    exists(span)?;
                    if !"=<>!~+-*/%^&|@.,;:#$?'".contains(*scalar) {
                        return Err(error("invalid Rust punctuation scalar"));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Key {
        let mut e = Encoder::new("lrr/v2/token-forest");
        e.u32(self.schema);
        e.count(self.contexts.len());
        for c in &self.contexts {
            c.encode(&mut e)
        }
        e.count(self.spans.len());
        for s in &self.spans {
            s.encode(&mut e)
        }
        e.count(self.roots.len());
        for t in &self.roots {
            t.encode(&mut e)
        }
        e.finish()
    }
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        codec::encode(self)
    }
    /// Count the exact schema2 wire bytes without allocating an output buffer.
    /// Limits can only reduce the fixed output budget.
    pub fn encoded_len_with_limit(&self, byte_limit: usize) -> Result<usize, WireError> {
        codec::encoded_len(self, byte_limit)
    }
    pub fn encoded_len(&self) -> Result<usize, WireError> {
        self.encoded_len_with_limit(MAX_WIRE)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        codec::decode(bytes)
    }
}
fn add_text(total: &mut usize, text: &str) -> Result<(), WireError> {
    *total = total
        .checked_add(text.len())
        .ok_or_else(|| error("token text overflow"))?;
    Ok(())
}

/// Version 2 binary wire. Keys refer to content-sorted owned tables; indices never identify
/// upstream arenas. Preorder child counts encode arbitrary group structure without JSON depth.
mod codec {
    use super::*;
    struct Writer {
        bytes: Option<Vec<u8>>,
        len: usize,
        limit: usize,
    }
    impl Writer {
        fn raw(&mut self, bytes: &[u8]) -> Result<(), WireError> {
            let needed = self
                .len
                .checked_add(bytes.len())
                .ok_or_else(|| limit("token wire byte limit"))?;
            if let Some(output) = &mut self.bytes {
                if needed > output.capacity() {
                    let capacity = output
                        .capacity()
                        .checked_mul(2)
                        .unwrap_or(self.limit)
                        .max(needed)
                        .min(self.limit);
                    output
                        .try_reserve_exact(capacity - output.len())
                        .map_err(|_| limit("token wire allocation failed"))?;
                }
                output.extend_from_slice(bytes);
            }
            self.len = needed;
            Ok(())
        }
        fn byte(&mut self, v: u8) -> Result<(), WireError> {
            self.raw(&[v])
        }
        fn u32(&mut self, v: u32) -> Result<(), WireError> {
            self.raw(&v.to_le_bytes())
        }
        fn count(&mut self, v: usize) -> Result<(), WireError> {
            self.u32(u32::try_from(v).map_err(|_| limit("token wire size overflow"))?)
        }
        fn key(&mut self, v: Key) -> Result<(), WireError> {
            self.raw(&v.0)
        }
        fn optional(&mut self, v: Option<Key>) -> Result<(), WireError> {
            match v {
                Some(v) => {
                    self.byte(1)?;
                    self.key(v)
                }
                None => self.byte(0),
            }
        }
        fn text(&mut self, v: &str) -> Result<(), WireError> {
            self.count(v.len())?;
            self.raw(v.as_bytes())
        }
    }
    pub(super) fn encode(f: &TokenForest) -> Result<Vec<u8>, WireError> {
        f.validate()?;
        let mut w = Writer {
            bytes: Some(Vec::new()),
            len: 0,
            limit: MAX_WIRE,
        };
        write(f, &mut w)?;
        w.bytes
            .ok_or_else(|| error("token writer missing output buffer"))
    }
    pub(super) fn encoded_len(f: &TokenForest, byte_limit: usize) -> Result<usize, WireError> {
        f.validate()?;
        let mut w = Writer {
            bytes: None,
            len: 0,
            limit: byte_limit,
        };
        write(f, &mut w)?;
        Ok(w.len)
    }
    fn write_context(c: &ContextRecord, w: &mut Writer) -> Result<(), WireError> {
        w.optional(c.parent)?;
        w.optional(c.expansion)?;
        w.byte(c.transparency.tag())?;
        w.key(c.defining_unit)?;
        w.byte(c.edition.tag())?;
        Ok(())
    }
    pub(super) fn context_size(c: &ContextRecord) -> Result<usize, WireError> {
        let mut writer = Writer {
            bytes: None,
            len: 0,
            limit: MAX_WIRE,
        };
        write_context(c, &mut writer)?;
        Ok(writer.len)
    }
    fn write_span(s: &SpanRecord, context_index: usize, w: &mut Writer) -> Result<(), WireError> {
        w.key(s.source.unit)?;
        w.key(s.source.content)?;
        w.text(&s.source.path)?;
        w.u32(s.start)?;
        w.u32(s.end)?;
        w.count(context_index)?;
        w.byte(s.edition.tag())?;
        w.key(s.origin)?;
        Ok(())
    }
    pub(super) fn record_sizes(
        c: &ContextRecord,
        s: &SpanRecord,
    ) -> Result<(usize, usize), WireError> {
        let mut w = Writer {
            bytes: None,
            len: 0,
            limit: MAX_WIRE,
        };
        write_context(c, &mut w)?;
        let context_bytes = w.len;
        w.len = 0;
        write_span(s, 0, &mut w)?;
        Ok((context_bytes, w.len))
    }
    fn write(f: &TokenForest, w: &mut Writer) -> Result<(), WireError> {
        w.raw(b"LRRT")?;
        w.u32(2)?;
        w.count(f.contexts.len())?;
        for c in &f.contexts {
            write_context(c, w)?;
        }
        let contexts: BTreeMap<_, _> = f
            .contexts
            .iter()
            .enumerate()
            .map(|(i, c)| (c.key(), i))
            .collect();
        w.count(f.spans.len())?;
        for s in &f.spans {
            let index = *contexts
                .get(&s.context)
                .ok_or_else(|| error("unregistered context"))?;
            write_span(s, index, w)?;
        }
        let spans: BTreeMap<_, _> = f
            .spans
            .iter()
            .enumerate()
            .map(|(i, s)| (s.key(), i))
            .collect();
        w.count(f.roots.len())?;
        let mut stack: Vec<_> = f.roots.iter().rev().collect();
        while let Some(t) = stack.pop() {
            let index = |key: &Key| {
                spans
                    .get(key)
                    .copied()
                    .ok_or_else(|| error("unregistered span"))
            };
            match t {
                TokenTree::Group {
                    delimiter,
                    open,
                    close,
                    children,
                } => {
                    w.byte(0)?;
                    w.byte(delimiter.tag())?;
                    w.count(index(open)?)?;
                    w.count(index(close)?)?;
                    w.count(children.len())?;
                    stack.extend(children.iter().rev());
                }
                TokenTree::Ident { text, raw, span } => {
                    w.byte(1)?;
                    w.text(text)?;
                    w.byte(u8::from(*raw))?;
                    w.count(index(span)?)?;
                }
                TokenTree::Punct {
                    scalar,
                    spacing,
                    span,
                } => {
                    w.byte(2)?;
                    w.u32(u32::from(*scalar))?;
                    w.byte(spacing.tag())?;
                    w.count(index(span)?)?;
                }
                TokenTree::Literal { token_text, span } => {
                    w.byte(3)?;
                    w.text(token_text)?;
                    w.count(index(span)?)?;
                }
            }
        }
        Ok(())
    }
    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
        text: usize,
        tokens: usize,
        pending: usize,
    }
    impl<'a> Reader<'a> {
        fn raw(&mut self, n: usize) -> Result<&'a [u8], WireError> {
            let end = self
                .at
                .checked_add(n)
                .ok_or_else(|| error("token wire offset overflow"))?;
            let value = self
                .bytes
                .get(self.at..end)
                .ok_or_else(|| error("truncated token wire"))?;
            self.at = end;
            Ok(value)
        }
        fn byte(&mut self) -> Result<u8, WireError> {
            Ok(self.raw(1)?[0])
        }
        fn u32(&mut self) -> Result<u32, WireError> {
            Ok(u32::from_le_bytes(
                self.raw(4)?
                    .try_into()
                    .map_err(|_| error("invalid integer size"))?,
            ))
        }
        fn key(&mut self) -> Result<Key, WireError> {
            Ok(Key(self
                .raw(32)?
                .try_into()
                .map_err(|_| error("invalid key size"))?))
        }
        fn optional(&mut self) -> Result<Option<Key>, WireError> {
            match self.byte()? {
                0 => Ok(None),
                1 => Ok(Some(self.key()?)),
                _ => Err(error("invalid optional-key tag")),
            }
        }
        fn count(&mut self, min_bytes: usize) -> Result<usize, WireError> {
            let n = usize::try_from(self.u32()?).map_err(|_| limit("token wire count overflow"))?;
            if n.checked_mul(min_bytes)
                .is_none_or(|v| v > self.bytes.len() - self.at)
            {
                return Err(error("declared count exceeds remaining wire"));
            }
            Ok(n)
        }
        fn text(&mut self) -> Result<String, WireError> {
            let n =
                usize::try_from(self.u32()?).map_err(|_| limit("token text length overflow"))?;
            self.text = self
                .text
                .checked_add(n)
                .ok_or_else(|| limit("token text size overflow"))?;
            let source =
                std::str::from_utf8(self.raw(n)?).map_err(|_| error("invalid token text UTF8"))?;
            let mut text = String::new();
            text.try_reserve_exact(n)
                .map_err(|_| limit("token text allocation failed"))?;
            text.push_str(source);
            Ok(text)
        }
        fn edition(&mut self) -> Result<Edition, WireError> {
            match self.byte()? {
                0 => Ok(Edition::E2015),
                1 => Ok(Edition::E2018),
                2 => Ok(Edition::E2021),
                3 => Ok(Edition::E2024),
                _ => Err(error("invalid edition tag")),
            }
        }
        fn span(&mut self, spans: &[Key]) -> Result<Key, WireError> {
            let i = usize::try_from(self.u32()?).map_err(|_| error("invalid span index"))?;
            spans
                .get(i)
                .copied()
                .ok_or_else(|| error("unknown span index"))
        }
        fn trees(
            &mut self,
            n: usize,
            depth: usize,
            spans: &[Key],
        ) -> Result<Vec<TokenTree>, WireError> {
            if n == 0 {
                return Ok(Vec::new());
            }
            if n.checked_mul(7)
                .is_none_or(|v| v > self.bytes.len() - self.at)
            {
                return Err(error("token count exceeds remaining wire"));
            }
            self.pending = self
                .pending
                .checked_add(n)
                .ok_or_else(|| limit("pending token count overflow"))?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(n)
                .map_err(|_| limit("token allocation failed"))?;
            for _ in 0..n {
                self.pending = self
                    .pending
                    .checked_sub(1)
                    .ok_or_else(|| error("pending token count underflow"))?;
                self.tokens = self
                    .tokens
                    .checked_add(1)
                    .ok_or_else(|| limit("token count overflow"))?;
                let token = match self.byte()? {
                    0 => {
                        let delimiter = match self.byte()? {
                            0 => Delimiter::Parenthesis,
                            1 => Delimiter::Brace,
                            2 => Delimiter::Bracket,
                            3 => Delimiter::Invisible,
                            _ => return Err(error("invalid delimiter tag")),
                        };
                        let open = self.span(spans)?;
                        let close = self.span(spans)?;
                        let count = self.count(7)?;
                        let children = self.trees(count, depth + 1, spans)?;
                        TokenTree::Group {
                            delimiter,
                            open,
                            close,
                            children,
                        }
                    }
                    1 => {
                        let text = self.text()?;
                        let raw = match self.byte()? {
                            0 => false,
                            1 => true,
                            _ => return Err(error("invalid raw-identifier flag")),
                        };
                        let span = self.span(spans)?;
                        TokenTree::Ident { text, raw, span }
                    }
                    2 => {
                        let scalar = char::from_u32(self.u32()?)
                            .ok_or_else(|| error("invalid punctuation scalar"))?;
                        let spacing = match self.byte()? {
                            0 => Spacing::Alone,
                            1 => Spacing::Joint,
                            2 => Spacing::JointHidden,
                            _ => return Err(error("invalid punctuation spacing")),
                        };
                        let span = self.span(spans)?;
                        TokenTree::Punct {
                            scalar,
                            spacing,
                            span,
                        }
                    }
                    3 => {
                        let token_text = self.text()?;
                        let span = self.span(spans)?;
                        TokenTree::Literal { token_text, span }
                    }
                    _ => return Err(error("invalid token tag")),
                };
                output.push(token);
            }
            Ok(output)
        }
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<TokenForest, WireError> {
        let mut r = Reader {
            bytes,
            at: 0,
            text: 0,
            tokens: 0,
            pending: 0,
        };
        if r.raw(4)? != b"LRRT" || r.u32()? != 2 {
            return Err(error("unsupported token wire schema"));
        }
        let n = r.count(36)?;
        let mut contexts = Vec::new();
        contexts
            .try_reserve_exact(n)
            .map_err(|_| limit("context allocation failed"))?;
        for _ in 0..n {
            let parent = r.optional()?;
            let expansion = r.optional()?;
            let transparency = match r.byte()? {
                0 => Transparency::Opaque,
                1 => Transparency::SemiTransparent,
                2 => Transparency::Transparent,
                _ => return Err(error("invalid transparency tag")),
            };
            let defining_unit = r.key()?;
            let edition = r.edition()?;
            contexts.push(ContextRecord {
                parent,
                expansion,
                transparency,
                defining_unit,
                edition,
            });
        }
        let n = r.count(114)?;
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(n)
            .map_err(|_| limit("span allocation failed"))?;
        for _ in 0..n {
            let unit = r.key()?;
            let content = r.key()?;
            let path = r.text()?;
            let start = r.u32()?;
            let end = r.u32()?;
            let index = usize::try_from(r.u32()?).map_err(|_| error("invalid context index"))?;
            let context = contexts
                .get(index)
                .ok_or_else(|| error("unknown context index"))?
                .key();
            let edition = r.edition()?;
            let origin = r.key()?;
            spans.push(SpanRecord {
                source: SourceIdentity {
                    unit,
                    content,
                    path,
                },
                start,
                end,
                context,
                edition,
                origin,
            });
        }
        let n = r.count(7)?;
        let span_keys: Vec<Key> = spans.iter().map(SpanRecord::key).collect();
        let roots = r.trees(n, 0, &span_keys)?;
        if r.at != bytes.len() {
            return Err(error("trailing token wire bytes"));
        }
        let cache = SpanKeyIndex::default();
        let _ = cache.0.set(span_keys);
        let f = TokenForest {
            schema: 2,
            roots,
            contexts,
            spans,
            span_keys: cache,
        };
        f.validate()?;
        Ok(f)
    }
}

/// Schema2 metadata costs for private backend admission. Field widths are written by
/// the same codec used for encoding; neither record allocates a wire buffer.
pub fn record_wire_sizes(
    context: &ContextRecord,
    span: &SpanRecord,
) -> Result<(usize, usize), WireError> {
    if span.context != context.key() {
        return Err(error("record context differs"));
    }
    codec::record_sizes(context, span)
}
/// Count the canonical context field writer without allocating wire bytes.
pub fn context_wire_size(context: &ContextRecord) -> Result<usize, WireError> {
    codec::context_size(context)
}
