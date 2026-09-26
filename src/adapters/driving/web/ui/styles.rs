//! CSS styles and typography definitions for Aina Web Dashboard.

pub fn get_styles() -> &'static str {
    r#"
        :root {
            --bg: #0b0f19;
            --surface: #151d2e;
            --border: #2c3a52;
            --border-subtle: #1e293b;
            --text: #f1f5f9;
            --text-muted: #94a3b8;
            --primary: #38bdf8;
            --primary-hover: #0284c7;
            --success: #10b981;
            --warning: #f59e0b;
            --danger: #ef4444;
            --font-sans: 'Google Sans', 'Google Sans Text', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            background: var(--bg);
            color: var(--text);
            font-family: var(--font-sans);
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
            line-height: 1.65;
            padding: 40px 20px;
            display: flex;
            justify-content: center;
        }
        .container {
            width: 100%;
            max-width: 760px;
        }
        .header {
            text-align: center;
            margin-bottom: 30px;
        }
        .header h1 {
            font-size: 2.1rem;
            font-weight: 700;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 10px;
            letter-spacing: -0.02em;
        }
        .badge {
            display: inline-block;
            padding: 4px 14px;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 0.5px;
            margin-top: 10px;
            background: rgba(255,255,255,0.08);
            border: 1px solid var(--border);
        }
        .badge-success { color: var(--success); border-color: rgba(16,185,129,0.3); background: rgba(16,185,129,0.08); }
        .badge-warning { color: var(--warning); border-color: rgba(245,158,11,0.3); background: rgba(245,158,11,0.08); }
        .card {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 14px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 10px 25px -5px rgba(0,0,0,0.4);
        }
        .card-header {
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 12px;
        }
        .card-header h2 {
            font-size: 1.25rem;
            font-weight: 600;
            letter-spacing: -0.01em;
        }
        .pulse {
            width: 12px;
            height: 12px;
            border-radius: 50%;
            display: inline-block;
            animation: pulse-animation 2s infinite;
        }
        @keyframes pulse-animation {
            0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0.7); }
            70% { transform: scale(1); box-shadow: 0 0 0 8px rgba(56, 189, 248, 0); }
            100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0); }
        }
        .info-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 12px;
            margin: 18px 0;
        }
        .info-item {
            background: rgba(0,0,0,0.25);
            padding: 10px 14px;
            border-radius: 8px;
            border: 1px solid rgba(255,255,255,0.05);
        }
        .info-item .label {
            font-size: 0.75rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 2px;
        }
        .info-item .val {
            font-size: 0.9rem;
            font-weight: 600;
            word-break: break-all;
        }
        .helper-box {
            background: #090d16;
            border: 1px solid #1e293b;
            padding: 14px;
            border-radius: 8px;
            margin-top: 14px;
        }
        code {
            background: rgba(255,255,255,0.08);
            color: var(--primary);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85rem;
        }
        .form-label {
            font-size: 0.85rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 6px;
            font-weight: 500;
        }
        .form-input {
            width: 100%;
            background: #090d16;
            border: 1px solid var(--border);
            border-radius: 8px;
            color: #fff;
            padding: 10px 14px;
            font-family: inherit;
            font-size: 0.9rem;
            margin-bottom: 12px;
            transition: border-color 0.2s;
        }
        .form-input:focus {
            outline: none;
            border-color: var(--primary);
        }
        textarea.form-input {
            font-family: var(--font-mono);
            font-size: 0.82rem;
            resize: vertical;
        }
        .btn {
            background: var(--primary);
            color: #0b0f19;
            border: none;
            padding: 10px 20px;
            border-radius: 8px;
            font-weight: 600;
            font-family: inherit;
            cursor: pointer;
            transition: all 0.2s;
        }
        .btn:hover { background: var(--primary-hover); color: #fff; }
        .btn-outline {
            background: transparent;
            border: 1px solid var(--border);
            color: var(--text-muted);
        }
        .btn-outline:hover { background: rgba(255,255,255,0.05); color: var(--text); }
        .alert {
            padding: 12px;
            border-radius: 8px;
            margin-top: 14px;
            display: none;
            font-size: 0.85rem;
        }
        .alert-success { background: rgba(16,185,129,0.15); border: 1px solid var(--success); color: var(--success); }
        .alert-error { background: rgba(239,68,68,0.15); border: 1px solid var(--danger); color: var(--danger); }
        
        /* Modern Chat Bubble & Multi-Turn Thread Styling */
        .chat-thread-container {
            min-height: 140px;
            max-height: 480px;
            overflow-y: auto;
            background: #080c14;
            border: 1px solid var(--border);
            border-radius: 10px;
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
            border-radius: 14px 14px 2px 14px;
            max-width: 82%;
            font-size: 0.92rem;
            line-height: 1.5;
            word-break: break-word;
            box-shadow: 0 2px 8px rgba(0,0,0,0.25);
        }
        .chat-row-bot {
            display: flex;
            justify-content: flex-start;
            width: 100%;
        }
        .chat-bubble-bot {
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 14px 18px;
            border-radius: 14px 14px 14px 2px;
            max-width: 92%;
            width: 100%;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }
        .chat-bubble-typing {
            background: rgba(255,255,255,0.04);
            border: 1px dashed var(--primary);
            padding: 10px 16px;
            border-radius: 12px;
            color: var(--primary);
            font-size: 0.85rem;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }
        .chat-bubble-system {
            align-self: center;
            background: rgba(255,255,255,0.05);
            border: 1px solid rgba(255,255,255,0.1);
            color: var(--text-muted);
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 0.78rem;
        }
        .chat-bubble {
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 18px 20px;
            border-radius: 12px;
            border-bottom-left-radius: 2px;
            position: relative;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }
        .markdown-body {
            font-size: 0.94rem;
            line-height: 1.68;
            color: #f1f5f9;
        }
        .markdown-body p { margin-bottom: 12px; }
        .markdown-body p:last-child { margin-bottom: 0; }
        .markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 {
            font-weight: 700;
            color: #ffffff;
            margin-top: 18px;
            margin-bottom: 8px;
            letter-spacing: -0.01em;
        }
        .markdown-body h1 { font-size: 1.35rem; border-bottom: 1px solid var(--border); padding-bottom: 6px; }
        .markdown-body h2 { font-size: 1.18rem; border-bottom: 1px solid rgba(255,255,255,0.08); padding-bottom: 4px; }
        .markdown-body h3 { font-size: 1.05rem; }
        .markdown-body ul, .markdown-body ol {
            padding-left: 22px;
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
            background: rgba(255,255,255,0.08);
            color: #7dd3fc;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85em;
            border: 1px solid rgba(255,255,255,0.06);
        }

        /* Code Block Action Bar & Highlighting */
        .code-block-wrapper {
            margin: 14px 0;
            border-radius: 8px;
            overflow: hidden;
            border: 1px solid #334155;
            background: #0d1117;
        }
        .code-block-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            background: #161b22;
            padding: 6px 14px;
            border-bottom: 1px solid #30363d;
            font-size: 0.78rem;
            color: #8b949e;
            font-family: var(--font-mono);
            font-weight: 500;
        }
        .copy-btn {
            background: transparent;
            border: 1px solid #30363d;
            color: #c9d1d9;
            padding: 3px 10px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.72rem;
            font-family: var(--font-sans);
            transition: all 0.2s;
        }
        .copy-btn:hover {
            background: #21262d;
            color: #58a6ff;
            border-color: #58a6ff;
        }
        .markdown-body pre {
            margin: 0;
            padding: 14px 16px;
            overflow-x: auto;
            background: transparent !important;
        }
        .markdown-body pre code {
            font-family: var(--font-mono);
            font-size: 0.86rem;
            line-height: 1.55;
            background: transparent !important;
            padding: 0 !important;
        }

        /* Tables */
        .markdown-body table {
            width: 100%;
            border-collapse: collapse;
            margin: 14px 0;
            font-size: 0.88rem;
            border-radius: 6px;
            overflow: hidden;
        }
        .markdown-body th, .markdown-body td {
            border: 1px solid #334155;
            padding: 8px 12px;
            text-align: left;
        }
        .markdown-body th {
            background: rgba(255,255,255,0.06);
            font-weight: 600;
            color: #ffffff;
        }
        .markdown-body tr:nth-child(even) {
            background: rgba(255,255,255,0.02);
        }

        /* GitHub-style Alerts / Callouts */
        .markdown-alert {
            padding: 12px 16px;
            margin: 14px 0;
            border-left: 4px solid;
            border-radius: 0 8px 8px 0;
            background: rgba(255, 255, 255, 0.03);
            font-size: 0.9rem;
        }
        .markdown-alert-title {
            font-weight: 700;
            margin-bottom: 4px;
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 0.8rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        .markdown-alert-note { border-color: #38bdf8; }
        .markdown-alert-note .markdown-alert-title { color: #38bdf8; }
        .markdown-alert-tip { border-color: #10b981; }
        .markdown-alert-tip .markdown-alert-title { color: #10b981; }
        .markdown-alert-important { border-color: #a855f7; }
        .markdown-alert-important .markdown-alert-title { color: #a855f7; }
        .markdown-alert-warning { border-color: #f59e0b; }
        .markdown-alert-warning .markdown-alert-title { color: #f59e0b; }
        .markdown-alert-caution { border-color: #ef4444; }
        .markdown-alert-caution .markdown-alert-title { color: #ef4444; }

        blockquote {
            border-left: 3px solid #38bdf8;
            padding-left: 12px;
            margin: 12px 0;
            color: #94a3b8;
            font-style: italic;
        }
    "#
}
