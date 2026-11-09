#!/usr/bin/env python3
"""Prepare the bundled IBM Plex fonts.

BloxCode ships IBM Plex Sans (Regular/Medium/SemiBold) and IBM Plex Mono
(Regular/Medium) as static TrueType files. The sans faces are instantiated from
the upstream variable font so every weight is a real static face, which the
egui/ab_glyph text stack can render directly.

Run from the repository root:

    python3 tools/prepare_fonts.py
"""

from __future__ import annotations

import pathlib
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "bloxcode-app" / "assets" / "fonts"
SANS_VAR = "https://raw.githubusercontent.com/google/fonts/main/ofl/ibmplexsans/IBMPlexSans%5Bwdth,wght%5D.ttf"
MONO_BASE = "https://raw.githubusercontent.com/google/fonts/main/ofl/ibmplexmono"
MONO_FILES = {
    "IBMPlexMono-Regular.ttf": "PlexMono-Regular.ttf",
    "IBMPlexMono-Medium.ttf": "PlexMono-Medium.ttf",
}
SANS_WEIGHTS = {
    400: ("PlexSans-Regular.ttf", "Regular"),
    500: ("PlexSans-Medium.ttf", "Medium"),
    600: ("PlexSans-SemiBold.ttf", "SemiBold"),
}


def download(url: str, target: pathlib.Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=60) as response:
        target.write_bytes(response.read())


def instantiate(source: pathlib.Path) -> None:
    from fontTools.ttLib import TTFont
    from fontTools.varLib.instancer import instantiateVariableFont

    for weight, (filename, label) in SANS_WEIGHTS.items():
        font = TTFont(source)
        instance = instantiateVariableFont(font, {"wght": weight, "wdth": 100}, inplace=False)
        instance["OS/2"].usWeightClass = weight
        family = "IBM Plex Sans"
        full = f"{family} {label}"
        postscript = f"IBMPlexSans-{label}"
        name = instance["name"]
        for name_id, value in (
            (1, family),
            (2, label),
            (4, full),
            (6, postscript),
            (16, family),
            (17, label),
        ):
            name.setName(value, name_id, 3, 1, 0x409)
        instance.save(OUT / filename)
        print(f"wrote {filename} (weight {weight})")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    sans_source = OUT / "_source-sans-variable.ttf"
    download(SANS_VAR, sans_source)
    instantiate(sans_source)
    sans_source.unlink()

    for remote, local in MONO_FILES.items():
        download(f"{MONO_BASE}/{remote}", OUT / local)
        print(f"wrote {local}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
