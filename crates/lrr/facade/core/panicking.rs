// `core::panicking`. The panic entry points (`panic`, `panic_fmt`,
// `panic_display`, ...) are closed facade functions installed into this
// module separately; this source declares the remaining items the library
// macros name.

// The comparison an `assert_eq!`/`assert_ne!`/`assert_matches!` failure
// reports (rustc's variant order).
pub enum AssertKind {
    Eq,
    Ne,
    Match,
}

impl crate::fmt::Debug for AssertKind {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        crate::fmt::Formatter::write_str(
            f,
            match self {
                AssertKind::Eq => "Eq",
                AssertKind::Ne => "Ne",
                AssertKind::Match => "Match",
            },
        )
    }
}
