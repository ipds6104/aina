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

# Ensure workspace, data directories, custom skills, and Antigravity config exist
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills"
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/knowledge"
mkdir -p "${AGENT_WORKSPACE:-/app/workspaces/default}/scripts"
mkdir -p "$(dirname "${DATABASE_PATH:-/app/data/aina.db}")"
mkdir -p /app/data/assets /app/assets /app/data/custom-skills
mkdir -p /root/.gemini/config/skills

# Ensure persistent shared_data directory across Coolify redeployments
mkdir -p /app/data/shared_data
ln -sfn /app/data/shared_data /app/shared_data
ln -sfn /app/data/shared_data "${AGENT_WORKSPACE:-/app/workspaces/default}/shared_data"

# Ensure persistent Python environment across Coolify redeployments
export PYTHONUSERBASE="/app/data/python-packages"
mkdir -p "$PYTHONUSERBASE/bin" "$PYTHONUSERBASE/lib/python3.11/site-packages"
export PATH="$PYTHONUSERBASE/bin:$PATH"
export PIP_BREAK_SYSTEM_PACKAGES="1"
mkdir -p /etc/pip && printf "[global]\nbreak-system-packages = true\nuser = true\n" > /etc/pip.conf

# Setup default Git author identity and authentication if GH_TOKEN is present
git config --global user.name "${GIT_AUTHOR_NAME:-Aina}" 2>/dev/null || true
git config --global user.email "${GIT_AUTHOR_EMAIL:-aina@dvlpid.my.id}" 2>/dev/null || true

# If GH_TOKEN is not in environment, attempt to retrieve from Infisical vault
if [ -z "$GH_TOKEN" ] && [ -x "/usr/local/bin/infisical" ]; then
    GH_TOKEN=$(/usr/local/bin/infisical secrets get GH_TOKEN --projectId "${INFISICAL_PROJECT_ID:-f13379e0-9661-4f8e-81ef-0e81d1502da1}" --env "${INFISICAL_ENV:-dev}" --plain 2>/dev/null || true)
fi

if [ -n "$GH_TOKEN" ]; then
    export GITHUB_TOKEN="$GH_TOKEN"
    git config --global url."https://${GH_TOKEN}@github.com/".insteadOf "https://github.com/" 2>/dev/null || true
fi
gh auth setup-git 2>/dev/null || true


# Sync user custom skills from dedicated Git repository if configured
if [ -n "$USER_SKILLS_REPO" ]; then
    echo "Syncing user custom skills from $USER_SKILLS_REPO..."
    if [ ! -d "/app/data/custom-skills/.git" ]; then
        git clone "$USER_SKILLS_REPO" /app/data/custom-skills 2>/dev/null || true
    else
        (cd /app/data/custom-skills && git pull --rebase) 2>/dev/null || true
    fi
fi

# Multi-entry skills.json discovery bridge for Antigravity CLI
cat << 'EOF' > "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills.json"
{
  "entries": [
    { "path": "/app/skills" },
    { "path": "/app/data/custom-skills" },
    { "path": "skills" },
    { "path": "data/custom-skills" }
  ]
}
EOF

# Mount/Sync built-in skills from image to global config and workspace
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
    ln -sf /app/skills/vision-document-extractor/scripts/doc_extract.py /usr/local/bin/agy-doc-extract 2>/dev/null || true
    ln -sf /app/skills/vision-document-extractor/scripts/doc_extract.py /usr/local/bin/doc_extract 2>/dev/null || true
    ln -sf /app/skills/infisical/scripts/secret_tool.py /usr/local/bin/secret_tool 2>/dev/null || true
fi

# Sync user custom skills into workspace discovery directory
if [ -d "/app/data/custom-skills" ]; then
    for skill_dir in /app/data/custom-skills/*; do
        if [ -d "$skill_dir" ] && [ -f "$skill_dir/SKILL.md" ]; then
            skill_name=$(basename "$skill_dir")
            cp -r "$skill_dir" "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills/$skill_name" 2>/dev/null || true
            chmod -R +x "${AGENT_WORKSPACE:-/app/workspaces/default}/.agents/skills/$skill_name/scripts" 2>/dev/null || true
        fi
    done
fi

# Optional Infisical machine identity auto-authentication
if [ -n "$INFISICAL_CLIENT_ID" ] && [ -n "$INFISICAL_CLIENT_SECRET" ] && [ -x "/usr/local/bin/infisical" ]; then
    echo "Authenticating Infisical CLI with machine identity..."
    mkdir -p /root/.infisical
    /usr/local/bin/infisical login --domain "${INFISICAL_DOMAIN:-https://secrets.dvlpid.my.id/api}" \
        --method=universal-auth \
        --client-id="$INFISICAL_CLIENT_ID" \
        --client-secret="$INFISICAL_CLIENT_SECRET" \
        --plain > /root/.infisical/cached_token 2>/dev/null || true
    chmod 600 /root/.infisical/cached_token 2>/dev/null || true
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
