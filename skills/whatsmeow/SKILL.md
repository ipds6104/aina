---
name: whatsmeow
description: >-
  Use this skill whenever the user asks to send WhatsApp messages or test sending messages,
  execute batch broadcast announcements, verify message delivery status,
  read recent WhatsApp chat history, summarize previous group conversations,
  inspect group participants or admins, export chat backups, or send media and document files via the Whatsmeow gateway.
---

# Whatsmeow Gateway & Chat Operations Skill

This skill equips Aina with procedural workflows to interact with the companion Whatsmeow WhatsApp Gateway via its REST API using the helper tool [`scripts/wa_tool.py`](./scripts/wa_tool.py).

---

## 1. When to Activate This Skill

Activate this skill when:
1. **Sending Direct or Self-Test Messages**: The user asks to send a WhatsApp message, or test sending a message to themselves or another contact/group.
2. **Executing Batch Broadcast Announcements**: The user asks to notify multiple contacts or partners (such as enumerators, survey partners, or staff) with personalized messages.
3. **Verifying Message Delivery**: Confirming whether a specific WhatsApp message ID has been acknowledged and recorded by the gateway.
4. **Summarizing Past Conversations**: The user in a group or DM asks to summarize recent discussions or context.
5. **Exporting Group Chat / Backups**: The user asks for a chat export, transcript backup, or meeting minutes compilation from a WhatsApp group.
6. **Inspecting Group Metadata**: Checking group participant lists, admin authority, or group topics.
7. **Sending Media / Generated Files**: Delivering files generated in the workspace (scripts, PDF reports, charts, CSV/Excel) directly to WhatsApp chats.

---

## 2. Helper Script Location & Calling Convention

The helper tool is located at `skills/whatsmeow/scripts/wa_tool.py` within this skill directory.
It automatically reads `WHATSMEOW_BASE_URL` and `WHATSMEOW_API_KEY` from the environment or configuration.

Always run it using Python 3:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]
```
*(Or in container context: `python3 /app/skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]`)*

### Invariants and CLI Contracts
- **Deterministic Exit Codes**: Exits with `0` on success and `1` on failure (such as HTTP 4xx/5xx or network errors). Scripts can safely use `check=True` in Python `subprocess.run` or `set -e` in Bash.
- **Normalized Response Envelope**: Key response fields (`message_id`, `status`, `recipient`) are always flattened at the top level of the JSON output.
- **Automatic JID Normalization**: Local Indonesian phone formats (`0896-xxx`, `+62 896 xxx`, `08xxx@s.whatsapp.net`) are automatically sanitized to canonical international JIDs (`628xxx@s.whatsapp.net`).

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Sending Direct WhatsApp Text Messages (Proactive / External Only)

> [!WARNING] **Anti-Double Send Rule**
> - **DILARANG** menggunakan `send-text` untuk membalas percakapan aktif yang sedang berlangsung saat ini. Cukup tuliskan teks jawaban langsung di respons akhir turn. Backend Aina akan otomatis mengirimkannya ke WhatsApp.
> - `send-text` **HANYA** digunakan jika user secara eksplisit memintamu mengirimkan pesan proaktif ke kontak lain atau grup lain di luar ruang obrolan saat ini.

When an authorized user asks to send a proactive message to another contact or group:
1. Target recipient can be specified as a raw phone number (`0896-xxx`) or canonical JID (`628xxx@s.whatsapp.net` or `120363xxx@g.us`).
2. Dispatch via `send-text` with built-in retry and backoff:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py send-text \
     --to "0896-9403-0497" \
     --text "Halo Kak Arya, berikut konfirmasi jadwal koordinasi besok." \
     --retry 3
   ```
3. Parse the returned JSON to verify `"status": "sent"` and report the generated `message_id` to the user.

---

### SOP 2: Batch Broadcast Messaging (Cold Contact Safe)

When sending personalized announcements or notifications to a list of recipients (such as acceptance letters or meeting invites):
1. Prepare a JSON or CSV file containing recipient records, or pass an inline JSON string:
   ```json
   [
     {"name": "Suyehdi", "phone": "0897-7834-431"},
     {"name": "Arya Daffa Dewansyah", "phone": "0896-9403-0497"}
   ]
   ```
2. Validate formatting using `--dry-run` first:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py broadcast \
     --recipients "recipients.json" \
     --template "Halo Kak {name}, salam hangat! Anda dinyatakan diterima sebagai petugas." \
     --dry-run
   ```
3. Execute live broadcast with anti-ban delay and per-recipient retries:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py broadcast \
     --recipients "recipients.json" \
     --template "Halo Kak {name}, salam hangat! Anda dinyatakan diterima sebagai petugas." \
     --delay-min 3.0 \
     --delay-max 6.0 \
     --retry 3 \
     --out "output/broadcast_report.json"
   ```
4. Features of the broadcast engine:
   - **Humanized Jitter Delay**: Random delays between `--delay-min` and `--delay-max` prevent triggering WhatsApp anti-spam filters.
   - **Cold Contact Retry**: Handshake prekey delays on previously uncontacted numbers are automatically retried with exponential backoff.
   - **Audit Report**: Generates a complete JSON audit log with delivery status and official Message IDs for every recipient.

---

### SOP 3: Verifying Message Delivery in Gateway Store

To verify whether a specific Message ID was recorded and acknowledged by the gateway:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py verify --id "3EB0508138023C95B93EEC"
```
Or scoped to a specific chat:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py verify --id "3EB0508138023C95B93EEC" --jid "6289694030497@s.whatsapp.net"
```
The command returns exit code `0` if found and verified, or exit code `1` if the message ID does not exist in the recent gateway store.

---

### SOP 4: Fetching Recent Messages & Context Summarization

When asked to summarize or catch up on what was discussed:
1. Determine the target chat JID (e.g. `120363xxx@g.us` for groups or `628xxx@s.whatsapp.net` for DM).
2. Fetch recent messages:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py recent --jid "<CHAT_JID>" --limit 30
   ```
3. Synthesize an objective, structured summary:
   - Key topics discussed.
   - Decisions or agreements made.
   - Action items / pending tasks and assigned owners.

---

### SOP 5: Searching & Filtering Chat History

When an authorized user asks to search specific conversations or topics:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py search \
  --jid "<CHAT_JID>" \
  --query "kata kunci" \
  --limit 100
```
Search for messages containing media attachments:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py search \
  --jid "<CHAT_JID>" \
  --has-media true \
  --limit 50
```

> [!TIP]
> **Pencarian Riwayat Lokal Super Cepat (<5ms) via Native CLI (`aina archive search`)**:
> Untuk menelusuri riwayat pesan yang telah tercatat di SQLite lokal (`aina.db`) dengan filter waktu presisi tanpa membebani gateway Whatsmeow:
> ```bash
> # Cari obrolan 7 hari terakhir
> aina archive search "kata kunci" --since 7d
>
> # Cari obrolan rentang tanggal spesifik
> aina archive search "kata kunci" --from 2026-09-01 --to 2026-09-10
> ```

---

### SOP 6: Exporting Group Chat Backups

When an authorized user requests a chat backup or comprehensive audit log:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py export-backup \
  --jid "<GROUP_JID>" \
  --limit 1000 \
  --out "workspace/backup_<GROUP_JID>.json"
```

---

### SOP 7: Dual-Gateway Operations & Companion Account Queries

Sistem Aina mendukung arsitektur Dual-Gateway (Gateway Bot Utama & Gateway Akun Pribadi Companion Admin).
Gunakan flag `--companion` ketika diminta mengakses informasi atau obrolan grup dari akun pribadi Companion Admin:
1. **Melihat daftar grup yang diikuti akun Companion**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py groups --companion
   ```
2. **Membaca riwayat obrolan grup pada akun Companion**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py recent --companion --jid "<GROUP_JID>" --limit 30
   ```
3. **Mencari pesan atau kata kunci tertentu di grup Companion**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py search --companion --jid "<GROUP_JID>" --query "kata kunci" --limit 50
   ```

> [!WARNING] **Batasan Privasi Mutlak**
> Dilarang membaca, mengekspor, atau menyebarluaskan riwayat obrolan pribadi (1-on-1 DM) antara pemilik akun Companion dengan kontak lain. Akses hanya diperuntukkan bagi grup koordinasi kedinasan.

---

### SOP 8: Inspecting Group Participants & Authority

To verify participant membership or administrative roles in a WhatsApp group:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py group-info --jid "<GROUP_JID>"
```
Inspect the `participants` array for `is_admin` or `is_super_admin` flags.

---

### SOP 9: Delivering Files & Media to WhatsApp

When delivering files generated in the workspace (reports, data charts, CSV/Excel spreadsheets):
```bash
python3 skills/whatsmeow/scripts/wa_tool.py send-media \
  --to "<RECIPIENT_JID>" \
  --file "workspace/laporan.pdf" \
  --type "document" \
  --caption "Berikut berkas laporan yang Anda minta." \
  --retry 3
```

---

### SOP 10: Monitoring Gateway Health & Anti-Ban Quota

To inspect connection uptime or check remaining daily anti-ban quota:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py stats
```
Output displays device connection state (`is_connected`, `uptime`) and anti-ban metrics (`daily_count` vs `daily_cap`). If daily count approaches quota limit, conserve proactive message dispatches.

---

### SOP 11: Conversational Closure & WhatsApp Emoji Reactions

Untuk pesan penutup obrolan yang santun (seperti ucapan terima kasih *"Sama-sama kak"*, *"Makasih infonya"*, atau konfirmasi *"Siap kak"*, *"Noted"*):
- Hindari balasan teks panjang yang membebani lawan bicara dengan kewajiban membalas lagi.
- Berikan reaksi emoji WhatsApp:
  ```bash
  # Reaksi terima kasih / santun
  python3 skills/whatsmeow/scripts/wa_tool.py reaction --to "<CHAT_JID>" --id "<MSG_ID>" --emoji "\U0001F64F"

  # Reaksi konfirmasi / siap
  python3 skills/whatsmeow/scripts/wa_tool.py reaction --to "<CHAT_JID>" --id "<MSG_ID>" --emoji "\U0001F44D"
  ```

---

### SOP 12: Inspecting & Updating Profile Picture or Bio

1. **View Profile Picture**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py profile-picture-get --jid "628xxx@s.whatsapp.net"
   ```
2. **Update Profile Picture**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py profile-picture-set --file "assets/avatar.jpg"
   ```
3. **Update About Bio Status**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py about-set --status "Aina AI Assistant BPS Mempawah"
   ```

---

### SOP 13: WhatsApp Ephemeral Status Stories

1. **Post Text Status Story**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py status-send-text \
     --text "Pelatihan Petugas Pengolahan SPKRT 2027 dimulai hari Rabu, 18 November 2026." \
     --background "0xFF25D366" \
     --font 2
   ```
2. **Post Media Status Story**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py status-send-media \
     --file "assets/pengumuman.png" \
     --caption "Pengumuman resmi pelatihan petugas SPKRT 2027."
   ```
3. **List Active Stories**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py status-list --active-only
   ```
4. **Revoke Story**:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py status-revoke --id "3EB0XXXXX"
   ```

---

## 4. OpSec & Authority Policy

In accordance with office privacy guidelines:
- **ADMIN** and **STAFF**: Authorized to read chat history, inspect group metadata, and trigger backups.
- **GUEST / EXTERNAL**: Strictly forbidden from exporting group chat history, querying previous messages of other members, or harvesting member phone numbers.
- If a guest requests private chat exports, politely decline:
  > *"Mohon maaf, demi menjaga privasi dan keamanan data tim, fitur penarikan riwayat percakapan hanya dapat diakses oleh staf internal yang terverifikasi."*
