# 10. Saving and exporting

## Deck files

A deck is one file whose name ends in `.suit.json`. It holds the slides, their steps, the
names of the objects, and the deck's size and frame rate. [Chapter 13](13-deck-file-format.md)
describes what is inside.

| File menu | What it does |
|---|---|
| **New deck** | An empty deck with one slide |
| **Open…** | Opens a deck. Suit's own file browser walks through folders; only decks are listed. |
| **Open the sample deck** | The deck Suit starts with, as a fresh unsaved copy |
| **Save** (Ctrl+S) | Saves to the deck's file, asking for one the first time |
| **Save as…** | Saves under another name or in another folder |

A dot after the name in the top bar means there are unsaved changes. Closing the window,
or opening another deck, asks first what to do with them.

Undo and Redo are not saved: they belong to the session.

### In Docker

The container sees one folder of your computer: `decks`, next to `docker-compose.yml`,
which is `/decks` inside. The file browser opens there. Save your decks there and keep
your pictures there, and they are ordinary files on your computer. Anything saved elsewhere
in the container is lost when it stops.

## Pictures

A deck names its pictures by where they are relative to the deck file, so a deck and its
pictures can be moved together. If you save a deck to another folder, the names are
rewritten so that they still point at the same files.

**File → Gather pictures** copies the pictures that are somewhere else into a folder next
to the deck. Do this before sending a deck to someone.

## Exporting Python

**File → Export Python** writes the deck as a Python file for
[manim-slides](https://manim-slides.eertmans.be/), next to the deck and with the same
name: `talk.suit.json` gives `talk.py`. A deck with errors in Problems is not exported
until they are fixed.

The file is ordinary Manim code with one call per step:

```python
class Deck(Slide):
    def construct(self):
        # ── Slide 1: Title ──
        self.next_slide(notes="Welcome everyone.")
        title_2 = Text("Manim, without the code", font_size=56.0)
        title_2.set_z_index(1)
        self.play(Write(title_2, run_time=1.5))
        self.wait(0.1)
```

It is yours to read, learn from, and render. It says at the top that edits are overwritten
by the next export: change the deck in Suit, not the file.

Two things in it are Suit's own rather than Manim's, and are written into any export that
uses them: the classes behind *Graph of a function* (`Plot`) and behind tangents
(`Tangent`, `Glide`). The file therefore needs nothing but Manim and manim-slides.

### Rendering and presenting the export

```bash
manim-slides render talk.py Deck      # renders every slide to video
manim-slides present Deck             # presents them, in manim-slides' own window
manim-slides convert Deck talk.html   # or: one HTML page that presents in any browser
```

`manim-slides present` needs a Qt library (`pip install "manim-slides[pyside6]"`); the
other two do not. The HTML page is the easiest thing to carry to another computer: it
comes with a folder of assets, `talk_assets`, which has to stay next to it.

With Docker, the same commands run in the container, on the files in `decks`:

```bash
docker compose run --rm suit manim-slides render talk.py Deck
docker compose run --rm suit manim-slides convert Deck talk.html
```

The results appear in `decks` on your computer. Open `talk.html` in a browser.
(`manim-slides present` opens a window of its own, which the container has no screen for;
use the HTML page, or Suit's own **Present**.)

Rendering a full-HD deck at 60 frames per second takes a while. For a quick look, export
a smaller picture with the command line tool below (`--height 270`).

## The command line

`suit-cli` does without a window what does not need one. From the project folder it is
run as `cargo run -p suit-cli -- …`; in Docker as `docker compose run --rm suit suit-cli …`.

| Command | What it does |
|---|---|
| `suit-cli check talk.suit.json` | Lists the problems in a deck. Fails if there are errors, so it can guard a script. |
| `suit-cli timeline talk.suit.json` | Lists every slide and step with its length and its frames |
| `suit-cli export talk.suit.json -o talk.py` | Writes the Python file. `--height 270` renders smaller, for speed; `--class-name Talk` names the class. |
| `suit-cli sample -o sample.suit.json` | Writes the sample deck |
| `suit-cli signature` | Prints the catalogue of objects, actions and animations as JSON |
| `suit-cli probe talk.suit.json` | Writes the script Suit runs in Manim to see the scene after every step. For the curious and for debugging. |

`timeline` is a quick way to see where the time of a talk goes:

```
Slide 2 “Its slope”  9.10 s  546 frames
   1  New tangent (Tangent)
   2  Create tangent  1.00 s, frames 0–59
   …
   7  Glide tangent  4.00 s, frames 120–359
Total  13.20 s  792 frames at 60 fps
```

These times are exact. They are worked out from the steps without running Manim, and the
tests check them against the number of frames Manim really renders.

## Size, frame rate and background

A deck is 1920×1080 at 60 frames per second on black. These are not yet in the window:
they are the first lines of the deck file, and can be changed there with a text editor
([chapter 13](13-deck-file-format.md#the-deck)).

Next: [11. Problems](11-problems.md)
