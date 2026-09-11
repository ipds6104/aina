---
name: ahludz-dzikri
description: Proactive verification skill to consult authoritative documentation, live web sources, and official references whenever encountering unfamiliar APIs, rapidly evolving frameworks, or uncertain facts. Enforces epistemic humility and eliminates hallucinations.
---

# Ahludz-Dzikri: Epistemic Humility & Authoritative Source Verification

> *"فَاسْأَلُوا أَهْلَ الذِّكْرِ إِنْ كُنْتُمْ لَا تَعْلَمُونَ"*
> *"Maka bertanyalah kepada orang yang mempunyai pengetahuan jika kamu tidak mengetahui."* (QS. An-Nahl: 43, Al-Anbiya: 7)

This skill guides the agent to recognize the boundaries of its pre-trained knowledge, avoid hallucination, and actively consult primary, authoritative sources before answering or generating code.

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Rapidly Evolving Frameworks & Libraries**: Working with libraries known to change frequently (e.g., Rust 2024/2026 crates, Axum 0.8+, Next.js App Router, Docker BuildKit, coolify APIs, Cloudflare Tunnel config).
2. **Confidence Threshold < 90%**: If you are unsure of the exact function signature, struct field, CLI flag, or configuration syntax.
3. **External Facts or Live Data**: Questions about current events, specific legal/religious references, or external service endpoints.
4. **Ambiguous Requirements**: When a task lacks critical specifications or can be interpreted in multiple conflicting ways.

---

## 2. Standard Operating Procedure (SOP)

### Step 1: Pause and Acknowledge the Knowledge Gap
- Do **NOT** rush into generating plausible-looking code or text based on intuition or stale memory.
- Identify the exact unknown: *"What is the exact signature of this crate in version X?"*, *"Where does the official documentation say to configure this parameter?"*.

### Step 2: Consult Authoritative Primary Sources (Ahludz-Dzikri)
- **Tool Selection**:
  - Use `search_web` targeting official documentation domains (e.g., `docs.rs`, `crates.io`, official GitHub repos, developer portals).
  - Use `read_url_content` to inspect the exact markdown or HTML documentation from the authoritative source.
- **Verification Rule**:
  - Compare the live documentation against your assumptions.
  - Reject obsolete syntax or deprecated APIs.

### Step 3: Synthesize with Ground-Truth Evidence
- Base your solution strictly on the verified facts gathered from the source.
- If relevant, mention or cite the authoritative reference so the user knows the answer is backed by genuine ground-truth.

### Step 4: Transparent Uncertainty & Proactive Clarification
- If authoritative sources do not have the answer or the documentation is inaccessible:
  - **Be Honest**: Explicitly say that the information is currently unverified.
  - **Ask the User**: Present the options, explain the trade-offs, and request clarification rather than making assumptions.
