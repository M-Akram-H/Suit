"""Run by Suit to render a deck for a website: manim-slides' render, saying how far it has
got, and then every slide's videos made ready to stream from a web server.

usage: python render.py DECK.py STAGING STOP

DECK.py is the deck exported for manim-slides, with a class called Deck. The videos and a
`manifest.json` describing them go to STAGING/videos and STAGING. Progress goes to the
standard output, one line each, starting with \\x1e:

    at SECONDS      the scene has been rendered up to this moment
    reverse N       the rewind video of the Nth slide is being made
    stream N        the Nth slide is being made ready to stream
    done

The render stops between two animations, so that no half-written video is left in
Manim's cache, once the file STOP exists or Suit, which started it, has gone. (Not a line
on the standard input: a thread waiting on it holds a lock that the processes manim-slides
starts to make the rewind videos then wait for for ever.)
"""

import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import sys

REPLY = "\x1e"


class Stopped(Exception):
    """Suit asked for the render to stop."""


def main():
    deck_file = pathlib.Path(sys.argv[1]).resolve()
    staging = pathlib.Path(sys.argv[2]).resolve()
    stop = pathlib.Path(sys.argv[3]).resolve()
    suit = os.getppid()

    # Whatever Manim prints goes where errors go; the standard output is for Suit.
    replies = sys.stdout
    sys.stdout = sys.stderr

    def say(*words):
        replies.write(REPLY + " ".join(str(word) for word in words) + "\n")
        replies.flush()

    def check():
        if stop.exists() or os.getppid() != suit:
            raise Stopped

    import av
    from manim import config
    import manim_slides.slide.base as base

    spec = importlib.util.spec_from_file_location("deck", deck_file)
    module = importlib.util.module_from_spec(spec)
    # The deck sets its own size, frame rate and background as it is read.
    spec.loader.exec_module(module)
    config.progress_bar = "none"
    config.verbosity = "WARNING"

    deck = module.Deck
    play = deck.play

    def watched(self, *args, **kwargs):
        # Between two animations, never in the middle of one.
        check()
        play(self, *args, **kwargs)
        say("at", f"{self.renderer.time:.6f}")

    deck.play = watched

    reverse = base.reverse_video_file
    reversed_count = 0

    def reversing(*args, **kwargs):
        nonlocal reversed_count
        check()
        reversed_count += 1
        say("reverse", reversed_count)
        reverse(*args, **kwargs)

    base.reverse_video_file = reversing

    scene = deck()
    scene.render()

    presentation = json.loads((pathlib.Path("slides") / "Deck.json").read_text())
    videos = staging / "videos"
    videos.mkdir(parents=True, exist_ok=True)

    def named(number, source, ending):
        """A name that changes with what is in the file, so that browsers and web servers
        may keep a copy for good: the next export of a changed slide has another name."""
        digest = hashlib.sha256(source.read_bytes()).hexdigest()[:10]
        return f"{number:02d}-{digest}{ending}{source.suffix.lower()}"

    def stream_ready(source, target):
        """Copies the video with its index at the front, so that a browser can start
        playing before it has the whole file."""
        with av.open(str(source)) as given:
            with av.open(str(target), "w", options={"movflags": "faststart"}) as made:
                inner = given.streams.video[0]
                outer = made.add_stream_from_template(inner)
                for packet in given.demux(inner):
                    if packet.dts is None:
                        continue
                    packet.stream = outer
                    made.mux(packet)

    def place(number, source, ending):
        target = videos / named(number, source, ending)
        if source.suffix.lower() == ".mp4":
            stream_ready(source, target)
        else:
            # A slide that shows a file of the author's: it goes as it is.
            shutil.copyfile(source, target)
        return f"videos/{target.name}"

    slides = []
    for number, slide in enumerate(presentation["slides"], start=1):
        check()
        say("stream", number)
        # Named from the folder the render ran in.
        forwards = pathlib.Path(slide["file"]).resolve()
        backwards = pathlib.Path(slide["rev_file"]).resolve()
        video = place(number, forwards, "")
        back = video if backwards == forwards else place(number, backwards, "-back")
        is_video = forwards.suffix.lower() in (".mp4", ".mov", ".webm", ".m4v")
        slides.append(
            {
                "kind": "video" if is_video else "image",
                "video": video,
                "back": back,
                "loop": slide.get("loop", False),
                "auto_next": slide.get("auto_next", False),
                "playback_rate": slide.get("playback_rate", 1.0),
                "reversed_playback_rate": slide.get("reversed_playback_rate", 1.0),
                "notes": slide.get("notes", ""),
            }
        )

    manifest = {
        "resolution": presentation.get("resolution"),
        "background": presentation.get("background_color"),
        "slides": slides,
    }
    (staging / "manifest.json").write_text(json.dumps(manifest, indent=1))
    say("done")


if __name__ == "__main__":
    try:
        main()
    except Stopped:
        print("stopped", file=sys.stderr)
        sys.exit(3)
