use super::state::WebhookServerState;

pub fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn render_html(is_authenticated: bool, state: &WebhookServerState, current_model: &str) -> String {
    let (badge_class, badge_text, badge_bg) = if is_authenticated {
        ("badge-success", "ONLINE & TERAUTENTIKASI", "#10b981")
    } else {
        ("badge-warning", "PERLU SETUP AUTENTIKASI", "#f59e0b")
    };

    let companion_info = if let Some(c_jid) = &state.companion_jid {
        let c_name = state.companion_name.as_deref().unwrap_or("Personal Account");
        format!("{} ({}) <span style=\"color: #10b981; font-weight: 600;\">[Aktif]</span>", c_name, c_jid)
    } else {
        "<span style=\"color: #94a3b8;\">Belum Dikonfigurasi (Opsional)</span>".to_string()
    };

    let status_card = if is_authenticated {
        format!(
            r#"
            <div class="card status-card">
                <div class="card-header">
                    <span class="pulse" style="background: {badge_bg}"></span>
                    <h2>Aina Siap Digunakan!</h2>
                </div>
                <p>Mesin agentik Google Antigravity telah terhubung dan aktif melayani pesan WhatsApp.</p>
                <div class="info-grid">
                    <div class="info-item"><span class="label">Nama Bot (Primary)</span><span class="val">{name}</span></div>
                    <div class="info-item"><span class="label">WhatsApp JID Bot</span><span class="val">{jid}</span></div>
                    <div class="info-item"><span class="label">Companion (Shadow Sensor)</span><span class="val">{companion_info}</span></div>
                    <div class="info-item">
                        <span class="label">Model AI Aktif</span>
                        <div style="display: flex; justify-content: space-between; align-items: baseline;">
                            <span class="val" id="active-model-display">{model}</span>
                            <a href="javascript:void(0)" onclick="openModelModal()" style="font-size: 0.75rem; color: var(--primary); text-decoration: none; font-weight: 600;">⚡ Ganti</a>
                        </div>
                    </div>
                    <div class="info-item"><span class="label">Whatsmeow</span><span class="val">{url}</span></div>
                    <div class="info-item"><span class="label">Zona Waktu / Locale</span><span class="val">{timezone} ({locale})</span></div>
                </div>
                <div class="helper-box">
                    <strong>Webhook Endpoint:</strong>
                    <code>POST /webhook</code>
                    <p style="margin-top: 6px; font-size: 0.85rem; color: #94a3b8;">Arahkan webhook dari instance Whatsmeow ke URL ini.</p>
                </div>
            </div>

            <!-- MULTI-ACCOUNT POOL CARD -->
            <div class="card" style="border-color: #3b82f6;">
                <div class="card-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div style="display: flex; align-items: center; gap: 8px;">
                        <span style="font-size: 1.2rem;">👥</span>
                        <h2>Pool Akun Antigravity (Multi-Account)</h2>
                    </div>
                    <button type="button" class="btn-outline" style="font-size: 0.8rem; padding: 4px 10px; cursor: pointer;" onclick="toggleAddAccountForm()">
                        + Tambah Akun Cadangan
                    </button>
                </div>
                <p style="color: var(--text-muted); font-size: 0.88rem; margin-bottom: 12px;">
                    Daftar akun Google Antigravity yang aktif untuk rotasi otomatis (Round-Robin) saat kuota harian akun habis.
                </p>
                <div id="account-pool-badges" style="display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 12px;">
                    <span style="color: #94a3b8; font-size: 0.85rem;">Memuat status pool akun...</span>
                </div>

                <!-- FORM TAMBAH AKUN CADANGAN (EXPANDABLE) -->
                <div id="add-account-form" style="display: none; background: rgba(15, 23, 42, 0.7); border: 1px solid rgba(59, 130, 246, 0.3); border-radius: 8px; padding: 16px; margin-top: 12px;">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                        <h4 style="margin: 0; font-size: 0.95rem; color: #60a5fa;">⚡ Tambah Akun Google via OAuth Cepat</h4>
                        <button type="button" class="btn-outline" style="font-size: 0.78rem; padding: 2px 8px; cursor: pointer;" onclick="toggleAddAccountForm()">Tutup</button>
                    </div>

                    <div style="margin-bottom: 10px;">
                        <label class="form-label" style="font-size: 0.85rem;">Admin Key / WHATSMEOW_API_KEY:</label>
                        <input id="add-token-key" class="form-input" type="password" placeholder="Masukkan WHATSMEOW_API_KEY atau ADMIN_KEY Anda..." style="font-size: 0.85rem; margin-bottom: 4px;" />
                    </div>

                    <!-- METODE 1: OAUTH CEPAT -->
                    <div id="oauth-step-1" style="background: rgba(255,255,255,0.03); border: 1px dashed rgba(255,255,255,0.15); border-radius: 6px; padding: 12px; margin-bottom: 12px;">
                        <p style="font-size: 0.83rem; color: #cbd5e1; margin-bottom: 8px;">
                            Hubungkan akun Google langsung tanpa perlu menyalin file JSON. Klik tombol di bawah untuk meminta tautan login resmi dari Google:
                        </p>
                        <button type="button" id="start-oauth-btn" class="btn" style="background: #2563eb; font-size: 0.82rem; padding: 6px 14px; cursor: pointer;" onclick="startOAuthFlow()">
                            🚀 Mulai Login Akun Google
                        </button>
                    </div>

                    <div id="oauth-step-2" style="display: none; background: rgba(37, 99, 235, 0.08); border: 1px solid rgba(59, 130, 246, 0.4); border-radius: 6px; padding: 14px; margin-bottom: 12px;">
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                            <span style="font-size: 0.84rem; font-weight: 600; color: #93c5fd;">Langkah Selanjutnya:</span>
                            <span id="oauth-timer-badge" style="font-size: 0.78rem; color: #fbbf24; background: rgba(245, 158, 11, 0.15); border: 1px solid rgba(245, 158, 11, 0.3); border-radius: 4px; padding: 2px 8px;">
                                ⏱️ Sisa Waktu: <strong id="oauth-countdown">60</strong>s
                            </span>
                        </div>
                        <p style="font-size: 0.83rem; color: #cbd5e1; margin-bottom: 10px; line-height: 1.5;">
                            1. Klik tombol biru di bawah untuk membuka halaman login Google.<br>
                            2. Pilih salah satu dari 11 akun Google Anda dan klik <strong>Izinkan (Allow)</strong>.<br>
                            3. Pada halaman Google yang terbuka, klik tombol <strong>'Copy to Clipboard'</strong> di tengah layar (jangan menyalin teks URL dari bilah alamat browser), lalu tempel kodenya di bawah:
                        </p>
                        <div style="margin-bottom: 10px;">
                            <a id="oauth-link-anchor" href="javascript:void(0)" target="_blank" class="btn" style="background: #3b82f6; color: #fff; text-decoration: none; display: inline-block; padding: 6px 14px; font-size: 0.82rem; border-radius: 4px;">
                                🔗 Buka Halaman Login Google ↗
                            </a>
                        </div>
                        <label class="form-label" style="font-size: 0.82rem;">Tempel Kode Otorisasi Google di sini:</label>
                        <div style="display: flex; gap: 8px;">
                            <input id="oauth-code-input" class="form-input" type="text" placeholder="Tempel kode 4/0A... di sini" style="flex: 1; font-size: 0.82rem; margin-bottom: 0;" />
                            <button type="button" id="submit-oauth-code-btn" class="btn" style="background: #10b981; font-size: 0.82rem; padding: 6px 14px; cursor: pointer; white-space: nowrap;" onclick="submitOAuthCode()">
                                ✅ Simpan &amp; Verifikasi
                            </button>
                        </div>
                    </div>

                    <div id="oauth-alert" style="display: none; font-size: 0.85rem; margin-top: 8px; margin-bottom: 10px;"></div>

                    <!-- METODE 2: MANUAL JSON ACCORDION -->
                    <div style="border-top: 1px dashed rgba(255,255,255,0.1); padding-top: 10px; margin-top: 10px;">
                        <a href="javascript:void(0)" onclick="toggleManualJsonInput()" style="color: #94a3b8; font-size: 0.78rem; text-decoration: none;">
                            ▸ Atau tempel manual file JSON token (cadangan/opsional)
                        </a>
                        <div id="manual-json-container" style="display: none; margin-top: 8px;">
                            <textarea id="add-token-input" class="form-input" style="height: 60px; font-family: monospace; font-size: 0.78rem;" placeholder='{{"token": "..."}}'></textarea>
                            <div style="display: flex; justify-content: flex-end; margin-top: 4px;">
                                <button type="button" id="add-token-btn" class="btn" style="font-size: 0.78rem; padding: 4px 12px; cursor: pointer;" onclick="submitAddAccount()">Simpan Token Manual</button>
                            </div>
                            <div id="add-token-alert" style="display: none; font-size: 0.85rem; margin-top: 8px;"></div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- ADMIN LOCK CARD -->
            <div id="sim-lock-card" class="card" style="border-color: #f59e0b; display: none;">
                <div class="card-header">
                    <span style="font-size: 1.2rem;">🔒</span>
                    <h2>Akses Simulator Terproteksi</h2>
                </div>
                <p style="color: var(--text-muted); font-size: 0.9rem; margin-bottom: 14px;">
                    Untuk mencegah orang luar mengeksekusi agen AI di server Anda, fitur simulator dilindungi. Masukkan <strong>Setup Code / Admin Key</strong> server Anda untuk membuka sesi.
                </p>
                <div style="display: flex; gap: 8px;">
                    <input id="admin-passcode-input" class="form-input" type="password" placeholder="Masukkan Admin Key / Setup Code..." style="margin-bottom: 0;" />
                    <button id="unlock-btn" class="btn" style="white-space: nowrap;" onclick="unlockAdminSession()">Buka Kunci</button>
                </div>
                <div id="unlock-error" style="display: none; color: #ef4444; font-size: 0.85rem; margin-top: 8px;"></div>
            </div>

            <!-- SIMULATOR CHAT REAL END-TO-END -->
            <div id="simulator-card" class="card simulator-card">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; padding: 6px 12px; background: rgba(16,185,129,0.1); border: 1px solid rgba(16,185,129,0.2); border-radius: 6px; font-size: 0.8rem; color: #10b981;">
                    <span>🛡️ Sesi Admin Terverifikasi</span>
                    <a href="javascript:void(0)" onclick="lockAdminSession()" style="color: #f87171; text-decoration: none; font-weight: 600;">Kunci Dashboard</a>
                </div>
                <div class="card-header">
                    <span style="font-size: 1.2rem;">🧪</span>
                    <h2>Simulator Percakapan WhatsApp (Real Test)</h2>
                </div>
                <p style="color: var(--text-muted); font-size: 0.9rem; margin-bottom: 16px;">
                    Uji langsung logika respons, dual-session routing (Bot Utama vs Companion Sensor), etika grup (Gatekeeper), dan eksekusi agentik Antigravity secara nyata tanpa harus mengirim chat dari HP Anda.
                </p>

                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Sesi WhatsApp (Pipeline Routing)</label>
                        <select id="sim-session-role" class="form-input" onchange="onSessionRoleChange(this.value)">
                            <option value="primary_bot">🤖 Sesi Bot Utama (Primary Dedicated)</option>
                            <option value="user_companion">👥 Sesi Companion (Akun Pribadi / Shadow Sensor)</option>
                        </select>
                    </div>
                    <div>
                        <label class="form-label">Tipe Obrolan</label>
                        <select id="sim-chat-type" class="form-input" onchange="onChatTypeChange(this.value)">
                            <option value="dm">Pesan Pribadi (DM)</option>
                            <option value="group">Grup WhatsApp Kantor</option>
                        </select>
                    </div>
                </div>

                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Nama Pengirim</label>
                        <input id="sim-sender-name" class="form-input" value="Owner" />
                    </div>
                    <div style="display: flex; align-items: center; padding-top: 20px;">
                        <label style="font-size: 0.85rem; color: #94a3b8; display: flex; align-items: center; gap: 8px; cursor: pointer;">
                            <input type="checkbox" id="sim-is-from-me" onchange="onIsFromMeChange(this.checked)" />
                            <span>Kirim Sebagai Diri Sendiri (is_from_me / Owner)</span>
                        </label>
                    </div>
                </div>

                <div style="margin-bottom: 12px;">
                    <label class="form-label">Pilihan Model AI (Sesi Simulasi Ini)</label>
                    <select id="sim-model-select" class="form-input">
                        <option value="default">Mengikuti Model Aktif Default ({model})</option>
                        <option value="gemini-3.8-flash-medium">Gemini 3.8 Flash (Medium) - Default Cepat &amp; Seimbang (5-15s)</option>
                        <option value="gemini-3.8-flash-high">Gemini 3.8 Flash (High) - Penalaran Tinggi / Deep Thinking</option>
                        <option value="gemini-3.8-flash-low">Gemini 3.8 Flash (Low) - Respons Kilat &amp; Kasual (&lt;3s)</option>
                        <option value="gemini-3.1-pro-high">Gemini 3.1 Pro (High) - Deep Coding &amp; Arsitektur Sistem</option>
                        <option value="claude-opus-4-6-thinking">Claude Opus 4.6 (Thinking) - Khusus Tugas Sangat Kompleks (Eksplisit)</option>
                        <option value="claude-sonnet-4-6">Claude Sonnet 4.6 (Thinking)</option>
                    </select>
                </div>

                <div id="mention-toggle-wrapper" style="display: none; margin-bottom: 12px;">
                    <label style="font-size: 0.85rem; color: #94a3b8; display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" id="sim-is-mention" />
                        <span>Simulasikan Tag / Mention (@Aina) dalam grup</span>
                    </label>
                </div>

                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 14px; margin-bottom: 8px;">
                    <label class="form-label" style="margin-bottom: 0; font-weight: 600;">💬 Alur Percakapan Interaktif (Multi-Turn)</label>
                    <button type="button" class="btn-outline" style="font-size: 0.78rem; padding: 4px 10px; border-radius: 6px; cursor: pointer;" onclick="resetSimulationChat()">
                        🔄 Reset Percakapan Baru
                    </button>
                </div>

                <!-- SCROLLABLE CHAT THREAD -->
                <div id="sim-chat-thread" class="chat-thread-container">
                    <div id="sim-thread-empty" class="chat-bubble-system">
                        Belum ada pesan. Mulai obrolan dengan Aina di bawah. Anda bisa membalas chat secara berkelanjutan layaknya di WhatsApp!
                    </div>
                </div>

                <label class="form-label">Ketik Pesan Chat (Tekan Enter untuk kirim, Shift+Enter untuk baris baru):</label>
                <div style="display: flex; gap: 8px; align-items: flex-start;">
                    <textarea id="sim-text" class="form-input" style="height: 60px; margin-bottom: 0; resize: none;" placeholder="Ketik pesan atau balasan Anda ke Aina (contoh: '!aina rangkum diskusi tadi' atau 'Sudah ku-authorize ya')..."></textarea>
                    <button id="sim-btn" class="btn" style="min-width: 130px; height: 60px; display: flex; align-items: center; justify-content: center; gap: 6px; font-weight: 600;" onclick="runSimulation()">
                        <span>Kirim</span> 🚀
                    </button>
                </div>
            </div>
            "#,
            badge_bg = badge_bg,
            name = state.bot_name,
            jid = state.bot_jid,
            companion_info = companion_info,
            model = current_model,
            url = state.whatsmeow_url,
            timezone = state.timezone,
            locale = state.locale,
        )
    } else {
        r#"
        <div class="card warning-card">
            <div class="card-header">
                <span class="pulse" style="background: #f59e0b"></span>
                <h2>Setup Autentikasi Google Antigravity</h2>
            </div>
            <p>Aina memerlukan OAuth Token untuk mengakses Google Antigravity CLI di server ini.</p>
        </div>
        "#.to_string()
    };

    let auth_form_display = if is_authenticated { "display: none;" } else { "display: block;" };

    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Aina (あいな) - Assistant Dashboard</title>
    
    <!-- Google Fonts: Google Sans & Google Sans Text (Gemini standard), JetBrains Mono -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Google+Sans:wght@400;500;700&family=Google+Sans+Text:ital,wght@0,400;0,500;0,700;1,400&family=JetBrains+Mono:ital,wght@0,400;0,500;0,600;1,400&display=swap" rel="stylesheet">
    
    <!-- Markdown Parser & Highlight.js CDN -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github-dark-dimmed.min.css">
    <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
    <script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/dompurify@3.0.9/dist/purify.min.js"></script>

    <style>
        :root {{
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
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background: var(--bg);
            color: var(--text);
            font-family: var(--font-sans);
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
            line-height: 1.65;
            padding: 40px 20px;
            display: flex;
            justify-content: center;
        }}
        .container {{
            width: 100%;
            max-width: 760px;
        }}
        .header {{
            text-align: center;
            margin-bottom: 30px;
        }}
        .header h1 {{
            font-size: 2.1rem;
            font-weight: 700;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 10px;
            letter-spacing: -0.02em;
        }}
        .badge {{
            display: inline-block;
            padding: 4px 14px;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 0.5px;
            margin-top: 10px;
            background: rgba(255,255,255,0.08);
            border: 1px solid var(--border);
        }}
        .badge-success {{ color: var(--success); border-color: rgba(16,185,129,0.3); background: rgba(16,185,129,0.08); }}
        .badge-warning {{ color: var(--warning); border-color: rgba(245,158,11,0.3); background: rgba(245,158,11,0.08); }}
        .card {{
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 14px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 10px 25px -5px rgba(0,0,0,0.4);
        }}
        .card-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 12px;
        }}
        .card-header h2 {{
            font-size: 1.25rem;
            font-weight: 600;
            letter-spacing: -0.01em;
        }}
        .pulse {{
            width: 12px;
            height: 12px;
            border-radius: 50%;
            display: inline-block;
            animation: pulse-animation 2s infinite;
        }}
        @keyframes pulse-animation {{
            0% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0.7); }}
            70% {{ transform: scale(1); box-shadow: 0 0 0 8px rgba(56, 189, 248, 0); }}
            100% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0); }}
        }}
        .info-grid {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 12px;
            margin: 18px 0;
        }}
        .info-item {{
            background: rgba(0,0,0,0.25);
            padding: 10px 14px;
            border-radius: 8px;
            border: 1px solid rgba(255,255,255,0.05);
        }}
        .info-item .label {{
            font-size: 0.75rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 2px;
        }}
        .info-item .val {{
            font-size: 0.9rem;
            font-weight: 600;
            word-break: break-all;
        }}
        .helper-box {{
            background: #090d16;
            border: 1px solid #1e293b;
            padding: 14px;
            border-radius: 8px;
            margin-top: 14px;
        }}
        code {{
            background: rgba(255,255,255,0.08);
            color: var(--primary);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85rem;
        }}
        .form-label {{
            font-size: 0.85rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 6px;
            font-weight: 500;
        }}
        .form-input {{
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
        }}
        .form-input:focus {{
            outline: none;
            border-color: var(--primary);
        }}
        textarea.form-input {{
            font-family: var(--font-mono);
            font-size: 0.82rem;
            resize: vertical;
        }}
        .btn {{
            background: var(--primary);
            color: #0b0f19;
            border: none;
            padding: 10px 20px;
            border-radius: 8px;
            font-weight: 600;
            font-family: inherit;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .btn:hover {{ background: var(--primary-hover); color: #fff; }}
        .btn-outline {{
            background: transparent;
            border: 1px solid var(--border);
            color: var(--text-muted);
        }}
        .btn-outline:hover {{ background: rgba(255,255,255,0.05); color: var(--text); }}
        .alert {{
            padding: 12px;
            border-radius: 8px;
            margin-top: 14px;
            display: none;
            font-size: 0.85rem;
        }}
        .alert-success {{ background: rgba(16,185,129,0.15); border: 1px solid var(--success); color: var(--success); }}
        .alert-error {{ background: rgba(239,68,68,0.15); border: 1px solid var(--danger); color: var(--danger); }}
        
        /* Modern Chat Bubble & Multi-Turn Thread Styling */
        .chat-thread-container {{
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
        }}
        .chat-row-user {{
            display: flex;
            justify-content: flex-end;
            width: 100%;
        }}
        .chat-bubble-user {{
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
        }}
        .chat-row-bot {{
            display: flex;
            justify-content: flex-start;
            width: 100%;
        }}
        .chat-bubble-bot {{
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 14px 18px;
            border-radius: 14px 14px 14px 2px;
            max-width: 92%;
            width: 100%;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }}
        .chat-bubble-typing {{
            background: rgba(255,255,255,0.04);
            border: 1px dashed var(--primary);
            padding: 10px 16px;
            border-radius: 12px;
            color: var(--primary);
            font-size: 0.85rem;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }}
        .chat-bubble-system {{
            align-self: center;
            background: rgba(255,255,255,0.05);
            border: 1px solid rgba(255,255,255,0.1);
            color: var(--text-muted);
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 0.78rem;
        }}
        .chat-bubble {{
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 18px 20px;
            border-radius: 12px;
            border-bottom-left-radius: 2px;
            position: relative;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }}
        .markdown-body {{
            font-size: 0.94rem;
            line-height: 1.68;
            color: #f1f5f9;
        }}
        .markdown-body p {{ margin-bottom: 12px; }}
        .markdown-body p:last-child {{ margin-bottom: 0; }}
        .markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 {{
            font-weight: 700;
            color: #ffffff;
            margin-top: 18px;
            margin-bottom: 8px;
            letter-spacing: -0.01em;
        }}
        .markdown-body h1 {{ font-size: 1.35rem; border-bottom: 1px solid var(--border); padding-bottom: 6px; }}
        .markdown-body h2 {{ font-size: 1.18rem; border-bottom: 1px solid rgba(255,255,255,0.08); padding-bottom: 4px; }}
        .markdown-body h3 {{ font-size: 1.05rem; }}
        .markdown-body ul, .markdown-body ol {{
            padding-left: 22px;
            margin-bottom: 12px;
        }}
        .markdown-body li {{ margin-bottom: 4px; }}
        .markdown-body hr {{
            border: 0;
            border-top: 1px solid var(--border);
            margin: 16px 0;
        }}
        
        /* Inline Code */
        .markdown-body :not(pre) > code {{
            background: rgba(255,255,255,0.08);
            color: #7dd3fc;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85em;
            border: 1px solid rgba(255,255,255,0.06);
        }}

        /* Code Block Action Bar & Highlighting */
        .code-block-wrapper {{
            margin: 14px 0;
            border-radius: 8px;
            overflow: hidden;
            border: 1px solid #334155;
            background: #0d1117;
        }}
        .code-block-header {{
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
        }}
        .copy-btn {{
            background: transparent;
            border: 1px solid #30363d;
            color: #c9d1d9;
            padding: 3px 10px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.72rem;
            font-family: var(--font-sans);
            transition: all 0.2s;
        }}
        .copy-btn:hover {{
            background: #21262d;
            color: #58a6ff;
            border-color: #58a6ff;
        }}
        .markdown-body pre {{
            margin: 0;
            padding: 14px 16px;
            overflow-x: auto;
            background: transparent !important;
        }}
        .markdown-body pre code {{
            font-family: var(--font-mono);
            font-size: 0.86rem;
            line-height: 1.55;
            background: transparent !important;
            padding: 0 !important;
        }}

        /* Tables */
        .markdown-body table {{
            width: 100%;
            border-collapse: collapse;
            margin: 14px 0;
            font-size: 0.88rem;
            border-radius: 6px;
            overflow: hidden;
        }}
        .markdown-body th, .markdown-body td {{
            border: 1px solid #334155;
            padding: 8px 12px;
            text-align: left;
        }}
        .markdown-body th {{
            background: rgba(255,255,255,0.06);
            font-weight: 600;
            color: #ffffff;
        }}
        .markdown-body tr:nth-child(even) {{
            background: rgba(255,255,255,0.02);
        }}

        /* GitHub-style Alerts / Callouts */
        .markdown-alert {{
            padding: 12px 16px;
            margin: 14px 0;
            border-left: 4px solid;
            border-radius: 0 8px 8px 0;
            background: rgba(255, 255, 255, 0.03);
            font-size: 0.9rem;
        }}
        .markdown-alert-title {{
            font-weight: 700;
            margin-bottom: 4px;
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 0.8rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}
        .markdown-alert-note {{ border-color: #38bdf8; }}
        .markdown-alert-note .markdown-alert-title {{ color: #38bdf8; }}
        .markdown-alert-tip {{ border-color: #10b981; }}
        .markdown-alert-tip .markdown-alert-title {{ color: #10b981; }}
        .markdown-alert-important {{ border-color: #a855f7; }}
        .markdown-alert-important .markdown-alert-title {{ color: #a855f7; }}
        .markdown-alert-warning {{ border-color: #f59e0b; }}
        .markdown-alert-warning .markdown-alert-title {{ color: #f59e0b; }}
        .markdown-alert-caution {{ border-color: #ef4444; }}
        .markdown-alert-caution .markdown-alert-title {{ color: #ef4444; }}

        blockquote {{
            border-left: 3px solid #38bdf8;
            padding-left: 12px;
            margin: 12px 0;
            color: #94a3b8;
            font-style: italic;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌸 Aina (あいな)</h1>
            <span class="badge {badge_class}">{badge_text}</span>
        </div>

        {status_card}

        <div id="auth-form-container" class="card" style="{auth_form_display}">
            <h3 style="margin-bottom: 10px;">🔐 Setup Autentikasi Pertama Kali</h3>
            <p style="color: var(--text-muted); font-size: 0.88rem; margin-bottom: 12px;">
                Untuk mencegah akses tidak sah pada server publik, Anda memerlukan <strong>Kode Setup</strong> yang tercetak pada log deployment server.
            </p>

            <div class="helper-box" style="margin-bottom: 14px;">
                <p style="font-size: 0.82rem; color: #94a3b8; margin-bottom: 4px;">1. Ambil token dari terminal laptop lokal Anda:</p>
                <code>cat ~/.gemini/antigravity-cli/antigravity-oauth-token</code>
            </div>

            <label class="form-label">Kode Setup / Admin Key (Lihat di log terminal/Coolify):</label>
            <input id="setup-code-input" class="form-input" placeholder="AINA-XXXXXX atau ADMIN_KEY Anda" />

            <label class="form-label">Tempelkan seluruh JSON token di bawah:</label>
            <textarea id="token-input" class="form-input" style="height: 120px;" placeholder='{{"auth_method":"oauth","id_token":"...","token":{{...}}}}'></textarea>
            
            <button id="submit-btn" class="btn" onclick="submitToken()">Verifikasi & Simpan Token</button>
            <div id="alert-box" class="alert"></div>
        </div>
    </div>

    <script>
        let lastSimulationResponseText = '';

        function checkAdminAuth() {{
            const key = localStorage.getItem('aina_admin_key');
            const simCard = document.getElementById('simulator-card');
            const lockCard = document.getElementById('sim-lock-card');
            if (!simCard || !lockCard) return;

            if (key) {{
                simCard.style.display = 'block';
                lockCard.style.display = 'none';
            }} else {{
                simCard.style.display = 'none';
                lockCard.style.display = 'block';
            }}
            loadAccountPool();
        }}

        async function loadAccountPool() {{
            const listEl = document.getElementById('account-pool-badges');
            if (!listEl) return;
            const key = localStorage.getItem('aina_admin_key') || '';
            try {{
                const res = await fetch('/api/auth/accounts?key=' + encodeURIComponent(key));
                const data = await res.json();
                if (res.ok && data.success && data.accounts) {{
                    if (data.accounts.length === 0) {{
                        listEl.innerHTML = '<span style="color: #94a3b8; font-size: 0.85rem;">Belum ada akun di pool (menggunakan token file default).</span>';
                    }} else {{
                        listEl.innerHTML = data.accounts.map(a => {{
                            const badgeColor = a.is_cooldown ? '#f59e0b' : '#10b981';
                            const statusText = a.is_cooldown ? ('Cooldown (' + a.cooldown_remaining_secs + 's)') : '🟢 Aktif';
                            const emailText = a.email ? (' <span style="color: #94a3b8; font-size: 0.78rem;">(' + a.email + ')</span>') : '';
                            const deleteBtn = (a.id > 1 || data.accounts.length > 1) ? 
                                ('<button type="button" onclick="deleteAccount(' + a.id + ')" title="Hapus Akun" style="background: none; border: none; color: #ef4444; cursor: pointer; padding: 0 4px; font-size: 0.85rem; line-height: 1;">✕</button>') : '';
                            return '<div style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); border-radius: 6px; padding: 6px 12px; font-size: 0.85rem; display: flex; align-items: center; gap: 8px;">' +
                                '<span style="font-weight: 600; color: #f8fafc;">#' + a.id + ' ' + a.label + '</span>' +
                                emailText +
                                '<span style="color: ' + badgeColor + '; font-size: 0.8rem;">' + statusText + '</span>' +
                                deleteBtn +
                            '</div>';
                        }}).join('');
                    }}
                }}
            }} catch(e) {{
                console.warn('Failed to load accounts:', e);
            }}
        }}

        let currentOAuthSessionId = null;
        let oauthCountdownInterval = null;

        function sanitizeOAuthCode(val) {{
            let s = (val || '').trim();
            try {{
                s = decodeURIComponent(s);
            }} catch(e) {{}}
            if (s.indexOf('code=') !== -1) {{
                s = s.split('code=')[1];
            }}
            const delimiters = ['&', '+http', ' http', 'userinfo.', 'rinfo.', '.profile', '+', ' '];
            for (let i = 0; i < delimiters.length; i++) {{
                const d = delimiters[i];
                if (s.indexOf(d) !== -1) {{
                    s = s.split(d)[0];
                }}
            }}
            return s.trim();
        }}

        function toggleManualJsonInput() {{
            const el = document.getElementById('manual-json-container');
            if (el) {{
                el.style.display = el.style.display === 'none' ? 'block' : 'none';
            }}
        }}

        async function startOAuthFlow() {{
            const keyInput = document.getElementById('add-token-key');
            const alertEl = document.getElementById('oauth-alert');
            const startBtn = document.getElementById('start-oauth-btn');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            if (!key) {{
                alertEl.innerText = 'Harap masukkan Admin Key atau WHATSMEOW_API_KEY terlebih dahulu di kolom atas.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }}

            startBtn.disabled = true;
            startBtn.innerText = 'Menyiapkan URL Google...';
            alertEl.style.display = 'none';

            try {{
                const res = await fetch('/api/auth/oauth/init?key=' + encodeURIComponent(key), {{
                    method: 'POST'
                }});
                const data = await res.json();
                if (res.ok && data.success && data.auth_url) {{
                    currentOAuthSessionId = data.session_id;
                    const anchor = document.getElementById('oauth-link-anchor');
                    if (anchor) anchor.href = data.auth_url;

                    document.getElementById('oauth-step-2').style.display = 'block';
                    document.getElementById('oauth-code-input').value = '';

                    // 60-second countdown for Google CLI
                    if (oauthCountdownInterval) clearInterval(oauthCountdownInterval);
                    let remaining = 60;
                    const countdownEl = document.getElementById('oauth-countdown');
                    const badgeEl = document.getElementById('oauth-timer-badge');
                    const submitBtn = document.getElementById('submit-oauth-code-btn');
                    if (countdownEl) countdownEl.innerText = remaining;
                    if (badgeEl) {{
                        badgeEl.style.color = '#fbbf24';
                        badgeEl.style.background = 'rgba(245, 158, 11, 0.15)';
                        badgeEl.innerHTML = '⏱️ Sisa Waktu: <strong id="oauth-countdown">' + remaining + '</strong>s';
                    }}
                    if (submitBtn) submitBtn.disabled = false;

                    oauthCountdownInterval = setInterval(() => {{
                        remaining--;
                        const cEl = document.getElementById('oauth-countdown');
                        if (cEl) cEl.innerText = remaining;
                        if (remaining <= 0) {{
                            clearInterval(oauthCountdownInterval);
                            oauthCountdownInterval = null;
                            if (badgeEl) {{
                                badgeEl.style.color = '#ef4444';
                                badgeEl.style.background = 'rgba(239, 68, 68, 0.15)';
                                badgeEl.innerHTML = '⚠️ Sesi Kadaluarsa (60s)';
                            }}
                            if (submitBtn) submitBtn.disabled = true;
                            alertEl.innerHTML = '⚠️ <strong>Sesi login Google telah melewati batas waktu 60 detik.</strong> Silakan klik tombol <strong>"🚀 Mulai Ulang / Akun Lain"</strong> untuk meminta tautan baru.';
                            alertEl.style.color = '#ef4444';
                            alertEl.style.display = 'block';
                        }}
                    }}, 1000);

                    window.open(data.auth_url, '_blank');

                    alertEl.innerHTML = 'ℹ️ Tab login Google telah dibuka. Pilih salah satu akun Anda, klik <strong>Izinkan</strong>, lalu salin kode yang muncul ke kotak di atas.';
                    alertEl.style.color = '#38bdf8';
                    alertEl.style.display = 'block';
                }} else {{
                    alertEl.innerText = '❌ ' + (data.error || 'Gagal memulai sesi login.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }}
            }} catch(e) {{
                alertEl.innerText = '❌ Error: ' + e;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            }} finally {{
                startBtn.disabled = false;
                startBtn.innerText = '🚀 Mulai Ulang / Akun Lain';
            }}
        }}

        async function submitOAuthCode() {{
            const codeInput = document.getElementById('oauth-code-input');
            const keyInput = document.getElementById('add-token-key');
            const alertEl = document.getElementById('oauth-alert');
            const submitBtn = document.getElementById('submit-oauth-code-btn');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            const rawCode = codeInput.value.trim();
            const code = sanitizeOAuthCode(rawCode);
            codeInput.value = code;

            if (!code) {{
                alertEl.innerText = 'Harap tempelkan kode otorisasi dari Google terlebih dahulu.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }}

            if (!currentOAuthSessionId) {{
                alertEl.innerText = 'Sesi login belum dimulai. Klik "Mulai Login Akun Google" terlebih dahulu.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }}

            submitBtn.disabled = true;
            submitBtn.innerText = 'Memverifikasi ke Google...';
            alertEl.style.display = 'none';

            try {{
                const res = await fetch('/api/auth/oauth/exchange?key=' + encodeURIComponent(key), {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{
                        session_id: currentOAuthSessionId,
                        code: code,
                        setup_code: key
                    }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    if (oauthCountdownInterval) {{
                        clearInterval(oauthCountdownInterval);
                        oauthCountdownInterval = null;
                    }}
                    localStorage.setItem('aina_admin_key', key);
                    alertEl.innerHTML = '🎉 <strong>' + data.message + '</strong><br><small>Akun siap digunakan! Anda bisa langsung klik tombol di atas lagi untuk menambahkan akun berikutnya.</small>';
                    alertEl.style.color = '#10b981';
                    alertEl.style.display = 'block';
                    codeInput.value = '';
                    currentOAuthSessionId = null;
                    document.getElementById('oauth-step-2').style.display = 'none';
                    loadAccountPool();
                }} else {{
                    alertEl.innerText = '❌ ' + (data.error || 'Kode otorisasi tidak valid.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }}
            }} catch(e) {{
                alertEl.innerText = '❌ Error: ' + e;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            }} finally {{
                submitBtn.disabled = false;
                submitBtn.innerText = '✅ Simpan & Verifikasi';
            }}
        }}

        async function deleteAccount(id) {{
            if (!confirm('Yakin ingin menghapus Akun #' + id + ' dari pool?')) return;
            const key = localStorage.getItem('aina_admin_key') || '';
            try {{
                const res = await fetch('/api/auth/accounts/' + id + '?key=' + encodeURIComponent(key), {{
                    method: 'DELETE'
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    loadAccountPool();
                }} else {{
                    alert('Gagal menghapus: ' + (data.error || 'Unknown error'));
                }}
            }} catch(e) {{
                alert('Gagal menghapus akun: ' + e);
            }}
        }}

        function toggleAddAccountForm() {{
            const form = document.getElementById('add-account-form');
            if (form) {{
                const isOpening = form.style.display === 'none';
                form.style.display = isOpening ? 'block' : 'none';
                if (isOpening) {{
                    const keyInput = document.getElementById('add-token-key');
                    if (keyInput && !keyInput.value) {{
                        keyInput.value = localStorage.getItem('aina_admin_key') || '';
                    }}
                }}
            }}
        }}

        async function submitAddAccount() {{
            const tokenInput = document.getElementById('add-token-input');
            const keyInput = document.getElementById('add-token-key');
            const btn = document.getElementById('add-token-btn');
            const alertEl = document.getElementById('add-token-alert');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            const tokenVal = tokenInput.value.trim();

            if (!key) {{
                alertEl.innerText = 'Harap masukkan Admin Key atau WHATSMEOW_API_KEY Anda pada kolom di atas.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }}

            if (!tokenVal) {{
                alertEl.innerText = 'Harap masukkan string JSON token OAuth.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }}

            btn.disabled = true;
            btn.innerText = 'Menyimpan & Memverifikasi...';
            alertEl.style.display = 'none';

            try {{
                const res = await fetch('/api/auth/token?key=' + encodeURIComponent(key), {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ token: tokenVal, setup_code: key }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    localStorage.setItem('aina_admin_key', key);
                    alertEl.innerText = '✅ ' + data.message;
                    alertEl.style.color = '#10b981';
                    alertEl.style.display = 'block';
                    tokenInput.value = '';
                    loadAccountPool();
                    setTimeout(() => {{ toggleAddAccountForm(); }}, 2000);
                }} else {{
                    alertEl.innerText = '❌ ' + (data.error || 'Gagal menambahkan akun.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }}
            }} catch(e) {{
                alertEl.innerText = '❌ Error koneksi: ' + e.message;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            }} finally {{
                btn.disabled = false;
                btn.innerText = 'Simpan & Verifikasi';
            }}
        }}

        async function unlockAdminSession() {{
            const input = document.getElementById('admin-passcode-input');
            const errEl = document.getElementById('unlock-error');
            const btn = document.getElementById('unlock-btn');
            const val = input.value.trim();
            if (!val) {{
                errEl.innerText = 'Harap masukkan Setup Code / Admin Key.';
                errEl.style.display = 'block';
                return;
            }}
            errEl.style.display = 'none';
            btn.disabled = true;
            btn.innerText = 'Memverifikasi...';

            try {{
                const res = await fetch('/api/auth/verify', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ admin_key: val }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    localStorage.setItem('aina_admin_key', val);
                    checkAdminAuth();
                }} else {{
                    errEl.innerText = data.message || 'Admin Key tidak valid!';
                    errEl.style.display = 'block';
                }}
            }} catch(e) {{
                errEl.innerText = 'Koneksi error: ' + e.message;
                errEl.style.display = 'block';
            }} finally {{
                btn.disabled = false;
                btn.innerText = 'Buka Kunci';
            }}
        }}

        function lockAdminSession() {{
            localStorage.removeItem('aina_admin_key');
            checkAdminAuth();
        }}

        async function openModelModal() {{
            const key = localStorage.getItem('aina_admin_key');
            if (!key) {{
                alert('Silakan masukkan Admin Key / Setup Code terlebih dahulu pada kartu simulator di bawah.');
                return;
            }}
            const currentEl = document.getElementById('active-model-display');
            const current = currentEl ? currentEl.innerText.trim() : '';
            const choice = prompt(
                `Pilih Model AI Default untuk Aina:\n\nModel aktif saat ini: ${{current}}\n\nPilihan Model Tersedia:\n• gemini-3.8-flash-medium (Default Cepat & Seimbang)\n• gemini-3.8-flash-high (Penalaran Tinggi / Deep Thinking)\n• gemini-3.8-flash-low (Respons Kilat & Kasual)\n• gemini-3.1-pro-high (Deep Coding & Arsitektur)\n• claude-opus-4-6-thinking (Claude Opus Thinking - Khusus Eksplisit)\n• claude-sonnet-4-6 (Claude Sonnet 4.6)\n\nMasukkan nama model baru:`,
                current
            );
            if (!choice || choice.trim() === current || choice.trim() === '') return;

            try {{
                const res = await fetch('/api/model', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                        'X-Admin-Key': key
                    }},
                    body: JSON.stringify({{ model: choice.trim() }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    alert(`✅ Sukses! Model default Aina sekarang: ${{data.model}}`);
                    if (currentEl) currentEl.innerText = data.model;
                }} else {{
                    alert(`⚠️ Gagal mengganti model: ${{data.error || data.message || 'Error'}}`);
                }}
            }} catch(e) {{
                alert(`Koneksi error: ${{e.message}}`);
            }}
        }}

        function onChatTypeChange(val) {{
            const el = document.getElementById('mention-toggle-wrapper');
            if (el) el.style.display = (val === 'group') ? 'block' : 'none';
        }}

        function onSessionRoleChange(role) {{
            const fromMeEl = document.getElementById('sim-is-from-me');
            if (role === 'user_companion' && fromMeEl && !fromMeEl.checked) {{
                // Keep checkbox flexible for testing companion personal DMs or owner group commands
            }}
        }}

        function onIsFromMeChange(checked) {{
            const nameInput = document.getElementById('sim-sender-name');
            if (checked) {{
                if (!nameInput.dataset.original) nameInput.dataset.original = nameInput.value;
                nameInput.value = 'Saya (Owner)';
            }} else if (nameInput.dataset.original) {{
                nameInput.value = nameInput.dataset.original;
            }}
        }}

        function escapeHtml(str) {{
            if (!str) return '';
            return str
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#039;");
        }}

        async function resetSimulationChat() {{
            const adminKey = localStorage.getItem('aina_admin_key') || '';
            const threadEl = document.getElementById('sim-chat-thread');
            try {{
                await fetch('/api/simulate/reset', {{
                    method: 'POST',
                    headers: {{ 'X-Admin-Key': adminKey }}
                }});
                if (threadEl) {{
                    threadEl.innerHTML = `
                        <div class="chat-bubble-system">
                            🔄 Sesi percakapan direset. Anda dapat memulai obrolan atau pengujian topik baru dari awal.
                        </div>
                    `;
                }}
                const textEl = document.getElementById('sim-text');
                if (textEl) {{
                    textEl.value = '';
                    textEl.focus();
                }}
            }} catch(e) {{
                alert('Gagal mereset sesi percakapan: ' + e.message);
            }}
        }}

        function copySnippetText(encoded) {{
            try {{
                const decoded = decodeURIComponent(encoded);
                navigator.clipboard.writeText(decoded);
                alert('Teks jawaban Aina berhasil disalin ke clipboard!');
            }} catch(e) {{
                console.error(e);
            }}
        }}

        async function runSimulation() {{
            const textInput = document.getElementById('sim-text');
            const text = textInput.value.trim();
            const sessionRole = document.getElementById('sim-session-role') ? document.getElementById('sim-session-role').value : 'primary_bot';
            const chatType = document.getElementById('sim-chat-type').value;
            const senderName = document.getElementById('sim-sender-name').value.trim() || 'Owner';
            const isMention = document.getElementById('sim-is-mention') ? document.getElementById('sim-is-mention').checked : false;
            const isFromMe = document.getElementById('sim-is-from-me') ? document.getElementById('sim-is-from-me').checked : false;
            const modelSelectEl = document.getElementById('sim-model-select');
            const selectedModel = modelSelectEl ? modelSelectEl.value : 'default';
            const modelOverride = (selectedModel !== 'default') ? selectedModel : null;

            const btn = document.getElementById('sim-btn');
            const threadEl = document.getElementById('sim-chat-thread');
            const adminKey = localStorage.getItem('aina_admin_key') || '';

            if (!text) {{
                alert('Tolong ketik pesan chat terlebih dahulu.');
                return;
            }}

            // 1. Remove initial empty placeholder if present
            const emptyEl = document.getElementById('sim-thread-empty');
            if (emptyEl) emptyEl.remove();

            // 2. Append User Message Bubble
            const userRow = document.createElement('div');
            userRow.className = 'chat-row-user';
            const roleTag = (sessionRole === 'user_companion') ? ' <span style="font-size: 0.65rem; background: rgba(56,189,248,0.2); padding: 1px 4px; border-radius: 4px; color: #38bdf8;">Companion</span>' : '';
            userRow.innerHTML = `
                <div class="chat-bubble-user">
                    <div style="font-size: 0.72rem; opacity: 0.8; margin-bottom: 3px; font-weight: 600;">${{escapeHtml(senderName)}}${{roleTag}}</div>
                    <div style="white-space: pre-wrap;">${{escapeHtml(text)}}</div>
                </div>
            `;
            threadEl.appendChild(userRow);

            // 3. Clear text input immediately for fast reply DX
            textInput.value = '';

            // 4. Append Typing Indicator Bubble
            const botRow = document.createElement('div');
            botRow.className = 'chat-row-bot';
            botRow.innerHTML = `
                <div class="chat-bubble-typing">
                    <span class="pulse" style="width: 8px; height: 8px;"></span>
                    <span class="typing-text">Aina sedang berpikir dan mengeksekusi... (0s)</span>
                </div>
            `;
            threadEl.appendChild(botRow);
            threadEl.scrollTop = threadEl.scrollHeight;

            btn.disabled = true;
            let secondsElapsed = 0;
            btn.innerText = 'Memproses (0s)...';
            const timerInterval = setInterval(() => {{
                secondsElapsed++;
                btn.innerText = `Memproses (${{secondsElapsed}}s)...`;
                const typingSpan = botRow.querySelector('.typing-text');
                if (typingSpan) typingSpan.innerText = `Aina sedang berpikir dan mengeksekusi... (${{secondsElapsed}}s)`;
            }}, 1000);

            try {{
                // Submit simulation job
                const res = await fetch('/api/simulate', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                        'X-Admin-Key': adminKey
                    }},
                    body: JSON.stringify({{
                        text: text,
                        session_role: sessionRole,
                        chat_type: chatType,
                        sender_name: senderName,
                        is_mention: isMention,
                        is_from_me: isFromMe,
                        model_override: modelOverride
                    }})
                }});

                if (res.status === 401) {{
                    alert('Sesi kedaluwarsa atau Admin Key tidak valid. Harap buka kunci kembali.');
                    lockAdminSession();
                    botRow.remove();
                    return;
                }}

                const contentType = res.headers.get('content-type') || '';
                if (!res.ok && !contentType.includes('application/json')) {{
                    const rawBody = await res.text();
                    let errMsg = `Server HTTP error ${{res.status}}`;
                    if (res.status === 524 || res.status === 504 || res.status === 529) {{
                        errMsg = `Gateway Timeout / Overloaded (${{res.status}}): Server proxy memutuskan koneksi.`;
                    }} else {{
                        errMsg = rawBody.slice(0, 200) || errMsg;
                    }}
                    throw new Error(errMsg);
                }}

                const initialData = await res.json();

                // If Gatekeeper answered immediately (e.g. Ignore or RecordOnly)
                if (initialData.decision && initialData.decision !== 'Respond') {{
                    botRow.innerHTML = `
                        <div class="chat-bubble-system">
                            🛡️ Gatekeeper: ${{escapeHtml(initialData.decision)}} (${{escapeHtml(initialData.reason)}}) - <em>(Aina menyimak tanpa membalas chat sesuai etika grup)</em>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    return;
                }}

                // If sync response was returned directly
                if (initialData.decision === 'Respond') {{
                    const replyText = initialData.response_text || '';
                    lastSimulationResponseText = replyText;
                    botRow.innerHTML = `
                        <div class="chat-bubble-bot">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                    <span>🌸 Aina</span>
                                    <span style="color: #94a3b8; font-weight: 400;">• ${{initialData.duration_seconds ? initialData.duration_seconds.toFixed(1) : secondsElapsed}}s</span>
                                    <span style="color: #64748b; font-weight: 400;">(${{escapeHtml(initialData.reason)}})</span>
                                </div>
                                <button type="button" class="copy-btn" onclick="copySnippetText('${{encodeURIComponent(replyText)}}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                            </div>
                            <div class="markdown-body">${{renderMarkdownToHtml(replyText)}}</div>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    textInput.focus();
                    return;
                }}

                const jobId = initialData.job_id;
                if (!jobId) {{
                    throw new Error(initialData.error || 'Gagal memulai pekerjaan simulasi.');
                }}

                // Poll job status
                let isFinished = false;
                while (!isFinished) {{
                    await new Promise(r => setTimeout(r, 1500));

                    const pollRes = await fetch(`/api/simulate/job/${{jobId}}`, {{
                        headers: {{ 'X-Admin-Key': adminKey }}
                    }});

                    if (!pollRes.ok) {{
                        if (pollRes.status === 404) {{
                            throw new Error('Sesi pekerjaan simulasi kedaluwarsa.');
                        }}
                        continue;
                    }}

                    const jobData = await pollRes.json();

                    if (jobData.status === 'processing') {{
                        continue;
                    }} else if (jobData.status === 'completed') {{
                        isFinished = true;
                        const replyText = jobData.response_text || '';
                        lastSimulationResponseText = replyText;
                        botRow.innerHTML = `
                            <div class="chat-bubble-bot">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                    <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                        <span>🌸 Aina</span>
                                        <span style="color: #94a3b8; font-weight: 400;">• ${{jobData.duration_seconds ? jobData.duration_seconds.toFixed(1) : secondsElapsed}}s</span>
                                        <span style="color: #64748b; font-weight: 400;">(${{escapeHtml(jobData.reason)}})</span>
                                    </div>
                                    <button type="button" class="copy-btn" onclick="copySnippetText('${{encodeURIComponent(replyText)}}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                                </div>
                                <div class="markdown-body">${{renderMarkdownToHtml(replyText)}}</div>
                            </div>
                        `;
                        threadEl.scrollTop = threadEl.scrollHeight;
                        textInput.focus();
                    }} else if (jobData.status === 'failed') {{
                        isFinished = true;
                        throw new Error(jobData.error || 'Eksekusi agen AI gagal.');
                    }}
                }}
            }} catch(err) {{
                botRow.innerHTML = `
                    <div class="chat-bubble-system" style="border-color: rgba(239,68,68,0.4); color: #f87171;">
                        ⚠️ Gagal memproses simulasi: ${{escapeHtml(err.message)}}
                    </div>
                `;
                threadEl.scrollTop = threadEl.scrollHeight;
            }} finally {{
                clearInterval(timerInterval);
                btn.disabled = false;
                btn.innerHTML = '<span>Kirim</span> 🚀';
            }}
        }}

        function renderMarkdownToHtml(rawText) {{
            if (!rawText) return '';

            // 1. Configure marked
            marked.setOptions({{
                breaks: true,
                gfm: true,
                highlight: function(code, lang) {{
                    const language = (lang && hljs.getLanguage(lang)) ? lang : 'plaintext';
                    try {{
                        return hljs.highlight(code, {{ language }}).value;
                    }} catch(e) {{
                        return code;
                    }}
                }}
            }});

            // 2. Parse Markdown
            let rawHtml = marked.parse(rawText);

            // 3. Sanitize HTML
            let cleanHtml = DOMPurify.sanitize(rawHtml);

            // 4. Transform DOM for modern code headers and callout alerts
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = cleanHtml;

            // Code block decoration
            tempDiv.querySelectorAll('pre').forEach((pre) => {{
                const codeEl = pre.querySelector('code');
                let lang = 'code';
                if (codeEl) {{
                    const classes = Array.from(codeEl.classList);
                    const langClass = classes.find(c => c.startsWith('language-'));
                    if (langClass) lang = langClass.replace('language-', '');
                }}

                const wrapper = document.createElement('div');
                wrapper.className = 'code-block-wrapper';

                const header = document.createElement('div');
                header.className = 'code-block-header';
                header.innerHTML = `<span>${{lang}}</span><button type="button" class="copy-btn" onclick="copyCodeBlock(this)">Salin Kode</button>`;

                wrapper.appendChild(header);
                pre.parentNode.insertBefore(wrapper, pre);
                wrapper.appendChild(pre);
            }});

            // GitHub-style alerts: > [!NOTE], > [!TIP], > [!IMPORTANT], > [!WARNING], > [!CAUTION]
            tempDiv.querySelectorAll('blockquote').forEach((bq) => {{
                const text = bq.innerHTML.trim();
                const alertTypes = ['NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION'];
                const icons = {{
                    NOTE: 'ℹ️',
                    TIP: '💡',
                    IMPORTANT: '📌',
                    WARNING: '⚠️',
                    CAUTION: '🚨'
                }};
                for (const type of alertTypes) {{
                    const marker = `[!${{type}}]`;
                    if (text.includes(marker)) {{
                        const alertDiv = document.createElement('div');
                        alertDiv.className = `markdown-alert markdown-alert-${{type.toLowerCase()}}`;
                        const content = text.replace(marker, '').trim();
                        alertDiv.innerHTML = `<div class="markdown-alert-title">${{icons[type]}} ${{type}}</div><div>${{content}}</div>`;
                        bq.parentNode.replaceChild(alertDiv, bq);
                        break;
                    }}
                }}
            }});

            return tempDiv.innerHTML;
        }}

        function copyCodeBlock(btn) {{
            const wrapper = btn.closest('.code-block-wrapper');
            const codeEl = wrapper ? wrapper.querySelector('pre code') : null;
            if (!codeEl) return;
            navigator.clipboard.writeText(codeEl.innerText).then(() => {{
                const orig = btn.innerText;
                btn.innerText = '✓ Tersalin!';
                btn.style.color = '#10b981';
                setTimeout(() => {{
                    btn.innerText = orig;
                    btn.style.color = '';
                }}, 2000);
            }}).catch(e => {{
                console.error('Clipboard copy failed:', e);
            }});
        }}

        function copyFullResponse(btn) {{
            const textToCopy = lastSimulationResponseText || (document.getElementById('sim-response-text') ? document.getElementById('sim-response-text').innerText : '');
            if (!textToCopy) return;

            navigator.clipboard.writeText(textToCopy).then(() => {{
                const orig = btn.innerHTML;
                btn.innerHTML = '<span>✓ Jawaban Tersalin!</span>';
                btn.style.color = '#10b981';
                btn.style.borderColor = '#10b981';
                setTimeout(() => {{
                    btn.innerHTML = orig;
                    btn.style.color = '';
                    btn.style.borderColor = '';
                }}, 2000);
            }}).catch(e => {{
                console.error('Copy full response failed:', e);
            }});
        }}

        async function submitToken() {{
            const token = document.getElementById('token-input').value.trim();
            const setupCode = document.getElementById('setup-code-input').value.trim();
            const btn = document.getElementById('submit-btn');
            const alertBox = document.getElementById('alert-box');

            if (!setupCode) {{
                showAlert('Harap masukkan Kode Setup / Admin Key yang tertera di log.', false);
                return;
            }}

            if (!token) {{
                showAlert('Harap tempelkan token JSON terlebih dahulu.', false);
                return;
            }}

            try {{
                JSON.parse(token);
            }} catch(e) {{
                showAlert('Format token tidak valid: Harus berupa JSON yang valid.', false);
                return;
            }}

            btn.disabled = true;
            btn.innerText = 'Memverifikasi token...';
            alertBox.style.display = 'none';

            try {{
                const res = await fetch('/api/setup', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ token, setup_code: setupCode }})
                }});

                const data = await res.json();
                if (res.ok && data.success) {{
                    localStorage.setItem('aina_admin_key', setupCode);
                    showAlert(data.message + ' Halaman akan dimuat ulang...', true);
                    setTimeout(() => window.location.reload(), 2000);
                }} else {{
                    showAlert(data.message || 'Gagal memverifikasi token.', false);
                }}
            }} catch(err) {{
                showAlert('Terjadi kesalahan koneksi: ' + err.message, false);
            }} finally {{
                btn.disabled = false;
                btn.innerText = 'Verifikasi & Simpan Token';
            }}
        }}

        function showAlert(msg, isSuccess) {{
            const alertBox = document.getElementById('alert-box');
            alertBox.style.display = 'block';
            alertBox.className = 'alert ' + (isSuccess ? 'alert-success' : 'alert-error');
            alertBox.innerText = msg;
        }}

        // Run check on page load and attach Enter key handler
        function initPage() {{
            checkAdminAuth();
            const textEl = document.getElementById('sim-text');
            if (textEl && !textEl.dataset.bound) {{
                textEl.dataset.bound = 'true';
                textEl.addEventListener('keydown', function(e) {{
                    if (e.key === 'Enter' && !e.shiftKey) {{
                        e.preventDefault();
                        runSimulation();
                    }}
                }});
            }}
        }}
        document.addEventListener('DOMContentLoaded', initPage);
        initPage();
    </script>
</body>
</html>
        "#,
        badge_class = badge_class,
        badge_text = badge_text,
        status_card = status_card,
        auth_form_display = auth_form_display,
    )
}
