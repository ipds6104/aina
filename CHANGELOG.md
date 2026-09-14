# Changelog

All notable changes to the Aina project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0] - 2026-09-14

### Added
- **Dual-Gateway WhatsApp Architecture**:
  - Support for 2 concurrent Whatsmeow gateways: Primary Bot (`WHATSMEOW_BASE_URL`) and User Companion Sensor (`WHATSMEOW_COMPANION_BASE_URL`).
  - Added `--companion` flag to `skills/whatsmeow/scripts/wa_tool.py` to route on-demand queries (groups, recent chats, search) to Bang Ihza's personal WhatsApp account.
  - Automatic dynamic routing in `WhatsmeowHttpAdapter` and media claim-check downloading in Axum webhook router.
- **Multi-Bubble Message Splitting (`<<<SPLIT_CHAT>>>`)**:
  - Ability for Aina to partition complex answers into multiple sequential WhatsApp message bubbles with natural typing pauses.
  - Clean forwardable draft pattern: user-facing explanation in bubble 1, pure unadorned forwardable message in bubble 2 without awkward bot prefixes.
- **Claim-Check Media Architecture**:
  - Support streaming media downloads for files > 512KB via `GET /api/v1/media/{id}/download`.
  - Automatic `.txt` companion sidecar file extraction when receiving media documents (e.g. PDF/Excel context).
- **Self-Version & Upstream Introspection (`aina version --check`)**:
  - Built-in `build.rs` compile-time metadata injection (`AINA_GIT_COMMIT`, `AINA_GIT_BRANCH`, `AINA_BUILD_TIME`).
  - Native CLI commands `aina version` and `aina version --check` to compare running container build against upstream GitHub (`https://api.github.com/repos/ipds6104/aina/compare/...`).
  - Machine-readable Capability Manifest exposing active capabilities to eliminate agent self-capability hallucination.

### Fixed
- Fixed issue where media uploads above Axum default limits resulted in 413 Payload Too Large by setting 100MB body limit with graceful fallback.
- Fixed single-gateway assumption that prevented Aina from accessing groups joined exclusively by the companion user account.

---

## [0.1.0] - 2026-09-11

### Added
- **Core Hexagonal Architecture (Ports & Adapters)**:
  - Clean domain separation between `core/domain`, `core/ports`, `core/usecases`, `adapters/driving`, and `adapters/driven`.
  - Tokio async runtime with Axum webhook server and CLI dispatcher.
- **Whatsmeow WhatsApp Integration**:
  - Webhook listener for incoming messages, quotes, mentions, and media attachments.
  - Gatekeeper engine enforcing OpSec privacy rules: ambient recording of group chats to SQLite FTS5 while strictly ignoring un-triggered personal DMs.
- **Knowledge Base & Temporal Archive Engine**:
  - Local SQLite FTS5 BM25 search with temporal filters (`--since`, `--from`, `--to`).
  - Deterministic knowledge base catalog generator (`knowledge/index.md`) and linter with auto-heal.
- **Antigravity CLI Agentic Execution Engine**:
  - Integration with `agy` CLI for autonomous tool execution, subagent delegation, and task scheduling.
  - Dynamic model selection (Gemini 3.8 Flash default, Gemini 3.1 Pro High, Claude Opus 4.6 Thinking).
- **First-Time Web Setup Wizard & Administrative Claiming**:
  - Interactive web dashboard at `/` and `/setup` secured via single-use or admin PIN.
