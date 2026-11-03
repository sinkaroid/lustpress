FROM redis:8-alpine AS redis

LABEL org.opencontainers.image.title="lustpress-redis"
LABEL org.opencontainers.image.description="Dedicated Redis cache backend for lustpress"
LABEL org.opencontainers.image.source="https://github.com/sinkaroid/lustpress"
LABEL org.opencontainers.image.licenses="MIT"

# Expose standard Redis port
EXPOSE 6379

# Start Redis with a default password (lustpress)
CMD ["redis-server", "--requirepass", "lustpress"]
