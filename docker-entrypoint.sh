#!/usr/bin/env bash
set -e

# Setup Antigravity config directory
mkdir -p /root/.gemini/antigravity-cli /root/.gemini/config

# If OAuth token is provided via environment variable (e.g. from Coolify secrets), write it directly
if [ -n "$AINA_OAUTH_TOKEN" ]; then
    echo "Found AINA_OAUTH_TOKEN in environment, writing to credentials store..."
    echo -n "$AINA_OAUTH_TOKEN" > /root/.gemini/antigravity-cli/antigravity-oauth-token
    chmod 600 /root/.gemini/antigravity-cli/antigravity-oauth-token
elif [ -n "$ANTIGRAVITY_OAUTH_TOKEN" ]; then
    echo "Found ANTIGRAVITY_OAUTH_TOKEN in environment, writing to credentials store..."
    echo -n "$ANTIGRAVITY_OAUTH_TOKEN" > /root/.gemini/antigravity-cli/antigravity-oauth-token
    chmod 600 /root/.gemini/antigravity-cli/antigravity-oauth-token
fi

# Ensure workspace and data directories exist
mkdir -p "${AGENT_WORKSPACE:-/app/workspace}"
mkdir -p "$(dirname "${DATABASE_PATH:-/app/data/aina.db}")"

# Make sure agy is in PATH
export PATH="/root/.local/bin:/usr/local/bin:$PATH"

exec "$@"
