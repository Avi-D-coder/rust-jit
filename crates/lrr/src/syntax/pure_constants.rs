//! Pure, sealed constant source plans. Pinned AST identities never cross into frontend.
use super::{
    ast_adapter::range,
    outline::{BodyHandle, DeclarationContext, ScopeKey},
    parse::ParsedFile,
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    frontend::{
        constant_macro_admission::AdmittedConstantPanic,
        constant_source_admission::CurrentConstantTypeCertificate, constants::ConstValue,
    },
    ir::op::{DecodedLiteral, NumericBinary, NumericUnary, PrimitiveType},
    source::{
        changes::{PreparedSourceView, SourceSelection},
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob},
    },
};
use ra_ap_syntax::{
    AstNode, AstToken,
    ast::{self, HasArgList, HasAttrs, HasGenericArgs, HasGenericParams, HasModuleItem, HasName},
};
use std::{collections::BTreeMap, rc::Rc};
#[path = "constant_parameter_lengths.rs"]
mod parameter_lengths;
use parameter_lengths::ParameterIndependentLength;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-SOURCE", message.into())
}
fn required<T>(v: Option<T>) -> Result<T, Diagnostic> {
    v.ok_or_else(|| fault("constant syntax is incomplete"))
}
#[derive(Debug, Clone)]
pub struct ConstantDeclaration {
    name: String,
    range: SourceRange,
    definition: Digest,
    body: BodyKey,
    blocked: bool,
    context: DeclarationContext,
    handle: BodyHandle,
}
impl ConstantDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub fn body_handle(&self) -> &BodyHandle {
        &self.handle
    }
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
}
#[derive(Debug)]
struct Captured {
    parsed: ParsedFile,
    cfg: CfgSet,
    crate_key: Digest,
    scope: ScopeKey,
    selection: Option<SourceSelection>,
    module_items: Vec<ast::Item>,
    type_certificate: Option<CurrentConstantTypeCertificate>,
    macro_proofs: BTreeMap<(u32, u32), AdmittedConstantPanic>,
    declarations: Vec<ConstantDeclaration>,
    items: BTreeMap<String, Vec<ast::Item>>,
    owners: Vec<(SourceRange, Digest, Option<BodyHandle>)>,
}
#[derive(Debug, Clone)]
pub struct ConstantSource(Rc<Captured>);
#[derive(Debug, Clone)]
pub struct ConstantPlan {
    source: ConstantSource,
    entry: Entry,
    identity: Digest,
    owner_definition: Digest,
    owner_context: DeclarationContext,
    owner_body: Option<BodyHandle>,
}
#[derive(Debug, Clone)]
enum Entry {
    Constant(ast::Const),
    Call(ast::Fn, Vec<ConstValue>),
    Length(ExprRef),
}
#[derive(Debug, Clone)]
pub(crate) struct ExprRef(ast::Expr);
impl ExprRef {
    pub(crate) fn key(&self) -> (u32, u32) {
        let r = self.range();
        (r.start, r.end)
    }
    pub(crate) fn range(&self) -> SourceRange {
        range(self.0.syntax().text_range())
    }
}
#[derive(Debug, Clone)]
pub(crate) enum ConstType {
    Scalar(PrimitiveType),
    Array(PrimitiveType, ExprRef),
}
#[derive(Debug, Clone)]
pub(crate) struct Function {
    pub(crate) declaration: ConstantDeclaration,
    pub(crate) params: Vec<(String, PrimitiveType)>,
    pub(crate) result: ConstType,
    pub(crate) body: ExprRef,
}
#[derive(Debug, Clone)]
pub(crate) struct Constant {
    pub(crate) declaration: ConstantDeclaration,
    pub(crate) ty: ConstType,
    pub(crate) body: ExprRef,
}
#[derive(Debug, Clone)]
pub(crate) struct StatementRef(ast::Stmt);
impl StatementRef {
    pub(crate) fn range(&self) -> SourceRange {
        range(self.0.syntax().text_range())
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Statement {
    Let(String, Option<PrimitiveType>, ExprRef),
    Expression(ExprRef),
    Unsupported(SourceRange),
}
/// Source-owned syntax data for an actually reached macro, never authority.
#[derive(Debug, Clone)]
pub(crate) struct RequiredConstantMacro {
    source: Digest,
    body: BodyKey,
    range: SourceRange,
}
impl RequiredConstantMacro {
    pub(crate) fn source(&self) -> Digest {
        self.source
    }
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Node {
    Integer(DecodedLiteral),
    Bool(bool),
    Char(char),
    Byte(u8),
    Float(String, Option<PrimitiveType>),
    Unit,
    Path(String),
    Parentheses(ExprRef),
    Unary(NumericUnary, ExprRef),
    Binary(NumericBinary, ExprRef, ExprRef),
    Logic(bool, ExprRef, ExprRef),
    Cast(ExprRef, PrimitiveType),
    If(ExprRef, ExprRef, Option<ExprRef>),
    Block(Vec<StatementRef>, Option<ExprRef>),
    Return(Option<ExprRef>),
    Call(String, Vec<ExprRef>),
    Array(Vec<ExprRef>),
    Repeat(ExprRef, ExprRef),
    Panic,
    AdmittedPanic(AdmittedConstantPanic),
    RequiredMacro(Box<RequiredConstantMacro>),
    Unsupported,
}
impl ConstantSource {
    /// Captures source bytes/profile/cfg privately. Never accepts caller-built outline/body proofs.
    pub fn parse(
        blob: &SourceBlob,
        profile: ProfileIdentity,
        crate_key: Digest,
        cfg: CfgSet,
    ) -> Result<Self, Diagnostic> {
        let parsed = ParsedFile::parse(blob, profile)?;
        let outline = parsed.outline(crate_key, vec![], &cfg)?;
        let scope = outline.root_scope.clone();
        let items = parsed.parsed.tree().items().collect();
        Self::capture_scope(parsed, cfg, crate_key, outline, scope, None, items)
    }
    /// Capture one actual original module, not a caller-issued namespace proof.
    /// Header inventory is structural; bodies are not typechecked or evaluated.
    pub(crate) fn capture_prepared(
        view: &PreparedSourceView,
        definition: Digest,
    ) -> Result<Self, Diagnostic> {
        view.require_original_hygiene()?;
        view.require_declaration_syntax()?;
        let selected = view.selection();
        let cfg = view.captured_macro_cfg()?.clone();
        let parsed = ParsedFile::parse(
            view.captured_macro_blob()?,
            selected.outline.parse.profile.clone(),
        )?;
        let outline = parsed.outline(
            selected.outline.crate_key,
            selected.outline.module_path.clone(),
            &cfg,
        )?;
        if parsed.key() != selected.outline.parse
            || outline.fingerprint() != view.outline().fingerprint()
        {
            return Err(fault("prepared constant parse/outline differs"));
        }
        let item = outline
            .items
            .iter()
            .find(|i| i.def_key.fingerprint() == definition)
            .ok_or_else(|| fault("prepared constant declaration absent"))?;
        if item.scope.owner.is_some() {
            return Err(fault(
                "lexical/associated constant owner requires separate preparation",
            ));
        }
        let scope = item.scope.clone();
        view.captured_inline_module_layout(&scope)?;
        let syntax = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Item::cast)
            .find(|i| range(i.syntax().text_range()) == item.declaration_span)
            .ok_or_else(|| fault("prepared constant declaration syntax absent"))?;
        if !matches!(syntax, ast::Item::Const(_) | ast::Item::Fn(_)) {
            return Err(fault(
                "constant plan requires a direct const/function body owner",
            ));
        }
        if syntax
            .syntax()
            .ancestors()
            .any(|a| ast::Impl::can_cast(a.kind()) || ast::Trait::can_cast(a.kind()))
        {
            return Err(fault(
                "associated constant needs an owning member certificate",
            ));
        }
        let parent = required(syntax.syntax().parent())?;
        if !ast::SourceFile::can_cast(parent.kind()) && !ast::ItemList::can_cast(parent.kind()) {
            return Err(fault("constant owner is not a captured module"));
        }
        let items = parent.children().filter_map(ast::Item::cast).collect();
        Self::capture_scope(
            parsed,
            cfg,
            selected.outline.crate_key,
            outline,
            scope,
            Some(selected.clone()),
            items,
        )
    }
    fn capture_scope(
        parsed: ParsedFile,
        cfg: CfgSet,
        crate_key: Digest,
        outline: super::outline::Outline,
        scope: ScopeKey,
        selection: Option<SourceSelection>,
        module_items: Vec<ast::Item>,
    ) -> Result<Self, Diagnostic> {
        let owners = outline
            .items
            .iter()
            .filter(|item| item.scope == scope)
            .map(|item| {
                (
                    item.declaration_span,
                    item.def_key.fingerprint(),
                    item.body.clone(),
                )
            })
            .collect();
        let mut declarations = Vec::new();
        let mut items: BTreeMap<String, Vec<ast::Item>> = BTreeMap::new();
        for item in &module_items {
            let at = range(item.syntax().text_range());
            let Some(header) = outline
                .items
                .iter()
                .find(|h| h.declaration_span == at && h.scope == scope)
            else {
                continue;
            };
            let name = match &item {
                ast::Item::Const(c) => c.name(),
                ast::Item::Fn(f) => f.name(),
                _ => None,
            };
            let Some(name) = name else { continue };
            let Some(body) = header.body.as_ref() else {
                continue;
            };
            if declarations.len() >= 100_000 {
                return Err(fault("constant declaration inventory limit"));
            }
            declarations
                .try_reserve(1)
                .map_err(|_| fault("constant declaration allocation"))?;
            declarations.push(ConstantDeclaration {
                name: semantic(name.text())?,
                range: at,
                definition: header.def_key.fingerprint(),
                body: body.key(),
                blocked: body.entry_blocked,
                context: DeclarationContext {
                    scope: header.scope.clone(),
                    range: at,
                },
                handle: body.clone(),
            });
            items
                .entry(semantic(name.text())?)
                .or_default()
                .push(item.clone());
        }
        Ok(Self(Rc::new(Captured {
            parsed,
            cfg,
            crate_key,
            scope,
            selection,
            module_items,
            type_certificate: None,
            macro_proofs: BTreeMap::new(),
            declarations,
            items,
            owners,
        })))
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.0.scope
    }
    pub(crate) fn selection(&self) -> Option<&SourceSelection> {
        self.0.selection.as_ref()
    }
    pub(crate) fn with_type_certificate(
        mut self,
        certificate: CurrentConstantTypeCertificate,
    ) -> Result<Self, Diagnostic> {
        if !self
            .0
            .selection
            .as_ref()
            .is_some_and(|selection| certificate.matches(selection, &self.0.scope))
        {
            return Err(fault("constant primitive certificate source/scope differs"));
        }
        certificate.metadata_bytes()?;
        let captured = Rc::get_mut(&mut self.0)
            .ok_or_else(|| fault("constant source certificate requires a fresh capture"))?;
        if captured.type_certificate.is_some() {
            return Err(fault("constant source certificate already attached"));
        }
        captured.type_certificate = Some(certificate);
        Ok(self)
    }
    pub(crate) fn with_macro_proof(
        mut self,
        proof: AdmittedConstantPanic,
    ) -> Result<Self, Diagnostic> {
        let selection =
            self.0.selection.as_ref().ok_or_else(|| {
                fault("content-only constants cannot attach current macro authority")
            })?;
        let range = proof.range();
        let body = self
            .0
            .owners
            .iter()
            .filter(|(owner, _, _)| owner.contains(range))
            .min_by_key(|(owner, _, _)| owner.end - owner.start)
            .and_then(|(_, _, body)| body.as_ref())
            .ok_or_else(|| fault("constant macro proof owning body absent"))?;
        if !proof.matches(selection, body, range) {
            return Err(fault("constant macro proof source/body/range differs"));
        }
        if self.0.macro_proofs.len() >= 256 {
            return Err(fault("constant macro proof count limit"));
        }
        let total = self
            .metadata_bytes()?
            .checked_add(proof.metadata_bytes())
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("constant macro proof metadata limit"))?;
        let _ = total;
        let captured = Rc::get_mut(&mut self.0)
            .ok_or_else(|| fault("constant macro proof requires a fresh capture"))?;
        if captured
            .macro_proofs
            .insert((range.start, range.end), proof)
            .is_some()
        {
            return Err(fault("constant macro proof range was already attached"));
        }
        Ok(self)
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let nodes = self
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants_with_tokens()
            .take(100_001)
            .count();
        if nodes > 100_000 {
            return Err(fault("constant source node count limit"));
        }
        let mut bytes = nodes
            .checked_mul(128)
            .and_then(|n| n.checked_add(std::mem::size_of::<Captured>()))
            .and_then(|n| n.checked_add(self.0.scope.heap_bytes()))
            .ok_or_else(|| fault("constant source metadata overflow"))?;
        for declaration in &self.0.declarations {
            bytes = bytes
                .checked_add(std::mem::size_of::<ConstantDeclaration>() + 256)
                .and_then(|n| n.checked_add(declaration.name.capacity()))
                .and_then(|n| n.checked_add(declaration.context.scope.heap_bytes()))
                .ok_or_else(|| fault("constant source declaration metadata overflow"))?;
        }
        for proof in self.0.macro_proofs.values() {
            bytes = bytes
                .checked_add(proof.metadata_bytes())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("constant macro metadata overflow"))?;
        }
        bytes = bytes
            .checked_add(
                self.0
                    .type_certificate
                    .as_ref()
                    .map_or(Ok(0), CurrentConstantTypeCertificate::metadata_bytes)?,
            )
            .ok_or_else(|| fault("constant type certificate metadata overflow"))?;
        bytes
            .checked_add(
                self.0
                    .selection
                    .as_ref()
                    .map_or(Ok(0), SourceSelection::metadata_bytes)?,
            )
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("constant source metadata limit"))
    }
    pub fn declarations(&self) -> &[ConstantDeclaration] {
        &self.0.declarations
    }
    fn exact(&self, at: SourceRange) -> Result<ast::Item, Diagnostic> {
        let d = self
            .0
            .declarations
            .iter()
            .find(|d| d.range == at)
            .ok_or_else(|| fault("selected range is not an exact captured constant declaration"))?;
        if d.blocked {
            return Err(fault(
                "selected constant body has unreliable syntax boundaries",
            ));
        }
        self.0
            .items
            .get(&d.name)
            .and_then(|items| items.iter().find(|i| range(i.syntax().text_range()) == at))
            .cloned()
            .ok_or_else(|| fault("captured declaration absent"))
    }
    pub fn constant(&self, at: SourceRange) -> Result<ConstantPlan, Diagnostic> {
        let ast::Item::Const(c) = self.exact(at)? else {
            return Err(fault("selected declaration is not a constant"));
        };
        self.constant_header(&c)?;
        self.plan(Entry::Constant(c), at)
    }
    pub fn const_call(
        &self,
        at: SourceRange,
        args: Vec<ConstValue>,
    ) -> Result<ConstantPlan, Diagnostic> {
        let ast::Item::Fn(f) = self.exact(at)? else {
            return Err(fault("selected declaration is not const fn"));
        };
        let header = self.function_header(&f)?;
        if header.params.len() != args.len() {
            return Err(fault("pure const argument cardinality differs"));
        }
        for ((_, ty), arg) in header.params.iter().zip(&args) {
            if !arg.matches_scalar(*ty)? {
                return Err(fault("pure const argument type differs"));
            }
        }
        self.plan(Entry::Call(f, args), at)
    }
    /// Only an exact array repeat count or array type length may request usize context.
    pub fn array_length(&self, at: SourceRange) -> Result<ConstantPlan, Diagnostic> {
        for node in self.0.parsed.parsed.tree().syntax().descendants() {
            let candidate = if let Some(a) = ast::ArrayExpr::cast(node.clone()) {
                match a.kind() {
                    ast::ArrayExprKind::Repeat { repeat, .. } => repeat,
                    _ => None,
                }
            } else if let Some(a) = ast::ArrayType::cast(node) {
                a.const_arg()
                    .and_then(|c| c.syntax().children().find_map(ast::Expr::cast))
            } else {
                None
            };
            if let Some(expr) = candidate
                && range(expr.syntax().text_range()) == at
            {
                if !super::attributes::source_root_enabled(
                    &self.0.parsed.parsed.tree(),
                    &self.0.cfg,
                )? {
                    return Err(fault("array length owning root is cfg-disabled"));
                }
                let mut parameter_length = None;
                for ancestor in expr.syntax().ancestors() {
                    if (ast::Module::can_cast(ancestor.kind()) && self.0.selection.is_none())
                        || ast::Impl::can_cast(ancestor.kind())
                        || ast::Trait::can_cast(ancestor.kind())
                        || ast::ClosureExpr::can_cast(ancestor.kind())
                    {
                        return Err(fault(
                            "anonymous array length requires nested source namespace preparation",
                        ));
                    }
                    if let Some(function) = ast::Fn::cast(ancestor.clone()) {
                        let params = required(function.param_list())?;
                        if params.self_param().is_some()
                            || function.generic_param_list().is_some()
                            || function.where_clause().is_some()
                            || ancestor.parent().is_none_or(|p| {
                                !ast::SourceFile::can_cast(p.kind())
                                    && !(self.0.selection.is_some()
                                        && ast::ItemList::can_cast(p.kind()))
                            })
                        {
                            return Err(fault(
                                "anonymous array length requires captured function binding context",
                            ));
                        }
                        if params.params().next().is_some() {
                            if parameter_length.is_some() {
                                return Err(fault("nested parameter length owner is ambiguous"));
                            }
                            parameter_length = Some(self.prove_parameter_length(&function, &expr)?);
                        }
                    }
                    if let Some(statements) = ast::StmtList::cast(ancestor.clone()) {
                        for statement in statements.statements() {
                            if let ast::Stmt::LetStmt(binding) = statement
                                && range(binding.syntax().text_range()).end <= at.start
                                && binding.pat().is_some_and(|p| {
                                    p.syntax()
                                        .descendants()
                                        .any(|n| ast::IdentPat::can_cast(n.kind()))
                                })
                            {
                                return Err(fault(
                                    "anonymous array length requires captured preceding local bindings",
                                ));
                            }
                        }
                    }
                    let metas = ancestor
                        .children()
                        .filter_map(ast::Attr::cast)
                        .map(|a| required(a.meta()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (enabled, remaining) =
                        super::attributes::effective_metas(metas, &self.0.cfg)?;
                    let remaining = remaining
                        .into_iter()
                        .filter(|meta| {
                            !self.0.type_certificate.as_ref().is_some_and(|proof| {
                                proof.permits_inert_attribute(range(meta.syntax().text_range()))
                            })
                        })
                        .collect::<Vec<_>>();
                    if !enabled
                        || remaining
                            .iter()
                            .any(|m| !matches!(m, ast::Meta::CfgMeta(_)))
                    {
                        return Err(fault(
                            "array length owning syntax requires attribute preparation",
                        ));
                    }
                }
                return self.plan_with_parameter_length(
                    Entry::Length(ExprRef(expr)),
                    at,
                    parameter_length,
                );
            }
        }
        Err(fault(
            "selected range is not an exact captured array length",
        ))
    }
    fn plan(&self, entry: Entry, at: SourceRange) -> Result<ConstantPlan, Diagnostic> {
        self.plan_with_parameter_length(entry, at, None)
    }
    fn plan_with_parameter_length(
        &self,
        entry: Entry,
        at: SourceRange,
        parameter_length: Option<ParameterIndependentLength>,
    ) -> Result<ConstantPlan, Diagnostic> {
        if self
            .0
            .parsed
            .errors
            .iter()
            .any(|e| e.range.start < at.end && at.start < e.range.end)
        {
            return Err(fault("selected constant syntax is malformed"));
        }
        let (owner_range, owner_definition, owner_body) = self
            .0
            .owners
            .iter()
            .filter(|(owner, _, _)| owner.start <= at.start && at.end <= owner.end)
            .min_by_key(|(owner, _, _)| owner.end - owner.start)
            .ok_or_else(|| fault("constant selection has no exact captured declaration owner"))?;
        if let Some(body) = owner_body {
            body.require_entry()?;
        }
        let mut e = StableEncoder::new("lrr/v5/pure-constant-source-plan");
        e.digest(self.0.crate_key);
        e.option(&self.0.type_certificate, |e, proof| {
            proof.encode_inert_attributes(e)
        });
        e.option(&parameter_length, |e, proof| e.digest(proof.identity()));
        e.u64(self.0.macro_proofs.len() as u64);
        for ((start, end), proof) in &self.0.macro_proofs {
            e.u32(*start);
            e.u32(*end);
            e.digest(proof.identity());
        }
        e.record(&self.0.scope);
        e.option(&self.0.selection, |e, selection| {
            e.string(selection.path.as_str());
            e.digest(selection.location_identity());
            e.digest(selection.outline.parse.content);
            e.record(&selection.outline.parse.profile);
            e.digest(selection.outline.cfg);
        });
        e.digest(*owner_definition);
        e.option(owner_body, |e, body| e.record(body));
        e.digest(self.0.parsed.content);
        e.record(&self.0.parsed.profile);
        e.digest(self.0.cfg.fingerprint());
        e.u32(at.start);
        e.u32(at.end);
        if let Some(d) = self.0.declarations.iter().find(|d| d.range == at) {
            e.digest(d.definition);
            e.record(&d.body)
        }
        match &entry {
            Entry::Constant(_) => e.u8(1),
            Entry::Call(_, args) => {
                e.u8(2);
                e.sequence(args, |e, a| e.record(a))
            }
            Entry::Length(_) => e.u8(3),
        }
        Ok(ConstantPlan {
            source: self.clone(),
            entry,
            identity: e.finish(),
            owner_definition: *owner_definition,
            owner_context: DeclarationContext {
                scope: self.0.scope.clone(),
                range: *owner_range,
            },
            owner_body: owner_body.clone(),
        })
    }
    pub(crate) fn source(&self) -> Digest {
        self.0.parsed.content
    }
    pub(crate) fn cfg(&self) -> Digest {
        self.0.cfg.fingerprint()
    }
    pub(crate) fn profile(&self) -> &ProfileIdentity {
        &self.0.parsed.profile
    }
    fn declaration(&self, item: &impl AstNode) -> Result<ConstantDeclaration, Diagnostic> {
        self.0
            .declarations
            .iter()
            .find(|d| d.range == range(item.syntax().text_range()))
            .cloned()
            .ok_or_else(|| fault("constant header is not captured"))
    }
    fn attributes_clear(&self, node: &ra_ap_syntax::SyntaxNode) -> Result<(), Diagnostic> {
        let metas = node
            .children()
            .filter_map(ast::Attr::cast)
            .map(|a| required(a.meta()))
            .collect::<Result<Vec<_>, _>>()?;
        let (enabled, metas) = super::attributes::effective_metas(metas, &self.0.cfg)?;
        if !enabled || metas.iter().any(|m| !matches!(m, ast::Meta::CfgMeta(_))) {
            return Err(fault(
                "constant attributes require prior source preparation",
            ));
        }
        Ok(())
    }
    fn primitive(&self, ty: ast::Type) -> Result<PrimitiveType, Diagnostic> {
        let spelling = ty.syntax().text().to_string();
        if spelling.trim() == "()" {
            return Ok(PrimitiveType::Unit);
        }
        let ast::Type::PathType(path) = &ty else {
            return Err(fault("constant type is not a handwritten primitive"));
        };
        let name = bare(required(path.path())?)?;
        use PrimitiveType as P;
        let primitive = match name.as_str() {
            "bool" => P::Bool,
            "char" => P::Char,
            "i8" => P::I8,
            "i16" => P::I16,
            "i32" => P::I32,
            "i64" => P::I64,
            "isize" => P::Isize,
            "u8" => P::U8,
            "u16" => P::U16,
            "u32" => P::U32,
            "u64" => P::U64,
            "usize" => P::Usize,
            "f32" => P::F32,
            "f64" => P::F64,
            _ => return Err(fault("constant primitive type identity is unavailable")),
        };
        // Conservative captured lexical proof. Imports/producers never grant primitive identity.
        let mut work = 0usize;
        for scope in ty.syntax().ancestors() {
            for child in scope.children() {
                work = work
                    .checked_add(1)
                    .filter(|n| *n <= 100_000)
                    .ok_or_else(|| fault("constant namespace work limit"))?;
                if let Some(g)=ast::GenericParamList::cast(child.clone()) && g.generic_params().any(|p|matches!(p,ast::GenericParam::TypeParam(t) if t.name().is_some_and(|n|n.text()==name))){return Err(fault("constant primitive type is shadowed by a parameter"))}
                let Some(item) = ast::Item::cast(child) else {
                    continue;
                };
                let metas = item
                    .attrs()
                    .map(|a| required(a.meta()))
                    .collect::<Result<Vec<_>, _>>()?;
                let (enabled, metas) = super::attributes::effective_metas(metas, &self.0.cfg)?;
                if !enabled {
                    continue;
                }
                if metas.iter().any(|m| !matches!(m, ast::Meta::CfgMeta(_)))
                    || matches!(item, ast::Item::Use(_) | ast::Item::MacroCall(_))
                {
                    return Err(fault(
                        "constant primitive namespace has an unresolved source producer/import",
                    ));
                }
                let declared = match &item {
                    ast::Item::TypeAlias(x) => x.name(),
                    ast::Item::Struct(x) => x.name(),
                    ast::Item::Enum(x) => x.name(),
                    ast::Item::Union(x) => x.name(),
                    ast::Item::Trait(x) => x.name(),
                    ast::Item::Module(x) => x.name(),
                    _ => None,
                };
                if declared.is_some_and(|n| semantic(n.text()).is_ok_and(|n| n == name)) {
                    return Err(fault("constant primitive type is shadowed in source"));
                }
            }
        }
        if self.0.selection.is_some()
            && !self
                .0
                .type_certificate
                .as_ref()
                .is_some_and(|proof| proof.permits(&name))
        {
            return Err(fault(
                "reached constant primitive requires current Type source identity",
            ));
        }
        Ok(primitive)
    }
    fn ty(&self, ty: ast::Type) -> Result<ConstType, Diagnostic> {
        if let ast::Type::ArrayType(a) = ty {
            let len = required(
                a.const_arg()
                    .and_then(|c| c.syntax().children().find_map(ast::Expr::cast)),
            )?;
            Ok(ConstType::Array(
                self.primitive(required(a.ty())?)?,
                ExprRef(len),
            ))
        } else {
            Ok(ConstType::Scalar(self.primitive(ty)?))
        }
    }
    fn constant_header(&self, c: &ast::Const) -> Result<Constant, Diagnostic> {
        self.attributes_clear(c.syntax())?;
        if self.declaration(c)?.blocked {
            return Err(fault("constant body boundaries are blocked"));
        }
        Ok(Constant {
            declaration: self.declaration(c)?,
            ty: self.ty(required(c.ty())?)?,
            body: ExprRef(required(c.body())?),
        })
    }
    fn function_header(&self, f: &ast::Fn) -> Result<Function, Diagnostic> {
        self.attributes_clear(f.syntax())?;
        if self.declaration(f)?.blocked {
            return Err(fault("const fn body boundaries are blocked"));
        }
        if f.const_token().is_none()
            || f.generic_param_list().is_some()
            || f.async_token().is_some()
            || f.unsafe_token().is_some()
        {
            return Err(Diagnostic::new(
                "LRR-E-CONST-EFFECT",
                "reached call is not a supported pure const fn".into(),
            ));
        }
        let mut params = Vec::new();
        for p in required(f.param_list())?.params() {
            if params.len() >= 256 {
                return Err(fault("constant parameter limit"));
            }
            let name = local_name(required(p.pat())?)?;
            let ty = self.primitive(required(p.ty())?)?;
            if params.iter().any(|(n, _)| n == &name) {
                return Err(fault("duplicate constant parameter"));
            }
            params.push((name, ty));
        }
        let result = match f.ret_type() {
            Some(r) => self.ty(required(r.ty())?)?,
            None => ConstType::Scalar(PrimitiveType::Unit),
        };
        Ok(Function {
            declaration: self.declaration(f)?,
            params,
            result,
            body: ExprRef(ast::Expr::BlockExpr(required(f.body())?)),
        })
    }
    pub(crate) fn named_constant(&self, name: &str) -> Result<Constant, Diagnostic> {
        let i = self.named(name)?;
        let ast::Item::Const(c) = i else {
            return Err(fault("reached name is not a named constant"));
        };
        self.constant_header(&c)
    }
    pub(crate) fn named_function(&self, name: &str) -> Result<Function, Diagnostic> {
        let i = self.named(name)?;
        let ast::Item::Fn(f) = i else {
            return Err(fault("reached callee is not const fn"));
        };
        self.function_header(&f)
    }
    fn named(&self, name: &str) -> Result<ast::Item, Diagnostic> {
        // Generated declarations/import routing is a separate provider integration.
        for item in &self.0.module_items {
            let metas = item
                .attrs()
                .map(|a| required(a.meta()))
                .collect::<Result<Vec<_>, _>>()?;
            let (enabled, _) = super::attributes::effective_metas(metas, &self.0.cfg)?;
            if enabled && matches!(item, ast::Item::Use(_) | ast::Item::MacroCall(_)) {
                return Err(fault("constant name requires source namespace preparation"));
            }
        }
        let items = self
            .0
            .items
            .get(name)
            .ok_or_else(|| fault("reached constant name is absent"))?;
        if items.len() != 1 {
            return Err(fault("reached constant name is ambiguous"));
        }
        Ok(items[0].clone())
    }
    pub(crate) fn statement(&self, s: &StatementRef) -> Result<Statement, Diagnostic> {
        Ok(match &s.0 {
            ast::Stmt::LetStmt(l) => {
                self.attributes_clear(l.syntax())?;
                if l.let_else().is_some() {
                    Statement::Unsupported(s.range())
                } else {
                    Statement::Let(
                        local_name(required(l.pat())?)?,
                        l.ty().map(|t| self.primitive(t)).transpose()?,
                        ExprRef(required(l.initializer())?),
                    )
                }
            }
            ast::Stmt::ExprStmt(s) => Statement::Expression(ExprRef(required(s.expr())?)),
            _ => Statement::Unsupported(s.range()),
        })
    }
    fn lexical_value_clear(
        &self,
        syntax: &ra_ap_syntax::SyntaxNode,
        wanted: &str,
    ) -> Result<(), Diagnostic> {
        let mut work = 0usize;
        for scope in syntax.ancestors() {
            if !ast::StmtList::can_cast(scope.kind()) {
                continue;
            }
            for child in scope.children() {
                work = work
                    .checked_add(1)
                    .filter(|n| *n <= 100_000)
                    .ok_or_else(|| fault("constant lexical value work limit"))?;
                let Some(item) = ast::Item::cast(child) else {
                    continue;
                };
                let metas = item
                    .attrs()
                    .map(|a| required(a.meta()))
                    .collect::<Result<Vec<_>, _>>()?;
                let (enabled, metas) = super::attributes::effective_metas(metas, &self.0.cfg)?;
                if !enabled {
                    continue;
                }
                if metas.iter().any(|m| !matches!(m, ast::Meta::CfgMeta(_)))
                    || matches!(item, ast::Item::Use(_) | ast::Item::MacroCall(_))
                {
                    return Err(fault(
                        "constant lexical value namespace requires source preparation",
                    ));
                }
                let name = match item {
                    ast::Item::Const(i) => i.name(),
                    ast::Item::Static(i) => i.name(),
                    ast::Item::Fn(i) => i.name(),
                    ast::Item::Struct(i) => i.name(),
                    _ => None,
                };
                if name.is_some_and(|n| semantic(n.text()).is_ok_and(|n| n == wanted)) {
                    return Err(fault(
                        "constant value has an unsupported local item identity",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn node(&self, e: &ExprRef) -> Result<Node, Diagnostic> {
        self.attributes_clear(e.0.syntax())?;
        use ast::Expr as E;
        Ok(match &e.0 {
            E::Literal(l) => match l.kind() {
                ast::LiteralKind::IntNumber(n) => {
                    Node::Integer(super::literal_decode::integer(&n, false))
                }
                ast::LiteralKind::Bool(v) => Node::Bool(v),
                ast::LiteralKind::Char(c) => {
                    Node::Char(c.value().map_err(|_| fault("invalid const char"))?)
                }
                ast::LiteralKind::Byte(c) => {
                    Node::Byte(c.value().map_err(|_| fault("invalid const byte"))?)
                }
                ast::LiteralKind::FloatNumber(n) => {
                    let ty = match n.suffix() {
                        None => None,
                        Some("f32") => Some(PrimitiveType::F32),
                        Some("f64") => Some(PrimitiveType::F64),
                        _ => return Err(fault("invalid constant float suffix")),
                    };
                    Node::Float(
                        n.text()
                            .to_string()
                            .trim_end_matches(n.suffix().unwrap_or(""))
                            .replace('_', ""),
                        ty,
                    )
                }
                _ => Node::Unsupported,
            },
            E::ParenExpr(p) => Node::Parentheses(ExprRef(required(p.expr())?)),
            E::TupleExpr(t) if t.fields().next().is_none() => Node::Unit,
            E::PathExpr(p) => {
                let name = bare(required(p.path())?)?;
                self.lexical_value_clear(p.syntax(), &name)?;
                Node::Path(name)
            }
            E::PrefixExpr(p) => {
                let expr = required(p.expr())?;
                if p.op_kind() == Some(ast::UnaryOp::Neg)
                    && let E::Literal(l) = &expr
                    && let ast::LiteralKind::IntNumber(n) = l.kind()
                {
                    Node::Integer(super::literal_decode::integer(&n, true))
                } else {
                    let op = match p.op_kind() {
                        Some(ast::UnaryOp::Neg) => NumericUnary::Neg,
                        Some(ast::UnaryOp::Not) => NumericUnary::Not,
                        _ => return Ok(Node::Unsupported),
                    };
                    Node::Unary(op, ExprRef(expr))
                }
            }
            E::BinExpr(b) => {
                let left = ExprRef(required(b.lhs())?);
                let right = ExprRef(required(b.rhs())?);
                match required(b.op_kind())? {
                    ast::BinaryOp::LogicOp(op) => Node::Logic(op == ast::LogicOp::And, left, right),
                    ast::BinaryOp::Assignment { .. } => Node::Unsupported,
                    op => {
                        use NumericBinary as B;
                        let op = match op.to_string().as_str() {
                            "+" => B::Add,
                            "-" => B::Sub,
                            "*" => B::Mul,
                            "/" => B::Div,
                            "%" => B::Rem,
                            "<<" => B::Shl,
                            ">>" => B::Shr,
                            "&" => B::BitAnd,
                            "|" => B::BitOr,
                            "^" => B::BitXor,
                            "==" => B::Eq,
                            "!=" => B::Ne,
                            "<" => B::Lt,
                            "<=" => B::Le,
                            ">" => B::Gt,
                            ">=" => B::Ge,
                            _ => return Ok(Node::Unsupported),
                        };
                        Node::Binary(op, left, right)
                    }
                }
            }
            E::CastExpr(c) => Node::Cast(
                ExprRef(required(c.expr())?),
                self.primitive(required(c.ty())?)?,
            ),
            E::IfExpr(i) => Node::If(
                ExprRef(required(i.condition())?),
                ExprRef(E::BlockExpr(required(i.then_branch())?)),
                i.else_branch().map(|b| {
                    ExprRef(match b {
                        ast::ElseBranch::Block(b) => E::BlockExpr(b),
                        ast::ElseBranch::IfExpr(i) => E::IfExpr(i),
                    })
                }),
            ),
            E::BlockExpr(b) => {
                if b.async_token().is_some()
                    || b.unsafe_token().is_some()
                    || b.const_token().is_some()
                    || b.gen_token().is_some()
                    || b.try_block_modifier().is_some()
                    || b.label().is_some()
                {
                    return Ok(Node::Unsupported);
                }
                let list = required(b.stmt_list())?;
                let mut stmts = Vec::new();
                for s in list.statements() {
                    if stmts.len() >= 100_000 {
                        return Err(fault("constant block inventory limit"));
                    }
                    stmts.push(StatementRef(s));
                }
                Node::Block(stmts, list.tail_expr().map(ExprRef))
            }
            E::ReturnExpr(r) => Node::Return(r.expr().map(ExprRef)),
            E::CallExpr(c) => {
                let E::PathExpr(p) = required(c.expr())? else {
                    return Ok(Node::Unsupported);
                };
                let args = required(c.arg_list())?.args().map(ExprRef).collect();
                let name = bare(required(p.path())?)?;
                self.lexical_value_clear(c.syntax(), &name)?;
                Node::Call(name, args)
            }
            E::ArrayExpr(a) => match a.kind() {
                ast::ArrayExprKind::Repeat {
                    initializer,
                    repeat,
                } => Node::Repeat(ExprRef(required(initializer)?), ExprRef(required(repeat)?)),
                ast::ArrayExprKind::ElementList(es) => Node::Array(es.map(ExprRef).collect()),
            },
            E::MacroExpr(m) => {
                if let Some(selection) = &self.0.selection {
                    let at = e.range();
                    let body = self
                        .0
                        .owners
                        .iter()
                        .filter(|(owner, _, _)| owner.contains(at))
                        .min_by_key(|(owner, _, _)| owner.end - owner.start)
                        .and_then(|(_, _, body)| body.as_ref())
                        .ok_or_else(|| fault("reached constant macro body identity absent"))?;
                    let Some(proof) = self.0.macro_proofs.get(&(at.start, at.end)) else {
                        return Ok(Node::RequiredMacro(Box::new(RequiredConstantMacro {
                            source: self.source(),
                            body: body.key(),
                            range: at,
                        })));
                    };
                    if !proof.matches(selection, body, at) {
                        return Err(fault(
                            "reached constant macro proof owner/source/range differs",
                        ));
                    }
                    return Ok(Node::AdmittedPanic(proof.clone()));
                }
                // Legacy content-only parsing retains its old syntactic classification;
                // this branch never authorizes current publication or reached Rust semantics.
                let call = required(m.macro_call())?;
                if bare(required(call.path())?)? == "panic"
                    && call.token_tree().is_some_and(|t| {
                        t.syntax()
                            .text()
                            .to_string()
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect::<String>()
                            == "()"
                    })
                {
                    Node::Panic
                } else {
                    Node::Unsupported
                }
            }
            _ => Node::Unsupported,
        })
    }
}
fn local_name(pat: ast::Pat) -> Result<String, Diagnostic> {
    let ast::Pat::IdentPat(p) = pat else {
        return Err(fault("constant local requires a scalar identifier"));
    };
    if p.ref_token().is_some() || p.pat().is_some() {
        return Err(fault("constant local binding mode unsupported"));
    }
    semantic(required(p.name())?.text())
}
fn bare(path: ast::Path) -> Result<String, Diagnostic> {
    let segment = required(path.segment())?;
    if path.qualifier().is_some()
        || segment.coloncolon_token().is_some()
        || segment.generic_arg_list().is_some()
    {
        return Err(fault("constant path requires captured namespace routing"));
    }
    semantic(required(segment.name_ref())?.text())
}
/// Ephemeral syntax selections remain on their owning query thread; memo values
/// contain canonical immutable bytes, not these non-Send rust-analyzer AST nodes.
type SelectedEntry = (
    Option<Constant>,
    Option<(Function, Vec<ConstValue>)>,
    Option<ExprRef>,
);

impl ConstantPlan {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn owner_definition(&self) -> Digest {
        self.owner_definition
    }
    pub fn owner_context(&self) -> &DeclarationContext {
        &self.owner_context
    }
    pub fn owner_body(&self) -> Option<&BodyHandle> {
        self.owner_body.as_ref()
    }
    pub fn profile(&self) -> &ProfileIdentity {
        self.source.profile()
    }
    pub(crate) fn source(&self) -> &ConstantSource {
        &self.source
    }
    pub(crate) fn entry(&self) -> Result<SelectedEntry, Diagnostic> {
        Ok(match &self.entry {
            Entry::Constant(c) => (Some(self.source.constant_header(c)?), None, None),
            Entry::Call(f, args) => (
                None,
                Some((self.source.function_header(f)?, args.clone())),
                None,
            ),
            Entry::Length(e) => (None, None, Some(e.clone())),
        })
    }
    pub(crate) fn range(&self) -> SourceRange {
        match &self.entry {
            Entry::Constant(c) => range(c.syntax().text_range()),
            Entry::Call(f, _) => range(f.syntax().text_range()),
            Entry::Length(e) => e.range(),
        }
    }
}

impl Node {
    pub(crate) fn owned_bytes(&self) -> Option<usize> {
        let extra = match self {
            Self::Path(s) | Self::Float(s, _) => s.capacity(),
            Self::RequiredMacro(_) => std::mem::size_of::<RequiredConstantMacro>(),
            Self::Block(s, _) => s
                .capacity()
                .checked_mul(std::mem::size_of::<StatementRef>())?,
            Self::Array(es) => es.capacity().checked_mul(std::mem::size_of::<ExprRef>())?,
            Self::Call(s, args) => s.capacity().checked_add(
                args.capacity()
                    .checked_mul(std::mem::size_of::<ExprRef>())?,
            )?,
            _ => 0,
        };
        std::mem::size_of::<Self>()
            .checked_add(128)?
            .checked_add(extra)
    }
}

fn semantic(text: &str) -> Result<String, Diagnostic> {
    super::ast_adapter::semantic_name(text).ok_or_else(|| fault("invalid captured constant name"))
}

#[cfg(test)]
#[path = "constant_length_fixture.rs"]
mod length_fixture;
#[cfg(test)]
pub(crate) use length_fixture::length_sites as fixture_length_sites;
