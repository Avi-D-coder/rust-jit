//! rustc-faithful token expanders for `#[rustc_builtin_macro]` function-like
//! macros whose library definitions have placeholder bodies.
//!
//! These are compiler expansion hooks inside the pinned adapter (E-R5-005): each
//! expander reads the invocation's owned tokens and returns ordinary token output
//! in the same owned wire format as declarative expansion
//! ([`crate::declarative::Expansion::tokens`]). No guest code is evaluated; the
//! caller attaches the output through its ordinary expansion-output path, where it
//! is parsed, indexed, resolved and executed like any other macro output.
//!
//! The behaviour follows the pinned rustc checkout
//! (`rustc/rust/compiler/rustc_builtin_macros/src/`):
//! `assert.rs`, `edition_panic.rs`, `format.rs`, `source_util.rs`, `cfg.rs`,
//! `compile_error.rs`, with printing from `rustc_ast_pretty`.
//!
//! # Spans and hygiene
//!
//! rustc builds builtin output with two span constructors applied to the
//! invocation span `sp`:
//! - `cx.with_def_site_ctxt(sp)`: an opaque mark of this expansion, used for
//!   library-owned tokens (`$crate`, the remaining library path segments,
//!   `builtin # format_args`, `stringify!`/`module_path!`/`cfg!` results);
//! - `cx.with_call_site_ctxt(sp)`: a transparent mark, used for user-visible
//!   syntax that must resolve at the caller (`if !(..) { .. }` of `assert!`, the
//!   unqualified pre-2021 `panic` path, macro-call delimiters).
//!
//! Here each generated token's span record keeps the invocation location record
//! (`call_site`) and replaces only its resolution context, exactly as the
//! declarative transcriber keeps a template location and replaces its context:
//! - def-site: [`ContextRecord`] opaque mark over the defining unit's root, with
//!   `expansion = site.expansion`, `defining_unit = site.defining_unit`. A `$crate`
//!   identifier therefore carries the defining unit's authority, which is how the
//!   declarative transcriber's `$crate` tokens are resolved;
//! - call-site: transparent mark applied to the caller's own root through the
//!   pinned `apply_mark_with_call_site`, so name resolution normalizes back to the
//!   caller's context.
//!
//! `$crate` is an [`TokenTree::Ident`] with text `"$crate"`, as produced by the
//! pinned `macro_rules` transcriber. Paths are emitted as tokens, e.g.
//! `$crate :: panicking :: panic`, with `::` as a `Joint` `:` followed by an
//! `Alone` `:`. User tokens (conditions, messages, format arguments) keep their
//! original span keys unchanged. Tokens for which rustc uses `DUMMY_SP` (the
//! non-first library path segments, the assertion message literal) use the
//! def-site span at the invocation location.
//!
//! The output root is one invisible group whose delimiters use the unmarked
//! `call_site` span, matching the declarative expander's output root.
//!
//! # Known differences from rustc
//!
//! - `line!`/`column!`/`file!` literals carry the invocation span; rustc uses the
//!   `expansion_cause()` span (values are supplied by the caller and are exact).
//! - `stringify!` recovers rustc lexer spacing from span adjacency; a captured
//!   source token followed by tokens other than its original neighbour is printed
//!   as `Spacing::Alone` (see `tts.rs`).
//! - `assert!` message printing reports `Unsupported` for expression attributes,
//!   item statements, `box` patterns and `builtin #` syntax; the unstable
//!   `generic_assert` expansion is not implemented.
//! - `cfg!` does not report feature-gated cfg names, check-cfg lints, or
//!   `version(..)` (unsupported).
//! - `format_args!` template/argument validation is left to the consumer of the
//!   `FORMAT_ARGS_EXPR` (rustc diagnoses it during expansion).
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{
        ContextRecord, Delimiter, Edition, Key, Spacing, TokenForest, TokenTree, Transparency,
    },
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

mod ast;
mod cfg;
mod derive;
mod pp;
mod print;
mod tts;

pub use derive::{BuiltinDeriveKind, DeriveContainerFacts, expand_builtin_derive};

#[cfg(test)]
mod derive_tests;
#[cfg(test)]
mod tests;

/// Builtin function-like macros known to the pinned rustc (`register_bang!` in
/// `rustc_builtin_macros/src/lib.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum BuiltinKind {
    Assert,
    CorePanic,
    StdPanic,
    Unreachable,
    FormatArgs,
    ConstFormatArgs,
    FormatArgsNl,
    Line,
    Column,
    File,
    ModulePath,
    Stringify,
    Cfg,
    CompileError,
    Concat,
    ConcatBytes,
    Env,
    OptionEnv,
    Include,
    IncludeBytes,
    IncludeStr,
    Asm,
    GlobalAsm,
    NakedAsm,
    CfgSelect,
    LogSyntax,
    TraceMacros,
    PatternType,
    Iter,
    Gca,
    TestBinderConstraints,
    ViewType,
}

/// Which component implements a builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BuiltinDisposition {
    /// [`expand_builtin`] produces the expansion.
    Adapter,
    /// Implemented by LRR's existing compile-input paths (`concat!`, `env!`,
    /// `include*!`); [`expand_builtin`] rejects these.
    HandledByLrr,
    /// rustc builtin not implemented by this adapter (explicit unsupported).
    Unsupported,
}

const BUILTINS: &[(&str, BuiltinKind)] = &[
    ("asm", BuiltinKind::Asm),
    ("assert", BuiltinKind::Assert),
    ("cfg", BuiltinKind::Cfg),
    ("cfg_select", BuiltinKind::CfgSelect),
    ("column", BuiltinKind::Column),
    ("compile_error", BuiltinKind::CompileError),
    ("concat", BuiltinKind::Concat),
    ("concat_bytes", BuiltinKind::ConcatBytes),
    ("const_format_args", BuiltinKind::ConstFormatArgs),
    ("core_panic", BuiltinKind::CorePanic),
    ("env", BuiltinKind::Env),
    ("file", BuiltinKind::File),
    ("format_args", BuiltinKind::FormatArgs),
    ("format_args_nl", BuiltinKind::FormatArgsNl),
    ("gca", BuiltinKind::Gca),
    ("global_asm", BuiltinKind::GlobalAsm),
    ("include", BuiltinKind::Include),
    ("include_bytes", BuiltinKind::IncludeBytes),
    ("include_str", BuiltinKind::IncludeStr),
    ("iter", BuiltinKind::Iter),
    ("line", BuiltinKind::Line),
    ("log_syntax", BuiltinKind::LogSyntax),
    ("module_path", BuiltinKind::ModulePath),
    ("naked_asm", BuiltinKind::NakedAsm),
    ("option_env", BuiltinKind::OptionEnv),
    ("pattern_type", BuiltinKind::PatternType),
    ("std_panic", BuiltinKind::StdPanic),
    ("stringify", BuiltinKind::Stringify),
    (
        "test_binder_constraints",
        BuiltinKind::TestBinderConstraints,
    ),
    ("trace_macros", BuiltinKind::TraceMacros),
    ("unreachable", BuiltinKind::Unreachable),
    ("view_type", BuiltinKind::ViewType),
];

/// Classify a builtin by its rustc builtin name: the argument of
/// `#[rustc_builtin_macro(name)]` when present (e.g. `core_panic`, `std_panic`,
/// `unreachable`), otherwise the macro item's own name (e.g. `assert`, `line`).
pub fn builtin_kind(name: &str) -> Option<BuiltinKind> {
    BUILTINS
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .map(|(_, kind)| *kind)
}

impl BuiltinKind {
    pub fn name(self) -> &'static str {
        BUILTINS
            .iter()
            .find(|(_, kind)| *kind == self)
            .map(|(name, _)| *name)
            .unwrap_or("")
    }
    pub fn disposition(self) -> BuiltinDisposition {
        use BuiltinKind::*;
        match self {
            Assert | CorePanic | StdPanic | Unreachable | FormatArgs | ConstFormatArgs
            | FormatArgsNl | Line | Column | File | ModulePath | Stringify | Cfg | CompileError => {
                BuiltinDisposition::Adapter
            }
            Concat | ConcatBytes | Env | OptionEnv | Include | IncludeBytes | IncludeStr => {
                BuiltinDisposition::HandledByLrr
            }
            Asm
            | GlobalAsm
            | NakedAsm
            | CfgSelect
            | LogSyntax
            | TraceMacros
            | PatternType
            | Iter
            | Gca
            | TestBinderConstraints
            | ViewType => BuiltinDisposition::Unsupported,
        }
    }
}

/// Expansion identity and definition authority, owned by the source index
/// (same roles as [`crate::declarative::DeclarativeSite`]).
#[derive(Debug, Clone, Copy)]
pub struct BuiltinSite {
    /// Stable identity of this expansion (the hygiene mark's expansion key).
    pub expansion: Key,
    /// Unit that declares the `#[rustc_builtin_macro]` item (`core` for
    /// `assert!`/`core_panic`/`format_args!`, `std` for `std_panic`); this is the
    /// authority of emitted `$crate` tokens.
    pub defining_unit: Key,
    /// Edition of the defining unit (rustc `ExpnData::edition` of the builtin).
    pub definition_edition: Edition,
}

/// Facts about the invocation that only the caller's source index knows.
/// Each method is called only by the builtins that need it.
pub trait BuiltinFacts {
    /// rustc `use_panic_2021(sp)` (`edition_panic.rs`): starting from the
    /// invocation span's syntax context, skip every enclosing expansion whose
    /// macro definition has `#[allow_internal_unstable(edition_panic)]` (e.g.
    /// `assert!`, `debug_assert!`, `panic!`, `unreachable!` in core/std) by moving
    /// to that expansion's call site; return the edition of the first expansion
    /// that lacks it (its macro definition's edition), or the crate edition when
    /// the root is reached. Used by `assert!`, `panic!` and `unreachable!`.
    fn panic_edition(&self) -> Result<Edition, AdapterError>;
    /// rustc `line!`: 1-based line of `cx.expansion_cause()`'s start, i.e. the
    /// call site of the outermost macro expansion in this invocation's chain
    /// (walking `ExpnData::call_site` until the root, stopping at an `include!`
    /// expansion). For a direct `line!()` in source, the invocation itself.
    fn line(&self) -> Result<u32, AdapterError>;
    /// rustc `column!`: 1-based column, counted in `char`s from the line start,
    /// of the same `expansion_cause()` position.
    fn column(&self) -> Result<u32, AdapterError>;
    /// rustc `file!`: the (remapped) display name of the source file containing
    /// the `expansion_cause()` position, as passed to the compiler.
    fn file(&self) -> Result<String, AdapterError>;
    /// rustc `module_path!`: `join_path_idents(cx.current_expansion.module.mod_path)`,
    /// the crate name followed by the enclosing `mod` names, joined with `::`.
    fn module_path(&self) -> Result<String, AdapterError>;
    /// rustc `sess.config.contains(&(name, value))` for `cfg!`.
    fn cfg(&self, name: &str, value: Option<&str>) -> Result<bool, AdapterError>;
    /// rustc `SyntaxContext::dollar_crate_name` for a `$crate` whose definition
    /// authority is `defining_unit`: `"crate"` for the current crate, otherwise
    /// the crate's name (printed as `::name`). Only needed when an `assert!`
    /// condition contains `$crate` (rustc prints AST paths with conversion).
    fn dollar_crate_name(&self, defining_unit: Key) -> Result<String, AdapterError>;
}

/// Plain [`BuiltinFacts`] values for callers that compute them eagerly.
#[derive(Debug, Clone)]
pub struct BuiltinCallSite {
    /// See [`BuiltinFacts::panic_edition`].
    pub edition: Edition,
    /// See [`BuiltinFacts::line`] (1-based).
    pub line: u32,
    /// See [`BuiltinFacts::column`] (1-based, chars).
    pub column: u32,
    pub file: String,
    pub module_path: String,
    /// Active configuration (`--cfg name` is `(name, None)`).
    pub cfg: BTreeSet<(String, Option<String>)>,
    /// `$crate` display names by defining unit.
    pub crate_names: BTreeMap<Key, String>,
}

impl BuiltinFacts for BuiltinCallSite {
    fn panic_edition(&self) -> Result<Edition, AdapterError> {
        Ok(self.edition)
    }
    fn line(&self) -> Result<u32, AdapterError> {
        Ok(self.line)
    }
    fn column(&self) -> Result<u32, AdapterError> {
        Ok(self.column)
    }
    fn file(&self) -> Result<String, AdapterError> {
        Ok(self.file.clone())
    }
    fn module_path(&self) -> Result<String, AdapterError> {
        Ok(self.module_path.clone())
    }
    fn cfg(&self, name: &str, value: Option<&str>) -> Result<bool, AdapterError> {
        Ok(self
            .cfg
            .contains(&(name.to_owned(), value.map(str::to_owned))))
    }
    fn dollar_crate_name(&self, defining_unit: Key) -> Result<String, AdapterError> {
        self.crate_names
            .get(&defining_unit)
            .cloned()
            .ok_or_else(|| fault("no crate name supplied for a `$crate` defining unit"))
    }
}

/// A successful builtin expansion.
#[derive(Debug, Clone)]
pub struct BuiltinExpansion {
    /// Owned output tokens: one invisible root group, as declarative output.
    pub tokens: TokenForest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BuiltinFailureKind {
    /// `compile_error!` reached expansion: compilation must fail with `message`.
    CompileError,
    /// rustc rejects the invocation's syntax (the message is rustc's diagnostic).
    Syntax,
    /// rustc would first eagerly expand a macro call in the argument
    /// (`compile_error!(concat!(..))`); expand it and invoke again.
    RequiresEagerExpansion,
    /// Valid input whose rustc behaviour this adapter does not reproduce yet.
    Unsupported,
}

/// Expansion diagnostic (the invocation does not produce tokens).
#[derive(Debug, Clone, Serialize)]
pub struct BuiltinFailure {
    pub kind: BuiltinFailureKind,
    pub message: String,
    pub span: Option<Key>,
}

impl std::fmt::Display for BuiltinFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for BuiltinFailure {}

fn fault(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-BUILTIN",
        message: message.into(),
    }
}

fn failure(
    kind: BuiltinFailureKind,
    message: impl Into<String>,
    span: Option<Key>,
) -> BuiltinFailure {
    BuiltinFailure {
        kind,
        message: message.into(),
        span,
    }
}

type Outcome = Result<Result<BuiltinExpansion, BuiltinFailure>, AdapterError>;

/// Expand one builtin invocation.
///
/// `input` is the invocation's argument group exactly as passed to declarative
/// expansion (one root group; its delimiter is the call's delimiter). `call_site`
/// must be a span key present in `input` (the call's span record, as for
/// [`crate::declarative::DeclarativeMacro::expand_hygienic_with_limits`]); its
/// location and context are the rustc invocation span `sp`.
///
/// The outer `Err` is an adapter/wire fault; the inner `Err` is a rustc
/// expansion diagnostic.
pub fn expand_builtin(
    kind: BuiltinKind,
    input: &TokenForest,
    call_site: Key,
    site: &BuiltinSite,
    facts: &dyn BuiltinFacts,
) -> Outcome {
    input.validate()?;
    let [
        TokenTree::Group {
            open,
            close,
            children,
            ..
        },
    ] = input.roots()
    else {
        return Err(fault("builtin input requires one explicit token group"));
    };
    let mut out = Output::new(input, call_site, *site)?;
    let args = Args {
        open: *open,
        close: *close,
        children,
    };
    use BuiltinKind::*;
    match kind {
        Assert => expand_assert(&mut out, &args, facts),
        CorePanic | StdPanic => {
            let mac = if at_least_2021(facts.panic_edition()?) {
                "panic_2021"
            } else {
                "panic_2015"
            };
            expand_edition_panic(&mut out, &args, mac)
        }
        Unreachable => {
            let mac = if at_least_2021(facts.panic_edition()?) {
                "unreachable_2021"
            } else {
                "unreachable_2015"
            };
            expand_edition_panic(&mut out, &args, mac)
        }
        FormatArgs | ConstFormatArgs => expand_format_args(&mut out, &args, false),
        FormatArgsNl => expand_format_args(&mut out, &args, true),
        Line | Column => {
            if let Some(failure) = check_zero_tts(&args, kind.name()) {
                return Ok(Err(failure));
            }
            let value = if kind == Line {
                facts.line()?
            } else {
                facts.column()?
            };
            let literal = TokenTree::Literal {
                token_text: format!("{value}u32"),
                span: call_site,
            };
            out.finish(vec![literal])
        }
        File => {
            if let Some(failure) = check_zero_tts(&args, "file") {
                return Ok(Err(failure));
            }
            let literal = TokenTree::Literal {
                token_text: string_literal(&facts.file()?),
                span: call_site,
            };
            out.finish(vec![literal])
        }
        ModulePath => {
            if let Some(failure) = check_zero_tts(&args, "module_path") {
                return Ok(Err(failure));
            }
            let span = out.def_site()?;
            let literal = TokenTree::Literal {
                token_text: string_literal(&facts.module_path()?),
                span,
            };
            out.finish(vec![literal])
        }
        Stringify => {
            let index = tts::SpanIndex::new(input);
            let text = tts::tts_to_string(&tts::build(children, &index, None)?);
            let span = out.def_site()?;
            out.finish(vec![TokenTree::Literal {
                token_text: string_literal(&text),
                span,
            }])
        }
        Cfg => {
            let entry = match cfg::parse(children) {
                Ok(entry) => entry,
                Err((message, span)) => {
                    return Ok(Err(failure(
                        BuiltinFailureKind::Syntax,
                        message,
                        Some(span.unwrap_or(call_site)),
                    )));
                }
            };
            let value = cfg::eval(&entry, &mut |name, value| facts.cfg(name, value))?;
            let span = out.def_site()?;
            out.finish(vec![TokenTree::Ident {
                text: if value { "true" } else { "false" }.into(),
                raw: false,
                span,
            }])
        }
        CompileError => Ok(Err(compile_error(children, call_site))),
        Concat | ConcatBytes | Env | OptionEnv | Include | IncludeBytes | IncludeStr => Err(fault(
            format!("`{}!` is expanded by LRR's compile-input path", kind.name()),
        )),
        Asm
        | GlobalAsm
        | NakedAsm
        | CfgSelect
        | LogSyntax
        | TraceMacros
        | PatternType
        | Iter
        | Gca
        | TestBinderConstraints
        | ViewType => Ok(Err(failure(
            BuiltinFailureKind::Unsupported,
            format!(
                "builtin `{}!` is not implemented by the adapter",
                kind.name()
            ),
            Some(call_site),
        ))),
    }
}

struct Args<'a> {
    open: Key,
    close: Key,
    children: &'a [TokenTree],
}

fn at_least_2021(edition: Edition) -> bool {
    matches!(edition, Edition::E2021 | Edition::E2024)
}

/// rustc `escape_string_symbol` (identical to `str::escape_default`) in quotes.
fn string_literal(value: &str) -> String {
    format!("\"{}\"", value.escape_default())
}

/// rustc `check_zero_tts`.
fn check_zero_tts(args: &Args<'_>, name: &str) -> Option<BuiltinFailure> {
    (!args.children.is_empty()).then(|| {
        failure(
            BuiltinFailureKind::Syntax,
            format!("`{name}!` takes no arguments"),
            Some(args.open),
        )
    })
}

/// Output token construction with rustc def-site/call-site spans.
struct Output {
    store: SpanStore,
    call_site: Key,
    site: BuiltinSite,
    def_site: Option<Key>,
    call_site_marked: Option<Key>,
}

impl Output {
    fn new(input: &TokenForest, call_site: Key, site: BuiltinSite) -> Result<Self, AdapterError> {
        let mut store = SpanStore::default();
        store.import(input)?;
        if store.record(call_site).is_none() {
            return Err(fault("builtin call site is not a span of the input"));
        }
        Ok(Self {
            store,
            call_site,
            site,
            def_site: None,
            call_site_marked: None,
        })
    }

    /// rustc `cx.with_def_site_ctxt(sp)`.
    fn def_site(&mut self) -> Result<Key, AdapterError> {
        if let Some(key) = self.def_site {
            return Ok(key);
        }
        let key = self
            .store
            .builtin_span(self.call_site, self.site, Transparency::Opaque)?;
        self.def_site = Some(key);
        Ok(key)
    }

    /// rustc `cx.with_call_site_ctxt(sp)`.
    fn call_site_ctxt(&mut self) -> Result<Key, AdapterError> {
        if let Some(key) = self.call_site_marked {
            return Ok(key);
        }
        let key = self
            .store
            .builtin_span(self.call_site, self.site, Transparency::Transparent)?;
        self.call_site_marked = Some(key);
        Ok(key)
    }

    fn finish(&self, children: Vec<TokenTree>) -> Outcome {
        let root = TokenTree::Group {
            delimiter: Delimiter::Invisible,
            open: self.call_site,
            close: self.call_site,
            children,
        };
        let mut used = BTreeSet::new();
        collect_spans(&root, &mut used);
        let tokens = self.store.export_records(vec![root], used)?;
        Ok(Ok(BuiltinExpansion { tokens }))
    }

    fn input_contexts(&self) -> &SpanStore {
        &self.store
    }
}

fn collect_spans(tree: &TokenTree, used: &mut BTreeSet<Key>) {
    match tree {
        TokenTree::Group {
            open,
            close,
            children,
            ..
        } => {
            used.insert(*open);
            used.insert(*close);
            for child in children {
                collect_spans(child, used);
            }
        }
        TokenTree::Ident { span, .. }
        | TokenTree::Punct { span, .. }
        | TokenTree::Literal { span, .. } => {
            used.insert(*span);
        }
    }
}

fn ident(text: &str, span: Key) -> TokenTree {
    TokenTree::Ident {
        text: text.into(),
        raw: false,
        span,
    }
}

fn punct(scalar: char, spacing: Spacing, span: Key) -> TokenTree {
    TokenTree::Punct {
        scalar,
        spacing,
        span,
    }
}

fn group(delimiter: Delimiter, span: Key, children: Vec<TokenTree>) -> TokenTree {
    TokenTree::Group {
        delimiter,
        open: span,
        close: span,
        children,
    }
}

/// `$crate::a::b` with the def-site span (rustc `cx.std_path`).
fn std_path(def_site: Key, components: &[&str]) -> Vec<TokenTree> {
    let mut path = vec![ident("$crate", def_site)];
    for component in components {
        path.push(punct(':', Spacing::Joint, def_site));
        path.push(punct(':', Spacing::Alone, def_site));
        path.push(ident(component, def_site));
    }
    path
}

/// rustc `edition_panic::expand`: `$crate::panic::<mac>!(<input>)`.
fn expand_edition_panic(out: &mut Output, args: &Args<'_>, mac: &str) -> Outcome {
    let def_site = out.def_site()?;
    let call = out.call_site_ctxt()?;
    let mut tokens = std_path(def_site, &["panic", mac]);
    tokens.push(punct('!', Spacing::Alone, call));
    tokens.push(group(Delimiter::Parenthesis, call, args.children.to_vec()));
    out.finish(tokens)
}

/// Count of leading top-level trees forming one expression (rustc `parse_expr`
/// via the pinned prefix expression parser), or a syntax failure.
fn expression_prefix(
    out: &mut Output,
    trees: &[TokenTree],
) -> Result<Result<usize, BuiltinFailure>, AdapterError> {
    if trees.is_empty() {
        return Ok(Ok(0));
    }
    let store = out.input_contexts();
    let open = store.span(out.call_site)?;
    let forest_root = TokenTree::Group {
        delimiter: Delimiter::Invisible,
        open: out.call_site,
        close: out.call_site,
        children: trees.to_vec(),
    };
    let mut used = BTreeSet::new();
    collect_spans(&forest_root, &mut used);
    let forest = store.export_records(vec![forest_root], used)?;
    let mut scratch = store.clone();
    let tree = scratch.import(&forest)?;
    let mut iter = tree.iter();
    let result = mbe::expect_fragment(
        scratch.database(),
        &mut iter,
        parser::PrefixEntryPoint::Expr,
        tt::DelimSpan::from_single(open),
    );
    if let Some(error) = result.err {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            error.to_string(),
            trees.first().map(first_span),
        )));
    }
    let consumed = result.value.iter().count();
    if consumed == 0 {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            "expected expression",
            trees.first().map(first_span),
        )));
    }
    Ok(Ok(consumed))
}

fn first_span(tree: &TokenTree) -> Key {
    match tree {
        TokenTree::Group { open, .. } => *open,
        TokenTree::Ident { span, .. }
        | TokenTree::Punct { span, .. }
        | TokenTree::Literal { span, .. } => *span,
    }
}

fn is_str_literal(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Literal { token_text, .. } if tts::unquote_str(token_text).is_some())
}

/// rustc `assert::expand_assert` (without the unstable `generic_assert`).
fn expand_assert(out: &mut Output, args: &Args<'_>, facts: &dyn BuiltinFacts) -> Outcome {
    if args.children.is_empty() {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            "macro requires a boolean expression as an argument",
            Some(out.call_site),
        )));
    }
    let consumed = match expression_prefix(out, args.children)? {
        Ok(n) => n,
        Err(failure) => return Ok(Err(failure)),
    };
    let (cond, rest) = args.children.split_at(consumed);
    let custom_message = match rest.first() {
        None => None,
        Some(TokenTree::Punct {
            scalar: ';', span, ..
        }) => {
            return Ok(Err(failure(
                BuiltinFailureKind::Syntax,
                "macro requires an expression as an argument",
                Some(*span),
            )));
        }
        Some(token) if is_str_literal(token) => {
            return Ok(Err(failure(
                BuiltinFailureKind::Syntax,
                "unexpected string literal",
                Some(first_span(token)),
            )));
        }
        Some(TokenTree::Punct { scalar: ',', .. }) => {
            let tokens = &rest[1..];
            (!tokens.is_empty()).then_some(tokens)
        }
        Some(token) => {
            return Ok(Err(failure(
                BuiltinFailureKind::Syntax,
                "unexpected token after assertion condition",
                Some(first_span(token)),
            )));
        }
    };
    let def_site = out.def_site()?;
    let call = out.call_site_ctxt()?;
    let panic_2021 = at_least_2021(facts.panic_edition()?);
    let then = match custom_message {
        Some(tokens) => {
            // `panic_path()`: 2021+ `$crate::panic::panic_2021`, otherwise the
            // unqualified call-site `panic` (std's or core's, whichever is in scope).
            let mut call_tokens = if panic_2021 {
                std_path(def_site, &["panic", "panic_2021"])
            } else {
                vec![ident("panic", call)]
            };
            call_tokens.push(punct('!', Spacing::Alone, call));
            call_tokens.push(group(Delimiter::Parenthesis, call, tokens.to_vec()));
            call_tokens
        }
        None => {
            let message = match assertion_message(out, cond, facts)? {
                Ok(message) => message,
                Err(failure) => return Ok(Err(failure)),
            };
            let mut call_tokens = std_path(def_site, &["panicking", "panic"]);
            call_tokens.push(group(
                Delimiter::Parenthesis,
                call,
                vec![TokenTree::Literal {
                    token_text: string_literal(&format!("assertion failed: {message}")),
                    span: def_site,
                }],
            ));
            call_tokens
        }
    };
    // `if !cond { then }`: the parentheses keep the parsed `Unary(Not, cond)`
    // structure that rustc builds directly as AST.
    let tokens = vec![
        ident("if", call),
        punct('!', Spacing::Alone, call),
        group(Delimiter::Parenthesis, call, cond.to_vec()),
        group(Delimiter::Brace, call, then),
    ];
    out.finish(tokens)
}

/// rustc `pprust::expr_to_string(&cond_expr)`.
fn assertion_message(
    out: &mut Output,
    cond: &[TokenTree],
    facts: &dyn BuiltinFacts,
) -> Result<Result<String, BuiltinFailure>, AdapterError> {
    let store = out.input_contexts();
    let root = TokenTree::Group {
        delimiter: Delimiter::Invisible,
        open: out.call_site,
        close: out.call_site,
        children: cond.to_vec(),
    };
    let mut used = BTreeSet::new();
    collect_spans(&root, &mut used);
    let forest = store.export_records(vec![root], used)?;
    let mut scratch = store.clone();
    let tree = scratch.import(&forest)?;
    let (parse, _) =
        syntax_bridge::token_tree_to_syntax_node(&tree, parser::TopEntryPoint::Expr, &mut |ctx| {
            ctx.edition(scratch.database())
        });
    if let Some(error) = parse.errors().first() {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            error.to_string(),
            cond.first().map(first_span),
        )));
    }
    let node = parse.syntax_node();
    let Some(expr) = std::iter::once(node.clone())
        .chain(node.descendants())
        .find_map(<syntax::ast::Expr as syntax::AstNode>::cast)
    else {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            "expected expression",
            cond.first().map(first_span),
        )));
    };
    let index = tts::SpanIndex::new(&forest);
    let contexts: BTreeMap<Key, &ContextRecord> =
        forest.contexts().iter().map(|c| (c.key(), c)).collect();
    let dollar_crate = |span: Key| -> Result<String, AdapterError> {
        let record = index.record(span)?;
        let context = contexts
            .get(&record.context)
            .ok_or_else(|| fault("`$crate` span has no context record"))?;
        if context.expansion.is_none() || context.transparency == Transparency::Transparent {
            return Err(fault("`$crate` token lacks definition-site authority"));
        }
        facts.dollar_crate_name(context.defining_unit)
    };
    let dollar_crate: &dyn Fn(Key) -> Result<String, AdapterError> = &dollar_crate;
    let converter = ast::Converter::new(&node, cond, &index, Some(dollar_crate))?;
    match converter.expr(&expr) {
        Ok(expr) => Ok(Ok(print::State::expr_to_string(&expr))),
        Err(error) if error.code == "LRR-E-MACRO-BUILTIN-UNSUPPORTED" => Ok(Err(failure(
            BuiltinFailureKind::Unsupported,
            format!("assert! message printing: {}", error.message),
            cond.first().map(first_span),
        ))),
        Err(error) => Err(error),
    }
}

/// rustc `format::expand_format_args_impl` as `builtin # format_args(..)`, the
/// pinned parser's `FORMAT_ARGS_EXPR` syntax. `format_args_nl!` appends a newline
/// to the template value (rustc `make_format_args`, `append_newline`).
fn expand_format_args(out: &mut Output, args: &Args<'_>, newline: bool) -> Outcome {
    if args.children.is_empty() {
        return Ok(Err(failure(
            BuiltinFailureKind::Syntax,
            "requires at least a format string argument",
            Some(out.call_site),
        )));
    }
    let mut children = args.children.to_vec();
    if newline && !append_newline_to_literal(&mut children) {
        // Non-literal template (e.g. `concat!(..)`): rustc eagerly expands it and
        // appends `\n` to the resulting string; `$crate::concat!` performs the same
        // eager expansion and concatenation.
        let consumed = match expression_prefix(out, args.children)? {
            Ok(n) => n,
            Err(failure) => return Ok(Err(failure)),
        };
        let def_site = out.def_site()?;
        let mut concat = std_path(def_site, &["concat"]);
        concat.push(punct('!', Spacing::Alone, def_site));
        let mut concat_args = args.children[..consumed].to_vec();
        concat_args.push(punct(',', Spacing::Alone, def_site));
        concat_args.push(TokenTree::Literal {
            token_text: "\"\\n\"".into(),
            span: def_site,
        });
        concat.push(group(Delimiter::Parenthesis, def_site, concat_args));
        concat.extend(args.children[consumed..].iter().cloned());
        children = concat;
    }
    let def_site = out.def_site()?;
    let tokens = vec![
        ident("builtin", def_site),
        punct('#', Spacing::Alone, def_site),
        ident("format_args", def_site),
        TokenTree::Group {
            delimiter: Delimiter::Parenthesis,
            open: args.open,
            close: args.close,
            children,
        },
    ];
    out.finish(tokens)
}

/// Append `\n` to a leading string literal template (through invisible groups).
fn append_newline_to_literal(children: &mut [TokenTree]) -> bool {
    match children.first_mut() {
        Some(TokenTree::Group {
            delimiter: Delimiter::Invisible,
            children,
            ..
        }) => append_newline_to_literal(children),
        Some(TokenTree::Literal { token_text, .. }) => {
            if let Some(raw) = token_text.strip_prefix('r') {
                let hashes = raw.chars().take_while(|c| *c == '#').count();
                let closing = format!("\"{}", "#".repeat(hashes));
                if !raw[hashes..].starts_with('"') || !token_text.ends_with(&closing) {
                    return false;
                }
                let at = token_text.len() - closing.len();
                token_text.insert(at, '\n');
                true
            } else if token_text.starts_with('"')
                && token_text.ends_with('"')
                && token_text.len() >= 2
            {
                let at = token_text.len() - 1;
                token_text.insert_str(at, "\\n");
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// rustc `compile_error::expand_compile_error` (`get_single_str_from_tts`).
fn compile_error(children: &[TokenTree], call_site: Key) -> BuiltinFailure {
    let mut flat = children;
    while let [
        TokenTree::Group {
            delimiter: Delimiter::Invisible,
            children,
            ..
        },
    ] = flat
    {
        flat = children;
    }
    let (first, rest) = match flat.split_first() {
        None => {
            return failure(
                BuiltinFailureKind::Syntax,
                "compile_error! takes 1 argument",
                Some(call_site),
            );
        }
        Some(split) => split,
    };
    let trailing_ok = matches!(rest, [] | [TokenTree::Punct { scalar: ',', .. }]);
    // `path!(..)`: rustc eagerly expands the call before reading the string.
    let macro_call = flat.windows(2).any(|pair| {
        matches!(
            pair,
            [
                TokenTree::Punct { scalar: '!', .. },
                TokenTree::Group {
                    delimiter: Delimiter::Parenthesis | Delimiter::Bracket | Delimiter::Brace,
                    ..
                }
            ]
        )
    }) && matches!(
        first,
        TokenTree::Ident { .. } | TokenTree::Punct { scalar: ':', .. }
    );
    match first {
        TokenTree::Literal { token_text, span } if trailing_ok => {
            match tts::unquote_str(token_text) {
                Some(message) => {
                    failure(BuiltinFailureKind::CompileError, message, Some(call_site))
                }
                None => failure(
                    BuiltinFailureKind::Syntax,
                    "argument must be a string literal",
                    Some(*span),
                ),
            }
        }
        _ if macro_call => failure(
            BuiltinFailureKind::RequiresEagerExpansion,
            "compile_error! argument must be eagerly expanded to a string literal",
            Some(call_site),
        ),
        _ if trailing_ok => failure(
            BuiltinFailureKind::Syntax,
            "argument must be a string literal",
            Some(first_span(first)),
        ),
        _ => failure(
            BuiltinFailureKind::Syntax,
            "compile_error! takes 1 argument",
            Some(call_site),
        ),
    }
}
