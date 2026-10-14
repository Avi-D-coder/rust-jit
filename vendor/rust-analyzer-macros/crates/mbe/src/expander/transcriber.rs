//! Transcriber takes a template, like `fn $ident() {}`, a set of bindings like
//! `$ident => foo`, interpolates variables in the template, to get `fn foo() {}`

use intern::{Symbol, sym};
use span::{Edition, Span};
use stdx::itertools::Itertools;
use tt::{Delimiter, TopSubtreeBuilder, iter::TtElement};

use super::TokensOrigin;
use crate::{
    ExpandError, ExpandErrorKind, ExpandResult, MetaTemplate,
    expander::{Binding, Bindings, Fragment},
    parser::{ConcatMetaVarExprElem, MetaVarKind, Op, RepeatKind, Separator},
};

impl<'t> Bindings<'t> {
    fn get(&self, name: &Symbol, span: Span) -> Result<&Binding<'t>, ExpandError> {
        match self.inner.get(name) {
            Some(binding) => Ok(binding),
            None => Err(ExpandError::new(
                span,
                ExpandErrorKind::UnresolvedBinding(Box::new(Box::from(name.as_str()))),
            )),
        }
    }

    fn get_fragment(
        &self,
        name: &Symbol,
        mut span: Span,
        nesting: &mut [NestingState],
        marker: impl Fn(&mut Span),
    ) -> Result<Fragment<'t>, ExpandError> {
        macro_rules! binding_err {
            ($($arg:tt)*) => { ExpandError::binding_error(span, format!($($arg)*)) };
        }

        let mut b = self.get(name, span)?;
        for nesting_state in nesting.iter_mut() {
            nesting_state.hit = true;
            b = match b {
                Binding::Fragment(_) => break,
                Binding::Missing(_) => {
                    nesting_state.at_end = true;
                    break;
                }
                Binding::Nested(bs) => {
                    if !nesting_state.observe_length(bs.len()) {
                        return Err(binding_err!(
                            "metavariable `{name}` has a different repetition length"
                        ));
                    }
                    bs.get(nesting_state.idx).ok_or_else(|| {
                        nesting_state.at_end = true;
                        binding_err!("could not find nested binding `{name}`")
                    })?
                }
                Binding::Empty => {
                    if !nesting_state.observe_length(0) {
                        return Err(binding_err!(
                            "metavariable `{name}` has a different repetition length"
                        ));
                    }
                    nesting_state.at_end = true;
                    return Err(binding_err!("could not find empty binding `{name}`"));
                }
            };
        }
        match b {
            Binding::Fragment(f) => Ok(f.clone()),
            // emit some reasonable default expansion for missing bindings,
            // this gives better recovery than emitting the `$fragment-name` verbatim
            Binding::Missing(it) => Ok({
                marker(&mut span);
                let mut builder = TopSubtreeBuilder::new(tt::Delimiter::invisible_spanned(span));
                match it {
                    MetaVarKind::Stmt => {
                        builder.push(tt::Leaf::Punct(tt::Punct {
                            span,
                            char: ';',
                            spacing: tt::Spacing::Alone,
                        }));
                    }
                    MetaVarKind::Block => {
                        builder.open(tt::DelimiterKind::Brace, span);
                        builder.close(span);
                    }
                    // FIXME: Meta and Item should get proper defaults
                    MetaVarKind::Meta | MetaVarKind::Item | MetaVarKind::Tt | MetaVarKind::Vis => {}
                    MetaVarKind::Path
                    | MetaVarKind::Ty
                    | MetaVarKind::Pat
                    | MetaVarKind::PatParam
                    | MetaVarKind::Expr(_)
                    | MetaVarKind::Ident => {
                        builder.push(tt::Leaf::Ident(tt::Ident {
                            sym: sym::missing,
                            span,
                            is_raw: tt::IdentIsRaw::No,
                        }));
                    }
                    MetaVarKind::Lifetime => {
                        builder.extend([
                            tt::Leaf::Punct(tt::Punct {
                                char: '\'',
                                span,
                                spacing: tt::Spacing::Joint,
                            }),
                            tt::Leaf::Ident(tt::Ident {
                                sym: sym::missing,
                                span,
                                is_raw: tt::IdentIsRaw::No,
                            }),
                        ]);
                    }
                    MetaVarKind::Literal => {
                        builder.push(tt::Leaf::Ident(tt::Ident {
                            sym: sym::missing,
                            span,
                            is_raw: tt::IdentIsRaw::No,
                        }));
                    }
                }
                Fragment::TokensOwned(builder.build())
            }),
            Binding::Nested(_) => {
                Err(binding_err!("expected simple binding, found nested binding `{name}`"))
            }
            Binding::Empty => {
                Err(binding_err!("expected simple binding, found empty binding `{name}`"))
            }
        }
    }
}

// LRR uses the same transcription algorithm in count-only mode before output allocation.
struct OutputBuilder<'a> {
    output: Option<tt::TopSubtreeBuilder>,
    tokens: usize,
    overflow: bool,
    open: Vec<(tt::DelimiterKind, bool)>,
    last_closed: Option<(tt::DelimiterKind, bool)>,
    byte_budget: Option<&'a crate::ByteBudget<'a>>,
    token_bytes: usize,
    metadata_bytes: usize,
    events: usize,
    spans: rustc_hash::FxHashMap<Span, (usize, usize)>,
    contexts: rustc_hash::FxHashMap<[u8; 32], (usize, usize)>,
    added: Vec<AddedRecord>,
    materialized_leaves: usize,
    plan: Option<ReplayPlan>,
    raw_tokens: usize,
    raw_peak: usize,
    raw_limit: Option<usize>,
}
struct ReplayPlan {
    iterations: Vec<usize>,
    cursor: usize,
    recording: bool,
}
enum AddedRecord {
    Span(Span),
    Context([u8; 32]),
}
#[derive(Clone, Copy)]
struct RestorePoint {
    output: Option<tt::SubtreeBuilderRestorePoint>,
    tokens: usize,
    open: usize,
    token_bytes: usize,
    events: usize,
    metadata_bytes: usize,
    added: usize,
    raw_tokens: usize,
}
impl<'a> OutputBuilder<'a> {
    fn new(
        call_site: Span,
        materialize: bool,
        byte_budget: Option<&'a crate::ByteBudget<'a>>,
    ) -> Self {
        let mut result = Self {
            output: materialize
                .then(|| tt::TopSubtreeBuilder::new(tt::Delimiter::invisible_spanned(call_site))),
            tokens: 1,
            overflow: false,
            open: Vec::new(),
            last_closed: None,
            byte_budget,
            token_bytes: 34,
            metadata_bytes: 0,
            events: 0,
            spans: rustc_hash::FxHashMap::default(),
            contexts: rustc_hash::FxHashMap::default(),
            added: Vec::new(),
            materialized_leaves: 0,
            raw_tokens: 1,
            raw_peak: 1,
            raw_limit: None,
            plan: (!materialize).then(|| ReplayPlan {
                iterations: Vec::new(),
                cursor: 0,
                recording: true,
            }),
        };
        result.span(call_site);
        result
    }
    fn admit_raw(&mut self, count: usize) {
        if self.output.is_none() {
            return;
        }
        let Some(tokens) = self
            .raw_tokens
            .checked_add(count)
            .filter(|tokens| self.raw_limit.is_none_or(|limit| *tokens <= limit))
        else {
            self.overflow = true;
            self.output = None;
            return;
        };
        self.raw_tokens = tokens;
        self.raw_peak = self.raw_peak.max(tokens);
    }
    fn materialized_leaves(&mut self, count: usize) {
        if let Some(leaves) = self.materialized_leaves.checked_add(count) {
            self.materialized_leaves = leaves;
        } else {
            self.overflow = true;
        }
    }
    fn enter_repeat(&mut self) -> Result<(Option<usize>, Option<usize>), ()> {
        let Some(plan) = &mut self.plan else {
            return Ok((None, None));
        };
        if plan.recording {
            plan.iterations.try_reserve(1).map_err(|_| ())?;
            let index = plan.iterations.len();
            plan.iterations.push(0);
            Ok((Some(index), None))
        } else {
            let count = *plan.iterations.get(plan.cursor).ok_or(())?;
            plan.cursor = plan.cursor.checked_add(1).ok_or(())?;
            Ok((None, Some(count)))
        }
    }
    fn plan_checkpoint(&self) -> Option<usize> {
        self.plan.as_ref().filter(|plan| plan.recording).map(|plan| plan.iterations.len())
    }
    fn discard_trial_plans(&mut self, checkpoint: Option<usize>) {
        if let (Some(plan), Some(checkpoint)) = (&mut self.plan, checkpoint) {
            plan.iterations.truncate(checkpoint);
        }
    }
    fn finish_repeat_plan(&mut self, index: Option<usize>, iterations: usize) -> Result<(), ()> {
        if let (Some(plan), Some(index)) = (&mut self.plan, index) {
            *plan.iterations.get_mut(index).ok_or(())? = iterations;
        }
        Ok(())
    }
    fn span(&mut self, span: Span) {
        let Some(budget) = self.byte_budget else {
            return;
        };
        let event = self.events;
        let Some(next) = event.checked_add(1) else {
            self.overflow = true;
            return;
        };
        self.events = next;
        if self.spans.contains_key(&span) {
            return;
        }
        let Some(cost) = (budget.span_cost)(span) else {
            self.overflow = true;
            return;
        };
        if self.spans.try_reserve(1).is_err() {
            self.overflow = true;
            return;
        }
        let mut extra = cost.span_bytes;
        let mut context =
            crate::ContextWireCost { context: cost.context, context_bytes: cost.context_bytes };
        let mut depth = 0_usize;
        while !self.contexts.contains_key(&context.context) {
            if depth > 256
                || self.added.try_reserve(2).is_err()
                || self.contexts.try_reserve(1).is_err()
            {
                self.overflow = true;
                return;
            }
            let Some(sum) = extra.checked_add(context.context_bytes) else {
                self.overflow = true;
                return;
            };
            extra = sum;
            self.contexts.insert(context.context, (event, context.context_bytes));
            self.added.push(AddedRecord::Context(context.context));
            match (budget.context_parent)(context.context) {
                Some(crate::ContextParent::Root) => break,
                Some(crate::ContextParent::Parent(parent)) => {
                    context = parent;
                    depth += 1;
                }
                None => {
                    self.overflow = true;
                    return;
                }
            }
        }
        if self.added.try_reserve(1).is_err() {
            self.overflow = true;
            return;
        }
        let Some(sum) = self.metadata_bytes.checked_add(extra) else {
            self.overflow = true;
            return;
        };
        self.metadata_bytes = sum;
        self.spans.insert(span, (event, cost.span_bytes));
        self.added.push(AddedRecord::Span(span));
    }
    fn bytes(&self) -> usize {
        self.token_bytes.checked_add(self.metadata_bytes).unwrap_or(usize::MAX)
    }
    fn add_bytes(&mut self, bytes: usize) {
        if let Some(sum) = self.token_bytes.checked_add(bytes) {
            self.token_bytes = sum;
        } else {
            self.overflow = true;
        }
    }
    fn measure_leaf(&mut self, leaf: &tt::Leaf) {
        if self.byte_budget.is_none() {
            return;
        }
        self.span(*leaf.span());
        match leaf {
            tt::Leaf::Punct(_) => self.add_bytes(10),
            tt::Leaf::Ident(ident) => {
                self.add_bytes(10);
                self.add_bytes(ident.sym.as_str().len());
            }
            tt::Leaf::Literal(literal) => {
                use std::fmt::Write;
                let mut length = TextLength(0);
                if write!(&mut length, "{literal}").is_err() {
                    self.overflow = true;
                }
                self.add_bytes(9);
                self.add_bytes(length.0);
            }
        }
    }
    fn add(&mut self, count: usize) {
        if let Some(tokens) = self.tokens.checked_add(count) {
            self.tokens = tokens;
        } else {
            self.overflow = true;
        }
    }
    fn push(&mut self, leaf: tt::Leaf) {
        self.add(1);
        self.measure_leaf(&leaf);
        self.admit_raw(1);
        if let Some(output) = &mut self.output {
            output.push(leaf);
            self.materialized_leaves =
                self.materialized_leaves.checked_add(1).unwrap_or(usize::MAX);
        }
    }
    fn extend(&mut self, leaves: impl IntoIterator<Item = tt::Leaf>) {
        for leaf in leaves {
            self.push(leaf);
        }
    }
    fn open(&mut self, kind: tt::DelimiterKind, span: Span) {
        self.add(1);
        self.add_bytes(14);
        self.span(span);
        self.open.push((kind, true));
        self.admit_raw(1);
        if let Some(output) = &mut self.output {
            output.open(kind, span);
        }
    }
    fn open_flattened(&mut self, span: Span) {
        self.open.push((tt::DelimiterKind::Invisible, false));
        self.admit_raw(1);
        if let Some(output) = &mut self.output {
            output.open(tt::DelimiterKind::Invisible, span);
        }
    }
    fn close(&mut self, span: Span) {
        self.last_closed = self.open.pop();
        if self.last_closed.is_some_and(|(_, counted)| counted) {
            self.span(span);
        }
        if let Some(output) = &mut self.output {
            output.close(span);
        }
    }
    fn extend_with_tt(&mut self, tree: tt::TokenTreesView<'_>) {
        if self.byte_budget.is_some() {
            for token in tree.iter_flat_tokens() {
                self.add(1);
                match token {
                    tt::TokenTree::Leaf(leaf) => self.measure_leaf(&leaf),
                    tt::TokenTree::Subtree(group) => {
                        self.add_bytes(14);
                        self.span(group.delimiter.open);
                        self.span(group.delimiter.close);
                    }
                }
            }
        } else {
            self.add(tree.iter_flat_tokens().count());
        }
        self.admit_raw(tree.iter_flat_tokens().count());
        if let Some(output) = &mut self.output {
            output.extend_with_tt(tree);
            let leaves = tree
                .iter_flat_tokens()
                .filter(|token| matches!(token, tt::TokenTree::Leaf(_)))
                .count();
            self.materialized_leaves(leaves);
        }
    }
    fn extend_with_tt_alone(&mut self, tree: tt::TokenTreesView<'_>) {
        if self.byte_budget.is_some() {
            for token in tree.iter_flat_tokens() {
                self.add(1);
                match token {
                    tt::TokenTree::Leaf(leaf) => self.measure_leaf(&leaf),
                    tt::TokenTree::Subtree(group) => {
                        self.add_bytes(14);
                        self.span(group.delimiter.open);
                        self.span(group.delimiter.close);
                    }
                }
            }
        } else {
            self.add(tree.iter_flat_tokens().count());
        }
        self.admit_raw(tree.iter_flat_tokens().count());
        if let Some(output) = &mut self.output {
            output.extend_with_tt_alone(tree);
            let leaves = tree
                .iter_flat_tokens()
                .filter(|token| matches!(token, tt::TokenTree::Leaf(_)))
                .count();
            self.materialized_leaves(leaves);
        }
    }
    fn remove_last_subtree_if_invisible(&mut self) {
        let closed = self.last_closed.take();
        if closed.is_some_and(|(kind, _)| kind == tt::DelimiterKind::Invisible)
            && self.output.is_some()
        {
            if let Some(tokens) = self.raw_tokens.checked_sub(1) {
                self.raw_tokens = tokens;
            } else {
                self.overflow = true;
            }
        }
        if closed == Some((tt::DelimiterKind::Invisible, true)) {
            if let Some(tokens) = self.tokens.checked_sub(1) {
                self.tokens = tokens;
            } else {
                self.overflow = true;
            }
        }
        if let Some(output) = &mut self.output {
            output.remove_last_subtree_if_invisible();
        }
    }
    fn restore_point(&mut self) -> RestorePoint {
        self.last_closed = None;
        RestorePoint {
            output: self.output.as_mut().map(|o| o.restore_point()),
            tokens: self.tokens,
            open: self.open.len(),
            token_bytes: self.token_bytes,
            events: self.events,
            metadata_bytes: self.metadata_bytes,
            added: self.added.len(),
            raw_tokens: self.raw_tokens,
        }
    }
    fn restore(&mut self, point: RestorePoint) {
        self.tokens = point.tokens;
        self.token_bytes = point.token_bytes;
        self.events = point.events;
        while self.added.len() > point.added {
            match self.added.pop() {
                Some(AddedRecord::Span(span)) => {
                    self.spans.remove(&span);
                }
                Some(AddedRecord::Context(context)) => {
                    self.contexts.remove(&context);
                }
                None => {
                    self.overflow = true;
                    break;
                }
            }
        }
        self.metadata_bytes = point.metadata_bytes;
        self.raw_tokens = point.raw_tokens;
        self.open.truncate(point.open);
        self.last_closed = None;
        if let (Some(output), Some(point)) = (&mut self.output, point.output) {
            output.restore(point);
        }
    }
    fn exceeded(&self, limit: usize) -> bool {
        self.overflow
            || self.tokens > limit
            || self.byte_budget.is_some_and(|budget| self.bytes() > budget.byte_limit)
    }
}
pub(super) fn transcribe(
    template: &MetaTemplate,
    bindings: &Bindings<'_>,
    marker: impl Fn(&mut Span) + Copy,
    call_site: Span,
    token_limit: Option<usize>,
    byte_budget: Option<&crate::ByteBudget<'_>>,
    stats: &mut crate::TranscriptionStats,
) -> ExpandResult<tt::TopSubtree> {
    let mut admitted_plan = None;
    if let Some(limit) = token_limit {
        let Some(calls) = stats.preflight_calls.checked_add(1) else {
            return ExpandResult::new(
                tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
                ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
            );
        };
        stats.preflight_calls = calls;
        let mut ctx = ExpandCtx { bindings, nesting: Vec::new(), call_site, token_limit };
        let mut counter = OutputBuilder::new(call_site, false, byte_budget);
        // Preview only immutable registered spans. The caller's materialization
        // marker still runs only after admission.
        let preview = |span: &mut Span| {
            if let Some(budget) = byte_budget {
                (budget.preview_marker)(span);
            }
        };
        let measured = expand_subtree(&mut ctx, template, &mut counter, preview);
        stats.max_preflight_tokens = stats.max_preflight_tokens.max(counter.tokens);
        stats.max_preflight_bytes = stats.max_preflight_bytes.max(counter.bytes());
        if let Some(error) = measured.err {
            return ExpandResult::new(
                tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
                error,
            );
        }
        if counter.exceeded(limit) {
            return ExpandResult::new(
                tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
                ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
            );
        }
        stats.admitted_tokens = counter.tokens;
        stats.admitted_bytes = counter.bytes();
        admitted_plan = counter.plan.take();
    }
    let mut ctx = ExpandCtx { bindings, nesting: Vec::new(), call_site, token_limit: None };
    let mut builder = OutputBuilder::new(call_site, true, None);
    if let Some(mut plan) = admitted_plan {
        plan.recording = false;
        builder.plan = Some(plan);
        // Owned definitions have at most 256 token-group levels. Only temporary
        // repetition wrappers remain beyond admitted final tokens during replay.
        let Some(raw_limit) = stats.admitted_tokens.checked_add(256) else {
            return ExpandResult::new(
                tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
                ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
            );
        };
        builder.raw_limit = Some(raw_limit);
    }
    let Some(materializations) = stats.materializations.checked_add(1) else {
        return ExpandResult::new(
            tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
            ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
        );
    };
    stats.materializations = materializations;
    let result = expand_subtree(&mut ctx, template, &mut builder, marker);
    stats.materialized_leaves = builder.materialized_leaves;
    stats.materialized_raw_peak = builder.raw_peak;
    if builder.overflow || builder.output.is_none() {
        return ExpandResult::new(
            tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
            ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
        );
    }
    if builder.plan.as_ref().is_some_and(|plan| plan.cursor != plan.iterations.len())
        || (builder.plan.is_some() && builder.tokens != stats.admitted_tokens)
    {
        return ExpandResult::new(
            tt::TopSubtree::empty(tt::DelimSpan::from_single(call_site)),
            ExpandError::new(call_site, ExpandErrorKind::LimitExceeded),
        );
    }
    result.map(|()| builder.output.unwrap().build())
}

#[derive(Debug)]
struct NestingState {
    idx: usize,
    /// `hit` is currently necessary to tell `expand_repeat` if it should stop
    /// because there is no variable in use by the current repetition
    hit: bool,
    /// `at_end` is currently necessary to tell `expand_repeat` if it should stop
    /// because there is no more value available for the current repetition
    at_end: bool,
    repetition_len: Option<usize>,
    mismatched_lengths: bool,
}

impl NestingState {
    fn observe_length(&mut self, length: usize) -> bool {
        if self.repetition_len.is_some_and(|expected| expected != length) {
            self.mismatched_lengths = true;
            false
        } else {
            self.repetition_len = Some(length);
            true
        }
    }
}

#[derive(Debug)]
struct ExpandCtx<'a> {
    bindings: &'a Bindings<'a>,
    nesting: Vec<NestingState>,
    call_site: Span,
    token_limit: Option<usize>,
}

fn expand_subtree_with_delimiter(
    ctx: &mut ExpandCtx<'_>,
    template: &MetaTemplate,
    builder: &mut OutputBuilder<'_>,
    delimiter: Option<Delimiter>,
    marker: impl Fn(&mut Span) + Copy,
) -> ExpandResult<()> {
    let flatten = delimiter.is_none();
    let delimiter = delimiter.unwrap_or_else(|| tt::Delimiter::invisible_spanned(ctx.call_site));
    if flatten {
        builder.open_flattened(delimiter.open);
    } else {
        builder.open(delimiter.kind, delimiter.open);
    }
    let result = expand_subtree(ctx, template, builder, marker);
    builder.close(delimiter.close);
    result
}

fn expand_subtree(
    ctx: &mut ExpandCtx<'_>,
    template: &MetaTemplate,
    builder: &mut OutputBuilder<'_>,
    marker: impl Fn(&mut Span) + Copy,
) -> ExpandResult<()> {
    let mut err = None;
    'ops: for op in template.iter() {
        match op {
            Op::Literal(it) => builder.push(tt::Leaf::from({
                let mut it = it.clone();
                marker(&mut it.span);
                it
            })),
            Op::Ident(it) => builder.push(tt::Leaf::from({
                let mut it = it.clone();
                marker(&mut it.span);
                it
            })),
            Op::Punct(puncts) => {
                builder.extend(puncts.iter().map(|punct| {
                    tt::Leaf::from({
                        let mut it = *punct;
                        marker(&mut it.span);
                        it
                    })
                }));
            }
            Op::Subtree { tokens, delimiter } => {
                let mut delimiter = *delimiter;
                marker(&mut delimiter.open);
                marker(&mut delimiter.close);
                let ExpandResult { value: (), err: e } =
                    expand_subtree_with_delimiter(ctx, tokens, builder, Some(delimiter), marker);
                err = err.or(e);
            }
            Op::Var { name, id, .. } => {
                let ExpandResult { value: (), err: e } =
                    expand_var(ctx, name, *id, builder, marker);
                err = err.or(e);
            }
            Op::Repeat { tokens: subtree, kind, separator } => {
                let ExpandResult { value: (), err: e } =
                    expand_repeat(ctx, subtree, *kind, separator.as_deref(), builder, marker);
                err = err.or(e);
            }
            Op::Ignore { name, id } => {
                // Expand the variable, but ignore the result. This registers the repetition count.
                let e = ctx.bindings.get_fragment(name, *id, &mut ctx.nesting, marker).err();
                // FIXME: The error gets dropped if there were any previous errors.
                // This should be reworked in a way where the errors can be combined
                // and reported rather than storing the first error encountered.
                err = err.or(e);
            }
            Op::Index { depth } => {
                let index =
                    ctx.nesting.get(ctx.nesting.len() - 1 - depth).map_or(0, |nest| nest.idx);
                builder.push(tt::Leaf::Literal(tt::Literal {
                    text_and_suffix: sym::Integer::get(index),
                    span: ctx.call_site,
                    kind: tt::LitKind::Integer,
                    suffix_len: 0,
                }));
            }
            Op::Len { depth } => {
                let length = ctx.nesting.get(ctx.nesting.len() - 1 - depth).map_or(0, |_nest| {
                    // FIXME: to be implemented
                    0
                });
                builder.push(tt::Leaf::Literal(tt::Literal {
                    text_and_suffix: sym::Integer::get(length),
                    span: ctx.call_site,
                    kind: tt::LitKind::Integer,
                    suffix_len: 0,
                }));
            }
            Op::Count { name, depth } => {
                let mut binding = match ctx.bindings.get(name, ctx.call_site) {
                    Ok(b) => b,
                    Err(e) => {
                        err = err.or(Some(e));
                        continue;
                    }
                };
                for state in ctx.nesting.iter_mut() {
                    state.hit = true;
                    match binding {
                        Binding::Fragment(_) | Binding::Missing(_) => {
                            // `count()` will report an error.
                            break;
                        }
                        Binding::Nested(bs) => {
                            if !state.observe_length(bs.len()) {
                                err = err.or(Some(ExpandError::binding_error(
                                    ctx.call_site,
                                    "metavariables have different repetition lengths",
                                )));
                                continue 'ops;
                            }
                            if let Some(b) = bs.get(state.idx) {
                                binding = b;
                            } else {
                                state.at_end = true;
                                continue 'ops;
                            }
                        }
                        Binding::Empty => {
                            state.at_end = true;
                            // FIXME: Breaking here and proceeding to `count()` isn't the most
                            // correct thing to do here. This could be a binding of some named
                            // fragment which we don't know the depth of, so `count()` will just
                            // return 0 for this no matter what `depth` is. See test
                            // `count_interaction_with_empty_binding` for example.
                            break;
                        }
                    }
                }

                let res = count(binding, 0, *depth);

                builder.push(tt::Leaf::Literal(tt::Literal {
                    text_and_suffix: sym::Integer::get(res),
                    span: ctx.call_site,
                    suffix_len: 0,
                    kind: tt::LitKind::Integer,
                }));
            }
            Op::Concat { elements, span: concat_span } => {
                let mut concatenated = String::new();
                for element in elements {
                    match element {
                        ConcatMetaVarExprElem::Ident(ident) => {
                            concatenated.push_str(ident.sym.as_str())
                        }
                        ConcatMetaVarExprElem::Literal(lit) => {
                            // FIXME: This isn't really correct wrt. escaping, but that's what rustc does and anyway
                            // escaping is used most of the times for characters that are invalid in identifiers.
                            concatenated.push_str(lit.text())
                        }
                        ConcatMetaVarExprElem::Var(var) => {
                            // Handling of repetitions in `${concat}` isn't fleshed out in rustc, so we currently
                            // err at it.
                            // FIXME: Do what rustc does for repetitions.
                            let var_value = match ctx.bindings.get_fragment(
                                &var.sym,
                                var.span,
                                &mut ctx.nesting,
                                marker,
                            ) {
                                Ok(var) => var,
                                Err(e) => {
                                    if err.is_none() {
                                        err = Some(e);
                                    };
                                    continue;
                                }
                            };
                            let values = match &var_value {
                                Fragment::Tokens { tree: tokens, .. } => {
                                    let mut iter = tokens.iter();
                                    (iter.next(), iter.next())
                                }
                                Fragment::TokensOwned(tokens) => {
                                    let mut iter = tokens.iter();
                                    (iter.next(), iter.next())
                                }
                                _ => (None, None),
                            };
                            let value = match &values {
                                (Some(TtElement::Leaf(tt::Leaf::Ident(ident))), None) => {
                                    ident.sym.as_str()
                                }
                                (Some(TtElement::Leaf(tt::Leaf::Literal(lit))), None) => lit.text(),
                                _ => {
                                    if err.is_none() {
                                        err = Some(ExpandError::binding_error(
                                            var.span,
                                            "metavariables of `${concat(..)}` must be of type `ident`, `literal` or `tt`",
                                        ))
                                    }
                                    continue;
                                }
                            };
                            concatenated.push_str(value);
                        }
                    }
                }

                // `${concat}` span comes from the macro (at least for now).
                // See https://github.com/rust-lang/rust/blob/b0af276da341/compiler/rustc_expand/src/mbe/transcribe.rs#L724-L726.
                let mut result_span = *concat_span;
                marker(&mut result_span);

                // FIXME: NFC normalize the result.
                if !rustc_lexer::is_ident(&concatenated) {
                    if err.is_none() {
                        err = Some(ExpandError::binding_error(
                            *concat_span,
                            "`${concat(..)}` is not generating a valid identifier",
                        ));
                    }
                    // Insert a dummy identifier for better parsing.
                    concatenated.clear();
                    concatenated.push_str("__ra_concat_dummy");
                }

                let needs_raw =
                    parser::SyntaxKind::from_keyword(&concatenated, Edition::LATEST).is_some();
                let is_raw = if needs_raw { tt::IdentIsRaw::Yes } else { tt::IdentIsRaw::No };
                builder.push(tt::Leaf::Ident(tt::Ident {
                    is_raw,
                    span: result_span,
                    sym: Symbol::intern(&concatenated),
                }));
            }
        }
        if ctx.nesting.is_empty() && ctx.token_limit.is_some_and(|limit| builder.exceeded(limit)) {
            return ExpandResult::new(
                (),
                ExpandError::new(ctx.call_site, ExpandErrorKind::LimitExceeded),
            );
        }
    }
    ExpandResult { value: (), err }
}

fn expand_var(
    ctx: &mut ExpandCtx<'_>,
    v: &Symbol,
    id: Span,
    builder: &mut OutputBuilder<'_>,
    marker: impl Fn(&mut Span) + Copy,
) -> ExpandResult<()> {
    // We already handle $crate case in mbe parser
    debug_assert!(*v != sym::crate_);

    match ctx.bindings.get_fragment(v, id, &mut ctx.nesting, marker) {
        Ok(fragment) => {
            match fragment {
                // rustc spacing is not like ours. Ours is like proc macros', it dictates how puncts will actually be joined.
                // rustc uses them mostly for pretty printing. So we have to deviate a bit from what rustc does here.
                // Basically, a metavariable can never be joined with whatever after it.
                Fragment::Tokens { tree, origin } => {
                    let view = match origin {
                        TokensOrigin::Raw => tree.strip_invisible(),
                        TokensOrigin::Ast => tree,
                    };
                    builder.extend_with_tt_alone(view);
                }
                Fragment::TokensOwned(tt) => {
                    builder.extend_with_tt_alone(tt.view().strip_invisible())
                }
                Fragment::Expr(sub) => {
                    let sub = sub.strip_invisible();
                    let mut span = id;
                    marker(&mut span);

                    // Check if this is a simple negative literal (MINUS + LITERAL)
                    // that should not be wrapped in parentheses
                    let is_negative_literal = matches!(
                        sub.iter().collect_array(),
                        Some([
                            tt::TtElement::Leaf(tt::Leaf::Punct(tt::Punct { char: '-', .. })),
                            tt::TtElement::Leaf(tt::Leaf::Literal(_))
                        ])
                    );

                    let wrap_in_parens = !is_negative_literal
                        && !matches!(sub.iter().collect_array(), Some([tt::TtElement::Leaf(_)]))
                        && sub.try_into_subtree().is_none_or(|it| {
                            it.top_subtree().delimiter.kind == tt::DelimiterKind::Invisible
                        });
                    if wrap_in_parens {
                        builder.open(tt::DelimiterKind::Parenthesis, span);
                    }
                    builder.extend_with_tt_alone(sub);
                    if wrap_in_parens {
                        builder.close(span);
                    }
                }
                Fragment::Path(tt) => fix_up_and_push_path_tt(ctx, builder, tt),
                Fragment::Empty => (),
            };
            ExpandResult::ok(())
        }
        Err(e) if matches!(e.inner.1, ExpandErrorKind::UnresolvedBinding(_)) => {
            // Note that it is possible to have a `$var` inside a macro which is not bound.
            // For example:
            // ```
            // macro_rules! foo {
            //     ($a:ident, $b:ident, $c:tt) => {
            //         macro_rules! bar {
            //             ($bi:ident) => {
            //                 fn $bi() -> u8 {$c}
            //             }
            //         }
            //     }
            // ```
            // We just treat it a normal tokens
            builder.extend([
                tt::Leaf::from(tt::Punct { char: '$', spacing: tt::Spacing::Alone, span: id }),
                tt::Leaf::from(tt::Ident { sym: v.clone(), span: id, is_raw: tt::IdentIsRaw::No }),
            ]);
            ExpandResult::ok(())
        }
        Err(e) => ExpandResult::only_err(e),
    }
}

fn expand_repeat(
    ctx: &mut ExpandCtx<'_>,
    template: &MetaTemplate,
    kind: RepeatKind,
    separator: Option<&Separator>,
    builder: &mut OutputBuilder<'_>,
    marker: impl Fn(&mut Span) + Copy,
) -> ExpandResult<()> {
    let (plan_index, planned_iterations) = match builder.enter_repeat() {
        Ok(plan) => plan,
        Err(()) => {
            return ExpandResult::new(
                (),
                ExpandError::new(ctx.call_site, ExpandErrorKind::LimitExceeded),
            );
        }
    };
    ctx.nesting.push(NestingState {
        idx: 0,
        at_end: false,
        hit: false,
        repetition_len: None,
        mismatched_lengths: false,
    });
    let mut counter = 0;
    let mut err = None;

    let initial_restore_point = builder.restore_point();
    let mut restore_point = builder.restore_point();
    loop {
        if planned_iterations.is_some_and(|iterations| counter >= iterations) {
            break;
        }
        let trial_plans = builder.plan_checkpoint();
        let ExpandResult { value: (), err: e } =
            expand_subtree_with_delimiter(ctx, template, builder, None, marker);
        let nesting_state = ctx.nesting.last_mut().unwrap();
        if nesting_state.mismatched_lengths {
            err = err.or(e);
            break;
        }
        if nesting_state.at_end {
            builder.discard_trial_plans(trial_plans);
            break;
        }
        if nesting_state.repetition_len.is_none() {
            builder.restore(initial_restore_point);
            restore_point = initial_restore_point;
            err = Some(ExpandError::binding_error(
                ctx.call_site,
                "repetition has no binding at this depth",
            ));
            break;
        }
        nesting_state.idx += 1;
        nesting_state.hit = false;

        builder.remove_last_subtree_if_invisible();

        restore_point = builder.restore_point();

        let Some(next) = counter.checked_add(1) else {
            err = Some(ExpandError::new(ctx.call_site, ExpandErrorKind::LimitExceeded));
            break;
        };
        counter = next;
        if !ctx.nesting.iter().any(|state| state.at_end)
            && ctx.token_limit.is_some_and(|limit| builder.exceeded(limit))
        {
            err = Some(ExpandError::new(ctx.call_site, ExpandErrorKind::LimitExceeded));
            break;
        }

        if e.is_some() {
            err = err.or(e);
            continue;
        }

        if planned_iterations.is_some_and(|iterations| counter >= iterations) {
            break;
        }
        if let Some(sep) = separator {
            match sep {
                Separator::Ident(ident) => builder.push(tt::Leaf::from(ident.clone())),
                Separator::Literal(lit) => builder.push(tt::Leaf::from(lit.clone())),
                Separator::Puncts(puncts) => {
                    for &punct in puncts {
                        builder.push(tt::Leaf::from(punct));
                    }
                }
                Separator::Lifetime(punct, ident) => {
                    builder.push(tt::Leaf::from(*punct));
                    builder.push(tt::Leaf::from(ident.clone()));
                }
            };
        }

        if RepeatKind::ZeroOrOne == kind {
            break;
        }
    }
    // Lose the last separator and last after-the-end round.
    builder.restore(restore_point);

    ctx.nesting.pop().unwrap();
    if builder.finish_repeat_plan(plan_index, counter).is_err() {
        err = Some(ExpandError::new(ctx.call_site, ExpandErrorKind::LimitExceeded));
    }

    // Check if it is a single token subtree without any delimiter
    // e.g {Delimiter:None> ['>'] /Delimiter:None>}

    if RepeatKind::OneOrMore == kind && counter == 0 && err.is_none() {
        err = Some(ExpandError::new(ctx.call_site, ExpandErrorKind::UnexpectedToken));
    }
    ExpandResult { value: (), err }
}

/// Inserts the path separator `::` between an identifier and its following generic
/// argument list, and then pushes into the buffer. See [`Fragment::Path`] for why
/// we need this fixup.
fn fix_up_and_push_path_tt(
    ctx: &ExpandCtx<'_>,
    builder: &mut OutputBuilder<'_>,
    subtree: tt::TokenTreesView<'_>,
) {
    let mut prev_was_ident = false;
    // Note that we only need to fix up the top-level `TokenTree`s because the
    // context of the paths in the descendant `Subtree`s won't be changed by the
    // mbe transcription.
    let mut iter = subtree.iter();
    while let Some(tt) = iter.next_as_view() {
        if prev_was_ident {
            // Pedantically, `(T) -> U` in `FnOnce(T) -> U` is treated as a generic
            // argument list and thus needs `::` between it and `FnOnce`. However in
            // today's Rust this type of path *semantically* cannot appear as a
            // top-level expression-context path, so we can safely ignore it.
            if let Some([tt::TtElement::Leaf(tt::Leaf::Punct(tt::Punct { char: '<', .. }))]) =
                tt.iter().collect_array()
            {
                builder.extend([
                    tt::Leaf::Punct(tt::Punct {
                        char: ':',
                        spacing: tt::Spacing::Joint,
                        span: ctx.call_site,
                    }),
                    tt::Leaf::Punct(tt::Punct {
                        char: ':',
                        spacing: tt::Spacing::Alone,
                        span: ctx.call_site,
                    }),
                ]);
            }
        }
        prev_was_ident =
            matches!(tt.iter().collect_array(), Some([tt::TtElement::Leaf(tt::Leaf::Ident(_))]));
        builder.extend_with_tt(tt);
    }
}

/// Handles `${count(t, depth)}`. `our_depth` is the recursion depth and `count_depth` is the depth
/// defined by the metavar expression.
fn count(binding: &Binding<'_>, depth_curr: usize, depth_max: usize) -> usize {
    match binding {
        Binding::Nested(bs) => {
            if depth_curr == depth_max {
                bs.len()
            } else {
                bs.iter().map(|b| count(b, depth_curr + 1, depth_max)).sum()
            }
        }
        Binding::Empty => 0,
        Binding::Fragment(_) | Binding::Missing(_) => 1,
    }
}

struct TextLength(usize);
impl std::fmt::Write for TextLength {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
