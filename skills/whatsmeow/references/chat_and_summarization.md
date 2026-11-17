# Chat History, Summarization, and Backup Operations

## Table of Contents
1. [Overview & Retrieval Strategy](#1-overview--retrieval-strategy)
2. [SOP: Fetching Recent Messages & Context Summarization](#2-sop-fetching-recent-messages--context-summarization)
3. [SOP: Searching & Filtering Chat History](#3-sop-searching--filtering-chat-history)
4. [SOP: High-Speed Local Search via Native CLI](#4-sop-high-speed-local-search-via-native-cli)
5. [SOP: Exporting Group Chat Backups](#5-sop-exporting-group-chat-backups)

---

## 1. Overview & Retrieval Strategy

When users ask Aina to catch up, summarize discussions, search past instructions, or export meeting notes from a WhatsApp chat:
- For recent live messages directly from the WhatsApp gateway, use `wa_tool.py recent` or `wa_tool.py search`.
- For fast historical search spanning days or weeks without putting load on the gateway, use the native CLI archive search: `aina archive search`.

---

## 2. SOP: Fetching Recent Messages & Context Summarization

### Execution
Fetch recent messages from a target chat (group JID or individual contact JID):
```bash
python3 skills/whatsmeow/scripts/wa_tool.py recent \
  --jid "<CHAT_JID>" \
  --limit 30
```

To fetch messages from a group observed by the companion account:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py recent \
  --jid "<GROUP_JID>" \
  --companion \
  --limit 30
```

### Synthesis Guidelines for the Agent
When generating a summary from fetched messages:
1. **Identify the Core Discussion**: State what topic was actively being resolved.
2. **Key Decisions & Agreements**: Explicitly list points agreed upon by participants.
3. **Action Items & Owners**: Highlight follow-up tasks, deadlines, and assigned staff.
4. **Tone**: Objective, structured, and free of fluff or conversational padding.

---

## 3. SOP: Searching & Filtering Chat History

### Keyword Search
Search recent chat records in the gateway store matching a specific string:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py search \
  --jid "<CHAT_JID>" \
  --query "jadwal pelatihan" \
  --limit 50
```

### Media Search
Filter messages that include attachments (images, PDFs, documents, audio):
```bash
python3 skills/whatsmeow/scripts/wa_tool.py search \
  --jid "<CHAT_JID>" \
  --has-media true \
  --limit 50
```

---

## 4. SOP: High-Speed Local Search via Native CLI

Aina automatically archives inbound and outbound messages in local SQLite storage (`/app/data/aina.db`). For historical queries across long time horizons, the native CLI provides sub-5ms indexed searches.

### Queries by Relative Time Horizon
```bash
# Search within the last 7 days
aina archive search "numpad" --since 7d

# Search within the last 24 hours
aina archive search "konfirmasi" --since 24h
```

### Queries by Specific Date Range
```bash
aina archive search "pengolahan SPKRT" --from 2026-11-01 --to 2026-11-16
```

---

## 5. SOP: Exporting Group Chat Backups

When authorized personnel request an offline transcript or backup file of a group chat:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py export-backup \
  --jid "<GROUP_JID>" \
  --limit 1000 \
  --out "workspace/backup_<GROUP_JID>.json"
```

The output file contains an array of message records with timestamps, sender JIDs, push names, message text, and media metadata.
Always confirm the caller has ADMIN or STAFF privileges before exporting complete conversation transcripts.
