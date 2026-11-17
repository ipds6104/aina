# syntax=docker/dockerfile:1
ARG BASE_IMAGE=aina-base:latest

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
# Stage 2: Runtime image (High-Speed Base)
# ==========================================
FROM ${BASE_IMAGE}

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

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["aina"]
