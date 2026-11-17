# syntax=docker/dockerfile:1

# ==========================================
# Stage 1: System Base (Runtime Environment)
# ==========================================
FROM debian:bookworm-slim AS system-base

WORKDIR /app

# Install runtime utilities (essential for agentic coding & headless browser)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    bash \
    git \
    gh \
    python3 \
    python3-pip \
    python3-requests \
    sqlite3 \
    poppler-utils \
    chromium \
    fonts-liberation \
    fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*

# Pre-install OLAP analytics engines for Parquet and tabular data
RUN pip install --no-cache-dir --break-system-packages duckdb pyarrow

# Install official Google Antigravity CLI (agy)
RUN curl -fsSL https://antigravity.google/cli/install.sh | bash && \
    (cp /root/.local/bin/agy /usr/local/bin/agy || true)

# Create application directories & pip defaults
RUN mkdir -p /app/config /app/data /app/workspaces /app/scripts /app/assets /root/.gemini/antigravity-cli && \
    mkdir -p /etc/pip && printf "[global]\nbreak-system-packages = true\nuser = true\n" > /etc/pip.conf

# ==========================================
# Stage 2: Build Rust binary
# ==========================================
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/aina

# Cache dependencies layer
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    mkdir src && echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > build.rs && \
    cargo build --release && \
    rm -rf src build.rs target/release/deps/aina* target/release/aina* target/release/build/aina*

# Copy compilation inputs (source code, build script, and compile-time baseline configuration)
COPY build.rs ./
COPY src/ ./src/
COPY config/activities.default.json ./config/activities.default.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

# ==========================================
# Stage 3: Final Production Runtime
# ==========================================
FROM system-base AS runtime

WORKDIR /app

# Copy compiled Aina binary from builder
COPY --from=builder /usr/src/aina/target/release/aina /usr/local/bin/aina

# Copy application configuration, workspaces, skills, scripts, and assets
COPY config/ /app/config/
COPY workspaces/ /app/workspaces/
COPY skills/ /app/skills/
COPY scripts/ /app/scripts/
COPY assets/ /app/assets/
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh /app/scripts/*.py 2>/dev/null || true

# Default environment configuration
ENV SERVER_HOST=0.0.0.0 \
    SERVER_PORT=8090 \
    TZ=Asia/Jakarta \
    AINA_TIMEZONE=Asia/Jakarta \
    AINA_LOCALE=id-ID \
    AINA_TIMEZONE_OFFSET=7 \
    AGENT_BINARY_PATH=/usr/local/bin/agy \
    AGENT_WORKSPACE=/app/workspaces/default \
    DATABASE_PATH=/app/data/aina.db \
    AGENT_PERSONA_FILE=/app/config/persona.md \
    AGENT_ORGANIZATION_FILE=/app/config/organization.md \
    PYTHONUSERBASE=/app/data/python-packages \
    PIP_BREAK_SYSTEM_PACKAGES=1 \
    PATH="/app/data/python-packages/bin:/app/scripts:/root/.local/bin:/usr/local/bin:${PATH}"

EXPOSE 8090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://127.0.0.1:${SERVER_PORT:-8090}/health || exit 1

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["aina"]
