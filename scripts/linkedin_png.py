#!/usr/bin/env python3
"""Render docs/linkedin.png (1080×1080) to match docs/linkedin.svg."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "linkedin.png"

W = H = 1080
CREAM = "#fff9ef"
NAVY = "#1d3550"
TAUPE = "#6a5c4e"


def font(paths: list[str], size: int) -> ImageFont.FreeTypeFont:
    for p in paths:
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            continue
    return ImageFont.load_default()


SERIF = [
    "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
]
SERIF_BOLD = [
    "/usr/share/fonts/opentype/noto/NotoSerifCJK-Bold.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Bold.ttf",
]
SANS = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
]
SANS_MED = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
]


def center(draw: ImageDraw.ImageDraw, xy: tuple[float, float], text: str, f: ImageFont.ImageFont, fill: str) -> None:
    x, y = xy
    draw.text((x, y), text, font=f, fill=fill, anchor="mm")


def main() -> None:
    img = Image.new("RGB", (W, H), CREAM)
    d = ImageDraw.Draw(img)

    kicker = font(SANS_MED, 20)
    title = font(SERIF_BOLD, 72)
    tag = font(SERIF, 30)
    head = font(SANS_MED, 26)
    cell = font(SERIF, 36)
    cell_b = font(SERIF_BOLD, 36)
    foot = font(SERIF, 20)

    center(d, (540, 78), "DEVOPS ESSENTIAL", kicker, TAUPE)
    center(d, (540, 156), "auto-healer loop", title, NAVY)
    center(d, (540, 218), "Let auto-healer do your work.", tag, TAUPE)

    d.rectangle((90, 280, 990, 910), outline=NAVY, width=2)
    d.rectangle((90, 280, 540, 385), fill=NAVY)
    d.rectangle((540, 280, 990, 385), fill=NAVY)
    d.line((540, 280, 540, 910), fill=CREAM, width=3)
    d.line((540, 385, 540, 910), fill=NAVY, width=2)

    center(d, (315, 332.5), "agent loop", head, CREAM)
    center(d, (765, 332.5), "auto-healer loop", head, CREAM)

    d.line((90, 385, 990, 385), fill=NAVY, width=2)
    center(d, (315, 472.5), "Call LLM", cell, TAUPE)
    center(d, (765, 472.5), "Call Prometheus", cell_b, NAVY)

    d.line((90, 560, 990, 560), fill=NAVY, width=2)
    center(d, (315, 647.5), "Run tools", cell, TAUPE)
    center(d, (765, 647.5), "Run healing actions", cell_b, NAVY)

    d.line((90, 735, 990, 735), fill=NAVY, width=2)
    center(d, (315, 822.5), "Loop", cell, TAUPE)
    center(d, (765, 822.5), "Cron jobs", cell_b, NAVY)

    center(d, (540, 990), "© 수영 책방 Swimming Bookstore", foot, TAUPE)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    img.save(OUT, "PNG")
    print(OUT)


if __name__ == "__main__":
    main()
