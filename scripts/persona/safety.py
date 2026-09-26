"""
scripts/persona/safety.py
-------------------------
StatusSafetyGuard: Single Responsibility module to ensure NO error messages,
technical dumps, stack traces, or corrupted media are ever posted to WhatsApp Status Stories.
"""

import os
import re
import json
from dataclasses import dataclass
from typing import Tuple, Optional, Any, Dict

# Pola deteksi prefix meta/preamble yang sering bocor dari model LLM/VLM
# (Contoh: "status whatsapp story:", "Caption Status WA:", "Berikut caption story:", dsb.)
META_LEAK_PATTERNS = [
    # Status / story / caption labels di awal baris
    re.compile(r"^\s*(?:#+\s*)?(?:\*\*)?(?:draf\s+)?(?:teks\s+)?(?:caption\s+)?(?:status\s+)?(?:whatsapp|wa)?\s*(?:story)?(?:\s*[-–—]\s*[a-zA-Z0-9_]+)?(?:\*\*)?\s*[:\-–—\n]+\s*", re.IGNORECASE),
    # Kalimat pengantar seperti "Berikut adalah status whatsapp story:", "Berikut caption status WhatsApp:"
    re.compile(r"^\s*(?:berikut\s+(?:adalah\s+)?(?:draf\s+)?(?:teks\s+)?(?:status|caption|story)[^:\n]*[:\-–—\n]+)\s*", re.IGNORECASE),
    # "Caption:", "Teks Caption:", "Status:"
    re.compile(r"^\s*(?:#+\s*)?(?:\*\*)?(?:teks\s+)?caption\s*(?:\*\*)?\s*[:\-–—\n]+\s*", re.IGNORECASE),
    re.compile(r"^\s*(?:#+\s*)?(?:\*\*)?status\s*(?:\*\*)?\s*[:\-–—\n]+\s*", re.IGNORECASE),
    # Markdown header seperti "### Status Story" atau "## Caption"
    re.compile(r"^\s*#+\s+[^\n]+\n+\s*", re.IGNORECASE),
]

# Pola deteksi pesan error teknis, dump sistem, atau laporan kegagalan
ERROR_REGEX_PATTERNS = [
    re.compile(r"\b[A-Za-z0-9_]*(error|exception|traceback|stderr|failed|failure)\b", re.IGNORECASE),
    re.compile(r"\b(invalid\s+syntax|segfault|segmentation\s+fault|fatal\s+error|panic\s+error|assertion\s*error)\b", re.IGNORECASE),
    re.compile(r"\b(fatal|panic|critical)\s*:", re.IGNORECASE),
    re.compile(r"\b(unhandled\s+rejection|unhandled\s+promise)\b", re.IGNORECASE),
    re.compile(r"\b(500|502|503|504|400|401|403|404)\s+(internal server error|bad gateway|service unavailable|gateway timeout|unauthorized|forbidden|not found)\b", re.IGNORECASE),
    re.compile(r"\b(quota\s+exceeded|rate\s+limit|resource\s+exhausted|token\s+expired)\b", re.IGNORECASE),
    re.compile(r"\b(connection\s+refused|connection\s+timed?\s*out|network\s+unreachable|broken\s+pipe)\b", re.IGNORECASE),
    re.compile(r"\b(curl:\s*\(\d+\)|exit\s+code\s*[:=]\s*\d+|errno\s+\d+)\b", re.IGNORECASE),
    re.compile(r"File\s+[\"'].+?[\"'],\s+line\s+\d+", re.IGNORECASE),  # Python traceback
    re.compile(r"^\s*[{[].*?[\"']error[\"']\s*:\s*(true|[\"'].+?[\"'])", re.IGNORECASE | re.DOTALL),  # JSON error blob
    re.compile(r"<!DOCTYPE\s+html", re.IGNORECASE),  # HTML error page dump
    re.compile(r"<\s*html[^>]*>", re.IGNORECASE),
    re.compile(r"^(bash|sh|zsh):\s*command\s+not\s+found", re.IGNORECASE),
    re.compile(r"⚠️\s*(error|gagal|kesalahan|terjadi\s+kesalahan)", re.IGNORECASE),
    re.compile(r"\b(task\s+id\s+[\"'].+?[\"']\s+finished\s+with\s+result)\b", re.IGNORECASE),
    re.compile(r"<\s*SYSTEM_MESSAGE\s*>", re.IGNORECASE),
    re.compile(r"\b(null|undefined|None|NaN)\b"),
]

# Magic bytes untuk verifikasi integritas format media gambar & video
MAGIC_BYTES = {
    "png": b"\x89PNG\r\n\x1a\n",
    "jpeg": b"\xff\xd8\xff",
    "webp_prefix": b"RIFF",
    "webp_suffix": b"WEBP",
    "gif87": b"GIF87a",
    "gif89": b"GIF89a",
    "mp4_ftyp": b"ftyp",
}

DEFAULT_SAFE_IMPACT_MAXXING_CAPTION = (
    "Rehat sejenak dan syukuri setiap momen hari ini. "
    "Semangat untuk hal-hal baik yang sedang kita usahakan yaa ✨"
)


@dataclass(frozen=True)
class WhatsAppStatusCaptionPayload:
    """
    Strict Type Schema untuk representasi data Caption Status WhatsApp.
    Mencegah type confusion, shell injection, dan field hallucination.
    """
    caption: str

    @classmethod
    def from_dict(cls, data: dict) -> "WhatsAppStatusCaptionPayload":
        if not isinstance(data, dict):
            raise TypeError(f"Payload harus berupa dictionary / JSON Object, bukan {type(data).__name__}")
        val = data.get("caption")
        if val is None or not isinstance(val, str) or not val.strip():
            for k in ["text", "status", "message", "content"]:
                candidate = data.get(k)
                if isinstance(candidate, str) and candidate.strip():
                    val = candidate
                    break
        if not val or not isinstance(val, str) or not val.strip():
            raise ValueError("Field 'caption' kosong atau bukan string.")
        
        clean = StatusSafetyGuard.sanitize_caption(val)
        return cls(caption=clean)


class StatusSafetyGuard:
    """Penjaga gerbang keselamatan status WhatsApp story (Anti-Error & Anti-Corruption)."""

    @classmethod
    def validate_typed_caption(cls, raw: Any) -> Tuple[bool, str, str]:
        """
        Validasi berbasis tipe ketat (Strict Typed Validation).
        Mengembalikan tuple: (is_valid: bool, clean_caption: str, reason: str).
        """
        try:
            if isinstance(raw, str):
                cleaned = cls.extract_caption_from_raw(raw)
                is_safe, reason = cls.validate_status_text(cleaned)
                if not is_safe:
                    return False, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION, reason
                return True, cleaned, ""
            elif isinstance(raw, dict):
                val = raw.get("caption")
                if val is None or not isinstance(val, str) or not val.strip():
                    for k in ["text", "status", "message", "content"]:
                        candidate = raw.get(k)
                        if isinstance(candidate, str) and candidate.strip():
                            val = candidate
                            break
                if not val or not isinstance(val, str) or not val.strip():
                    return False, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION, "Field 'caption' kosong atau bukan string."

                cleaned = cls.extract_caption_from_raw(val)
                is_safe, reason = cls.validate_status_text(cleaned)
                if not is_safe:
                    return False, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION, reason
                return True, cleaned, ""
            else:
                return False, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION, f"Tipe input tidak legal: {type(raw).__name__}"
        except Exception as e:
            return False, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION, str(e)

    @staticmethod
    def validate_status_text(text: Optional[str]) -> Tuple[bool, str]:
        """
        Validasi teks status atau caption story.
        Mengembalikan (True, "") jika aman, atau (False, reason) jika terdeteksi error.
        """
        if not text or not str(text).strip():
            return False, "Teks atau caption kosong (empty/whitespace)."

        clean_text = str(text).strip()

        # Deteksi string teknis mencurigakan
        for pattern in ERROR_REGEX_PATTERNS:
            if pattern.search(clean_text):
                return False, f"Teks mengandung pola error/dump teknis (pattern: {pattern.pattern})."

        # Deteksi teks yang terlalu pendek atau hanya simbol teknis
        if len(clean_text) < 3:
            return False, "Teks terlalu pendek untuk sebuah status story."

        return True, ""

    @staticmethod
    def validate_status_media(file_path: Optional[str]) -> Tuple[bool, str]:
        """
        Validasi file media status (gambar/video).
        Memastikan berkas ada, ukuran valid (>1KB, <50MB), dan header bytes valid.
        """
        if not file_path:
            return False, "Jalur berkas media kosong."

        abs_path = os.path.abspath(file_path)
        if not os.path.isfile(abs_path):
            return False, f"Berkas media tidak ditemukan di disk: {abs_path}"

        try:
            file_size = os.path.getsize(abs_path)
        except OSError as e:
            return False, f"Gagal membaca ukuran berkas: {e}"

        # Minimal 1024 bytes (1 KB) agar mencegah berkas 0-byte atau log error berkedok .png
        if file_size < 1024:
            return False, f"Ukuran berkas terlalu kecil ({file_size} bytes), diduga berkas rusak atau kosong."

        # Maksimal 50 MB (batas wajar media story WhatsApp)
        if file_size > 50 * 1024 * 1024:
            return False, f"Ukuran berkas melebihi batas 50MB ({file_size / (1024*1024):.2f} MB)."

        # Validasi Magic Bytes
        try:
            with open(abs_path, "rb") as f:
                header = f.read(32)
        except Exception as e:
            return False, f"Gagal membaca header berkas media: {e}"

        is_valid = False
        media_type = "unknown"

        if header.startswith(MAGIC_BYTES["png"]):
            is_valid = True
            media_type = "image/png"
        elif header.startswith(MAGIC_BYTES["jpeg"]):
            is_valid = True
            media_type = "image/jpeg"
        elif header.startswith(MAGIC_BYTES["webp_prefix"]) and len(header) >= 12 and header[8:12] == MAGIC_BYTES["webp_suffix"]:
            is_valid = True
            media_type = "image/webp"
        elif header.startswith(MAGIC_BYTES["gif87"]) or header.startswith(MAGIC_BYTES["gif89"]):
            is_valid = True
            media_type = "image/gif"
        elif len(header) >= 8 and header[4:8] == MAGIC_BYTES["mp4_ftyp"]:
            is_valid = True
            media_type = "video/mp4"

        if not is_valid:
            return False, "Format berkas tidak valid atau tidak dikenali (bukan berkas PNG, JPEG, WebP, atau MP4 valid)."

        return True, media_type

    @classmethod
    def extract_caption_from_raw(cls, raw: Optional[str]) -> str:
        """
        Mengekstrak teks caption murni dari output mentah yang mungkin berupa:
        1. Strict JSON ({"caption": "..."})
        2. Markdown code block (```json\n{"caption": "..."}\n``` atau ```{"caption": "..."}```)
        3. Teks yang diawali label meta (contoh: "status whatsapp story: ...", "Berikut caption:")
        4. Teks biasa bertanda kutip pembungkus.
        """
        if not raw or not str(raw).strip():
            return ""

        text = str(raw).strip()

        # 1. Periksa apakah teks dibungkus markdown code fence (misal ```json ... ``` atau ``` ... ```)
        fence_match = re.search(r"```(?:json)?\s*([\s\S]*?)\s*```", text, re.IGNORECASE)
        if fence_match:
            candidate_json = fence_match.group(1).strip()
            try:
                parsed = json.loads(candidate_json)
                if isinstance(parsed, dict):
                    for k in ["caption", "text", "status", "content", "message"]:
                        if k in parsed and isinstance(parsed[k], str) and parsed[k].strip():
                            text = parsed[k].strip()
                            break
                elif isinstance(parsed, str) and parsed.strip():
                    text = parsed.strip()
            except Exception:
                text = candidate_json

        # 2. Coba parse teks langsung sebagai JSON
        if text.startswith("{") or '"caption"' in text.lower():
            try:
                parsed = json.loads(text)
                if isinstance(parsed, dict):
                    for k in ["caption", "text", "status", "content", "message"]:
                        if k in parsed and isinstance(parsed[k], str) and parsed[k].strip():
                            text = parsed[k].strip()
                            break
            except Exception:
                # Jika json.loads gagal (misal ada teks pengantar sebelum {), cari blok JSON dengan regex
                json_blob_match = re.search(r'\{[\s\S]*?["\']caption["\']\s*:\s*["\']((?:[^"\'\\]|\\.)*)["\'][\s\S]*?\}', text, re.IGNORECASE)
                if json_blob_match:
                    try:
                        escaped_str = '"' + json_blob_match.group(1) + '"'
                        text = json.loads(escaped_str)
                    except Exception:
                        text = json_blob_match.group(1)

        # 3. Bersihkan prefix meta / header bocor secara berulang hingga tuntas
        changed = True
        iterations = 0
        while changed and iterations < 5:
            changed = False
            iterations += 1
            for pat in META_LEAK_PATTERNS:
                m = pat.match(text)
                if m:
                    text = text[m.end():].strip()
                    changed = True

        # 4. Bersihkan tanda kutip pembungkus atau format markdown bold/italic di awal & akhir
        for _ in range(3):
            if (text.startswith('"') and text.endswith('"')) or \
               (text.startswith("'") and text.endswith("'")) or \
               (text.startswith("“") and text.endswith("”")) or \
               (text.startswith("«") and text.endswith("»")):
                text = text[1:-1].strip()
            elif text.startswith("**") and text.endswith("**") and len(text) > 4:
                text = text[2:-2].strip()

        return text.strip()

    @classmethod
    def sanitize_caption(cls, caption: Optional[str], fallback: Optional[str] = None) -> str:
        """
        Membersihkan caption. Bila caption terdeteksi bermasalah atau mengandung error,
        kembalikan fallback aman bertaraf Impact Maxxing. DILARANG mengembalikan pesan error.
        Mengekstrak teks murni jika output berupa Strict JSON atau mengandung label meta.
        """
        cleaned = cls.extract_caption_from_raw(caption)
        is_safe, _ = cls.validate_status_text(cleaned)
        if is_safe and cleaned:
            return cleaned.strip()

        # Gunakan fallback yang valid atau default Impact Maxxing
        safe_fallback = cls.extract_caption_from_raw(fallback) if fallback else ""
        if safe_fallback and cls.validate_status_text(safe_fallback)[0]:
            return safe_fallback.strip()

        return DEFAULT_SAFE_IMPACT_MAXXING_CAPTION
