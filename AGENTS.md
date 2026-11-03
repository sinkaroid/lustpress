# Lustpress — Agent Operating Guide

This document defines the engineering standards, architecture, and toolings for **Lustpress**. All AI agents operating on this repository must read and follow these directives.

---

## 1. Project Overview & Architecture

- **Project**: `lustpress` — High-efficiency, memory-bounded Rust REST API (Axum 0.8) porting legacy video scraper backends.
- **Language / Edition**: Rust 1.80+ (2024 Edition ready).
- **Runtime & Allocator**: Tokio multi-thread runtime + `mimalloc` global allocator.
- **Physical Memory Target**: Sub-10 MB baseline idle RSS (< 15 MB under sustained traffic).
- **Core Dependencies**: Axum 0.8, Tower, Reqwest (rustls-tls), Scraper, DeadPool Redis, Moka, Mimalloc.

---

## 2. Core Skill: `rust-shapeshift`

This repository strictly adheres to the **`rust-shapeshift`** engineering discipline.

- **Skill Location**: [`.agents/skills/rust-shapeshift/SKILL.md`](.agents/skills/rust-shapeshift/SKILL.md)
- **Primary Focus**: Extreme memory efficiency, bounded concurrency, allocator tuning, and zero-allocation parsing.

### When working on this codebase, always apply:

1. **Zero-Panic Guarantee**: `#![deny(clippy::unwrap_used, clippy::expect_used)]` is enforced at the root. Always propagate errors using `Result<T, AppError>` or fallback defaults.
2. **Allocator Hygiene**:
   - `mimalloc` is set as `#[global_allocator]` in `src/main.rs`.
   - `ENV MALLOC_ARENA_MAX=2` is enforced in the `Dockerfile`.
   - Call `crate::util::trim_memory()` in middleware / loops to ensure pages return to the OS kernel.
3. **Zero-Copy & Pre-Parse Pruning**:
   - Truncate or window-scan byte slices (`bytes.windows(...)`) instead of allocating multi-megabyte `String` heap buffers.
   - Drop `scraper::Html` AST trees immediately after data extraction.
4. **Bounded Buffers & Caches**:
   - Cache data is stored in **remote Redis** when `REDIS_URL` is set.
   - When running in-memory fallback, cache is strictly bounded by byte-weigher (`CACHE_MAX_BYTES`).

---

## 3. Dedicated MCP Server: `rust-shapeshift`

Agents with MCP capability must leverage the **`rust-shapeshift`** server (`rust-shapeshift mcp` or `npx @sinkaroid/rust-shapeshift mcp`).

| MCP Tool                    | Purpose & When to Call                                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `audit_memory_antipatterns` | **Call before finishing any task.** Audits code for glibc traps, unbounded channels, release profiles, and sync locks across `.await`.            |
| `inspect_struct_layout`     | Analyzes struct field ordering, alignment, padding bytes, and cache-line false sharing hazards.                                                   |
| `calculate_hardware_budget` | Calculates worst-case RSS ceilings and safe per-connection buffers to prevent Linux OOMKilled (Exit 137).                                         |
| `explain_recipe`            | Queries targeted production snippets (e.g., query `jemalloc`, `allocator`, `ring buffer`, `cancellation safety`) without polluting agent context. |
| `scaffold_archetype`        | Generates verified boilerplates for low-memory daemons, Axum routes, or zero-copy pipelines.                                                      |

---

## 4. Standard Agent Verification Workflow

Before reporting completion or committing changes, run this standard verification pipeline:

```bash
# 1. Check code formatting
cargo fmt --check

# 2. Run strict clippy with zero warnings
cargo clippy --all-targets -- -D warnings

# 3. Execute all unit and integration tests
cargo test

# 4. (If MCP available) Audit memory antipatterns
# Call tool: rust-shapeshift -> audit_memory_antipatterns
```

---

## 5. Directory Structure Quick Reference

```text
lustpress/
├── .agents/
│   └── skills/
│       └── rust-shapeshift/   # Memory & systems architecture skill & reference guides
├── src/
│   ├── main.rs                # Entrypoint, mimalloc global allocator, background maintenance
│   ├── config.rs              # Env parsing and defaults
│   ├── state.rs               # AppState, cache and client initialization
│   ├── fetch.rs               # Upstream HTTP fetch with bypass & zero-alloc challenge scans
│   ├── metrics.rs             # Prometheus metrics & middleware with RAII InFlightGuard
│   ├── util.rs                # trim_memory(), query encoding, ID validation (is_valid_id)
│   ├── cache/                 # Redis Pool & byte-weighted Moka fallback
│   ├── error/                 # AppError and IntoResponse implementation
│   └── providers/             # Scraper modules (pornhub, redtube, xvideos, etc.)
├── Dockerfile                 # Multi-stage build, MALLOC_ARENA_MAX=2, non-root user
└── AGENTS.md                  # This file
```
