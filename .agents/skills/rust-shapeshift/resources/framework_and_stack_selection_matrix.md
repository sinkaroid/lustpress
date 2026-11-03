# Rust Framework & Stack Selection Matrix

## The Problem: The "One-Size-Fits-All" Architectural Trap

A frequent failure mode in Rust backend engineering is choosing a web framework based on hype or generic benchmark leaderboards rather than physical workload constraints. 

Reaching for Tokio and Axum for an ephemeral monitoring sidecar wastes 20MB–40MB of memory on unused thread pools. Conversely, hand-rolling HTTP/1.1 parsers with `std::net` for a complex SaaS application wastes months reinventing routing, OpenAPI schemas, and multipart streams.

This reference guide provides an objective, production-tested **Selection Matrix** comparing **Zero-Dependency Manual (`std::net`)**, **Axum**, **Actix Web**, **Salvo**, **Poem**, **Rocket**, and **Loco**.

---

## 1. Comprehensive Framework Comparison Matrix

| Stack / Framework | Idle RSS (Baseline) | Ecosystem & Runtime | Primary Architectural Strength | Weakness / Trade-off | Best Production Use Case |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Manual (Zero-Deps)** | **<1 MB** (~50KB) | Pure `std::net`, blocking I/O, static `musl` | Indestructible stability, 0 dependencies, instant builds (<2s). | No built-in routing, HTTP/2, or middleware. | Container sidecars, low-QPS daemons, CLI tools, embedded monitors. |
| **Axum** (v0.8+) | **15 MB – 35 MB** | Tokio multi-thread, Tower, Hyper | Official Tokio ecosystem standard, modular Tower middleware, type-safe extractors. | Requires Tokio overhead; Tower compiler errors can be complex. | Cloud microservices, REST/GraphQL gateways, WebSocket clusters. |
| **Actix Web** (v4+) | **20 MB – 45 MB** | Custom `actix-rt` multi-arbiter over Tokio | Top-tier raw throughput (TechEmpower), mature built-in HTTP primitives. | Heavy macro and unsafe usage; clunky Tower compatibility. | High-throughput public APIs, high-RPS batch ingestion engines. |
| **Salvo** | **20 MB – 40 MB** | Tokio, Hyper | Built-in OpenAPI (`oapi`), unified handlers, ACME auto-TLS, Go/Gin ergonomics. | Smaller English community; frequent API breaking changes across minors. | Fast REST API delivery with automatic OpenAPI documentation. |
| **Poem** | **18 MB – 38 MB** | Tokio, Hyper (also targets WASM) | First-class OpenAPI (`poem-openapi`) & GraphQL, elegant functional design. | Single primary maintainer; smaller third-party plugin ecosystem. | Contract-first OpenAPI services, type-driven GraphQL microservices. |
| **Rocket** (v0.5+) | **25 MB – 50 MB** | Tokio (migrated), custom state | Unmatched developer experience (DX), intuitive request guards, clean syntax. | Heavy procedural macro overhead (slower builds); higher memory footprint. | Internal tools, admin dashboards, rapid prototypes where DX > raw throughput. |
| **Loco** (Rails of Rust) | **40 MB – 75 MB** | Tokio, Axum, SeaORM | True batteries-included full-stack: DB migrations, background workers, auth, mailers. | Large dependency tree, long compile times, bloated for minimal microservices. | Full-stack web applications, SaaS MVPs, teams migrating from Rails/Django/Laravel. |

---

## 2. In-Depth Framework Breakdown: Pros & Cons

### 1. Manual Zero-Dependency (The Koburi Pattern)
```rust
use std::net::TcpListener;
use std::io::{Read, Write};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9911")?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf)?;
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")?;
    }
    Ok(())
}
```
* **Pros**: Sub-megabyte memory (53 kB verified idle RSS under continuous scraping), zero external crate vulnerabilities, compile times under 3 seconds, trivial static linking (`musl`).
* **Cons**: You must manually implement HTTP parsing, path routing, keepalive headers, and TLS.
* **When to Choose**: When the service runs on tight edge hardware, container sidecars (e.g. Istio/Grafana scraping sidecars), or polling jobs where an async executor is pure waste.

---

### 2. Axum (The Modern Cloud Standard)
```rust
use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(|| async { "OK" }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```
* **Pros**: Developed by the Tokio team. Seamlessly interoperates with `tower`, `tower-http` (CORS, Trace, Compression), and `hyper`. Extractor model (`Path`, `Query`, `Json`, `State`) provides compile-time guarantee of request data. Minimal procedural macros mean fast compile times.
* **Cons**: No out-of-the-box CLI, ORM, or auth generators; you must assemble your own stack.
* **When to Choose**: Production cloud microservices, high-density WebSockets, Kubernetes deployments, and teams building long-term enterprise systems.

---

### 3. Actix Web (The High-Throughput Heavyweight)
* **Pros**: Consistently dominates raw requests-per-second benchmarks. Multi-arbiter architecture assigns dedicated single-threaded workers per OS thread, maximizing thread cache locality.
* **Cons**: Historical API instability, heavier binary footprint, custom actor runtime layer that complicates integration with generic Tokio libraries.
* **When to Choose**: When your business metric is maximum raw HTTP throughput per server node and you don't need Tower middleware interoperability.

---

### 4. Salvo (The All-In-One API Framework)
```rust
use salvo::prelude::*;

#[handler]
async fn hello() -> &'static str {
    "Hello World"
}

#[tokio::main]
async fn main() {
    let router = Router::new().get(hello);
    let acceptor = TcpListener::new("0.0.0.0:5800").bind().await;
    Server::new(acceptor).serve(router).await;
}
```
* **Pros**: Ergonomic handler design. Native built-in OpenAPI documentation generator (`salvo-oapi`), automated Let's Encrypt / ACME TLS certification, and built-in rate limiting. Feels like Go's Gin or Fiber.
* **Cons**: Heavy macro usage increases compiler load. Smaller community compared to Axum.
* **When to Choose**: Teams wanting built-in Swagger UI and OpenAPI generation without hand-crafting schema annotations across dozens of crates.

---

### 5. Poem (The OpenAPI & GraphQL Specialist)
* **Pros**: Exceptionally clean functional design. `poem-openapi` is arguably the cleanest OpenAPI 3.0 implementation in Rust with complete compile-time schema validation. First-class integration with `async-graphql`.
* **Cons**: Smaller ecosystem, heavily maintained by one developer.
* **When to Choose**: When your application is strictly contract-first OpenAPI or GraphQL and schema validation accuracy is your top priority.

---

### 6. Rocket (The DX & Prototype Champion)
* **Pros**: Beautiful, readable code. Request guards validate cookies, headers, and permissions effortlessly. Friendly compiler error messages and intuitive configuration management.
* **Cons**: Heavy reliance on procedural macros slows build times significantly. Higher memory baseline and less flexible for low-level socket manipulations.
* **When to Choose**: Fast prototypes, internal company dashboards, hackathons, or developers transitioning from Flask/Express who want maximum ergonomics.

---

### 7. Loco (The Full-Stack Rails of Rust)
```bash
# Loco CLI generates the entire project structure
loco new
cargo loco start
```
* **Pros**: Convention-over-configuration. Comes bundled with SeaORM database migrations, JWT authentication, background task queues, mailer templates, and scaffolding generators.
* **Cons**: Highest idle memory footprint (40MB–75MB), longest compilation times, large dependency graph. Violates minimalist microservice principles.
* **When to Choose**: Solo developers or small startups building a full-stack SaaS web application where rapid feature delivery and database scaffolding outweigh raw memory minimization.

---

## 3. Decision Tree: How to Guide the User

When a user asks: *"Framework apa yang harus gue pakai untuk project Rust ini?"*, follow this decision tree:

```text
Are you building a full-stack monolithic SaaS with DB, Auth, and background queues?
  ├── YES ──► LOCO (Fastest time-to-market, Rails-like conventions)
  └── NO
       │
Is this a simple background daemon, container monitor, CLI, or scraping sidecar?
  ├── YES ──► MANUAL ZERO-DEPS / KOBURI PATTERN (Pure std::net, <1MB RSS, instant builds)
  └── NO
       │
Do you require automatic OpenAPI / Swagger UI generated directly from code?
  ├── YES ──► SALVO (All-in-one) or POEM-OPENAPI (Contract-first)
  └── NO
       │
Is raw maximum RPS throughput the single defining KPI?
  ├── YES ──► ACTIX WEB (TechEmpower champion, multi-arbiter)
  └── NO  ──► AXUM 0.8 (Default recommendation: Rock-solid Tokio standard, Tower ecosystem)
```
