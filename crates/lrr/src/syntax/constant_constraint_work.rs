//! Charge the actual source surfaces used by subordinate namespace/header probes.
//! Conservative charges cover structural scans required by length admission.
//! Cold bodies are never typechecked or executed by these probes.
use super::*;

fn source_fact<T>(result: Result<T, Diagnostic>) -> Result<Option<T>, Diagnostic> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if matches!(
                error.code,
                "LRR-E-CONST-LIMIT" | "LRR-E-METADATA-LIMIT" | "LRR-E-SOURCE-LIMIT"
            ) =>
        {
            Err(error)
        }
        // Missing source semantics make a connected component opaque. They
        // cannot supply a primitive context or certify constant execution.
        Err(_) => Ok(None),
    }
}
impl Graph<'_> {
    pub(super) fn checked_target_source(
        &mut self,
        target: &StatementRef,
    ) -> Result<(), Diagnostic> {
        let mut root = None;
        for (depth, ancestor) in target.0.syntax().ancestors().enumerate() {
            if depth >= 128 {
                return Err(limit());
            }
            self.source_units(1)?;
            root = Some(ancestor);
        }
        let root = root.ok_or_else(incomplete)?;
        // Parse::tree creates a fresh red root over the retained immutable green
        // tree. Red-node pointer equality would reject even our own selections.
        // This ephemeral identity check never becomes a query/cache encoding.
        let captured = self.source.0.parsed.parsed.tree();
        if !std::ptr::eq(root.green(), captured.syntax().green()) {
            return Err(fault(
                "constant local demand is not from this captured source syntax",
            ));
        }
        Ok(())
    }
    pub(super) fn checked_array_length(
        &mut self,
        expression: ExprRef,
    ) -> Result<ExprRef, Diagnostic> {
        let source = self.source;
        let root = source.0.parsed.parsed.tree();
        let width = usize::from(root.syntax().text_range().len());
        // array_length performs a structural source-index scan and owning
        // syntax checks. Charge those surfaces before calling it; do not
        // treat it as a free type/namespace probe or enter any cold body.
        self.source_units(width.checked_mul(4).ok_or_else(limit)?)?;
        for _ in root.syntax().descendants_with_tokens() {
            self.source_units(1)?;
        }
        self.charge(
            width
                .checked_mul(16)
                .and_then(|n| n.checked_add(std::mem::size_of::<ConstantPlan>() + 1024))
                .ok_or_else(limit)?,
        )?;
        self.charge(source.metadata_bytes()?)?;
        let plan = source.array_length(expression.range())?;
        let Entry::Length(actual) = plan.entry else {
            return Err(fault(
                "selected source context is not an actual array length",
            ));
        };
        if actual.range() != expression.range()
            || !std::ptr::eq(actual.0.syntax().green(), expression.0.syntax().green())
        {
            return Err(fault(
                "selected array length differs from the retained syntax",
            ));
        }
        Ok(actual)
    }
    fn source_units(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(count)
            .filter(|n| *n <= self.work_limit)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn source_text(&mut self, node: &ra_ap_syntax::SyntaxNode) -> Result<(), Diagnostic> {
        let width = usize::from(node.text_range().len());
        if width > 64 * 1024 {
            return Err(limit());
        }
        self.source_units(width.checked_add(1).ok_or_else(limit)?)?;
        // Charge bounded copied syntax conservatively as metadata as well.
        self.charge(width.checked_mul(4).ok_or_else(limit)?)
    }
    fn source_attributes(&mut self, node: &ra_ap_syntax::SyntaxNode) -> Result<(), Diagnostic> {
        for child in node.children() {
            self.source_units(1)?;
            if ast::Attr::can_cast(child.kind()) {
                self.source_text(&child)?;
            }
        }
        Ok(())
    }
    fn source_surface(
        &mut self,
        node: &ra_ap_syntax::SyntaxNode,
        type_namespace: bool,
    ) -> Result<(), Diagnostic> {
        for scope in node.ancestors() {
            self.source_units(1)?;
            if !type_namespace && !ast::StmtList::can_cast(scope.kind()) {
                continue;
            }
            for child in scope.children() {
                self.source_units(1)?;
                if ast::Item::can_cast(child.kind()) {
                    self.source_attributes(&child)?;
                    // Item names and generic parameter lists are header
                    // surfaces. Function/constant bodies remain untouched.
                    for part in child.children() {
                        self.source_units(1)?;
                        if ast::Name::can_cast(part.kind()) {
                            self.source_text(&part)?;
                        }
                    }
                } else if type_namespace && ast::GenericParamList::can_cast(child.kind()) {
                    self.source_text(&child)?;
                }
            }
        }
        Ok(())
    }
    fn source_type(&mut self, ty: &ast::Type) -> Result<(), Diagnostic> {
        self.source_text(ty.syntax())?;
        self.source_surface(ty.syntax(), true)
    }
    fn source_declaration(&mut self) -> Result<(), Diagnostic> {
        self.source_units(
            self.source
                .0
                .declarations
                .len()
                .checked_mul(2)
                .ok_or_else(limit)?,
        )
    }
    fn source_function(&mut self, function: &ast::Fn) -> Result<(), Diagnostic> {
        self.source_attributes(function.syntax())?;
        self.source_declaration()?;
        let params = required(function.param_list())?;
        for (index, param) in params.params().enumerate() {
            if index >= 256 {
                return Err(limit());
            }
            self.source_units(index.checked_add(1).ok_or_else(limit)?)?;
            self.source_text(required(param.pat())?.syntax())?;
            self.source_type(&required(param.ty())?)?;
        }
        if let Some(result) = function.ret_type() {
            self.source_type(&required(result.ty())?)?;
        }
        Ok(())
    }
    fn source_constant(&mut self, constant: &ast::Const) -> Result<(), Diagnostic> {
        self.source_attributes(constant.syntax())?;
        self.source_declaration()?;
        self.source_type(&required(constant.ty())?)
    }
    fn source_name(&mut self) -> Result<(), Diagnostic> {
        let source = self.source;
        for item in &source.0.module_items {
            self.source_units(1)?;
            self.source_attributes(item.syntax())?;
        }
        Ok(())
    }
    pub(super) fn checked_function_header(
        &mut self,
        function: &ast::Fn,
    ) -> Result<Function, Diagnostic> {
        self.source_function(function)?;
        self.source.function_header(function)
    }
    pub(super) fn checked_constant_header(
        &mut self,
        constant: &ast::Const,
    ) -> Result<Constant, Diagnostic> {
        self.source_constant(constant)?;
        self.source.constant_header(constant)
    }
    pub(super) fn checked_attributes(
        &mut self,
        node: &ra_ap_syntax::SyntaxNode,
    ) -> Result<bool, Diagnostic> {
        self.source_attributes(node)?;
        Ok(source_fact(self.source.attributes_clear(node))?.is_some())
    }
    pub(super) fn checked_primitive(
        &mut self,
        ty: ast::Type,
    ) -> Result<Option<PrimitiveType>, Diagnostic> {
        self.source_type(&ty)?;
        source_fact(self.source.primitive(ty))
    }
    pub(super) fn checked_path(&mut self, path: ast::Path) -> Result<Option<String>, Diagnostic> {
        self.source_text(path.syntax())?;
        source_fact(bare(path))
    }
    pub(super) fn checked_lexical_value(
        &mut self,
        node: &ra_ap_syntax::SyntaxNode,
        name: &str,
    ) -> Result<bool, Diagnostic> {
        self.source_surface(node, false)?;
        Ok(source_fact(self.source.lexical_value_clear(node, name))?.is_some())
    }
    pub(super) fn checked_named_constant(
        &mut self,
        name: &str,
    ) -> Result<Option<Constant>, Diagnostic> {
        self.source_name()?;
        let Some(ast::Item::Const(constant)) = source_fact(self.source.named(name))? else {
            return Ok(None);
        };
        self.source_constant(&constant)?;
        source_fact(self.source.constant_header(&constant))
    }
    pub(super) fn checked_named_function(
        &mut self,
        name: &str,
    ) -> Result<Option<Function>, Diagnostic> {
        self.source_name()?;
        let Some(ast::Item::Fn(function)) = source_fact(self.source.named(name))? else {
            return Ok(None);
        };
        self.source_function(&function)?;
        source_fact(self.source.function_header(&function))
    }
    pub(super) fn checked_literal(&mut self, expr: ast::Expr) -> Result<Option<Node>, Diagnostic> {
        self.source_text(expr.syntax())?;
        self.source_attributes(expr.syntax())?;
        source_fact(self.source.node(&ExprRef(expr)))
    }
    pub(super) fn checked_loop_target(
        &mut self,
        node: &ra_ap_syntax::SyntaxNode,
        label: Option<ast::Lifetime>,
    ) -> Result<Option<ControlTarget>, Diagnostic> {
        if let Some(label) = &label {
            self.source_text(label.syntax())?;
        }
        for parent in node.ancestors() {
            self.source_units(1)?;
            for child in parent.children() {
                self.source_units(1)?;
                if ast::Label::can_cast(child.kind()) {
                    self.source_text(&child)?;
                }
            }
            if ast::Fn::can_cast(parent.kind()) || ast::Const::can_cast(parent.kind()) {
                break;
            }
        }
        source_fact(loops::reached_target(node, label))
    }
}
