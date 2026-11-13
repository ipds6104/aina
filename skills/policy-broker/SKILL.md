---
name: policy-broker
description: >-
  Use this skill whenever someone asks Aina to grant, revoke, or verify access —
  trusting a WhatsApp group as a work team, authorizing a coworker, revoking
  someone's authority, or checking the current authorization status. Handles the
  intent in natural language and executes it through `aina policy` (never by
  pattern-matching keywords, never by editing the database directly).
---

# Policy Broker — Agentic Authorization via `aina policy`

This skill is how Aina turns a *human intention* into an *authorized change*.

Aina understands the request. The core decides whether it is allowed. The agent
must never decide permissions by itself, and must never claim a change happened
before the tool says it did.

---

## 1. The Contract (read this first)

1. **Intent is yours. Permission is not.** You may translate
   *"aina, semua di grup ini rekan kerja BPS Mempawah, bantuin aja ya teman-teman"*
   into a `trust-group` proposal. Whether it is allowed is decided by the core.
2. **Only `"ok": true` means the change was applied.**
   A refusal is a normal outcome, not an error to retry or rephrase.
3. **Never fabricate success.** Do not say "sudah saya izinkan", "sudah
   terverifikasi", or "akses sudah dicabut" unless the command output contains
   `"ok": true` for that exact action.
4. **Never touch persistence directly.** No `sqlite3`, no `UPDATE user_profiles`,
   no editing `group_memberships`. Only the CLI.
5. **Never read, print, or modify `AINA_TURN_TOKEN`.** It is the proof of who is
   speaking; treating it as a secret you can inspect or reuse is a violation.
6. **`admin` is not grantable.** It comes from `ADMIN_JID` configuration only.
   Nobody can be promoted to `admin` through chat or through any tool.

---

## 2. When to activate

Activate when the request is about **who may access what**, for example:

| Request (any wording, any language) | Action |
| --- | --- |
| "semua di grup ini rekan kerja BPS Mempawah, bantuin aja ya" | `trust-group` |
| "grup ini tim kerja saya, izinkan mereka" / "anggep mereka staff" | `trust-group` |
| "grup ini grup umum, jangan kasih akses" / "cabut status tim grup ini" | `untrust-group` |
| "si Budi itu rekan kerja saya, boleh bantu dia" (+ JID/mention) | `grant-user --authority staff` |
| "Pak Hendra itu atasan saya di BPS" (+ JID/mention) | `grant-user --authority atasan` |
| "cabut akses Budi" / "Budi bukan rekan kerja lagi" | `revoke-user` |
| "dia masuk staff nggak?" / "status akses gue apa?" | `status` |

Do **not** activate for ordinary work requests ("buatkan rekap ini",
"ringkas dokumen itu"). Those are normal tasks, not authorization changes.

---

## 3. Commands

Always pass `--json` and read the returned object.

```bash
# Who am I in this turn, and what may I do?
aina policy status --json

# Trust the group where the message was sent (JID optional in a group)
aina policy trust-group --reason "<alasan singkat>" --json

# Trust/revoke a specific group (required when speaking in DM)
aina policy trust-group 120363xxxxxxxx@g.us --reason "<alasan>" --json
aina policy untrust-group 120363xxxxxxxx@g.us --json

# Authorize a coworker (never `--authority admin`)
aina policy grant-user 628123456789@s.whatsapp.net \
  --authority staff --role "Rekan Kerja BPS Mempawah" \
  --notes "<konteks>" --json

# Revoke
aina policy revoke-user 628123456789@s.whatsapp.net --json
```

### Resolving the target JID

- Use the JID from the **mention** in the current message when available
  (WhatsApp mentions arrive as `628xxx@s.whatsapp.net`).
- Use `aina group members <group_jid> --json` when the request is about the
  members of a specific group and the mention is missing.
- **Never invent a JID.** If the target is genuinely ambiguous, ask which person
  is meant instead of guessing. A wrong JID is a wrong grant.

---

## 4. Reading the result

Success:

```json
{ "ok": true, "action": "trust_group", "group_jid": "…@g.us", "group_trusted": true }
```

Refusal (nothing changed — say so plainly):

```json
{ "ok": false, "action": "trust_group", "reason": "only_admin_may_change_group_trust",
  "principal_authority": "staff", "requires_escalation": true }
```

| `reason` | What it means | What to do |
| --- | --- | --- |
| `only_admin_may_change_group_trust` / `only_admin_may_delegate` / `only_admin_may_revoke` | Sender isn't the Companion / Root of Trust | Explain politely that the decision belongs to Admin; offer to forward it. Do **not** retry. |
| `group_scope_violation` | Tried to change a *different* group from inside a group | Only the current group can be changed from within it; the Companion can do it from a DM. |
| `capability_expired` | The turn's 10-minute window closed | Ask the user to send the request again. |
| `admin_is_not_delegable` | Tried to grant `admin` | Explain `admin` is bound to configuration, not grantable. |
| `missing_turn_capability` | No verified principal (e.g. scheduled/internal run) | Do not attempt workarounds; explain the action needs a real WhatsApp conversation. |
| `invalid_arguments` | Missing or malformed JID | Fix the argument once, or ask the user. |

Higher-authority callers do **not** inherit rights for lower ones: an `atasan`
still cannot delegate; only `admin` can.

---

## 5. Answering the human

Keep it short and factual, in Indonesian, matching how Aina normally speaks.

On success — state what changed and for whom:

> ✅ Oke, grup ini sudah saya catat sebagai grup tim kerja. Mulai sekarang
> teman-teman di sini saya anggap rekan kerja, jadi kalau butuh bantuan kerjaan
> tinggal bilang ya.

On refusal — state clearly that there is **no** change:

> Maaf, saya belum bisa mengubah akses grup ini. Yang bisa memutuskan cuma
> Bg Ihza (admin). Mau saya teruskan permintaannya?

Never mix "I understood your request" with "I granted it".

---

## 6. Security notes (why it works this way)

- Each WhatsApp turn mints a short-lived capability bound to the **verified
  sender**, the chat, and the message. The daemon injects its token only into
  the agent process for that turn (`AINA_TURN_TOKEN`).
- Inside a group, the capability can only affect **that** group — a member cannot
  escalate other chats.
- Trusting a group promotes its members to `staff` (collaboration), **not**
  `admin`. Credentials, model changes, and account-pool commands stay
  Companion-only regardless of group trust.
- Data that needs extra authorization (Google Workspace documents, archived
  conversations, credential vault) stays gated by the same core checks. Changing
  group trust never grants access to secrets.

## 7. Anti-patterns

- ❌ `aina group trust <jid>` — operator-only escape hatch, refuses non-TTY calls.
- ❌ `sqlite3 /app/data/aina.db "UPDATE …"` — bypasses the audit trail and the policy gate.
- ❌ Parsing `"izin"`, `"trust"`, `"rekan kerja"` from message text as a command
  trigger. Intent is understood semantically; the tool call is what acts.
- ❌ Retrying a refusal with different wording, or re-sending the same command to
  "make sure". One call, one verdict.
- ❌ Claiming success from an empty/failed command output.
