//! Smoke tests for the telemetry module.
//!
//! These tests exercise the fmt-only fallback (no OTLP collector
//! required) and the "init-once" contract. The OTLP-over-gRPC wire
//! path needs a running collector and is documented as a manual
//! recipe in `docs/observability/README.md`.
//!
//! Note: each `#[test]` runs in its own integration-test binary, so
//! the process-global `INITIALIZED` flag is fresh per test. (Cargo
//! compiles each `tests/*.rs` file as its own binary by default —
//! but multiple `#[test]` fns inside ONE file share a process and
//! the same global subscriber. We deliberately keep `init(None)` to
//! a single call in this file and put the "second init returns Err"
//! check in a unit test inside `src/telemetry.rs` where we can reset
//! the flag.)

use notes_api::telemetry;

#[test]
fn fallback_init_returns_ok() {
    // Reset the flag is NOT possible from an integration test (the
    // symbol is private). So we make exactly one init call and
    // assert it succeeds. The second-init-returns-Err contract is
    // exercised by the unit test in src/telemetry.rs.
    let guard = telemetry::init(None);
    assert!(
        guard.is_ok(),
        "telemetry::init(None) should install the fmt-only fallback cleanly, got: {:?}",
        guard.err()
    );

    // Holding the guard demonstrates the RAII contract. When this
    // test function returns, Drop runs; for the fmt-only branch
    // it's a no-op (no exporter to flush) and must not panic.
    drop(guard);
}
