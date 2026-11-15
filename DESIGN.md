# Aina Web Dashboard Design System (DESIGN.md)

> Formal design direction and visual specifications for the Aina Web Dashboard & Simulation Interface.
> Aligned with Anti-Slop (miqdadbadjuber/anti-slop) and Google Antigravity UI Design Hygiene standards.

## 1. Product Identity & Audience

- **Product**: Aina (あいな) - Autonomous Agentic AI Assistant & WhatsApp Gateway.
- **Audience**: Public service professionals (BPS Kabupaten Mempawah), administrators, and AI platform engineers.
- **Tone & Mood**: Understated, utilitarian, highly structured, and confident (Linear / GitHub / GOV.UK design language).
- **Core Stance**: Clean software engineering tool; zero marketing fluff, zero placeholder vanity metrics, zero AI emojis.

## 2. Three Dials (Liveliness Toolkit)

```
Dial: ENERGY 1 / RHYTHM 2 / MOTION 1
```

- **ENERGY 1 (Calm)**: Restrained, grounded surfaces. No full-page colored glows, no harsh gradients, no purple-neon themes.
- **RHYTHM 2 (Balanced)**: Clear spatial hierarchy with varied sections: top masthead, telemetry strip, expandable account drawer, observability breakdown, and interactive conversation thread.
- **MOTION 1 (Hover & Transitions Only)**: No perpetual endless loops or distracting pulsing dots. Transitions exist only as affordances for user interaction (hover, modal dismissal, tab switching).

## 3. Color Palette & Surface Tokens

Restrained 2-color neutral foundation with singular semantic accents:

- **Neutral Background (`--bg`)**: `#090d16` (Deep Slate / Pitch)
- **Primary Surface (`--surface`)**: `#111827` (Slate 900)
- **Subtle Surface (`--surface-subtle`)**: `#0d1424` (Slate 950 tinted)
- **Borders (`--border`)**: `#1f2937` (Slate 800, 1px solid crisp)
- **Border Hover (`--border-hover`)**: `#374151` (Slate 700)
- **Primary Text (`--text`)**: `#f8fafc` (Slate 50, contrast ratio > 12:1)
- **Secondary Text (`--text-secondary`)**: `#cbd5e1` (Slate 300, contrast ratio > 7:1)
- **Muted Text (`--text-muted`)**: `#94a3b8` (Slate 400, contrast ratio > 4.5:1)
- **Primary Brand Accent (`--primary`)**: `#38bdf8` (Sky 400, reserved for primary CTA & active indicators)
- **Success Accent (`--success`)**: `#10b981` (Emerald 500, state markers)
- **Warning Accent (`--warning`)**: `#f59e0b` (Amber 500, cooldowns and attention)
- **Danger Accent (`--danger`)**: `#ef4444` (Rose 500, locks and error states)

## 4. Typography

- **Interface Font**: `'Google Sans', 'Google Sans Text', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`
- **Monospace Font**: `'JetBrains Mono', 'Fira Code', monospace`
- **Typographic Rules**:
  - Zero em dashes (`—`) in UI copy (R-02); use commas, periods, or colons.
  - Generous line-height (`1.6` for body, `1.3` for titles).
  - Explicit size scaling: `h1` (1.5rem), `h2` (1.15rem), `h3` (1.0rem), `caption` (0.78rem).

## 5. Components & Ergonomics

- **Interactive Target Size**: Minimum 44px tap target height for all buttons, inputs, and selects (R-03).
- **Focus Rings**: Accessible `:focus-visible` with `outline: 2px solid var(--primary); outline-offset: 2px;` (R-32).
- **Status Markers**: Crisp static dot indicators (`.status-dot`), zero infinite animations (R-19).
- **Cards**: Flat elevation with crisp borders, radius `8px` (`--radius-md`).
- **Icons**: Inline standard SVG vector icons only; 0 unicode emojis (R-04).
