#!/usr/bin/env bash
set -e

# Setup Antigravity config directory
mkdir -p /root/.gemini/antigravity-cli /root/.gemini/config

# Ensure persistent brain storage across Coolify redeployments
mkdir -p /app/data/brain
if [ -d "/root/.gemini/antigravity-cli/brain" ] && [ ! -L "/root/.gemini/antigravity-cli/brain" ]; then
    cp -rn /root/.gemini/antigravity-cli/brain/* /app/data/brain/ 2>/dev/null || true
    rm -rf /root/.gemini/antigravity-cli/brain
fi
ln -sfn /app/data/brain /root/.gemini/antigravity-cli/brain

# If OAuth token is provided via environment variable (e.g. from Coolify secrets), write it directly
if [ -n "$AINA_OAUTH_TOKEN" ]; then
    echo "Found AINA_OAUTH_TOKEN in environment, writing to credentials store..."
    echo -n "$AINA_OAUTH_TOKEN" > /root/.gemini/antigravity-cli/antigravity-oauth-token
    chmod 600 /root/.gemini/antigravity-cli/antigravity-oauth-token
elif [ -n "$ANTIGRAVITY_OAUTH_TOKEN" ]; then
    echo "Found ANTIGRAVITY_OAUTH_TOKEN in environment, writing to credentials store..."
    echo -n "$ANTIGRAVITY_OAUTH_TOKEN" > /root/.gemini/antigravity-cli/antigravity-oauth-token
    chmod 600 /root/.gemini/antigravity-cli/antigravity-oauth-token
elif [ -n "$AINA_OAUTH_TOKENS" ]; then
    echo "Found AINA_OAUTH_TOKENS in environment, extracting first token to credentials store..."
    FIRST_TOKEN=$(echo "$AINA_OAUTH_TOKENS" | grep -o '{"[^}]*}' | head -n 1)
    if [ -z "$FIRST_TOKEN" ]; then
        FIRST_TOKEN=$(echo "$AINA_OAUTH_TOKENS" | grep -o '"[^"]*"' | head -n 1 | tr -d '"')
    fi
    if [ -z "$FIRST_TOKEN" ]; then
        FIRST_TOKEN=$(echo "$AINA_OAUTH_TOKENS" | cut -d',' -f1 | tr -d ' \n\r')
    fi
    if [ -n "$FIRST_TOKEN" ]; then
        echo -n "$FIRST_TOKEN" > /root/.gemini/antigravity-cli/antigravity-oauth-token
        chmod 600 /root/.gemini/antigravity-cli/antigravity-oauth-token
    fi
elif [ -n "$AINA_OAUTH_TOKEN_1" ]; then
    echo "Found AINA_OAUTH_TOKEN_1 in environment, writing to credentials store..."
    echo -n "$AINA_OAUTH_TOKEN_1" > /root/.gemini/antigravity-cli/antigravity-oauth-token
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
    mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/skills"
    cp -r /app/skills/* "${AGENT_WORKSPACE:-/app/workspaces/default}/skills/" 2>/dev/null || true
    chmod -R +x /root/.gemini/config/skills/*/scripts 2>/dev/null || true
    chmod -R +x "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills"/*/scripts 2>/dev/null || true
    chmod -R +x "${AGENT_WORKSPACE:-/app/workspaces/default}/skills"/*/scripts 2>/dev/null || true
    ln -sf /app/skills/whatsmeow/scripts/wa_tool.py /usr/local/bin/wa_tool 2>/dev/null || true
    ln -sf /app/skills/gdrive/scripts/gdrive_tool.py /usr/local/bin/gdrive_tool 2>/dev/null || true
fi

# Optional Google OAuth client secrets or token from environment variable
if [ -n "$GOOGLE_CLIENT_SECRETS_JSON" ] && [ ! -f /app/config/client_secrets.json ]; then
    echo "Found GOOGLE_CLIENT_SECRETS_JSON in environment, writing to /app/config/client_secrets.json..."
    echo -n "$GOOGLE_CLIENT_SECRETS_JSON" > /app/config/client_secrets.json
    chmod 600 /app/config/client_secrets.json
fi
if [ -n "$GOOGLE_TOKEN_JSON" ] && [ ! -f /app/data/google_token.json ]; then
    echo "Found GOOGLE_TOKEN_JSON in environment, writing to /app/data/google_token.json..."
    echo -n "$GOOGLE_TOKEN_JSON" > /app/data/google_token.json
    chmod 600 /app/data/google_token.json
fi

# Sync runtime sandbox GEMINI.md rules into workspace if provided
if [ -f "/app/config/sandbox.GEMINI.md" ]; then
    cp /app/config/sandbox.GEMINI.md "${AGENT_WORKSPACE:-/app/workspaces/default}/GEMINI.md" 2>/dev/null || true
fi

# Ensure google-chrome alias is available for agy headless browser
if [ ! -x "/usr/bin/google-chrome" ] && [ -x "/usr/bin/chromium" ]; then
    ln -s /usr/bin/chromium /usr/bin/google-chrome 2>/dev/null || true
fi

# Make sure agy is in PATH
export PATH="/root/.local/bin:/usr/local/bin:$PATH"

exec "$@"
