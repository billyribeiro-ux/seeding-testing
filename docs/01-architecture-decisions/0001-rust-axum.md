# ADR 0001 — Rust + Axum for the API

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team
- Tags: language, http, foundation

## Context and Problem Statement

MemberClub's API serves authenticated browser sessions and API clients.
It must be correct under concurrency, comfortable to operate, and
extensible to background workers and CLIs. The choice of language and
HTTP framework shapes hiring, latency, and reliability for years.

## Decision Drivers

- Memory safety without a garbage collector.
- Predictable latency tail under load.
- First-class async I/O for the database, Stripe, and mail providers.
- Mature ecosystem (sqlx, argon2, jsonwebtoken, async-stripe).
- The same language for the API binary, CLIs, and background workers
  so the team learns one stack deeply.

## Considered Options

1. **Rust + Axum** — modern, ergonomic, tower-based middleware.
2. Rust + Actix Web — older, fast, less ergonomic.
3. Go + chi/echo — simpler, GC pauses, less type expressivity.
4. TypeScript + Fastify — fastest hiring path, runtime errors common.

## Decision Outcome

Chose **Rust + Axum**.

- Axum is Tower-based; layers (CORS, trace, timeout) compose cleanly.
- Tokio is the dominant Rust async runtime; sqlx + reqwest + async-stripe
  all integrate without glue.
- The dual-mode auth pattern (Phase 6) uses Axum's `FromRequestParts`
  extractor in a uniform way.

## Consequences

- **Positive:** memory-safe, ~10× faster than the TS option for the
  same RPS budget; predictable p99 (no GC pauses); one language for
  every backend artifact.
- **Negative:** slower onboarding for engineers without Rust background
  (multi-week ramp); slower compile times than Go.
- **Mitigations:** Phase 1 of the curriculum teaches the language from
  zero; `sccache` + `mold` keep CI under 10 min.

## Notes

Re-evaluate every 24 months. The day a credible "Rust without the borrow
checker" appears (Mojo? Carbon?), revisit.
