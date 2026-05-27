//! OpenTelemetry / OTLP tracing wiring for notes-api.
//!
//! This module is the *only* place that knows how to build a tracer
//! provider and bridge `tracing` spans into OTel. Everything else in
//! the crate keeps using the `tracing` macros as before — adding OTel
//! is a single `telemetry::init(...)` call in `main`.
//!
//! ## Modes
//!
//! - [`init(None)`] — keep the existing behavior: install a
//!   `tracing-subscriber::FmtSubscriber` honoring `RUST_LOG` and bail
//!   out before any OTel pipeline is created. This is what tests and
//!   local `cargo run` invocations use.
//! - [`init(Some(endpoint))`] — additionally install an OTLP/gRPC span
//!   exporter pointed at `endpoint` (e.g. `http://localhost:4317`),
//!   batched on the Tokio runtime, and a `tracing-opentelemetry` layer
//!   that bridges every `tracing::span!` and `#[instrument]` macro into
//!   OTel spans tagged with the service-resource attributes.
//!
//! ## Service-resource attributes
//!
//! Every exported span carries:
//!   * `service.name = "notes-api"`
//!   * `service.version = env!("CARGO_PKG_VERSION")`
//!   * `deployment.environment = $DEPLOYMENT_ENVIRONMENT` (when set)
//!
//! These map straight to Tempo/Jaeger's service selector — the
//! `service.name` is what shows up in Grafana's "Service" dropdown.
//!
//! ## Init-once contract
//!
//! The global tracer-provider, the global subscriber, and the global
//! propagator are all process-wide singletons. Calling [`init`] twice
//! in the same process is an error, not a panic — the second call
//! returns `Err` instead of poisoning the global subscriber. Tests
//! exercise both branches.
//!
//! ## Shutdown
//!
//! [`TelemetryGuard`] is RAII. When it drops, it calls
//! [`opentelemetry::global::shutdown_tracer_provider`] which flushes
//! the in-flight batch through the OTLP exporter before returning.
//! Forget the guard and you may lose the last ~5 s of spans.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, anyhow};
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Global flip-flop that records whether [`init`] has already run in
/// this process. The OTel + `tracing` globals will themselves panic on
/// re-init; flipping this bit first lets us return a clean `Err`
/// instead of taking the process down.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Default OTLP/gRPC timeout. The exporter wants a *per-batch* timeout —
/// 10 s matches the OTel-collector default and is generous for the
/// localhost case.
const OTLP_TIMEOUT: Duration = Duration::from_secs(10);

/// RAII handle for the telemetry pipeline.
///
/// When the guard drops it calls
/// [`opentelemetry::global::shutdown_tracer_provider`] to flush
/// in-flight spans. Hold the guard for the lifetime of `main`.
#[derive(Debug)]
pub struct TelemetryGuard {
    /// `true` if [`init`] actually installed an OTLP pipeline; `false`
    /// for the fmt-only fallback. Drop only calls `shutdown_tracer_provider`
    /// in the OTLP case — there's nothing to flush otherwise.
    has_otlp: bool,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if self.has_otlp {
            // Flush whatever the batch processor has buffered. This is
            // blocking; the OTel SDK enforces a 5 s inner timeout.
            opentelemetry::global::shutdown_tracer_provider();
        }
    }
}

/// Install the global subscriber.
///
/// * `otlp_endpoint == None` — fmt-only fallback (today's behavior).
/// * `otlp_endpoint == Some(url)` — additionally wire an OTLP/gRPC
///   exporter pointed at `url` (e.g. `http://localhost:4317`).
///
/// Returns a [`TelemetryGuard`] whose `Drop` flushes pending spans.
///
/// # Errors
///
/// * Re-init in the same process returns `Err` (see the init-once
///   contract in the module docs).
/// * Building the OTLP exporter or installing the subscriber returns
///   `Err` with the underlying SDK error.
pub fn init(otlp_endpoint: Option<&str>) -> anyhow::Result<TelemetryGuard> {
    // The OTel + tracing globals will panic if installed twice. Flip
    // the bit first so the second caller gets a clean error.
    if INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(anyhow!(
            "telemetry::init has already been called in this process"
        ));
    }

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,notes_api=debug,tower_http=info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    let Some(endpoint) = otlp_endpoint else {
        // Fmt-only fallback. Matches the pre-OTel subscriber line-for-line.
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .map_err(|e| anyhow!("install fmt subscriber: {e}"))?;
        return Ok(TelemetryGuard { has_otlp: false });
    };

    // ---- OTLP/gRPC exporter ----
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .with_timeout(OTLP_TIMEOUT)
        .build()
        .context("build OTLP span exporter")?;

    // ---- Resource attributes ----
    let mut resource_kvs = vec![
        KeyValue::new("service.name", "notes-api"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ];
    if let Ok(env_name) = std::env::var("DEPLOYMENT_ENVIRONMENT") {
        if !env_name.is_empty() {
            resource_kvs.push(KeyValue::new("deployment.environment", env_name));
        }
    }
    let resource = Resource::new(resource_kvs);

    // ---- TracerProvider with batch export ----
    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(resource)
        .build();
    let tracer = provider.tracer("notes-api");
    opentelemetry::global::set_tracer_provider(provider);

    // ---- tracing -> OTel bridge ----
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| anyhow!("install OTel subscriber: {e}"))?;

    tracing::info!(otlp.endpoint = %endpoint, "OTLP tracing enabled");
    Ok(TelemetryGuard { has_otlp: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset the init-once flag. **Only safe in tests where you can prove
    /// no other code is touching the global subscriber.** The internal
    /// tests below all run in-process, sequentially via `#[test]`'s
    /// default single-binary-per-test semantics — Rust's test runner gives
    /// each `#[test]` its own thread but they share globals, so we use
    /// `serial_test`-style flag manipulation only inside the same module.
    fn reset_for_test() {
        INITIALIZED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn second_init_returns_error_not_panic() {
        reset_for_test();
        // First call wins the race for the global flag.
        let _g = init(None).expect("first init must succeed in a fresh process");
        // Second call must produce a clean Err.
        let err = init(None).expect_err("second init must return Err");
        assert!(
            err.to_string().contains("already been called"),
            "unexpected error: {err}"
        );
    }
}
