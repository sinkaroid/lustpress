FROM oven/bun:1.3.14-alpine AS base
WORKDIR /srv/app

# 1. Install production dependencies only
FROM base AS deps
COPY package.json bun.lock ./
RUN bun install --production

# 2. Builder stage to compile TypeScript
FROM base AS builder
COPY package.json bun.lock ./
RUN bun install
COPY . .
RUN bun run build

# 3. Final release stage
FROM base AS release
COPY --from=deps /srv/app/node_modules ./node_modules
COPY --from=builder /srv/app/build ./build
COPY package.json ./

# Run as non-root user
USER bun
EXPOSE 3000

CMD ["bun", "run", "start"]