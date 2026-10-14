//! Nested compile-input selection through the existing hygiene and path namespaces.
use super::{
    compile_input_catalog::CompileInputCatalog,
    hygiene::HygieneFacts,
    macro_names::{MacroNamespaces, PathLookup},
    macro_origins::{ExpansionAncestry, MacroSelection},
    macro_scopes::{HygienicLookup, MacroDeclarations, MacroReference},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::{
        compile_input_identity::CompileInputDescriptor,
        include_arguments::NestedStringInvocation,
        scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes},
    },
    syntax::{
        imports::ImportSegment, outline::ScopeKey as SourceScope, parse::ParseKey, token_wire::Key,
    },
};
pub struct NestedLexicalContext {
    pub source: ParseKey,
    pub scope: SourceScope,
    pub textual_scope: ScopeKey,
    pub origin: Option<Key>,
}
pub enum NestedMacroLookup {
    Compiler {
        definition: MacroDefinition,
        descriptor: CompileInputDescriptor,
    },
    Other(MacroDefinition),
    NeedInput(Digest),
    Absent,
    Ambiguous(Vec<MacroDefinition>),
    Inaccessible,
}
pub struct NestedExpansionContext<'a> {
    pub environment: &'a crate::expand::builtins::FrozenCompileEnvironment,
    pub engine_version: Digest,
}
pub struct NestedExpansionSelection {
    lookup: NestedMacroLookup,
    key: Option<crate::expand::identity::ExpansionKey>,
}
impl NestedExpansionSelection {
    pub fn into_parts(
        self,
    ) -> (
        NestedMacroLookup,
        Option<crate::expand::identity::ExpansionKey>,
    ) {
        (self.lookup, self.key)
    }
    pub fn lookup(&self) -> &NestedMacroLookup {
        &self.lookup
    }
    pub fn key(&self) -> Option<&crate::expand::identity::ExpansionKey> {
        self.key.as_ref()
    }
}
pub struct NestedCompileInputResolver<'a> {
    pub namespaces: &'a MacroNamespaces,
    pub textual: &'a TextualScopes,
    pub declarations: &'a MacroDeclarations,
    pub hygiene: &'a HygieneFacts,
    pub ancestry: &'a ExpansionAncestry,
    pub compiler: &'a CompileInputCatalog,
}
pub(crate) struct InheritedCompileInputResolver<'a> {
    pub inner: NestedCompileInputResolver<'a>,
    pub outers: &'a [super::macro_scopes::OuterMacroScope<'a>],
    /// Declaration arenas use a retained lexical position, not physical span offsets.
    pub lexical_before: Option<u64>,
}
impl EagerMacroResolver for InheritedCompileInputResolver<'_> {
    fn compiler(&self) -> &CompileInputCatalog {
        self.inner.compiler
    }
    fn resolve_returned_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
        facts: &HygieneFacts,
        before: u64,
    ) -> Result<NestedExpansionSelection, Diagnostic> {
        let mut hygiene = self.inner.hygiene.clone();
        hygiene.extend(facts)?;
        let resolver = NestedCompileInputResolver {
            hygiene: &hygiene,
            ..self.inner
        };
        NestedExpansionSelection::from_lookup(
            resolver.resolve_at_with_outers(
                nested,
                caller,
                Some(self.lexical_before.unwrap_or(before)),
                self.outers,
            )?,
            nested,
            caller,
            context,
        )
    }
    fn resolve_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
    ) -> Result<NestedExpansionSelection, Diagnostic> {
        // The logical prefix cannot substitute for the original source identity.
        NestedCompileInputResolver::validate_original_reference(nested, caller)?;
        NestedExpansionSelection::from_lookup(
            self.inner
                .resolve_at_with_outers(nested, caller, self.lexical_before, self.outers)?,
            nested,
            caller,
            context,
        )
    }
}
/// Selection hook for the same eager worklist in captured and generated lexical worlds.
pub trait EagerMacroResolver {
    fn compiler(&self) -> &CompileInputCatalog;
    /// Captured-source callers use their physical token source. Generated callers
    /// may provide a separately retained, checked invocation-file base.
    fn validate_include_location(
        &self,
        site: &crate::syntax::token_wire::SpanRecord,
        from: &crate::source::include_paths::IncludeLocation,
        included: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Result<(), Diagnostic> {
        let fail = |message: &str| Diagnostic::new("LRR-E-COMPILE-INPUT", message.into());
        if from.path.as_str() != site.source.path {
            return Err(fail("eager source authority caller location mismatch"));
        }
        if let Some(view) = included
            && (view.location() != from
                || view.blob().content().0 != site.source.content.0
                || view.caller_context() != site.context
                || view.origin() != site.origin)
        {
            return Err(fail("eager included source view mismatch"));
        }
        Ok(())
    }
    fn resolve_returned_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
        facts: &HygieneFacts,
        before: u64,
    ) -> Result<NestedExpansionSelection, Diagnostic>;
    fn resolve_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
    ) -> Result<NestedExpansionSelection, Diagnostic>;
}
impl EagerMacroResolver for NestedCompileInputResolver<'_> {
    fn compiler(&self) -> &CompileInputCatalog {
        self.compiler
    }
    fn resolve_returned_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
        facts: &HygieneFacts,
        before: u64,
    ) -> Result<NestedExpansionSelection, Diagnostic> {
        let mut hygiene = self.hygiene.clone();
        hygiene.extend(facts)?;
        let resolver = NestedCompileInputResolver {
            hygiene: &hygiene,
            ..*self
        };
        NestedExpansionSelection::from_lookup(
            resolver.resolve_at(nested, caller, Some(before))?,
            nested,
            caller,
            context,
        )
    }
    fn resolve_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
    ) -> Result<NestedExpansionSelection, Diagnostic> {
        NestedCompileInputResolver::resolve_expansion(self, nested, caller, context)
    }
}
impl NestedExpansionSelection {
    pub(crate) fn from_lookup(
        lookup: NestedMacroLookup,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
    ) -> Result<Self, Diagnostic> {
        if context.environment.unit() != caller.scope.crate_key {
            return Err(fault("nested expansion consumer environment unit mismatch"));
        }
        let key = match &lookup {
            NestedMacroLookup::Compiler { definition, .. }
            | NestedMacroLookup::Other(definition) => {
                let reference = nested
                    .arguments()
                    .spans()
                    .iter()
                    .find(|s| s.key() == nested.reference_span())
                    .ok_or_else(|| fault("nested compiler reference span absent"))?;
                let key = crate::expand::identity::ExpansionKey {
                    macro_identity: match &lookup {
                        NestedMacroLookup::Compiler { .. } => {
                            crate::expand::identity::MacroIdentity::Builtin {
                                definition: definition.identity,
                            }
                        }
                        _ => crate::expand::identity::MacroIdentity::Declaration {
                            definition: definition.identity,
                        },
                    },
                    invocation: nested.identity(),
                    input_tokens: nested.argument_identity(),
                    consumer_compile_env: context.environment.fingerprint(),
                    parent_context: Digest(reference.context.0),
                    definition_edition: definition.definition_edition,
                    call_edition: caller.source.profile.edition,
                    grammar: crate::expand::identity::GrammarSlot::Expression,
                    engine_version: context.engine_version,
                    procedural_epoch: None,
                };
                key.validate()?;
                Some(key)
            }
            _ => None,
        };
        Ok(Self { lookup, key })
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl NestedCompileInputResolver<'_> {
    /// Bind a compiler key only after ordinary source/hygiene/name resolution.
    /// Other macros retain their definition outcome for the explicit tool/source path.
    pub fn resolve_expansion(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        context: &NestedExpansionContext<'_>,
    ) -> Result<NestedExpansionSelection, Diagnostic> {
        NestedExpansionSelection::from_lookup(
            self.resolve(nested, caller)?,
            nested,
            caller,
            context,
        )
    }
    /// Source context must come from the retained caller publication/entered frame.
    /// This selects candidates; it neither seals the namespace nor executes a macro.
    pub fn resolve(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
    ) -> Result<NestedMacroLookup, Diagnostic> {
        self.resolve_at(nested, caller, None)
    }
    fn resolve_at(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        before: Option<u64>,
    ) -> Result<NestedMacroLookup, Diagnostic> {
        self.resolve_at_with_outers(nested, caller, before, &[])
    }
    fn validate_original_reference(
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
    ) -> Result<(), Diagnostic> {
        caller.source.profile.validate_source()?;
        let reference = nested
            .arguments()
            .spans()
            .iter()
            .find(|r| r.key() == nested.reference_span())
            .ok_or_else(|| fault("nested macro reference span is absent"))?;
        if reference.source.unit.0 != caller.scope.crate_key.0
            || reference.source.content.0 != caller.source.content.0
            || (match reference.edition {
                crate::syntax::token_wire::Edition::E2015 => 2015,
                crate::syntax::token_wire::Edition::E2018 => 2018,
                crate::syntax::token_wire::Edition::E2021 => 2021,
                crate::syntax::token_wire::Edition::E2024 => 2024,
            }) != caller.source.profile.edition
        {
            return Err(fault("nested macro source context mismatch"));
        }
        Ok(())
    }
    fn resolve_at_with_outers(
        &self,
        nested: &NestedStringInvocation,
        caller: &NestedLexicalContext,
        before: Option<u64>,
        outers: &[super::macro_scopes::OuterMacroScope<'_>],
    ) -> Result<NestedMacroLookup, Diagnostic> {
        caller.source.profile.validate_source()?;
        let reference = nested
            .arguments()
            .spans()
            .iter()
            .find(|r| r.key() == nested.reference_span())
            .ok_or_else(|| fault("nested macro reference span is absent"))?;
        if before.is_none() {
            Self::validate_original_reference(nested, caller)?;
        }
        let parts = nested
            .segments()
            .iter()
            .map(|p| match p.as_str() {
                "crate" => ImportSegment::Crate,
                "self" => ImportSegment::SelfModule,
                "super" => ImportSegment::Super,
                _ => ImportSegment::Name(p.clone()),
            })
            .collect::<Vec<_>>();
        let name = nested
            .segments()
            .last()
            .ok_or_else(|| fault("nested macro member is absent"))?;
        let name = MacroName::parse(name, caller.source.profile.edition)?;
        let textual = self.declarations.resolve_with_outers(
            self.textual,
            MacroReference {
                scope: caller.textual_scope,
                before: before.unwrap_or(u64::from(reference.start)),
                name: &name,
                context: reference.context,
                origin: caller.origin,
            },
            outers,
            self.hygiene,
            self.ancestry,
        )?;
        let lookup = match textual {
            HygienicLookup::NeedInput(input) => PathLookup::NeedInput(input),
            HygienicLookup::Selection(selection) if !nested.absolute() && parts.len() == 1 => {
                match selection {
                    MacroSelection::Defined(definition) => PathLookup::Defined(definition),
                    MacroSelection::Ambiguous(definitions) => PathLookup::Ambiguous(definitions),
                    MacroSelection::Absent => {
                        self.lookup_path(&parts, nested.absolute(), caller)?
                    }
                }
            }
            _ => self.lookup_path(&parts, nested.absolute(), caller)?,
        };
        Ok(match lookup {
            PathLookup::Defined(definition) => match self.compiler.select(&definition)? {
                Some(descriptor) => NestedMacroLookup::Compiler {
                    definition,
                    descriptor: descriptor.clone(),
                },
                None => NestedMacroLookup::Other(definition),
            },
            PathLookup::NeedInput(input) => NestedMacroLookup::NeedInput(input),
            PathLookup::Absent => NestedMacroLookup::Absent,
            PathLookup::Ambiguous(definitions) => NestedMacroLookup::Ambiguous(definitions),
            PathLookup::Inaccessible(_) => NestedMacroLookup::Inaccessible,
        })
    }
    fn lookup_path(
        &self,
        parts: &[ImportSegment],
        absolute: bool,
        caller: &NestedLexicalContext,
    ) -> Result<PathLookup, Diagnostic> {
        self.namespaces.lookup_invocation_segments(
            parts,
            absolute,
            &caller.scope,
            caller.source.profile.edition,
        )
    }
}
