//! Developer-only exclusive-time spans, enabled by `LRR_PROFILE=1`. Disabled
//! spans cost one cached environment check; nothing here affects semantics.
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("LRR_PROFILE").is_some())
}

#[derive(Default)]
struct Totals {
    stack: Vec<(String, Instant, Duration)>,
    exclusive: HashMap<String, (Duration, Duration, u64)>,
}
thread_local! {
    static TOTALS: RefCell<Totals> = RefCell::new(Totals::default());
}

pub struct Span {
    active: bool,
}
impl Span {
    pub fn enter(name: impl FnOnce() -> String) -> Self {
        if !enabled() {
            return Self { active: false };
        }
        let name = name();
        TOTALS.with(|totals| {
            totals
                .borrow_mut()
                .stack
                .push((name, Instant::now(), Duration::ZERO))
        });
        Self { active: true }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        TOTALS.with(|totals| {
            let mut totals = totals.borrow_mut();
            let Some((name, start, children)) = totals.stack.pop() else {
                return;
            };
            let inclusive = start.elapsed();
            if name.starts_with("stage ") {
                // Developer-only: wall-clock stage boundaries, for correlating
                // an external memory timeline with the stage that produced it.
                static ORIGIN: OnceLock<Instant> = OnceLock::new();
                let origin = *ORIGIN.get_or_init(|| start);
                let unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                eprintln!(
                    "LRR_STAGE end_ms={:.0} dur_ms={:.0} unix_ms={} {}",
                    origin.elapsed().as_secs_f64() * 1e3,
                    inclusive.as_secs_f64() * 1e3,
                    unix_ms,
                    name
                );
            }
            if let Some(parent) = totals.stack.last_mut() {
                parent.2 += inclusive;
            }
            let entry = totals.exclusive.entry(name).or_default();
            entry.0 += inclusive.saturating_sub(children);
            entry.1 += inclusive;
            entry.2 += 1;
        });
    }
}

pub fn report() {
    if !enabled() {
        return;
    }
    TOTALS.with(|totals| {
        let totals = totals.borrow();
        let mut rows: Vec<_> = totals.exclusive.iter().collect();
        rows.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        eprintln!("LRR_PROFILE exclusive_ms inclusive_ms count name");
        for (name, (exclusive, inclusive, count)) in rows.iter().take(40) {
            eprintln!(
                "LRR_PROFILE {:>10.1} {:>10.1} {:>7} {}",
                exclusive.as_secs_f64() * 1e3,
                inclusive.as_secs_f64() * 1e3,
                count,
                name
            );
        }
    });
}
