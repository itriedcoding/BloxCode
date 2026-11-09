#!/usr/bin/env python3
"""Generate the BloxCode mark: a white isometric cube on a near-black tile.

Outputs:
  crates/bloxcode-app/assets/icon.ico       Windows executable icon (multi-size)
  crates/bloxcode-app/assets/icon-64.rgba   raw RGBA for the window icon

Pure standard library. Re-run from the repository root:

    python3 tools/make_icon.py
"""

from __future__ import annotations

import math
import pathlib
import struct

ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = ROOT / "crates" / "bloxcode-app" / "assets"
SIZES = [16, 24, 32, 48, 64, 128, 256]
SUPERSAMPLE = 4

TILE_BG = (10, 10, 10)
MARK = (255, 255, 255)

CUBE = {
    "top": (0.500, 0.205),
    "upper_left": (0.238, 0.348),
    "upper_right": (0.762, 0.348),
    "mid_left": (0.238, 0.652),
    "mid_right": (0.762, 0.652),
    "bottom": (0.500, 0.795),
    "center": (0.500, 0.500),
}
EDGES = [
    ("top", "upper_right"),
    ("upper_right", "mid_right"),
    ("mid_right", "bottom"),
    ("bottom", "mid_left"),
    ("mid_left", "upper_left"),
    ("upper_left", "top"),
    ("center", "top"),
    ("center", "mid_left"),
    ("center", "mid_right"),
]


def inside_rounded_tile(x: float, y: float, inset: float = 0.045, radius: float = 0.175) -> bool:
    x0, y0, x1, y1 = inset, inset, 1.0 - inset, 1.0 - inset
    if x < x0 or x > x1 or y < y0 or y > y1:
        return False
    nearest_x = min(max(x, x0 + radius), x1 - radius)
    nearest_y = min(max(y, y0 + radius), y1 - radius)
    return math.hypot(x - nearest_x, y - nearest_y) <= radius


def segment_distance(
    px: float, py: float, ax: float, ay: float, bx: float, by: float
) -> float:
    dx, dy = bx - ax, by - ay
    length_sq = dx * dx + dy * dy
    if length_sq == 0.0:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / length_sq))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def render(size: int) -> bytes:
    half_width = max(0.024, 0.9 / size)
    samples = SUPERSAMPLE * SUPERSAMPLE
    pixels = bytearray()
    for y in range(size):
        for x in range(size):
            tile_hits = 0
            mark_hits = 0
            for sy in range(SUPERSAMPLE):
                for sx in range(SUPERSAMPLE):
                    px = (x + (sx + 0.5) / SUPERSAMPLE) / size
                    py = (y + (sy + 0.5) / SUPERSAMPLE) / size
                    if not inside_rounded_tile(px, py):
                        continue
                    tile_hits += 1
                    hit = False
                    for start, end in EDGES:
                        ax, ay = CUBE[start]
                        bx, by = CUBE[end]
                        if segment_distance(px, py, ax, ay, bx, by) <= half_width:
                            hit = True
                            break
                    if hit:
                        mark_hits += 1
            if tile_hits == 0:
                pixels.extend((0, 0, 0, 0))
                continue
            tile_cov = tile_hits / samples
            mark_cov = mark_hits / samples
            alpha = round(255 * tile_cov)
            blend = mark_cov / max(tile_cov, 1e-6)
            blend = max(0.0, min(1.0, blend))
            colour = tuple(
                round(TILE_BG[channel] + (MARK[channel] - TILE_BG[channel]) * blend)
                for channel in range(3)
            )
            pixels.extend((colour[0], colour[1], colour[2], alpha))
    return bytes(pixels)


def dib_entry(size: int, rgba: bytes) -> bytes:
    header = struct.pack(
        "<IiiHHIIiiII",
        40,          # biSize
        size,        # biWidth
        size * 2,    # biHeight (image + mask)
        1,           # biPlanes
        32,          # biBitCount
        0,           # biCompression
        size * size * 4,
        2835,
        2835,
        0,
        0,
    )
    body = bytearray()
    for row in range(size - 1, -1, -1):
        offset = row * size * 4
        for column in range(0, size * 4, 4):
            r, g, b, a = rgba[offset + column : offset + column + 4]
            body.extend((b, g, r, a))
    mask_row_bytes = ((size + 31) // 32) * 4
    mask = bytearray()
    for row in range(size - 1, -1, -1):
        bits = bytearray(mask_row_bytes)
        for column in range(size):
            alpha = rgba[(row * size + column) * 4 + 3]
            if alpha < 128:
                bits[column // 8] |= 0x80 >> (column % 8)
        mask.extend(bits)
    return header + bytes(body) + bytes(mask)


def write_ico(path: pathlib.Path) -> None:
    images = [(size, dib_entry(size, render(size))) for size in SIZES]
    directory = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = bytearray()
    payload = bytearray()
    for size, data in images:
        width = 0 if size >= 256 else size
        entries.extend(
            struct.pack(
                "<BBBBHHII",
                width,
                width,
                0,
                0,
                1,
                32,
                len(data),
                offset,
            )
        )
        payload.extend(data)
        offset += len(data)
    path.write_bytes(directory + bytes(entries) + bytes(payload))


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    write_ico(ASSETS / "icon.ico")
    (ASSETS / "icon-64.rgba").write_bytes(render(64))
    print(f"wrote {ASSETS / 'icon.ico'}")
    print(f"wrote {ASSETS / 'icon-64.rgba'}")


if __name__ == "__main__":
    main()
