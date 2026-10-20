//! Availability of a single prepared compiler query, independently of neighbors.
//! Known(None), Known(false) and Known([]) retain their ordinary semantic value.

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum QueryAvailability {
    Unprepared,
    Known,
    NotApplicable,
}

#[derive(Debug)]
pub(super) enum QueryFact<T> {
    Unprepared,
    Known(T),
    NotApplicable,
}

impl<T> QueryFact<T> {
    pub(super) fn availability(&self) -> QueryAvailability {
        match self {
            Self::Unprepared => QueryAvailability::Unprepared,
            Self::Known(_) => QueryAvailability::Known,
            Self::NotApplicable => QueryAvailability::NotApplicable,
        }
    }

    pub(super) fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unprepared | Self::NotApplicable => None,
        }
    }

    pub(super) fn checked(&self, query: &str) -> Result<&T, crate::core::diagnostic::Diagnostic> {
        self.known().ok_or_else(|| {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-UPSTREAM-SOLVER-INPUT",
                format!("compiler query {query} is {:?}", self.availability()),
            )
        })
    }

    /// Callback use requires actual admission beforehand. Missing preparation
    /// is an adapter invariant fault, never a solver answer or empty default.
    pub(super) fn require(&self, query: &str) -> &T {
        match self {
            Self::Known(value) => value,
            Self::Unprepared => panic!("unprepared compiler query: {query}"),
            Self::NotApplicable => panic!("invalid compiler query domain: {query}"),
        }
    }
}

impl<T: Copy> QueryFact<T> {
    pub(super) fn copied(&self, query: &str) -> T {
        *self.require(query)
    }
}
