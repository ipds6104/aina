# Media Delivery, Reactions, Profile, and Status Operations

## Table of Contents
1. [SOP: Delivering Files & Media to WhatsApp](#1-sop-delivering-files--media-to-whatsapp)
2. [SOP: WhatsApp Emoji Reactions for Conversational Closure](#2-sop-whatsapp-emoji-reactions-for-conversational-closure)
3. [SOP: Managing Profile Picture & About Bio](#3-sop-managing-profile-picture--about-bio)
4. [SOP: Ephemeral 24-Hour WhatsApp Status Stories](#4-sop-ephemeral-24-hour-whatsapp-status-stories)

---

## 1. SOP: Delivering Files & Media to WhatsApp

When asked to send files generated in the workspace (such as PDF reports, data tables, spreadsheets, or charts):
```bash
python3 skills/whatsmeow/scripts/wa_tool.py send-media \
  --to "<RECIPIENT_JID>" \
  --file "workspace/laporan.pdf" \
  --type "document" \
  --caption "Berikut berkas laporan yang Anda minta." \
  --retry 3
```

Supported Media Types (`--type`):
- `document`: PDF, Excel (.xlsx), Word (.docx), CSV, ZIP, TXT.
- `image`: PNG, JPG, WebP.
- `audio`: MP3, OGG, M4A, WAV.
- `video`: MP4, MKV.

---

## 2. SOP: WhatsApp Emoji Reactions for Conversational Closure

When concluding a conversation with polite acknowledgments (e.g. "Sama-sama", "Makasih infonya", "Siap kak", "Noted"):
- Avoid sending an extra text message that burdens the user with a perceived obligation to respond again.
- Send a native WhatsApp emoji reaction directly to the user's last message:

```bash
# Polite thanks / appreciation reaction (folded hands)
python3 skills/whatsmeow/scripts/wa_tool.py reaction \
  --to "<CHAT_JID>" \
  --id "<MESSAGE_ID>" \
  --emoji "\U0001F64F"

# Confirmation / acknowledgment reaction (thumbs up)
python3 skills/whatsmeow/scripts/wa_tool.py reaction \
  --to "<CHAT_JID>" \
  --id "<MESSAGE_ID>" \
  --emoji "\U0001F44D"
```

---

## 3. SOP: Managing Profile Picture & About Bio

### View Profile Picture
```bash
python3 skills/whatsmeow/scripts/wa_tool.py profile-picture-get --jid "628xxx@s.whatsapp.net"
```

### Update Bot Profile Picture
```bash
python3 skills/whatsmeow/scripts/wa_tool.py profile-picture-set --file "assets/bot_avatar.jpg"
```

### Remove Profile Picture
```bash
python3 skills/whatsmeow/scripts/wa_tool.py profile-picture-remove
```

### Update About Bio Status Text
```bash
python3 skills/whatsmeow/scripts/wa_tool.py about-set --status "Aina AI Assistant BPS Kabupaten Mempawah"
```

---

## 4. SOP: Ephemeral 24-Hour WhatsApp Status Stories

### Post Text Status Story
```bash
python3 skills/whatsmeow/scripts/wa_tool.py status-send-text \
  --text "Pelatihan Petugas Pengolahan SPKRT 2027 dimulai Rabu, 18 November 2026." \
  --background "0xFF25D366" \
  --font 2
```

### Post Media Status Story
```bash
python3 skills/whatsmeow/scripts/wa_tool.py status-send-media \
  --file "assets/pengumuman.png" \
  --caption "Pengumuman resmi pelatihan petugas SPKRT 2027."
```

### List Active Status Stories
```bash
python3 skills/whatsmeow/scripts/wa_tool.py status-list --active-only
```

### Revoke / Delete a Status Story
```bash
python3 skills/whatsmeow/scripts/wa_tool.py status-revoke --id "<MESSAGE_ID>"
```
