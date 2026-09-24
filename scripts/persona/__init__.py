"""
scripts/persona package
-----------------------
Modular persona status components:
- StatusSafetyGuard: Zero-error validation and media verification
- AtmosphereEngine: Makoto Shinkai contextual aesthetics without forced clouds
- WardrobeManager: Wardrobe presets, boredom tracking, and weekly guardrails
- ImageCaptionEngine: Post-image multimodal captioning aligned with generated image
"""

from .safety import StatusSafetyGuard, DEFAULT_SAFE_IMPACT_MAXXING_CAPTION
from .atmosphere import AtmosphereEngine
from .wardrobe import WardrobeManager, WARDROBE_PRESETS
from .captioner import ImageCaptionEngine

__all__ = [
    "StatusSafetyGuard",
    "DEFAULT_SAFE_IMPACT_MAXXING_CAPTION",
    "AtmosphereEngine",
    "WardrobeManager",
    "WARDROBE_PRESETS",
    "ImageCaptionEngine",
]
