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

// `assert_failed` / `assert_failed_inner`: the failure message of
// `assert_eq!`/`assert_ne!`, rendered through the operands' own `Debug`
// impls into one formatter buffer and raised as an ordinary panic. Real core
// formats with `panic!`; facade sources expand no macros, so the same text is
// written through the formatter directly (`vm/frame_format_calls.rs` reaches
// user `Debug` impls only through ordinary source calls).
pub fn assert_failed<T: crate::fmt::Debug + ?Sized, U: crate::fmt::Debug + ?Sized>(
    kind: AssertKind,
    left: &T,
    right: &U,
    args: crate::option::Option<crate::fmt::Arguments<'_>>,
) -> ! {
    let mut f = crate::fmt::lrr_formatter();
    crate::fmt::Formatter::write_str(&mut f, "assertion `left ");
    crate::fmt::Formatter::write_str(
        &mut f,
        match kind {
            AssertKind::Eq => "==",
            AssertKind::Ne => "!=",
            AssertKind::Match => "matches",
        },
    );
    crate::fmt::Formatter::write_str(&mut f, " right` failed");
    match args {
        crate::option::Option::Some(message) => {
            crate::fmt::Formatter::write_str(&mut f, ": ");
            crate::fmt::Formatter::write_str(&mut f, message);
        }
        crate::option::Option::None => {}
    }
    crate::fmt::Formatter::write_str(&mut f, "\n  left: ");
    crate::fmt::Debug::fmt(left, &mut f);
    crate::fmt::Formatter::write_str(&mut f, "\n right: ");
    crate::fmt::Debug::fmt(right, &mut f);
    let text = core::intrinsics::lrr_fmt_take(f.lrr_sink());
    core::panicking::panic_fmt(text)
}
