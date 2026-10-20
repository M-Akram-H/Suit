"""Keeps Manim imported and runs probe scripts on request.

Started with one argument: a directory of its own, for typeset text and for the pixels of
pictures. Scripts find it as `_scratch`.

One request per line on stdin: the script's path, a tab, the path for its result. One reply
per request on stdout: a line starting with the record separator. Everything a script prints
goes to stderr, so it cannot be mistaken for a reply.

A line that starts with the cancel character and names a script says that its result is no
longer wanted. The script is told so at its next check (`_wanted`, which the probes call
after every step and at every frame) and stops. Closing stdin cancels whatever is running
and ends the process.

Scripts run one at a time per working directory, also across processes, and each process
keeps its typeset text to itself. Both are because of how Manim caches: it rewrites a cached
text file every time it uses it (`PangoUtils.remove_last_M`), so two Manims making the same
text at the same moment read each other's half-written files, and one that is stopped in the
middle leaves a broken file behind for everyone after it.
"""
import gc
import os
import queue
import sys
import threading
import time
import traceback

import manim  # noqa: F401 - importing it is the point: it takes seconds, once
from manim.mobject.text import tex_mobject
from manim.utils import tex_file_writing

try:
    import fcntl
except ImportError:  # no such thing on Windows; there the directory is shared unguarded
    fcntl = None

CANCEL = "\x18"

scratch = sys.argv[1] if len(sys.argv) > 1 else "."
manim.config.text_dir = os.path.join(scratch, "texts")

replies = sys.stdout
sys.stdout = sys.stderr
turn = open("manim.lock", "w")  # noqa: SIM115 - held for as long as the process lives

# Python's collector looks through every object there is each time enough new ones have
# been made, and a probe makes hundreds of thousands, restoring a slide's state alone: left
# to itself it took half of every probe. So it collects while nothing is asked of it, and
# leaves out of its looking what Manim made on being imported, which stays for good.
gc.disable()
gc.freeze()
# Seconds without a request after which it collects.
IDLE = 0.25

requests = queue.Queue()
# What the probes keep from one script to the next: the scene, and its state before every
# slide, so that a slide nobody touched is not run again.
worlds = {}
# The script whose result is not wanted any more; `True` once nothing is.
unwanted = None
running = None


def listen():
    global unwanted
    for line in sys.stdin:
        line = line.rstrip("\n")
        if line.startswith(CANCEL):
            unwanted = line[len(CANCEL):]
        else:
            requests.put(line)
    unwanted = True
    requests.put(None)


def wanted():
    return unwanted is not True and unwanted != running


def typeset_whole(expression, environment=None, tex_template=None):
    """Manim's `tex_to_svg_file`, except that a formula whose typesetting was cut short is
    typeset again instead of being read back half-written for ever after."""
    tex = tex_file_writing.generate_tex_file(expression, environment, tex_template)
    svg = tex.with_suffix(".svg")
    # Named so that Manim's own tidying, which keeps `.tex` files, leaves it alone.
    busy = tex.with_name(tex.stem + ".busy.tex")
    if busy.exists():
        svg.unlink(missing_ok=True)
    if not svg.exists():
        busy.touch()
    result = typeset(expression, environment, tex_template)
    busy.unlink(missing_ok=True)
    return result


typeset = tex_file_writing.tex_to_svg_file
tex_file_writing.tex_to_svg_file = typeset_whole
tex_mobject.tex_to_svg_file = typeset_whole

threading.Thread(target=listen, daemon=True).start()

def next_request():
    while True:
        try:
            return requests.get(timeout=IDLE)
        except queue.Empty:
            if gc.get_count()[0] > 1000:
                gc.collect()


while (line := next_request()) is not None:
    script, _, result = line.partition("\t")
    running = script
    sys.argv = [script, result]
    status = "abandoned"
    held = fcntl is None
    while not held and wanted():
        try:
            fcntl.flock(turn, fcntl.LOCK_EX | fcntl.LOCK_NB)
            held = True
        except OSError:
            time.sleep(0.02)
    try:
        if wanted():
            with open(script, encoding="utf-8") as source:
                code = compile(source.read(), script, "exec")
            given = {"_wanted": wanted, "_scratch": scratch, "_worlds": worlds}
            exec(code, {"__name__": "__main__", **given})
            status = "ok"
    except BaseException as error:  # noqa: BLE001 - a script must never take the server down
        if wanted():
            traceback.print_exc()
        status = f"failed {type(error).__name__}: {error}".replace("\n", " ")
    finally:
        if held and fcntl:
            fcntl.flock(turn, fcntl.LOCK_UN)
    running = None
    replies.write(f"\x1e{status}\n")
    replies.flush()
