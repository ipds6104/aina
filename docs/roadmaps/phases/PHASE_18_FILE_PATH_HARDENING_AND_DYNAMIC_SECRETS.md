# 🔒 Fase 18: Canonical Path Sandboxing & Env-Var First Dynamic Secrets (Zero Hardcoded Credentials)

> **Status**: COMPLETED (`05cc43d` — Aligned with ADR-004, ADR-005, ADR-017, RISK-16, DEBT-05, QS-14)  
> **Komponen Target**:  
> - `skills/infisical/scripts/secret_tool.py`  
> - `src/adapters/driven/agy_cli/oauth.rs`  
> - `src/adapters/driving/web/controllers/auth_setup.rs`  
> - `scripts/oauth_helper.py`  
> - `scripts/login_account.py`  
> **Prinsip**: *"100% Env-Var First. Zero Hardcoded Secrets in Git. Optional Vendor-Agnostic Vault. Canonical Path Containment."*

---

## 1. Problem Statement & Root Cause

1. **Hardcoded Plaintext Master Vault Credentials (`skills/infisical/scripts/secret_tool.py`)**:
   - Baris 109–110 menanam `INFISICAL_CLIENT_ID` dan `INFISICAL_CLIENT_SECRET` secara **plaintext** di source code, disertai `DEFAULT_INFISICAL_DOMAIN` dan `DEFAULT_INFISICAL_PROJECT_ID` milik tenant privat (baris 32–34).
   - Hal ini melanggar mutlak Bab 2.2 poin 3 (*Larangan Rahasia Statis*), `ADR-004` (*Netralitas Core Upstream*), dan `ADR-005` (*Vendor-Agnostic Secret Broker*).
2. **Kredensial OAuth Statis Ter-obfuscate XOR di Skrip Python**:
   - `scripts/oauth_helper.py` (baris 24–28) dan `scripts/login_account.py` (baris 21–25) menyimpan `CLIENT_ID` dan `CLIENT_SECRET` Google dalam array byte XOR 42 (`_decode_key`).
   - `scripts/login_account.py` (baris 64 & 224) dan `src/adapters/driven/agy_cli/oauth.rs` (baris 82 & 141) juga masih memuat domain/path lokal hardcoded (`https://aina.dvlpid.my.id`, `/root/projects/aina/...`).
3. **Path Traversal & Arbitrary Directory Deletion pada Sesi OAuth**:
   - `get_oauth_session_dir(session_id)` di `src/adapters/driven/agy_cli/oauth.rs` (baris 5–23) dan `get_base_dir(session_id)` di `scripts/oauth_helper.py` (baris 67–73) menggabungkan `session_id` mentah dari payload JSON `POST /api/auth/oauth/exchange`.
   - Selain penulisan arbitrer (`status.txt`, `token.json`), `oauth.rs` (baris 168 & 182) memanggil `tokio::fs::remove_dir_all(&base_dir)` — payload `session_id: ".."` berisiko menghapus direktori induk!
4. **Ketidaksesuaian Runtime Docker untuk Vault**:
   - `Dockerfile` tidak memasang biner `/usr/local/bin/infisical` (dan tidak boleh mewajibkannya agar image upstream tetap minimalis). Skrip tidak boleh berasumsi Infisical CLI selalu tersedia atau mengimpor fungsi non-eksis.

---

## 2. Solusi Teknis & Arsitektur (Upstream Minimal-Dependency)

```text
[ Client Request: session_id ]
             │
             ▼
[ Dual-Side Strict Path Guard (Rust oauth.rs + Python oauth_helper.py) ]
    ├── 1. Whitelist: ^[a-zA-Z0-9_-]{16,64}$ (reject '..', '/', '\', null, out-of-range)
    ├── 2. Resolve root dir (/app/data/oauth_sessions | data/oauth_sessions | /tmp/aina_oauth)
    └── 3. Canonical containment: canonical_target.starts_with(&canonical_root)
             │
             ├── Pelanggaran? ──► [ REJECT: Err -> HTTP 400 Bad Request (tanpa remove_dir_all) ]
             └── Valid ─────────► [ Safe Session Dir ]
                                          │
                                          ▼
[ 100% Env-Var First Credential Resolution (Zero Mandatory Vault) ]
    ├── Rust/Python baca env standar: GOOGLE_CLIENT_ID / AINA_GOOGLE_CLIENT_ID
    │                                 GOOGLE_CLIENT_SECRET / AINA_GOOGLE_CLIENT_SECRET
    ├── Operator dengan Vault (opsional): suntikkan via Coolify Env / .env / `secret_tool run --`
    └── Tidak disetel? ──► Fail-Fast dengan pesan actionable (manual /api/auth/token tetap berjalan)
```

### A. Pembersihan Total Hardcoded Secrets di `skills/infisical/scripts/secret_tool.py` (Prioritas #1)
1. Hapus seluruh fallback plaintext `client_id` dan `client_secret` pada `InfisicalProvider.__init__` (baris 109–110).
2. Hapus konstanta domain & project ID privat (`secrets.dvlpid.my.id`, `f13379e0-...`); ganti default dengan string kosong `""` yang wajib diisi melalui env var (`INFISICAL_DOMAIN`, `INFISICAL_PROJECT_ID`, `INFISICAL_CLIENT_ID`, `INFISICAL_CLIENT_SECRET`) atau file konfigurasi lokal `.vault_config.json`.
3. Jika `InfisicalProvider` dipilih namun kredensial tidak disetel, kembalikan error eksplisit yang terstruktur (fail-fast), sementara mode `auto` tetap jatuh ke `EnvSecretProvider` lokal (nol dependensi cloud wajib — patuh `ADR-005`).
4. *(Tindakan operasional operator)*: Rotasi kredensial Infisical machine identity yang sebelumnya sempat ter-commit di riwayat Git.

### B. Pembersihan Total `_decode_key` & Netralisasi Upstream (`oauth_helper.py`, `login_account.py`, `oauth.rs`)
1. Hapus fungsi `_decode_key` dan seluruh array byte statis dari `scripts/oauth_helper.py` dan `scripts/login_account.py`.
2. Baca kredensial murni dari variabel lingkungan standar:
   ```python
   CLIENT_ID = (os.environ.get("GOOGLE_CLIENT_ID") or os.environ.get("AINA_GOOGLE_CLIENT_ID") or "").strip()
   CLIENT_SECRET = (os.environ.get("GOOGLE_CLIENT_SECRET") or os.environ.get("AINA_GOOGLE_CLIENT_SECRET") or "").strip()
   ```
   - **Dilarang** mengimpor modul internal `skills/infisical` secara langsung dari `scripts/` (menjaga decoupling & menghindari ketergantungan wajib Infisical). Jika pengguna memakai Vault (Infisical/HashiCorp/Doppler), injeksi dilakukan melalui kontrak resmi `IF-06` (`secret_tool run -- ...`) atau environment kontainer.
   - Jika `CLIENT_ID` atau `CLIENT_SECRET` kosong saat `init`/`exchange` dipanggil, keluar dengan exit code non-zero dan pesan jelas bahwa operator perlu menyetel env `GOOGLE_CLIENT_ID` & `GOOGLE_CLIENT_SECRET` (atau menggunakan input token manual di `/api/auth/token`).
3. Hapus default domain/path privat hardcoded:
   - `scripts/login_account.py`: ganti default `server_url` dari `https://aina.dvlpid.my.id` menjadi `http://127.0.0.1:8090` (override via `AINA_SERVER_URL`).
   - `src/adapters/driven/agy_cli/oauth.rs`: hapus kandidat path mesin lokal `/root/projects/aina/scripts/oauth_helper.py`; gunakan relative `scripts/oauth_helper.py` dan `/app/scripts/oauth_helper.py`.
4. Di `oauth.rs`, teruskan `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` (termasuk alias `AINA_GOOGLE_CLIENT_ID` / `AINA_GOOGLE_CLIENT_SECRET`) secara eksplisit ke `Command::env` jika terset di proses Rust.

### C. Sanitasi Jalur & Canonical Containment Ganda (Rust + Python)
1. **Di `src/adapters/driven/agy_cli/oauth.rs`**:
   - Tambah fungsi murni:
     ```rust
     pub fn is_safe_session_id(session_id: &str) -> bool {
         let len = session_id.len();
         (16..=64).contains(&len)
             && session_id
                 .bytes()
                 .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
     }
     ```
     *(Format generator `init_oauth_session`: `<13-digit millis>_<8-hex>` = 22 karakter, masuk tepat dalam rentang `16..=64` sesuai `RISK-16`.)*
   - Ubah signature `pub fn get_oauth_session_dir(session_id: &str) -> anyhow::Result<PathBuf>`.
   - Resolve direktori akar (`oauth_sessions` di `/app/data/oauth_sessions`, `data/oauth_sessions`, atau `/tmp/aina_oauth_sessions`), buat akar bila belum ada, lalu verifikasi bahwa path kandidat yang bergabung berada secara ketat di bawah akar kanonikal (`starts_with(&canonical_root)` dengan `Component::Normal` tunggal) **sebelum** `create_dir_all`, baca/tulis berkas, maupun `remove_dir_all`.
   - Di `auth_setup.rs`, pastikan kegagalan validasi `session_id` mengembalikan HTTP `400 Bad Request` yang bersih.
2. **Di `scripts/oauth_helper.py`**:
   - Validasi `session_id` dengan regex `^[a-zA-Z0-9_-]{16,64}$` di `get_base_dir()`.
   - Verifikasi `os.path.commonpath([root_real, target_real]) == root_real`; jika gagal, keluar dengan error sebelum membuat direktori atau menulis file.

---

## 3. Rencana Pengujian & Kriteria Selesai (Definition of Done)

1. **Uji Path Traversal Rejection (Rust & Python)**:
   - `get_oauth_session_dir("../../etc/passwd")`, `get_oauth_session_dir("..")`, `get_oauth_session_dir("short")`, `get_oauth_session_dir("a/b/c/1234567890123456")` semuanya mengembalikan `Err`.
   - `get_oauth_session_dir("1732150000000_a1b2c3d4")` mengembalikan `Ok(path)` di dalam direktori akar `oauth_sessions`.
   - `POST /api/auth/oauth/exchange` dengan `{"session_id": "../../evil", "code": "x"}` ditolak `400 Bad Request` dan tidak menghapus atau menulis berkas apa pun.
   - Uji langsung `python3 scripts/oauth_helper.py init ../../evil` mengembalikan exit code non-zero.
2. **Uji Zero Hardcoded Secrets & Neutrality**:
   - `rg "_decode_key|1a73548b|d972328c|secrets\.dvlpid\.my\.id|aina\.dvlpid\.my\.id|/root/projects/aina" src/ scripts/ skills/` mengembalikan **0 temuan** pada file target.
   - `secret_tool.py --provider infisical status` tanpa env var tidak lagi menggunakan kredensial hardcoded.
3. **Uji Env-Var First Graceful Failure & Success**:
   - Menjalankan `oauth_helper.py` tanpa `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` menghasilkan pesan error actionable (tanpa traceback `ModuleNotFoundError`).
   - Menjalankan dengan env var terset menghasilkan URL PKCE valid di dalam direktori sesi terisolasi.
4. **Zero New Dependencies**:
   - `Cargo.toml` dan `Dockerfile` tidak menambah crate Rust maupun biner vault wajib baru; `cargo test` lulus 100%.
