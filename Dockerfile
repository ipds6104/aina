# syntax=docker/dockerfile:1
# ==========================================
# Stage 1: Build binary
# ==========================================
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/aina

# Cache dependencies layer
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/aina* target/release/aina*

# Copy actual source code and compile
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

# ==========================================
# Stage 2: Runtime image
# ==========================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime utilities (essential for agentic coding)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    bash \
    git \
    python3 \
    python3-pip \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Install official Google Antigravity CLI (agy)
RUN curl -fsSL https://antigravity.google/cli/install.sh | bash && \
    (cp /root/.local/bin/agy /usr/local/bin/agy || true)

# Copy compiled Aina binary
COPY --from=builder /usr/src/aina/target/release/aina /usr/local/bin/aina

# Create application directories
RUN mkdir -p /app/config /app/data /app/workspace /root/.gemini/antigravity-cli

# Copy configuration and entrypoint
COPY config/persona.md /app/config/persona.md
COPY config/config.yaml /app/config/config.yaml
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Default environment configuration
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8090
ENV AGENT_BINARY_PATH=/usr/local/bin/agy
ENV AGENT_WORKSPACE=/app/workspace
ENV DATABASE_PATH=/app/data/aina.db
ENV AGENT_PERSONA_FILE=/app/config/persona.md
ENV PATH="/root/.local/bin:/usr/local/bin:${PATH}"

EXPOSE 8090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://127.0.0.1:${SERVER_PORT:-8090}/health || exit 1

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["aina"]
