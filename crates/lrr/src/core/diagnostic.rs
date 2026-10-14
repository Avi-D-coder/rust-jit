use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub macro_origin: Option<Box<crate::syntax::origin::MacroOrigin>>,
    pub rendered_macro_origin: Option<Box<crate::syntax::origin::RenderedMacroOrigin>>,
    pub generated_origin: Option<Box<crate::syntax::origin::GeneratedOperationOrigin>>,
    pub rendered_generated_origin: Option<Box<crate::syntax::origin::RenderedGeneratedOrigin>>,
    pub guest_stack: Option<Box<crate::syntax::origin::GuestStackOrigin>>,
    pub rendered_guest_stack: Option<Box<crate::syntax::origin::RenderedGuestStack>>,
    pub origin_attachment_failure: Option<&'static str>,
    /// Dispatcher facts stay local; query errors construct this as absent. Wire
    /// forwarding and the full worker diagnostic protocol have separate gates.
    #[serde(skip)]
    pub(crate) failure_context: Option<Box<crate::engine::failure::FailureContext>>,
}
impl Diagnostic {
    pub fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = self.message.capacity();
        if let Some(stack) = &self.guest_stack {
            bytes = bytes.checked_add(stack.heap_bytes()?)?;
        }
        if let Some(stack) = &self.rendered_guest_stack {
            bytes = bytes
                .checked_add(std::mem::size_of_val(stack.as_ref()))?
                .checked_add(stack.frames.capacity().checked_mul(std::mem::size_of::<
                    Vec<Option<crate::syntax::origin::OriginLocation>>,
                >())?)?;
            for frame in &stack.frames {
                bytes = bytes.checked_add(frame.capacity().checked_mul(std::mem::size_of::<
                    Option<crate::syntax::origin::OriginLocation>,
                >())?)?;
                for at in frame.iter().flatten() {
                    bytes = bytes.checked_add(at.heap_bytes()?)?;
                }
            }
        }
        if let Some(context) = &self.failure_context {
            bytes = bytes.checked_add(context.heap_bytes()?)?;
        }
        if let Some(origin) = &self.generated_origin {
            bytes = bytes.checked_add(origin.heap_bytes()?)?;
        }
        if let Some(rendered) = &self.rendered_generated_origin {
            bytes = bytes
                .checked_add(std::mem::size_of_val(rendered.as_ref()))?
                .checked_add(rendered.tokens.capacity().checked_mul(std::mem::size_of::<
                    Option<crate::syntax::origin::OriginLocation>,
                >())?)?;
            bytes = bytes.checked_add(rendered.parents.capacity().checked_mul(
                std::mem::size_of::<Option<crate::syntax::origin::OriginLocation>>(),
            )?)?;
            for at in rendered
                .invocation
                .iter()
                .chain(rendered.definition.iter())
                .chain(rendered.tokens.iter().flatten())
                .chain(rendered.parents.iter().flatten())
            {
                bytes = bytes.checked_add(at.heap_bytes()?)?;
            }
        }
        if let Some(origin) = &self.macro_origin {
            bytes = bytes.checked_add(origin.heap_bytes()?)?;
        }
        if let Some(rendered) = &self.rendered_macro_origin {
            bytes = bytes.checked_add(std::mem::size_of_val(rendered.as_ref()))?;
            bytes = bytes.checked_add(rendered.parents.capacity().checked_mul(
                std::mem::size_of::<Option<crate::syntax::origin::OriginLocation>>(),
            )?)?;
            for at in rendered.parents.iter().flatten() {
                bytes = bytes.checked_add(at.heap_bytes()?)?;
            }
            for at in [
                &rendered.invocation,
                &rendered.definition,
                &rendered.failure,
            ]
            .into_iter()
            .flatten()
            {
                bytes = bytes.checked_add(at.heap_bytes()?)?;
            }
        }
        Some(bytes)
    }
    /// Owned host diagnostic; constructing it performs no effects or queries.
    pub fn new(code: &'static str, message: String) -> Self {
        Self {
            code,
            message,
            macro_origin: None,
            rendered_macro_origin: None,
            generated_origin: None,
            rendered_generated_origin: None,
            guest_stack: None,
            rendered_guest_stack: None,
            origin_attachment_failure: None,
            failure_context: None,
        }
    }
    pub(crate) fn is_internal(&self) -> bool {
        matches!(
            self.code,
            "LRR-E-INTERNAL-IR" | "LRR-E-INTERNAL-SNAPSHOT" | "LRR-E-INTERNAL-SOURCE-MAP"
        )
    }
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(rendered) = &self.rendered_macro_origin {
            if let Some(at) = &rendered.invocation {
                write!(
                    f,
                    "\n  --> {}:{}:{}: macro invocation",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column
                )?;
                at.write_excerpt(f)?;
            }
            if rendered.omitted_parent_count > 0 {
                write!(
                    f,
                    "\n  = {} outer macro expansion frames omitted",
                    rendered.omitted_parent_count
                )?;
            }
            if let Some(at) = &rendered.definition {
                let name = self
                    .macro_origin
                    .as_ref()
                    .and_then(|o| o.definition.as_ref())
                    .map(|d| d.name.as_str())
                    .unwrap_or("macro");
                write!(
                    f,
                    "\n  --> {}:{}:{}: definition of `{}`",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column,
                    name
                )?;
                at.write_excerpt(f)?;
            }
            for at in rendered.parents.iter().flatten() {
                write!(
                    f,
                    "\n  --> {}:{}:{}: in macro expansion invoked here",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column
                )?;
                at.write_excerpt(f)?;
            }
            if let Some(at) = &rendered.failure {
                let label = if self.macro_origin.as_ref().is_some_and(|origin| {
                    matches!(
                        origin.failure_kind,
                        Some(crate::syntax::origin::MacroFailureKind::UnresolvedGeneratedProducer)
                    )
                }) {
                    "generated macro producer"
                } else {
                    "matcher failure"
                };
                write!(
                    f,
                    "\n  --> {}:{}:{}: {}",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column,
                    label
                )?;
                at.write_excerpt(f)?;
            }
        }
        if let Some(rendered) = &self.rendered_generated_origin {
            if let Some(at) = &rendered.invocation {
                write!(
                    f,
                    "\n  --> {}:{}:{}: generated body macro invocation",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column
                )?;
                at.write_excerpt(f)?;
            }
            if let Some(at) = &rendered.definition {
                let name = self
                    .generated_origin
                    .as_ref()
                    .and_then(|origin| origin.definition.as_ref())
                    .map_or("macro", |definition| definition.name.as_str());
                write!(
                    f,
                    "\n  --> {}:{}:{}: definition of `{}`",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column,
                    name
                )?;
                at.write_excerpt(f)?;
            }
            for at in rendered.parents.iter().flatten() {
                write!(
                    f,
                    "\n  --> {}:{}:{}: parent expansion invocation",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column
                )?;
                at.write_excerpt(f)?;
            }
            if rendered.omitted_parents != 0 {
                write!(
                    f,
                    "\n  = {} parent expansion origins omitted",
                    rendered.omitted_parents
                )?;
            }
            for at in rendered.tokens.iter().flatten() {
                write!(
                    f,
                    "\n  --> {}:{}:{}: generated body token",
                    at.file.as_str(),
                    at.start_line,
                    at.start_column
                )?;
                at.write_excerpt(f)?;
            }
            if rendered.omitted_tokens != 0 {
                write!(
                    f,
                    "\n  = {} generated token origins omitted",
                    rendered.omitted_tokens
                )?;
            }
        }
        if let Some(stack) = &self.rendered_guest_stack {
            for (i, frame) in stack.frames.iter().enumerate() {
                for at in frame.iter().flatten() {
                    write!(
                        f,
                        "\n  --> {}:{}:{}: guest frame {}",
                        at.file.as_str(),
                        at.start_line,
                        at.start_column,
                        i
                    )?;
                    at.write_excerpt(f)?;
                }
            }
            if stack.omitted_frames != 0 || stack.omitted_locations != 0 {
                write!(
                    f,
                    "\n  = {} guest frames and {} frame locations omitted",
                    stack.omitted_frames, stack.omitted_locations
                )?;
            }
        }
        if let Some(code) = self.origin_attachment_failure {
            write!(f, "\n  = generated origin attachment unavailable: {code}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Diagnostic {}
