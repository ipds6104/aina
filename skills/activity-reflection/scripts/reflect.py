#!/usr/bin/env python3
"""
skills/activity-reflection/scripts/reflect.py
--------------------------------------------
Engine Refleksi Aktivitas & Adaptasi Diri Otonom Aina:
1. Memanen bukti empiris dari run scheduler (`scheduled_task_runs`), jurnal visual (`status_journal.jsonl`),
   dan jejak aksi WhatsApp (`whatsapp_action_audits`).
2. Menghitung metrik variasi (Boredom & Repetition Score) serta rotasi kuliner/destinasi Pontianak.
3. Mengambil data cuaca faktual Pontianak (real-time via Open-Meteo) untuk grounding fisik.
4. Menghasilkan saran adaptasi nyata dan mengeksekusi modifikasi aktivitas via CLI `aina activity edit`.
"""

import os
import sys
import json
import sqlite3
import argparse
import urllib.request
import urllib.error
from datetime import datetime, timezone, timedelta
from typing import Dict, Any, List, Optional, Tuple

WIB = timezone(timedelta(hours=7))

# Pontianak Geolocation (Khatulistiwa)
PONTIANAK_LAT = -0.0263
PONTIANAK_LON = 109.3425

# Resolusi path direktori kerja
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.abspath(os.path.join(_SCRIPT_DIR, ".."))
PROJECT_ROOT = os.path.abspath(os.path.join(SKILL_DIR, "..", ".."))

_SCRIPTS_PATH = os.path.join(PROJECT_ROOT, "scripts")
if _SCRIPTS_PATH not in sys.path:
    sys.path.insert(0, _SCRIPTS_PATH)

try:
    from persona.interest import InterestDiscoveryEngine
except Exception:
    InterestDiscoveryEngine = None

# Database & Data Paths (Mendukung container /app dan lokal)
DEFAULT_DB_PATH = os.environ.get(
    "DATABASE_PATH",
    os.path.join(PROJECT_ROOT, "data", "aina.db") if os.path.isdir(os.path.join(PROJECT_ROOT, "data")) else "/app/data/aina.db"
)
DEFAULT_JOURNAL_PATH = os.environ.get(
    "STATUS_JOURNAL_PATH",
    os.path.join(PROJECT_ROOT, "data", "status_journal.jsonl") if os.path.isdir(os.path.join(PROJECT_ROOT, "data")) else "/app/data/status_journal.jsonl"
)
DEFAULT_ACTIVITIES_DIR = os.environ.get(
    "ACTIVITIES_DIR",
    os.path.join(PROJECT_ROOT, "data", "activities") if os.path.isdir(os.path.join(PROJECT_ROOT, "data")) else "/app/data/activities"
)
DEFAULT_BASELINE_FILE = os.path.join(PROJECT_ROOT, "config", "activities.default.json")

# Ikon Kuliner Khas Pontianak untuk Rotasi Variasi
PONTIANAK_CULINARY_CATALOG = [
    {
        "name": "Es Lidah Buaya",
        "category": "beverage_cold",
        "best_for": ["siang", "panas", "terik"],
        "desc": "Minuman segar es lidah buaya khas Pontianak dengan sirup manis dan selasih, pelepas dahaga di siang khatulistiwa."
    },
    {
        "name": "Chai Kwe Siam",
        "category": "snack",
        "best_for": ["siang", "sore", "gerimis", "santai"],
        "desc": "Chai Kwe kukus/goreng isi bengkuang, kucai, dan talas dengan taburan bawang putih goreng renyah dan sambal pedas manis."
    },
    {
        "name": "Bubur Pedas Pontianak/Sambas",
        "category": "meal_warm",
        "best_for": ["sore", "malam", "hujan", "dingin"],
        "desc": "Bubur gurih kaya rempah dan sayuran daun kesum, pakis, taburan kacang tanah goreng dan teri renyah yang menghangatkan suasana."
    },
    {
        "name": "Pisang Goreng Selai Srikaya",
        "category": "snack",
        "best_for": ["sore", "pagi", "kopi"],
        "desc": "Pisang goreng kipas renyah berbalur selai srikaya legit khas warung kopi Pontianak."
    },
    {
        "name": "Kopi Saring Panas (Kopi Kohie)",
        "category": "beverage_warm",
        "best_for": ["pagi", "sore", "malam", "hujan"],
        "desc": "Kopi saring pekat aroma khas warung kopi Jalan Gajah Mada / Reformasi Pontianak."
    },
    {
        "name": "Mie Tiaw Apollo / Pattimura",
        "category": "meal_warm",
        "best_for": ["malam", "makan_malam"],
        "desc": "Kwetiau goreng sapi atau siram Pontianak beraroma wok-hei yang sedap dan mengenyangkan."
    }
]

# Destinasi Jalan-jalan & Refreshing Khas Pontianak
PONTIANAK_DESTINATIONS = [
    {
        "slug": "waterfront_kapuas",
        "name": "Waterfront Tepian Sungai Kapuas",
        "vibes": ["senja", "sore", "malam", "angin_sepoi", "outdoor"],
        "desc": "Menikmati semilir angin tepi Sungai Kapuas saat senja, kapal wisata berlalu-lalang, dan lampu kota mulai gemerlap."
    },
    {
        "slug": "taman_digulis",
        "name": "Taman Digulis Untan (Tugu Digulis)",
        "vibes": ["pagi", "sore", "olahraga", "teduh", "outdoor"],
        "desc": "Jalan santai di bawah rindangnya pepohonan taman kota Untan, suasana segar menyegarkan pikiran."
    },
    {
        "slug": "taman_akcaya",
        "name": "Taman Akcaya & Sentra Kuliner",
        "vibes": ["sore", "senja", "santai", "jajanan"],
        "desc": "Duduk santai di bangku taman sore hari sambil menikmati aneka kudapan ringan."
    },
    {
        "slug": "warkop_gajah_mada",
        "name": "Kawasan Warung Kopi Jalan Gajah Mada",
        "vibes": ["pagi", "malam", "kerja_santai", "sosial"],
        "desc": "Menyeruput kopi saring di sudut warkop yang hidup dan penuh interaksi khas Pontianak."
    },
    {
        "slug": "teras_stargazing",
        "name": "Teras Rumah Menatap Langit Malam",
        "vibes": ["malam", "tenang", "stargaze", "hening"],
        "desc": "Duduk di teras rumah memandang langit khatulistiwa malam hari dengan secangkir teh chamomile hangat."
    }
]


def get_wib_now() -> datetime:
    return datetime.now(WIB)


def fetch_pontianak_weather() -> Dict[str, Any]:
    """Mengambil kondisi cuaca faktual Pontianak secara real-time dari Open-Meteo."""
    url = (
        f"https://api.open-meteo.com/v1/forecast?"
        f"latitude={PONTIANAK_LAT}&longitude={PONTIANAK_LON}&"
        f"current=temperature_2m,relative_humidity_2m,weather_code,is_day,precipitation,rain"
    )
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "AinaPersona/0.2 (Pontianak; AutonomousAgent)"}
    )

    try:
        with urllib.request.urlopen(req, timeout=4.0) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            current = data.get("current", {})
            temp = current.get("temperature_2m", 28.0)
            humidity = current.get("relative_humidity_2m", 85)
            wmo_code = current.get("weather_code", 0)
            precip = current.get("precipitation", 0.0)
            rain = current.get("rain", 0.0)

            # Terjemahan WMO Weather Code
            condition, is_rainy = parse_wmo_code(wmo_code, precip, rain)

            return {
                "source": "open-meteo",
                "location": "Pontianak, Kalimantan Barat",
                "temperature_c": temp,
                "humidity_pct": humidity,
                "weather_code": wmo_code,
                "condition": condition,
                "is_rainy": is_rainy,
                "precipitation_mm": precip,
                "summary": f"{condition}, {temp}°C, kelembapan {humidity}%"
            }
    except Exception as e:
        # Fallback estimasi tropis khatulistiwa bila offline
        now = get_wib_now()
        is_afternoon = 13 <= now.hour <= 17
        temp_est = 31.0 if is_afternoon else 26.5
        return {
            "source": "offline_tropical_fallback",
            "location": "Pontianak, Kalimantan Barat",
            "temperature_c": temp_est,
            "humidity_pct": 85,
            "weather_code": 1,
            "condition": "Sebagian Berawan (Estimasi Khatulistiwa)",
            "is_rainy": False,
            "precipitation_mm": 0.0,
            "summary": f"Sebagian Berawan, {temp_est}°C (perkiraan lokal: {e})"
        }


def parse_wmo_code(code: int, precip: float, rain: float) -> Tuple[str, bool]:
    """Menerjemahkan kode cuaca WMO ke bahasa Indonesia yang ramah."""
    if code == 0:
        return "Cerah Lapang", False
    elif code in [1, 2]:
        return "Cerah Berawan", False
    elif code == 3:
        return "Mendung / Berawan Tebal", False
    elif code in [45, 48]:
        return "Berkabut Tipis", False
    elif code in [51, 53, 55]:
        return "Gerimis Halus", True
    elif code in [61, 63, 65]:
        return "Hujan Basah", True
    elif code in [80, 81, 82]:
        return "Hujan Rintik-Rintik / Hujan Lokal", True
    elif code in [95, 96, 99]:
        return "Hujan Disertai Petir (Badai Tropis)", True
    else:
        if precip > 0.1 or rain > 0.1:
            return "Gerimis Tropis", True
        return "Berawan Khatulistiwa", False


def get_sqlite_conn(db_path: str = DEFAULT_DB_PATH) -> Optional[sqlite3.Connection]:
    if not os.path.isfile(db_path):
        return None
    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        return conn
    except Exception:
        return None


def harvest_daily_runs(db_path: str = DEFAULT_DB_PATH, limit: int = 15) -> List[Dict[str, Any]]:
    """Mengambil riwayat eksekusi scheduled_task_runs hari ini."""
    conn = get_sqlite_conn(db_path)
    if not conn:
        return []

    try:
        cur = conn.cursor()
        # Periksa apakah tabel ada
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='scheduled_task_runs'")
        if not cur.fetchone():
            return []

        # Ambil awal hari ini dalam WIB (epoch seconds)
        now_wib = get_wib_now()
        start_of_day = datetime(now_wib.year, now_wib.month, now_wib.day, 0, 0, 0, tzinfo=WIB)
        start_epoch = int(start_of_day.timestamp())

        cur.execute(
            """
            SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
            FROM scheduled_task_runs
            WHERE executed_at_epoch >= ?
            ORDER BY executed_at_epoch DESC
            LIMIT ?
            """,
            (start_epoch, limit)
        )
        rows = cur.fetchall()
        runs = []
        for r in rows:
            dt = datetime.fromtimestamp(r["executed_at_epoch"], tz=WIB)
            runs.append({
                "id": r["id"],
                "task_id": r["task_id"],
                "title": r["task_title"],
                "target_jid": r["target_jid"],
                "status": r["status"],
                "duration_secs": r["duration_secs"],
                "error": r["error_message"],
                "output_preview": r["output_preview"][:150] if r["output_preview"] else None,
                "time_str": dt.strftime("%H:%M:%S WIB"),
                "epoch": r["executed_at_epoch"]
            })
        return runs
    except Exception as e:
        sys.stderr.write(f"Gagal memanen scheduled_task_runs: {e}\n")
        return []
    finally:
        conn.close()


def harvest_experience_records(db_path: str = DEFAULT_DB_PATH, limit: int = 5) -> List[Dict[str, Any]]:
    """Membaca catatan pengalaman teknis (Experience Library - Harness² format) dari SQLite."""
    conn = get_sqlite_conn(db_path)
    if not conn:
        return []
    try:
        cur = conn.cursor()
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='experience_records'")
        if not cur.fetchone():
            return []
        cur.execute(
            """
            SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
            FROM experience_records
            ORDER BY created_at_epoch DESC
            LIMIT ?
            """,
            (limit,)
        )
        rows = cur.fetchall()
        exps = []
        for r in rows:
            exps.append({
                "id": r["id"],
                "domain": r["domain"],
                "cue": r["cue"],
                "lesson": r["lesson"],
                "evidence": r["evidence"],
                "confidence": r["confidence"],
                "task_id": r["task_id"]
            })
        return exps
    except Exception as e:
        sys.stderr.write(f"Gagal memanen experience_records: {e}\n")
        return []
    finally:
        conn.close()


def find_matching_experience(query: str, db_path: str = DEFAULT_DB_PATH) -> Optional[Dict[str, Any]]:
    """Mencari apakah ada pelajaran pengalaman terdahulu yang cocok dengan error/query."""
    conn = get_sqlite_conn(db_path)
    if not conn:
        return None
    try:
        cur = conn.cursor()
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='experience_records'")
        if not cur.fetchone():
            return None
        pattern = f"%{query.strip()}%"
        cur.execute(
            """
            SELECT id, domain, cue, lesson, evidence, confidence
            FROM experience_records
            WHERE cue LIKE ? OR lesson LIKE ? OR evidence LIKE ?
            ORDER BY created_at_epoch DESC
            LIMIT 1
            """,
            (pattern, pattern, pattern)
        )
        row = cur.fetchone()
        if row:
            return {
                "id": row["id"],
                "domain": row["domain"],
                "cue": row["cue"],
                "lesson": row["lesson"],
                "evidence": row["evidence"],
                "confidence": row["confidence"]
            }
        return None
    except Exception:
        return None
    finally:
        conn.close()


def harvest_recent_status_journal(journal_path: str = DEFAULT_JOURNAL_PATH, limit: int = 10) -> List[Dict[str, Any]]:
    """Membaca riwayat publikasi status WhatsApp dari file journal."""
    if not os.path.isfile(journal_path):
        return []

    entries = []
    try:
        with open(journal_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
        return entries[-limit:]
    except Exception as e:
        sys.stderr.write(f"Gagal membaca status_journal: {e}\n")
        return []


def harvest_chat_audits(db_path: str = DEFAULT_DB_PATH, limit: int = 10) -> List[Dict[str, Any]]:
    """Membaca interaksi obrolan WhatsApp hari ini dari whatsapp_action_audits."""
    conn = get_sqlite_conn(db_path)
    if not conn:
        return []

    try:
        cur = conn.cursor()
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='whatsapp_action_audits'")
        if not cur.fetchone():
            return []

        now_wib = get_wib_now()
        start_of_day = datetime(now_wib.year, now_wib.month, now_wib.day, 0, 0, 0, tzinfo=WIB)
        start_epoch = int(start_of_day.timestamp())

        cur.execute(
            """
            SELECT id, sender_jid, chat_jid, tools_invoked, duration_seconds, status, error_message, created_at_epoch, usecase
            FROM whatsapp_action_audits
            WHERE created_at_epoch >= ?
            ORDER BY created_at_epoch DESC
            LIMIT ?
            """,
            (start_epoch, limit)
        )
        rows = cur.fetchall()
        audits = []
        for r in rows:
            dt = datetime.fromtimestamp(r["created_at_epoch"], tz=WIB)
            audits.append({
                "id": r["id"],
                "sender_jid": r["sender_jid"],
                "chat_jid": r["chat_jid"],
                "tools": json.loads(r["tools_invoked"]) if r["tools_invoked"] else [],
                "duration_secs": r["duration_seconds"],
                "status": r["status"],
                "error": r["error_message"],
                "usecase": r["usecase"],
                "time_str": dt.strftime("%H:%M:%S WIB")
            })
        return audits
    except Exception as e:
        sys.stderr.write(f"Gagal memanen whatsapp_action_audits: {e}\n")
        return []
    finally:
        conn.close()


def evaluate_daily_experience(
    runs: List[Dict[str, Any]],
    journal: List[Dict[str, Any]],
    weather: Dict[str, Any]
) -> Dict[str, Any]:
    """
    Mengevaluasi keseimbangan aktivitas, tingkat kebosanan (boredom score),
    serta merumuskan rekomendasi penyesuaian (kuliner, destinasi, busana, instruksi).
    """
    now_wib = get_wib_now()

    # 1. Analisis Kinerja Eksekusi
    total_runs = len(runs)
    success_runs = sum(1 for r in runs if r["status"] == "success")
    failed_runs = sum(1 for r in runs if r["status"] != "success")
    failed_task_items = [r for r in runs if r["status"] != "success"]

    # 2. Analisis Kejenuhan Tema & Busana dari Jurnal
    recent_themes = [j.get("theme", "") for j in journal[-5:]]
    recent_outfits = [j.get("outfit", "") for j in journal[-5:]]

    # Hitung frekuensi pengulangan
    theme_counts = {}
    for t in recent_themes:
        if t:
            theme_counts[t] = theme_counts.get(t, 0) + 1

    outfit_counts = {}
    for o in recent_outfits:
        if o:
            outfit_counts[o] = outfit_counts.get(o, 0) + 1

    # Skor kebosanan (0.0 = segar, 1.0 = sangat jenuh)
    max_theme_rep = max(theme_counts.values()) if theme_counts else 1
    max_outfit_rep = max(outfit_counts.values()) if outfit_counts else 1
    boredom_score = min(1.0, (max_theme_rep * 0.25) + (max_outfit_rep * 0.25))

    # 3. Pemilihan Rekomendasi Kuliner Pontianak
    is_rain = weather.get("is_rainy", False)
    temp = weather.get("temperature_c", 28.0)
    hour = now_wib.hour

    suitable_culinary = []
    for item in PONTIANAK_CULINARY_CATALOG:
        score = 0
        if is_rain and any(v in item["best_for"] for v in ["hujan", "dingin", "gerimis"]):
            score += 3
        if temp >= 30.0 and any(v in item["best_for"] for v in ["panas", "terik"]):
            score += 3
        if hour < 11 and "pagi" in item["best_for"]:
            score += 2
        elif 11 <= hour < 15 and "siang" in item["best_for"]:
            score += 2
        elif 15 <= hour < 19 and "sore" in item["best_for"]:
            score += 2
        elif hour >= 19 and "malam" in item["best_for"]:
            score += 2

        suitable_culinary.append((score, item))

    suitable_culinary.sort(key=lambda x: x[0], reverse=True)
    recommended_food = suitable_culinary[0][1] if suitable_culinary else PONTIANAK_CULINARY_CATALOG[0]

    # 4. Pemilihan Rekomendasi Destinasi Jalan-jalan
    suitable_dest = []
    for dest in PONTIANAK_DESTINATIONS:
        score = 0
        if is_rain:
            # Jika hujan, preferensi indoor / teras / warkop tertutup
            if "outdoor" in dest["vibes"]:
                score -= 2
            if "warkop" in dest["slug"] or "teras" in dest["slug"]:
                score += 3
        else:
            if "outdoor" in dest["vibes"]:
                score += 2
        if 16 <= hour <= 18 and "senja" in dest["vibes"]:
            score += 3
        elif hour >= 19 and "malam" in dest["vibes"]:
            score += 3

        suitable_dest.append((score, dest))

    suitable_dest.sort(key=lambda x: x[0], reverse=True)
    recommended_destination = suitable_dest[0][1] if suitable_dest else PONTIANAK_DESTINATIONS[0]

    # 5. Rekomendasi Busana Berdasarkan Cuaca & Destinasi
    if is_rain:
        recommended_outfit = "wfh_cozy"
        outfit_reason = "Suhu lebih sejuk karena hujan/gerimis; oversized knit sweater lavender/krem memberikan rasa nyaman dan hangat."
    elif "waterfront" in recommended_destination["slug"] or "taman" in recommended_destination["slug"]:
        recommended_outfit = "outdoor_nature"
        outfit_reason = "Aktivitas luar ruangan santai; blus katun pastel dan celana linen longgar sejuk berangin."
    elif hour >= 20:
        recommended_outfit = "night_stargaze"
        outfit_reason = "Suasana malam tenang; fleece hoodie navy tebal yang nyaman untuk bersantai."
    else:
        recommended_outfit = "smart_casual"
        outfit_reason = "Busana kasual kemeja katun rapi dan knit vest yang fleksibel untuk kerja maupun kafe."

    # 6. Formulasi Kebutuhan Riset Web Terarah (Gated Novelty Web Search)
    web_search_prompts = []
    month_year = now_wib.strftime("%B %Y")

    if is_rain:
        web_search_prompts.append({
            "topic": "Cuaca & Peringatan Dini BMKG",
            "query": "prakiraan cuaca bmkg pontianak hari ini dan besok",
            "reason": "Memastikan durasi hujan rintik dan potensi petir di Pontianak sebelum merencanakan aktivitas luar."
        })
    else:
        web_search_prompts.append({
            "topic": "Suhu & Kenyamanan Luar Ruangan",
            "query": "suhu dan cuaca pontianak sore ini",
            "reason": "Memverifikasi kenyamanan suhu di luar ruangan sebelum merencanakan jalan santai."
        })

    if boredom_score >= 0.5 or now_wib.weekday() in [4, 5, 6]:
        web_search_prompts.append({
            "topic": "Eksplorasi Destinasi Baru / Event",
            "query": f"rekomendasi coffee shop santai hidden gem pontianak {now_wib.year}",
            "reason": "Mencari inspirasi sudut kota atau kedai kopi baru di Pontianak untuk mengatasi kejenuhan rutinitas."
        })
    else:
        web_search_prompts.append({
            "topic": "Eksplorasi Kuliner Lokal",
            "query": f"kuliner khas pontianak legendaris {recommended_food['name']}",
            "reason": "Memperkaya wawasan sensorik dan kisah budaya di balik hidangan khas Pontianak."
        })

    if hour >= 18:
        web_search_prompts.append({
            "topic": "Fenomena Astronomi Langit Malam",
            "query": f"fenomena astronomi langit malam indonesia {month_year}",
            "reason": "Mencari inspirasi adegan stargazing khatulistiwa (fase bulan, hujan meteor, konjungsi planet)."
        })

    if InterestDiscoveryEngine:
        sampled_interest = InterestDiscoveryEngine.sample_interest()
        web_search_prompts.append({
            "topic": f"Penemuan Ketertarikan Baru ({sampled_interest['category'].replace('_', ' ').title()})",
            "query": sampled_interest.get("search_query", "tren inovasi teknologi lifestyle terkini"),
            "reason": f"Mengeksplorasi wawasan baru '{sampled_interest.get('title')}' untuk memperkaya status WhatsApp Aina."
        })

    return {
        "timestamp_wib": now_wib.strftime("%Y-%m-%d %H:%M:%S WIB"),
        "performance": {
            "total_runs_today": total_runs,
            "success_runs": success_runs,
            "failed_runs": failed_runs,
            "failed_task_items": failed_task_items
        },
        "novelty": {
            "boredom_score": round(boredom_score, 2),
            "recent_themes": recent_themes,
            "recent_outfits": recent_outfits,
            "is_stagnant": boredom_score >= 0.7
        },
        "recommendations": {
            "food": recommended_food,
            "destination": recommended_destination,
            "outfit": {
                "preset": recommended_outfit,
                "reason": outfit_reason
            }
        },
        "web_search_prompts": web_search_prompts
    }


def generate_adaptation_suggestions(
    eval_result: Dict[str, Any],
    weather: Dict[str, Any]
) -> List[Dict[str, str]]:
    """Menyusun saran modifikasi perintah CLI `aina activity edit` konkret."""
    recs = eval_result["recommendations"]
    food = recs["food"]
    dest = recs["destination"]
    outfit = recs["outfit"]
    weather_cond = weather["condition"]
    is_rain = weather["is_rainy"]
    temp = weather["temperature_c"]

    suggestions = []

    # 1. Modifikasi Status Senja / Sore
    senja_note = (
        f"Kondisi cuaca Pontianak sore ini: {weather_cond} ({temp}°C). "
        f"Jika santai sore, nikmati {food['name']} ({food['desc']}) atau mampir ke {dest['name']}. "
        f"Gunakan busana `{outfit['preset']}` agar serasi dengan suasana."
    )
    suggestions.append({
        "slug": "status_senja",
        "action": "edit",
        "cmd": f'aina activity edit status_senja --append "{senja_note}" --file',
        "description": f"Sematkan nuansa sore Pontianak ({food['name']} & {dest['name']}) ke status senja hari ini."
    })

    # 2. Modifikasi Status Siang jika Terik
    if temp >= 30.0 or not is_rain:
        siang_note = (
            f"Cuaca Pontianak siang ini terik ({temp}°C). Jangan lupa rehat sejenak dan hidrasi dengan {food['name']} dingin. "
            f"Tetap sejuk dan semangat selesaikan tugas kawan-kawan."
        )
        suggestions.append({
            "slug": "status_siang",
            "action": "edit",
            "cmd": f'aina activity edit status_siang --append "{siang_note}" --file',
            "description": f"Tambahkan pengingat cuaca panas khatulistiwa dan kesegaran {food['name']} pada status siang."
        })

    # 3. Modifikasi Status Malam untuk Stargaze / Warkop
    malam_note = (
        f"Suasana malam khatulistiwa: {weather_cond}. Refleksi tenang di {dest['name']} "
        f"sambil mengenakan {outfit['preset']} ditemani aroma sedap Kopi Saring Pontianak."
    )
    suggestions.append({
        "slug": "status_malam",
        "action": "edit",
        "cmd": f'aina activity edit status_malam --append "{malam_note}" --file',
        "description": f"Tambahkan ketenangan malam hari di {dest['name']} ke status malam."
    })

    # 4. Bridge ke Experience Library: Kristalisasi Pelajaran jika Ada Tugas yang Gagal
    perf = eval_result.get("performance", {})
    failed_items = perf.get("failed_task_items", [])
    for fr in failed_items:
        err_msg = (fr.get("error") or "execution error").replace('"', "'")[:40]
        sug_cmd = f'aina experience add --cue "{fr["title"]} gagal: {err_msg}" --lesson "<solusi perbaikan>" --evidence "Tercatat di run #{fr["id"]}" --domain "system"'
        suggestions.append({
            "slug": f"experience_{fr['id']}",
            "action": "experience_distill",
            "cmd": sug_cmd,
            "description": f"Kristalisasi kegagalan teknis tugas `{fr['title']}` ke dalam Experience Library (Harness² format)."
        })

    return suggestions


def cmd_inspect(args: argparse.Namespace) -> None:
    """Menampilkan ringkasan apa yang telah dikerjakan Aina hari ini."""
    runs = harvest_daily_runs(args.db, limit=args.limit)
    journal = harvest_recent_status_journal(args.journal, limit=args.limit)
    audits = harvest_chat_audits(args.db, limit=args.limit)
    weather = fetch_pontianak_weather()

    if args.json:
        print(json.dumps({
            "weather": weather,
            "scheduled_runs_today": runs,
            "recent_status_journal": journal,
            "chat_audits_today": audits
        }, indent=2, ensure_ascii=False))
        return

    print("=" * 68)
    print("🌅 RINGKASAN AKTIVITAS HARIAN AINA (EMPIRICAL INSPECTION)")
    print("=" * 68)
    print(f"📍 Lokasi & Cuaca: {weather['summary']}")
    print(f"⏰ Waktu Sekarang: {get_wib_now().strftime('%A, %d %B %Y - %H:%M:%S WIB')}")
    print("-" * 68)

    print(f"📋 Tugas Terjadwal Berjalan Hari Ini ({len(runs)} eksekusi):")
    if not runs:
        print("   (Belum ada eksekusi tugas terjadwal tercatat hari ini)")
    else:
        for r in runs:
            icon = "✅" if r["status"] == "success" else "❌"
            print(f"   {icon} [{r['time_str']}] {r['title']} ({r['duration_secs']:.1f}s)")
            if r["error"]:
                print(f"      ⚠️ Error: {r['error']}")
                matching = find_matching_experience(r["error"][:30], args.db) or find_matching_experience(r["title"], args.db)
                if matching:
                    print(f"      💡 Solusi Teruji (Experience Library): \"{matching['lesson']}\"")
                else:
                    print(f"      👉 Rekomendasi: `aina experience add --cue \"{r['title']} error: {r['error'][:40]}\" --lesson \"<solusi>\" --evidence \"Run {r['id']}\"`")
            elif r["output_preview"]:
                print(f"      💬 Output: {r['output_preview']}")

    print("-" * 68)
    exps = harvest_experience_records(args.db, limit=3)
    print(f"🧠 Perpustakaan Pengalaman Teknis Terkini ({len(exps)} entri teruji):")
    if not exps:
        print("   (Belum ada entri pengalaman. Simpan via `aina experience add`)")
    else:
        for e in exps:
            print(f"   • [{e['domain']}] Pemicu: \"{e['cue']}\"")
            print(f"     -> Pelajaran: \"{e['lesson']}\" (Confidence: {e['confidence']:.2f})")

    print("-" * 68)
    print(f"📸 Riwayat Status WhatsApp ({len(journal)} publikasi terakhir):")
    if not journal:
        print("   (Belum ada publikasi status WhatsApp di file jurnal)")
    else:
        for j in journal[-4:]:
            print(f"   • [{j.get('time_str', 'N/A')}] Tema: {j.get('theme', 'N/A')} | Outfit: {j.get('outfit', 'N/A')}")
            print(f"     \"{j.get('caption', '')[:100]}...\"")

    print("-" * 68)
    print(f"💬 Interaksi Chat Terkini ({len(audits)} aksi):")
    if not audits:
        print("   (Nihil aksi chat tercatat hari ini)")
    else:
        for a in audits[-3:]:
            tools = ", ".join(a["tools"]) if a["tools"] else "none"
            print(f"   • [{a['time_str']}] Chat: {a['chat_jid'][:15]}... | Tools: [{tools}] ({a['duration_secs']:.1f}s)")

    print("=" * 68)


def cmd_weather(args: argparse.Namespace) -> None:
    """Mengambil kondisi cuaca Pontianak terkini."""
    w = fetch_pontianak_weather()
    if args.json:
        print(json.dumps(w, indent=2, ensure_ascii=False))
    else:
        print(f"🌤️ Cuaca Faktual Pontianak: {w['summary']}")
        print(f"   Suhu: {w['temperature_c']}°C | Kelembapan: {w['humidity_pct']}% | Curah Hujan: {w['precipitation_mm']}mm")
        print(f"   Status Hujan: {'Ya (Gerimis/Hujan)' if w['is_rainy'] else 'Tidak (Kering/Cerah/Berawan)'}")


def cmd_evaluate(args: argparse.Namespace) -> None:
    """Mengevaluasi pengalaman hari ini dan menghitung tingkat kebosanan & variasi."""
    runs = harvest_daily_runs(args.db, limit=args.limit)
    journal = harvest_recent_status_journal(args.journal, limit=args.limit)
    weather = fetch_pontianak_weather()

    res = evaluate_daily_experience(runs, journal, weather)
    if args.json:
        print(json.dumps(res, indent=2, ensure_ascii=False))
        return

    perf = res["performance"]
    nov = res["novelty"]
    recs = res["recommendations"]

    print("=" * 68)
    print("🧠 EVALUASI REFLEKSI DIRI & VARIASI RUTINITAS AINA")
    print("=" * 68)
    print(f"📅 Waktu Evaluasi: {res['timestamp_wib']}")
    print(f"🌤️ Kondisi Cuaca : {weather['summary']}")
    print("-" * 68)
    print("📊 Kinerja Pelaksanaan Tugas:")
    print(f"   • Total Run Hari Ini: {perf['total_runs_today']}")
    print(f"   • Berhasil          : {perf['success_runs']}")
    print(f"   • Gagal/Kendala     : {perf['failed_runs']}")
    print("-" * 68)
    print("🎨 Analisis Variasi & Deteksi Kebosanan:")
    print(f"   • Skor Kebosanan    : {nov['boredom_score']} (0.0=Segar, 1.0=Jenuh)")
    print(f"   • Status Kejenuhan  : {'⚠️ PERLU VARIASI BARU' if nov['is_stagnant'] else '✅ Variasi Masih Terjaga'}")
    print(f"   • Tema Terakhir     : {', '.join(nov['recent_themes']) if nov['recent_themes'] else '-'}")
    print(f"   • Busana Terakhir   : {', '.join(nov['recent_outfits']) if nov['recent_outfits'] else '-'}")
    print("-" * 68)
    print("💡 Rekomendasi Adaptasi Harmonis:")
    print(f"   🍲 Kuliner Pontianak: {recs['food']['name']}")
    print(f"      \"{recs['food']['desc']}\"")
    print(f"   🚶 Destinasi Santai : {recs['destination']['name']}")
    print(f"      \"{recs['destination']['desc']}\"")
    print(f"   👗 Pilihan Busana   : `{recs['outfit']['preset']}`")
    print(f"      Alasan: {recs['outfit']['reason']}")
    print("=" * 68)


def cmd_suggest(args: argparse.Namespace) -> None:
    """Menyusun saran modifikasi perintah aktivitas CLI yang siap dieksekusi."""
    runs = harvest_daily_runs(args.db, limit=args.limit)
    journal = harvest_recent_status_journal(args.journal, limit=args.limit)
    weather = fetch_pontianak_weather()

    res = evaluate_daily_experience(runs, journal, weather)
    suggestions = generate_adaptation_suggestions(res, weather)

    if args.json:
        print(json.dumps({
            "evaluation": res,
            "suggestions": suggestions
        }, indent=2, ensure_ascii=False))
        return

    print("=" * 68)
    print("🛠️ SARAN MODIFIKASI AKTIVITAS OTONOM (SELF-ADAPTATION)")
    print("=" * 68)
    print(f"Refleksi hari ini menghasilkan {len(suggestions)} rekomendasi adaptasi:")
    print("-" * 68)
    for i, s in enumerate(suggestions, 1):
        print(f"[{i}] Target Aktivitas: `{s['slug']}`")
        print(f"    Tujuan: {s['description']}")
        print(f"    Perintah CLI Siap Eksekusi:")
        print(f"    👉  {s['cmd']}")
        print()
    if res.get("web_search_prompts"):
        print("-" * 68)
        print("🌐 REKOMENDASI RISET WEB TERARAH (GATED NOVELTY WEB SEARCH):")
        print("Gunakan tool `search_web` untuk verifikasi fakta riil dan bumbu sensorik baru:")
        for i, wp in enumerate(res["web_search_prompts"], 1):
            print(f"   [{i}] Topik : {wp['topic']}")
            print(f"       Query : search_web: \"{wp['query']}\"")
            print(f"       Alasan: {wp['reason']}")

    print("=" * 68)
    print("💡 Petunjuk: Anda dapat menyalin perintah di atas atau menjalankan:")
    print("   python3 skills/activity-reflection/scripts/reflect.py adapt --apply")
    print("=" * 68)


def cmd_adapt(args: argparse.Namespace) -> None:
    """Mengeksekusi penyesuaian aktivitas secara langsung atau via dry-run."""
    runs = harvest_daily_runs(args.db, limit=args.limit)
    journal = harvest_recent_status_journal(args.journal, limit=args.limit)
    weather = fetch_pontianak_weather()

    res = evaluate_daily_experience(runs, journal, weather)
    suggestions = generate_adaptation_suggestions(res, weather)

    if not suggestions:
        print("Tidak ada aktivitas yang memerlukan adaptasi saat ini. Rutinitas berjalan optimal.")
        return

    print(f"Ditemukan {len(suggestions)} saran adaptasi aktivitas.")
    for s in suggestions:
        cmd = s["cmd"]
        print(f"\n[Menjalankan]: {cmd}")
        if args.apply:
            import subprocess
            try:
                ret = subprocess.run(cmd, shell=True, capture_output=True, text=True)
                if ret.returncode == 0:
                    print(f"✅ Sukses: {ret.stdout.strip()}")
                else:
                    print(f"⚠️ Gagal (exit {ret.returncode}): {ret.stderr.strip()}")
            except Exception as e:
                print(f"❌ Error eksekusi: {e}")
        else:
            print("   (Dry-Run: gunakan flag `--apply` untuk mengeksekusi langsung ke sistem)")



def cmd_search_queries(args: argparse.Namespace) -> None:
    """Menampilkan query riset web yang direkomendasikan berdasarkan kondisi saat ini."""
    runs = harvest_daily_runs(args.db, limit=args.limit)
    journal = harvest_recent_status_journal(args.journal, limit=args.limit)
    weather = fetch_pontianak_weather()

    res = evaluate_daily_experience(runs, journal, weather)
    prompts = res.get("web_search_prompts", [])

    if args.json:
        print(json.dumps(prompts, indent=2, ensure_ascii=False))
        return

    print("=" * 68)
    print("🌐 FORMULASI RISET WEB TERARAH (GATED WEB SEARCH QUERIES)")
    print("=" * 68)
    for i, p in enumerate(prompts, 1):
        print(f"[{i}] Topik  : {p['topic']}")
        print(f"    Query  : search_web: \"{p['query']}\"")
        print(f"    Alasan : {p['reason']}")
        print()
    print("=" * 68)


def cmd_experience(args: argparse.Namespace) -> None:
    """Bridge ke Experience Library (Harness² Paradigm): memantau pelajaran teknis teruji."""
    exps = harvest_experience_records(args.db, limit=args.limit)
    if args.json:
        print(json.dumps(exps, indent=2, ensure_ascii=False))
        return

    print("=" * 68)
    print("🧠 BRIDGE: EXPERIENCE LIBRARY AINA (HARNESS² PARADIGM)")
    print("=" * 68)
    if not exps:
        print("Belum ada pelajaran teknis teruji di perpustakaan pengalaman.")
        print("Tambahkan dengan perintah: aina experience add --cue ... --lesson ... --evidence ...")
    else:
        print(f"Ditemukan {len(exps)} entri pelajaran teruji tersimpan:\n")
        for i, e in enumerate(exps, 1):
            print(f"[{i}] ID: `{e['id']}` (Domain: `{e['domain']}` | Confidence: {e['confidence']:.2f})")
            print(f"    🎯 Pemicu   : \"{e['cue']}\"")
            print(f"    💡 Pelajaran : \"{e['lesson']}\"")
            print(f"    🔬 Bukti     : \"{e['evidence']}\"")
            print()
    print("=" * 68)


def main():
    parent_parser = argparse.ArgumentParser(add_help=False)
    parent_parser.add_argument("--db", default=DEFAULT_DB_PATH, help="Path ke database SQLite aina.db")
    parent_parser.add_argument("--journal", default=DEFAULT_JOURNAL_PATH, help="Path ke status_journal.jsonl")
    parent_parser.add_argument("--limit", type=int, default=10, help="Jumlah baris riwayat yang dibaca")
    parent_parser.add_argument("--json", action="store_true", help="Output dalam format JSON murni")

    parser = argparse.ArgumentParser(
        description="Aina Autonomous Activity Reflection & Self-Adaptation Engine",
        parents=[parent_parser]
    )

    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # Subcommand: inspect
    p_inspect = subparsers.add_parser("inspect", parents=[parent_parser], help="Ringkasan apa yang telah dikerjakan hari ini")
    p_inspect.set_defaults(func=cmd_inspect)

    # Subcommand: weather
    p_weather = subparsers.add_parser("weather", parents=[parent_parser], help="Cek cuaca faktual Pontianak secara real-time")
    p_weather.set_defaults(func=cmd_weather)

    # Subcommand: evaluate
    p_eval = subparsers.add_parser("evaluate", parents=[parent_parser], help="Evaluasi kebosanan, variasi, dan rekomendasi harmonis")
    p_eval.set_defaults(func=cmd_evaluate)

    # Subcommand: suggest
    p_sugg = subparsers.add_parser("suggest", parents=[parent_parser], help="Saran modifikasi perintah aktivitas CLI yang siap pakai")
    p_sugg.set_defaults(func=cmd_suggest)

    # Subcommand: search-queries
    p_search = subparsers.add_parser("search-queries", parents=[parent_parser], help="Rekomendasi query pencarian web terarah")
    p_search.set_defaults(func=cmd_search_queries)

    # Subcommand: experience (Bridge ke Harness² Experience Library)
    p_exp = subparsers.add_parser("experience", parents=[parent_parser], help="Pantau pelajaran teknis teruji dari Experience Library")
    p_exp.set_defaults(func=cmd_experience)

    # Subcommand: adapt
    p_adapt = subparsers.add_parser("adapt", parents=[parent_parser], help="Terapkan adaptasi aktivitas ke scheduler dan database")
    p_adapt.add_argument("--apply", action="store_true", help="Eksekusi perubahan secara nyata (bukan dry-run)")
    p_adapt.set_defaults(func=cmd_adapt)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
