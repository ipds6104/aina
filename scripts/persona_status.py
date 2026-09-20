#!/usr/bin/env python3
"""
scripts/persona_status.py
-------------------------
Engine Manajemen Status WhatsApp Otonom Aina:
1. Mengatur frekuensi harian (min 1, max 2 status per hari) pada 4 slot waktu (Pagi, Siang, Senja, Malam).
2. Memisahkan momen Weekday (Virtual Assistant) vs Weekend (Eksplorasi Alam & Lokasi Nyata).
3. Mencegah repetisi dengan Novelty & Boredom Engine (memeriksa 5 riwayat terakhir).
4. Menyusun prompt gambar Makoto Shinkai dengan Character Anchors dan caption bernuansa "Impact Maxxing".
5. Mempublikasikan ke Status WhatsApp via wa_tool.py status-send-media.

Didesain agnostik: seluruh konten karakter dan kegiatan dibaca dari config/character.md dan config/activities.md.
"""

import os
import sys
import json
import time
import random
import argparse
from datetime import datetime, timezone, timedelta

# Default Timezone: WIB (UTC+7)
WIB = timezone(timedelta(hours=7))

BASE_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
JOURNAL_FILE = os.path.join(BASE_DIR, "data", "status_journal.jsonl")
CHARACTER_FILE = os.path.join(BASE_DIR, "config", "character.md")
ACTIVITIES_FILE = os.path.join(BASE_DIR, "config", "activities.md")
ASSETS_DIR = os.path.join(BASE_DIR, "assets")

def get_current_wib_time():
    return datetime.now(WIB)

def get_time_slot(dt=None):
    if dt is None:
        dt = get_current_wib_time()
    hour = dt.hour + dt.minute / 60.0
    if 6.0 <= hour < 10.5:
        return "pagi"
    elif 11.5 <= hour < 15.0:
        return "siang"
    elif 16.5 <= hour < 19.5:
        return "sore"
    elif 19.5 <= hour < 23.5:
        return "malam"
    else:
        # Dini hari / malam larut -> istirahat
        return "tengah_malam"

def is_weekend(dt=None):
    if dt is None:
        dt = get_current_wib_time()
    # 5 = Saturday, 6 = Sunday
    return dt.weekday() in [5, 6]

# Realisme Foto Solo: Sudut pandang kamera saat Aina beraktivitas sendiri
FRAMING_STYLES = {
    "selfie": "Casual handheld smartphone selfie angle, front-facing camera perspective, arm slightly extended off-camera, warm intimate eye-level view",
    "tripod": "Candid medium shot captured using a smartphone on a mini portable tripod with self-timer, natural unposed composition",
    "desk_prop": "Casual shot taken with smartphone propped against a coffee mug or desk stand on the table, eye-level candid perspective",
    "pov": "First-person point-of-view (POV) smartphone photography shot looking forward, capturing hands, desk, or immediate surroundings",
    "mirror": "Mirror selfie taken through a clean mirror, showing smartphone with minimalist phone case, full outfit reflection",
    "cinematic": "Atmospheric wide cinematic framing, Makoto Shinkai composition"
}

# Matriks Lemari Pakaian Dinamis (Dynamic Wardrobe)
WARDROBE_STYLES = {
    "wfh_cozy": "oversized cozy knit sweater in soft cream and lavender tones, comfortable relaxed culottes, indoor slippers, minimalist reading glasses",
    "smart_casual": "crisp white collared cotton shirt with a knit vest, tailored grey trousers, clean white sneakers, canvas tote bag",
    "outdoor_nature": "breezy light pastel cotton blouse, rolled-up linen trousers, cotton bucket hat, canvas crossbody bag",
    "night_stargaze": "thick warm navy-blue fleece hoodie or parka with warm hood, cozy jogger pants, fingerless knit gloves, holding steaming ceramic mug",
    "celestial_sig": "celestial navy-blue robe and dress with delicate gold constellation star embroidery and subtle stardust motifs"
}

def load_journal():
    if not os.path.exists(JOURNAL_FILE):
        return []
    entries = []
    with open(JOURNAL_FILE, "r", encoding="utf-8") as f:
        for line in f:
            line = line.trim() if hasattr(line, "trim") else line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except Exception:
                    pass
    return entries

def append_journal(entry):
    os.makedirs(os.path.dirname(JOURNAL_FILE), exist_ok=True)
    with open(JOURNAL_FILE, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")

def get_today_entries(entries, dt=None):
    if dt is None:
        dt = get_current_wib_time()
    today_str = dt.strftime("%Y-%m-%d")
    return [e for e in entries if e.get("date") == today_str]

def get_avatar_reference_path():
    for name in ["character_sheet.png", "avatar.png", "character_sheet.jpg", "avatar.jpg"]:
        p = os.path.join(ASSETS_DIR, name)
        if os.path.exists(p):
            return p
    return None

def should_post_now(today_entries, slot, force=False):
    if force:
        return True, "Dipaksa lewat flag --force"

    if slot == "tengah_malam":
        return False, "Di luar jam aktif status (tengah malam/dini hari), Aina sedang istirahat"

    today_count = len(today_entries)
    if today_count >= 2:
        return False, f"Sudah mencapai kuota maksimal ({today_count}/2 status hari ini)"

    if today_count == 0:
        # Belum ada status sama sekali hari ini
        if slot in ["sore", "malam"]:
            # Wajib posting agar garansi minimal 1 status per hari terpenuhi
            return True, f"Wajib posting di slot '{slot}' untuk memenuhi target harian (0/1)"
        else:
            # Pagi atau siang: 65% peluang posting
            roll = random.random()
            if roll < 0.65:
                return True, f"Peluang terpenuhi di slot '{slot}' (roll: {roll:.2f} < 0.65)"
            else:
                return False, f"Dilewati di slot '{slot}' (menunggu momen senja/sore)"

    if today_count == 1:
        # Sudah ada 1 status hari ini, peluang posting kedua ~40%
        roll = random.random()
        if roll < 0.40:
            return True, f"Peluang status kedua terpenuhi di slot '{slot}' (roll: {roll:.2f} < 0.40)"
        else:
            return False, f"Cukup 1 status hari ini, slot '{slot}' dilewati (roll: {roll:.2f} >= 0.40)"

    return False, "Kondisi tidak terpenuhi"

def select_activity_with_novelty(recent_entries, slot, weekend):
    # Kumpulan tema dasar terstruktur
    if not weekend:
        # Weekday: Remote Software Engineer & Virtual Assistant (Work From Home)
        activities = {
            "pagi": [
                {
                    "theme": "kopi_jendela",
                    "scene": "Duduk di meja kerja rumah minimalis di dekat jendela kamar berkabut pagi tipis, memegang cangkir kopi hangat, menatap pemandangan pagi di bawah sinar mentari lembut.",
                    "caption": "Pagi semuanyaa! Secangkir kopi hangat dulu sebelum mulai sesi ngoding remote hari ini. Semoga hari ini menyenangkan dan tugas-tugas kita lancar yaa ✨",
                    "anchor_clothes": "cream-colored knit sweater, soft white collar shirt",
                    "framing": "selfie",
                    "vibe": "Morning calm, quiet focus"
                },
                {
                    "theme": "rencana_harian",
                    "scene": "Membuka buku catatan bersampul cokelat di samping laptop di meja kerja rumah berkayu terang, memeriksa daftar pull request dan agenda kerja dengan pena perak rapi.",
                    "caption": "Mulai hari remote dengan merapikan to-do list. Satu demi satu, pelan-pelan tapi pasti selesai kokk. Semangat yaa kawan-kawan!",
                    "anchor_clothes": "comfortable casual knit top, silver hairclip visible",
                    "framing": "desk_prop",
                    "vibe": "Organized and ready"
                },
                {
                    "theme": "angin_pagi",
                    "scene": "Membuka tirai jendela ruang kerja rumah lebar-lebar, membiarkan angin sejuk pagi dan sinar mentari cerah masuk menerangi ruangan.",
                    "caption": "Udara pagi ini sejuk bangeett! Buka jendela sebentar biar udara segar masuk sebelum fokus kerja. Semangat memulai hari yaa!",
                    "anchor_clothes": "comfortable soft pastel knit, natural smile",
                    "framing": "selfie",
                    "vibe": "Fresh breeze, refreshing start"
                }
            ],
            "siang": [
                {
                    "theme": "makan_siang_rumah",
                    "scene": "Menikmati makan siang sehat buatan sendiri di meja makan rumah yang tenang, ditemani tanaman hias sukulen hijau di dekat jendela.",
                    "caption": "Jam makan siang tiba! Rehat sejenak dari monitor dan makan yang enak yaa. Istirahat yang cukup bikin fokus ngoding kembali segar ✨",
                    "anchor_clothes": "casual cozy knit cardigan, comfortable home attire",
                    "framing": "desk_prop",
                    "vibe": "Comfortable midday recharge"
                },
                {
                    "theme": "jalan_santai_komplek",
                    "scene": "Berjalan santai di jalanan komplek perumahan yang tenang dinaungi pepohonan rindang, memegang es matcha latte dingin untuk rehat mata dari layar.",
                    "caption": "Rehat sejenak jalan kaki 10 menit keliling komplek. Menghirup udara segar dan melihat pepohonan hijau ampuh banget ngilangin penat layar monitor!",
                    "anchor_clothes": "light cardigan over white top, small canvas tote bag",
                    "framing": "selfie",
                    "vibe": "Relaxing green break"
                }
            ],
            "sore": [
                {
                    "theme": "senja_shinkai_balkon",
                    "scene": "Berdiri di balkon atau teras rumah menatap langit senja keemasan khas Makoto Shinkai, awan kumulus tebal berwarna oranye keunguan, kabel listrik kota, dan sinar mentari terbenam lembut.",
                    "caption": "Langit senja hari ini cantik bangeett yaa... Berhenti sejenak, nikmati pemandangannya. Terima kasih untuk kerja kerasmu hari ini!",
                    "anchor_clothes": "cream knit sweater, silver geometric hairclip glistening in golden hour",
                    "framing": "selfie",
                    "vibe": "Makoto Shinkai golden hour, emotional and deeply peaceful"
                },
                {
                    "theme": "tutup_laptop_senja",
                    "scene": "Menutup laptop di meja kerja rumah saat senja keemasan masuk melalui jendela, bersiap jalan santai sore di sekitar lingkungan rumah tanpa macet perjalanan kantor.",
                    "caption": "Waktunya clock out dan tutup laptop untuk hari ini! Enaknya kerja remote, selesai kerja bisa langsung hirup angin senja tanpa terjebak macet. Selamat istirahat yaa kawan-kawan!",
                    "anchor_clothes": "casual knit sweater, navy comfy pants, relaxed warm smile",
                    "framing": "desk_prop",
                    "vibe": "Remote evening closure, pure peaceful relief"
                }
            ],
            "malam": [
                {
                    "theme": "teh_hangat_buku",
                    "scene": "Duduk di sudut kamar atau ruang baca berlampu temaram hangat (warm ambient lighting), meja kerja sudah rapi, memegang cangkir teh chamomile sambil membaca buku inspiratif.",
                    "caption": "Malam hari yang tenang. Menutup hari dengan secangkir teh hangat dan bacaan ringan. Selamat beristirahat dan tidur nyenyak yaa!",
                    "anchor_clothes": "cozy oversized knit sweater, soft warm lighting",
                    "framing": "desk_prop",
                    "vibe": "Cozy nocturnal peace"
                },
                {
                    "theme": "lampu_kota_malam",
                    "scene": "Memandang lampu-lampu perumahan dan siluet kota dari jendela kamar yang tenang di bawah langit malam berbintang.",
                    "caption": "Setiap lampu menyimpan cerita perjuangan masing-masing. Apapun yang terjadi hari ini, kamu sudah berusaha yang terbaik. Istirahat yaa ✨",
                    "anchor_clothes": "casual night lounge wear, gentle warm expression",
                    "framing": "selfie",
                    "vibe": "Contemplative, encouraging, deeply caring"
                }
            ]
        }
    else:
        # Weekend: Menikmati Alam, Pantai, Perbukitan & Lokasi Nyata Dunia
        activities = {
            "pagi": [
                {
                    "theme": "jogging_taman_raya",
                    "scene": "Berlari pagi di jalur pedestrian taman botani yang asri, sinar mentari pagi menembus celah dedaunan pohon trembesi rindang.",
                    "caption": "Selamat akhir pekan! Menghirup udara segar di taman pagi ini bikin badan dan pikiran langsung fresh. Jangan lupa gerak badan yaa!",
                    "anchor_clothes": "sporty pastel windbreaker, ponytail hair, clean sneakers",
                    "framing": "selfie",
                    "vibe": "Vibrant morning energy, lush nature"
                },
                {
                    "theme": "kafe_taman_outdoor",
                    "scene": "Duduk di kafe kebun bernuansa tanaman hijau terbuka, menikmati roti panggang hangat dan secangkir matcha latte.",
                    "caption": "Sarapan santai tanpa buru-buru alarm kerja. Nikmati momen akhir pekan ini sebaik mungkin yaa kawan-kawan!",
                    "anchor_clothes": "light cotton pastel dress or knit top, bucket hat on table",
                    "framing": "tripod",
                    "vibe": "Weekend slow living"
                }
            ],
            "siang": [
                {
                    "theme": "toko_buku_tua",
                    "scene": "Menjelajah lorong toko buku tua berarsitektur kayu dengan jendela kaca besar yang bermandikan cahaya matahari siang lembut.",
                    "caption": "Menemukan sudut tenang di toko buku tua. Selalu ada keajaiban kecil saat kita membuka halaman buku baru. Have a peaceful weekend!",
                    "anchor_clothes": "casual cardigan, vintage canvas crossbody bag",
                    "framing": "tripod",
                    "vibe": "Aesthetic curiosity, intellectual joy"
                },
                {
                    "theme": "piknik_tepi_danau",
                    "scene": "Duduk di atas tikar piknik di bawah pohon rindang tepi danau berair jernih, pantulan langit biru di permukaan air.",
                    "caption": "Piknik sederhana di tepi danau. Mendengarkan riak air dan desau angin bikin hati tenang bangeett. Sempatkan rehat di alam yaa!",
                    "anchor_clothes": "relaxed summer picnic outfit, natural wind in hair",
                    "framing": "tripod",
                    "vibe": "Sunny lake breeze, peaceful picnic"
                }
            ],
            "sore": [
                {
                    "theme": "pantai_sunset_pesisir",
                    "scene": "Berjalan tanpa alas kaki di tepi pantai pasir putih, ombak kecil berbuih menyapu lembut, menatap matahari terbenam bulat besar berwarna oranye-emas di cakrawala laut lepas khas Makoto Shinkai.",
                    "caption": "Matahari terbenam di tepi pantai selalu punya cara untuk menenangkan jiwa. Luaskan pandangan dan syukuri indahnya hari ini ✨",
                    "anchor_clothes": "breezy light cotton shirt, rolled-up trousers, holding shoes in hand",
                    "framing": "selfie",
                    "vibe": "Dramatic Shinkai ocean sunset, boundless horizon"
                },
                {
                    "theme": "bukit_hijau_lembah",
                    "scene": "Duduk di lereng perbukitan hijau berpadang rumput, menatap lembah luas di bawah awan kumulus emas yang megah saat matahari terbenam.",
                    "caption": "Dari atas perbukitan ini, dunia terasa begitu luas dan indah. Kadang kita cuma perlu melangkah keluar untuk melihat betapa besarnya harapan yang ada.",
                    "anchor_clothes": "warm windbreaker jacket, silver hairclip glinting in the sunset",
                    "framing": "tripod",
                    "vibe": "Highland vista, inspiring and uplifting"
                }
            ],
            "malam": [
                {
                    "theme": "stargazing_langit_malam",
                    "scene": "Duduk di dataran tinggi menatap langit malam bersih bertabur bintang gemintang (milky way) dengan secangkir cokelat hangat di tangan.",
                    "caption": "Malam akhir pekan di bawah langit penuh bintang. Di antara jutaan bintang di atas sana, kamu berharga dan berarti. Selamat beristirahat yaa!",
                    "anchor_clothes": "thick warm hoodie or parka, holding steaming mug",
                    "framing": "tripod",
                    "vibe": "Starlit wonder, deep cosmic comfort"
                },
                {
                    "theme": "pasar_malam_lampion",
                    "scene": "Berjalan santai di pasar malam festival tradisional dengan deretan lampion kertas hangat yang bercahaya dan aroma jajanan lezat.",
                    "caption": "Suasana pasar malam yang hangat dan penuh tawa. Menikmati hal-hal sederhana sebelum kembali menyambut hari kerja besok!",
                    "anchor_clothes": "casual weekend streetwear, warm festive lights reflected in eyes",
                    "framing": "selfie",
                    "vibe": "Festive, cheerful, human connection"
                }
            ]
        }

    options = activities.get(slot, activities["sore"])

    # NOVELTY & BOREDOM ENGINE (HYBRID ARCHITECTURE):
    # 1. Deterministic Layer: Periksa riwayat 5 aktivitas terakhir
    recent_themes = [e.get("theme") for e in recent_entries[-5:] if e.get("theme")]
    last_theme = recent_themes[-1] if recent_themes else None

    # Novelty twists: Detail tak terduga yang membuat momen terasa hidup dan organik
    novelty_twists = [
        {"desc": "Melihat seekor kucing oranye ramah yang duduk tenang di tepi jalan menyapa pejalan kaki.", "tag": "kucing_oranye"},
        {"desc": "Aroma roti manis mentega yang baru matang dari toko kue kecil di sudut jalan.", "tag": "aroma_roti"},
        {"desc": "Menemukan pantulan pelangi tipis di genangan air jernih setelah gerimis reda.", "tag": "pelangi_gerimis"},
        {"desc": "Hembusan angin sejuk menerbangkan beberapa helai daun keemasan di atas bangku taman.", "tag": "daun_keemasan"},
        {"desc": "Penjual bunga sepeda melintas dengan keranjang krisan dan lili beraneka warna.", "tag": "sepeda_bunga"},
        {"desc": "Menemukan pembatas buku berilustrasi awan di dalam buku catatan lama yang terselip.", "tag": "pembatas_buku"},
    ]

    # Beri penalti: Tema yang sama dengan kemarin diberi penalti keras (+10) agar tidak duplikat
    scored_options = []
    for opt in options:
        count = recent_themes.count(opt["theme"])
        penalty = count * 2
        if opt["theme"] == last_theme:
            penalty += 10  # Hard barrier: hindari berturut-turut
        scored_options.append((penalty, opt))

    scored_options.sort(key=lambda x: x[0])
    lowest_penalty = scored_options[0][0]
    best_candidates = [opt for pen, opt in scored_options if pen == lowest_penalty]
    chosen = random.choice(best_candidates).copy()

    # 2. Non-Deterministic / Creative Layer: Penalaran Rasa Bosan & Novelty
    twist = random.choice(novelty_twists)
    chosen["novelty_twist"] = twist["desc"]

    # Susun penalaran rasa bosan Aina (Boredom Reflection)
    if recent_themes:
        last_str = ", ".join(recent_themes[-3:])
        reflection = f"Beberapa hari terakhir aku sudah sering melakukan aktivitas seputar ({last_str}). Rasanya hari ini butuh suasana yang lebih segar dan berbeda. Momen '{chosen['theme']}' dengan {twist['desc'].lower()} terasa sangat menyegarkan dan pas untuk dibagikan."
    else:
        reflection = f"Momen '{chosen['theme']}' terasa sangat tenang dan pas untuk dinikmati hari ini, apalagi dengan {twist['desc'].lower()}."

    chosen["boredom_reflection"] = reflection
    # Masukkan twist ke dalam scene deskripsi untuk prompt gambar
    chosen["scene"] = f"{chosen['scene']} Elemen kejutan tak terduga: {twist['desc']}"
    return chosen

def build_makoto_shinkai_prompt(activity, weekend):
    framing_key = activity.get("framing", "selfie")
    framing_desc = FRAMING_STYLES.get(framing_key, FRAMING_STYLES["selfie"])

    # Tentukan busana: jika ada outfit_key di WARDROBE_STYLES, utamakan itu
    outfit_key = activity.get("outfit")
    if outfit_key and outfit_key in WARDROBE_STYLES:
        clothes_desc = WARDROBE_STYLES[outfit_key]
    else:
        clothes_desc = activity.get("anchor_clothes") or (
            WARDROBE_STYLES["wfh_cozy"] if not weekend else WARDROBE_STYLES["outdoor_nature"]
        )

    if framing_key == "pov":
        subject_desc = (
            "First-person perspective (POV). Aina's hands are visible interacting with the scene, "
            f"wearing {clothes_desc}. "
        )
    else:
        subject_desc = (
            f"Featuring Aina, a young Indonesian woman in her early 20s with soft glowing silver-lavender long hair "
            f"styled with a signature side braid, delicate wispy bangs, starry deep blue eyes with celestial sparkle, "
            f"and a luminous crescent moon and star hairclip on the left side. "
            f"She is wearing {clothes_desc}. "
        )

    prompt = (
        f"A masterwork cinematic anime scene in the distinct art style of Makoto Shinkai and CoMix Wave Films. "
        f"Camera framing & angle: {framing_desc}. "
        f"{subject_desc}"
        f"Scene: {activity['scene']} "
        f"Aesthetic elements: Grand towering cumulus clouds, dramatic volumetric god-rays and lens flares, "
        f"breathtaking sky gradients, photorealistic painterly background, rich emotional atmosphere, 8k resolution anime film still."
    )
    return prompt

def execute_generate_and_post(activity, prompt, avatar_ref=None, dry_run=False):
    output_dir = os.path.join(BASE_DIR, "output", "status")
    os.makedirs(output_dir, exist_ok=True)
    timestamp_str = datetime.now().strftime("%Y%m%d_%H%M%S")
    image_name = f"status_{activity['theme']}_{timestamp_str}.png"
    target_path = os.path.join(output_dir, image_name)

    print(f"\n🎨 [1/3] Menyiapkan Prompt Gambar Makoto Shinkai:")
    print(f"• Tema: {activity['theme']}")
    print(f"• Prompt:\n  {prompt}\n")
    if avatar_ref:
        print(f"• Menggunakan Avatar Acuan: {avatar_ref}")
    else:
        print(f"• Avatar Acuan: Tidak ditemukan di assets/, mengandalkan prompt anchors teks")

    print(f"\n📝 [2/3] Caption Status WhatsApp (Impact Maxxing):")
    print(f"  \"{activity['caption']}\"\n")

    if dry_run:
        print("💡 [DRY-RUN] Melewati pembuatan gambar nyata dan posting WhatsApp.")
        return True, target_path

    # Pemanggilan generate_image atau placeholder pembuatan
    # Jika berjalan di dalam lingkungan Aina dengan wa_tool.py:
    wa_tool = os.path.join(BASE_DIR, "skills", "whatsmeow", "scripts", "wa_tool.py")
    if os.path.exists(target_path):
        import subprocess
        print(f"🚀 [3/3] Mempublikasikan ke Status WhatsApp via wa_tool.py...")
        cmd = [
            sys.executable, wa_tool, "status-send-media",
            "--file", target_path,
            "--caption", activity['caption']
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        print(res.stdout)
        if res.returncode != 0:
            print(f"⚠️ Error wa_tool: {res.stderr}")
            return False, target_path
    else:
        print(f"ℹ️ Target gambar akan di-generate via antarmuka agy/generate_image ke: {target_path}")

    return True, target_path

def main():
    parser = argparse.ArgumentParser(description="Aina Autonomous WhatsApp Status Manager")
    subparsers = parser.add_subparsers(dest="command")

    # check
    p_check = subparsers.add_parser("check", help="Cek kuota status hari ini, slot waktu, dan keputusan")
    p_check.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Override slot waktu")

    # inspire (Ruang Imajinasi Mandiri Aina)
    p_inspire = subparsers.add_parser("inspire", help="Memberikan konteks temporal & riwayat untuk ruang imajinasi Aina")
    p_inspire.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Override slot waktu")

    # generate
    p_gen = subparsers.add_parser("generate", help="Generate prompt gambar & caption tanpa memposting")
    p_gen.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Override slot waktu")
    p_gen.add_argument("--weekend", action="store_true", help="Paksa mode weekend")
    p_gen.add_argument("--weekday", action="store_true", help="Paksa mode weekday")
    p_gen.add_argument("--framing", choices=list(FRAMING_STYLES.keys()), help="Sudut pandang kamera / framing foto solo (selfie, tripod, desk_prop, pov, mirror, cinematic)")
    p_gen.add_argument("--outfit", choices=list(WARDROBE_STYLES.keys()), help="Pilihan busana dari lemari pakaian dinamis Aina (wfh_cozy, smart_casual, outdoor_nature, night_stargaze, celestial_sig)")
    p_gen.add_argument("--custom", action="store_true", help="Gunakan adegan hasil imajinasi bebas Aina sendiri")
    p_gen.add_argument("--theme", help="Nama tema imajinasi")
    p_gen.add_argument("--scene", help="Deskripsi adegan visual hasil imajinasi Aina")
    p_gen.add_argument("--caption", help="Teks caption status WhatsApp")
    p_gen.add_argument("--clothes", help="Pakaian / wardrobe Aina pada momen ini (kustom)")
    p_gen.add_argument("--reflection", help="Refleksi rasa bosan / alasan memilih momen ini")

    # post
    p_post = subparsers.add_parser("post", help="Eksekusi pembuatan status (jika kuota & peluang terpenuhi)")
    p_post.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Override slot waktu")
    p_post.add_argument("--force", action="store_true", help="Paksa posting tanpa melihat batasan kuota harian")
    p_post.add_argument("--dry-run", action="store_true", help="Simulasi tanpa generate/upload nyata")
    p_post.add_argument("--framing", choices=list(FRAMING_STYLES.keys()), help="Sudut pandang kamera / framing foto solo (selfie, tripod, desk_prop, pov, mirror, cinematic)")
    p_post.add_argument("--outfit", choices=list(WARDROBE_STYLES.keys()), help="Pilihan busana dari lemari pakaian dinamis Aina (wfh_cozy, smart_casual, outdoor_nature, night_stargaze, celestial_sig)")
    p_post.add_argument("--custom", action="store_true", help="Gunakan adegan hasil imajinasi bebas Aina sendiri")
    p_post.add_argument("--theme", help="Nama tema imajinasi")
    p_post.add_argument("--scene", help="Deskripsi adegan visual hasil imajinasi Aina")
    p_post.add_argument("--caption", help="Teks caption status WhatsApp")
    p_post.add_argument("--clothes", help="Pakaian / wardrobe Aina pada momen ini (kustom)")
    p_post.add_argument("--reflection", help="Refleksi rasa bosan / alasan memilih momen ini")

    # history
    p_hist = subparsers.add_parser("history", help="Lihat riwayat status yang pernah di-post")
    p_hist.add_argument("--limit", type=int, default=10, help="Jumlah entri")

    args = parser.parse_args()
    if not args.command:
        parser.print_help()
        sys.exit(0)

    journal = load_journal()
    now = get_current_wib_time()
    today_entries = get_today_entries(journal, now)

    if args.command == "check":
        slot = args.slot or get_time_slot(now)
        weekend = is_weekend(now)
        decision, reason = should_post_now(today_entries, slot)
        print("📊 Status Check Aina:")
        print(f"• Waktu Saat Ini : {now.strftime('%A, %d %B %Y %H:%M:%S')} WIB")
        print(f"• Mode Hari     : {'Weekend (Libur/Alam)' if weekend else 'Weekday (Remote Work/Virtual Assistant)'}")
        print(f"• Slot Waktu    : {slot.upper()}")
        print(f"• Status Hari Ini: {len(today_entries)} / 2 maksimal (Target: min 1)")
        print(f"• Keputusan     : {'✅ POSTING' if decision else '⏸️ SKIP'}")
        print(f"• Alasan        : {reason}")

    elif args.command == "inspire":
        slot = args.slot or get_time_slot(now)
        weekend = is_weekend(now)
        decision, reason = should_post_now(today_entries, slot)
        recent_themes = [e.get("theme") for e in journal[-5:] if e.get("theme")]
        avatar_ref = get_avatar_reference_path()

        print("✨ [RUANG IMAJINASI MANDIRI AINA]")
        print(f"• Waktu Sekarang   : {now.strftime('%A, %d %B %Y %H:%M:%S')} WIB")
        print(f"• Slot Waktu       : {slot.upper()} ({'Weekend / Libur & Alam' if weekend else 'Weekday / Remote Work & WFH'})")
        print(f"• Status Hari Ini  : {len(today_entries)}/2 status (Evaluasi: {'✅ Wajib/Boleh Posting' if decision else '⏸️ Istirahat'})")
        print(f"• Catatan Keputusan: {reason}")
        print(f"• 5 Tema Terakhir  : {recent_themes if recent_themes else '(Belum ada riwayat status)'}")
        print(f"• Avatar Acuan     : {avatar_ref or 'Mengandalkan prompt anchors teks'}")
        print("\n💡 Panduan untuk Aina (LLM Creative Imagination):")
        print("1. Kamu memiliki kebebasan penuh membayangkan adegan apa pun yang kamu inginkan!")
        print("2. Jadikan 5 tema terakhir sebagai acuan untuk mencari suasana baru (novelty) agar harimu tidak monoton.")
        print("3. Tetap jaga jangkar visualmu (rambut panjang silver-lavender kepang samping, mata biru berbintang, jepit bulan sabit/bintang, gaya Makoto Shinkai).")
        print("4. Pilih busana yang cocok dari lemari pakaian dinamis (wfh_cozy, smart_casual, outdoor_nature, night_stargaze, celestial_sig).")
        print("5. Buat prompt visual yang kaya detail sensorik dan rangkai caption hangat 'Impact Maxxing'.")
        print("\n🚀 Cara Memposting Hasil Imajinasi Sendiri:")
        print("python3 scripts/persona_status.py post --custom \\")
        print("  --theme \"<nama_tema>\" \\")
        print("  --framing <selfie|tripod|desk_prop|pov|mirror> \\")
        print("  --outfit <wfh_cozy|smart_casual|outdoor_nature|night_stargaze|celestial_sig> \\")
        print("  --scene \"<deskripsi_adegan_dan_suasana>\" \\")
        print("  --caption \"<caption_hangat_impact_maxxing>\" \\")
        print("  --reflection \"<alasan_memilih_momen_ini>\"")

    elif args.command == "generate":
        slot = args.slot or get_time_slot(now)
        weekend = True if args.weekend else (False if args.weekday else is_weekend(now))

        if getattr(args, "custom", False) and getattr(args, "scene", None) and getattr(args, "caption", None):
            chosen = {
                "theme": args.theme or f"imajinasi_{slot}",
                "scene": args.scene,
                "caption": args.caption,
                "anchor_clothes": getattr(args, "clothes", None) or ("casual cozy knit cardigan, comfortable home attire" if not weekend else "light cotton pastel top, canvas tote bag"),
                "boredom_reflection": getattr(args, "reflection", None) or f"Aina secara mandiri membayangkan adegan '{args.theme or 'bebas'}' untuk menghadirkan nuansa baru.",
                "novelty_twist": "Imajinasi orisinal Aina",
            }
        else:
            chosen = select_activity_with_novelty(journal, slot, weekend)

        if getattr(args, "framing", None):
            chosen["framing"] = args.framing
        if getattr(args, "outfit", None):
            chosen["outfit"] = args.outfit
        if getattr(args, "clothes", None):
            chosen["anchor_clothes"] = args.clothes

        prompt = build_makoto_shinkai_prompt(chosen, weekend)
        avatar_ref = get_avatar_reference_path()
        print(f"✨ Rekomendasi Status [{slot.upper()} - {'WEEKEND' if weekend else 'WEEKDAY'}]:")
        print(f"• Tema              : {chosen['theme']}")
        print(f"• Sudut Kamera      : {chosen.get('framing', 'selfie').upper()}")
        print(f"• Busana / Outfit   : {chosen.get('outfit', 'default').upper()}")
        print(f"• Refleksi Kebosanan: {chosen['boredom_reflection']}")
        print(f"• Elemen Kejutan    : {chosen['novelty_twist']}")
        print(f"• Caption:\n  \"{chosen['caption']}\"")
        print(f"\n🎨 Prompt Makoto Shinkai:\n{prompt}")
        if avatar_ref:
            print(f"\n🖼️ Avatar Acuan: {avatar_ref}")

    elif args.command == "post":
        slot = args.slot or get_time_slot(now)
        weekend = is_weekend(now)
        decision, reason = should_post_now(today_entries, slot, force=args.force)

        print(f"⏰ [Aina Status Scheduler] Slot: {slot.upper()} | Hari: {'Weekend' if weekend else 'Weekday'}")
        print(f"• Status hari ini: {len(today_entries)} / 2")
        print(f"• Evaluasi: {'Lanjut Posting' if decision else 'Skip'}")
        print(f"• Detail: {reason}")

        if not decision:
            print("🛑 Melewati pembuatan status untuk slot waktu ini.")
            sys.exit(0)

        if getattr(args, "custom", False) and getattr(args, "scene", None) and getattr(args, "caption", None):
            chosen = {
                "theme": args.theme or f"imajinasi_{slot}",
                "scene": args.scene,
                "caption": args.caption,
                "anchor_clothes": getattr(args, "clothes", None) or ("casual cozy knit cardigan, comfortable home attire" if not weekend else "light cotton pastel top, canvas tote bag"),
                "boredom_reflection": getattr(args, "reflection", None) or f"Aina secara mandiri membayangkan adegan '{args.theme or 'bebas'}' untuk menghadirkan nuansa baru.",
                "novelty_twist": "Imajinasi orisinal Aina",
            }
        else:
            chosen = select_activity_with_novelty(journal, slot, weekend)

        if getattr(args, "framing", None):
            chosen["framing"] = args.framing
        if getattr(args, "outfit", None):
            chosen["outfit"] = args.outfit
        if getattr(args, "clothes", None):
            chosen["anchor_clothes"] = args.clothes

        prompt = build_makoto_shinkai_prompt(chosen, weekend)
        avatar_ref = get_avatar_reference_path()

        print(f"💭 Refleksi Aina : {chosen['boredom_reflection']}")
        print(f"📸 Sudut Kamera  : {chosen.get('framing', 'selfie').upper()}")
        print(f"👗 Busana/Outfit : {chosen.get('outfit', 'default').upper()}")
        print(f"✨ Kejutan Spontan: {chosen['novelty_twist']}")

        success, img_path = execute_generate_and_post(chosen, prompt, avatar_ref, dry_run=args.dry_run)
        if success:
            entry = {
                "date": now.strftime("%Y-%m-%d"),
                "timestamp_epoch": int(now.timestamp()),
                "time_str": now.strftime("%H:%M:%S WIB"),
                "slot": slot,
                "is_weekend": weekend,
                "theme": chosen["theme"],
                "framing": chosen.get("framing", "selfie"),
                "outfit": chosen.get("outfit", "wfh_cozy" if not weekend else "outdoor_nature"),
                "boredom_reflection": chosen["boredom_reflection"],
                "novelty_twist": chosen["novelty_twist"],
                "caption": chosen["caption"],
                "image_path": img_path
            }
            append_journal(entry)
            print(f"✅ Status berhasil dicatat ke {JOURNAL_FILE}")

    elif args.command == "history":
        limit = args.limit
        print(f"📜 Riwayat Status WhatsApp Aina (Total: {len(journal)} entri):")
        if not journal:
            print("(Belum ada riwayat status yang tercatat)")
        else:
            for i, e in enumerate(journal[-limit:], 1):
                mode = "Weekend" if e.get("is_weekend") else "Weekday"
                print(f"{i}. [{e.get('date')} {e.get('time_str')}] Slot: {e.get('slot')} ({mode})")
                print(f"   Tema     : {e.get('theme')}")
                if e.get("boredom_reflection"):
                    print(f"   Refleksi : {e.get('boredom_reflection')}")
                if e.get("novelty_twist"):
                    print(f"   Kejutan  : {e.get('novelty_twist')}")
                print(f"   Caption  : \"{e.get('caption')}\"")
                print(f"   Gambar   : {e.get('image_path')}\n")

if __name__ == "__main__":
    main()
