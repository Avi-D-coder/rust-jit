//! Shared diagnostic projection for validated, ordered origin intervals.
//! Locations remain distinct even when they select adjacent output tokens.
use super::SourceRange;
use crate::core::diagnostic::Diagnostic;

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SOURCE-MAP", message.into())
}
fn limit(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", message.into())
}

/// Both callers own canonical intervals produced by the pinned adapter and
/// validate them before retention. Resolve only the reached interval slice;
/// do not clone the complete origin/context graph for each diagnostic.
pub(crate) fn project<'a, O, S: PartialEq + 'a>(
    text: &str,
    origins: &[O],
    range: SourceRange,
    interval: impl Fn(&O) -> (u32, u32),
    resolve: impl Fn(&O) -> Result<&'a S, Diagnostic>,
) -> Result<Vec<&'a S>, Diagnostic> {
    SourceRange::checked(range.start, range.end)?;
    if text.get(range.start as usize..range.end as usize).is_none() {
        return Err(invalid("origin range is outside its UTF8 arena"));
    }
    if origins.len() > 2_000_000 {
        return Err(limit("diagnostic origin interval count limit"));
    }
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(64)
        .map_err(|_| limit("diagnostic span allocation"))?;
    let first = origins.partition_point(|origin| interval(origin).1 <= range.start);
    for origin in origins[first..].iter().take_while(|origin| {
        let (start, _) = interval(origin);
        if range.is_empty() {
            start <= range.start
        } else {
            start < range.end
        }
    }) {
        let (start, end) = interval(origin);
        if start >= end || end as usize > text.len() {
            return Err(invalid("diagnostic origin interval extent differs"));
        }
        let span = resolve(origin)?;
        if !selected.contains(&span) {
            if selected.len() == 64 {
                return Err(limit("diagnostic origin count limit"));
            }
            selected.push(span);
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn select<'a>(
        text: &str,
        origins: &[(u32, u32, usize)],
        spans: &'a [usize],
        range: SourceRange,
    ) -> Result<Vec<&'a usize>, Diagnostic> {
        project(
            text,
            origins,
            range,
            |&(start, end, _)| (start, end),
            |&(_, _, index)| {
                spans
                    .get(index)
                    .ok_or_else(|| invalid("fixture span absent"))
            },
        )
    }
    #[test]
    fn empty_ranges_choose_the_right_interval_and_eof_has_no_origin() {
        let origins = [(0, 1, 0), (1, 2, 1)];
        assert_eq!(
            select("ab", &origins, &[7, 8], SourceRange { start: 1, end: 1 }).unwrap(),
            [&8]
        );
        assert!(
            select("ab", &origins, &[7, 8], SourceRange { start: 2, end: 2 })
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            select("ab", &origins, &[7, 8], SourceRange { start: 0, end: 2 }).unwrap(),
            [&7, &8]
        );
    }
    #[test]
    fn repeated_origins_do_not_exhaust_the_distinct_origin_budget() {
        let text = "a".repeat(65);
        let origins = (0..65).map(|i| (i, i + 1, 0)).collect::<Vec<_>>();
        assert_eq!(
            select(&text, &origins, &[7], SourceRange { start: 0, end: 65 }).unwrap(),
            [&7]
        );
        let origins = (0..65).map(|i| (i, i + 1, i as usize)).collect::<Vec<_>>();
        let spans = (0..65).collect::<Vec<_>>();
        assert_eq!(
            select(&text, &origins, &spans, SourceRange { start: 0, end: 65 })
                .unwrap_err()
                .code,
            "LRR-E-METADATA-LIMIT"
        );
        assert_eq!(
            select(&text, &origins, &spans, SourceRange { start: 0, end: 64 })
                .unwrap()
                .len(),
            64
        );
    }
    #[test]
    fn malformed_utf8_offsets_and_unresolved_spans_are_explicit_errors() {
        assert_eq!(
            select("é", &[(0, 2, 0)], &[7], SourceRange { start: 0, end: 1 })
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
        assert_eq!(
            select("a", &[(0, 1, 0)], &[], SourceRange { start: 0, end: 1 })
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
        assert_eq!(
            select("a", &[(0, 2, 0)], &[7], SourceRange { start: 0, end: 1 })
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-SOURCE-MAP"
        );
    }
}
