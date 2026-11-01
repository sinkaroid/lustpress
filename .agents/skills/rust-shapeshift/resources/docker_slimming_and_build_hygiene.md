# Docker Slimming & Build Context Hygiene: Achieving Sub-80MB Images

Achieving production efficiency requires minimizing **both runtime memory consumption (RSS) and physical Docker image size**. A bloated Docker image wastes registry bandwidth, slows CI/CD deployments, expands vulnerability attack surfaces, and invalidates build caches needlessly.

This playbook documents the exact multi-stage container recipe and `.dockerignore` discipline used in **Jandapress** and **Lustpress** to achieve **sub-80MB Debian runtime images** and instant build context transfers.

---

## 1. Strict `.dockerignore` Discipline (Focus-Only Build Context)

Sending local build artifacts, documentation, or git history to the Docker daemon wastes gigabytes of upload bandwidth and breaks Docker layer caching whenever a non-code file changes.

### The Production `.dockerignore` Template
Place this at the root of your Rust repository:

```text
# 1. Local compiler build artifacts (Can be 2GB - 5GB!)
target
tools/**/target

# 2. VCS and CI/CD pipelines
.git
.github
.agent
.vscode

# 3. Documentation, licenses, and media clutter
*.md
**/*.md
LICENSE
resources
resources/**

# 4. Environment secrets and schemas (Never bake secrets into images!)
.env
.env.*
!.env.example

# 5. Legacy migration artifacts
legacy
node_modules
bun.lock
package.json
tsconfig.json
```

### Why This Matters:
- **Instant Context Transfer**: Transfers only source code (`src/`, `Cargo.toml`, `Cargo.lock`) to the daemon (under 2MB instead of 3GB+).
- **Cache Invalidation Protection**: Editing a `README.md`, bumping a workflow, or changing local test data **will not** invalidate your expensive `cargo build` layers.
- **Secret Leak Prevention**: Prevents developer `.env` files from ever leaking into intermediate Docker layers.

---

## 2. Hardened Multi-Stage Build Recipe (Sub-80MB Final Image)

A single-stage Dockerfile retains `cargo`, `rustc`, `gcc`, `pkg-config`, and hundreds of megabytes of intermediate `.rlib` / `.o` objects, resulting in images exceeding **1.5GB**.

The two-stage multi-stage approach extracts **only** the stripped binary and runtime shared libraries:

```dockerfile
# ========================================================
# STAGE 1: Builder (Debian Bookworm glibc 2.36)
# ========================================================
FROM rust:1.96-slim-bookworm AS builder

# Install build-only toolchain dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY . .

# Compile optimized release binary with binary stripping enabled
RUN cargo build --release

# ========================================================
# STAGE 2: Minimal Production Runtime (Sub-80MB)
# ========================================================
FROM debian:bookworm-slim

# Install only essential dynamic runtime libraries:
# 1. ca-certificates: Required for TLS/HTTPS verification in Reqwest
# 2. libssl3: Required for dynamic OpenSSL linking (matches libssl-dev from builder)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy ONLY the compiled binary from builder stage
COPY --from=builder /usr/src/app/target/release/app_binary /app/app_binary

# CRITICAL RUNTIME CONFIGURATION:
# 1. Bounded glibc arenas to eliminate multi-threaded RSS fragmentation
ENV MALLOC_ARENA_MAX=2
ENV PORT=3000

EXPOSE 3000

CMD ["/app/app_binary"]
```

---

## 3. Four Rules for Minimal Image Footprint

| Rule | Implementation | Why / Savings |
| :--- | :--- | :--- |
| **1. Strip Debug Symbols** | `[profile.release]` with `strip = true` in `Cargo.toml` | Reduces compiled binary size from ~60MB down to ~12MB by stripping unneeded symbol tables. |
| **2. Clean APT Package Lists** | `rm -rf /var/lib/apt/lists/*` on every `apt-get` layer | Saves 30MB–50MB of cached package metadata in image layers. |
| **3. Avoid Recommended Packages** | `--no-install-recommends` on `apt-get install` | Prevents Debian from pulling in hundreds of megabytes of documentation, man pages, and optional daemons. |
| **4. Strict Codename Pairing** | `rust:1.xx-slim-bookworm` ➔ `debian:bookworm-slim` | Guarantees identical glibc 2.36 dynamic ABI without missing symbols or version mismatch crashes. |
