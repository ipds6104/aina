//! CSS styles and typography definitions for Aina Web Dashboard.
//! Engineered in strict compliance with DESIGN.md (Anti-Slop & UI Design Hygiene).

pub fn get_styles() -> &'static str {
    r#"
        :root {
            --bg: #090d16;
            --surface: #111827;
            --surface-subtle: #0d1424;
            --border: #1f2937;
            --border-muted: #1e293b;
            --border-hover: #374151;
            --text: #f8fafc;
            --text-secondary: #cbd5e1;
            --text-muted: #94a3b8;
            --primary: #38bdf8;
            --primary-hover: #0284c7;
            --primary-muted: rgba(56, 189, 248, 0.12);
            --success: #10b981;
            --success-muted: rgba(16, 185, 129, 0.12);
            --warning: #f59e0b;
            --warning-muted: rgba(245, 158, 11, 0.12);
            --danger: #ef4444;
            --danger-muted: rgba(239, 68, 68, 0.12);
            --font-sans: 'Google Sans', 'Google Sans Text', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
            --radius-sm: 6px;
            --radius-md: 8px;
            --radius-lg: 10px;
        }

        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            background-color: var(--bg);
            color: var(--text);
            font-family: var(--font-sans);
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
            line-height: 1.6;
            padding: 24px 16px;
            display: flex;
            justify-content: center;
            min-height: 100vh;
        }

        :focus-visible {
            outline: 2px solid var(--primary);
            outline-offset: 2px;
        }

        .container {
            width: 100%;
            max-width: 980px;
            margin: 0 auto;
        }

        /* Top Masthead & Navigation */
        .masthead {
            display: flex;
            justify-content: space-between;
            align-items: center;
            flex-wrap: wrap;
            gap: 16px;
            padding: 16px 20px;
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: var(--radius-md);
            margin-bottom: 24px;
        }

        .masthead-brand {
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .masthead-title {
            font-size: 1.35rem;
            font-weight: 700;
            color: var(--text);
            letter-spacing: -0.02em;
            display: flex;
            align-items: baseline;
            gap: 6px;
        }

        .masthead-sub {
            font-size: 0.82rem;
            color: var(--text-muted);
            font-weight: 400;
        }

        .masthead-actions {
            display: flex;
            align-items: center;
            gap: 12px;
            flex-wrap: wrap;
        }

        /* Status Dot Indicator (R-19: No endless infinite pulsing loops) */
        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            display: inline-block;
            flex-shrink: 0;
        }
        .status-dot-online { background-color: var(--success); }
        .status-dot-warning { background-color: var(--warning); }
        .status-dot-offline { background-color: var(--danger); }

        .badge {
            display: inline-flex;
            align-items: center;
            gap: 6px;
            padding: 4px 10px;
            border-radius: var(--radius-sm);
            font-size: 0.75rem;
            font-weight: 600;
            letter-spacing: 0.02em;
            background: var(--surface-subtle);
            border: 1px solid var(--border);
        }
        .badge-success {
            color: var(--success);
            border-color: rgba(16, 185, 129, 0.3);
            background: var(--success-muted);
        }
        .badge-warning {
            color: var(--warning);
            border-color: rgba(245, 158, 11, 0.3);
            background: var(--warning-muted);
        }

        /* SSO User Profile Pill in Masthead */
        .sso-profile {
            display: flex;
            align-items: center;
            gap: 10px;
            background: var(--surface-subtle);
            border: 1px solid var(--border);
            padding: 4px 12px;
            border-radius: var(--radius-sm);
        }

        .sso-avatar {
            width: 26px;
            height: 26px;
            border-radius: 50%;
            background: var(--primary-muted);
            color: var(--primary);
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .sso-user-name {
            font-weight: 600;
            font-size: 0.85rem;
            color: var(--text);
        }

        .sso-admin-tag {
            font-size: 0.68rem;
            font-weight: 700;
            padding: 1px 6px;
            border-radius: 4px;
            background: var(--success-muted);
            color: var(--success);
            border: 1px solid rgba(16, 185, 129, 0.4);
        }

        .sso-staff-tag {
            font-size: 0.68rem;
            font-weight: 700;
            padding: 1px 6px;
            border-radius: 4px;
            background: var(--primary-muted);
            color: var(--primary);
            border: 1px solid rgba(56, 189, 248, 0.4);
        }

        .btn-sso-logout {
            color: var(--danger);
            font-size: 0.78rem;
            text-decoration: none;
            font-weight: 600;
            padding: 4px 8px;
            border: 1px solid rgba(239, 68, 68, 0.3);
            border-radius: var(--radius-sm);
            transition: all 0.15s ease;
        }
        .btn-sso-logout:hover {
            background: var(--danger-muted);
            border-color: var(--danger);
        }

        .btn-sso-login {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            gap: 6px;
            background: var(--primary);
            color: #090d16;
            text-decoration: none;
            font-size: 0.85rem;
            font-weight: 600;
            padding: 8px 16px;
            border-radius: var(--radius-sm);
            transition: background 0.15s ease;
            min-height: 44px;
        }
        .btn-sso-login:hover {
            background: var(--primary-hover);
            color: #ffffff;
        }

        /* Cards & Section Architecture */
        .card {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: var(--radius-md);
            padding: 22px;
            margin-bottom: 20px;
            transition: border-color 0.15s ease;
        }

        .card:hover {
            border-color: var(--border-hover);
        }

        .card-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 12px;
            margin-bottom: 12px;
        }

        .card-header h2 {
            font-size: 1.15rem;
            font-weight: 600;
            color: var(--text);
            letter-spacing: -0.01em;
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .card-desc {
            color: var(--text-muted);
            font-size: 0.88rem;
            margin-bottom: 16px;
            line-height: 1.5;
        }

        /* Telemetry Key-Value Grid */
        .info-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
            gap: 12px;
            margin: 16px 0;
        }

        .info-item {
            background: var(--surface-subtle);
            border: 1px solid var(--border-muted);
            border-radius: var(--radius-sm);
            padding: 12px 14px;
        }

        .info-item .label {
            font-size: 0.76rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 4px;
            text-transform: uppercase;
            letter-spacing: 0.04em;
        }

        .info-item .val {
            font-size: 0.88rem;
            font-weight: 500;
            color: var(--text);
            word-break: break-all;
            font-family: var(--font-mono);
        }

        .helper-box {
            background: var(--surface-subtle);
            border: 1px solid var(--border-muted);
            padding: 14px 16px;
            border-radius: var(--radius-sm);
            margin-top: 14px;
        }

        code {
            background: rgba(255, 255, 255, 0.06);
            color: var(--primary);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.84rem;
        }

        /* Forms, Inputs & Action Buttons (R-03: min 44px tap targets) */
        .form-label {
            font-size: 0.84rem;
            color: var(--text-secondary);
            display: block;
            margin-bottom: 6px;
            font-weight: 500;
        }

        .form-input {
            width: 100%;
            min-height: 44px;
            background: var(--surface-subtle);
            border: 1px solid var(--border);
            border-radius: var(--radius-sm);
            color: var(--text);
            padding: 10px 14px;
            font-family: inherit;
            font-size: 0.9rem;
            margin-bottom: 12px;
            transition: border-color 0.15s ease;
        }

        .form-input:focus {
            outline: none;
            border-color: var(--primary);
        }

        select.form-input {
            min-height: 44px;
            cursor: pointer;
        }

        textarea.form-input {
            font-family: var(--font-mono);
            font-size: 0.84rem;
            resize: vertical;
            min-height: 80px;
        }

        .btn {
            min-height: 44px;
            display: inline-flex;
            align-items: center;
            justify-content: center;
            background: var(--primary);
            color: #090d16;
            border: none;
            padding: 10px 18px;
            border-radius: var(--radius-sm);
            font-weight: 600;
            font-size: 0.88rem;
            font-family: inherit;
            cursor: pointer;
            transition: background 0.15s ease;
        }
        .btn:hover {
            background: var(--primary-hover);
            color: #ffffff;
        }

        .btn-outline {
            min-height: 38px;
            background: transparent;
            border: 1px solid var(--border);
            color: var(--text-secondary);
            padding: 6px 14px;
            border-radius: var(--radius-sm);
            font-size: 0.82rem;
            cursor: pointer;
            transition: all 0.15s ease;
        }
        .btn-outline:hover {
            background: rgba(255, 255, 255, 0.05);
            border-color: var(--border-hover);
            color: var(--text);
        }

        .alert {
            padding: 12px 14px;
            border-radius: var(--radius-sm);
            margin-top: 14px;
            display: none;
            font-size: 0.85rem;
            line-height: 1.5;
        }
        .alert-success {
            background: var(--success-muted);
            border: 1px solid var(--success);
            color: var(--success);
        }
        .alert-error {
            background: var(--danger-muted);
            border: 1px solid var(--danger);
            color: var(--danger);
        }

        /* Modern Chat Simulator & Multi-Turn Thread */
        .chat-thread-container {
            min-height: 160px;
            max-height: 480px;
            overflow-y: auto;
            background: var(--surface-subtle);
            border: 1px solid var(--border);
            border-radius: var(--radius-md);
            padding: 16px;
            margin-bottom: 14px;
            display: flex;
            flex-direction: column;
            gap: 14px;
            scroll-behavior: smooth;
        }

        .chat-row-user {
            display: flex;
            justify-content: flex-end;
            width: 100%;
        }

        .chat-bubble-user {
            background: #1e3a8a;
            border: 1px solid #2563eb;
            color: #ffffff;
            padding: 10px 16px;
            border-radius: 12px 12px 2px 12px;
            max-width: 82%;
            font-size: 0.9rem;
            line-height: 1.5;
            word-break: break-word;
        }

        .chat-row-bot {
            display: flex;
            justify-content: flex-start;
            width: 100%;
        }

        .chat-bubble-bot {
            background: #111d2e;
            border: 1px solid var(--border-muted);
            padding: 14px 18px;
            border-radius: 12px 12px 12px 2px;
            max-width: 94%;
            width: 100%;
        }

        .chat-bubble-typing {
            background: rgba(255, 255, 255, 0.04);
            border: 1px dashed var(--primary);
            padding: 10px 16px;
            border-radius: var(--radius-sm);
            color: var(--primary);
            font-size: 0.85rem;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }

        .chat-bubble-system {
            align-self: center;
            background: rgba(255, 255, 255, 0.04);
            border: 1px solid rgba(255, 255, 255, 0.08);
            color: var(--text-muted);
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 0.78rem;
        }

        .markdown-body {
            font-size: 0.92rem;
            line-height: 1.65;
            color: var(--text);
        }
        .markdown-body p { margin-bottom: 12px; }
        .markdown-body p:last-child { margin-bottom: 0; }
        .markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 {
            font-weight: 700;
            color: #ffffff;
            margin-top: 16px;
            margin-bottom: 8px;
            letter-spacing: -0.01em;
        }
        .markdown-body h1 { font-size: 1.25rem; border-bottom: 1px solid var(--border); padding-bottom: 6px; }
        .markdown-body h2 { font-size: 1.1rem; border-bottom: 1px solid var(--border-muted); padding-bottom: 4px; }
        .markdown-body h3 { font-size: 1.0rem; }
        .markdown-body ul, .markdown-body ol {
            padding-left: 20px;
            margin-bottom: 12px;
        }
        .markdown-body li { margin-bottom: 4px; }
        .markdown-body hr {
            border: 0;
            border-top: 1px solid var(--border);
            margin: 16px 0;
        }

        /* Inline Code */
        .markdown-body :not(pre) > code {
            background: rgba(255, 255, 255, 0.08);
            color: #7dd3fc;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85em;
            border: 1px solid rgba(255, 255, 255, 0.06);
        }

        /* Code Block Action Bar */
        .code-block-wrapper {
            margin: 14px 0;
            border-radius: var(--radius-sm);
            overflow: hidden;
            border: 1px solid var(--border);
            background: #090d16;
        }
        .code-block-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            background: #111827;
            padding: 6px 14px;
            border-bottom: 1px solid var(--border);
            font-size: 0.78rem;
            color: var(--text-muted);
            font-family: var(--font-mono);
        }
        .copy-btn {
            background: transparent;
            border: 1px solid var(--border);
            color: var(--text-secondary);
            padding: 3px 10px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.72rem;
            font-family: var(--font-sans);
            transition: all 0.15s;
        }
        .copy-btn:hover {
            background: rgba(255, 255, 255, 0.08);
            color: #ffffff;
            border-color: var(--border-hover);
        }

        .markdown-body pre {
            margin: 0;
            padding: 14px 16px;
            overflow-x: auto;
            background: transparent !important;
        }
        .markdown-body pre code {
            font-family: var(--font-mono);
            font-size: 0.84rem;
            line-height: 1.55;
            background: transparent !important;
            padding: 0 !important;
        }

        /* Tables in Markdown */
        .markdown-body table {
            width: 100%;
            border-collapse: collapse;
            margin: 14px 0;
            font-size: 0.86rem;
            border-radius: var(--radius-sm);
            overflow: hidden;
        }
        .markdown-body th, .markdown-body td {
            border: 1px solid var(--border);
            padding: 8px 12px;
            text-align: left;
        }
        .markdown-body th {
            background: rgba(255, 255, 255, 0.05);
            font-weight: 600;
            color: #ffffff;
        }
        .markdown-body tr:nth-child(even) {
            background: rgba(255, 255, 255, 0.02);
        }

        /* GitHub-style Alerts / Callouts */
        .markdown-alert {
            padding: 12px 16px;
            margin: 14px 0;
            border-left: 3px solid;
            border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
            background: rgba(255, 255, 255, 0.02);
            font-size: 0.88rem;
        }
        .markdown-alert-title {
            font-weight: 700;
            margin-bottom: 4px;
            font-size: 0.78rem;
            letter-spacing: 0.04em;
        }
        .markdown-alert-note { border-color: var(--primary); }
        .markdown-alert-note .markdown-alert-title { color: var(--primary); }
        .markdown-alert-tip { border-color: var(--success); }
        .markdown-alert-tip .markdown-alert-title { color: var(--success); }
        .markdown-alert-important { border-color: #a855f7; }
        .markdown-alert-important .markdown-alert-title { color: #a855f7; }
        .markdown-alert-warning { border-color: var(--warning); }
        .markdown-alert-warning .markdown-alert-title { color: var(--warning); }
        .markdown-alert-caution { border-color: var(--danger); }
        .markdown-alert-caution .markdown-alert-title { color: var(--danger); }

        blockquote {
            border-left: 3px solid var(--primary);
            padding-left: 12px;
            margin: 12px 0;
            color: var(--text-muted);
            font-style: italic;
        }

        .admin-only-card {
            transition: all 0.2s ease;
        }

        @media (max-width: 640px) {
            body { padding: 16px 10px; }
            .masthead { flex-direction: column; align-items: flex-start; gap: 12px; }
            .masthead-actions { width: 100%; justify-content: space-between; }
            .info-grid { grid-template-columns: 1fr; }
            .chat-bubble-bot { max-width: 100%; }
            .chat-bubble-user { max-width: 90%; }
        }
    "#
}
