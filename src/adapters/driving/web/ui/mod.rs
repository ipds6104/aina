//! Web UI Dashboard rendering module partitioned according to Single Responsibility Principle (SRP):
//! - `styles`: CSS stylesheets, typography, and dark theme definitions.
//! - `scripts`: Frontend interactive JavaScript, simulation pipeline, and metrics loader.
//! - `mod`: HTML structure assembly, server status cards, and dashboard renderer.

pub mod scripts;
pub mod styles;

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
                            <span id="oauth-timer-badge" style="font-size: 0.78rem; color: #10b981; background: rgba(16, 185, 129, 0.15); border: 1px solid rgba(16, 185, 129, 0.3); border-radius: 4px; padding: 2px 8px;">
                                ⏱️ Sisa Waktu: <strong id="oauth-countdown">300</strong>s
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
                        <div style="margin-bottom: 8px;">
                            <button type="button" id="paste-submit-oauth-btn" class="btn" style="width: 100%; background: #059669; color: #fff; font-size: 0.82rem; padding: 8px 14px; font-weight: 600; cursor: pointer; border-radius: 4px; border: none; display: flex; align-items: center; justify-content: center; gap: 6px;" onclick="pasteAndSubmitOAuth()">
                                📋 Tempel dari Clipboard &amp; Simpan (1-Klik Cepat)
                            </button>
                        </div>
                        <label class="form-label" style="font-size: 0.78rem; color: #94a3b8;">Atau tempel manual di bawah:</label>
                        <div style="display: flex; gap: 8px;">
                            <input id="oauth-code-input" class="form-input" type="text" placeholder="Tempel kode 4/0A... di sini" style="flex: 1; font-size: 0.82rem; margin-bottom: 0;" />
                            <button type="button" id="submit-oauth-code-btn" class="btn" style="background: #10b981; font-size: 0.82rem; padding: 6px 14px; cursor: pointer; white-space: nowrap;" onclick="submitOAuthCode()">
                                ✅ Simpan
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

            <!-- OBSERVABILITY & USECASES METRICS CARD -->
            <div class="card" style="border-color: #8b5cf6;">
                <div class="card-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div style="display: flex; align-items: center; gap: 8px;">
                        <span style="font-size: 1.2rem;">📊</span>
                        <h2>Observabilitas: Alat & Usecase Pengguna</h2>
                    </div>
                    <button type="button" class="btn-outline" style="font-size: 0.8rem; padding: 4px 10px; cursor: pointer;" onclick="loadObservabilityMetrics()">
                        🔄 Segarkan Metrik
                    </button>
                </div>
                <p style="color: var(--text-muted); font-size: 0.88rem; margin-bottom: 12px;">
                    Statistik real-time mengenai jenis usecase interaksi dan alat agentik yang paling sering digunakan oleh pengguna Aina.
                </p>
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 14px;">
                    <div style="background: rgba(15, 23, 42, 0.6); border: 1px solid rgba(139, 92, 246, 0.25); border-radius: 8px; padding: 14px;">
                        <h4 style="margin: 0 0 10px 0; font-size: 0.92rem; color: #a78bfa; display: flex; align-items: center; gap: 6px;">
                            <span>🎯</span> Usecase Paling Sering Digunakan
                        </h4>
                        <div id="obs-usecases-list" style="font-size: 0.85rem; color: #cbd5e1;">
                            <span style="color: #94a3b8;">Memuat data usecase...</span>
                        </div>
                    </div>
                    <div style="background: rgba(15, 23, 42, 0.6); border: 1px solid rgba(139, 92, 246, 0.25); border-radius: 8px; padding: 14px;">
                        <h4 style="margin: 0 0 10px 0; font-size: 0.92rem; color: #a78bfa; display: flex; align-items: center; gap: 6px;">
                            <span>🛠️</span> Alat AI Teratas (Frekuensi)
                        </h4>
                        <div id="obs-tools-list" style="font-size: 0.85rem; color: #cbd5e1;">
                            <span style="color: #94a3b8;">Memuat data alat...</span>
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
    let css_styles = styles::get_styles();
    let js_scripts = scripts::get_scripts();

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
{css_styles}
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
{js_scripts}
    </script>
</body>
</html>
        "#,
        css_styles = css_styles,
        js_scripts = js_scripts,
        badge_class = badge_class,
        badge_text = badge_text,
        status_card = status_card,
        auth_form_display = auth_form_display,
    )
}
