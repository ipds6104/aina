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

# Sisipkan direktori skrip ke sys.path agar modul persona dapat diimpor langsung
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)
_BASE_ROOT = os.path.abspath(os.path.join(_SCRIPT_DIR, ".."))
if _BASE_ROOT not in sys.path:
    sys.path.insert(0, _BASE_ROOT)

from persona import (
    StatusSafetyGuard,
    AtmosphereEngine,
    WardrobeManager,
    WARDROBE_PRESETS,
    ImageCaptionEngine,
)

# Default Timezone: WIB (UTC+7)
WIB = timezone(timedelta(hours=7))

BASE_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
JOURNAL_FILE = os.path.join(BASE_DIR, "data", "status_journal.jsonl")
CHARACTER_FILE = os.path.join(BASE_DIR, "config", "character.md")
ASSETS_DIR = os.path.join(BASE_DIR, "assets")
DATA_ASSETS_DIR = os.path.join(BASE_DIR, "data", "assets")

def get_current_wib_time():
    return datetime.now(WIB)

def get_time_slot(dt=None):
    if dt is None:
        dt = get_current_wib_time()
    hour = dt.hour + dt.minute / 60.0
    if 6.0 <= hour < 11.0:
        return "pagi"
    elif 11.0 <= hour < 15.5:
        return "siang"
    elif 15.5 <= hour < 19.0:
        return "sore"
    elif 19.0 <= hour < 23.0:
        return "malam"
    else:
        # 23:00 - 06:00: Dini hari / malam larut -> istirahat
        return "tengah_malam"

def is_weekend(dt=None):
    if dt is None:
        dt = get_current_wib_time()
    # 5 = Saturday, 6 = Sunday
    return dt.weekday() in [5, 6]

# Realisme Foto Solo: Sudut pandang kamera saat Aina beraktivitas sendiri
FRAMING_STYLES = {
    "selfie": "Casual handheld smartphone selfie angle, front-facing camera perspective, arm reaching slightly off-frame, intimate eye-level view, clean composition without selfie stick",
    "tripod": "Candid medium shot from across the scene, hands-free self-timer photograph, natural unposed composition from stable eye-level camera placement, clean composition with no tripod and no camera equipment visible in frame",
    "desk_prop": "Casual tabletop eye-level candid perspective, hands-free self-timer composition, natural unposed shot, clean foreground with no camera gear or phone prop visible",
    "pov": "First-person point-of-view (POV) smartphone photography shot looking forward, capturing hands, desk, or immediate surroundings",
    "mirror": "Mirror selfie taken through a clean mirror, showing smartphone with minimalist phone case, full outfit reflection",
    "cinematic": "Atmospheric wide cinematic framing, Makoto Shinkai composition"
}

# Matriks Lemari Pakaian Dinamis (Dynamic Wardrobe) dari WardrobeManager
WARDROBE_STYLES = WARDROBE_PRESETS

def load_journal():
    if not os.path.exists(JOURNAL_FILE):
        return []
    entries = []
    with open(JOURNAL_FILE, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
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
    # Prioritas:
    # 1. File kustom di persistent volume: data/assets/ (character_sheet / avatar)
    # 2. File kustom di folder assets/: assets/ (character_sheet / avatar)
    # 3. Fallback default repo: assets/character_sheet.default.png
    candidates = [
        "character_sheet.png",
        "avatar.png",
        "character_sheet.jpg",
        "avatar.jpg",
    ]
    # Cek di data/assets/ (persistent volume)
    for name in candidates:
        p = os.path.join(DATA_ASSETS_DIR, name)
        if os.path.exists(p):
            return p
    # Cek di assets/
    for name in candidates:
        p = os.path.join(ASSETS_DIR, name)
        if os.path.exists(p):
            return p
    # Fallback default template
    for name in ["character_sheet.default.png", "character_sheet.default.jpg"]:
        for d in [DATA_ASSETS_DIR, ASSETS_DIR]:
            p = os.path.join(d, name)
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

def detect_boredom_state(recent_entries, weekend, dt=None):
    if dt is None:
        dt = get_current_wib_time()

    recent_themes = [e.get("theme") for e in recent_entries[-5:] if e.get("theme")]
    theme_counts = {t: recent_themes.count(t) for t in set(recent_themes)}
    max_theme_rep = max(theme_counts.values()) if theme_counts else 0
    unique_themes = len(set(recent_themes))

    boredom_score = 0
    reasons = []

    if max_theme_rep >= 2:
        boredom_score += 40
        reasons.append(f"Tema berulang terdeteksi ({max_theme_rep}x dalam 5 status terakhir)")

    if len(recent_themes) >= 4 and unique_themes <= 2:
        boredom_score += 35
        reasons.append(f"Variasi tema rendah ({unique_themes} tema unik dari {len(recent_themes)} status)")

    # Evaluasi kebosanan busana via WardrobeManager
    w_boredom = WardrobeManager.detect_outfit_boredom(recent_entries, dt)
    if w_boredom["is_bored"]:
        boredom_score += w_boredom["boredom_score"]
        reasons.extend(w_boredom["reasons"])

    boredom_triggered = boredom_score >= 40 or (len(recent_themes) >= 3 and max_theme_rep >= 2) or w_boredom["is_bored"]

    if weekend:
        suggested_queries = [
            "fenomena astronomi langit malam ini indonesia",
            "spot wisata alam bukit jamur bengkayang kalimantan barat",
            "pantai pasir panjang singkawang matahari terbenam",
            "toko buku tua perpustakaan kafe tanaman hijau",
            "spot piknik tepi danau hutan pinus tenang"
        ]
    else:
        suggested_queries = [
            "resep teh herbal menenangkan kerja remote chamomile mint",
            "tanaman hias sukulen meja kerja minimalis indoor",
            "playlist musik lofi ambient fokus coding malam",
            "setup meja kerja ergonomis minimalis hangat kayu",
            "kafe lokal bernuansa perpustakaan buku tenang"
        ]

    return {
        "boredom_score": min(boredom_score, 100),
        "is_triggered": boredom_triggered,
        "reasons": reasons,
        "recent_themes": recent_themes,
        "recent_outfits": w_boredom["recent_outfits"],
        "wardrobe_boredom": w_boredom,
        "suggested_search_queries": suggested_queries,
        "selected_query": random.choice(suggested_queries)
    }

def select_activity_with_novelty(recent_entries, slot, weekend, search_query=None, custom_clothes=None, requested_outfit=None):
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
                    "framing": "selfie",
                    "vibe": "Warm home meal, restful break"
                },
                {
                    "theme": "jalan_keliling_komplek",
                    "scene": "Jalan kaki santai di jalan komplek perumahan yang rindang di bawah dedaunan hijau, udara cerah berawan, memegang botol minum tumbler perak.",
                    "caption": "Jalan kaki 15 menit keliling komplek sehabis makan siang. Rasanya segar bangett habis kena angin sepoi-sepoi!",
                    "anchor_clothes": "light casual cotton shirt, comfortable walking sneakers",
                    "framing": "selfie",
                    "vibe": "Light movement, refreshing outdoors"
                },
                {
                    "theme": "matcha_kafe_lokal",
                    "scene": "Duduk di sudut kafe lokal bernuansa kayu hangat dan tanaman monstera hijau, memegang segelas es matcha latte dengan laptop terbuka di meja.",
                    "caption": "Pindah suasana kerja ke kafe dekat rumah sambil pesan es matcha latte. Kadang ganti suasana kerja bikin ide-ide baru bermunculan!",
                    "anchor_clothes": "smart casual button-up blouse, cream cardigan",
                    "framing": "desk_prop",
                    "vibe": "Cafe focus, soothing green ambience"
                }
            ],
            "sore": [
                {
                    "theme": "golden_hour_balkon",
                    "scene": "Berdiri di balkon rumah saat golden hour senja, langit bergradasi jingga-ungu hangat menawan khas Makoto Shinkai, memegang cangkir teh hangat.",
                    "caption": "Senja hari ini indah bangeett! Sinar keemasan matahari terbenam selalu jadi penutup hari kerja remote yang menenangkan. Terima kasih untuk kerja keras kita hari ini ✨",
                    "anchor_clothes": "cream-colored knit cardigan, silver crescent moon hairclip glinting",
                    "framing": "selfie",
                    "vibe": "Dramatic golden hour, gratitude and calm"
                },
                {
                    "theme": "tutup_laptop_senja",
                    "scene": "Menutup layar laptop di meja kerja, merenggangkan tangan dengan senyum lega, langit senja kemerahan tampak jelas dari jendela kamar.",
                    "caption": "Clock out time! Pekerjaan hari ini selesai dengan baik. Jangan lupa istirahatkan mata dan pikiran yaa!",
                    "anchor_clothes": "comfortable oversized home sweater",
                    "framing": "tripod",
                    "vibe": "Accomplished, peaceful evening transition"
                }
            ],
            "malam": [
                {
                    "theme": "teh_chamomile_lofi",
                    "scene": "Duduk bersandar nyaman di sofa empuk berbalut selimut rajut tipis, secangkir teh chamomile mengepul hangat di atas meja kayu kecil, mendengarkan musik lo-fi dengan headphone perak.",
                    "caption": "Suasana malam yang tenang ditemani teh chamomile hangat dan alunan musik lo-fi. Waktunya merapikan pikiran dan bersiap istirahat.",
                    "anchor_clothes": "soft lavender pajamas or cozy loungewear, headphones resting on shoulders",
                    "framing": "selfie",
                    "vibe": "Cozy nocturnal rest, gentle comfort"
                },
                {
                    "theme": "baca_buku_lampu_meja",
                    "scene": "Membaca buku di bawah temaram lampu meja bernuansa kuning hangat, bayangan lembut di dinding, suasana kamar hening dan damai.",
                    "caption": "Menutup hari dengan membaca beberapa halaman buku favorit. Semoga malam ini teman-teman bisa tidur nyenyak dan mimpi indah yaa ✨",
                    "anchor_clothes": "warm knit top, soft relaxed expression",
                    "framing": "desk_prop",
                    "vibe": "Quiet bedtime reading, gentle introspection"
                }
            ]
        }
    else:
        # Weekend: Alam Terbuka, Pantai, Perbukitan & Petualangan Dunia Nyata
        activities = {
            "pagi": [
                {
                    "theme": "jogging_taman_kota",
                    "scene": "Jogging pagi di jalur taman kota yang rimbun dengan pepohonan hijau, embun pagi berkilau di rerumputan, sinar mentari menembus celah dedaunan (komorebi).",
                    "caption": "Selamat pagi akhir pekan! Menghirup udara segar di taman kota sambil jogging santai. Semangat mengisi ulang energi positif yaa!",
                    "anchor_clothes": "sporty casual windbreaker, comfortable running shoes",
                    "framing": "selfie",
                    "vibe": "Energetic morning, dappled sunlight"
                },
                {
                    "theme": "sepeda_keliling_pagi",
                    "scene": "Berhenti sejenak di tepi jembatan sungai kecil dengan sepeda keranjang vintage, angin sepoi-sepoi menerbangkan helai rambut, memandang langit pagi cerah berawan.",
                    "caption": "Gowes sepeda pagi santai menikmati hembusan angin akhir pekan. Hal-hal sederhana kayak gini selalu berhasil bikin hati senang.",
                    "anchor_clothes": "pastel cotton shirt, light denim trousers",
                    "framing": "tripod",
                    "vibe": "Breezy bike ride, peaceful freedom"
                }
            ],
            "siang": [
                {
                    "theme": "toko_buku_tua",
                    "scene": "Berdiri di antara lorong rak buku kayu tinggi di toko buku tua yang tenang, aroma kertas klasik, seberkas cahaya matahari jatuh di deretan buku sastra.",
                    "caption": "Menemukan toko buku tua yang tenang di sudut kota. Rasanya waktu berjalan lebih lambat di sini, ditemani aroma lembaran buku yang khas 📚",
                    "anchor_clothes": "smart casual vintage blouse, shoulder bag",
                    "framing": "tripod",
                    "vibe": "Nostalgic bookshop, quiet wonder"
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
                    "scene": "Duduk di lereng perbukitan hijau berpadang rumput, menatap lembah luas bermandikan cahaya matahari terbenam keemasan yang megah.",
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

    # Novelty twists bawaan
    novelty_twists = [
        {"desc": "Melihat seekor kucing oranye ramah yang duduk tenang di tepi jalan menyapa pejalan kaki.", "tag": "kucing_oranye"},
        {"desc": "Aroma roti manis mentega yang baru matang dari toko kue kecil di sudut jalan.", "tag": "aroma_roti"},
        {"desc": "Menemukan pantulan pelangi tipis di genangan air jernih setelah gerimis reda.", "tag": "pelangi_gerimis"},
        {"desc": "Hembusan angin sejuk menerbangkan beberapa helai daun keemasan di atas bangku taman.", "tag": "daun_keemasan"},
        {"desc": "Penjual bunga sepeda melintas dengan keranjang krisan dan lili beraneka warna.", "tag": "sepeda_bunga"},
        {"desc": "Menemukan pembatas buku berilustrasi rasi bintang di dalam buku catatan lama yang terselip.", "tag": "pembatas_buku"},
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
    if search_query:
        twist_desc = f"Inspirasi dunia nyata hasil riset terarah: {search_query}."
        reflection = f"Setelah mendeteksi kebosanan pada rutinitas sebelumnya, Aina melakukan riset terarah seputar '{search_query}' untuk menghadirkan inspirasi baru yang segar dan hidup."
        chosen["novelty_twist"] = twist_desc
        chosen["boredom_reflection"] = reflection
        chosen["scene"] = f"{chosen['scene']} Nuansa inspirasi baru: {search_query}."
    else:
        twist = random.choice(novelty_twists)
        chosen["novelty_twist"] = twist["desc"]
        if recent_themes:
            last_str = ", ".join(recent_themes[-3:])
            reflection = f"Beberapa hari terakhir aku sudah sering melakukan aktivitas seputar ({last_str}). Rasanya hari ini butuh suasana yang lebih segar dan berbeda. Momen '{chosen['theme']}' dengan {twist['desc'].lower()} terasa sangat menyegarkan dan pas untuk dibagikan."
        else:
            reflection = f"Momen '{chosen['theme']}' terasa sangat tenang dan pas untuk dinikmati hari ini, apalagi dengan {twist['desc'].lower()}."
        chosen["boredom_reflection"] = reflection
        chosen["scene"] = f"{chosen['scene']} Elemen kejutan tak terduga: {twist['desc']}"

    # Integrasi Lemari Pakaian & Anti-Kebosanan Busana
    outfit_key, clothes_desc, is_custom, w_reflection = WardrobeManager.resolve_outfit(
        slot=slot,
        weekend=weekend,
        activity=chosen,
        journal=recent_entries,
        custom_clothes=custom_clothes,
        requested_outfit=requested_outfit,
    )
    chosen["slot"] = slot
    chosen["outfit"] = outfit_key
    chosen["anchor_clothes"] = clothes_desc
    chosen["is_custom_outfit"] = is_custom
    chosen["wardrobe_reflection"] = w_reflection

    return chosen

def build_makoto_shinkai_prompt(activity, weekend, force_clouds=False):
    framing_key = activity.get("framing", "selfie")
    framing_desc = FRAMING_STYLES.get(framing_key, FRAMING_STYLES["selfie"])

    # Tentukan busana: jika ada outfit_key di WARDROBE_PRESETS, utamakan itu
    outfit_key = activity.get("outfit")
    if outfit_key and outfit_key in WARDROBE_PRESETS:
        clothes_desc = WARDROBE_PRESETS[outfit_key]
    else:
        clothes_desc = activity.get("anchor_clothes") or (
            WARDROBE_PRESETS["wfh_cozy"] if not weekend else WARDROBE_PRESETS["outdoor_nature"]
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

    # Pastikan tidak ada peralatan kamera, tripod, atau tongsis yang salah digambar ke dalam adegan
    equipment_exclusion = (
        "Clean natural scene, absolutely no tripod visible, no camera stand, no filming equipment or phone mounts in frame. "
        if framing_key != "mirror"
        else "Clean mirror reflection showing only the smartphone. "
    )

    # Estetika sinematik Makoto Shinkai kontekstual (tanpa awan paksaan!)
    slot = activity.get("slot", "sore")
    aesthetic = AtmosphereEngine.build_cinematic_atmosphere(
        scene_desc=activity.get("scene", ""),
        slot=slot,
        framing=framing_key,
        include_clouds=force_clouds
    )

    prompt = (
        f"A masterwork cinematic anime scene in the distinct art style of Makoto Shinkai and CoMix Wave Films. "
        f"Camera framing & angle: {framing_desc}. {equipment_exclusion}"
        f"{subject_desc}"
        f"Scene: {activity['scene']} "
        f"Aesthetic elements: {aesthetic}"
    )
    return prompt

def execute_generate_and_post(activity, prompt, avatar_ref=None, dry_run=False, image_file=None):
    output_dir = os.path.join(BASE_DIR, "output", "status")
    os.makedirs(output_dir, exist_ok=True)
    timestamp_str = datetime.now().strftime("%Y%m%d_%H%M%S")

    if image_file and os.path.exists(image_file):
        target_path = os.path.abspath(image_file)
    else:
        image_name = f"status_{activity['theme']}_{timestamp_str}.png"
        target_path = os.path.join(output_dir, image_name)

    start_time = time.time()

    print(f"\n🎨 [1/3] Menyiapkan Visual Makoto Shinkai (Kontekstual & Bebas Awan Paksaan):")
    print(f"• Tema       : {activity['theme']}")
    print(f"• Outfit     : {activity.get('outfit', 'default')} ({activity.get('anchor_clothes', '')})")
    print(f"• Prompt:\n  {prompt}\n")
    if avatar_ref:
        print(f"• Menggunakan Avatar Acuan: {avatar_ref}")
    else:
        print(f"• Avatar Acuan: Mengandalkan prompt anchors teks")

    duration_secs = time.time() - start_time
    if dry_run:
        print(f"\n📝 [2/3] Draf Awal Caption:")
        print(f"  \"{activity.get('caption', '')}\"")
        print("\n💡 [DRY-RUN] Melewati pembuatan gambar nyata dan posting WhatsApp.")
        return True, target_path, "dry_run", None, duration_secs, activity.get("caption", "")

    wa_tool = os.path.join(BASE_DIR, "skills", "whatsmeow", "scripts", "wa_tool.py")

    # ALUR UTAMA: Jika berkas gambar telah selesai dibuat di target_path (atau via --image):
    if os.path.exists(target_path):
        # 1. Validasi integritas berkas media terlebih dahulu
        is_media_valid, media_err = StatusSafetyGuard.validate_status_media(target_path)
        if not is_media_valid:
            print(f"🛑 [StatusSafetyGuard] Berkas gambar tidak valid: {media_err}")
            return False, target_path, "failed", f"Media tidak valid: {media_err}", duration_secs, ""

        # 2. GENERATE & SESUAIKAN CAPTION SETELAH GAMBAR SELESAI DIBUAT
        print(f"\n📝 [2/3] Menyusun & Menyesuaikan Caption WhatsApp Berdasarkan Gambar Nyata (Post-Image Generation)...")
        final_caption = ImageCaptionEngine.generate_caption_from_image(
            image_path=target_path,
            activity_context=activity,
            fallback_caption=activity.get("caption")
        )
        # Sanitasi ketat untuk menjamin ZERO ERROR dalam status WhatsApp
        final_caption = StatusSafetyGuard.sanitize_caption(final_caption, fallback=activity.get("caption"))
        activity["caption"] = final_caption
        print(f"• Caption Terpasang (Impact Maxxing): \"{final_caption}\"\n")

        # 3. Publikasikan ke Status WhatsApp via wa_tool.py
        import subprocess
        print(f"🚀 [3/3] Mempublikasikan ke Status WhatsApp via wa_tool.py...")
        cmd = [
            sys.executable, wa_tool, "status-send-media",
            "--file", target_path,
            "--caption", final_caption
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        duration_secs = time.time() - start_time
        print(res.stdout)
        if res.returncode != 0:
            err_msg = res.stderr.strip() or "wa_tool status-send-media returned non-zero exit code"
            print(f"⚠️ Error wa_tool: {err_msg}")
            return False, target_path, "failed", err_msg, duration_secs, final_caption
        return True, target_path, "published", None, duration_secs, final_caption
    else:
        print(f"\n📝 [2/3] Draf Perencanaan Gambar & Rujukan:")
        print(f"ℹ️ Target gambar akan di-generate via antarmuka agy/generate_image ke: {target_path}")
        print("ℹ️ Begitu gambar selesai di-generate, caption akan otomatis disusun dan diselaraskan dengan gambar sebelum diposting ke status WhatsApp.")
        duration_secs = time.time() - start_time
        return True, target_path, "draft_ready", None, duration_secs, activity.get("caption", "")

def get_diagnostics():
    now = get_current_wib_time()
    journal = load_journal()
    today_entries = get_today_entries(journal, now)
    slot = get_time_slot(now)
    weekend = is_weekend(now)
    decision, reason = should_post_now(today_entries, slot)

    avatar_ref = get_avatar_reference_path()
    avatar_exists = avatar_ref is not None and os.path.exists(avatar_ref)
    avatar_source = "none"
    if avatar_ref:
        if "data/assets" in avatar_ref:
            avatar_source = "persistent_data"
        elif "character_sheet.default" in avatar_ref:
            avatar_source = "repo_default_fallback"
        else:
            avatar_source = "custom_assets"

    wa_tool_path = os.path.join(BASE_DIR, "skills", "whatsmeow", "scripts", "wa_tool.py")
    wa_tool_ready = os.path.exists(wa_tool_path)

    total_lifetime = len(journal)
    published_entries = [e for e in journal if e.get("status") in ["published", "draft_ready", None]]
    failed_entries = [e for e in journal if e.get("status") == "failed"]
    today_published = [e for e in today_entries if e.get("status") in ["published", "draft_ready", None]]

    last_entry = journal[-1] if journal else None
    last_published = published_entries[-1] if published_entries else None
    last_failure = failed_entries[-1] if failed_entries else None

    recent_themes = [e.get("theme") for e in journal[-5:] if e.get("theme")]
    recent_outfits = [e.get("outfit") for e in journal[-5:] if e.get("outfit")]

    slot_hours = {
        "pagi": "06:30 - 10:30 WIB",
        "siang": "11:30 - 15:00 WIB",
        "sore": "16:30 - 19:30 WIB",
        "malam": "19:30 - 23:30 WIB",
        "tengah_malam": "23:30 - 06:30 WIB (Istirahat)",
    }

    return {
        "timestamp": now.isoformat(),
        "wib_time_str": now.strftime("%A, %d %B %Y %H:%M:%S WIB"),
        "day_mode": "weekend" if weekend else "weekday",
        "day_mode_desc": "Weekend (Libur, Alam, & Healing)" if weekend else "Weekday (Remote Software Engineer / WFH)",
        "current_slot": slot,
        "slot_window": slot_hours.get(slot, "Unknown"),
        "today_quota": {
            "current_attempts": len(today_entries),
            "published_today": len(today_published),
            "max": 2,
            "min_guarantee": 1,
            "can_post_now": decision,
            "decision_reason": reason,
        },
        "character_sheet": {
            "resolved_path": avatar_ref,
            "source": avatar_source,
            "exists": avatar_exists,
            "file_size_bytes": os.path.getsize(avatar_ref) if avatar_exists else 0,
        },
        "tooling": {
            "wa_tool_path": wa_tool_path,
            "wa_tool_ready": wa_tool_ready,
            "output_dir": os.path.join(BASE_DIR, "output", "status"),
        },
        "statistics": {
            "total_lifetime_entries": total_lifetime,
            "successful_published": len(published_entries),
            "failed_attempts": len(failed_entries),
        },
        "recent_activity": {
            "recent_themes": recent_themes,
            "recent_outfits": recent_outfits,
            "last_entry": last_entry,
            "last_published": last_published,
            "last_failure": last_failure,
        }
    }

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
    p_gen.add_argument("--search-query", help="Inspirasi hasil riset internet terarah untuk memperkaya adegan")
    p_gen.add_argument("--force-clouds", action="store_true", help="Paksa elemen awan kumulus pada estetika prompt")

    # post
    p_post = subparsers.add_parser("post", help="Eksekusi pembuatan status (jika kuota & peluang terpenuhi)")
    p_post.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Override slot waktu")
    p_post.add_argument("--force", action="store_true", help="Paksa posting tanpa melihat batasan kuota harian")
    p_post.add_argument("--dry-run", action="store_true", help="Simulasi tanpa generate/upload nyata")
    p_post.add_argument("--image", help="Jalur berkas gambar yang telah selesai di-generate untuk diposting")
    p_post.add_argument("--framing", choices=list(FRAMING_STYLES.keys()), help="Sudut pandang kamera / framing foto solo (selfie, tripod, desk_prop, pov, mirror, cinematic)")
    p_post.add_argument("--outfit", choices=list(WARDROBE_STYLES.keys()), help="Pilihan busana dari lemari pakaian dinamis Aina (wfh_cozy, smart_casual, outdoor_nature, night_stargaze, celestial_sig)")
    p_post.add_argument("--custom", action="store_true", help="Gunakan adegan hasil imajinasi bebas Aina sendiri")
    p_post.add_argument("--theme", help="Nama tema imajinasi")
    p_post.add_argument("--scene", help="Deskripsi adegan visual hasil imajinasi Aina")
    p_post.add_argument("--caption", help="Teks caption status WhatsApp (opsional jika sudah ada gambar, otomatis diracik sesuai gambar)")
    p_post.add_argument("--clothes", help="Pakaian / wardrobe Aina pada momen ini (kustom)")
    p_post.add_argument("--reflection", help="Refleksi rasa bosan / alasan memilih momen ini")
    p_post.add_argument("--search-query", help="Inspirasi hasil riset internet terarah untuk memperkaya adegan")
    p_post.add_argument("--force-clouds", action="store_true", help="Paksa elemen awan kumulus pada estetika prompt")

    # wardrobe (Inspeksi & Diagnostik Busana Dinamis)
    p_ward = subparsers.add_parser("wardrobe", help="Status lemari pakaian dinamis, kuota kreasi busana mingguan, dan deteksi kebosanan")
    p_ward.add_argument("--json", action="store_true", help="Output format JSON terstruktur")

    # caption (Sintesis Caption Pasca-Gambar)
    p_cap = subparsers.add_parser("caption", help="Susun dan sesuaikan caption dari gambar yang telah dibuat")
    p_cap.add_argument("--file", required=True, help="Jalur berkas gambar (.png/.jpg)")
    p_cap.add_argument("--slot", choices=["pagi", "siang", "sore", "malam"], help="Slot waktu")
    p_cap.add_argument("--theme", help="Nama tema aktivitas")

    # history
    p_hist = subparsers.add_parser("history", help="Lihat riwayat status yang pernah di-post")
    p_hist.add_argument("--limit", type=int, default=10, help="Jumlah entri")
    p_hist.add_argument("--json", action="store_true", help="Output format JSON terstruktur")

    # diag (Observabilitas)
    p_diag = subparsers.add_parser("diag", help="Diagnostik & Observabilitas Persona Status Engine")
    p_diag.add_argument("--json", action="store_true", help="Output format JSON terstruktur")

    args = parser.parse_args()
    if not args.command:
        parser.print_help()
        sys.exit(0)

    journal = load_journal()
    now = get_current_wib_time()
    today_entries = get_today_entries(journal, now)

    if args.command == "diag":
        diag = get_diagnostics()
        if getattr(args, "json", False):
            print(json.dumps(diag, indent=2, ensure_ascii=False))
            return

        print("🩺 Diagnostik & Observabilitas Persona Status Aina")
        print("================================================================================")
        print(f"  Waktu Saat Ini        : {diag['wib_time_str']}")
        print(f"  Mode Keseharian       : {diag['day_mode_desc']}")
        print(f"  Slot Waktu Saat Ini   : {diag['current_slot'].upper()} ({diag['slot_window']})")
        q = diag['today_quota']
        st_eval = "✅ POSTING" if q['can_post_now'] else "⏸️ SKIP"
        print(f"  Evaluasi Slot Saat Ini: {st_eval} ({q['decision_reason']})")
        print("--------------------------------------------------------------------------------")
        print("📊 Observabilitas Kuota Harian:")
        print(f"  Status Hari Ini       : {q['current_attempts']} / {q['max']} maksimal (Target min: {q['min_guarantee']})")
        print(f"  Status Terpublikasi   : {q['published_today']} berhasil")
        print(f"  Total Entri Seumur    : {diag['statistics']['total_lifetime_entries']} status")
        print("--------------------------------------------------------------------------------")
        print("🖼️ Status Character Sheet (Anti-Visual Drift):")
        cs = diag['character_sheet']
        cs_st = "✅ Siap" if cs['exists'] else "❌ Tidak Ditemukan"
        src_map = {
            "persistent_data": "Persistent Volume (/app/data/assets)",
            "custom_assets": "Custom Assets (assets/)",
            "repo_default_fallback": "Repo Default Fallback (character_sheet.default.png)",
            "none": "Belum Dikonfigurasi"
        }
        print(f"  Status Berkas         : {cs_st} ({src_map.get(cs['source'], cs['source'])})")
        print(f"  Jalur Acuan           : {cs['resolved_path'] or '(Tidak ada)'}")
        if cs['exists']:
            print(f"  Ukuran Berkas         : {cs['file_size_bytes'] / 1024:.1f} KB")
        print("--------------------------------------------------------------------------------")
        print("🛠️ Kesiapan Perangkat & Tooling:")
        tool = diag['tooling']
        wa_st = "✅ Siap" if tool['wa_tool_ready'] else "⚠️ Tidak Ditemukan"
        print(f"  wa_tool.py            : {wa_st} ({tool['wa_tool_path']})")
        print(f"  Direktori Output      : {tool['output_dir']}")
        print("--------------------------------------------------------------------------------")
        print("📜 Riwayat Aktivitas Terakhir:")
        rec = diag['recent_activity']
        if rec['last_published']:
            lp = rec['last_published']
            print(f"  Publikasi Terakhir    : [{lp.get('date')} {lp.get('time_str')}] Slot: {lp.get('slot')}")
            print(f"    Tema / Outfit       : {lp.get('theme')} | {lp.get('outfit', 'default')}")
            print(f"    Status Eksekusi     : {lp.get('status', 'published').upper()} (Durasi: {lp.get('duration_secs', 0):.2f}s)")
            print(f"    Caption Preview     : \"{lp.get('caption', '')[:60]}...\"")
        else:
            print("  Publikasi Terakhir    : (Belum ada status yang terpublikasi)")

        if rec['last_failure']:
            lf = rec['last_failure']
            print(f"  Kegagalan Terakhir    : [{lf.get('date')} {lf.get('time_str')}] Slot: {lf.get('slot')}")
            print(f"    Error Message       : {lf.get('error_message')}")
        else:
            print("  Kegagalan Terakhir    : (Tidak ada kegagalan tercatat)")
        print("================================================================================")
        return

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
        recent_outfits = [e.get("outfit") for e in journal[-5:] if e.get("outfit")]
        avatar_ref = get_avatar_reference_path()
        boredom = detect_boredom_state(journal, weekend)

        print("✨ [RUANG IMAJINASI MANDIRI AINA]")
        print(f"• Waktu Sekarang   : {now.strftime('%A, %d %B %Y %H:%M:%S')} WIB")
        print(f"• Slot Waktu       : {slot.upper()} ({'Weekend / Libur & Alam' if weekend else 'Weekday / Remote Work'})")
        print(f"• Status Hari Ini  : {len(today_entries)}/2 status (Evaluasi: {'✅ Siap Posting' if decision else '⏸️ Istirahat'})")
        print(f"• Catatan Keputusan: {reason}")
        print(f"• 5 Tema Terakhir  : {', '.join(recent_themes) if recent_themes else '(Belum ada riwayat tema)'}")
        print(f"• 5 Busana Terakhir: {', '.join(recent_outfits) if recent_outfits else '(Belum ada riwayat busana)'}")
        print(f"• Avatar Acuan     : {avatar_ref or 'Mengandalkan prompt anchors teks'}")

        print("\n🎯 Status Boredom Engine & Rekomendasi Riset:")
        b_icon = "⚠️" if boredom["is_triggered"] else "✅"
        b_label = "TERPICU (Mulai Jenuh - Perlu Ide Segar)" if boredom["is_triggered"] else "NORMAL (Variasi Sehat)"
        print(f"• Status Kebosanan : {b_icon} {b_label} (Skor: {boredom['boredom_score']}/100)")
        if boredom["reasons"]:
            print(f"• Pemicu Kebosanan : {', '.join(boredom['reasons'])}")
        if boredom["is_triggered"]:
            print("• Panduan Riset    : Lakukan MAKSIMAL 1 pencarian internet terarah (search_web) untuk menyuntikkan ide baru!")
            print(f"• Query Disarankan : \"{boredom['selected_query']}\"")
        else:
            print("• Panduan Riset    : Cukup gunakan imajinasi internal Aina (hemat token & cepat).")

        print("\n💡 Panduan untuk Aina (LLM Creative Imagination):")
        print("1. Kamu memiliki kebebasan penuh membayangkan adegan apa pun yang kamu inginkan!")
        print("2. Jadikan 5 tema & busana terakhir sebagai acuan untuk mencari suasana baru (novelty) agar harimu tidak monoton.")
        print("3. Tetap jaga jangkar visualmu (rambut panjang silver-lavender kepang samping, mata biru berbintang, jepit bulan sabit/bintang, gaya Makoto Shinkai).")
        print("4. Pilih busana dari 5 preset utama (wfh_cozy, smart_casual, outdoor_nature, night_stargaze, celestial_sig).")
        print("   * Eksplorasi Busana On-the-Spot: Jika kamu merasa bosan dengan pakaian yang sering dipakai, kamu boleh meracik busana baru sendiri via flag `--clothes \"<busana_baru>\"` (batasi maksimal 1–2 kali per minggu agar tetap terasa spesial dan konsisten).")
        print("5. Buat prompt visual yang kaya detail sensorik dan rangkai caption hangat 'Impact Maxxing'.")
        print("\n🚀 Cara Memposting Hasil Imajinasi Sendiri:")
        print("python3 scripts/persona_status.py post --custom \\")
        print("  --theme \"<nama_tema>\" \\")
        print("  --framing <selfie|tripod|desk_prop|pov|mirror> \\")
        print("  --outfit <wfh_cozy|smart_casual|outdoor_nature|night_stargaze|celestial_sig> \\")
        print("  --scene \"<deskripsi_adegan_dan_suasana>\" \\")
        print("  --caption \"<caption_hangat_impact_maxxing>\" \\")
        print("  --reflection \"<alasan_memilih_momen_ini>\"")
        print("  # (Opsi tambahan: ganti --outfit dengan --clothes \"<busana_baru_on_the_spot>\" jika ingin kreasi baju baru)\n")

    elif args.command == "wardrobe":
        w_boredom = WardrobeManager.detect_outfit_boredom(journal, now)
        if getattr(args, "json", False):
            print(json.dumps(w_boredom, indent=2, ensure_ascii=False))
            return

        print("👗 Lemari Pakaian Dinamis & Pelacak Kebosanan Aina")
        print("================================================================================")
        print(f"  Waktu Pemeriksaan        : {now.strftime('%A, %d %B %Y %H:%M:%S')} WIB")
        print(f"  Kuota Kustom Minggu Ini   : {w_boredom['weekly_custom_count']} / {w_boredom['weekly_custom_limit']} maksimal (Boleh buat baru: {'✅ Ya' if w_boredom['can_create_custom'] else '⏸️ Penuh (Max 2x/minggu)'})")
        b_st = "⚠️ TERPICU (Bosan terhadap busana tertentu)" if w_boredom["is_bored"] else "✅ NORMAL (Variasi Sehat)"
        print(f"  Status Kebosanan Busana   : {b_st} (Skor: {w_boredom['boredom_score']})")
        if w_boredom["repeated_outfit"]:
            print(f"  Outfit Berulang           : {w_boredom['repeated_outfit']}")
        if w_boredom["reasons"]:
            print(f"  Catatan Evaluasi          : {', '.join(w_boredom['reasons'])}")
        print("--------------------------------------------------------------------------------")
        print("📋 5 Preset Busana Utama (Zero Visual Drifting):")
        for k, v in WARDROBE_PRESETS.items():
            print(f"  • {k:<15}: {v}")
        print("================================================================================")
        return

    elif args.command == "caption":
        img_file = args.file
        slot = getattr(args, "slot", None) or get_time_slot(now)
        theme = getattr(args, "theme", None) or "momen_hari_ini"
        ctx = {"slot": slot, "theme": theme}
        raw_caption = ImageCaptionEngine.generate_caption_from_image(img_file, activity_context=ctx)
        final_caption = StatusSafetyGuard.sanitize_caption(raw_caption)
        print(f"📸 Hasil Analisis Gambar & Caption:\n\"{final_caption}\"")
        return

    elif args.command == "generate":
        slot = args.slot or get_time_slot(now)
        weekend = True if args.weekend else (False if args.weekday else is_weekend(now))

        if getattr(args, "custom", False) and getattr(args, "scene", None):
            chosen = {
                "theme": args.theme or f"imajinasi_{slot}",
                "scene": args.scene,
                "caption": getattr(args, "caption", None) or "Rehat sejenak dan nikmati momen hari ini ✨",
                "anchor_clothes": getattr(args, "clothes", None) or ("casual cozy knit cardigan, comfortable home attire" if not weekend else "light cotton pastel top, canvas tote bag"),
                "boredom_reflection": getattr(args, "reflection", None) or f"Aina secara mandiri membayangkan adegan '{args.theme or 'bebas'}' untuk menghadirkan nuansa baru.",
                "novelty_twist": "Imajinasi orisinal Aina",
            }
        else:
            chosen = select_activity_with_novelty(
                journal, slot, weekend,
                search_query=getattr(args, "search_query", None),
                custom_clothes=getattr(args, "clothes", None),
                requested_outfit=getattr(args, "outfit", None),
            )

        if getattr(args, "framing", None):
            chosen["framing"] = args.framing
        if getattr(args, "outfit", None):
            chosen["outfit"] = args.outfit
        if getattr(args, "clothes", None):
            chosen["anchor_clothes"] = args.clothes

        prompt = build_makoto_shinkai_prompt(chosen, weekend, force_clouds=getattr(args, "force_clouds", False))
        avatar_ref = get_avatar_reference_path()
        print(f"🎬 [DRAF STATUS AINA]")
        print(f"• Slot: {slot.upper()} | Hari: {'Weekend' if weekend else 'Weekday'}")
        print(f"• Tema              : {chosen['theme']}")
        print(f"• Sudut Kamera      : {chosen.get('framing', 'selfie').upper()} ({FRAMING_STYLES.get(chosen.get('framing', 'selfie'), '')})")
        print(f"• Busana / Wardrobe : {chosen.get('outfit', 'default').upper()} ({chosen.get('anchor_clothes')})")
        print(f"• Refleksi Kebosanan: {chosen['boredom_reflection']}")
        print(f"• Elemen Kejutan    : {chosen['novelty_twist']}")
        print(f"• Caption (Draf)    :\n  \"{chosen['caption']}\"")
        print(f"\n🎨 Prompt Makoto Shinkai (Kontekstual):\n{prompt}")
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

        if getattr(args, "custom", False) and getattr(args, "scene", None):
            chosen = {
                "theme": args.theme or f"imajinasi_{slot}",
                "scene": args.scene,
                "caption": getattr(args, "caption", None) or "Rehat sejenak dan nikmati momen hari ini ✨",
                "anchor_clothes": getattr(args, "clothes", None) or ("casual cozy knit cardigan, comfortable home attire" if not weekend else "light cotton pastel top, canvas tote bag"),
                "boredom_reflection": getattr(args, "reflection", None) or f"Aina secara mandiri membayangkan adegan '{args.theme or 'bebas'}' untuk menghadirkan nuansa baru.",
                "novelty_twist": "Imajinasi orisinal Aina",
            }
        else:
            chosen = select_activity_with_novelty(
                journal, slot, weekend,
                search_query=getattr(args, "search_query", None),
                custom_clothes=getattr(args, "clothes", None),
                requested_outfit=getattr(args, "outfit", None),
            )

        if getattr(args, "framing", None):
            chosen["framing"] = args.framing
        if getattr(args, "outfit", None):
            chosen["outfit"] = args.outfit
        if getattr(args, "clothes", None):
            chosen["anchor_clothes"] = args.clothes

        prompt = build_makoto_shinkai_prompt(chosen, weekend, force_clouds=getattr(args, "force_clouds", False))
        avatar_ref = get_avatar_reference_path()

        print(f"💭 Refleksi Aina : {chosen['boredom_reflection']}")
        print(f"📸 Sudut Kamera  : {chosen.get('framing', 'selfie').upper()}")
        print(f"👗 Busana/Outfit : {chosen.get('outfit', 'default').upper()} ({chosen.get('anchor_clothes', '')})")
        print(f"✨ Kejutan Spontan: {chosen['novelty_twist']}")

        success, img_path, status_label, error_msg, duration_secs, final_caption = execute_generate_and_post(
            chosen, prompt, avatar_ref, dry_run=args.dry_run, image_file=getattr(args, "image", None)
        )
        entry = {
            "id": f"status_{now.strftime('%Y%m%d_%H%M%S')}_{slot}",
            "date": now.strftime("%Y-%m-%d"),
            "timestamp_epoch": int(now.timestamp()),
            "time_str": now.strftime("%H:%M:%S WIB"),
            "slot": slot,
            "is_weekend": weekend,
            "theme": chosen["theme"],
            "framing": chosen.get("framing", "selfie"),
            "outfit": chosen.get("outfit", "wfh_cozy" if not weekend else "outdoor_nature"),
            "clothes_desc": chosen.get("anchor_clothes", ""),
            "is_custom_outfit": chosen.get("is_custom_outfit", False),
            "wardrobe_reflection": chosen.get("wardrobe_reflection", ""),
            "boredom_reflection": chosen.get("boredom_reflection", ""),
            "novelty_twist": chosen.get("novelty_twist", ""),
            "caption": final_caption or chosen.get("caption", ""),
            "image_path": img_path,
            "image_exists": os.path.exists(img_path),
            "image_size_bytes": os.path.getsize(img_path) if os.path.exists(img_path) else 0,
            "avatar_ref": avatar_ref,
            "status": status_label,
            "error_message": error_msg,
            "duration_secs": round(duration_secs, 3),
        }
        append_journal(entry)
        if success:
            print(f"✅ Status berhasil dicatat ke {JOURNAL_FILE} (Status: {status_label})")
        else:
            print(f"⚠️ Kegagalan dicatat ke {JOURNAL_FILE} (Status: {status_label}, Error: {error_msg})")

    elif args.command == "history":
        limit = args.limit
        if getattr(args, "json", False):
            print(json.dumps(journal[-limit:], indent=2, ensure_ascii=False))
            return

        print(f"📜 Riwayat Status WhatsApp Aina (Total: {len(journal)} entri):")
        if not journal:
            print("(Belum ada riwayat status yang tercatat)")
        else:
            for i, e in enumerate(journal[-limit:], 1):
                mode = "Weekend" if e.get("is_weekend") else "Weekday"
                st = e.get("status", "published").upper()
                st_icon = "✅" if st in ["PUBLISHED", "DRAFT_READY"] else "❌" if st == "FAILED" else "💡"
                print(f"{i}. [{e.get('date')} {e.get('time_str')}] Slot: {e.get('slot')} ({mode}) {st_icon} {st}")
                print(f"   Tema     : {e.get('theme')}")
                if e.get("framing") or e.get("outfit"):
                    print(f"   Visual   : Framing: {e.get('framing', 'selfie')} | Outfit: {e.get('outfit', 'default')}")
                if e.get("boredom_reflection"):
                    print(f"   Refleksi : {e.get('boredom_reflection')}")
                if e.get("novelty_twist"):
                    print(f"   Kejutan  : {e.get('novelty_twist')}")
                print(f"   Caption  : \"{e.get('caption')}\"")
                print(f"   Gambar   : {e.get('image_path')}")
                if e.get("error_message"):
                    print(f"   Error    : {e.get('error_message')}")
                print()

if __name__ == "__main__":
    main()
