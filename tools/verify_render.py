"""Compare a manim-slides render with the frame counts the checker predicted (spec §6.7, P3).

usage: verify_render.py SLIDES_JSON FRAMES [FRAMES ...]
"""
import json
import pathlib
import sys

import av


def frames_in(path):
    with av.open(str(path)) as container:
        return sum(1 for _ in container.decode(video=0))


def main():
    slides_json = pathlib.Path(sys.argv[1])
    expected = [int(n) for n in sys.argv[2:]]
    presentation = json.loads(slides_json.read_text())
    slides = presentation["slides"]
    root = slides_json.parent.parent
    ok = len(slides) == len(expected)
    if not ok:
        print(f"slide count: rendered {len(slides)}, predicted {len(expected)}")
    for index, (slide, want) in enumerate(zip(slides, expected), start=1):
        got = frames_in(root / slide["file"])
        reversed_frames = frames_in(root / slide["rev_file"])
        verdict = "ok" if got == want == reversed_frames else "MISMATCH"
        ok &= verdict == "ok"
        flags = " loop" if slide["loop"] else ""
        print(f"slide {index}: rendered {got}, reversed {reversed_frames}, predicted {want}  {verdict}{flags}")
    print("resolution", presentation["resolution"], "background", presentation["background_color"])
    sys.exit(0 if ok else 1)


main()
