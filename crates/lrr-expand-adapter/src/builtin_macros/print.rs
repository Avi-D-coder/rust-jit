//! Port of rustc's AST expression printer used by `pprust::expr_to_string`
//! (`rustc_ast_pretty/src/pprust/state.rs`, `state/expr.rs`, `state/fixup.rs`,
//! `rustc_ast/src/util/{parser,classify}.rs`). Only the printing-relevant
//! structure is modelled; see `ast.rs`.
use super::ast::*;
use super::pp::{BoxMarker, Breaks, Printer};
use super::tts::{self, Delim, INDENT_UNIT, MacHeader};

/// rustc `FixupContext`.
#[derive(Copy, Clone, Debug, Default)]
struct Fixup {
    stmt: bool,
    leftmost_subexpression_in_stmt: bool,
    match_arm: bool,
    leftmost_subexpression_in_match_arm: bool,
    parenthesize_exterior_struct_lit: bool,
    next_operator_can_begin_expr: bool,
    next_operator_can_continue_expr: bool,
}

impl Fixup {
    fn new_stmt() -> Self {
        Fixup {
            stmt: true,
            ..Fixup::default()
        }
    }
    fn new_match_arm() -> Self {
        Fixup {
            match_arm: true,
            ..Fixup::default()
        }
    }
    fn new_cond() -> Self {
        Fixup {
            parenthesize_exterior_struct_lit: true,
            ..Fixup::default()
        }
    }
    fn leftmost_subexpression(self) -> Self {
        Fixup {
            stmt: false,
            leftmost_subexpression_in_stmt: self.stmt || self.leftmost_subexpression_in_stmt,
            match_arm: false,
            leftmost_subexpression_in_match_arm: self.match_arm
                || self.leftmost_subexpression_in_match_arm,
            next_operator_can_begin_expr: false,
            next_operator_can_continue_expr: true,
            ..self
        }
    }
    fn leftmost_subexpression_with_dot(self) -> Self {
        Fixup {
            stmt: self.stmt || self.leftmost_subexpression_in_stmt,
            leftmost_subexpression_in_stmt: false,
            match_arm: self.match_arm || self.leftmost_subexpression_in_match_arm,
            leftmost_subexpression_in_match_arm: false,
            next_operator_can_begin_expr: false,
            next_operator_can_continue_expr: true,
            ..self
        }
    }
    fn leftmost_subexpression_with_operator(self, next_operator_can_begin_expr: bool) -> Self {
        Fixup {
            next_operator_can_begin_expr,
            ..self.leftmost_subexpression()
        }
    }
    fn rightmost_subexpression(self) -> Self {
        Fixup {
            stmt: false,
            leftmost_subexpression_in_stmt: false,
            match_arm: false,
            leftmost_subexpression_in_match_arm: false,
            ..self
        }
    }
    fn would_cause_statement_boundary(self, expr: &Expr) -> bool {
        (self.leftmost_subexpression_in_stmt && !expr_requires_semi_to_be_stmt(expr))
            || (self.leftmost_subexpression_in_match_arm && expr_is_complete(expr))
    }
    fn needs_par_as_let_scrutinee(self, expr: &Expr) -> bool {
        self.parenthesize_exterior_struct_lit && contains_exterior_struct_lit(expr)
            || needs_par_as_let_scrutinee(self.precedence(expr))
    }
    fn precedence(self, expr: &Expr) -> Prec {
        if self.next_operator_can_begin_expr
            && matches!(
                expr,
                Expr::Break(..) | Expr::Ret(..) | Expr::Yeet(..) | Expr::Yield(..)
            )
        {
            return Prec::Jump;
        }
        if !self.next_operator_can_continue_expr
            && matches!(
                expr,
                Expr::Break(..)
                    | Expr::Closure(..)
                    | Expr::Ret(..)
                    | Expr::Yeet(..)
                    | Expr::Yield(..)
                    | Expr::Range(None, ..)
            )
        {
            return Prec::Prefix;
        }
        expr.precedence()
    }
}

fn needs_par_as_let_scrutinee(order: Prec) -> bool {
    order <= Prec::LAnd
}

/// rustc `parser::contains_exterior_struct_lit`.
fn contains_exterior_struct_lit(value: &Expr) -> bool {
    match value {
        Expr::Struct(..) => true,
        Expr::Assign(lhs, rhs) | Expr::AssignOp(_, lhs, rhs) | Expr::Binary(_, lhs, rhs) => {
            contains_exterior_struct_lit(lhs) || contains_exterior_struct_lit(rhs)
        }
        Expr::Await(x)
        | Expr::Unary(_, x)
        | Expr::Cast(x, _)
        | Expr::Field(x, _)
        | Expr::Index(x, _) => contains_exterior_struct_lit(x),
        Expr::MethodCall { receiver, .. } => contains_exterior_struct_lit(receiver),
        _ => false,
    }
}

/// rustc `classify::expr_is_complete`.
fn expr_is_complete(e: &Expr) -> bool {
    matches!(
        e,
        Expr::If(..)
            | Expr::Match(..)
            | Expr::Block(..)
            | Expr::While(..)
            | Expr::Loop(..)
            | Expr::ForLoop { .. }
            | Expr::TryBlock(..)
            | Expr::ConstBlock(..)
    )
}

/// rustc `classify::expr_requires_semi_to_be_stmt`.
fn expr_requires_semi_to_be_stmt(e: &Expr) -> bool {
    match e {
        Expr::MacCall(mac) => mac.delim != Delim::Brace,
        _ => !expr_is_complete(e),
    }
}

/// rustc `classify::leading_labeled_expr`.
fn leading_labeled_expr(mut expr: &Expr) -> bool {
    loop {
        match expr {
            Expr::Block(_, label) | Expr::Loop(_, label) | Expr::While(_, _, label) => {
                return label.is_some();
            }
            Expr::ForLoop { label, .. } => return label.is_some(),
            Expr::Assign(e, _)
            | Expr::AssignOp(_, e, _)
            | Expr::Await(e)
            | Expr::Binary(_, e, _)
            | Expr::Call(e, _)
            | Expr::Cast(e, _)
            | Expr::Field(e, _)
            | Expr::Index(e, _)
            | Expr::Range(Some(e), _, _)
            | Expr::Try(e) => expr = e,
            Expr::MethodCall { receiver, .. } => expr = receiver,
            _ => return false,
        }
    }
}

/// rustc `classify::expr_trailing_brace` (presence only).
fn expr_has_trailing_brace(mut expr: &Expr) -> bool {
    loop {
        match expr {
            Expr::AddrOf { expr: e, .. }
            | Expr::Assign(_, e)
            | Expr::AssignOp(_, _, e)
            | Expr::Binary(_, _, e)
            | Expr::Break(_, Some(e))
            | Expr::Let(_, e)
            | Expr::Range(_, Some(e), _)
            | Expr::Ret(Some(e))
            | Expr::Unary(_, e)
            | Expr::Yeet(Some(e))
            | Expr::Become(e)
            | Expr::Yield(Some(e)) => expr = e,
            Expr::Closure(c) => expr = &c.body,
            Expr::Gen(..)
            | Expr::Block(..)
            | Expr::ForLoop { .. }
            | Expr::If(..)
            | Expr::Loop(..)
            | Expr::Match(..)
            | Expr::Struct(..)
            | Expr::TryBlock(..)
            | Expr::While(..)
            | Expr::ConstBlock(..) => return true,
            Expr::Cast(_, ty) => return type_trailing_braced_mac_call(ty),
            Expr::MacCall(mac) => return mac.delim == Delim::Brace,
            _ => return false,
        }
    }
}

fn type_trailing_braced_mac_call(mut ty: &Ty) -> bool {
    loop {
        match ty {
            Ty::MacCall(mac) => return mac.delim == Delim::Brace,
            Ty::Ptr(_, inner) | Ty::Ref(_, _, inner) => ty = inner,
            Ty::FnPtr {
                output: Some(ret), ..
            } => ty = ret,
            Ty::FnPtr { output: None, .. } => return false,
            Ty::Path(_, path) => match path_return_type(path) {
                Some(ret) => ty = ret,
                None => return false,
            },
            Ty::TraitObject(_, bounds) | Ty::ImplTrait(bounds) => match bounds.last() {
                Some(Bound::Trait {
                    parens: false,
                    path,
                    ..
                }) => match path_return_type(path) {
                    Some(ret) => ty = ret,
                    None => return false,
                },
                _ => return false,
            },
            _ => return false,
        }
    }
}

fn path_return_type(path: &Path) -> Option<&Ty> {
    match &path.segments.last()?.args {
        Some(GenericArgs::Paren {
            output: Some(ret), ..
        }) => Some(ret),
        _ => None,
    }
}

/// rustc `expr_ends_with_dot`.
fn expr_ends_with_dot(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(text) => {
            text.ends_with('.') && text.chars().next().is_some_and(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

pub(crate) struct State {
    p: Printer,
}

impl State {
    pub(crate) fn new() -> Self {
        State { p: Printer::new() }
    }

    pub(crate) fn finish(self) -> String {
        self.p.eof()
    }

    /// rustc `pprust::expr_to_string`.
    pub(crate) fn expr_to_string(expr: &Expr) -> String {
        let mut s = State::new();
        s.print_expr(expr, Fixup::default());
        s.finish()
    }

    // ----- generic helpers --------------------------------------------

    fn strsep<T>(
        &mut self,
        sep: &'static str,
        space_before: bool,
        b: Breaks,
        elts: &[T],
        mut op: impl FnMut(&mut Self, &T),
    ) {
        let rb = self.p.rbox(0, b);
        let mut it = elts.iter();
        if let Some(first) = it.next() {
            op(self, first);
            for elt in it {
                if space_before {
                    self.p.space();
                }
                self.p.word_space(sep);
                op(self, elt);
            }
        }
        self.p.end(rb);
    }

    fn commasep<T>(&mut self, b: Breaks, elts: &[T], op: impl FnMut(&mut Self, &T)) {
        self.strsep(",", false, b, elts, op)
    }

    /// rustc `commasep_cmnt` (no comments are retained in macro input).
    fn commasep_cmnt<T>(&mut self, b: Breaks, elts: &[T], mut op: impl FnMut(&mut Self, &T)) {
        let rb = self.p.rbox(0, b);
        let len = elts.len();
        for (i, elt) in elts.iter().enumerate() {
            op(self, elt);
            if i + 1 < len {
                self.p.word(",");
                self.p.space_if_not_bol();
            }
        }
        self.p.end(rb);
    }

    fn commasep_exprs(&mut self, b: Breaks, exprs: &[Expr]) {
        self.commasep_cmnt(b, exprs, |s, e| s.print_expr(e, Fixup::default()))
    }

    fn bopen(&mut self, ib: BoxMarker) {
        self.p.word("{");
        self.p.end(ib);
    }

    fn bclose_maybe_open(&mut self, no_space: bool, cb: Option<BoxMarker>) {
        if !no_space {
            self.p.break_offset_if_not_bol(1, -INDENT_UNIT);
        }
        self.p.word("}");
        if let Some(cb) = cb {
            self.p.end(cb);
        }
    }

    fn bclose(&mut self, no_space: bool, cb: BoxMarker) {
        self.bclose_maybe_open(no_space, Some(cb))
    }

    fn print_mutability(&mut self, mutbl: bool, print_const: bool) {
        if mutbl {
            self.p.word_nbsp("mut");
        } else if print_const {
            self.p.word_nbsp("const");
        }
    }

    fn print_lifetime(&mut self, lifetime: &str) {
        self.p.word(lifetime.to_owned());
    }

    fn print_opt_lifetime(&mut self, lifetime: &Option<String>) {
        if let Some(lt) = lifetime {
            self.print_lifetime(lt);
            self.p.nbsp();
        }
    }

    // ----- paths ----------------------------------------------------------

    fn print_path(&mut self, path: &Path, colons_before_params: bool, depth: usize) {
        let end = path.segments.len().saturating_sub(depth);
        for (i, segment) in path.segments[..end].iter().enumerate() {
            if i > 0 {
                self.p.word("::")
            }
            self.print_path_segment(segment, colons_before_params);
        }
    }

    fn print_path_segment(&mut self, segment: &PathSegment, colons_before_params: bool) {
        if !segment.ident.is_empty() {
            self.p.word(segment.ident.clone());
            if let Some(args) = &segment.args {
                self.print_generic_args(args, colons_before_params);
            }
        }
    }

    fn print_qpath(&mut self, path: &Path, qself: &QSelf, colons_before_params: bool) {
        self.p.word("<");
        self.print_type(&qself.ty);
        if qself.position > 0 {
            self.p.space();
            self.p.word_space("as");
            let depth = path.segments.len() - qself.position;
            self.print_path(path, false, depth);
        }
        self.p.word(">");
        for item_segment in &path.segments[qself.position..] {
            self.p.word("::");
            self.p.word(item_segment.ident.clone());
            if let Some(args) = &item_segment.args {
                self.print_generic_args(args, colons_before_params)
            }
        }
    }

    fn print_maybe_qpath(&mut self, qself: &Option<QSelf>, path: &Path, colons: bool) {
        match qself {
            Some(qself) => self.print_qpath(path, qself, colons),
            None => self.print_path(path, colons, 0),
        }
    }

    fn print_generic_args(&mut self, args: &GenericArgs, colons_before_params: bool) {
        if colons_before_params {
            self.p.word("::")
        }
        match args {
            GenericArgs::Angle(args) => {
                self.p.word("<");
                self.commasep(Breaks::Inconsistent, args, |s, arg| match arg {
                    AngleArg::Lifetime(lt) => s.print_lifetime(lt),
                    AngleArg::Type(ty) => s.print_type(ty),
                    AngleArg::Const(expr) => s.print_expr(expr, Fixup::default()),
                    AngleArg::Constraint { ident, args, kind } => {
                        s.p.word(ident.clone());
                        if let Some(args) = args {
                            s.print_generic_args(args, false)
                        }
                        s.p.space();
                        match kind {
                            ConstraintKind::EqualityTy(ty) => {
                                s.p.word_space("=");
                                s.print_type(ty);
                            }
                            ConstraintKind::EqualityConst(expr) => {
                                s.p.word_space("=");
                                s.print_expr_anon_const(expr);
                            }
                            ConstraintKind::Bound(bounds) => {
                                if !bounds.is_empty() {
                                    s.p.word_nbsp(":");
                                    s.print_type_bounds(bounds);
                                }
                            }
                        }
                    }
                });
                self.p.word(">")
            }
            GenericArgs::Paren { inputs, output } => {
                self.p.word("(");
                self.commasep(Breaks::Inconsistent, inputs, |s, ty| {
                    let ib = s.p.ibox(INDENT_UNIT);
                    s.print_type(ty);
                    s.p.end(ib);
                });
                self.p.word(")");
                if let Some(output) = output {
                    self.print_fn_ret_ty(output);
                }
            }
            GenericArgs::ParenElided => {
                self.p.word("(");
                self.p.word("..");
                self.p.word(")");
            }
        }
    }

    // ----- types and bounds -------------------------------------------

    fn print_type(&mut self, ty: &Ty) {
        let ib = self.p.ibox(0);
        match ty {
            Ty::Slice(ty) => {
                self.p.word("[");
                self.print_type(ty);
                self.p.word("]");
            }
            Ty::Ptr(mutbl, ty) => {
                self.p.word("*");
                self.print_mutability(*mutbl, true);
                self.print_type(ty);
            }
            Ty::Ref(lifetime, mutbl, ty) => {
                self.p.word("&");
                self.print_opt_lifetime(lifetime);
                self.print_mutability(*mutbl, false);
                self.print_type(ty);
            }
            Ty::Never => self.p.word("!"),
            Ty::Tup(elts) => {
                self.p.popen();
                self.commasep(Breaks::Inconsistent, elts, |s, ty| s.print_type(ty));
                if elts.len() == 1 {
                    self.p.word(",");
                }
                self.p.pclose();
            }
            Ty::Paren(ty) => {
                self.p.popen();
                self.print_type(ty);
                self.p.pclose();
            }
            Ty::FnPtr {
                generic_params,
                unsafety,
                abi,
                params,
                output,
            } => {
                let ib = self.p.ibox(INDENT_UNIT);
                if !generic_params.is_empty() {
                    self.p.word("for");
                    self.print_generic_params(generic_params);
                    self.p.nbsp();
                }
                if *unsafety {
                    self.p.word_nbsp("unsafe");
                }
                if let Some(abi) = abi {
                    self.p.word_nbsp("extern");
                    if let Some(abi) = abi {
                        self.p.word(abi.clone());
                        self.p.nbsp();
                    }
                }
                self.p.word("fn");
                self.print_fn_params_and_ret(params, output.as_deref(), false);
                self.p.end(ib);
            }
            Ty::Path(qself, path) => self.print_maybe_qpath(qself, path, false),
            Ty::TraitObject(dyn_, bounds) => {
                if *dyn_ {
                    self.p.word_nbsp("dyn");
                }
                self.print_type_bounds(bounds);
            }
            Ty::ImplTrait(bounds) => {
                self.p.word_nbsp("impl");
                self.print_type_bounds(bounds);
            }
            Ty::Infer => self.p.word("_"),
            Ty::MacCall(m) => self.print_mac(m),
            Ty::Array(ty, length) => {
                self.p.word("[");
                self.print_type(ty);
                self.p.word("; ");
                self.print_expr(length, Fixup::default());
                self.p.word("]");
            }
        }
        self.p.end(ib);
    }

    fn print_type_bounds(&mut self, bounds: &[Bound]) {
        let mut first = true;
        for bound in bounds {
            if first {
                first = false;
            } else {
                self.p.nbsp();
                self.p.word_space("+");
            }
            match bound {
                Bound::Trait {
                    parens,
                    for_params,
                    constness,
                    asyncness,
                    polarity,
                    path,
                } => {
                    if *parens {
                        self.p.popen();
                    }
                    if !for_params.is_empty() {
                        self.p.word("for");
                        self.print_generic_params(for_params);
                        self.p.nbsp();
                    }
                    if let Some(constness) = constness {
                        self.p.word_space(*constness);
                    }
                    if *asyncness {
                        self.p.word_space("async");
                    }
                    if let Some(polarity) = polarity {
                        self.p.word(*polarity);
                    }
                    self.print_path(path, false, 0);
                    if *parens {
                        self.p.pclose();
                    }
                }
                Bound::Outlives(lt) => self.print_lifetime(lt),
                Bound::Use(args) => {
                    self.p.word("use");
                    self.p.word("<");
                    self.commasep(Breaks::Inconsistent, args, |s, arg| match arg {
                        UseArg::Path(p) => s.print_path(p, false, 0),
                        UseArg::Lifetime(lt) => s.print_lifetime(lt),
                    });
                    self.p.word(">")
                }
            }
        }
    }

    fn print_generic_params(&mut self, params: &[GenericParam]) {
        if params.is_empty() {
            return;
        }
        self.p.word("<");
        self.commasep(Breaks::Inconsistent, params, |s, param| match param {
            GenericParam::Lifetime { name, bounds } => {
                s.print_lifetime(name);
                if !bounds.is_empty() {
                    s.p.word_nbsp(":");
                    for (i, bound) in bounds.iter().enumerate() {
                        if i != 0 {
                            s.p.word(" + ");
                        }
                        s.print_lifetime(bound);
                    }
                }
            }
            GenericParam::Type {
                ident,
                bounds,
                default,
            } => {
                s.p.word(ident.clone());
                if !bounds.is_empty() {
                    s.p.word_nbsp(":");
                    s.print_type_bounds(bounds);
                }
                if let Some(default) = default {
                    s.p.space();
                    s.p.word_space("=");
                    s.print_type(default)
                }
            }
            GenericParam::Const { ident, ty, default } => {
                s.p.word_space("const");
                s.p.word(ident.clone());
                s.p.space();
                s.p.word_space(":");
                s.print_type(ty);
                if let Some(default) = default {
                    s.p.space();
                    s.p.word_space("=");
                    s.print_expr(default, Fixup::default());
                }
            }
        });
        self.p.word(">");
    }

    fn print_param(&mut self, input: &Param, is_closure: bool) {
        let ib = self.p.ibox(INDENT_UNIT);
        match (&input.pat, &input.ty) {
            (Some(pat), None) if is_closure => self.print_pat(pat),
            (Some(pat), Some(Ty::Infer)) if is_closure => self.print_pat(pat),
            (pat, Some(ty)) => {
                if let Some(pat) = pat {
                    self.print_pat(pat);
                    self.p.word(":");
                    self.p.space();
                }
                self.print_type(ty);
            }
            (Some(pat), None) => self.print_pat(pat),
            (None, None) => {}
        }
        self.p.end(ib);
    }

    fn print_fn_ret_ty(&mut self, ty: &Ty) {
        self.p.space_if_not_bol();
        let ib = self.p.ibox(INDENT_UNIT);
        self.p.word_space("->");
        self.print_type(ty);
        self.p.end(ib);
    }

    fn print_fn_params_and_ret(&mut self, params: &[Param], output: Option<&Ty>, closure: bool) {
        let (open, close) = if closure { ("|", "|") } else { ("(", ")") };
        self.p.word(open);
        self.commasep(Breaks::Inconsistent, params, |s, param| {
            s.print_param(param, closure)
        });
        self.p.word(close);
        if let Some(output) = output {
            self.print_fn_ret_ty(output);
        }
    }

    // ----- macros -------------------------------------------------------

    fn print_mac(&mut self, m: &MacCall) {
        let path = m.path.clone();
        tts::print_mac_common(&mut self.p, MacHeader::Bang, m.delim, None, &m.tts, |p| {
            let mut s = State {
                p: std::mem::replace(p, Printer::new()),
            };
            s.print_path(&path, false, 0);
            s.p.word("!");
            *p = s.p;
        });
    }

    // ----- patterns -----------------------------------------------------

    fn print_pat_paren_if_or(&mut self, pat: &Pat) {
        let needs_paren = matches!(pat, Pat::Or(..));
        if needs_paren {
            self.p.popen();
        }
        self.print_pat(pat);
        if needs_paren {
            self.p.pclose();
        }
    }

    fn print_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Wild => self.p.word("_"),
            Pat::Ident {
                by_ref,
                mutbl,
                ident,
                sub,
            } => {
                if *mutbl && !*by_ref {
                    self.p.word_nbsp("mut");
                }
                if *by_ref {
                    self.p.word_nbsp("ref");
                    if *mutbl {
                        self.p.word_nbsp("mut");
                    }
                }
                self.p.word(ident.clone());
                if let Some(p) = sub {
                    self.p.space();
                    self.p.word_space("@");
                    self.print_pat_paren_if_or(p);
                }
            }
            Pat::TupleStruct(qself, path, elts) => {
                self.print_maybe_qpath(qself, path, true);
                self.p.popen();
                self.commasep(Breaks::Inconsistent, elts, |s, p| s.print_pat(p));
                self.p.pclose();
            }
            Pat::Or(pats) => {
                self.strsep("|", true, Breaks::Inconsistent, pats, |s, p| s.print_pat(p));
            }
            Pat::Path(None, path) => self.print_path(path, true, 0),
            Pat::Path(Some(qself), path) => self.print_qpath(path, qself, false),
            Pat::Struct(qself, path, fields, rest) => {
                self.print_maybe_qpath(qself, path, true);
                self.p.nbsp();
                self.p.word("{");
                let empty = fields.is_empty() && !rest;
                if !empty {
                    self.p.space();
                }
                self.commasep_cmnt(Breaks::Consistent, fields, |s, f| {
                    let cb = s.p.cbox(INDENT_UNIT);
                    if let Some(ident) = &f.ident {
                        s.p.word(ident.clone());
                        s.p.word_nbsp(":");
                    }
                    s.print_pat(&f.pat);
                    s.p.end(cb);
                });
                if *rest {
                    if !fields.is_empty() {
                        self.p.word_space(",");
                    }
                    self.p.word("..");
                }
                if !empty {
                    self.p.space();
                }
                self.p.word("}");
            }
            Pat::Tuple(elts) => {
                self.p.popen();
                self.commasep(Breaks::Inconsistent, elts, |s, p| s.print_pat(p));
                if elts.len() == 1 {
                    self.p.word(",");
                }
                self.p.pclose();
            }
            Pat::Deref(inner) => {
                self.p.word("deref!");
                self.p.popen();
                self.print_pat(inner);
                self.p.pclose();
            }
            Pat::Ref(inner, mutbl) => {
                self.p.word("&");
                if *mutbl {
                    self.p.word("mut ");
                }
                if matches!(
                    **inner,
                    Pat::Ident {
                        by_ref: false,
                        mutbl: true,
                        ..
                    }
                ) {
                    self.p.popen();
                    self.print_pat(inner);
                    self.p.pclose();
                } else {
                    self.print_pat_paren_if_or(inner);
                }
            }
            Pat::Expr(e) => self.print_expr(e, Fixup::default()),
            Pat::Range(begin, end, op) => {
                if let Some(e) = begin {
                    self.print_expr(e, Fixup::default());
                }
                self.p.word(*op);
                if let Some(e) = end {
                    self.print_expr(e, Fixup::default());
                }
            }
            Pat::Slice(elts) => {
                self.p.word("[");
                self.commasep(Breaks::Inconsistent, elts, |s, p| s.print_pat(p));
                self.p.word("]");
            }
            Pat::Rest => self.p.word(".."),
            Pat::Paren(inner) => {
                self.p.popen();
                self.print_pat(inner);
                self.p.pclose();
            }
            Pat::MacCall(m) => self.print_mac(m),
        }
    }

    // ----- blocks and statements --------------------------------------

    fn print_block(&mut self, blk: &Block, cb: BoxMarker, ib: BoxMarker) {
        self.print_block_maybe_unclosed(blk, Some(cb), ib)
    }

    fn print_block_unclosed_indent(&mut self, blk: &Block, ib: BoxMarker) {
        self.print_block_maybe_unclosed(blk, None, ib)
    }

    fn print_block_maybe_unclosed(&mut self, blk: &Block, cb: Option<BoxMarker>, ib: BoxMarker) {
        if blk.unsafe_ {
            self.p.word_space("unsafe");
        }
        self.bopen(ib);
        let len = blk.stmts.len();
        for (i, st) in blk.stmts.iter().enumerate() {
            match st {
                Stmt::Expr(expr) if i + 1 == len => {
                    self.p.space_if_not_bol();
                    self.print_expr_outer_attr_style(expr, false, Fixup::new_stmt());
                }
                _ => self.print_stmt(st),
            }
        }
        let no_space = blk.stmts.is_empty();
        self.bclose_maybe_open(no_space, cb);
    }

    fn print_stmt(&mut self, st: &Stmt) {
        match st {
            Stmt::Let {
                super_,
                pat,
                ty,
                init,
                els,
            } => {
                self.p.space_if_not_bol();
                let ib1 = self.p.ibox(INDENT_UNIT);
                if *super_ {
                    self.p.word_nbsp("super");
                }
                self.p.word_nbsp("let");
                let ib2 = self.p.ibox(INDENT_UNIT);
                self.print_pat(pat);
                if let Some(ty) = ty {
                    self.p.word_space(":");
                    self.print_type(ty);
                }
                self.p.end(ib2);
                if let Some(init) = init {
                    self.p.nbsp();
                    self.p.word_space("=");
                    self.print_expr_cond_paren(
                        init,
                        els.is_some() && expr_has_trailing_brace(init),
                        Fixup::default(),
                    );
                    if let Some(els) = els {
                        let cb = self.p.cbox(INDENT_UNIT);
                        let ib = self.p.ibox(INDENT_UNIT);
                        self.p.word(" else ");
                        self.print_block(els, cb, ib);
                    }
                }
                self.p.word(";");
                self.p.end(ib1);
            }
            Stmt::Expr(expr) => {
                self.p.space_if_not_bol();
                self.print_expr_outer_attr_style(expr, false, Fixup::new_stmt());
                if expr_requires_semi_to_be_stmt(expr) {
                    self.p.word(";");
                }
            }
            Stmt::Semi(expr) => {
                self.p.space_if_not_bol();
                self.print_expr_outer_attr_style(expr, false, Fixup::new_stmt());
                self.p.word(";");
            }
            Stmt::Item(item) => self.print_item(item),
            Stmt::Empty => {
                self.p.space_if_not_bol();
                self.p.word(";");
            }
            Stmt::MacCall(mac, semicolon) => {
                self.p.space_if_not_bol();
                self.print_mac(mac);
                if *semicolon {
                    self.p.word(";");
                }
            }
        }
    }

    // Printing-only nominal declarations, following rustc's item printer.
    fn hardbreak_if_not_bol(&mut self) {
        if !self.p.is_beginning_of_line() {
            self.p.hardbreak();
        }
    }
    fn item_head(&mut self, keyword: &str) -> (BoxMarker, BoxMarker) {
        let cb = self.p.cbox(INDENT_UNIT);
        let ib = self.p.ibox(0);
        if !keyword.is_empty() {
            self.p.word_nbsp(keyword.to_owned());
        }
        (cb, ib)
    }
    fn print_visibility(&mut self, visibility: &Visibility) {
        match visibility {
            Visibility::Inherited => {}
            Visibility::Public => self.p.word_nbsp("pub"),
            Visibility::Restricted { path, shorthand } => {
                self.p.word("pub(");
                if !shorthand {
                    self.p.word_nbsp("in");
                }
                self.print_path(path, false, 0);
                self.p.word_nbsp(")");
            }
        }
    }
    fn print_where_predicates(&mut self, predicates: &[WherePredicate]) {
        if predicates.is_empty() {
            return;
        }
        self.p.space();
        self.p.word_space("where");
        for (ordinal, predicate) in predicates.iter().enumerate() {
            if ordinal != 0 {
                self.p.word_space(",");
            }
            if !predicate.params.is_empty() {
                self.p.word("for");
                self.print_generic_params(&predicate.params);
                self.p.nbsp();
            }
            if let Some(ty) = &predicate.ty {
                self.print_type(ty);
            }
            if let Some(lifetime) = &predicate.lifetime {
                self.print_lifetime(lifetime);
            }
            self.p.word(":");
            if !predicate.bounds.is_empty() {
                self.p.nbsp();
                self.print_type_bounds(&predicate.bounds);
            }
        }
    }
    fn print_fields(
        &mut self,
        fields: &Fields,
        predicates: &[WherePredicate],
        finalizer: bool,
        cb: BoxMarker,
        ib: BoxMarker,
    ) {
        match fields {
            Fields::Unit | Fields::Tuple(_) => {
                if let Fields::Tuple(fields) = fields {
                    self.p.popen();
                    self.commasep(Breaks::Inconsistent, fields, |s, field| {
                        s.print_visibility(&field.visibility);
                        s.print_type(&field.ty);
                    });
                    self.p.pclose();
                }
                self.print_where_predicates(predicates);
                if finalizer {
                    self.p.word(";");
                }
                self.p.end(ib);
                self.p.end(cb);
            }
            Fields::Record(fields) => {
                self.print_where_predicates(predicates);
                self.p.nbsp();
                self.bopen(ib);
                for field in fields {
                    self.hardbreak_if_not_bol();
                    self.print_visibility(&field.visibility);
                    self.p.word(field.ident.clone().expect("record field name"));
                    self.p.word_nbsp(":");
                    self.print_type(&field.ty);
                    self.p.word(",");
                }
                self.bclose(fields.is_empty(), cb);
            }
        }
    }
    fn print_item(&mut self, item: &Item) {
        self.hardbreak_if_not_bol();
        let (cb, ib) = self.item_head("");
        self.print_visibility(&item.visibility);
        self.p.word_nbsp(match &item.kind {
            ItemKind::Enum(_) => "enum",
            ItemKind::Struct(_) => "struct",
            ItemKind::Union(_) => "union",
        });
        self.p.word(item.ident.clone());
        self.print_generic_params(&item.generics);
        match &item.kind {
            ItemKind::Enum(variants) => {
                self.print_where_predicates(&item.predicates);
                self.p.space();
                self.bopen(ib);
                for variant in variants {
                    self.p.space_if_not_bol();
                    let variant_ib = self.p.ibox(0);
                    let (vcb, vib) = self.item_head("");
                    self.print_visibility(&variant.visibility);
                    self.p.word(variant.ident.clone());
                    self.print_fields(&variant.fields, &[], false, vcb, vib);
                    if let Some(expr) = &variant.discriminant {
                        self.p.space();
                        self.p.word_space("=");
                        self.print_expr(expr, Fixup::default());
                    }
                    self.p.word(",");
                    self.p.end(variant_ib);
                }
                self.bclose(variants.is_empty(), cb);
            }
            ItemKind::Struct(fields) | ItemKind::Union(fields) => {
                self.print_fields(fields, &item.predicates, true, cb, ib)
            }
        }
    }

    // ----- expressions --------------------------------------------------

    fn print_else(&mut self, els: Option<&Expr>) {
        if let Some(els) = els {
            match els {
                Expr::If(i, then, e) => {
                    let cb = self.p.cbox(0);
                    let ib = self.p.ibox(0);
                    self.p.word(" else if ");
                    self.print_expr_as_cond(i);
                    self.p.space();
                    self.print_block(then, cb, ib);
                    self.print_else(e.as_deref())
                }
                Expr::Block(b, None) => {
                    let cb = self.p.cbox(0);
                    let ib = self.p.ibox(0);
                    self.p.word(" else ");
                    self.print_block(b, cb, ib)
                }
                _ => {}
            }
        }
    }

    fn print_if(&mut self, test: &Expr, blk: &Block, elseopt: Option<&Expr>) {
        let cb = self.p.cbox(0);
        let ib = self.p.ibox(0);
        self.p.word_nbsp("if");
        self.print_expr_as_cond(test);
        self.p.space();
        self.print_block(blk, cb, ib);
        self.print_else(elseopt)
    }

    fn print_call_post(&mut self, args: &[Expr]) {
        self.p.popen();
        self.commasep_exprs(Breaks::Inconsistent, args);
        self.p.pclose()
    }

    fn print_expr_as_cond(&mut self, expr: &Expr) {
        self.print_expr_cond_paren(expr, Self::cond_needs_par(expr), Fixup::new_cond())
    }

    fn cond_needs_par(expr: &Expr) -> bool {
        match expr {
            Expr::Break(..) | Expr::Closure(..) | Expr::Ret(..) | Expr::Yeet(..) => true,
            _ => contains_exterior_struct_lit(expr),
        }
    }

    fn print_expr_cond_paren(&mut self, expr: &Expr, needs_par: bool, mut fixup: Fixup) {
        if needs_par {
            self.p.popen();
            fixup = Fixup::default();
        }
        self.print_expr(expr, fixup);
        if needs_par {
            self.p.pclose();
        }
    }

    fn print_expr_vec(&mut self, exprs: &[Expr]) {
        let ib = self.p.ibox(INDENT_UNIT);
        self.p.word("[");
        self.commasep_exprs(Breaks::Inconsistent, exprs);
        self.p.word("]");
        self.p.end(ib);
    }

    fn print_expr_anon_const(&mut self, value: &Expr) {
        let ib = self.p.ibox(INDENT_UNIT);
        self.p.word("const");
        self.p.nbsp();
        if let Expr::Block(block, None) = value {
            let cb = self.p.cbox(0);
            let ib = self.p.ibox(0);
            self.print_block(block, cb, ib);
        } else {
            self.print_expr(value, Fixup::default());
        }
        self.p.end(ib);
    }

    fn print_const_block(&mut self, block: &Block) {
        let ib = self.p.ibox(INDENT_UNIT);
        self.p.word("const");
        self.p.nbsp();
        let cb = self.p.cbox(0);
        let ib2 = self.p.ibox(0);
        self.print_block(block, cb, ib2);
        self.p.end(ib);
    }

    fn print_expr_repeat(&mut self, element: &Expr, count: &Expr) {
        let ib = self.p.ibox(INDENT_UNIT);
        self.p.word("[");
        self.print_expr(element, Fixup::default());
        self.p.word_space(";");
        self.print_expr(count, Fixup::default());
        self.p.word("]");
        self.p.end(ib);
    }

    fn print_expr_struct(&mut self, se: &StructExpr) {
        self.print_maybe_qpath(&se.qself, &se.path, true);
        self.p.nbsp();
        self.p.word("{");
        let has_rest = !matches!(se.rest, StructRest::None);
        if se.fields.is_empty() && !has_rest {
            self.p.word("}");
            return;
        }
        let cb = self.p.cbox(0);
        let len = se.fields.len();
        for (idx, (ident, expr)) in se.fields.iter().enumerate() {
            let is_first = idx == 0;
            let is_last = idx + 1 == len;
            if is_first {
                self.p.space_if_not_bol();
            }
            if let Some(ident) = ident {
                self.p.word(ident.clone());
                self.p.word_nbsp(":");
            }
            self.print_expr(expr, Fixup::default());
            if !is_last || has_rest {
                self.p.word_space(",");
            } else {
                self.p.trailing_comma_or_space();
            }
        }
        if has_rest {
            if se.fields.is_empty() {
                self.p.space();
            }
            self.p.word("..");
            if let StructRest::Base(expr) = &se.rest {
                self.print_expr(expr, Fixup::default());
            }
            self.p.space();
        }
        self.p.offset(-INDENT_UNIT);
        self.p.end(cb);
        self.p.word("}");
    }

    fn print_expr_tup(&mut self, exprs: &[Expr]) {
        self.p.popen();
        self.commasep_exprs(Breaks::Inconsistent, exprs);
        if exprs.len() == 1 {
            self.p.word(",");
        }
        self.p.pclose()
    }

    fn print_expr_call(&mut self, func: &Expr, args: &[Expr], fixup: Fixup) {
        let func_fixup = fixup.leftmost_subexpression_with_operator(true);
        let needs_paren = match func {
            Expr::Field(_, name) => !name.bytes().all(|b| b.is_ascii_digit()),
            _ => func_fixup.precedence(func) < Prec::Unambiguous || expr_is_complete(func),
        };
        self.print_expr_cond_paren(func, needs_paren, func_fixup);
        self.print_call_post(args)
    }

    fn print_expr_method_call(
        &mut self,
        ident: &str,
        args: &Option<GenericArgs>,
        receiver: &Expr,
        base_args: &[Expr],
        fixup: Fixup,
    ) {
        let needs_paren = receiver.precedence() < Prec::Unambiguous;
        self.print_expr_cond_paren(
            receiver,
            needs_paren,
            fixup.leftmost_subexpression_with_dot(),
        );
        if !needs_paren && expr_ends_with_dot(receiver) {
            self.p.word(" ");
        }
        self.p.word(".");
        self.p.word(ident.to_owned());
        if let Some(args) = args {
            self.print_generic_args(args, true);
        }
        self.print_call_post(base_args)
    }

    fn print_expr_binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, fixup: Fixup) {
        let operator_can_begin_expr = matches!(
            op,
            BinOp::Sub
                | BinOp::Mul
                | BinOp::And
                | BinOp::Or
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::Shl
                | BinOp::Lt
        );
        let left_fixup = fixup.leftmost_subexpression_with_operator(operator_can_begin_expr);
        let binop_prec = op.precedence();
        let left_prec = left_fixup.precedence(lhs);
        let right_prec = fixup.precedence(rhs);
        let (mut left_needs_paren, right_needs_paren) = if op.is_comparison() {
            (left_prec <= binop_prec, right_prec <= binop_prec)
        } else {
            (left_prec < binop_prec, right_prec <= binop_prec)
        };
        match (lhs, op) {
            (Expr::Cast(..), BinOp::Lt | BinOp::Shl) => left_needs_paren = true,
            (Expr::Let(..), _) if !needs_par_as_let_scrutinee(binop_prec) => {
                left_needs_paren = true
            }
            _ => {}
        }
        self.print_expr_cond_paren(lhs, left_needs_paren, left_fixup);
        self.p.space();
        self.p.word_space(op.as_str());
        self.print_expr_cond_paren(rhs, right_needs_paren, fixup.rightmost_subexpression());
    }

    fn print_expr_unary(&mut self, op: &'static str, expr: &Expr, fixup: Fixup) {
        self.p.word(op);
        self.print_expr_cond_paren(
            expr,
            fixup.precedence(expr) < Prec::Prefix,
            fixup.rightmost_subexpression(),
        );
    }

    fn print_expr_addr_of(&mut self, raw: bool, mutbl: bool, expr: &Expr, fixup: Fixup) {
        self.p.word("&");
        if raw {
            self.p.word_nbsp("raw");
            self.print_mutability(mutbl, true);
        } else {
            self.print_mutability(mutbl, false);
        }
        self.print_expr_cond_paren(
            expr,
            fixup.precedence(expr) < Prec::Prefix,
            fixup.rightmost_subexpression(),
        );
    }

    fn print_let(&mut self, pat: &Pat, expr: &Expr, fixup: Fixup) {
        self.p.word("let ");
        self.print_pat(pat);
        self.p.space();
        self.p.word_space("=");
        self.print_expr_cond_paren(
            expr,
            fixup.needs_par_as_let_scrutinee(expr),
            Fixup::default(),
        );
    }

    fn print_arm(&mut self, arm: &Arm) {
        self.p.space();
        let cb = self.p.cbox(INDENT_UNIT);
        let ib = self.p.ibox(0);
        self.print_pat(&arm.pat);
        self.p.space();
        if let Some(guard) = &arm.guard {
            self.p.word_space("if");
            self.print_expr(guard, Fixup::default());
            self.p.space();
        }
        self.p.word_space("=>");
        match &arm.body {
            Expr::Block(blk, label) => {
                if let Some(label) = label {
                    self.print_lifetime(label);
                    self.p.word_space(":");
                }
                self.print_block_unclosed_indent(blk, ib);
                if blk.unsafe_ {
                    self.p.word(",");
                }
            }
            body => {
                self.p.end(ib);
                self.print_expr(body, Fixup::new_match_arm());
                self.p.word(",");
            }
        }
        self.p.end(cb);
    }

    fn print_expr(&mut self, expr: &Expr, fixup: Fixup) {
        self.print_expr_outer_attr_style(expr, true, fixup)
    }

    fn print_expr_outer_attr_style(&mut self, expr: &Expr, _is_inline: bool, mut fixup: Fixup) {
        let ib = self.p.ibox(INDENT_UNIT);
        let needs_par = fixup.would_cause_statement_boundary(expr);
        if needs_par {
            self.p.popen();
            fixup = Fixup::default();
        }
        match expr {
            Expr::Array(exprs) => self.print_expr_vec(exprs),
            Expr::ConstBlock(block) => self.print_const_block(block),
            Expr::Repeat(element, count) => self.print_expr_repeat(element, count),
            Expr::Struct(se) => self.print_expr_struct(se),
            Expr::Tup(exprs) => self.print_expr_tup(exprs),
            Expr::Call(func, args) => self.print_expr_call(func, args, fixup),
            Expr::MethodCall {
                ident,
                args,
                receiver,
                call_args,
            } => self.print_expr_method_call(ident, args, receiver, call_args, fixup),
            Expr::Binary(op, lhs, rhs) => self.print_expr_binary(*op, lhs, rhs, fixup),
            Expr::Unary(op, expr) => self.print_expr_unary(op, expr, fixup),
            Expr::AddrOf { raw, mutbl, expr } => self.print_expr_addr_of(*raw, *mutbl, expr, fixup),
            Expr::Lit(text) => self.p.word(text.clone()),
            Expr::Cast(expr, ty) => {
                self.print_expr_cond_paren(
                    expr,
                    expr.precedence() < Prec::Cast,
                    fixup.leftmost_subexpression(),
                );
                self.p.space();
                self.p.word_space("as");
                self.print_type(ty);
            }
            Expr::Let(pat, scrutinee) => self.print_let(pat, scrutinee, fixup),
            Expr::If(test, blk, elseopt) => self.print_if(test, blk, elseopt.as_deref()),
            Expr::While(test, blk, label) => {
                if let Some(label) = label {
                    self.print_lifetime(label);
                    self.p.word_space(":");
                }
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.p.word_nbsp("while");
                self.print_expr_as_cond(test);
                self.p.space();
                self.print_block(blk, cb, ib);
            }
            Expr::ForLoop {
                pat,
                iter,
                body,
                label,
            } => {
                if let Some(label) = label {
                    self.print_lifetime(label);
                    self.p.word_space(":");
                }
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.p.word_nbsp("for");
                self.print_pat(pat);
                self.p.space();
                self.p.word_space("in");
                self.print_expr_as_cond(iter);
                self.p.space();
                self.print_block(body, cb, ib);
            }
            Expr::Loop(blk, label) => {
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                if let Some(label) = label {
                    self.print_lifetime(label);
                    self.p.word_space(":");
                }
                self.p.word_nbsp("loop");
                self.print_block(blk, cb, ib);
            }
            Expr::Match(scrutinee, arms) => {
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.p.word_nbsp("match");
                self.print_expr_as_cond(scrutinee);
                self.p.space();
                self.bopen(ib);
                for arm in arms {
                    self.print_arm(arm);
                }
                self.bclose(arms.is_empty(), cb);
            }
            Expr::Closure(c) => {
                if !c.for_params.is_empty() {
                    self.p.word("for");
                    self.print_generic_params(&c.for_params);
                    self.p.nbsp();
                }
                if c.constness {
                    self.p.word_nbsp("const");
                }
                if c.static_ {
                    self.p.word_space("static");
                }
                if let Some(kind) = c.coroutine {
                    for word in kind.split(' ') {
                        self.p.word_nbsp(word);
                    }
                }
                if c.move_ {
                    self.p.word_space("move");
                }
                self.print_fn_params_and_ret(&c.params, c.output.as_ref(), true);
                self.p.space();
                self.print_expr(&c.body, Fixup::default());
            }
            Expr::Block(blk, label) => {
                if let Some(label) = label {
                    self.print_lifetime(label);
                    self.p.word_space(":");
                }
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.print_block(blk, cb, ib);
            }
            Expr::Gen(kind, move_, blk) => {
                self.p.word_nbsp(*kind);
                if *move_ {
                    self.p.word_space("move");
                }
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.print_block(blk, cb, ib);
            }
            Expr::Await(expr) => {
                self.print_expr_cond_paren(
                    expr,
                    expr.precedence() < Prec::Unambiguous,
                    fixup.leftmost_subexpression_with_dot(),
                );
                self.p.word(".await");
            }
            Expr::Assign(lhs, rhs) => {
                self.print_expr_cond_paren(
                    lhs,
                    lhs.precedence() <= Prec::Range,
                    fixup.leftmost_subexpression(),
                );
                self.p.space();
                self.p.word_space("=");
                self.print_expr_cond_paren(
                    rhs,
                    fixup.precedence(rhs) < Prec::Assign,
                    fixup.rightmost_subexpression(),
                );
            }
            Expr::AssignOp(op, lhs, rhs) => {
                self.print_expr_cond_paren(
                    lhs,
                    lhs.precedence() <= Prec::Range,
                    fixup.leftmost_subexpression(),
                );
                self.p.space();
                self.p.word_space(format!("{}=", op.as_str()));
                self.print_expr_cond_paren(
                    rhs,
                    fixup.precedence(rhs) < Prec::Assign,
                    fixup.rightmost_subexpression(),
                );
            }
            Expr::Field(expr, ident) => {
                let needs_paren = expr.precedence() < Prec::Unambiguous;
                self.print_expr_cond_paren(
                    expr,
                    needs_paren,
                    fixup.leftmost_subexpression_with_dot(),
                );
                if !needs_paren && expr_ends_with_dot(expr) {
                    self.p.word(" ");
                }
                self.p.word(".");
                self.p.word(ident.clone());
            }
            Expr::Index(expr, index) => {
                let expr_fixup = fixup.leftmost_subexpression_with_operator(true);
                self.print_expr_cond_paren(
                    expr,
                    expr_fixup.precedence(expr) < Prec::Unambiguous || expr_is_complete(expr),
                    expr_fixup,
                );
                self.p.word("[");
                self.print_expr(index, Fixup::default());
                self.p.word("]");
            }
            Expr::Range(start, end, closed) => {
                let fake_prec = Prec::LOr;
                if let Some(e) = start {
                    let start_fixup = fixup.leftmost_subexpression_with_operator(true);
                    let needs_paren = start_fixup.precedence(e) < fake_prec;
                    self.print_expr_cond_paren(e, needs_paren, start_fixup);
                    if !needs_paren && expr_ends_with_dot(e) {
                        self.p.word(" ");
                    }
                }
                self.p.word(if *closed { "..=" } else { ".." });
                if let Some(e) = end {
                    self.print_expr_cond_paren(
                        e,
                        fixup.precedence(e) < fake_prec,
                        fixup.rightmost_subexpression(),
                    );
                }
            }
            Expr::Underscore => self.p.word("_"),
            Expr::Path(qself, path) => self.print_maybe_qpath(qself, path, true),
            Expr::Break(label, value) => {
                self.p.word("break");
                if let Some(label) = label {
                    self.p.space();
                    self.print_lifetime(label);
                }
                if let Some(expr) = value {
                    self.p.space();
                    self.print_expr_cond_paren(
                        expr,
                        label.is_none() && leading_labeled_expr(expr),
                        fixup.rightmost_subexpression(),
                    );
                }
            }
            Expr::Continue(label) => {
                self.p.word("continue");
                if let Some(label) = label {
                    self.p.space();
                    self.print_lifetime(label);
                }
            }
            Expr::Ret(result) => {
                self.p.word("return");
                if let Some(expr) = result {
                    self.p.word(" ");
                    self.print_expr(expr, fixup.rightmost_subexpression());
                }
            }
            Expr::Yeet(result) => {
                self.p.word("do");
                self.p.word(" ");
                self.p.word("yeet");
                if let Some(expr) = result {
                    self.p.word(" ");
                    self.print_expr(expr, fixup.rightmost_subexpression());
                }
            }
            Expr::Become(result) => {
                self.p.word("become");
                self.p.word(" ");
                self.print_expr(result, fixup.rightmost_subexpression());
            }
            Expr::MacCall(m) => self.print_mac(m),
            Expr::Paren(e) => {
                self.p.popen();
                self.print_expr(e, Fixup::default());
                self.p.pclose();
            }
            Expr::Yield(e) => {
                self.p.word("yield");
                if let Some(expr) = e {
                    self.p.space();
                    self.print_expr(expr, fixup.rightmost_subexpression());
                }
            }
            Expr::Try(e) => {
                self.print_expr_cond_paren(
                    e,
                    e.precedence() < Prec::Unambiguous,
                    fixup.leftmost_subexpression_with_dot(),
                );
                self.p.word("?")
            }
            Expr::TryBlock(blk) => {
                let cb = self.p.cbox(0);
                let ib = self.p.ibox(0);
                self.p.word_nbsp("try");
                self.print_block(blk, cb, ib)
            }
        }
        if needs_par {
            self.p.pclose();
        }
        self.p.end(ib);
    }
}
