# Panduan Kontribusi Aina (Contributing Guide)

Terima kasih atas minat Anda untuk berkontribusi pada proyek **Aina**! Proyek ini mengusung standar kode bersih (*Clean Architecture*), kinerja tinggi (*zero runtime overhead*), serta kejelasan epistimik.

---

## 🛠️ Prasyarat Pengembangan (Prerequisites)

1. **Rust Toolchain**: Rust 2024 edition (disarankan versi `1.85+`).
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **Google Antigravity CLI (`agy`)**:
   ```bash
   curl -fsSL https://antigravity.google/cli/install.sh | bash
   agy auth login
   ```
3. **SQLite3**: Diperlukan untuk kompilasi modul pencarian teks FTS5.
4. **Python 3.10+**: Diperlukan untuk skrip pendukung di `scripts/`.

---

## 🧪 Menjalankan Pengujian (Testing)

Sebelum mengirimkan Pull Request (PR), seluruh unit test dan regression test wajib lulus 100%:

```bash
# Menjalankan seluruh test suite
cargo test

# Menjalankan test dengan output detail
cargo test -- --nocapture
```

---

## 🔍 Standar Kualitas Kode (Linting & Formatting)

Pastikan kode Anda mematuhi konvensi Rust dan bebas dari peringatan:

```bash
# Format kode sesuai standar rustfmt
cargo fmt --all -- --check

# Jalankan linter Clippy
cargo clippy --all-targets --all-features -- -D warnings
```

---

## 🚀 Alur Pull Request (PR Workflow)

1. **Fork Repositori**: Buat fork ke akun GitHub Anda.
2. **Buat Branch Fitur**: Gunakan penamaan branch yang deskriptif:
   - `feat/nama-fitur`
   - `fix/nama-bug`
   - `docs/nama-dokumentasi`
3. **Commit Pesan Konvensional**: Ikuti format Conventional Commits:
   - `feat: add support for telegram sensor port`
   - `fix: prevent timeout on heavy chat archive imports`
   - `docs: improve getting started guide`
4. **Buka Pull Request**: Jelaskan masalah yang dipecahkan, perubahan arsitektur (bila ada), dan lampirkan bukti lulus `cargo test`.

---

## 🛡️ Etika Kode & Kebijakan Privasi
Setiap kontribusi yang berinteraksi dengan WhatsApp atau gateway pesan wajib mematuhi:
- **Zero Privacy Leakage**: Jangan pernah mencatat pesan pribadi tanpa izin atau mengekspos token kredensial ke log.
- **Dependency Inversion**: Logika bisnis domain tidak boleh bergantung pada pustaka HTTP atau database eksternal (gunakan Ports & Adapters).
