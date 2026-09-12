---
name: whatsmeow
description: >-
  Use this skill whenever the user asks to send WhatsApp messages or test sending messages,
  read recent WhatsApp chat history, summarize previous group conversations,
  inspect group participants or admins, export chat backups, or send media and document files via the Whatsmeow gateway.
---

# Whatsmeow Gateway & Chat Operations Skill

This skill equips Aina with procedural workflows to interact with the companion Whatsmeow WhatsApp Gateway via its REST API using the helper tool [`scripts/wa_tool.py`](./scripts/wa_tool.py).

---

## 1. When to Activate This Skill

Activate this skill when:
1. **Sending Direct / Self-Test Messages**: The user asks to send a WhatsApp message, or test sending a message to themselves or another contact/group (e.g. *"Aina, tes kirim pesan ke diri sendiri di WhatsApp"*).
2. **Summarizing Past Conversations**: The user in a group or DM asks: *"Aina tolong rangkum diskusi tadi"*, *"Apa yang dibahas sebelum aku join?"*, or asks about recent context.
3. **Exporting Group Chat / Backups**: The user asks for a chat export, transcript backup, or meeting minutes compilation from a WhatsApp group.
4. **Inspecting Group Metadata**: Checking group participant list, who the admins are, or group subject/topic.
5. **Sending Media / Generated Files**: Aina has created a file in `workspace/` (e.g., Python script, PDF report, data chart, CSV/Excel) and needs to deliver it directly to the user on WhatsApp.

---

## 2. Helper Script Location & Calling Convention

The helper tool is located at `scripts/wa_tool.py` within this skill directory.
It automatically reads `WHATSMEOW_BASE_URL` and `WHATSMEOW_API_KEY` from the environment.

Always run it using Python 3:
```bash
python3 /app/workspace/.agents/skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]
```
*(Or relative to CWD: `python3 .agents/skills/whatsmeow/scripts/wa_tool.py <subcommand> [flags]`)*

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Sending Direct WhatsApp Text Messages & Self-Test
When an authorized user asks to send a WhatsApp message, or test sending a message to themselves or another contact/group:
1. Identify the target recipient JID (e.g. `$WHATSMEOW_BOT_JID` or specific phone number JID `628xxx@s.whatsapp.net` or group `120363xxx@g.us`).
2. Send the message immediately via `send-text`:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py send-text --to "<RECIPIENT_JID>" --text "<PESAN>"
   ```
   *Contoh tes kirim ke diri sendiri:*
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py send-text --to "${WHATSMEOW_BOT_JID:-6289625345646@s.whatsapp.net}" --text "Halo! Ini pesan tes verifikasi dari Aina di WhatsApp."
   ```
3. Konfirmasikan bahwa output JSON mengembalikan `"status": "sent"` dan sertakan ID pesan ke user.

### SOP 2: Fetching Recent Messages & Context Summarization
When asked to summarize or catch up on what was discussed:
1. Determine the target chat JID (from the incoming message context, e.g. `120363xxx@g.us` for groups or `628xxx@s.whatsapp.net` for DM).
2. Fetch the recent messages (e.g., 25–50 messages):
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py recent --jid "<CHAT_JID>" --limit 30
   ```
3. Parse the returned JSON to read the sequence of messages, senders, and timestamps.
4. Synthesize an objective, structured summary:
   - Key topics discussed.
   - Decisions or agreements made.
   - Action items / pending tasks (and who is assigned).

### SOP 3: Searching & Filtering Chat History
When an authorized user asks to search specific conversations or topics:
1. Run search with keyword query or sender filter:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py search \
     --jid "<CHAT_JID>" \
     --query "kata kunci" \
     --limit 100
   ```
   *Mencari pesan khusus yang memiliki lampiran media:*
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py search \
     --jid "<CHAT_JID>" \
     --has-media true \
     --limit 50
   ```
2. Analisis `matches` yang ditemukan untuk menjawab pertanyaan spesifik pengguna.

> [!TIP]
> **Pencarian Riwayat Lokal Super Cepat (<5ms) via Native CLI (`aina archive search`)**:
> Untuk menelusuri riwayat pesan yang telah tercatat di SQLite lokal (`aina.db` atau `messages.db`) dengan filter waktu presisi tanpa membebani gateway Whatsmeow:
> ```bash
> # Cari obrolan 7 hari terakhir
> aina archive search "kata kunci" --since 7d
>
> # Cari obrolan rentang tanggal spesifik
> aina archive search "kata kunci" --from 2026-09-01 --to 2026-09-10
> ```

### SOP 4: Exporting Group Chat Backups
When an authorized user requests a chat backup or comprehensive audit log:
1. Execute the backup command:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py export-backup --jid "<GROUP_JID>" --limit 1000 --out "/app/workspace/backup_<GROUP_JID>.json"
   ```
2. Verify that the file was created in `workspace/`.
3. Inform the user that the backup has been compiled, provide summary metrics (total messages, date range), and offer to deliver or analyze the file.

### SOP 5: Inspecting Group Participants & Authority
To verify if someone claiming to be an admin really has admin privileges in a WhatsApp group:
1. Run:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py group-info --jid "<GROUP_JID>"
   ```
2. Check the `participants` array for `is_admin` or `is_superadmin` flags.

### SOP 6: Delivering Files & Media to WhatsApp
If the user asked Aina to generate a report, script, or image:
1. Ensure the file is saved in the workspace.
2. Send the file to the recipient:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py send-media \
     --to "<RECIPIENT_JID>" \
     --file "/app/workspace/laporan.pdf" \
     --type "document" \
     --caption "Berikut berkas laporan yang Anda minta."
   ```

### SOP 7: Monitoring Gateway Health & Anti-Ban Quota
To inspect bot connection uptime or check how much daily message quota remains:
1. Run stats check:
   ```bash
   python3 .agents/skills/whatsmeow/scripts/wa_tool.py stats
   ```
2. Output menampilkan status koneksi device (`is_connected`, `uptime`) dan kuota anti-ban (`daily_count` vs `daily_cap`). Jika kuota mendekati batas harian (misal >280/300), hemat pengiriman pesan baru.

---

## 4. OpSec & Authority Policy (Strict Enforcement)

In accordance with the **OpSec** and **Tabayyun** guidelines:
- **ADMIN** and **STAFF**: Authorized to read chat history, inspect group metadata, and trigger backups.
- **GUEST / EXTERNAL**: **STRICTLY FORBIDDEN** from exporting group chat history, querying previous messages of other members, or extracting member phone numbers.
  - *Response for GUEST*: Politely refuse citing office privacy policy:
    > *"Mohon maaf, demi menjaga privasi dan keamanan data tim, fitur penarikan riwayat percakapan hanya dapat diakses oleh staf internal yang terverifikasi."*
