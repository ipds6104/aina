"""
scripts/persona/atmosphere.py
-----------------------------
AtmosphereEngine: Single Responsibility module to generate contextual
Makoto Shinkai & CoMix Wave Films cinematic aesthetics without forcing clouds.

Prinsip Utama:
- Awan BUKAN elemen wajib di semua adegan Makoto Shinkai.
- Adegan dalam ruangan (indoor/kamar/meja kerja/kafe) TIDAK BOLEH memuat awan luar ruangan.
- Adegan malam hari (stargazing/istirahat) menampilkan langit berbintang/bulan, bukan awan kumulus siang.
- Nuansa senja (katawaredoki) berfokus pada gradasi langit ungu-jingga, sinar crepuscular, dan pantulan air.
- Variasi cuaca mencakup langit cerah lapang, komorebi celah daun, gerimis sejuk, dan kabut pagi.
"""

import re
from typing import Optional

class AtmosphereEngine:
    """Mesin penyusun atmosfer estetika sinematik Makoto Shinkai yang variatif dan kontekstual."""

    INDOOR_KEYWORDS = [
        "meja kerja", "kamar", "ruang kerja", "kafe", "cafe", "toko buku",
        "sofa", "dapur", "laptop", "lampu meja", "dalam rumah", "indoor",
        "perpustakaan", "restoran", "jendela kamar", "selimut", "kursi"
    ]

    RAIN_KEYWORDS = [
        "hujan", "gerimis", "genangan air", "rintik", "payung", "wet pavement", "puddle"
    ]

    NIGHT_KEYWORDS = [
        "malam", "stargazing", "bintang", "milky way", "bulan", "lampion", "tidur"
    ]

    @classmethod
    def is_indoor_scene(cls, scene_desc: str, framing: Optional[str] = None) -> bool:
        """Deteksi apakah adegan berada di dalam ruangan."""
        if framing in ["desk_prop", "mirror"]:
            return True
        scene_lower = scene_desc.lower()
        return any(kw in scene_lower for kw in cls.INDOOR_KEYWORDS)

    @classmethod
    def build_cinematic_atmosphere(
        cls,
        scene_desc: str,
        slot: str,
        framing: Optional[str] = None,
        include_clouds: bool = False
    ) -> str:
        """
        Menyusun deskripsi atmosfer visual Makoto Shinkai yang kaya detail sensorik,
        disesuaikan secara presisi dengan slot waktu dan lokasi (indoor vs outdoor).
        """
        is_indoor = cls.is_indoor_scene(scene_desc, framing)
        scene_lower = scene_desc.lower()
        is_rain = any(kw in scene_lower for kw in cls.RAIN_KEYWORDS)
        is_night = slot == "malam" or any(kw in scene_lower for kw in cls.NIGHT_KEYWORDS)

        # 1. Kasus Adegan Indoor (Kamar, Meja Kerja, Kafe, Toko Buku)
        if is_indoor:
            if is_night:
                return (
                    "Exquisite Makoto Shinkai interior anime lighting, gentle warm lamplight glow, "
                    "delicate soft shadows on wooden walls, tranquil night window reflection, "
                    "intimate and soothing atmosphere, painterly background detail, 8k resolution anime film still."
                )
            else:
                return (
                    "Warm natural sunlight filtering gently through window blinds, luminous atmospheric dust motes dancing in sunbeams, "
                    "rich painterly wooden textures, soft warm shadows, peaceful and productive home atmosphere, "
                    "Makoto Shinkai interior aesthetic, 8k resolution anime film still."
                )

        # 2. Kasus Hujan / Pasca-Gerimis (Khas Kotonoha no Niwa / The Garden of Words)
        if is_rain:
            return (
                "Subtle translucent raindrops, exquisite glistening water reflections on pavement and puddles, "
                "vibrant saturated green foliage with delicate droplets, soft overcast atmospheric lighting, "
                "gentle melancholic serenity, Makoto Shinkai painterly realism, 8k resolution anime film still."
            )

        # 3. Kasus Malam Hari (Khas Kimi no Na wa / Stargazing)
        if is_night:
            return (
                "Deep indigo and violet celestial night sky, brilliant glittering stars and subtle cosmic milky way dust, "
                "soft luminous crescent moon radiance, warm ambient fairy lights and lantern bokeh in background, "
                "breathtaking nocturnal wonder, photorealistic painterly background, 8k resolution anime film still."
            )

        # 4. Kasus Senja / Golden Hour / Sore (Magic Hour / Katawaredoki)
        if slot == "sore":
            cloud_clause = "with soft golden-lit horizon contours" if include_clouds else "crystal clear horizon with golden twilight glow"
            return (
                f"Breathtaking twilight magic hour (katawaredoki), dramatic sky gradient of apricot, amber, and deep violet, "
                f"warm crepuscular god-rays piercing through the horizon, {cloud_clause}, "
                f"gleaming reflections on distant water and roads, rich emotional resonance, 8k resolution anime film still."
            )

        # 5. Kasus Pagi (Dawn / Early Morning)
        if slot == "pagi":
            cloud_clause = "gentle wisps of morning cirrus" if include_clouds else "crisp clear pale cerulean morning sky"
            return (
                f"Fresh crisp early morning illumination, soft translucent dawn mist dissolving in warmth, "
                f"dappled komorebi sunlight filtering through fresh green leaves, {cloud_clause}, "
                f"invigorating and optimistic atmosphere, photorealistic painterly background, 8k resolution anime film still."
            )

        # 6. Kasus Siang (Midday / Afternoon Outdoor)
        cloud_clause = "occasional soft wispy clouds in the distance" if include_clouds else "boundless open blue sky"
        return (
            f"Vibrant midday sunlight, expansive {cloud_clause}, lush saturated natural colors, "
            f"dynamic high-contrast anime shadows, reflective glass and water accents, "
            f"pure cheerful open-air feeling, photorealistic painterly background, 8k resolution anime film still."
        )
