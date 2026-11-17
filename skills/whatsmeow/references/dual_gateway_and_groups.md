# Dual-Gateway Architecture, Groups, and OpSec Policies

## Table of Contents
1. [Dual-Gateway Architecture](#1-dual-gateway-architecture)
2. [SOP: Companion Gateway Operations](#2-sop-companion-gateway-operations)
3. [SOP: Inspecting Group Metadata & Membership](#3-sop-inspecting-group-metadata--membership)
4. [SOP: Synchronizing Members to SQLite](#4-sop-synchronizing-members-to-sqlite)
5. [OpSec & Privacy Boundaries](#5-opsec--privacy-boundaries)

---

## 1. Dual-Gateway Architecture

Aina connects to two distinct WhatsApp gateways concurrently:
1. **Primary Bot Gateway (`whatsmeow-bot`)**:
   - The dedicated identity of Aina (`628982157341@s.whatsapp.net`).
   - Acts as the primary responder in groups and direct messages.
   - Target for incoming webhooks and standard proactive messages.
2. **Companion Shadow Gateway (`whatsmeow-companion`)**:
   - The personal WhatsApp account of the administrator (`6289625345646@s.whatsapp.net`).
   - Acts as a shadow sensor for monitoring official internal satker groups where adding a dedicated bot number is prohibited by protocol or policy.
   - Invoked via the `--companion` flag in `wa_tool.py`.

---

## 2. SOP: Companion Gateway Operations

### List Joined Groups
To view which groups the companion account currently participates in:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py groups --companion
```

### Fetch Messages from Companion Groups
To read recent context from a group observed via the companion account:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py recent \
  --jid "<GROUP_JID>" \
  --companion \
  --limit 30
```

### Search Conversations in Companion Groups
```bash
python3 skills/whatsmeow/scripts/wa_tool.py search \
  --jid "<GROUP_JID>" \
  --query "surat tugas" \
  --companion \
  --limit 50
```

---

## 3. SOP: Inspecting Group Metadata & Membership

To inspect group participants, topic, and administrative privileges:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py group-info --jid "<GROUP_JID>"
```

Key fields in output:
- `subject`: The visible group name.
- `owner_jid`: The creator or primary owner of the group.
- `participants`: List of member JIDs with `is_admin` and `is_super_admin` boolean indicators.

---

## 4. SOP: Synchronizing Members to SQLite

To synchronize group participants into Aina's local SQLite store for role-based access checks:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py sync-members --jid "<GROUP_JID>"
```

This updates local tables (`group_memberships`), allowing Aina to determine whether a message sender is a registered member or administrator of a specific coordination group.

---

## 5. OpSec & Privacy Boundaries

### Absolute Prohibition: 1-on-1 Companion DMs
- Under no circumstances may Aina read, query, summarize, or export private 1-on-1 direct messages between the companion account owner and other personal contacts.
- The `--companion` flag is restricted exclusively to coordination group JIDs (`@g.us`).

### Role-Based Access Control (RBAC)
- **ADMIN** and **STAFF**: Authorized to inspect group metadata, view participants, and trigger chat backups.
- **GUEST / EXTERNAL**: Strictly prohibited from viewing member phone numbers, exporting chat logs, or requesting member rosters.
- Standard decline response for unauthorized requests:
  > "Mohon maaf, demi menjaga privasi dan keamanan data tim, fitur penarikan informasi keanggotaan dan riwayat percakapan hanya dapat diakses oleh staf internal yang terverifikasi."
