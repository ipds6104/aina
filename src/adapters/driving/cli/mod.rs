pub mod commands;
pub mod workspace_resolver;

pub struct CliDispatcher;

impl CliDispatcher {
    pub fn print_help() {
        println!(
            r#"Aina CLI - High-Performance Agentic Utilities

PENGGUNAAN:
    aina [SUBCOMMAND] [OPTIONS]
    aina [server | daemon]    # Menjalankan daemon webhook Aina (default)

    archive search [query]    Pencarian instan riwayat obrolan (FTS5 BM25 + Filter Temporal)
                              Options:
                                --since, -s <durasi>    Filter durasi lampau (misal: 1h, 24h, 7d, 30d)
                                --days, -d <n>          Filter n hari terakhir (alias cepat --since nd)
                                --from <YYYY-MM-DD>     Filter tanggal/waktu awal (misal: 2026-09-01)
                                --to <YYYY-MM-DD>       Filter tanggal/waktu akhir (misal: 2026-09-10)
                                --limit, -l <n>         Batas hasil (default: 10)
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --json                  Output format JSON terstruktur

    archive import <file>     Impor arsip obrolan WhatsApp (.zip / .txt) ke SQLite lokal
                              Options:
                                --slug, -s <nama>       Nama folder / pengenal obrolan
                                --no-media              Lewati ekstraksi dokumen dan lampiran media
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

    archive stats             Statistik arsip obrolan di workspace
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --json                  Output format JSON terstruktur

    kb lint                   Pemeriksaan kepatuhan kriteria kerapian Knowledge Base
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --auto-heal             Perbaiki struktur & kompilasi ulang otomatis
                                --json                  Output format JSON terstruktur

    kb groom                  Kompilasi ulang katalog `knowledge/index.md` deterministik
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

    kb schedule               Tampilkan ringkasan tenggat waktu (deadlines)
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

    audit [actions|summary|diag|transcripts] Audit jejak tindakan WhatsApp & Antigravity CLI
                              Subcommands:
                                actions                 Audit seluruh aksi & pesan WhatsApp yang ditangani Aina
                                summary                 Ringkasan agregat metrik audit, respons, dan frekuensi alat
                                diag, diagnostics       Diagnostik mendalam memori, repositori workspace, dan sistem
                                transcripts             Pencarian langsung pada transkrip log Antigravity CLI
                              Options:
                                --query, -q <text>      Pencarian kata kunci pada audit trail
                                --status <status>       Filter status aksi (success, failed, recorded, ignored)
                                --errors-only, -e       Hanya tampilkan langkah yang berstatus ERROR
                                --limit, -l <n>         Batas hasil (default: 20)
                                --json                  Output format JSON terstruktur

    workspace info            Informasi status & metadata workspace aktif
                              Options:
                                --workspace, -w <dir>   Path workspace (default: AGENT_WORKSPACE / default)
                                --json                  Output format JSON terstruktur

    workspace init <path>     Inisialisasi direktori workspace baru (di mana saja di disk)
                              Options:
                                --title, -t <nama>      Judul / deskripsi domain workspace
                              
    workspace sync            Tarik perubahan, kompilasi index, commit, dan push ke Git
                              Options:
                                --workspace, -w <dir>   Path workspace
                                --message, -m <text>    Custom commit message
                                --json                  Output format JSON

    workspace link <git-url>  Hubungkan workspace ke Git origin remote (auto .gitignore)

    workspace gh-status       Periksa status autentikasi GitHub CLI (`gh`)

    workspace gh-device       Mulai login GitHub via Device Code OAuth (Non-Blocking)
                              Subcommand: start (default), poll

    workspace gh-login <pat>  Login ke GitHub CLI secara non-interaktif via Personal Access Token

    workspace gh-create <n>   Buat repositori GitHub baru secara instan via `gh`
                              Options:
                                --public                Buat repositori publik (default: private)

    clone <git-url> [path]    Clone repositori GitHub knowledge base yang sudah ada
                              (Otomatis sinkronisasi, lint, dan generate katalog index.md)

    sync                      Alias cepat untuk `workspace sync`

    link <git-url>            Alias cepat untuk `workspace link`

    gh-device                 Alias cepat untuk `workspace gh-device start`

    gh-poll                   Alias cepat untuk `workspace gh-device poll`

    gh-login <pat>            Alias cepat untuk `workspace gh-login`

    whatsapp status           Status pipeline multi-session WhatsApp (Bot Utama & Companion)
                              Options:
                                --json                  Output format JSON terstruktur

    status                    Alias cepat untuk `whatsapp status`

    user list                 Daftar seluruh profil rekan kerja & wewenang (Profiling Memory)
                              Options:
                                --json                  Output format JSON terstruktur

    user get <jid>            Detail profil & wewenang rekan kerja spesifik
                              Options:
                                --json                  Output format JSON terstruktur

    user set <jid>            Perbarui/simpan profil & wewenang rekan kerja
                              Options:
                                --name <nama>           Nama rekan kerja
                                --role <peran>          Peran / jabatan tim
                                --authority <level>     Tingkat wewenang (admin | staff | guest)
                                --notes <catatan>       Catatan otorisasi & izin yang diberikan

    user search <query>       Cari profil rekan kerja berdasarkan kata kunci
                              Options:
                                --json                  Output format JSON terstruktur

    schedule list             Daftar tugas & riset terjadwal aktif
                              Options:
                                --all                   Tampilkan semua tugas (termasuk non-aktif)
                                --json                  Output format JSON terstruktur

    schedule add              Tambah tugas pengingat atau riset terjadwal baru
                              Options:
                                --title <judul>         Judul pengingat / tugas
                                --type <agent|notify>   Tipe: 'agent' (riset/tindakan AI) atau 'notify' (pesan teks langsung)
                                --target <jid>          Target WhatsApp (misal: 628xxx@s.whatsapp.net)
                                --when <once|daily|interval> Tipe: 'once' (satu kali), 'daily' (tiap hari), 'interval' (berkala)
                                --time <waktu>          Waktu eksekusi (misal: '07:30', '22:26', '+10m', '+1h')
                                --payload <pesan>       Prompt riset (untuk agent) atau teks pesan (untuk notify)

    schedule delete <id>      Hapus tugas terjadwal berdasarkan ID

    persona diag              Diagnostik & observabilitas Persona Status Engine
                              Options:
                                --json                  Output format JSON terstruktur

    persona journal           Riwayat jurnal publikasi status WhatsApp Aina
                              Options:
                                --limit <n>             Jumlah entri yang ditampilkan (default: 10)
                                --json                  Output format JSON terstruktur

    persona check             Periksa evaluasi slot dan kuota status WhatsApp saat ini

    persona post              Eksekusi pembuatan & publikasi status WhatsApp Aina
                              Options:
                                --slot <slot>           Override slot (pagi, siang, sore, malam)
                                --force                 Bypass kuota/peluang posting
                                --dry-run               Simulasi tanpa generate/upload

    model get                 Tampilkan model AI aktif saat ini
                              Options:
                                --json                  Output format JSON terstruktur

    model list                Daftar model AI yang didukung dan status aktifnya
                              Options:
                                --json                  Output format JSON terstruktur

    model set <model_name>    Ganti model AI aktif secara instan di runtime
                              Options:
                                --json                  Output format JSON terstruktur

    version, -v, --version    Informasi versi binary, commit hash, dan daftar kapabilitas aktif
                              Options:
                                --check, -c             Periksa & bandingkan commit dengan upstream GitHub
                                --json                  Output format JSON terstruktur

    update [check]            Periksa pembaruan commit dan fitur baru dari upstream repo GitHub
                              Options:
                                --json                  Output format JSON terstruktur

    metacog [diag|capabilities|calibration] Observabilitas Metakognisi & Kalibrasi Self-Awareness
                              Subcommands:
                                diag, diagnostics       Representasi diri R, ringkasan Brier score, dan status kalibrasi
                                capabilities            Daftar kontrak tools, limit memori, batas OS, dan domain
                                calibration             Statistik kalibrasi probabilistik (Brier Score, reliability diagram)
                              Options:
                                --domain <domain>       Filter domain tertentu
                                --limit <n>             Batas entri prediksi (default: 10)
                                --json                  Output format JSON terstruktur

    help, --help, -h          Tampilkan panduan ini
"#
        );
    }

    pub async fn run(args: Vec<String>) -> anyhow::Result<()> {
        if args.len() < 2 {
            Self::print_help();
            return Ok(());
        }

        let cmd = args[1].as_str();
        match cmd {
            "help" | "--help" | "-h" => {
                Self::print_help();
                Ok(())
            }
            "version" | "-v" | "--version" => commands::handle_version(&args[2..]).await,
            "update" => commands::handle_update(&args[2..]).await,
            "archive" => commands::handle_archive(&args[2..]),
            "kb" => commands::handle_kb(&args[2..]),
            "workspace" | "ws" => commands::handle_workspace(&args[2..]),
            "clone" => commands::handle_workspace_clone(&args[2..]),
            "sync" => commands::handle_workspace_sync(&args[2..]),
            "link" => commands::handle_workspace_link(&args[2..]),
            "gh-device" => commands::handle_gh_device(&args[2..]),
            "gh-poll" => commands::handle_gh_device(&["poll".to_string()]),
            "gh-login" => commands::handle_gh_login(&args[2..]),
            "audit" => commands::handle_audit(&args[2..]).await,
            "whatsapp" | "wa" => commands::handle_whatsapp(&args[2..]),
            "status" => commands::handle_whatsapp(&args[1..]),
            "user" | "profile" => commands::handle_user(&args[2..]),
            "schedule" | "cron" => commands::handle_schedule(&args[2..]).await,
            "persona" => commands::handle_persona(&args[2..]).await,
            "model" => commands::handle_model(&args[2..]).await,
            "metacog" | "metacognition" => commands::handle_metacognition(&args[2..]).await,
            _ => {
                eprintln!("Subcommand tidak dikenal: `{}`. Ketik `aina help`.", cmd);
                std::process::exit(1);
            }
        }
    }
}
