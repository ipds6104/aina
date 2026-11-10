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
from scripts.persona.interest import (
    InterestDiscoveryEngine,
    INTEREST_CATEGORIES,
    CURATED_INTEREST_CATALOG,
)
from scripts.persona_status import build_makoto_shinkai_prompt


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


class TestInterestDiscoveryEngine(unittest.TestCase):
    """Test suite for Autonomous Novelty & Interest Discovery Engine."""

    def test_get_domains_and_catalog_size(self):
        domains = InterestDiscoveryEngine.get_domains()
        self.assertIn("ai_engineering", domains)
        self.assertIn("culinary", domains)
        self.assertIn("muslimah_fashion", domains)
        self.assertIn("travel_indonesia", domains)
        self.assertIn("travel_asia", domains)
        self.assertIn("travel_europe", domains)

        catalog = InterestDiscoveryEngine.list_catalog()
        self.assertGreaterEqual(len(catalog), 20)

    def test_list_catalog_aliases(self):
        ai_items = InterestDiscoveryEngine.list_catalog("ai")
        self.assertTrue(all(item["category"] == "ai_engineering" for item in ai_items))
        self.assertGreaterEqual(len(ai_items), 3)

        food_items = InterestDiscoveryEngine.list_catalog("culinary")
        self.assertTrue(all(item["category"] == "culinary" for item in food_items))
        self.assertGreaterEqual(len(food_items), 4)

        muslimah_items = InterestDiscoveryEngine.list_catalog("muslimah")
        self.assertTrue(all(item["category"] == "muslimah_fashion" for item in muslimah_items))
        self.assertGreaterEqual(len(muslimah_items), 3)

        travel_items = InterestDiscoveryEngine.list_catalog("travel")
        self.assertTrue(all(item["category"].startswith("travel_") for item in travel_items))
        self.assertGreaterEqual(len(travel_items), 10)

    def test_synthesize_activity_structure(self):
        catalog = InterestDiscoveryEngine.list_catalog("ai_engineering")
        item = catalog[0]
        act = InterestDiscoveryEngine.synthesize_activity(
            interest_item=item,
            web_findings="Paper baru tentang reasoning model"
        )
        self.assertTrue(act["theme"].startswith("interest_"))
        self.assertIn("reasoning model", act["scene"])
        self.assertIn("caption", act)
        self.assertIn("vibe", act)
        self.assertIn("key_learning", act)
        self.assertTrue(act["is_discovered_interest"])

    def test_muslimah_wardrobe_in_presets(self):
        self.assertIn("muslimah_chic", WARDROBE_PRESETS)
        self.assertIn("muslimah_syari", WARDROBE_PRESETS)
        self.assertIn("muslimah_cozy", WARDROBE_PRESETS)

        desc = WARDROBE_PRESETS["muslimah_chic"]
        self.assertIn("pashmina", desc.lower())
        self.assertIn("crescent moon and star", desc.lower())

    def test_prompt_harmonization_for_hijab(self):
        activity = {
            "theme": "interest_muslimah_test",
            "scene": "Duduk di kafe membaca buku",
            "outfit": "muslimah_chic",
            "anchor_clothes": WARDROBE_PRESETS["muslimah_chic"],
            "framing": "selfie"
        }
        prompt = build_makoto_shinkai_prompt(activity, weekend=False)
        self.assertIn("elegant modest Muslimah style", prompt)
        self.assertIn("silver crescent moon and star brooch pinned gracefully on the left side of her hijab", prompt)
        self.assertIn("starry deep blue eyes", prompt)
        # Verify it does NOT erroneously describe bare side-braided hair
        self.assertNotIn("signature side braid", prompt)

    def test_prompt_for_standard_outfit(self):
        activity = {
            "theme": "wfh_coding",
            "scene": "Duduk di depan laptop",
            "outfit": "wfh_cozy",
            "anchor_clothes": WARDROBE_PRESETS["wfh_cozy"],
            "framing": "selfie"
        }
        prompt = build_makoto_shinkai_prompt(activity, weekend=False)
        self.assertIn("signature side braid", prompt)
        self.assertIn("luminous crescent moon and star hairclip on the left side", prompt)

    def test_discover_end_to_end(self):
        discovered = InterestDiscoveryEngine.discover(
            category="travel_asia",
            search_query="spot musim gugur kyoto 2026",
            notes="Wawasan baru tentang momijigari"
        )
        self.assertIsInstance(discovered, dict)
        self.assertEqual(discovered["interest_category"], "travel_asia")
        self.assertTrue(discovered["theme"].startswith("interest_"))
        self.assertGreater(len(discovered["caption"]), 15)

    def test_recency_query_formatting(self):
        now = datetime(2026, 11, 10, 14, 0, tzinfo=timezone.utc)
        q_today = InterestDiscoveryEngine.format_recency_query("trending AI", recency="today", now=now)
        self.assertIn("hari ini", q_today)
        self.assertIn("10 November 2026", q_today)
        self.assertIn("when:1d", q_today)

        q_week = InterestDiscoveryEngine.format_recency_query("kopi viral", recency="week", now=now)
        self.assertIn("minggu ini", q_week)
        self.assertIn("when:7d", q_week)

        discovered = InterestDiscoveryEngine.discover(
            category="culinary",
            recency="today",
            web_findings="Dirty latte kopi susu dingin viral",
            now=now
        )
        self.assertIn("10 November 2026", discovered["scene"])
        self.assertIn("Update seru hari ini!", discovered["caption"])

    def test_dynamic_topic_discovery_without_catalog_seed(self):
        """Uji penemuan minat bebas (Zero Hardcode) langsung dari topik baru."""
        now = datetime(2026, 11, 10, 10, 30, tzinfo=timezone.utc)
        topic = "Video Andrej Karpathy tentang visualisasi tokenizer BPE"
        findings = "Membahas pemecahan kata menjadi representasi token byte level."

        discovered = InterestDiscoveryEngine.discover(
            topic=topic,
            web_findings=findings,
            recency="today",
            now=now
        )
        self.assertEqual(discovered["interest_category"], "ai_engineering")
        self.assertIn("tokenizer BPE", discovered["scene"])
        self.assertIn("Update seru hari ini!", discovered["caption"])
        self.assertIn("tokenizer BPE", discovered["caption"])
        self.assertTrue(discovered["is_discovered_interest"])

    def test_dynamic_muslimah_topic_deduction(self):
        """Uji deduksi busana dan suasana otomatis untuk topik fashion Muslimah."""
        now = datetime(2026, 11, 10, 16, 0, tzinfo=timezone.utc)
        topic = "Abaya syari earth-tone katun madinah"

        discovered = InterestDiscoveryEngine.discover(
            topic=topic,
            web_findings="Warna moka lembut sangat populer untuk siluet syari santun.",
            recency="today",
            now=now
        )
        self.assertEqual(discovered["interest_category"], "muslimah_fashion")
        self.assertEqual(discovered["outfit"], "muslimah_syari")
        self.assertEqual(discovered["framing"], "tripod")
        self.assertIn("bros bulan sabit perak", discovered["scene"])

    def test_dynamic_culinary_topic_deduction(self):
        """Uji deduksi busana dan framing otomatis untuk topik kuliner / minuman."""
        now = datetime(2026, 11, 10, 13, 0, tzinfo=timezone.utc)
        topic = "Pour-over kopi v60 beans sidikalang citrus"

        discovered = InterestDiscoveryEngine.discover(
            topic=topic,
            web_findings="Catatan rasa jeruk keprok segar dengan aftertaste karamel manis.",
            recency="today",
            now=now
        )
        self.assertEqual(discovered["interest_category"], "culinary")
        self.assertEqual(discovered["framing"], "desk_prop")
        self.assertIn("sidikalang", discovered["scene"])


class TestAutonomousSceneSynthesizer(unittest.TestCase):
    """Test suite untuk AutonomousSceneSynthesizer."""

    def test_autonomous_scene_synthesis_makoto_shinkai(self):
        from scripts.persona.interest import AutonomousSceneSynthesizer
        now = datetime(2026, 11, 10, 17, 30, tzinfo=timezone.utc)
        synth = AutonomousSceneSynthesizer.synthesize(
            topic="Danau Kelimutu Flores 3 warna",
            findings="Warna kawah danau toska muda saat golden hour senja.",
            category="travel_indonesia",
            slot="sore",
            now=now
        )
        self.assertIn("Aina berada", synth["scene"])
        self.assertIn("Kelimutu", synth["scene"])
        self.assertIn("golden hour", synth["scene"].lower())
        self.assertIn("Kelimutu", synth["caption"])
        self.assertIn("Update seru hari ini!", synth["caption"])
        self.assertIn(synth["outfit"], ["outdoor_nature", "night_stargaze"])

    def test_fresh_2026_internet_grounding(self):
        """Memastikan penelusuran selalu tertambat pada data terkini 2026, bukan memori masa lalu."""
        now = datetime(2026, 11, 10, 10, 0, tzinfo=timezone.utc)
        topic = "Andrej Karpathy Eureka Labs & Deep Dive LLM Architecture"
        findings = "Membahas post-training reasoning RL dan kurikulum AI native"

        discovered = InterestDiscoveryEngine.discover(
            topic=topic,
            web_findings=findings,
            recency="today",
            now=now
        )
        self.assertEqual(discovered["interest_category"], "ai_engineering")
        self.assertIn("10 November 2026", discovered["scene"])
        self.assertIn("Eureka Labs", discovered["scene"])
        self.assertIn("post-training reasoning RL", discovered["scene"])
        self.assertNotIn("2024", discovered["scene"])
        self.assertNotIn("micrograd", discovered["scene"].lower())


if __name__ == "__main__":
    unittest.main()
