//! Web UI Dashboard rendering module partitioned according to Single Responsibility Principle (SRP):
//! - `styles`: CSS stylesheets, typography, and dark theme definitions (Anti-Slop compliant).
//! - `scripts`: Frontend interactive JavaScript, simulation pipeline, and metrics loader.
//! - `mod`: HTML structure assembly, masthead, and dashboard renderer.

pub mod scripts;
pub mod styles;

use super::state::WebhookServerState;

pub fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn render_html(
    is_authenticated: bool,
    state: &WebhookServerState,
    current_model: &str,
    current_user: Option<&crate::adapters::driving::web::state::AuthenticatedUserSession>,
) -> String {
    let is_user_authenticated = current_user.is_some();
    let is_user_admin = current_user.map(|u| u.is_admin).unwrap_or(false);

    let (status_dot_class, badge_class, badge_text) = if is_authenticated {
        ("status-dot status-dot-online", "badge badge-success", "ENGINE ONLINE")
    } else {
        ("status-dot status-dot-warning", "badge badge-warning", "ENGINE PERLU SETUP")
    };

    let sso_section = if state.oidc_config.enabled {
        if let Some(user) = current_user {
            let role_badge = if user.is_admin {
                "<span class=\"sso-admin-tag\">ADMIN</span>"
            } else {
                "<span class=\"sso-staff-tag\">PEGAWAI</span>"
            };
            format!(
                r#"
                <div class="sso-profile">
                    <span class="sso-avatar"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg></span>
                    <span class="sso-user-name">{}</span>
                    {}
                    <a href="/auth/logout" class="btn-sso-logout">Keluar</a>
                </div>
                "#,
                html_escape(&user.name),
                role_badge,
            )
        } else {
            format!(
                r#"
                <a href="/auth/login" class="btn-sso-login">
                    Masuk dengan {}
                </a>
                "#,
                html_escape(&state.oidc_config.provider_name),
            )
        }
    } else {
        String::new()
    };

    let model_change_action = if is_user_admin {
        r#"<a href="javascript:void(0)" onclick="openModelModal()" style="font-size: 0.75rem; color: var(--primary); text-decoration: none; font-weight: 600;">Ganti Model</a>"#
    } else {
        ""
    };

    let account_card_style = if is_user_admin {
        "display: block;"
    } else {
        "display: none;"
    };

    let (sim_lock_display, simulator_display) = if is_user_authenticated {
        ("display: none;", "display: block;")
    } else {
        ("display: block;", "display: none;")
    };

    let session_status_label = if let Some(u) = current_user {
        let tag = if u.is_admin { "Admin" } else { "Pegawai" };
        format!("Sesi SSO: {} ({})", html_escape(&u.name), tag)
    } else {
        "Sesi Terverifikasi".to_string()
    };

    let companion_info = if let Some(c_jid) = &state.companion_jid {
        let c_name = state.companion_name.as_deref().unwrap_or("Personal Account");
        format!("{} ({}) <span style=\"color: var(--success); font-weight: 600;\">[Aktif]</span>", c_name, c_jid)
    } else {
        "<span style=\"color: var(--text-muted);\">Belum Dikonfigurasi (Opsional)</span>".to_string()
    };

    let status_card = if is_authenticated {
        format!(
            r#"
            <div class="card status-card">
                <div class="card-header">
                    <h2>Status Sistem &amp; Konektivitas</h2>
                </div>
                <p class="card-desc">Layanan asisten AI terhubung dan siap melayani pesan WhatsApp.</p>
                <div class="info-grid">
                    <div class="info-item"><span class="label">Nama Bot (Primary)</span><span class="val">{name}</span></div>
                    <div class="info-item"><span class="label">WhatsApp JID Bot</span><span class="val">{jid}</span></div>
                    <div class="info-item"><span class="label">Companion (Shadow Sensor)</span><span class="val">{companion_info}</span></div>
                    <div class="info-item">
                        <span class="label">Model AI Aktif</span>
                        <div style="display: flex; justify-content: space-between; align-items: baseline;">
                            <span class="val" id="active-model-display">{model}</span>
                            {model_change_action}
                        </div>
                    </div>
                    <div class="info-item"><span class="label">Whatsmeow Gateway</span><span class="val">{url}</span></div>
                    <div class="info-item"><span class="label">Zona Waktu / Locale</span><span class="val">{timezone} ({locale})</span></div>
                </div>
                <div class="helper-box">
                    <div style="font-size: 0.8rem; color: var(--text-muted); margin-bottom: 4px;">Webhook Endpoint:</div>
                    <code>POST /webhook</code>
                    <p style="margin-top: 6px; font-size: 0.82rem; color: var(--text-muted);">Arahkan webhook dari instance Whatsmeow ke URL ini.</p>
                </div>
            </div>

            <!-- MULTI-ACCOUNT POOL CARD -->
            <div class="card admin-only-card" style="{account_card_style}">
                <div class="card-header">
                    <h2>Pool Akun Antigravity</h2>
                    <button type="button" class="btn-outline" onclick="toggleAddAccountForm()">
                        + Tambah Akun
                    </button>
                </div>
                <p class="card-desc">
                    Daftar akun Google Antigravity aktif untuk rotasi otomatis saat kuota harian akun habis.
                </p>
                <div id="account-pool-badges" style="display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 12px;">
                    <span style="color: var(--text-muted); font-size: 0.85rem;">Memuat status pool akun...</span>
                </div>

                <!-- FORM TAMBAH AKUN CADANGAN (EXPANDABLE) -->
                <div id="add-account-form" style="display: none; background: var(--surface-subtle); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 16px; margin-top: 12px;">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                        <h4 style="margin: 0; font-size: 0.92rem; color: var(--text);">Tambah Akun Google via OAuth</h4>
                        <button type="button" class="btn-outline" onclick="toggleAddAccountForm()">Tutup</button>
                    </div>

                    <div style="margin-bottom: 10px;">
                        <label class="form-label">Admin Key / Setup Code:</label>
                        <input id="add-token-key" class="form-input" type="password" placeholder="Masukkan ADMIN_KEY atau Setup Code Anda..." />
                    </div>

                    <!-- METODE 1: OAUTH CEPAT -->
                    <div id="oauth-step-1" style="background: rgba(255,255,255,0.02); border: 1px dashed var(--border); border-radius: var(--radius-sm); padding: 14px; margin-bottom: 12px;">
                        <p style="font-size: 0.83rem; color: var(--text-secondary); margin-bottom: 10px;">
                            Hubungkan akun Google langsung tanpa perlu menyalin file JSON. Klik tombol di bawah untuk meminta tautan login resmi dari Google:
                        </p>
                        <button type="button" id="start-oauth-btn" class="btn" onclick="startOAuthFlow()">
                            Mulai Login Akun Google
                        </button>
                    </div>

                    <div id="oauth-step-2" style="display: none; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 14px; margin-bottom: 12px;">
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                            <span style="font-size: 0.84rem; font-weight: 600; color: var(--text);">Langkah Selanjutnya:</span>
                            <span id="oauth-timer-badge" style="font-size: 0.78rem; color: var(--success); background: var(--success-muted); border: 1px solid rgba(16, 185, 129, 0.3); border-radius: 4px; padding: 2px 8px;">
                                Sisa Waktu: <strong id="oauth-countdown">300</strong>s
                            </span>
                        </div>
                        <p style="font-size: 0.83rem; color: var(--text-secondary); margin-bottom: 10px; line-height: 1.5;">
                            1. Buka halaman login Google di bawah.<br>
                            2. Pilih akun Google Anda dan setujui izin.<br>
                            3. Salin kode otorisasi dan tempelkan di kotak berikut:
                        </p>
                        <div style="margin-bottom: 10px;">
                            <a id="oauth-link-anchor" href="javascript:void(0)" target="_blank" class="btn" style="text-decoration: none;">
                                Buka Halaman Login Google
                            </a>
                        </div>
                        <div style="margin-bottom: 8px;">
                            <button type="button" id="paste-submit-oauth-btn" class="btn" style="width: 100%;" onclick="pasteAndSubmitOAuth()">
                                Tempel dari Clipboard &amp; Simpan
                            </button>
                        </div>
                        <label class="form-label">Atau tempel manual di bawah:</label>
                        <div style="display: flex; gap: 8px;">
                            <input id="oauth-code-input" class="form-input" type="text" placeholder="Tempel kode 4/0A... di sini" style="flex: 1; margin-bottom: 0;" />
                            <button type="button" id="submit-oauth-code-btn" class="btn" onclick="submitOAuthCode()">
                                Simpan
                            </button>
                        </div>
                    </div>

                    <div id="oauth-alert" style="display: none; font-size: 0.85rem; margin-top: 8px; margin-bottom: 10px;"></div>

                    <!-- METODE 2: MANUAL JSON ACCORDION -->
                    <div style="border-top: 1px dashed var(--border); padding-top: 10px; margin-top: 10px;">
                        <a href="javascript:void(0)" onclick="toggleManualJsonInput()" style="color: var(--text-muted); font-size: 0.78rem; text-decoration: none;">
                            Atau tempel manual file JSON token (cadangan/opsional)
                        </a>
                        <div id="manual-json-container" style="display: none; margin-top: 8px;">
                            <textarea id="add-token-input" class="form-input" style="height: 60px;" placeholder='{{"token": "..."}}'></textarea>
                            <div style="display: flex; justify-content: flex-end; margin-top: 4px;">
                                <button type="button" id="add-token-btn" class="btn" onclick="submitAddAccount()">Simpan Token Manual</button>
                            </div>
                            <div id="add-token-alert" style="display: none; font-size: 0.85rem; margin-top: 8px;"></div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- OBSERVABILITY & USECASES METRICS CARD -->
            <div class="card">
                <div class="card-header">
                    <h2>Observabilitas: Penggunaan Alat &amp; Kategori</h2>
                    <button type="button" class="btn-outline" onclick="loadObservabilityMetrics()">
                        Segarkan
                    </button>
                </div>
                <p class="card-desc">
                    Distribusi usecase interaksi dan frekuensi pemanggilan alat agentik secara real-time.
                </p>
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 14px;">
                    <div style="background: var(--surface-subtle); border: 1px solid var(--border-muted); border-radius: var(--radius-sm); padding: 14px;">
                        <h4 style="margin: 0 0 10px 0; font-size: 0.88rem; color: var(--text);">
                            Kategori Interaksi Teratas
                        </h4>
                        <div id="obs-usecases-list" style="font-size: 0.85rem; color: var(--text-secondary);">
                            <span style="color: var(--text-muted);">Memuat data usecase...</span>
                        </div>
                    </div>
                    <div style="background: var(--surface-subtle); border: 1px solid var(--border-muted); border-radius: var(--radius-sm); padding: 14px;">
                        <h4 style="margin: 0 0 10px 0; font-size: 0.88rem; color: var(--text);">
                            Alat AI Teratas
                        </h4>
                        <div id="obs-tools-list" style="font-size: 0.85rem; color: var(--text-secondary);">
                            <span style="color: var(--text-muted);">Memuat data alat...</span>
                        </div>
                    </div>
                </div>
            </div>

            <!-- ADMIN LOCK CARD -->
            <div id="sim-lock-card" class="card" style="{sim_lock_display}">
                <div class="card-header">
                    <h2>Akses Simulator Terproteksi</h2>
                </div>
                <p class="card-desc">
                    Fitur simulator dilindungi. Masuk melalui SSO Pegawai BPS Mempawah di atas atau masukkan Setup Code / Admin Key server untuk membuka sesi.
                </p>
                <div style="display: flex; gap: 8px;">
                    <input id="admin-passcode-input" class="form-input" type="password" placeholder="Masukkan Admin Key / Setup Code..." style="margin-bottom: 0;" />
                    <button id="unlock-btn" class="btn" style="white-space: nowrap;" onclick="unlockAdminSession()">Buka Kunci</button>
                </div>
                <div id="unlock-error" style="display: none; color: var(--danger); font-size: 0.85rem; margin-top: 8px;"></div>
            </div>

            <!-- SIMULATOR CHAT REAL END-TO-END -->
            <div id="simulator-card" class="card simulator-card" style="{simulator_display}">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; padding: 6px 12px; background: var(--surface-subtle); border: 1px solid var(--border); border-radius: var(--radius-sm); font-size: 0.8rem;">
                    <span id="admin-session-status-text" style="color: var(--success); font-weight: 500;">{session_status_label}</span>
                    <a href="javascript:void(0)" onclick="lockAdminSession()" style="color: var(--danger); text-decoration: none; font-weight: 600;">Kunci Dasbor</a>
                </div>
                <div class="card-header">
                    <h2>Simulator Percakapan WhatsApp</h2>
                </div>
                <p class="card-desc">
                    Uji langsung logika respons, routing percakapan, etika grup (Gatekeeper), dan eksekusi agentik tanpa mengirim chat dari WhatsApp fisik.
                </p>

                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Sesi WhatsApp (Pipeline Routing)</label>
                        <select id="sim-session-role" class="form-input" onchange="onSessionRoleChange(this.value)">
                            <option value="primary_bot">Sesi Bot Utama (Primary Dedicated)</option>
                            <option value="user_companion">Sesi Companion (Akun Pribadi / Shadow Sensor)</option>
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

                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Nama Pengirim</label>
                        <input id="sim-sender-name" class="form-input" value="Owner" />
                    </div>
                    <div style="display: flex; align-items: center; padding-top: 10px;">
                        <label style="font-size: 0.85rem; color: var(--text-secondary); display: flex; align-items: center; gap: 8px; cursor: pointer;">
                            <input type="checkbox" id="sim-is-from-me" onchange="onIsFromMeChange(this.checked)" />
                            <span>Kirim Sebagai Diri Sendiri (is_from_me / Owner)</span>
                        </label>
                    </div>
                </div>

                <div style="margin-bottom: 12px;">
                    <label class="form-label">Pilihan Model AI (Sesi Simulasi Ini)</label>
                    <select id="sim-model-select" class="form-input">
                        <option value="default">Mengikuti Model Aktif Default ({model})</option>
                        <option value="__loading__" disabled>Memuat daftar model…</option>
                    </select>
                </div>

                <div id="mention-toggle-wrapper" style="display: none; margin-bottom: 12px;">
                    <label style="font-size: 0.85rem; color: var(--text-secondary); display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" id="sim-is-mention" />
                        <span>Simulasikan Tag / Mention (@Aina) dalam grup</span>
                    </label>
                </div>

                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 14px; margin-bottom: 8px;">
                    <label class="form-label" style="margin-bottom: 0; font-weight: 600;">Alur Percakapan Interaktif (Multi-Turn)</label>
                    <button type="button" class="btn-outline" onclick="resetSimulationChat()">
                        Reset Percakapan
                    </button>
                </div>

                <!-- SCROLLABLE CHAT THREAD -->
                <div id="sim-chat-thread" class="chat-thread-container">
                    <div id="sim-thread-empty" class="chat-bubble-system">
                        Belum ada pesan. Mulai obrolan dengan Aina di bawah. Anda dapat membalas chat secara berkelanjutan.
                    </div>
                </div>

                <label class="form-label">Ketik Pesan Chat (Enter untuk kirim, Shift+Enter untuk baris baru):</label>
                <div style="display: flex; gap: 8px; align-items: flex-start;">
                    <textarea id="sim-text" class="form-input" style="height: 60px; margin-bottom: 0; resize: none;" placeholder="Ketik pesan atau instruksi untuk Aina..."></textarea>
                    <button id="sim-btn" class="btn" style="min-width: 120px; height: 60px; font-weight: 600;" onclick="runSimulation()">
                        <span>Kirim</span>
                    </button>
                </div>
            </div>
            "#,
            name = state.bot_name,
            jid = state.bot_jid,
            companion_info = companion_info,
            model = current_model,
            model_change_action = model_change_action,
            url = state.whatsmeow_url,
            timezone = state.timezone,
            locale = state.locale,
            account_card_style = account_card_style,
            sim_lock_display = sim_lock_display,
            simulator_display = simulator_display,
            session_status_label = session_status_label,
        )
    } else {
        r#"
        <div class="card warning-card">
            <div class="card-header">
                <h2>Setup Autentikasi Google Antigravity</h2>
            </div>
            <p class="card-desc">Aina memerlukan OAuth Token untuk mengakses Google Antigravity CLI di server ini.</p>
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
    <title>Aina (あいな) - Dashboard</title>
    
    <!-- Google Fonts: Google Sans & Google Sans Text, JetBrains Mono -->
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
        <header class="masthead">
            <div class="masthead-brand">
                <span class="{status_dot_class}"></span>
                <div class="masthead-title">
                    Aina <span class="masthead-sub">(あいな)</span>
                </div>
            </div>
            <div class="masthead-actions">
                <span class="{badge_class}">{badge_text}</span>
                {sso_section}
            </div>
        </header>

        {status_card}

        <div id="auth-form-container" class="card" style="{auth_form_display}">
            <div class="card-header">
                <h2>Setup Autentikasi Pertama Kali</h2>
            </div>
            <p class="card-desc">
                Anda memerlukan Kode Setup yang tercetak pada log deployment server untuk menghubungkan akun.
            </p>

            <div class="helper-box" style="margin-bottom: 14px;">
                <p style="font-size: 0.82rem; color: var(--text-muted); margin-bottom: 4px;">Ambil token dari terminal lokal Anda:</p>
                <code>cat ~/.gemini/antigravity-cli/antigravity-oauth-token</code>
            </div>

            <label class="form-label">Kode Setup / Admin Key:</label>
            <input id="setup-code-input" class="form-input" placeholder="AINA-XXXXXX atau ADMIN_KEY Anda" />

            <label class="form-label">Tempelkan seluruh JSON token di bawah:</label>
            <textarea id="token-input" class="form-input" style="height: 120px;" placeholder='{{"auth_method":"oauth","id_token":"...","token":{{...}}}}'></textarea>
            
            <button id="submit-btn" class="btn" onclick="submitToken()">Verifikasi &amp; Simpan Token</button>
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
        status_dot_class = status_dot_class,
        badge_class = badge_class,
        badge_text = badge_text,
        status_card = status_card,
        auth_form_display = auth_form_display,
        sso_section = sso_section,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
