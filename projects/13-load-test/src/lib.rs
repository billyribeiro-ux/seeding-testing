//! load-test — the tiny load generator the perf-retrospective template
//! reaches for.
//!
//! Why write our own instead of `oha` / `k6`? Because the curriculum
//! pairs the harness with `docs/perf/methodology.md` which expects a
//! very specific report shape (per-status-class counts + p50/p95/p99
//! latency, no other noise). A 100-line in-tree implementation is
//! easier to teach than configuring k6.
//!
//! The library half is just the metric aggregator + percentile math;
//! the binary half (`src/main.rs`) is the runner that fans out tokio
//! tasks and feeds them into this aggregator.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// One observation produced by a single completed request.
#[derive(Debug, Clone)]
pub struct Sample {
    pub status: u16,
    pub duration: Duration,
}

/// The summary written to JSON + printed to stdout at the end of a run.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub total_requests: u64,
    pub status_counts: StatusClassCounts,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub mean_ms: u64,
    pub max_ms: u64,
    pub failed: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct StatusClassCounts {
    pub class_2xx: u64,
    pub class_3xx: u64,
    pub class_4xx: u64,
    pub class_5xx: u64,
    pub network_error: u64,
}

/// Build a `Report` from a vector of samples (and a count of how many
/// requests failed outright before they could produce a sample).
#[must_use]
pub fn summarize(samples: &mut [Sample], network_failures: u64) -> Report {
    samples.sort_by_key(|s| s.duration);
    let total = samples.len() as u64 + network_failures;
    let mut counts = StatusClassCounts {
        network_error: network_failures,
        ..StatusClassCounts::default()
    };
    let mut latency_sum = Duration::ZERO;
    for s in samples.iter() {
        match s.status / 100 {
            2 => counts.class_2xx += 1,
            3 => counts.class_3xx += 1,
            4 => counts.class_4xx += 1,
            5 => counts.class_5xx += 1,
            _ => counts.network_error += 1,
        }
        latency_sum += s.duration;
    }
    let mean_ms = if samples.is_empty() {
        0
    } else {
        (latency_sum.as_millis() as u64) / samples.len() as u64
    };
    let max_ms = samples
        .last()
        .map(|s| s.duration.as_millis() as u64)
        .unwrap_or_default();
    Report {
        total_requests: total,
        status_counts: counts,
        p50_ms: percentile_ms(samples, 0.50),
        p95_ms: percentile_ms(samples, 0.95),
        p99_ms: percentile_ms(samples, 0.99),
        mean_ms,
        max_ms,
        failed: network_failures + samples.iter().filter(|s| s.status >= 500).count() as u64,
    }
}

fn percentile_ms(sorted: &[Sample], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    debug_assert!((0.0..=1.0).contains(&p));
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx].duration.as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(status: u16, ms: u64) -> Sample {
        Sample {
            status,
            duration: Duration::from_millis(ms),
        }
    }

    #[test]
    fn empty_run_yields_all_zeros() {
        let mut samples = Vec::<Sample>::new();
        let r = summarize(&mut samples, 0);
        assert_eq!(r.total_requests, 0);
        assert_eq!(r.p50_ms, 0);
        assert_eq!(r.p99_ms, 0);
        assert_eq!(r.failed, 0);
    }

    #[test]
    fn counts_by_status_class() {
        let mut samples = vec![
            s(200, 5),
            s(201, 6),
            s(301, 7),
            s(404, 8),
            s(500, 9),
            s(503, 10),
        ];
        let r = summarize(&mut samples, 2);
        assert_eq!(r.status_counts.class_2xx, 2);
        assert_eq!(r.status_counts.class_3xx, 1);
        assert_eq!(r.status_counts.class_4xx, 1);
        assert_eq!(r.status_counts.class_5xx, 2);
        assert_eq!(r.status_counts.network_error, 2);
        // failed = 2 network + 2 5xx
        assert_eq!(r.failed, 4);
    }

    #[test]
    fn percentiles_are_sorted() {
        let mut samples: Vec<_> = (1..=100).map(|n| s(200, n)).collect();
        let r = summarize(&mut samples, 0);
        // For 100 samples 1..=100ms, p50 = 50th (sorted index 49), p99 = 99th.
        assert!((48..=51).contains(&r.p50_ms), "p50 was {}", r.p50_ms);
        assert!((94..=96).contains(&r.p95_ms), "p95 was {}", r.p95_ms);
        assert!((98..=100).contains(&r.p99_ms), "p99 was {}", r.p99_ms);
        assert_eq!(r.max_ms, 100);
    }
}
