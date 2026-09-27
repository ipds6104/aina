# User Custom Skills Directory

This directory stores dynamic, user-generated custom skills created by Aina at runtime upon user request (e.g. ad-hoc third-party integrations, custom workflows).

## Architecture Highlights
- **Persistent Volume**: In Docker/Coolify deployments, this directory is mounted under the persistent volume `aina_data:/app/data/custom-skills`. It is preserved across image rebuilds and redeployments.
- **AGY Discovery**: Discovered by Antigravity CLI via `.agents/skills.json` (`/app/data/custom-skills` or `../../data/custom-skills`).
- **Optional Git Backup**: Can be backed up to a dedicated private GitHub repository (e.g. `aina-custom-skills`) via `gh` CLI for seamless plug-and-play synchronization across machines and environments.
