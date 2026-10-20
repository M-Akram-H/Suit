#!/usr/bin/env python3
"""Renders a Suit deck small with Manim and tiles frames of every slide into one picture,
so that a deck can be looked at before it is handed over.

    python3 preview.py DECK.suit.json [--out PICTURE.png] [--samples 3] [--height 270] [--keep]

Each row is a slide; its pictures are taken evenly through the slide, the last one being
how the slide ends. Rendering is what manim-slides does for a real talk, only smaller:
formulas are typeset by LaTeX, so a formula LaTeX cannot read fails here too, with
LaTeX's message.

Needs `suit-cli` and a Python with Manim and manim-slides (the one Suit uses is found
by itself, as suitdeck.py finds it). No screen is needed.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))


def _have_imaging():
    try:
        import av  # noqa: F401
        import PIL  # noqa: F401
    except ImportError:
        return False
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("deck")
    parser.add_argument("--out", help="where the picture goes (default: next to the deck, in a .preview.png)")
    parser.add_argument("--samples", type=int, default=3, help="pictures per slide")
    parser.add_argument("--height", type=int, default=270, help="pixel height to render at")
    parser.add_argument("--keep", action="store_true", help="keep the rendering folder")
    args = parser.parse_args()

    from suitdeck import find_cli, find_python

    # The pictures are cut out of the videos with PyAV and PIL, which Manim's Python has.
    if not _have_imaging():
        python = find_python()
        if python is None:
            sys.exit("preview.py: no Python with Manim was found. Set SUIT_PYTHON, or run suit-setup.")
        os.execv(python, [python, str(Path(__file__).resolve()), *sys.argv[1:]])

    cli = find_cli()
    if cli is None:
        sys.exit("preview.py: suit-cli was not found. Set SUIT_CLI, or install Suit.")
    deck = Path(args.deck).resolve()
    out = Path(args.out).resolve() if args.out else deck.with_name(deck.name.replace(".suit.json", ".preview.png"))
    work = Path(tempfile.mkdtemp(prefix="suit-preview-"))
    try:
        script = work / "deck.py"
        made = subprocess.run(
            [cli, "export", str(deck), "--height", str(args.height), "-o", str(script)],
            capture_output=True, text=True,
        )
        if made.returncode != 0:
            print((made.stdout + made.stderr).strip())
            sys.exit("preview.py: the deck has errors, so it was not rendered.")
        # Pictures are named from the deck's own folder, not from this one.
        text = script.read_text()
        text = re.sub(r"^config\.assets_dir = .*$", f"config.assets_dir = {str(deck.parent)!r}", text, flags=re.M)
        script.write_text(text)

        began = time.time()
        render = subprocess.run(
            [sys.executable, "-m", "manim_slides", "render", "deck.py", "Deck"],
            cwd=work, capture_output=True, text=True,
        )
        if render.returncode != 0 or not (work / "slides" / "Deck.json").is_file():
            lines = (render.stdout + render.stderr).strip().splitlines()
            print("\n".join(lines[-40:]))
            sys.exit("preview.py: Manim could not render the deck; what it said is above.")
        print(f"rendered in {time.time() - began:.0f} s")
        sheet(work / "slides" / "Deck.json", deck, out, max(1, args.samples))
    finally:
        if args.keep:
            print("kept", work)
        else:
            shutil.rmtree(work, ignore_errors=True)


def sheet(slides_json, deck, out, samples):
    import av
    from PIL import Image, ImageDraw

    titles = [slide["title"] for slide in json.loads(deck.read_text())["slides"]]
    rendered = json.loads(slides_json.read_text())["slides"]
    root = slides_json.parent.parent
    rows = []
    for slide in rendered:
        with av.open(str(root / slide["file"])) as video:
            frames = [frame.to_image() for frame in video.decode(video=0)]
        if not frames:
            continue
        count = min(samples, len(frames))
        picks = [round((i + 1) * (len(frames) - 1) / count) for i in range(count)]
        rows.append([frames[i] for i in picks])
    width, height = rows[0][0].size
    label = 18
    picture = Image.new("RGB", (width * samples, (height + label) * len(rows)), "#202020")
    draw = ImageDraw.Draw(picture)
    for y, row in enumerate(rows):
        top = y * (height + label)
        title = titles[y] if y < len(titles) else ""
        draw.text((6, top + 3), f"slide {y + 1}: {title}", fill="#ffffff")
        for x, frame in enumerate(row):
            picture.paste(frame, (x * width, top + label))
            draw.rectangle([x * width, top + label, (x + 1) * width - 1, top + label + height - 1], outline="#555555")
    picture.save(out)
    print(f"wrote {out}  ({len(rows)} slides, {samples} pictures each, the last one as the slide ends)")


if __name__ == "__main__":
    main()
