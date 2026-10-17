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
    ast::{
        self, HasArgList, HasAttrs, HasGenericArgs, HasGenericParams, HasLoopBody, HasModuleItem,
        HasName,
    },
};
use std::{collections::BTreeMap, rc::Rc};
#[path = "constant_parameter_lengths.rs"]
mod parameter_lengths;
use parameter_lengths::ParameterIndependentLength;
#[path = "constant_loops.rs"]
mod loops;
pub(crate) use loops::ControlTarget;
#[path = "constant_local_constraints.rs"]
mod local_constraints;
#[derive(Debug)]
pub(crate) struct LocalNumericContext {
    primitive: PrimitiveType,
    work: usize,
    metadata: usize,
}
impl LocalNumericContext {
    pub(crate) fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    pub(crate) fn work(&self) -> usize {
        self.work
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata
    }
}
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
    /// A body-local item of an enclosing block: selected by the entered
    /// lexical scope, never by the module's Value namespace.
    local: bool,
}
impl ConstantDeclaration {
    pub fn is_local(&self) -> bool {
        self.local
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_anonymous(&self) -> bool {
        self.name.is_empty()
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
    parsed: std::sync::Arc<ParsedFile>,
    cfg: CfgSet,
    crate_key: Digest,
    scope: ScopeKey,
    selection: Option<SourceSelection>,
    module_items: Vec<ast::Item>,
    /// Captured body-local items of the blocks enclosing a local constant.
    local_items: Vec<ast::Item>,
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
    /// An immutable static's initializer: a const context in rustc whose
    /// value a read of the static observes.
    Static(ast::Static),
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
    pub(crate) params: Vec<(String, PrimitiveType, bool)>,
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
    Let(String, bool, Option<PrimitiveType>, ExprRef),
    Expression(ExprRef),
    Declaration,
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
    Assign(String, Option<NumericBinary>, ExprRef),
    Logic(bool, ExprRef, ExprRef),
    Cast(ExprRef, PrimitiveType),
    If(ExprRef, ExprRef, Option<ExprRef>),
    Block(Vec<StatementRef>, Option<ExprRef>, Option<ControlTarget>),
    Return(Option<ExprRef>),
    Loop(ControlTarget, Option<ExprRef>, ExprRef),
    Break(ControlTarget, Option<ExprRef>),
    Continue(ControlTarget),
    Call(String, Vec<ExprRef>),
    Array(Vec<ExprRef>),
    Repeat(ExprRef, ExprRef),
    Panic,
    AdmittedPanic(AdmittedConstantPanic),
    RequiredMacro(Box<RequiredConstantMacro>),
    /// A reached form outside the specified pure constant domain.
    OutsideDomain(&'static str),
    Unsupported,
}
impl ConstantSource {
    pub(crate) fn numeric_local_context(
        &self,
        statement: &StatementRef,
        work_limit: usize,
        byte_limit: usize,
    ) -> Result<LocalNumericContext, Diagnostic> {
        local_constraints::select(self, statement, work_limit, byte_limit)
    }
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
            let item = item.clone();
            return Self::capture_associated(view, parsed, cfg, outline, item);
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
        // A `static mut` initializer is a const context like any static's;
        // only reads of a `static mut` from other constants are refused.
        if !matches!(syntax, ast::Item::Const(_) | ast::Item::Fn(_) | ast::Item::Static(_)) {
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
    /// An associated constant of a non-generic impl or trait. Its
    /// initializer names resolve in the enclosing module, exactly as rustc
    /// resolves them (an impl or trait introduces no item namespace); the
    /// member itself is selected by the member index, never by name here.
    fn capture_associated(
        view: &PreparedSourceView,
        parsed: ParsedFile,
        cfg: CfgSet,
        outline: super::outline::Outline,
        item: super::outline::ItemOutline,
    ) -> Result<Self, Diagnostic> {
        let selected = view.selection();
        let module_scope = ScopeKey {
            crate_key: item.scope.crate_key,
            module_path: item.scope.module_path.clone(),
            owner: None,
        };
        view.captured_inline_module_layout(&module_scope)?;
        let body = item
            .body
            .clone()
            .ok_or_else(|| fault("associated constant has no initializer"))?;
        let syntax = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Item::cast)
            .find(|i| range(i.syntax().text_range()) == item.declaration_span)
            .ok_or_else(|| fault("prepared constant declaration syntax absent"))?;
        let ast::Item::Const(constant) = &syntax else {
            return Err(fault("associated constant plan requires a direct const"));
        };
        let owner = required(syntax.syntax().parent())
            .ok()
            .filter(|list| ast::AssocItemList::can_cast(list.kind()))
            .and_then(|list| list.parent())
            .ok_or_else(|| fault("associated constant owner is not an impl or trait"))?;
        let generic = if let Some(owner) = ast::Impl::cast(owner.clone()) {
            owner.generic_param_list().is_some() || owner.where_clause().is_some()
        } else if let Some(owner) = ast::Trait::cast(owner.clone()) {
            owner.generic_param_list().is_some() || owner.where_clause().is_some()
        } else {
            return Err(fault("associated constant owner is not an impl or trait"));
        };
        if generic {
            return Err(fault(
                "associated constant of a generic owner requires its substitution",
            ));
        }
        let module = owner
            .parent()
            .filter(|parent| {
                ast::SourceFile::can_cast(parent.kind()) || ast::ItemList::can_cast(parent.kind())
            })
            .ok_or_else(|| fault("associated constant owner is not a captured module item"))?;
        let items = module.children().filter_map(ast::Item::cast).collect();
        let mut captured = Self::capture_scope(
            parsed,
            cfg,
            selected.outline.crate_key,
            outline,
            module_scope,
            Some(selected.clone()),
            items,
        )?;
        let inner = Rc::get_mut(&mut captured.0)
            .ok_or_else(|| fault("associated constant capture requires a fresh capture"))?;
        let at = item.declaration_span;
        let name = semantic(required(constant.name())?.text())?;
        inner.declarations.push(ConstantDeclaration {
            name,
            range: at,
            definition: item.def_key.fingerprint(),
            body: body.key(),
            blocked: body.entry_blocked,
            context: DeclarationContext {
                scope: item.scope.clone(),
                range: at,
            },
            handle: body.clone(),
            local: false,
        });
        inner.owners.push((at, item.def_key.fingerprint(), Some(body)));
        inner.local_items.push(syntax);
        inner.scope = item.scope;
        Ok(captured)
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
            let name = match item {
                ast::Item::Const(c) if c.underscore_token().is_some() => String::new(),
                ast::Item::Const(c) => semantic(required(c.name())?.text())?,
                ast::Item::Static(s) => semantic(required(s.name())?.text())?,
                ast::Item::Fn(f) => semantic(required(f.name())?.text())?,
                _ => continue,
            };
            // A `static mut` is captured for its own initializer, but a
            // constant expression can never name (read) it.
            let const_visible =
                !matches!(item, ast::Item::Static(s) if s.mut_token().is_some());
            let Some(body) = header.body.as_ref() else {
                continue;
            };
            declarations
                .try_reserve(1)
                .map_err(|_| fault("constant declaration allocation"))?;
            declarations.push(ConstantDeclaration {
                name: name.clone(),
                range: at,
                definition: header.def_key.fingerprint(),
                body: body.key(),
                blocked: body.entry_blocked,
                context: DeclarationContext {
                    scope: header.scope.clone(),
                    range: at,
                },
                handle: body.clone(),
                local: false,
            });
            if !name.is_empty() && const_visible {
                items.entry(name).or_default().push(item.clone());
            }
        }
        Ok(Self(Rc::new(Captured {
            parsed: std::sync::Arc::new(parsed),
            cfg,
            crate_key,
            scope,
            selection,
            module_items,
            local_items: Vec::new(),
            type_certificate: None,
            macro_proofs: BTreeMap::new(),
            declarations,
            items,
            owners,
        })))
    }
    /// Capture a body-local constant or static in its lexical scope, as
    /// rustc resolves a const context inside a function body: the items of
    /// every enclosing block (innermost first, each visible throughout its
    /// block, never a `let` binding), then the enclosing module's items.
    /// `rows` are the owning body's actual entered item rows; `scopes` its
    /// block scopes. Bodies are not typechecked or evaluated here.
    /// `parsed` is the owning body's actual admitted source (the original
    /// file, or its retained assembly with attached macro outputs): the
    /// coordinates of `rows` and of the captured syntax.
    pub(crate) fn capture_local(
        view: &PreparedSourceView,
        parsed: std::sync::Arc<ParsedFile>,
        definition: Digest,
        rows: &[crate::syntax::body_items::BodyItem],
        scopes: &[crate::syntax::entered_macros::BodyMacroScope],
    ) -> Result<Self, Diagnostic> {
        view.require_original_hygiene()?;
        view.require_declaration_syntax()?;
        let selected = view.selection();
        let cfg = view.captured_macro_cfg()?.clone();
        if parsed.profile != selected.outline.parse.profile {
            return Err(fault("local constant owning parse profile differs"));
        }
        // Module item identities and headers of the owning file, in the
        // owning parse's own coordinates.
        let outline = parsed.outline(
            selected.outline.crate_key,
            selected.outline.module_path.clone(),
            &cfg,
        )?;
        let mut owned = rows
            .iter()
            .filter(|row| row.item.def_key.fingerprint() == definition);
        let row = owned
            .next()
            .ok_or_else(|| fault("local constant declaration absent from its owning body"))?;
        if owned.next().is_some() || row.attribute_input.is_some() {
            return Err(fault("local constant declaration requires attribute preparation"));
        }
        let local_scope = row.item.scope.clone();
        if local_scope.owner.is_none() {
            return Err(fault("local constant scope has no owning body"));
        }
        let module_scope = ScopeKey {
            crate_key: local_scope.crate_key,
            module_path: local_scope.module_path.clone(),
            owner: None,
        };
        view.captured_inline_module_layout(&module_scope)?;
        let tree = parsed.parsed.tree();
        let item_at = |at: SourceRange| -> Result<ast::Item, Diagnostic> {
            tree.syntax()
                .descendants()
                .filter_map(ast::Item::cast)
                .find(|i| range(i.syntax().text_range()) == at)
                .ok_or_else(|| fault("local constant declaration syntax absent"))
        };
        let syntax = item_at(row.item.declaration_span)?;
        if !matches!(syntax, ast::Item::Const(_) | ast::Item::Static(_))
            || !syntax
                .syntax()
                .parent()
                .is_some_and(|parent| ast::StmtList::can_cast(parent.kind()))
        {
            return Err(fault("local constant plan requires a direct block const/static"));
        }
        // The enclosing module's own item list (the root file or an inline
        // module body); impls/traits introduce no module namespace.
        let module = syntax
            .syntax()
            .ancestors()
            .find(|node| {
                ast::SourceFile::can_cast(node.kind())
                    || (ast::ItemList::can_cast(node.kind())
                        && node.parent().is_some_and(|p| ast::Module::can_cast(p.kind())))
            })
            .ok_or_else(|| fault("local constant enclosing module absent"))?;
        let module_items: Vec<ast::Item> = module.children().filter_map(ast::Item::cast).collect();
        // Enclosing block scopes, innermost first.
        let mut chain = Vec::new();
        let mut current = Some(row.scope);
        while let Some(ordinal) = current {
            if chain.contains(&ordinal) {
                return Err(fault("local constant block ancestry cycle"));
            }
            chain.push(ordinal);
            current = scopes
                .iter()
                .find(|scope| scope.ordinal == ordinal)
                .ok_or_else(|| fault("local constant block scope absent"))?
                .parent;
        }
        let mut declarations = Vec::new();
        let mut items: BTreeMap<String, Vec<ast::Item>> = BTreeMap::new();
        let mut local_items = Vec::new();
        let mut owners = Vec::new();
        for ordinal in &chain {
            let mut block: BTreeMap<String, ast::Item> = BTreeMap::new();
            for local in rows.iter().filter(|local| local.scope == *ordinal) {
                if !matches!(
                    local.item.def_key.item_kind,
                    crate::source::manifest::ItemKind::Constant
                        | crate::source::manifest::ItemKind::Static
                        | crate::source::manifest::ItemKind::Function
                ) || local.attribute_input.is_some()
                {
                    continue;
                }
                let Some(body) = local.item.body.as_ref() else {
                    continue;
                };
                let at = local.item.declaration_span;
                let item = item_at(at)?;
                let name = match &item {
                    ast::Item::Const(c) if c.underscore_token().is_some() => String::new(),
                    ast::Item::Const(c) => semantic(required(c.name())?.text())?,
                    ast::Item::Static(s) => semantic(required(s.name())?.text())?,
                    ast::Item::Fn(f) => semantic(required(f.name())?.text())?,
                    _ => return Err(fault("local constant row syntax kind differs")),
                };
                let const_visible =
                    !matches!(&item, ast::Item::Static(s) if s.mut_token().is_some());
                declarations.push(ConstantDeclaration {
                    name: name.clone(),
                    range: at,
                    definition: local.item.def_key.fingerprint(),
                    body: body.key(),
                    blocked: body.entry_blocked,
                    context: DeclarationContext {
                        scope: local.item.scope.clone(),
                        range: at,
                    },
                    handle: body.clone(),
                    local: true,
                });
                owners.push((at, local.item.def_key.fingerprint(), Some(body.clone())));
                if !name.is_empty() && const_visible && !items.contains_key(&name) {
                    // Duplicate names in one block are an ambiguity rustc rejects.
                    if block.insert(name.clone(), item.clone()).is_some() {
                        return Err(fault("local constant block declares a name twice"));
                    }
                }
                local_items.push(item);
            }
            for (name, item) in block {
                items.insert(name, vec![item]);
            }
        }
        for item in &module_items {
            let at = range(item.syntax().text_range());
            let Some(header) = outline
                .items
                .iter()
                .find(|h| h.declaration_span == at && h.scope == module_scope)
            else {
                continue;
            };
            let name = match item {
                ast::Item::Const(c) if c.underscore_token().is_some() => String::new(),
                ast::Item::Const(c) => semantic(required(c.name())?.text())?,
                ast::Item::Static(s) => semantic(required(s.name())?.text())?,
                ast::Item::Fn(f) => semantic(required(f.name())?.text())?,
                _ => continue,
            };
            let const_visible = !matches!(item, ast::Item::Static(s) if s.mut_token().is_some());
            let Some(body) = header.body.as_ref() else {
                continue;
            };
            declarations.push(ConstantDeclaration {
                name: name.clone(),
                range: at,
                definition: header.def_key.fingerprint(),
                body: body.key(),
                blocked: body.entry_blocked,
                context: DeclarationContext {
                    scope: header.scope.clone(),
                    range: at,
                },
                handle: body.clone(),
                local: false,
            });
            owners.push((at, header.def_key.fingerprint(), Some(body.clone())));
            // A block item shadows a same-named module item.
            if !name.is_empty() && const_visible && !items.contains_key(&name) {
                items.entry(name).or_default().push(item.clone());
            }
        }
        Ok(Self(Rc::new(Captured {
            parsed,
            cfg,
            crate_key: selected.outline.crate_key,
            scope: local_scope,
            selection: Some(selected.clone()),
            module_items,
            local_items,
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
        let total = self
            .metadata_bytes()?
            .checked_add(proof.metadata_bytes())
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
            .count();
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
            .module_items
            .iter()
            .chain(&self.0.local_items)
            .find(|i| range(i.syntax().text_range()) == at)
            .cloned()
            .ok_or_else(|| fault("captured declaration absent"))
    }
    pub fn constant(&self, at: SourceRange) -> Result<ConstantPlan, Diagnostic> {
        match self.exact(at)? {
            ast::Item::Const(c) => {
                self.constant_header(&c)?;
                self.plan(Entry::Constant(c), at)
            }
            ast::Item::Static(s) => {
                self.static_header(&s)?;
                self.plan(Entry::Static(s), at)
            }
            _ => Err(fault("selected declaration is not a constant")),
        }
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
        for ((_, ty, _), arg) in header.params.iter().zip(&args) {
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
                // A length inside a captured associated constant of a
                // non-generic impl/trait (`capture_associated`) resolves in
                // the enclosing module like its initializer.
                let associated = self.0.declarations.iter().any(|declaration| {
                    declaration.context.scope.owner.is_some()
                        && declaration.range.start <= at.start
                        && at.end <= declaration.range.end
                });
                for ancestor in expr.syntax().ancestors() {
                    if (ast::Module::can_cast(ancestor.kind()) && self.0.selection.is_none())
                        || (!associated
                            && (ast::Impl::can_cast(ancestor.kind())
                                || ast::Trait::can_cast(ancestor.kind())))
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
                            // Builtin inert and tool attributes (`#![allow(..)]`)
                            // are never macro calls; the declaration scan records
                            // no producer or proof for them.
                            let path: String = meta
                                .path()
                                .map(|path| path.syntax().text().to_string())
                                .unwrap_or_default()
                                .chars()
                                .filter(|c| !c.is_whitespace())
                                .collect();
                            !(super::outline::is_builtin_inert_attribute(&path)
                                || super::outline::is_tool_attribute(&path))
                        })
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
            Entry::Static(_) => e.u8(4),
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
            return Err(match ty {
                ast::Type::RefType(_) => outside_domain("a reference type"),
                ast::Type::PtrType(_) => outside_domain("a raw pointer type"),
                ast::Type::FnPtrType(_) => outside_domain("a function pointer type"),
                ast::Type::SliceType(_) => outside_domain("a slice type"),
                _ => fault("constant type is not a handwritten primitive"),
            });
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
            _ => {
                // A nominal (struct/enum/union) type of the captured module.
                return Err(if self.names_nominal(&name) {
                    outside_domain("a nominal type")
                } else {
                    fault("constant primitive type identity is unavailable")
                });
            }
        };
        // Conservative captured lexical proof. Imports/producers never grant primitive identity.
        // Block scopes (and generic parameters) inside the declaration's module
        // are scanned. A module's own Type namespace — its items, imports and
        // producers — is the current certificate's answer, when one is
        // attached; enclosing modules' items are never in scope in Rust.
        let mut work = 0usize;
        for scope in ty.syntax().ancestors() {
            let module_scope = ast::SourceFile::can_cast(scope.kind())
                || (ast::ItemList::can_cast(scope.kind())
                    && scope.parent().is_some_and(|p| ast::Module::can_cast(p.kind())));
            if module_scope && self.0.type_certificate.is_some() {
                break;
            }
            for child in scope.children() {
                work = work
                    .checked_add(1)
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
    /// An immutable static's initializer is evaluated exactly like a
    /// constant body; its declared type is the result type.
    fn static_header(&self, s: &ast::Static) -> Result<Constant, Diagnostic> {
        self.attributes_clear(s.syntax())?;
        if self.declaration(s)?.blocked {
            return Err(fault("static initializer boundaries are blocked"));
        }
        Ok(Constant {
            declaration: self.declaration(s)?,
            ty: self.ty(required(s.ty())?)?,
            body: ExprRef(required(s.body())?),
        })
    }
    fn function_header(&self, f: &ast::Fn) -> Result<Function, Diagnostic> {
        self.attributes_clear(f.syntax())?;
        if self.declaration(f)?.blocked {
            return Err(fault("const fn body boundaries are blocked"));
        }
        // `const unsafe fn` is an ordinary const fn to the evaluator; the
        // `unsafe` obligation is the caller's, checked by rustc.
        if f.const_token().is_none()
            || f.generic_param_list().is_some()
            || f.async_token().is_some()
        {
            return Err(Diagnostic::new(
                "LRR-E-CONST-EFFECT",
                "reached call is not a supported pure const fn".into(),
            ));
        }
        let mut params = Vec::new();
        for p in required(f.param_list())?.params() {
            let (name, mutable) = local_binding(required(p.pat())?)?;
            let ty = self.primitive(required(p.ty())?)?;
            if params.iter().any(|(n, _, _)| n == &name) {
                return Err(fault("duplicate constant parameter"));
            }
            params.push((name, ty, mutable));
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
        match self.named(name)? {
            ast::Item::Const(c) => self.constant_header(&c),
            // A const context may read an immutable static's value
            // (rustc-checkout tests/ui/static/refer-to-other-statics-by-value.rs);
            // a `static mut` read is never const-evaluable.
            ast::Item::Static(s) if s.mut_token().is_none() => self.static_header(&s),
            _ => Err(fault("reached name is not a named constant")),
        }
    }
    pub(crate) fn named_function(&self, name: &str) -> Result<Function, Diagnostic> {
        let i = self.named(name)?;
        let ast::Item::Fn(f) = i else {
            return Err(fault("reached callee is not const fn"));
        };
        self.function_header(&f)
    }
    fn named(&self, name: &str) -> Result<ast::Item, Diagnostic> {
        if name.is_empty() {
            return Err(fault("anonymous constants have no named lookup identity"));
        }
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
        let items = self.0.items.get(name).ok_or_else(|| {
            if self.names_nominal(name) {
                outside_domain("a struct value or constructor")
            } else {
                fault("reached constant name is absent")
            }
        })?;
        if items.len() != 1 {
            return Err(fault("reached constant name is ambiguous"));
        }
        Ok(items[0].clone())
    }
    /// A single-name value path. A path through a nominal type (an enum
    /// variant, associated constant or function) leaves the pure domain;
    /// other qualified paths still need captured namespace routing.
    fn value_path_name(&self, path: ast::Path) -> Result<String, Diagnostic> {
        if let Some(qualifier) = path.qualifier() {
            let mut first = qualifier;
            while let Some(next) = first.qualifier() {
                first = next;
            }
            let through_type = first.segment().is_some_and(|segment| {
                segment.type_anchor().is_some()
                    || segment.kind() == Some(ast::PathSegmentKind::SelfTypeKw)
                    || segment.name_ref().is_some_and(|name| {
                        semantic(name.text()).is_ok_and(|name| self.names_nominal(&name))
                    })
            });
            if through_type {
                return Err(outside_domain("an enum variant or associated item path"));
            }
        }
        bare(path)
    }
    /// Whether a captured module or enclosing block declares a struct,
    /// enum or union `name` (a nominal value, outside the pure domain).
    fn names_nominal(&self, name: &str) -> bool {
        self.0
            .module_items
            .iter()
            .chain(&self.0.local_items)
            .any(|item| {
                let declared = match item {
                    ast::Item::Struct(x) => x.name(),
                    ast::Item::Enum(x) => x.name(),
                    ast::Item::Union(x) => x.name(),
                    _ => None,
                };
                declared.is_some_and(|n| semantic(n.text()).is_ok_and(|n| n == name))
            })
    }
    pub(crate) fn statement(&self, s: &StatementRef) -> Result<Statement, Diagnostic> {
        Ok(match &s.0 {
            ast::Stmt::LetStmt(l) => {
                self.attributes_clear(l.syntax())?;
                if l.let_else().is_some() {
                    Statement::Unsupported(s.range())
                } else {
                    let (name, mutable) = local_binding(required(l.pat())?)?;
                    Statement::Let(
                        name,
                        mutable,
                        l.ty().map(|t| self.primitive(t)).transpose()?,
                        ExprRef(required(l.initializer())?),
                    )
                }
            }
            ast::Stmt::ExprStmt(s) => Statement::Expression(ExprRef(required(s.expr())?)),
            ast::Stmt::Item(item)
                if !matches!(item, ast::Item::MacroCall(_) | ast::Item::Use(_)) =>
            {
                self.attributes_clear(item.syntax())?;
                // Declarations perform no block-time evaluation. A later
                // lookup still requires the actual local source member.
                Statement::Declaration
            }
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
                let name = match &item {
                    ast::Item::Const(i) => i.name(),
                    ast::Item::Static(i) => i.name(),
                    ast::Item::Fn(i) => i.name(),
                    ast::Item::Struct(i) => i.name(),
                    _ => None,
                };
                if name.is_some_and(|n| semantic(n.text()).is_ok_and(|n| n == wanted)) {
                    // The innermost block item of this name is the captured
                    // lexical selection of a body-local constant's scope.
                    let at = range(item.syntax().text_range());
                    if self
                        .0
                        .items
                        .get(wanted)
                        .is_some_and(|selected| {
                            selected.len() == 1 && range(selected[0].syntax().text_range()) == at
                        })
                        && self
                            .0
                            .local_items
                            .iter()
                            .any(|local| range(local.syntax().text_range()) == at)
                    {
                        return Ok(());
                    }
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
                let name = self.value_path_name(required(p.path())?)?;
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
                    ast::BinaryOp::Assignment { op } => {
                        let ast::Expr::PathExpr(path) = &left.0 else {
                            return Ok(Node::Unsupported);
                        };
                        self.attributes_clear(path.syntax())?;
                        let name = bare(required(path.path())?)?;
                        self.lexical_value_clear(path.syntax(), &name)?;
                        let op = op.map(|op| match op {
                            ast::ArithOp::Add => NumericBinary::Add,
                            ast::ArithOp::Sub => NumericBinary::Sub,
                            ast::ArithOp::Mul => NumericBinary::Mul,
                            ast::ArithOp::Div => NumericBinary::Div,
                            ast::ArithOp::Rem => NumericBinary::Rem,
                            ast::ArithOp::Shl => NumericBinary::Shl,
                            ast::ArithOp::Shr => NumericBinary::Shr,
                            ast::ArithOp::BitAnd => NumericBinary::BitAnd,
                            ast::ArithOp::BitOr => NumericBinary::BitOr,
                            ast::ArithOp::BitXor => NumericBinary::BitXor,
                        });
                        Node::Assign(name, op, right)
                    }
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
                // `unsafe { .. }` and `const { .. }` blocks evaluate as
                // ordinary blocks in a const context.
                if b.async_token().is_some()
                    || b.gen_token().is_some()
                    || b.try_block_modifier().is_some()
                {
                    return Ok(Node::Unsupported);
                }
                let list = required(b.stmt_list())?;
                let mut stmts = Vec::new();
                for s in list.statements() {
                    stmts.push(StatementRef(s));
                }
                Node::Block(
                    stmts,
                    list.tail_expr().map(ExprRef),
                    b.label().map(|_| ControlTarget::block_expression(b)),
                )
            }
            E::LoopExpr(loop_expression) => Node::Loop(
                ControlTarget::loop_expression(e),
                None,
                ExprRef(E::BlockExpr(required(loop_expression.loop_body())?)),
            ),
            E::WhileExpr(loop_expression) => Node::Loop(
                ControlTarget::loop_expression(e),
                Some(ExprRef(required(loop_expression.condition())?)),
                ExprRef(E::BlockExpr(required(loop_expression.loop_body())?)),
            ),
            E::BreakExpr(exit) => Node::Break(
                loops::reached_target(exit.syntax(), exit.lifetime())?,
                exit.expr().map(ExprRef),
            ),
            E::ContinueExpr(exit) => Node::Continue(loops::reached_continue_target(
                exit.syntax(),
                exit.lifetime(),
            )?),
            E::ReturnExpr(r) => Node::Return(r.expr().map(ExprRef)),
            E::CallExpr(c) => {
                let E::PathExpr(p) = required(c.expr())? else {
                    return Ok(Node::Unsupported);
                };
                let args = required(c.arg_list())?.args().map(ExprRef).collect();
                let name = self.value_path_name(required(p.path())?)?;
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
            // Spec §18.3 keeps nominal values, references and method
            // selection out of the pure evaluator; their const-context
            // evaluation awaits the const-evaluation design decision.
            E::RecordExpr(_) => Node::OutsideDomain("a struct or variant value"),
            E::MethodCallExpr(_) => Node::OutsideDomain("a method call"),
            E::RefExpr(_) => Node::OutsideDomain("a reference value"),
            E::FieldExpr(_) => Node::OutsideDomain("a field projection"),
            E::MatchExpr(_) => Node::OutsideDomain("a match expression"),
            E::ClosureExpr(_) => Node::OutsideDomain("a closure value"),
            _ => Node::Unsupported,
        })
    }
}
/// The diagnostic for a constant whose initializer or type leaves the
/// specified pure constant domain (spec §18.3: scalars, strings, tuples
/// and arrays). Its const-context evaluation awaits the const-evaluation
/// design decision; it is never approximated here.
pub(crate) fn outside_domain(what: &str) -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CONST-UNSUPPORTED",
        format!(
            "constant initializer outside the pure constant domain ({what}: nominal values, references and method calls await the const-evaluation design decision)"
        ),
    )
}
fn local_binding(pat: ast::Pat) -> Result<(String, bool), Diagnostic> {
    let ast::Pat::IdentPat(p) = pat else {
        return Err(fault("constant local requires a scalar identifier"));
    };
    if p.ref_token().is_some() || p.pat().is_some() {
        return Err(fault("constant local binding mode unsupported"));
    }
    Ok((
        semantic(required(p.name())?.text())?,
        p.mut_token().is_some(),
    ))
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
            Entry::Static(s) => (Some(self.source.static_header(s)?), None, None),
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
            Entry::Static(s) => range(s.syntax().text_range()),
            Entry::Call(f, _) => range(f.syntax().text_range()),
            Entry::Length(e) => e.range(),
        }
    }
}

impl Node {
    pub(crate) fn owned_bytes(&self) -> Option<usize> {
        let extra = match self {
            Self::Path(s) | Self::Float(s, _) | Self::Assign(s, _, _) => s.capacity(),
            Self::RequiredMacro(_) => std::mem::size_of::<RequiredConstantMacro>(),
            Self::Block(s, _, _) => s
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

#[cfg(test)]
#[path = "constant_labelled_block_tests.rs"]
mod labelled_block_tests;
