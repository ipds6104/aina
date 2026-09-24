"""
scripts/persona/safety.py
-------------------------
StatusSafetyGuard: Single Responsibility module to ensure NO error messages,
technical dumps, stack traces, or corrupted media are ever posted to WhatsApp Status Stories.
"""

import os
import re
from typing import Tuple, Optional

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


class StatusSafetyGuard:
    """Penjaga gerbang keselamatan status WhatsApp story (Anti-Error & Anti-Corruption)."""

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
    def sanitize_caption(cls, caption: Optional[str], fallback: Optional[str] = None) -> str:
        """
        Membersihkan caption. Bila caption terdeteksi bermasalah, kembalikan fallback aman
        bertaraf Impact Maxxing. DILARANG mengembalikan pesan error.
        """
        is_safe, reason = cls.validate_status_text(caption)
        if is_safe and caption:
            return caption.strip()

        # Gunakan fallback yang valid atau default Impact Maxxing
        safe_fallback = fallback.strip() if fallback and cls.validate_status_text(fallback)[0] else DEFAULT_SAFE_IMPACT_MAXXING_CAPTION
        return safe_fallback
