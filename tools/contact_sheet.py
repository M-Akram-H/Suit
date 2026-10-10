"""Tile sampled frames of every slide of a manim-slides render into one image.

usage: contact_sheet.py SLIDES_JSON OUTPUT_PNG [SAMPLES_PER_SLIDE]
"""
import json
import pathlib
import sys

import av
from PIL import Image, ImageDraw


def main():
    slides_json = pathlib.Path(sys.argv[1])
    samples = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    root = slides_json.parent.parent
    rows = []
    for slide in json.loads(slides_json.read_text())["slides"]:
        with av.open(str(root / slide["file"])) as container:
            frames = [frame.to_image() for frame in container.decode(video=0)]
        picks = [round(i * (len(frames) - 1) / (samples - 1)) for i in range(samples)]
        rows.append([(index, frames[index]) for index in picks])
    width, height = rows[0][0][1].size
    sheet = Image.new("RGB", (width * samples, height * len(rows)), "white")
    draw = ImageDraw.Draw(sheet)
    for y, row in enumerate(rows):
        for x, (index, frame) in enumerate(row):
            sheet.paste(frame, (x * width, y * height))
            draw.rectangle([x * width, y * height, (x + 1) * width - 1, (y + 1) * height - 1], outline="#444444")
            draw.text((x * width + 6, y * height + 4), f"slide {y + 1}  frame {index}", fill="#ffff66")
    sheet.save(sys.argv[2])
    print("wrote", sys.argv[2], sheet.size)


main()
