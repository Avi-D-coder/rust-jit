//! Immutable pinned representation, not source/expansion or execution authority.
use super::*;
use syntax::GreenNode;

/// The green tree and exact origin/store allocations are shared by every fork.
/// No private Salsa ID or allocation address participates in `identity`.
pub struct SyntaxAttachmentSeed {
    root: GreenNode,
    grammar: DeclarationGrammar,
    origins: Arc<SpanMap>,
    store: Arc<Mutex<SpanStore>>,
    retained_input_bytes: usize,
    identity: Key,
    text_len: u32,
    metadata_bytes: usize,
}

fn add(a: usize, b: usize) -> Result<usize, AdapterError> {
    a.checked_add(b)
        .ok_or_else(|| error("attachment seed metadata budget"))
}

impl SyntaxAttachment {
    /// Consume an actual pinned tree. This cold validation/hash/size pass runs
    /// once; subsequent forks do not parse, materialize, walk or hash it.
    pub fn into_seed(self) -> Result<SyntaxAttachmentSeed, AdapterError> {
        check_tree(&self.root)?;
        let text_len = u32::from(self.root.text_range().len());
        check_origins(&self.origins, text_len)?;
        let identity = self.syntax_identity()?;
        let mut metadata_bytes = std::mem::size_of::<SyntaxAttachmentSeed>();
        // Conservative pinned green-node/token/child-array estimate. Rowan
        // shares unchanged children; charging the complete tree once is safe
        // for a seed owner even when another seed shares a subtree.
        for event in self.root.preorder_with_tokens() {
            if let WalkEvent::Enter(element) = event {
                metadata_bytes = add(metadata_bytes, 128)?;
                if let syntax::NodeOrToken::Token(token) = element {
                    metadata_bytes = add(metadata_bytes, token.text().len())?;
                }
            }
        }
        let intervals = self.origins.iter().count();
        let origin_bytes = intervals
            .checked_mul(2 * std::mem::size_of::<(span::TextSize, span::Span)>())
            .and_then(|n| n.checked_add(std::mem::size_of::<SpanMap>()))
            .ok_or_else(|| error("attachment origin metadata overflow"))?;
        metadata_bytes = add(metadata_bytes, origin_bytes)?;
        metadata_bytes = add(metadata_bytes, 4 * std::mem::size_of::<usize>())?;
        metadata_bytes = add(metadata_bytes, std::mem::size_of::<Mutex<SpanStore>>())?;
        {
            let store = self
                .store
                .lock()
                .map_err(|_| error("attachment span store lock poisoned"))?;
            metadata_bytes = add(metadata_bytes, store.metadata_bytes()?)?;
        }
        Ok(SyntaxAttachmentSeed {
            root: self.root.green().to_owned(),
            grammar: self.grammar,
            origins: self.origins,
            store: self.store,
            retained_input_bytes: self.retained_input_bytes,
            identity,
            text_len,
            metadata_bytes,
        })
    }
}

impl SyntaxAttachmentSeed {
    /// A local red-tree handle over the same immutable green tree. Only actual
    /// output attachment creates new origins/store data; failure leaves seed
    /// and other forks intact. Mutex protects the pinned Send-only Salsa store.
    pub fn fork(&self) -> Result<SyntaxAttachment, AdapterError> {
        if self.store.is_poisoned() {
            return Err(error("attachment span store lock poisoned"));
        }
        Ok(SyntaxAttachment {
            root: SyntaxNode::new_root(self.root.clone()),
            grammar: self.grammar,
            origins: Arc::clone(&self.origins),
            store: Arc::clone(&self.store),
            retained_input_bytes: self.retained_input_bytes,
        })
    }
    pub fn identity(&self) -> Key {
        self.identity
    }
    pub fn text_len(&self) -> u32 {
        self.text_len
    }
    /// Full seed-owned structural/map estimate, shared payload charged once.
    /// Includes reserved hash capacities and owned hygiene tables. The pinned
    /// Salsa arena behind its fixed Storage handle has no stable non-cancelling
    /// size API and is excluded: this is not full Section32.2 acceptance. Fork
    /// creates no new arena entries; real output imports retain existing debt.
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod tests;
