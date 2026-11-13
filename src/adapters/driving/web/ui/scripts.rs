//! Client-side JavaScript interactivity, multi-turn simulator, and metrics fetching for Aina Web Dashboard.

pub fn get_scripts() -> &'static str {
    r#"
        let lastSimulationResponseText = '';

        function checkAdminAuth() {
            const key = localStorage.getItem('aina_admin_key');
            const simCard = document.getElementById('simulator-card');
            const lockCard = document.getElementById('sim-lock-card');
            if (!simCard || !lockCard) return;

            if (key) {
                simCard.style.display = 'block';
                lockCard.style.display = 'none';
            } else {
                simCard.style.display = 'none';
                lockCard.style.display = 'block';
            }
            loadAccountPool();
            loadObservabilityMetrics();
        }

        async function loadAccountPool() {
            const listEl = document.getElementById('account-pool-badges');
            if (!listEl) return;
            const key = localStorage.getItem('aina_admin_key') || '';
            try {
                const res = await fetch('/api/auth/accounts?key=' + encodeURIComponent(key));
                const data = await res.json();
                if (res.ok && data.success && data.accounts) {
                    if (data.accounts.length === 0) {
                        listEl.innerHTML = '<span style="color: #94a3b8; font-size: 0.85rem;">Belum ada akun di pool (menggunakan token file default).</span>';
                    } else {
                        listEl.innerHTML = data.accounts.map(a => {
                            const badgeColor = a.is_cooldown ? '#f59e0b' : '#10b981';
                            const reasonText = (a.is_cooldown && a.cooldown_reason) ? (' <span style="color: #f87171; font-size: 0.75rem;">[' + a.cooldown_reason + ']</span>') : '';
                            const statusText = a.is_cooldown ? ('Cooldown (' + a.cooldown_remaining_secs + 's)') : '🟢 Aktif';
                            const emailText = a.email ? (' <span style="color: #94a3b8; font-size: 0.78rem;">(' + a.email + ')</span>') : '';
                            const deleteBtn = (a.id > 1 || data.accounts.length > 1) ? 
                                ('<button type="button" onclick="deleteAccount(' + a.id + ')" title="Hapus Akun" style="background: none; border: none; color: #ef4444; cursor: pointer; padding: 0 4px; font-size: 0.85rem; line-height: 1;">✕</button>') : '';
                            return '<div style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); border-radius: 6px; padding: 6px 12px; font-size: 0.85rem; display: flex; align-items: center; gap: 8px;">' +
                                '<span style="font-weight: 600; color: #f8fafc;">#' + a.id + ' ' + a.label + '</span>' +
                                emailText +
                                '<span style="color: ' + badgeColor + '; font-size: 0.8rem;">' + statusText + '</span>' +
                                reasonText +
                                deleteBtn +
                            '</div>';
                        }).join('');
                    }
                }
            } catch(e) {
                console.warn('Failed to load accounts:', e);
            }
        }

        let currentOAuthSessionId = null;
        let oauthCountdownInterval = null;

        function sanitizeOAuthCode(val) {
            let s = (val || '').trim();
            try {
                s = decodeURIComponent(s);
            } catch(e) {}
            if (s.indexOf('code=') !== -1) {
                s = s.split('code=')[1];
            }
            const delimiters = ['&', '+http', ' http', 'userinfo.', 'rinfo.', '.profile', '+', ' '];
            for (let i = 0; i < delimiters.length; i++) {
                const d = delimiters[i];
                if (s.indexOf(d) !== -1) {
                    s = s.split(d)[0];
                }
            }
            return s.trim();
        }

        function toggleManualJsonInput() {
            const el = document.getElementById('manual-json-container');
            if (el) {
                el.style.display = el.style.display === 'none' ? 'block' : 'none';
            }
        }

        async function pasteAndSubmitOAuth() {
            try {
                if (navigator.clipboard && navigator.clipboard.readText) {
                    const text = await navigator.clipboard.readText();
                    if (text && text.trim()) {
                        document.getElementById('oauth-code-input').value = text.trim();
                    }
                }
            } catch(e) {
                console.log('Clipboard read note:', e);
            }
            submitOAuthCode();
        }

        async function startOAuthFlow() {
            const keyInput = document.getElementById('add-token-key');
            const alertEl = document.getElementById('oauth-alert');
            const startBtn = document.getElementById('start-oauth-btn');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            if (!key) {
                alertEl.innerText = 'Harap masukkan Admin Key atau WHATSMEOW_API_KEY terlebih dahulu di kolom atas.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }

            localStorage.setItem('aina_admin_key', key);

            // Pre-buka tab baru secara sinkronis pada interaksi pengguna untuk mencegah pemblokiran popup browser
            let newTab = null;
            try {
                newTab = window.open('about:blank', '_blank');
            } catch(e) {
                console.log('Popup pre-open prevented:', e);
            }

            startBtn.disabled = true;
            startBtn.innerText = 'Menyiapkan URL Google...';
            alertEl.style.display = 'none';

            try {
                const res = await fetch('/api/auth/oauth/init?key=' + encodeURIComponent(key), {
                    method: 'POST'
                });
                const data = await res.json();
                if (res.ok && data.success && data.auth_url) {
                    currentOAuthSessionId = data.session_id;
                    const anchor = document.getElementById('oauth-link-anchor');
                    if (anchor) {
                        anchor.href = data.auth_url;
                        anchor.style.display = 'inline-block';
                    }

                    document.getElementById('oauth-step-2').style.display = 'block';
                    document.getElementById('oauth-code-input').value = '';

                    // Buka URL OAuth di tab yang telah disiapkan, atau pandu pengguna mengklik tautan langsung
                    if (newTab && !newTab.closed) {
                        newTab.location.href = data.auth_url;
                        alertEl.innerHTML = 'ℹ️ Tab login Google telah dibuka. Pilih salah satu akun Google Anda, klik <strong>Izinkan</strong>, lalu salin kode yang muncul ke kotak di bawah.<br><small style="color: #94a3b8;">(Jika tab tidak muncul, klik tombol biru <strong>"🔗 Buka Halaman Login Google ↗"</strong> di bawah)</small>';
                    } else {
                        alertEl.innerHTML = '⚠️ Tab otomatis diblokir oleh browser Anda. Silakan <strong>klik tombol biru di bawah ("🔗 Buka Halaman Login Google ↗")</strong> untuk login ke akun Google.';
                    }
                    alertEl.style.color = '#38bdf8';
                    alertEl.style.display = 'block';

                    // 300-second (5-minute) countdown for user convenience
                    if (oauthCountdownInterval) clearInterval(oauthCountdownInterval);
                    let remaining = 300;
                    const countdownEl = document.getElementById('oauth-countdown');
                    const badgeEl = document.getElementById('oauth-timer-badge');
                    const submitBtn = document.getElementById('submit-oauth-code-btn');
                    const pasteBtn = document.getElementById('paste-submit-oauth-btn');
                    if (countdownEl) countdownEl.innerText = remaining;
                    if (badgeEl) {
                        badgeEl.style.color = '#10b981';
                        badgeEl.style.background = 'rgba(16, 185, 129, 0.15)';
                        badgeEl.innerHTML = '⏱️ Sisa Waktu: <strong id="oauth-countdown">' + remaining + '</strong>s';
                    }
                    if (submitBtn) submitBtn.disabled = false;
                    if (pasteBtn) pasteBtn.disabled = false;

                    oauthCountdownInterval = setInterval(() => {
                        remaining--;
                        const cEl = document.getElementById('oauth-countdown');
                        if (cEl) cEl.innerText = remaining;
                        if (remaining <= 0) {
                            clearInterval(oauthCountdownInterval);
                            oauthCountdownInterval = null;
                            if (badgeEl) {
                                badgeEl.style.color = '#fbbf24';
                                badgeEl.style.background = 'rgba(245, 158, 11, 0.15)';
                                badgeEl.innerHTML = '⏱️ Sesi > 5 Menit (Klik Mulai Ulang jika butuh tautan baru)';
                            }
                        }
                    }, 1000);
                } else {
                    if (newTab && !newTab.closed) newTab.close();
                    alertEl.innerText = '❌ ' + (data.error || 'Gagal memulai sesi login.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }
            } catch(e) {
                if (newTab && !newTab.closed) newTab.close();
                alertEl.innerText = '❌ Error: ' + e;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            } finally {
                startBtn.disabled = false;
                startBtn.innerText = '🚀 Mulai Ulang / Akun Lain';
            }
        }

        async function submitOAuthCode() {
            const codeInput = document.getElementById('oauth-code-input');
            const keyInput = document.getElementById('add-token-key');
            const alertEl = document.getElementById('oauth-alert');
            const submitBtn = document.getElementById('submit-oauth-code-btn');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            const rawCode = codeInput.value.trim();
            const code = sanitizeOAuthCode(rawCode);
            codeInput.value = code;

            if (!code) {
                alertEl.innerText = 'Harap tempelkan kode otorisasi dari Google terlebih dahulu.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }

            if (!currentOAuthSessionId) {
                alertEl.innerText = 'Sesi login belum dimulai. Klik "Mulai Login Akun Google" terlebih dahulu.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }

            submitBtn.disabled = true;
            submitBtn.innerText = 'Memverifikasi ke Google...';
            alertEl.style.display = 'none';

            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), 20000);

            try {
                const res = await fetch('/api/auth/oauth/exchange?key=' + encodeURIComponent(key), {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    signal: controller.signal,
                    body: JSON.stringify({
                        session_id: currentOAuthSessionId,
                        code: code,
                        setup_code: key
                    })
                });
                clearTimeout(timeoutId);
                const data = await res.json();
                if (res.ok && data.success) {
                    if (oauthCountdownInterval) {
                        clearInterval(oauthCountdownInterval);
                        oauthCountdownInterval = null;
                    }
                    localStorage.setItem('aina_admin_key', key);
                    alertEl.innerHTML = '🎉 <strong>' + data.message + '</strong><br><small>Akun siap digunakan! Anda bisa langsung klik tombol di atas lagi untuk menambahkan akun berikutnya.</small>';
                    alertEl.style.color = '#10b981';
                    alertEl.style.display = 'block';
                    codeInput.value = '';
                    currentOAuthSessionId = null;
                    document.getElementById('oauth-step-2').style.display = 'none';
                    loadAccountPool();
                } else {
                    alertEl.innerText = '❌ ' + (data.error || 'Kode otorisasi tidak valid.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }
            } catch(e) {
                clearTimeout(timeoutId);
                const isAbort = e.name === 'AbortError';
                alertEl.innerText = isAbort
                    ? '⚠️ Waktu verifikasi habis (20 detik). Server sedang memproses atau koneksi lambat. Silakan periksa daftar akun di bawah atau klik tombol Verifikasi lagi.'
                    : '❌ Error: ' + e;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            } finally {
                submitBtn.disabled = false;
                submitBtn.innerText = '✅ Simpan & Verifikasi';
            }
        }

        async function deleteAccount(id) {
            if (!confirm('Yakin ingin menghapus Akun #' + id + ' dari pool?')) return;
            const key = localStorage.getItem('aina_admin_key') || '';
            try {
                const res = await fetch('/api/auth/accounts/' + id + '?key=' + encodeURIComponent(key), {
                    method: 'DELETE'
                });
                const data = await res.json();
                if (res.ok && data.success) {
                    loadAccountPool();
                } else {
                    alert('Gagal menghapus: ' + (data.error || 'Unknown error'));
                }
            } catch(e) {
                alert('Gagal menghapus akun: ' + e);
            }
        }

        function toggleAddAccountForm() {
            const form = document.getElementById('add-account-form');
            if (form) {
                const isOpening = form.style.display === 'none';
                form.style.display = isOpening ? 'block' : 'none';
                if (isOpening) {
                    const keyInput = document.getElementById('add-token-key');
                    if (keyInput && !keyInput.value) {
                        keyInput.value = localStorage.getItem('aina_admin_key') || '';
                    }
                }
            }
        }

        async function submitAddAccount() {
            const tokenInput = document.getElementById('add-token-input');
            const keyInput = document.getElementById('add-token-key');
            const btn = document.getElementById('add-token-btn');
            const alertEl = document.getElementById('add-token-alert');

            let key = (keyInput ? keyInput.value.trim() : '') || localStorage.getItem('aina_admin_key') || '';
            const tokenVal = tokenInput.value.trim();

            if (!key) {
                alertEl.innerText = 'Harap masukkan Admin Key atau WHATSMEOW_API_KEY Anda pada kolom di atas.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }

            if (!tokenVal) {
                alertEl.innerText = 'Harap masukkan string JSON token OAuth.';
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
                return;
            }

            btn.disabled = true;
            btn.innerText = 'Menyimpan & Memverifikasi...';
            alertEl.style.display = 'none';

            try {
                const res = await fetch('/api/auth/token?key=' + encodeURIComponent(key), {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ token: tokenVal, setup_code: key })
                });
                const data = await res.json();
                if (res.ok && data.success) {
                    localStorage.setItem('aina_admin_key', key);
                    alertEl.innerText = '✅ ' + data.message;
                    alertEl.style.color = '#10b981';
                    alertEl.style.display = 'block';
                    tokenInput.value = '';
                    loadAccountPool();
                    setTimeout(() => { toggleAddAccountForm(); }, 2000);
                } else {
                    alertEl.innerText = '❌ ' + (data.error || 'Gagal menambahkan akun.');
                    alertEl.style.color = '#ef4444';
                    alertEl.style.display = 'block';
                }
            } catch(e) {
                alertEl.innerText = '❌ Error koneksi: ' + e.message;
                alertEl.style.color = '#ef4444';
                alertEl.style.display = 'block';
            } finally {
                btn.disabled = false;
                btn.innerText = 'Simpan & Verifikasi';
            }
        }

        async function unlockAdminSession() {
            const input = document.getElementById('admin-passcode-input');
            const errEl = document.getElementById('unlock-error');
            const btn = document.getElementById('unlock-btn');
            const val = input.value.trim();
            if (!val) {
                errEl.innerText = 'Harap masukkan Setup Code / Admin Key.';
                errEl.style.display = 'block';
                return;
            }
            errEl.style.display = 'none';
            btn.disabled = true;
            btn.innerText = 'Memverifikasi...';

            try {
                const res = await fetch('/api/auth/verify', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ admin_key: val })
                });
                const data = await res.json();
                if (res.ok && data.success) {
                    localStorage.setItem('aina_admin_key', val);
                    checkAdminAuth();
                    loadSimModelOptions();
                } else {
                    errEl.innerText = data.message || 'Admin Key tidak valid!';
                    errEl.style.display = 'block';
                }
            } catch(e) {
                errEl.innerText = 'Koneksi error: ' + e.message;
                errEl.style.display = 'block';
            } finally {
                btn.disabled = false;
                btn.innerText = 'Buka Kunci';
            }
        }

        function lockAdminSession() {
            localStorage.removeItem('aina_admin_key');
            checkAdminAuth();
        }

        // ── Model Picker Modal ──────────────────────────────────────────────────
        function _buildModelModal() {
            const existing = document.getElementById('model-picker-overlay');
            if (existing) return existing;

            const overlay = document.createElement('div');
            overlay.id = 'model-picker-overlay';
            overlay.style.cssText = `
                display:none; position:fixed; inset:0; z-index:9999;
                background:rgba(0,0,0,0.65); backdrop-filter:blur(4px);
                align-items:center; justify-content:center;
            `;
            overlay.innerHTML = `
                <div id="model-picker-card" style="
                    background:var(--bg-card,#1a2236); border:1px solid var(--border,#2d3a52);
                    border-radius:16px; padding:28px 24px; width:min(480px,92vw);
                    max-height:80vh; display:flex; flex-direction:column; gap:16px;
                    box-shadow:0 24px 64px rgba(0,0,0,0.5);
                    font-family:'Google Sans',sans-serif;
                ">
                    <div style="display:flex; align-items:center; justify-content:space-between;">
                        <h3 style="margin:0; font-size:1.1rem; font-weight:700; color:var(--text-primary,#e2e8f0);">
                            ⚡ Pilih Model AI
                        </h3>
                        <button onclick="closeModelModal()" style="
                            background:none; border:none; color:var(--text-muted,#64748b);
                            font-size:1.4rem; cursor:pointer; line-height:1; padding:4px 8px;
                            border-radius:8px; transition:background 0.15s;
                        " onmouseover="this.style.background='rgba(255,255,255,0.08)'"
                           onmouseout="this.style.background='none'">✕</button>
                    </div>
                    <p id="model-picker-current" style="
                        margin:0; font-size:0.82rem; color:var(--text-muted,#64748b);
                    ">Model aktif: —</p>
                    <div id="model-picker-loading" style="
                        text-align:center; padding:32px 0; color:var(--text-muted,#64748b); font-size:0.9rem;
                    ">
                        <span style="display:inline-block; animation:aina-spin 1s linear infinite;">⟳</span>
                        &nbsp; Memuat daftar model…
                    </div>
                    <div id="model-picker-list" style="
                        overflow-y:auto; flex-direction:column; gap:8px;
                        max-height:340px; display:none;
                    "></div>
                    <div id="model-picker-error" style="
                        display:none; color:#f87171; font-size:0.85rem; text-align:center; padding:12px 0;
                    "></div>
                    <div id="model-picker-source" style="
                        font-size:0.73rem; color:var(--text-muted,#64748b); text-align:right; display:none;
                    "></div>
                </div>
                <style>
                    @keyframes aina-spin { to { transform: rotate(360deg); } }
                    .model-item:hover { background: rgba(99,179,237,0.12) !important; border-color: var(--primary,#4a9eff) !important; }
                    .model-item.active-model { border-color: var(--primary,#4a9eff) !important; background: rgba(74,158,255,0.1) !important; }
                    .cat-badge { display:inline-block; padding:2px 8px; border-radius:99px; font-size:0.7rem; font-weight:600; letter-spacing:0.03em; }
                    .cat-gemini { background:rgba(66,153,225,0.2); color:#63b3ed; }
                    .cat-claude { background:rgba(214,158,46,0.2); color:#d6a52e; }
                    .cat-gpt    { background:rgba(72,187,120,0.2); color:#48bb78; }
                    .cat-other  { background:rgba(160,174,192,0.15); color:#a0aec0; }
                </style>
            `;
            overlay.addEventListener('click', (e) => { if (e.target === overlay) closeModelModal(); });
            document.body.appendChild(overlay);
            return overlay;
        }

        function closeModelModal() {
            const overlay = document.getElementById('model-picker-overlay');
            if (overlay) overlay.style.display = 'none';
        }

        async function openModelModal() {
            const key = localStorage.getItem('aina_admin_key');
            if (!key) {
                alert('Silakan masukkan Admin Key / Setup Code terlebih dahulu pada kartu simulator di bawah.');
                return;
            }
            const overlay = _buildModelModal();
            overlay.style.display = 'flex';

            const listEl   = document.getElementById('model-picker-list');
            const loadEl   = document.getElementById('model-picker-loading');
            const errorEl  = document.getElementById('model-picker-error');
            const sourceEl = document.getElementById('model-picker-source');
            const currentEl     = document.getElementById('active-model-display');
            const currentLblEl  = document.getElementById('model-picker-current');

            const activeCurrent = currentEl ? currentEl.innerText.trim() : '';
            if (currentLblEl) currentLblEl.textContent = `Model aktif saat ini: ${activeCurrent}`;

            listEl.style.display  = 'none';
            loadEl.style.display  = 'block';
            errorEl.style.display = 'none';
            sourceEl.style.display = 'none';
            listEl.innerHTML = '';

            try {
                const res = await fetch(`/api/models?key=${encodeURIComponent(key)}`);
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const data = await res.json();

                loadEl.style.display = 'none';

                if (!data.available || data.available.length === 0) {
                    errorEl.textContent = 'Tidak ada model tersedia.';
                    errorEl.style.display = 'block';
                    return;
                }

                data.available.forEach(model => {
                    const isActive = model.id === (data.current || activeCurrent);
                    const catClass = `cat-${model.category || 'other'}`;
                    const item = document.createElement('div');
                    item.className = 'model-item' + (isActive ? ' active-model' : '');
                    item.dataset.modelId = model.id;
                    item.style.cssText = `
                        display:flex; align-items:center; justify-content:space-between; gap:12px;
                        padding:12px 14px; border-radius:10px; cursor:pointer;
                        border:1px solid var(--border,#2d3a52);
                        background:rgba(255,255,255,0.03); transition:all 0.15s ease;
                        margin-bottom:4px;
                    `;
                    item.innerHTML = `
                        <div style="display:flex; flex-direction:column; gap:3px; min-width:0; overflow:hidden;">
                            <span style="font-size:0.88rem; font-weight:600; color:var(--text-primary,#e2e8f0); font-family:monospace; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;">${_escHtml(model.id)}</span>
                            <span style="font-size:0.76rem; color:var(--text-muted,#64748b);">${_escHtml(model.name)}</span>
                        </div>
                        <div style="display:flex; align-items:center; gap:8px; flex-shrink:0;">
                            <span class="cat-badge ${catClass}">${_escHtml(model.category || 'other')}</span>
                            ${isActive ? '<span style="color:var(--primary,#4a9eff); font-size:0.78rem; font-weight:700;">✓ Aktif</span>' : ''}
                        </div>
                    `;
                    item.addEventListener('click', () => _selectModel(model.id, key));
                    listEl.appendChild(item);
                });

                listEl.style.display = 'flex';
                if (data.source) {
                    sourceEl.textContent = data.source === 'dynamic' ? '🌐 Sumber: AGY CLI (live)' : '📦 Sumber: Static fallback';
                    sourceEl.style.display = 'block';
                }

            } catch (e) {
                loadEl.style.display = 'none';
                errorEl.textContent = `Gagal memuat daftar model: ${e.message}`;
                errorEl.style.display = 'block';
            }
        }

        async function _selectModel(modelId, key) {
            if (!key) return;
            const currentEl = document.getElementById('active-model-display');
            document.querySelectorAll('.model-item').forEach(el => {
                el.classList.toggle('active-model', el.dataset.modelId === modelId);
            });
            try {
                const res = await fetch('/api/model', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', 'X-Admin-Key': key },
                    body: JSON.stringify({ model: modelId })
                });
                const data = await res.json();
                if (res.ok && data.success) {
                    if (currentEl) currentEl.innerText = data.model;
                    const lbl = document.getElementById('model-picker-current');
                    if (lbl) lbl.textContent = `Model aktif saat ini: ${data.model}`;
                    setTimeout(() => closeModelModal(), 600);
                } else {
                    alert(`⚠️ Gagal: ${data.error || data.message || 'Error tidak diketahui'}`);
                    const prev = currentEl ? currentEl.innerText.trim() : '';
                    document.querySelectorAll('.model-item').forEach(el => {
                        el.classList.toggle('active-model', el.dataset.modelId === prev);
                    });
                }
            } catch (e) {
                alert(`Koneksi error: ${e.message}`);
            }
        }

        function _escHtml(str) {
            return String(str).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
        }
        // ── End Model Picker Modal ──────────────────────────────────────────

        // Dynamically populate the simulator's model <select> from /api/models,
        // so it always reflects what AGY actually exposes (no hardcoded list).
        async function loadSimModelOptions() {
            const sel = document.getElementById('sim-model-select');
            if (!sel) return;
            const loadingOpt = sel.querySelector('option[value="__loading__"]');

            const key = localStorage.getItem('aina_admin_key');
            if (!key) {
                // No admin key yet — keep only the "default" option; the list
                // will refresh once the user authenticates.
                return;
            }

            try {
                const res = await fetch(`/api/models?key=${encodeURIComponent(key)}`);
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const data = await res.json();

                if (loadingOpt) loadingOpt.remove();
                const current = data.current || '';
                const available = data.available || [];
                const frag = document.createDocumentFragment();
                available.forEach(model => {
                    const opt = document.createElement('option');
                    opt.value = model.id;
                    opt.textContent = (model.id === current ? '✓ ' : '') + model.name;
                    frag.appendChild(opt);
                });
                sel.appendChild(frag);
            } catch (e) {
                // Discovery failed — drop the loading placeholder so the user
                // still has a working "default" choice; the modal remains the
                // primary path for switching models.
                if (loadingOpt) loadingOpt.remove();
                console.warn('sim-model-select: failed to load model list:', e);
            }
        }
        // ── End Simulator Model Options Loader ──────────────────────────────

        function onChatTypeChange(val) {
            const el = document.getElementById('mention-toggle-wrapper');
            if (el) el.style.display = (val === 'group') ? 'block' : 'none';
        }

        function onSessionRoleChange(role) {
            const fromMeEl = document.getElementById('sim-is-from-me');
            if (role === 'user_companion' && fromMeEl && !fromMeEl.checked) {
                // Keep checkbox flexible for testing companion personal DMs or owner group commands
            }
        }

        function onIsFromMeChange(checked) {
            const nameInput = document.getElementById('sim-sender-name');
            if (checked) {
                if (!nameInput.dataset.original) nameInput.dataset.original = nameInput.value;
                nameInput.value = 'Saya (Owner)';
            } else if (nameInput.dataset.original) {
                nameInput.value = nameInput.dataset.original;
            }
        }

        function escapeHtml(str) {
            if (!str) return '';
            return str
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#039;");
        }

        async function resetSimulationChat() {
            const adminKey = localStorage.getItem('aina_admin_key') || '';
            const threadEl = document.getElementById('sim-chat-thread');
            try {
                await fetch('/api/simulate/reset', {
                    method: 'POST',
                    headers: { 'X-Admin-Key': adminKey }
                });
                if (threadEl) {
                    threadEl.innerHTML = `
                        <div class="chat-bubble-system">
                            🔄 Sesi percakapan direset. Anda dapat memulai obrolan atau pengujian topik baru dari awal.
                        </div>
                    `;
                }
                const textEl = document.getElementById('sim-text');
                if (textEl) {
                    textEl.value = '';
                    textEl.focus();
                }
            } catch(e) {
                alert('Gagal mereset sesi percakapan: ' + e.message);
            }
        }

        function copySnippetText(encoded) {
            try {
                const decoded = decodeURIComponent(encoded);
                navigator.clipboard.writeText(decoded);
                alert('Teks jawaban Aina berhasil disalin ke clipboard!');
            } catch(e) {
                console.error(e);
            }
        }

        async function runSimulation() {
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

            if (!text) {
                alert('Tolong ketik pesan chat terlebih dahulu.');
                return;
            }

            // 1. Remove initial empty placeholder if present
            const emptyEl = document.getElementById('sim-thread-empty');
            if (emptyEl) emptyEl.remove();

            // 2. Append User Message Bubble
            const userRow = document.createElement('div');
            userRow.className = 'chat-row-user';
            const roleTag = (sessionRole === 'user_companion') ? ' <span style="font-size: 0.65rem; background: rgba(56,189,248,0.2); padding: 1px 4px; border-radius: 4px; color: #38bdf8;">Companion</span>' : '';
            userRow.innerHTML = `
                <div class="chat-bubble-user">
                    <div style="font-size: 0.72rem; opacity: 0.8; margin-bottom: 3px; font-weight: 600;">${escapeHtml(senderName)}${roleTag}</div>
                    <div style="white-space: pre-wrap;">${escapeHtml(text)}</div>
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
            const timerInterval = setInterval(() => {
                secondsElapsed++;
                btn.innerText = `Memproses (${secondsElapsed}s)...`;
                const typingSpan = botRow.querySelector('.typing-text');
                if (typingSpan) typingSpan.innerText = `Aina sedang berpikir dan mengeksekusi... (${secondsElapsed}s)`;
            }, 1000);

            try {
                // Submit simulation job
                const res = await fetch('/api/simulate', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Admin-Key': adminKey
                    },
                    body: JSON.stringify({
                        text: text,
                        session_role: sessionRole,
                        chat_type: chatType,
                        sender_name: senderName,
                        is_mention: isMention,
                        is_from_me: isFromMe,
                        model_override: modelOverride
                    })
                });

                if (res.status === 401) {
                    alert('Sesi kedaluwarsa atau Admin Key tidak valid. Harap buka kunci kembali.');
                    lockAdminSession();
                    botRow.remove();
                    return;
                }

                const contentType = res.headers.get('content-type') || '';
                if (!res.ok && !contentType.includes('application/json')) {
                    const rawBody = await res.text();
                    let errMsg = `Server HTTP error ${res.status}`;
                    if (res.status === 524 || res.status === 504 || res.status === 529) {
                        errMsg = `Gateway Timeout / Overloaded (${res.status}): Server proxy memutuskan koneksi.`;
                    } else {
                        errMsg = rawBody.slice(0, 200) || errMsg;
                    }
                    throw new Error(errMsg);
                }

                const initialData = await res.json();

                // If Gatekeeper answered immediately (e.g. Ignore or RecordOnly)
                if (initialData.decision && initialData.decision !== 'Respond') {
                    botRow.innerHTML = `
                        <div class="chat-bubble-system">
                            🛡️ Gatekeeper: ${escapeHtml(initialData.decision)} (${escapeHtml(initialData.reason)}) - <em>(Aina menyimak tanpa membalas chat sesuai etika grup)</em>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    return;
                }

                // If sync response was returned directly
                if (initialData.decision === 'Respond') {
                    const replyText = initialData.response_text || '';
                    lastSimulationResponseText = replyText;
                    botRow.innerHTML = `
                        <div class="chat-bubble-bot">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                    <span>🌸 Aina</span>
                                    <span style="color: #94a3b8; font-weight: 400;">• ${initialData.duration_seconds ? initialData.duration_seconds.toFixed(1) : secondsElapsed}s</span>
                                    <span style="color: #64748b; font-weight: 400;">(${escapeHtml(initialData.reason)})</span>
                                </div>
                                <button type="button" class="copy-btn" onclick="copySnippetText('${encodeURIComponent(replyText)}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                            </div>
                            <div class="markdown-body">${renderMarkdownToHtml(replyText)}</div>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    textInput.focus();
                    return;
                }

                const jobId = initialData.job_id;
                if (!jobId) {
                    throw new Error(initialData.error || 'Gagal memulai pekerjaan simulasi.');
                }

                // Poll job status
                let isFinished = false;
                while (!isFinished) {
                    await new Promise(r => setTimeout(r, 1500));

                    const pollRes = await fetch(`/api/simulate/job/${jobId}`, {
                        headers: { 'X-Admin-Key': adminKey }
                    });

                    if (!pollRes.ok) {
                        if (pollRes.status === 404) {
                            throw new Error('Sesi pekerjaan simulasi kedaluwarsa.');
                        }
                        continue;
                    }

                    const jobData = await pollRes.json();

                    if (jobData.status === 'processing') {
                        continue;
                    } else if (jobData.status === 'completed') {
                        isFinished = true;
                        const replyText = jobData.response_text || '';
                        lastSimulationResponseText = replyText;
                        botRow.innerHTML = `
                            <div class="chat-bubble-bot">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                    <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                        <span>🌸 Aina</span>
                                        <span style="color: #94a3b8; font-weight: 400;">• ${jobData.duration_seconds ? jobData.duration_seconds.toFixed(1) : secondsElapsed}s</span>
                                        <span style="color: #64748b; font-weight: 400;">(${escapeHtml(jobData.reason)})</span>
                                    </div>
                                    <button type="button" class="copy-btn" onclick="copySnippetText('${encodeURIComponent(replyText)}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                                </div>
                                <div class="markdown-body">${renderMarkdownToHtml(replyText)}</div>
                            </div>
                        `;
                        threadEl.scrollTop = threadEl.scrollHeight;
                        textInput.focus();
                    } else if (jobData.status === 'failed') {
                        isFinished = true;
                        throw new Error(jobData.error || 'Eksekusi agen AI gagal.');
                    }
                }
            } catch(err) {
                botRow.innerHTML = `
                    <div class="chat-bubble-system" style="border-color: rgba(239,68,68,0.4); color: #f87171;">
                        ⚠️ Gagal memproses simulasi: ${escapeHtml(err.message)}
                    </div>
                `;
                threadEl.scrollTop = threadEl.scrollHeight;
            } finally {
                clearInterval(timerInterval);
                btn.disabled = false;
                btn.innerHTML = '<span>Kirim</span> 🚀';
            }
        }

        function renderMarkdownToHtml(rawText) {
            if (!rawText) return '';

            // 1. Configure marked
            marked.setOptions({
                breaks: true,
                gfm: true,
                highlight: function(code, lang) {
                    const language = (lang && hljs.getLanguage(lang)) ? lang : 'plaintext';
                    try {
                        return hljs.highlight(code, { language }).value;
                    } catch(e) {
                        return code;
                    }
                }
            });

            // 2. Parse Markdown
            let rawHtml = marked.parse(rawText);

            // 3. Sanitize HTML
            let cleanHtml = DOMPurify.sanitize(rawHtml);

            // 4. Transform DOM for modern code headers and callout alerts
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = cleanHtml;

            // Code block decoration
            tempDiv.querySelectorAll('pre').forEach((pre) => {
                const codeEl = pre.querySelector('code');
                let lang = 'code';
                if (codeEl) {
                    const classes = Array.from(codeEl.classList);
                    const langClass = classes.find(c => c.startsWith('language-'));
                    if (langClass) lang = langClass.replace('language-', '');
                }

                const wrapper = document.createElement('div');
                wrapper.className = 'code-block-wrapper';

                const header = document.createElement('div');
                header.className = 'code-block-header';
                header.innerHTML = `<span>${lang}</span><button type="button" class="copy-btn" onclick="copyCodeBlock(this)">Salin Kode</button>`;

                wrapper.appendChild(header);
                pre.parentNode.insertBefore(wrapper, pre);
                wrapper.appendChild(pre);
            });

            // GitHub-style alerts: > [!NOTE], > [!TIP], > [!IMPORTANT], > [!WARNING], > [!CAUTION]
            tempDiv.querySelectorAll('blockquote').forEach((bq) => {
                const text = bq.innerHTML.trim();
                const alertTypes = ['NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION'];
                const icons = {
                    NOTE: 'ℹ️',
                    TIP: '💡',
                    IMPORTANT: '📌',
                    WARNING: '⚠️',
                    CAUTION: '🚨'
                };
                for (const type of alertTypes) {
                    const marker = `[!${type}]`;
                    if (text.includes(marker)) {
                        const alertDiv = document.createElement('div');
                        alertDiv.className = `markdown-alert markdown-alert-${type.toLowerCase()}`;
                        const content = text.replace(marker, '').trim();
                        alertDiv.innerHTML = `<div class="markdown-alert-title">${icons[type]} ${type}</div><div>${content}</div>`;
                        bq.parentNode.replaceChild(alertDiv, bq);
                        break;
                    }
                }
            });

            return tempDiv.innerHTML;
        }

        function copyCodeBlock(btn) {
            const wrapper = btn.closest('.code-block-wrapper');
            const codeEl = wrapper ? wrapper.querySelector('pre code') : null;
            if (!codeEl) return;
            navigator.clipboard.writeText(codeEl.innerText).then(() => {
                const orig = btn.innerText;
                btn.innerText = '✓ Tersalin!';
                btn.style.color = '#10b981';
                setTimeout(() => {
                    btn.innerText = orig;
                    btn.style.color = '';
                }, 2000);
            }).catch(e => {
                console.error('Clipboard copy failed:', e);
            });
        }

        function copyFullResponse(btn) {
            const textToCopy = lastSimulationResponseText || (document.getElementById('sim-response-text') ? document.getElementById('sim-response-text').innerText : '');
            if (!textToCopy) return;

            navigator.clipboard.writeText(textToCopy).then(() => {
                const orig = btn.innerHTML;
                btn.innerHTML = '<span>✓ Jawaban Tersalin!</span>';
                btn.style.color = '#10b981';
                btn.style.borderColor = '#10b981';
                setTimeout(() => {
                    btn.innerHTML = orig;
                    btn.style.color = '';
                    btn.style.borderColor = '';
                }, 2000);
            }).catch(e => {
                console.error('Copy full response failed:', e);
            });
        }

        async function submitToken() {
            const token = document.getElementById('token-input').value.trim();
            const setupCode = document.getElementById('setup-code-input').value.trim();
            const btn = document.getElementById('submit-btn');
            const alertBox = document.getElementById('alert-box');

            if (!setupCode) {
                showAlert('Harap masukkan Kode Setup / Admin Key yang tertera di log.', false);
                return;
            }

            if (!token) {
                showAlert('Harap tempelkan token JSON terlebih dahulu.', false);
                return;
            }

            try {
                JSON.parse(token);
            } catch(e) {
                showAlert('Format token tidak valid: Harus berupa JSON yang valid.', false);
                return;
            }

            btn.disabled = true;
            btn.innerText = 'Memverifikasi token...';
            alertBox.style.display = 'none';

            try {
                const res = await fetch('/api/setup', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ token, setup_code: setupCode })
                });

                const data = await res.json();
                if (res.ok && data.success) {
                    localStorage.setItem('aina_admin_key', setupCode);
                    showAlert(data.message + ' Halaman akan dimuat ulang...', true);
                    setTimeout(() => window.location.reload(), 2000);
                } else {
                    showAlert(data.message || 'Gagal memverifikasi token.', false);
                }
            } catch(err) {
                showAlert('Terjadi kesalahan koneksi: ' + err.message, false);
            } finally {
                btn.disabled = false;
                btn.innerText = 'Verifikasi & Simpan Token';
            }
        }

        function showAlert(msg, isSuccess) {
            const alertBox = document.getElementById('alert-box');
            alertBox.style.display = 'block';
            alertBox.className = 'alert ' + (isSuccess ? 'alert-success' : 'alert-error');
            alertBox.innerText = msg;
        }

        async function loadObservabilityMetrics() {
            const usecasesEl = document.getElementById('obs-usecases-list');
            const toolsEl = document.getElementById('obs-tools-list');
            if (!usecasesEl || !toolsEl) return;

            const key = localStorage.getItem('aina_admin_key') || '';
            const url = '/api/audit/summary' + (key ? ('?key=' + encodeURIComponent(key)) : '');

            try {
                const res = await fetch(url);
                const data = await res.json();
                if (res.ok && data.success && data.summary) {
                    const s = data.summary;
                    // Render Top Usecases
                    if (!s.top_usecases || s.top_usecases.length === 0) {
                        usecasesEl.innerHTML = '<span style="color: #94a3b8; font-size: 0.82rem;">Belum ada aktivitas usecase tercatat.</span>';
                    } else {
                        let totalUsecases = 0;
                        for (let i = 0; i < s.top_usecases.length; i++) {
                            const item = s.top_usecases[i];
                            const count = Array.isArray(item) ? item[1] : (typeof item === 'object' && item ? (item.count || 0) : 0);
                            totalUsecases += count;
                        }
                        if (totalUsecases <= 0) totalUsecases = 1;

                        let htmlUsecases = '';
                        for (let i = 0; i < s.top_usecases.length; i++) {
                            const item = s.top_usecases[i];
                            const rawName = Array.isArray(item) ? item[0] : (typeof item === 'object' && item ? (item.key || item.usecase || item.name || 'other') : String(item));
                            const count = Array.isArray(item) ? item[1] : (typeof item === 'object' && item ? (item.count || 0) : 0);
                            const pct = Math.round((count / totalUsecases) * 100);
                            const label = formatUsecaseLabel(rawName);
                            htmlUsecases += '<div style="margin-bottom: 8px;">' +
                                '<div style="display: flex; justify-content: space-between; font-size: 0.82rem; margin-bottom: 3px;">' +
                                    '<span style="font-weight: 500; color: #f1f5f9;">' + label + '</span>' +
                                    '<span style="color: #a78bfa; font-family: monospace;">' + count + ' (' + pct + '%)</span>' +
                                '</div>' +
                                '<div style="background: rgba(255,255,255,0.08); border-radius: 4px; height: 6px; overflow: hidden;">' +
                                    '<div style="background: linear-gradient(90deg, #8b5cf6, #38bdf8); height: 100%; width: ' + pct + '%;"></div>' +
                                '</div>' +
                            '</div>';
                        }
                        usecasesEl.innerHTML = htmlUsecases;
                    }

                    // Render Top Tools
                    const toolList = (s.tool_call_frequency && s.tool_call_frequency.length > 0)
                        ? s.tool_call_frequency
                        : ((s.top_tools_used && s.top_tools_used.length > 0)
                            ? s.top_tools_used
                            : (s.unique_tools_used || []));

                    if (!toolList || toolList.length === 0) {
                        toolsEl.innerHTML = '<span style="color: #94a3b8; font-size: 0.82rem;">Belum ada pemanggilan alat AI tercatat.</span>';
                    } else {
                        let maxCalls = 1;
                        for (let i = 0; i < toolList.length; i++) {
                            const t = toolList[i];
                            const count = Array.isArray(t) ? t[1] : (typeof t === 'object' && t ? (t.count || 1) : 1);
                            if (count > maxCalls) maxCalls = count;
                        }

                        let htmlTools = '';
                        const limitTools = Math.min(toolList.length, 10);
                        for (let i = 0; i < limitTools; i++) {
                            const t = toolList[i];
                            const rawName = Array.isArray(t) ? t[0] : (typeof t === 'object' && t ? (t.key || t.name || 'unknown') : String(t));
                            const name = String(rawName || '');
                            const count = Array.isArray(t) ? t[1] : (typeof t === 'object' && t ? (t.count || 1) : 1);
                            const pct = Math.round((count / maxCalls) * 100);
                            const isBuiltin = name.startsWith('builtin:');
                            const badgeColor = isBuiltin ? '#f59e0b' : '#10b981';
                            const badgeBg = isBuiltin ? 'rgba(245, 158, 11, 0.15)' : 'rgba(16, 185, 129, 0.15)';
                            htmlTools += '<div style="margin-bottom: 8px;">' +
                                '<div style="display: flex; justify-content: space-between; align-items: center; font-size: 0.82rem; margin-bottom: 3px;">' +
                                    '<span style="font-family: monospace; color: #f8fafc; display: inline-flex; align-items: center; gap: 4px;">' +
                                        name +
                                        (isBuiltin ? ' <span style="font-size: 0.68rem; padding: 1px 4px; border-radius: 3px; background:' + badgeBg + '; color:' + badgeColor + ';">builtin</span>' : '') +
                                    '</span>' +
                                    '<span style="color: #38bdf8; font-family: monospace; font-weight: 600;">' + count + '×</span>' +
                                '</div>' +
                                '<div style="background: rgba(255,255,255,0.08); border-radius: 4px; height: 6px; overflow: hidden;">' +
                                    '<div style="background: linear-gradient(90deg, #10b981, #06b6d4); height: 100%; width: ' + pct + '%;"></div>' +
                                '</div>' +
                            '</div>';
                        }
                        toolsEl.innerHTML = htmlTools;
                    }
                } else if (res.status === 401) {
                    usecasesEl.innerHTML = '<span style="color: #f59e0b; font-size: 0.82rem;">🔒 Masukkan Admin Key di simulator untuk membuka metrik observabilitas.</span>';
                    toolsEl.innerHTML = '<span style="color: #f59e0b; font-size: 0.82rem;">🔒 Masukkan Admin Key di simulator untuk membuka metrik observabilitas.</span>';
                } else {
                    usecasesEl.innerHTML = '<span style="color: #ef4444; font-size: 0.82rem;">Gagal memuat metrik: ' + (data.error || 'Server error') + '</span>';
                    toolsEl.innerHTML = '<span style="color: #ef4444; font-size: 0.82rem;">Gagal memuat metrik: ' + (data.error || 'Server error') + '</span>';
                }
            } catch (err) {
                usecasesEl.innerHTML = '<span style="color: #ef4444; font-size: 0.82rem;">Error koneksi: ' + err + '</span>';
                toolsEl.innerHTML = '<span style="color: #ef4444; font-size: 0.82rem;">Error koneksi: ' + err + '</span>';
            }
        }

        function formatUsecaseLabel(key) {
            const labels = {
                'code_and_devops': '💻 Coding & DevOps',
                'document_and_vision': '📄 Dokumen & Vision',
                'data_analysis': '📊 Analisis Data',
                'information_and_research': '🔍 Riset & Informasi',
                'task_and_scheduling': '⏰ Jadwal & Tugas',
                'knowledge_and_context': '🧠 Konteks & Memori',
                'system_and_control': '⚙️ Kontrol Sistem',
                'media_and_whatsapp': '📱 Media & WhatsApp',
                'casual_and_consultation': '💬 Konsultasi & Santai',
                'other': '📦 Lainnya'
            };
            return labels[key] || key;
        }

        // Run check on page load and attach Enter key handler
        function initPage() {
            checkAdminAuth();
            loadObservabilityMetrics();
            loadSimModelOptions();
            const textEl = document.getElementById('sim-text');
            if (textEl && !textEl.dataset.bound) {
                textEl.dataset.bound = 'true';
                textEl.addEventListener('keydown', function(e) {
                    if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault();
                        runSimulation();
                    }
                });
            }
        }
        document.addEventListener('DOMContentLoaded', initPage);
        initPage();
    "#
}
