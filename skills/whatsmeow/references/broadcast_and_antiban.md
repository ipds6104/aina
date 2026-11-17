# Broadcast Messaging, Verification, and Anti-Ban Guidelines

## Table of Contents
1. [Overview & Anti-Ban Architecture](#1-overview--anti-ban-architecture)
2. [SOP: Batch Broadcast Messaging](#2-sop-batch-broadcast-messaging)
3. [SOP: Verifying Message Delivery in Gateway Store](#3-sop-verifying-message-delivery-in-gateway-store)
4. [SOP: Monitoring Gateway Health & Anti-Ban Quota](#4-sop-monitoring-gateway-health--anti-ban-quota)
5. [Troubleshooting Cold Contact Prekey Failures](#5-troubleshooting-cold-contact-prekey-failures)

---

## 1. Overview & Anti-Ban Architecture

WhatsApp implements strict heuristic rate limits and anomaly detection algorithms against automated accounts. When contacting recipients for the first time without prior conversation history (cold contacts), WhatsApp requires an initial Olm/Signal cryptographic prekey handshake.

Sending messages too rapidly or in parallel without jittered delays triggers:
- Prekey fetch timeouts (HTTP 500 on Whatsmeow gateway).
- Temporary account suspensions or shadow-bans.

To guarantee delivery and safety, the `broadcast` engine in `wa_tool.py` enforces:
- **Sequential Dispatch with Humanized Jitter Delay**: Configurable random delay between `--delay-min` and `--delay-max` (defaults: 3.0 to 6.0 seconds per contact).
- **Per-Contact Auto-Retry with Backoff**: Automatic retries for transient HTTP 500/network glitches.
- **Dry-Run Audit**: Pre-flight validation of placeholders and normalized recipient JIDs before actual network transmission.
- **Post-Dispatch Audit Log**: Machine-readable JSON summary of message IDs and delivery outcomes saved to disk.

---

## 2. SOP: Batch Broadcast Messaging

### Step 1: Prepare the Recipient Dataset
Recipients can be provided as a JSON file, a CSV file, or an inline JSON array string.
Supported recipient fields:
- `phone` or `recipient` or `jid`: The target phone number (e.g. `0896-9403-0497`, `+6289694030497`, or `6289694030497@s.whatsapp.net`).
- Any custom template variables, such as `name`, `role`, `location`, `schedule`.

Example `recipients.json`:
```json
[
  {
    "name": "Suyehdi",
    "phone": "0897-7834-431",
    "role": "Petugas Lapangan"
  },
  {
    "name": "Arya Daffa Dewansyah",
    "phone": "0896-9403-0497",
    "role": "Petugas Pengolahan"
  }
]
```

### Step 2: Validate Template with Dry-Run
Always perform a dry-run first to inspect placeholder rendering and normalized JIDs:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py broadcast \
  --recipients "recipients.json" \
  --template "Halo Kak {name}, Anda diterima sebagai {role} SPKRT 2027 BPS Mempawah." \
  --dry-run
```
The CLI will print rendered previews for each contact with `dry_run: true` without sending real WhatsApp packets.

### Step 3: Execute Live Broadcast Dispatch
Once verified, execute the live dispatch with humanized jitter delay and audit reporting:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py broadcast \
  --recipients "recipients.json" \
  --template "Halo Kak {name}, Anda diterima sebagai {role} SPKRT 2027 BPS Mempawah." \
  --delay-min 3.0 \
  --delay-max 6.0 \
  --retry 3 \
  --retry-delay 2.0 \
  --out "output/broadcast_spkrt_report.json"
```

Available Flags:
- `--recipients <path|json>`: Path to JSON/CSV file or inline JSON string.
- `--csv-file <path>`: Explicit CSV input path (alternative to `--recipients`).
- `--template <string>`: Message template containing `{placeholder}` variables.
- `--delay-min <float>`: Minimum delay in seconds between messages (default: 3.0).
- `--delay-max <float>`: Maximum delay in seconds between messages (default: 6.0).
- `--retry <int>`: Maximum retry attempts per failed recipient (default: 3).
- `--retry-delay <float>`: Base delay in seconds before retrying (default: 2.0).
- `--dry-run`: Parse and validate only without network calls.
- `--out <path>`: Write detailed JSON audit report to disk.
- `--allow-partial`: Return exit code 0 even if some recipients failed after retries.

---

## 3. SOP: Verifying Message Delivery in Gateway Store

To verify whether a specific Message ID returned during dispatch exists and is formally acknowledged in the gateway store:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py verify \
  --id "3EB0508138023C95B93EEC" \
  --jid "6289694030497@s.whatsapp.net"
```

Exit Codes & Behavior:
- Exit code `0`: Message ID found, verified as sent/recorded in gateway.
- Exit code `1`: Message ID not found in recent store or verification query failed. Pass `--allow-error` to suppress non-zero exit code if conditional branching is required in external scripts.

Sample Output:
```json
{
  "success": true,
  "error": false,
  "verified": true,
  "found": true,
  "message_id": "3EB0508138023C95B93EEC",
  "chat_jid": "6289694030497@s.whatsapp.net",
  "sender_jid": "628982157341:2@s.whatsapp.net",
  "timestamp": "2026-11-16T15:23:01.284934+07:00",
  "is_from_me": true,
  "text": "Halo Kak Arya Daffa Dewansyah..."
}
```

---

## 4. SOP: Monitoring Gateway Health & Anti-Ban Quota

Before initiating a large broadcast (e.g. > 50 recipients), inspect the current gateway connection uptime and daily quota metrics:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py stats
```

Sample Output:
```json
{
  "success": true,
  "is_connected": true,
  "uptime_seconds": 86400,
  "device_jid": "628982157341:2@s.whatsapp.net",
  "daily_count": 142,
  "daily_cap": 500,
  "is_rate_limited": false
}
```

Governance Rules:
- If `is_connected` is `false`, stop dispatch and alert the administrator to re-authenticate or check Docker logs.
- If `daily_count` reaches 85% of `daily_cap`, split broadcasts into multiple batches across different days or use the Companion gateway account where appropriate.

---

## 5. Troubleshooting Cold Contact Prekey Failures

If individual numbers return HTTP 500 or timeout during broadcast:
1. **Check Phone Format**: Confirm that the number was properly normalized. Raw `08xx` digits must become `628xx@s.whatsapp.net`. The script handles this automatically, but manually supplied JIDs must not start with `08`.
2. **Increase Jitter Delay**: If sending to fresh numbers without prior contact history, widen the delay window:
   `--delay-min 5.0 --delay-max 10.0`
3. **Inspect Output Audit**: Read the report saved via `--out <path>` to isolate failed contacts and re-dispatch only the failed entries.
