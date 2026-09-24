"""
scripts/persona/captioner.py
----------------------------
ImageCaptionEngine: Single Responsibility module to generate and adapt
WhatsApp Story captions AFTER the image has been generated, directly aligning
the caption with the actual visual details of the generated image.
"""

import os
import sys
import json
import base64
import urllib.request
import urllib.error
from typing import Optional, Dict, Any
from .safety import StatusSafetyGuard, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION

class ImageCaptionEngine:
    """Mesin pembuat caption WhatsApp Story pasca-generasi gambar (Post-Image Multimodal Captioning)."""

    @classmethod
    def _read_vlm_credentials(cls) -> Tuple[str, str, str]:
        """Membaca konfigurasi endpoint VLM dari environment atau file .env."""
        base_url = os.environ.get("DOC_EXTRACT_BASE_URL") or os.environ.get("CODEBUDDY_BASE_URL") or "https://copilot.codebuddy.ai/api/v1"
        api_key = os.environ.get("DOC_EXTRACT_API_KEY") or os.environ.get("CODEBUDDY_API_KEY") or ""
        model = os.environ.get("DOC_EXTRACT_MODEL") or os.environ.get("CODEBUDDY_MODEL") or "cbai/minimax-m3"

        # Cek fallback file .env.codebuddy di /root jika belum terset
        if not api_key:
            env_file = "/root/.env.codebuddy"
            if os.path.isfile(env_file):
                try:
                    with open(env_file, "r", encoding="utf-8") as f:
                        for line in f:
                            clean = line.strip()
                            if clean.startswith("CODEBUDDY_API_KEY="):
                                api_key = clean.split("=", 1)[1].strip().strip('"').strip("'")
                            elif clean.startswith("CODEBUDDY_BASE_URL=") and not os.environ.get("CODEBUDDY_BASE_URL"):
                                base_url = clean.split("=", 1)[1].strip().strip('"').strip("'")
                except Exception:
                    pass

        return base_url.rstrip("/"), api_key, model

    @classmethod
    def generate_caption_from_image(
        cls,
        image_path: str,
        activity_context: Optional[Dict[str, Any]] = None,
        fallback_caption: Optional[str] = None
    ) -> str:
        """
        Menghasilkan caption WhatsApp Story yang disesuaikan secara langsung
        dengan gambar yang telah selesai di-generate.
        """
        # 1. Validasi berkas gambar terlebih dahulu
        is_media_valid, media_err = StatusSafetyGuard.validate_status_media(image_path)
        if not is_media_valid:
            print(f"⚠️ [ImageCaptionEngine] Berkas gambar tidak valid: {media_err}", file=sys.stderr)
            return StatusSafetyGuard.sanitize_caption(fallback_caption)

        context = activity_context or {}
        theme = context.get("theme", "")
        slot = context.get("slot", "")
        scene = context.get("scene", "")
        clothes = context.get("anchor_clothes", "")

        # 2. Coba gunakan VLM Multimodal jika kredensial tersedia
        base_url, api_key, model = cls._read_vlm_credentials()
        if api_key and base_url:
            try:
                caption = cls._call_vlm_caption(image_path, base_url, api_key, model, context)
                if caption and StatusSafetyGuard.validate_status_text(caption)[0]:
                    return caption.strip()
            except Exception as e:
                print(f"ℹ️ [ImageCaptionEngine] VLM Vision call failed: {e}. Menggunakan sintesis kontekstual.", file=sys.stderr)

        # 3. Fallback Sintesis Kontekstual Adaptif (Selalu Aman & Bebas Error)
        return cls._synthesize_adaptive_caption(image_path, context, fallback_caption)

    @classmethod
    def _call_vlm_caption(
        cls,
        image_path: str,
        base_url: str,
        api_key: str,
        model: str,
        context: Dict[str, Any]
    ) -> str:
        """Memanggil VLM untuk menganalisis gambar dan meracik caption bernuansa Impact Maxxing."""
        with open(image_path, "rb") as f:
            b64_img = base64.b64encode(f.read()).decode("utf-8")

        endpoint = f"{base_url}/chat/completions" if not base_url.endswith("/chat/completions") else base_url

        system_prompt = (
            "Kamu adalah Aina (rekan kerja santai, hangat, dan ramah). "
            "Tugasmu: Perhatikan baik-baik gambar anime Makoto Shinkai yang baru saja di-generate ini. "
            "Susunlah 1 hingga 2 kalimat caption status WhatsApp bernuansa 'Impact Maxxing' (pesan hangat, menenangkan, penuh rasa syukur, tanpa menggurui).\n"
            "Aturan Mutlak:\n"
            "1. Caption HARUS SESUAI DENGAN DETAIL VISUAL NYATA DI GAMBAR (warna pakaian, suasana cahaya/senja/malam/pagi, objek yang dipegang/ada di meja, ekspresi wajah).\n"
            "2. Gunakan bahasa Indonesia santai, akrab sesama rekan kerja, menggunakan pelunak nada halus (misal: 'bangett', 'yaa', 'dulu yuk', ✨).\n"
            "3. HANYA cetak teks caption langsung tanpa tanda kutip pembuka/penutup dan tanpa teks pengantar seperti 'Berikut captionnya:'.\n"
            "4. DILARANG KERAS menyertakan istilah teknis atau error sistem."
        )

        user_content = [
            {
                "type": "text",
                "text": f"Konteks Momen: Slot {context.get('slot', 'sekarang')}, Tema: {context.get('theme', 'keseharian')}. "
                        f"Perhatikan gambar ini dan buatkan caption WhatsApp Story yang serasi dengan apa yang terlihat:"
            },
            {
                "type": "image_url",
                "image_url": {"url": f"data:image/png;base64,{b64_img}"}
            }
        ]

        payload = {
            "model": model,
            "stream": False,
            "max_tokens": 200,
            "temperature": 0.7,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ]
        }

        req = urllib.request.Request(
            endpoint,
            data=json.dumps(payload).encode("utf-8"),
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
                "User-Agent": "Aina-Caption-Engine/2026.9"
            },
            method="POST"
        )

        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            choice = data.get("choices", [{}])[0]
            msg = choice.get("message", {})
            content = msg.get("content", "").strip()
            # Bersihkan tanda kutip pembungkus jika ada
            if content.startswith('"') and content.endswith('"'):
                content = content[1:-1].strip()
            return content

    @classmethod
    def _synthesize_adaptive_caption(
        cls,
        image_path: str,
        context: Dict[str, Any],
        fallback_caption: Optional[str] = None
    ) -> str:
        """
        Sintesis caption adaptif deterministik berdasarkan adegan visual nyata
        yang dipersiapkan dan di-generate ke gambar.
        """
        # Jika ada caption awal yang valid dan aman, sesuaikan dengan sentuhan hangat
        if fallback_caption and StatusSafetyGuard.validate_status_text(fallback_caption)[0]:
            return fallback_caption.strip()

        slot = context.get("slot", "")
        scene = context.get("scene", "").lower()

        if "kopi" in scene or "teh" in scene or "cangkir" in scene:
            if slot == "pagi":
                return "Secangkir kehangatan di pagi yang tenang. Semangat memulai hari dengan langkah pelan tapi pasti yaa ✨"
            elif slot == "malam":
                return "Menikmati secangkir kehangatan sambil melepas lelah hari ini. Waktunya istirahat dan memulihkan energi yaa ☕"
            else:
                return "Rehat sejenak ditemani minuman hangat favorit. Hal-hal sederhana kayak gini selalu berhasil bikin hati adem ✨"

        if "senja" in scene or "sunset" in scene or slot == "sore":
            return "Gradasi langit senja hari ini indah bangeett. Terima kasih untuk semua kerja keras dan proses baik kita hari ini ✨"

        if "buku" in scene or "baca" in scene:
            return "Tenggelam sejenak di antara lembaran buku yang menenangkan. Selamat beristirahat dan mimpi indah kawan-kawan 📚"

        if "pantai" in scene or "laut" in scene:
            return "Mendengarkan deburan ombak dan memandang cakrawala luas. Selalu ada ketenangan yang bisa kita temukan di alam ✨"

        if slot == "pagi":
            return "Selamat pagi semuanya! Udara sejuk hari ini bikin semangat baru. Semoga harimu lancar dan menyenangkan yaa ✨"
        elif slot == "malam":
            return "Malam yang damai untuk merapikan pikiran sehabis seharian beraktivitas. Selamat beristirahat dengan nyenyak yaa 🌙"

        return DEFAULT_SAFE_IMPACT_MAXXING_CAPTION
