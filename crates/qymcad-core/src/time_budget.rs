//! THE BUDGETS OF TIME OF THE PROBES. A probe of time measures the machine as much as the program: run among the other
//! checks of a crate it reads their load - a budget of 3 s read over it on a sketch of 1 200 segments that takes 1.66 s
//! alone, three runs of nine. So a budget holds at the level of the probes of time alone (`QYMCAD_TIER=time`,
//! `tools/gate.py time`), where they run one at a time, and is scaled there by `QYMCAD_TIME_SCALE` (CI sets 2). What a
//! probe of time checks besides the time - that the work was done, and done right - holds at every level.

use std::time::Duration;

/// Whether this run is the level of the probes of time.
pub fn at_the_level_of_time() -> bool {
    std::env::var("QYMCAD_TIER").as_deref() == Ok("time")
}

/// The budget on this machine: the budget times `QYMCAD_TIME_SCALE`, 1 when unset.
pub fn scaled(budget: Duration) -> Duration {
    let k = std::env::var("QYMCAD_TIME_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|k| *k >= 1.0).unwrap_or(1.0);
    budget.mul_f64(k)
}

/// Whether `took` keeps within `budget`: always, but at the level of the probes of time, against the scaled budget.
pub fn within(took: Duration, budget: Duration) -> bool {
    !at_the_level_of_time() || took <= scaled(budget)
}
