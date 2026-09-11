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

# Ensure workspace, data directories, and skills exist
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills"
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/knowledge"
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/scripts"
mkdir -p "$(dirname "${DATABASE_PATH:-/app/data/aina.db}")"
mkdir -p /root/.gemini/config/skills

# Mount/Sync skills from image to global config and workspace
if [ -d "/app/skills" ]; then
    cp -r /app/skills/* /root/.gemini/config/skills/ 2>/dev/null || true
    cp -r /app/skills/* "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills/" 2>/dev/null || true
    chmod -R +x /root/.gemini/config/skills/*/scripts 2>/dev/null || true
    chmod -R +x "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills"/*/scripts 2>/dev/null || true
fi

# Sync runtime sandbox GEMINI.md rules into workspace if provided
if [ -f "/app/config/sandbox.GEMINI.md" ]; then
    cp /app/config/sandbox.GEMINI.md "${AGENT_WORKSPACE:-/app/workspaces/default}/GEMINI.md" 2>/dev/null || true
fi

# Make sure agy is in PATH
export PATH="/root/.local/bin:/usr/local/bin:$PATH"

exec "$@"
