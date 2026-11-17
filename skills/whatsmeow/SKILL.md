---
name: whatsmeow
description: >-
  Use this skill whenever the user asks to send WhatsApp messages, test sending messages,
  execute batch broadcast announcements, verify delivery status, read recent chat history,
  summarize conversations, inspect group participants, export backups, or send media files via the Whatsmeow gateway.
---

# Whatsmeow Gateway & Chat Operations Skill

This skill equips Aina with procedural workflows to interact with the companion Whatsmeow WhatsApp Gateway via its REST API using the helper tool [`scripts/wa_tool.py`](./scripts/wa_tool.py).

---

## 1. Helper Script Location & CLI Contracts

The executable CLI helper tool is located at `skills/whatsmeow/scripts/wa_tool.py`.
It automatically reads `WHATSMEOW_BASE_URL` and `WHATSMEOW_API_KEY` from the environment or configuration.

Always execute it via Python 3:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]
```
*(In production container context: `python3 /app/skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]`)*

### Invariants & CLI Contracts
- **Deterministic Exit Codes**: Exits with `0` on success and `1` on failure (HTTP 4xx/5xx or network errors). Safe for `subprocess.run(check=True)` in Python or `set -e` in Bash.
- **Normalized Response Envelope**: Key response attributes (`message_id`, `status`, `recipient`) are flattened at the root JSON level.
- **Automatic JID Normalization**: Indonesian numbers (`0896-xxx`, `+62896xxx`) are automatically converted to canonical international JIDs (`62896xxx@s.whatsapp.net`).
- **Anti-Double Send Rule**: DO NOT use `send-text` to reply to the current active chat session. The final turn response text is automatically dispatched to WhatsApp by Aina backend. Use `send-text` only for proactive messages directed to external contacts or other groups.

---

## 2. Quick Operations Cheatsheet

For routine actions, use these core commands directly:

```bash
# 1. Proactive direct text message to another contact
python3 skills/whatsmeow/scripts/wa_tool.py send-text --to "0896-9403-0497" --text "Halo Kak Arya, pesan konfirmasi." --retry 3

# 2. Fetch recent messages from a chat (limit 30)
python3 skills/whatsmeow/scripts/wa_tool.py recent --jid "<CHAT_JID>" --limit 30

# 3. Verify message delivery status in gateway store
python3 skills/whatsmeow/scripts/wa_tool.py verify --id "<MESSAGE_ID>" --jid "<CHAT_JID>"

# 4. Inspect gateway uptime and daily anti-ban quota
python3 skills/whatsmeow/scripts/wa_tool.py stats
```

---

## 3. Progressive Disclosure: Reference Routing Map

For complex, fragile, or multi-step tasks, read the specialized Level 3 reference documentation in `references/`:

| User Intent & Workflow | Specialized Reference Guide | Key Subcommands |
| :--- | :--- | :--- |
| **Batch Broadcast & Cold Contacts**<br>Announcements, anti-ban jitter delays, auto-retry, delivery verification | [`references/broadcast_and_antiban.md`](./references/broadcast_and_antiban.md) | `broadcast`, `verify`, `stats` |
| **Chat History, Search, & Backups**<br>Recent chat context, query search, sub-5ms local SQLite archive, export backup | [`references/chat_and_summarization.md`](./references/chat_and_summarization.md) | `recent`, `search`, `export-backup` |
| **Dual-Gateway & Group Metadata**<br>Companion account queries, group participants, member sync, OpSec privacy rules | [`references/dual_gateway_and_groups.md`](./references/dual_gateway_and_groups.md) | `groups`, `group-info`, `sync-members` |
| **Media, Reactions, & Status**<br>Delivering PDFs/images, emoji reactions, profile avatar/bio, 24h status stories | [`references/media_and_interactions.md`](./references/media_and_interactions.md) | `send-media`, `reaction`, `status-*` |

Refer to [`assets/sample_broadcast.json`](./assets/sample_broadcast.json) for an example recipient dataset schema.
