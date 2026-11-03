FROM rust:1.96-slim-bookworm as builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/lustpress
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
RUN useradd -u 10001 -m -s /bin/bash lustpress

WORKDIR /app
COPY --from=builder /usr/src/lustpress/target/release/lustpress /app/lustpress

ENV PORT=3000
ENV MALLOC_ARENA_MAX=2
EXPOSE 3000

USER lustpress
CMD ["/app/lustpress"]