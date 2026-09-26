#!/usr/bin/env python3
"""
tests/test_persona_status.py - Comprehensive Unit Tests for Persona Status Engine

Covers:
1. StatusSafetyGuard: Zero-error enforcement, error-pattern detection, media integrity validation.
2. AtmosphereEngine: Contextual Makoto Shinkai atmosphere (zero forced clouds indoors/night, dramatic sky outdoors).
3. WardrobeManager: Boredom detection, 7-day sliding window custom outfit quota, deterministic resolution.
4. ImageCaptionEngine: Post-image inspection, adaptive caption synthesis, safe fallback.
"""

import os
import sys
import tempfile
import unittest
from pathlib import Path
from datetime import datetime, timezone, timedelta

# Ensure repo root is on sys.path
REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT))

from scripts.persona.safety import (
    StatusSafetyGuard,
    DEFAULT_SAFE_IMPACT_MAXXING_CAPTION,
    WhatsAppStatusCaptionPayload,
)
from scripts.persona.atmosphere import AtmosphereEngine
from scripts.persona.wardrobe import WardrobeManager, WARDROBE_PRESETS
from scripts.persona.captioner import ImageCaptionEngine


class TestStatusSafetyGuard(unittest.TestCase):
    """Test zero-error enforcement and media safety checks."""

    def test_valid_wholesome_caption(self):
        text = "Sore yang tenang di tepi pantai... Anginnya sepoi-sepoi banget yaa ✨"
        is_valid, reason = StatusSafetyGuard.validate_status_text(text)
        self.assertTrue(is_valid)
        self.assertEqual(reason, "")

        sanitized = StatusSafetyGuard.sanitize_caption(text)
        self.assertEqual(sanitized, text)

    def test_block_error_keywords(self):
        error_samples = [
            "Error: Connection refused to whatsmeow server at port 8080",
            "Traceback (most recent call last):\n  File 'test.py', line 10, in <module>",
            "Exception: Task id 'task-123' finished with result: failed",
            "SyntaxError: invalid syntax in script.py line 45",
            "500 Internal Server Error: Gateway timeout",
            "fatal: ambiguous argument 'HEAD': unknown revision",
            "Command failed with exit code 1",
            "CRITICAL: unhandled promise rejection",
            "NullPointerException at com.whatsapp.net",
        ]
        for err in error_samples:
            is_valid, reason = StatusSafetyGuard.validate_status_text(err)
            self.assertFalse(is_valid, f"Should reject error sample: {err}")
            self.assertIn("error/dump teknis", reason)

            # Sanitization must replace error text with safe fallback
            sanitized = StatusSafetyGuard.sanitize_caption(err)
            self.assertNotEqual(sanitized, err)
            self.assertNotIn("Error", sanitized)
            self.assertNotIn("Traceback", sanitized)
            self.assertIn("yaa", sanitized)

    def test_block_empty_or_whitespace_caption(self):
        is_valid, reason = StatusSafetyGuard.validate_status_text("   \n  \t ")
        self.assertFalse(is_valid)
        self.assertIn("kosong", reason.lower())

    def test_strict_json_extraction(self):
        """Strict JSON payload should extract clean caption without any JSON markup."""
        json_samples = [
            ('{"caption": "Pagi kawan-kawan! Secangkir kopi hangat dulu sebelum ngoding remote ✨"}',
             "Pagi kawan-kawan! Secangkir kopi hangat dulu sebelum ngoding remote ✨"),
            ('```json\n{"caption": "Senja hari ini indah bangeett di ufuk barat ✨"}\n```',
             "Senja hari ini indah bangeett di ufuk barat ✨"),
            ('{"message": "Selamat malam semuanya, istirahat yang cukup yaa 🌙"}',
             "Selamat malam semuanya, istirahat yang cukup yaa 🌙"),
        ]
        for raw, expected in json_samples:
            cleaned = StatusSafetyGuard.extract_caption_from_raw(raw)
            self.assertEqual(cleaned, expected)
            sanitized = StatusSafetyGuard.sanitize_caption(raw)
            self.assertEqual(sanitized, expected)

    def test_strip_meta_leaks_and_preambles(self):
        """Status/Story meta labels like 'status whatsapp story:' must be completely stripped."""
        leak_samples = [
            ("status whatsapp story: Pagi semuanyaa! Semangat memulai hari remote yaa ✨",
             "Pagi semuanyaa! Semangat memulai hari remote yaa ✨"),
            ("Status WhatsApp Story:\nSecangkir teh chamomile hangat penutup hari yang menenangkan ✨",
             "Secangkir teh chamomile hangat penutup hari yang menenangkan ✨"),
            ("Status WA Story - Pagi: Udara sejuk pagi ini bikin suasana ngoding jadi tenang bangeett!",
             "Udara sejuk pagi ini bikin suasana ngoding jadi tenang bangeett!"),
            ("Berikut adalah status whatsapp story:\n\nRehat sejenak dan nikmati indahnya senja hari ini ✨",
             "Rehat sejenak dan nikmati indahnya senja hari ini ✨"),
            ("Berikut caption status WhatsApp: Makan siang dulu yuk kawan-kawan ✨",
             "Makan siang dulu yuk kawan-kawan ✨"),
            ("**Caption Status WhatsApp Story:**\nJangan lupa istirahatkan mata sejenak yaa!",
             "Jangan lupa istirahatkan mata sejenak yaa!"),
            ("### Status Story\nMenikmati semilir angin di taman kota sore ini ✨",
             "Menikmati semilir angin di taman kota sore ini ✨"),
            ('"Pagi kawan-kawan! Selamat beraktivitas hari ini yaa ✨"',
             "Pagi kawan-kawan! Selamat beraktivitas hari ini yaa ✨"),
            # Double leak test: Preamble + JSON containing prefix
            ('Tentu, ini status whatsapp story:\n```json\n{"caption": "Status WA Story: Selamat pagi semuanya ✨"}\n```',
             "Selamat pagi semuanya ✨"),
        ]
        for leaked, expected in leak_samples:
            sanitized = StatusSafetyGuard.sanitize_caption(leaked)
            self.assertEqual(sanitized, expected, f"Failed on sample: {leaked}")
            self.assertFalse(sanitized.lower().startswith("status"))
            self.assertFalse(sanitized.lower().startswith("caption"))
            self.assertFalse(sanitized.lower().startswith("berikut"))

    def test_strict_typed_schema_validation(self):
        """Test strongly-typed schema model and validate_typed_caption function."""
        # 1. Valid Dict with caption
        payload = {"caption": "Pagi semuanyaa! Selamat beraktivitas hari ini yaa ✨"}
        is_valid, clean, reason = StatusSafetyGuard.validate_typed_caption(payload)
        self.assertTrue(is_valid)
        self.assertEqual(clean, "Pagi semuanyaa! Selamat beraktivitas hari ini yaa ✨")
        self.assertEqual(reason, "")

        # 2. Dict with fallback key 'text' and leaked prefix
        payload2 = {"text": "Status WhatsApp Story: Secangkir kopi hangat dulu yuk!"}
        is_valid, clean2, reason2 = StatusSafetyGuard.validate_typed_caption(payload2)
        self.assertTrue(is_valid)
        self.assertEqual(clean2, "Secangkir kopi hangat dulu yuk!")

        # 3. Invalid payload (missing caption field)
        payload_missing = {"error": "Something went wrong"}
        is_valid, clean_missing, reason_missing = StatusSafetyGuard.validate_typed_caption(payload_missing)
        self.assertFalse(is_valid)
        self.assertIn("Field 'caption' kosong", reason_missing)
        self.assertEqual(clean_missing, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION)

        # 4. Error dump in caption field
        payload_err = {"caption": "Internal server error 500: Gateway timeout"}
        is_valid, clean_err, reason_err = StatusSafetyGuard.validate_typed_caption(payload_err)
        self.assertFalse(is_valid)
        self.assertIn("error/dump teknis", reason_err)
        self.assertEqual(clean_err, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION)

        # 5. WhatsAppStatusCaptionPayload dataclass direct instantiation
        obj = WhatsAppStatusCaptionPayload.from_dict({"caption": "status whatsapp story: Semangat pagi!"})
        self.assertEqual(obj.caption, "Semangat pagi!")

        # 6. Type error rejection
        with self.assertRaises(TypeError):
            WhatsAppStatusCaptionPayload.from_dict(["bukan", "dict"])

    def test_media_file_validation(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)

            # Non-existent file
            is_valid, reason = StatusSafetyGuard.validate_status_media(str(tmp_path / "ghost.png"))
            self.assertFalse(is_valid)
            self.assertIn("tidak ditemukan", reason)

            # Empty / 0-byte file (too small)
            empty_png = tmp_path / "empty.png"
            empty_png.touch()
            is_valid, reason = StatusSafetyGuard.validate_status_media(str(empty_png))
            self.assertFalse(is_valid)
            self.assertIn("terlalu kecil", reason)

            # Corrupt header (PNG extension but invalid magic bytes)
            fake_png = tmp_path / "fake.png"
            fake_png.write_bytes(b"NOT_A_REAL_PNG_HEADER_CONTENT_PADDED_TO_1024_BYTES" * 30)
            is_valid, reason = StatusSafetyGuard.validate_status_media(str(fake_png))
            self.assertFalse(is_valid)
            self.assertIn("tidak valid", reason)

            # Valid PNG (starts with \x89PNG\r\n\x1a\n)
            valid_png = tmp_path / "valid.png"
            png_header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"
            valid_png.write_bytes(png_header + b"\x00" * 1500)
            is_valid, media_type = StatusSafetyGuard.validate_status_media(str(valid_png))
            self.assertTrue(is_valid)
            self.assertEqual(media_type, "image/png")

            # Valid JPEG (starts with \xff\xd8\xff)
            valid_jpg = tmp_path / "valid.jpg"
            jpg_header = b"\xff\xd8\xff\xe0\x00\x10JFIF"
            valid_jpg.write_bytes(jpg_header + b"\x00" * 1500)
            is_valid, media_type = StatusSafetyGuard.validate_status_media(str(valid_jpg))
            self.assertTrue(is_valid)
            self.assertEqual(media_type, "image/jpeg")


class TestAtmosphereEngine(unittest.TestCase):
    """Test dynamic Makoto Shinkai atmosphere selection without forced clouds."""

    def test_environment_detection(self):
        # Indoor keywords
        self.assertTrue(AtmosphereEngine.is_indoor_scene("meja kerja rumah, laptop, cangkir teh"))
        self.assertTrue(AtmosphereEngine.is_indoor_scene("kamar tidur, lampu temaram"))
        self.assertTrue(AtmosphereEngine.is_indoor_scene("sudut kafe, meja kayu, buku catatan"))
        self.assertTrue(AtmosphereEngine.is_indoor_scene("dapur bersih, seduh teh"))
        self.assertTrue(AtmosphereEngine.is_indoor_scene("ruang tamu", framing="desk_prop"))

        # Outdoor keywords
        self.assertFalse(AtmosphereEngine.is_indoor_scene("pantai pasir putih, ombak laut"))
        self.assertFalse(AtmosphereEngine.is_indoor_scene("bukit rumput hijau, pemandangan lembah"))
        self.assertFalse(AtmosphereEngine.is_indoor_scene("taman kota, jalan setapak pepohonan"))

    def test_indoor_scene_has_no_forced_clouds(self):
        """Indoor scene must NOT contain cumulus clouds in lighting, sky, or atmosphere."""
        atmosphere = AtmosphereEngine.build_cinematic_atmosphere(
            scene_desc="meja kerja remote, laptop dan kopi",
            slot="pagi",
            include_clouds=False
        )
        self.assertIn("dust motes", atmosphere.lower())
        self.assertNotIn("cumulus", atmosphere.lower())
        self.assertNotIn("cumulonimbus", atmosphere.lower())

    def test_night_scene_has_no_forced_clouds(self):
        """Night scene must feature starry celestial sky, not cumulus clouds."""
        atmosphere = AtmosphereEngine.build_cinematic_atmosphere(
            scene_desc="balkon menatap bintang di langit",
            slot="malam",
            include_clouds=False
        )
        self.assertIn("indigo", atmosphere.lower())
        self.assertIn("star", atmosphere.lower())
        self.assertNotIn("cumulus", atmosphere.lower())

    def test_rainy_scene_atmosphere(self):
        atmosphere = AtmosphereEngine.build_cinematic_atmosphere(
            scene_desc="gerimis rintik hujan di luar jendela",
            slot="siang",
            include_clouds=False
        )
        self.assertIn("raindrops", atmosphere.lower())
        self.assertNotIn("cumulus", atmosphere.lower())

    def test_outdoor_daytime_has_clouds_when_requested(self):
        """Outdoor daytime or sunset scene features dramatic clouds when requested."""
        atmosphere = AtmosphereEngine.build_cinematic_atmosphere(
            scene_desc="pantai pasir panjang, matahari terbenam",
            slot="sore",
            include_clouds=True
        )
        self.assertIn("katawaredoki", atmosphere.lower())
        self.assertIn("golden-lit horizon contours", atmosphere.lower())

    def test_outdoor_daytime_clear_sky_when_no_clouds(self):
        """Outdoor daytime without include_clouds produces crisp clear sky."""
        atmosphere = AtmosphereEngine.build_cinematic_atmosphere(
            scene_desc="pantai pasir panjang, matahari terbenam",
            slot="sore",
            include_clouds=False
        )
        self.assertIn("crystal clear horizon", atmosphere.lower())
        self.assertNotIn("cumulus", atmosphere.lower())


class TestWardrobeManager(unittest.TestCase):
    """Test wardrobe boredom detection, weekly quota, and resolution flow."""

    def test_detect_outfit_boredom_trigger(self):
        # 3 repeats in last 5 entries should trigger boredom
        history_bored = [
            {"outfit": "wfh_cozy", "timestamp_epoch": 1700000000},
            {"outfit": "wfh_cozy", "timestamp_epoch": 1700001000},
            {"outfit": "wfh_cozy", "timestamp_epoch": 1700002000},
            {"outfit": "smart_casual", "timestamp_epoch": 1700003000},
            {"outfit": "wfh_cozy", "timestamp_epoch": 1700004000},
        ]
        boredom = WardrobeManager.detect_outfit_boredom(history_bored)
        self.assertTrue(boredom["is_bored"])
        self.assertEqual(boredom["repeated_outfit"], "wfh_cozy")
        self.assertGreaterEqual(boredom["boredom_score"], 35)

        # Diverse history should NOT trigger boredom
        history_fresh = [
            {"outfit": "wfh_cozy", "timestamp_epoch": 1700000000},
            {"outfit": "smart_casual", "timestamp_epoch": 1700001000},
            {"outfit": "outdoor_nature", "timestamp_epoch": 1700002000},
            {"outfit": "night_stargaze", "timestamp_epoch": 1700003000},
            {"outfit": "celestial_sig", "timestamp_epoch": 1700004000},
        ]
        boredom_fresh = WardrobeManager.detect_outfit_boredom(history_fresh)
        self.assertFalse(boredom_fresh["is_bored"])

    def test_weekly_custom_outfit_quota(self):
        wib = timezone(timedelta(hours=7))
        now = datetime.now(wib)
        t_now = int(now.timestamp())
        t_2d = int((now - timedelta(days=2)).timestamp())
        t_4d = int((now - timedelta(days=4)).timestamp())
        t_10d = int((now - timedelta(days=10)).timestamp())

        # 2 custom outfits in past 4 days (within 7-day window), 1 older from 10 days ago
        journal = [
            {"outfit": "custom:pastel_sage", "is_custom_outfit": True, "timestamp_epoch": t_10d},
            {"outfit": "custom:cozy_cardigan", "is_custom_outfit": True, "timestamp_epoch": t_4d},
            {"outfit": "custom:vintage_vest", "is_custom_outfit": True, "timestamp_epoch": t_2d},
            {"outfit": "wfh_cozy", "is_custom_outfit": False, "timestamp_epoch": t_now},
        ]

        weekly_custom = WardrobeManager.get_weekly_custom_outfits(journal, now)
        self.assertEqual(len(weekly_custom), 2)

        boredom = WardrobeManager.detect_outfit_boredom(journal, now)
        self.assertEqual(boredom["weekly_custom_count"], 2)
        self.assertFalse(boredom["can_create_custom"])

    def test_resolve_outfit_deterministic_fallback(self):
        """When user provides explicit outfit, it is respected."""
        code, desc, is_custom, reason = WardrobeManager.resolve_outfit(
            slot="pagi",
            weekend=False,
            activity={"scene": "meja kerja"},
            journal=[],
            requested_outfit="outdoor_nature"
        )
        self.assertEqual(code, "outdoor_nature")
        self.assertEqual(desc, WARDROBE_PRESETS["outdoor_nature"])
        self.assertFalse(is_custom)

    def test_resolve_custom_outfit_with_clothes_flag(self):
        """Custom clothes description passed directly should be marked as custom outfit."""
        code, desc, is_custom, reason = WardrobeManager.resolve_outfit(
            slot="siang",
            weekend=False,
            activity={"scene": "kafe"},
            journal=[],
            custom_clothes="Kemeja flanel merah marun dan celana kargo santai"
        )
        self.assertTrue(code.startswith("custom_on_the_spot_"))
        self.assertIn("flanel merah marun", desc)
        self.assertTrue(is_custom)

    def test_resolve_outfit_when_bored_and_quota_exhausted(self):
        """When bored but weekly quota exhausted, rotate to another preset."""
        wib = timezone(timedelta(hours=7))
        now = datetime.now(wib)
        t_now = int(now.timestamp())
        t_1d = int((now - timedelta(days=1)).timestamp())
        t_2d = int((now - timedelta(days=2)).timestamp())

        journal = [
            {"outfit": "custom:outfit_1", "is_custom_outfit": True, "timestamp_epoch": t_2d},
            {"outfit": "custom:outfit_2", "is_custom_outfit": True, "timestamp_epoch": t_1d},
            {"outfit": "wfh_cozy", "is_custom_outfit": False, "timestamp_epoch": t_now},
            {"outfit": "wfh_cozy", "is_custom_outfit": False, "timestamp_epoch": t_now},
            {"outfit": "wfh_cozy", "is_custom_outfit": False, "timestamp_epoch": t_now},
        ]

        code, desc, is_custom, reason = WardrobeManager.resolve_outfit(
            slot="pagi",
            weekend=False,
            activity={"scene": "meja kerja"},
            journal=journal,
            now=now
        )
        # Should not be wfh_cozy (because repeated), should not be custom (quota exhausted)
        self.assertNotEqual(code, "wfh_cozy")
        self.assertFalse(is_custom)
        self.assertIn("rotasi ke preset", reason)


class TestImageCaptionEngine(unittest.TestCase):
    """Test image inspection and caption generation."""

    def test_synthesize_adaptive_caption_morning_coffee(self):
        with tempfile.NamedTemporaryFile(suffix=".png") as f:
            png_header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"
            f.write(png_header + b"\x00" * 1500)
            f.flush()

            caption = ImageCaptionEngine.generate_caption_from_image(
                image_path=f.name,
                activity_context={
                    "slot": "pagi",
                    "theme": "kopi_pagi",
                    "scene": "menyeruput cangkir teh hangat di meja kerja",
                }
            )
            self.assertIsInstance(caption, str)
            self.assertGreater(len(caption), 10)
            self.assertTrue(any(w in caption.lower() for w in ["kehangatan", "pagi", "semangat", "teh"]))

    def test_synthesize_adaptive_caption_night_reflection(self):
        with tempfile.NamedTemporaryFile(suffix=".png") as f:
            png_header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"
            f.write(png_header + b"\x00" * 1500)
            f.flush()

            caption = ImageCaptionEngine.generate_caption_from_image(
                image_path=f.name,
                activity_context={
                    "slot": "malam",
                    "theme": "refleksi_malam",
                    "scene": "menatap lampu kota dari balkon kamar",
                }
            )
            self.assertIsInstance(caption, str)
            self.assertTrue(any(w in caption.lower() for w in ["malam", "istirahat", "damai", "pikiran"]))

    def test_invalid_image_uses_safe_fallback(self):
        caption = ImageCaptionEngine.generate_caption_from_image(
            image_path="/tmp/non_existent_image_12345.png",
            fallback_caption="Caption fallback yang aman dan hangat yaa ✨"
        )
        self.assertEqual(caption, "Caption fallback yang aman dan hangat yaa ✨")


if __name__ == "__main__":
    unittest.main()
