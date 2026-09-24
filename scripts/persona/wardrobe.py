"""
scripts/persona/wardrobe.py
---------------------------
WardrobeManager: Single Responsibility module to manage Aina's dynamic wardrobe,
track outfit repetition/boredom, enforce weekly custom styling guardrails (max 1-2x/week),
and support on-the-spot outfit creation.
"""

import os
import json
import random
from datetime import datetime, timezone, timedelta
from typing import Dict, Any, List, Optional, Tuple

WIB = timezone(timedelta(hours=7))

# 5 Preset Busana Utama (Zero Visual Drifting)
WARDROBE_PRESETS = {
    "wfh_cozy": "oversized cozy knit sweater in soft cream and lavender tones, comfortable relaxed culottes, indoor slippers, minimalist reading glasses",
    "smart_casual": "crisp white collared cotton shirt with a knit vest, tailored grey trousers, clean white sneakers, canvas tote bag",
    "outdoor_nature": "breezy light pastel cotton blouse, rolled-up linen trousers, cotton bucket hat, canvas crossbody bag",
    "night_stargaze": "thick warm navy-blue fleece hoodie or parka with warm hood, cozy jogger pants, fingerless knit gloves, holding steaming ceramic mug",
    "celestial_sig": "celestial navy-blue robe and dress with delicate gold constellation star embroidery and subtle stardust motifs"
}

# Inspirasi busana baru terarah saat bosan (Novelty Styling Seeds)
AUTONOMOUS_OUTFIT_SEEDS = [
    {
        "slug": "pastel_sage_linen",
        "desc": "breezy light sage-green cotton blouse, comfortable rolled-up beige linen trousers, and woven canvas tote bag",
        "vibes": ["siang", "sore", "outdoor", "cafe"]
    },
    {
        "slug": "cozy_oatmeal_cardigan",
        "desc": "warm oversized oatmeal knit cardigan over a soft white round-neck top, relaxed slate-grey culottes, and silver hair accessory",
        "vibes": ["pagi", "sore", "malam", "indoor", "hujan"]
    },
    {
        "slug": "vintage_lavender_vest",
        "desc": "crisp mandarin-collar cotton shirt with a pastel lavender knit vest, tailored pleated trousers, and minimalist canvas bag",
        "vibes": ["siang", "sore", "cafe", "toko_buku"]
    },
    {
        "slug": "autumn_peach_windbreaker",
        "desc": "light peach windbreaker jacket over a relaxed cotton tee, comfortable joggers, and neat white walking sneakers",
        "vibes": ["pagi", "sore", "outdoor", "jalan"]
    },
    {
        "slug": "nocturnal_cashmere_wrap",
        "desc": "soft dark-navy cashmere knit wrap sweater, cozy flannel lounge pants, and delicate starlight-motif brooch",
        "vibes": ["malam", "indoor", "stargaze"]
    }
]


class WardrobeManager:
    """Manajer lemari pakaian dinamis & pelacak kebosanan busana Aina."""

    @classmethod
    def get_preset_desc(cls, outfit_key: str) -> Optional[str]:
        return WARDROBE_PRESETS.get(outfit_key)

    @classmethod
    def get_weekly_custom_outfits(cls, journal: List[Dict[str, Any]], now: Optional[datetime] = None) -> List[Dict[str, Any]]:
        """Mendapatkan daftar busana kustom yang dipakai dalam 7 hari terakhir."""
        if now is None:
            now = datetime.now(WIB)
        cutoff_epoch = int((now - timedelta(days=7)).timestamp())

        custom_list = []
        for entry in journal:
            epoch = entry.get("timestamp_epoch")
            if not epoch:
                # Fallback to date parsing
                try:
                    dt = datetime.strptime(entry.get("date", ""), "%Y-%m-%d").replace(tzinfo=WIB)
                    epoch = int(dt.timestamp())
                except Exception:
                    continue

            if epoch >= cutoff_epoch:
                outfit = entry.get("outfit", "")
                is_custom = entry.get("is_custom_outfit", False) or outfit.startswith("custom:")
                if is_custom:
                    custom_list.append(entry)

        return custom_list

    @classmethod
    def detect_outfit_boredom(cls, journal: List[Dict[str, Any]], now: Optional[datetime] = None) -> Dict[str, Any]:
        """
        Mendeteksi kebosanan pakaian berdasarkan riwayat status:
        - Memeriksa 5 outfit terakhir.
        - Memeriksa pengulangan outfit >= 3 kali beruntun atau dominasi 4 dari 5 status.
        - Menghitung kuota kreasi busana kustom mingguan (maksimal 2x per minggu).
        """
        if now is None:
            now = datetime.now(WIB)

        recent_entries = journal[-5:] if journal else []
        recent_outfits = [e.get("outfit") for e in recent_entries if e.get("outfit")]

        weekly_custom = cls.get_weekly_custom_outfits(journal, now)
        weekly_custom_count = len(weekly_custom)
        can_create_custom = weekly_custom_count < 2

        boredom_score = 0
        reasons = []
        repeated_outfit = None

        if recent_outfits:
            # Cek pengulangan berurutan 3 kali
            if len(recent_outfits) >= 3 and len(set(recent_outfits[-3:])) == 1:
                repeated_outfit = recent_outfits[-1]
                boredom_score += 40
                reasons.append(f"Outfit '{repeated_outfit}' dipakai 3 kali berturut-turut")

            # Cek dominasi outfit yang sama dalam 5 riwayat terakhir
            counts = {o: recent_outfits.count(o) for o in set(recent_outfits)}
            for outf, cnt in counts.items():
                if cnt >= 4:
                    if outf != repeated_outfit:
                        repeated_outfit = outf
                    boredom_score += 35
                    reasons.append(f"Outfit '{outf}' sangat mendominasi ({cnt} dari {len(recent_outfits)} status terakhir)")

        is_bored = boredom_score >= 35

        return {
            "is_bored": is_bored,
            "boredom_score": boredom_score,
            "repeated_outfit": repeated_outfit,
            "recent_outfits": recent_outfits,
            "weekly_custom_count": weekly_custom_count,
            "weekly_custom_limit": 2,
            "can_create_custom": can_create_custom,
            "reasons": reasons,
        }

    @classmethod
    def resolve_outfit(
        cls,
        slot: str,
        weekend: bool,
        activity: Dict[str, Any],
        journal: List[Dict[str, Any]],
        custom_clothes: Optional[str] = None,
        requested_outfit: Optional[str] = None,
        now: Optional[datetime] = None
    ) -> Tuple[str, str, bool, Optional[str]]:
        """
        Menentukan busana akhir:
        Mengembalikan tuple: (outfit_key, clothes_desc, is_custom, reason)
        """
        if now is None:
            now = datetime.now(WIB)

        boredom = cls.detect_outfit_boredom(journal, now)

        # 1. Jika pengguna/agen secara eksplisit memberikan busana kustom (--clothes)
        if custom_clothes and custom_clothes.strip():
            slug = f"custom_on_the_spot_{now.strftime('%m%d_%H%M')}"
            reason = "Kreasi busana mandiri on-the-spot oleh Aina."
            if not boredom["can_create_custom"]:
                reason += f" (Peringatan: Kuota kustom minggu ini sudah mencapai {boredom['weekly_custom_count']}/2)"
            return slug, custom_clothes.strip(), True, reason

        # 2. Jika ada requested_outfit spesifik dari preset
        if requested_outfit and requested_outfit in WARDROBE_PRESETS:
            return requested_outfit, WARDROBE_PRESETS[requested_outfit], False, f"Memilih preset resmi '{requested_outfit}'."

        # 3. Alur Otomatis Berbasis Novelty & Anti-Kebosanan
        scene_lower = activity.get("scene", "").lower()
        theme_lower = activity.get("theme", "").lower()

        # Bila terdeteksi bosan terhadap pakaian tertentu:
        if boredom["is_bored"]:
            # Jika kuota mingguan masih ada (< 2x), racik busana kustom baru yang elegan
            if boredom["can_create_custom"]:
                # Pilih benih busana yang belum pernah dipakai baru-baru ini
                candidates = [s for s in AUTONOMOUS_OUTFIT_SEEDS if s["slug"] not in boredom["recent_outfits"]]
                if not candidates:
                    candidates = AUTONOMOUS_OUTFIT_SEEDS
                chosen_seed = random.choice(candidates)
                slug = f"custom:{chosen_seed['slug']}"
                reason = f"Bosan dengan outfit '{boredom['repeated_outfit']}'. Meracik kreasi busana baru '{chosen_seed['slug']}' (Kuota kustom: {boredom['weekly_custom_count'] + 1}/2 minggu ini)."
                return slug, chosen_seed["desc"], True, reason
            else:
                # Kuota kustom minggu ini habis -> rotasi ke preset lain yang berbeda
                other_presets = [
                    k for k in WARDROBE_PRESETS.keys()
                    if k != boredom["repeated_outfit"] and k not in boredom["recent_outfits"][-2:]
                ]
                if not other_presets:
                    other_presets = [k for k in WARDROBE_PRESETS.keys() if k != boredom["repeated_outfit"]]

                alt_outfit = random.choice(other_presets)
                reason = f"Bosan dengan outfit '{boredom['repeated_outfit']}', tetapi kuota kustom minggu ini penuh ({boredom['weekly_custom_count']}/2). Melakukan rotasi ke preset '{alt_outfit}'."
                return alt_outfit, WARDROBE_PRESETS[alt_outfit], False, reason

        # 4. Kondisi Normal (Tidak Bosan): Pilih preset terbaik sesuai konteks aktivitas
        if "kafe" in scene_lower or "cafe" in scene_lower or "toko buku" in scene_lower:
            outfit_key = "smart_casual"
        elif slot == "malam":
            outfit_key = "night_stargaze" if ("stargaz" in scene_lower or "luar" in scene_lower) else "wfh_cozy"
        elif weekend:
            outfit_key = "outdoor_nature"
        elif slot in ["siang", "sore"] and ("jalan" in scene_lower or "keliling" in scene_lower):
            outfit_key = "smart_casual"
        else:
            outfit_key = "wfh_cozy"

        clothes_desc = activity.get("anchor_clothes") or WARDROBE_PRESETS[outfit_key]
        return outfit_key, clothes_desc, False, f"Preset '{outfit_key}' sesuai konteks waktu ({slot}) dan adegan."
