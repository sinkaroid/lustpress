FROM rust:1.96-slim-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/lustpress
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/lustpress/target/release/lustpress /app/lustpress

ENV PORT=3000
ENV MALLOC_ARENA_MAX=2
EXPOSE 3000

CMD ["/app/lustpress"]