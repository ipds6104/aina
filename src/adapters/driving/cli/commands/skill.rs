use std::path::PathBuf;
use std::process::Command;

/// Mencari lokasi script skill_scaffolder.py di sistem
fn find_skill_scaffolder_script() -> Option<PathBuf> {
    let candidates = [
        // 1. Relatif dari direktori kerja
        PathBuf::from("skills/skill-builder/scripts/skill_scaffolder.py"),
        // 2. Relatif dari binary
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                exe_dir.join("../skills/skill-builder/scripts/skill_scaffolder.py")
            } else {
                PathBuf::new()
            }
        } else {
            PathBuf::new()
        },
        // 3. Lokasi standar container / Linux
        PathBuf::from("/app/skills/skill-builder/scripts/skill_scaffolder.py"),
        PathBuf::from("/usr/local/bin/skill_scaffolder"),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub fn handle_skill(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        print_skill_help();
        return Ok(());
    }

    let script_path = find_skill_scaffolder_script().ok_or_else(|| {
        anyhow::anyhow!(
            "Skrip `skill_scaffolder.py` tidak ditemukan di `skills/skill-builder/scripts/` maupun `/usr/local/bin/`."
        )
    })?;

    // Jalankan skill_scaffolder.py via python3
    let status = Command::new("python3")
        .arg(&script_path)
        .args(args)
        .status()?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }

    Ok(())
}

fn print_skill_help() {
    println!(
        r#"Aina Skill - Manajemen & Scaffolding Custom Skills Persisten

PENGGUNAAN:
    aina skill [SUBCOMMAND] [OPTIONS]

SUBCOMMANDS:
    list                      Daftar seluruh custom skill yang terpasang di persistent volume
    init <name> [options]     Scaffold skill kustom baru di storage persisten (`data/custom-skills/`)
                              Options:
                                --template, -t <type>   Template arsitektur: minimal, api, cli, data (default: minimal)
                                --desc, -d <text>       Deskripsi fungsi dan tujuan skill
                                --force, -f             Timpa direktori skill jika sudah ada

    install <url> [options]   Install custom skill dari repositori Git atau folder lokal
                              Options:
                                --skill, -s <name>      Nama skill spesifik yang ingin diinstall (dari multi-skill repo)
                                --branch, -b <branch>   Branch atau tag Git yang ingin di-clone
                                --all, -a               Install seluruh skill yang ditemukan di repositori
                                --force, -f             Timpa direktori skill jika sudah ada

    test <name>               Jalankan pengujian otomatis (syntax compile, --help, status check)
    validate <name>           Validasi kepatuhan YAML frontmatter, izin eksekusi, dan OpSec
    delete <name>             Hapus skill kustom dari volume persisten
                              Options:
                                --force, -f             Konfirmasi penghapusan permanen

    sync [options]            Sinkronisasi custom skills ke remote Git repository (USER_SKILLS_REPO)
                              Options:
                                --repo, -r <url>        URL Git repository tujuan
                                --message, -m <msg>     Pesan commit git

CONTOH:
    aina skill init tally-api --template api --desc "Integrasi formulir Tally.so"
    aina skill install https://github.com/ihkaru/agent-skills --skill arc42-architect
    aina skill test tally-api
    aina skill validate tally-api
    aina skill list
"#
    );
}
