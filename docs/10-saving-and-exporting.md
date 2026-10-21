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

## Exporting

A deck can leave Suit in three forms. Each is written next to the deck, so the deck has
to have been saved once; and none is written while Problems lists errors.

| File menu | Gives | For |
|---|---|---|
| **Export PDF…** | `talk.pdf` | Printing, handing out, reading without Manim |
| **Export website…** | the folder `talk-website` | Watching the talk in any browser, on any device, and putting it online |
| **Export Python** | `talk.py` | Rendering with manim-slides yourself, and learning Manim from your own deck |

## Exporting a PDF

**File → Export PDF…** asks one thing:

| Choice | Pages |
|---|---|
| **A page for every slide, as it ends** | The picture each slide rests on: what the audience sees before you move on. This is what manim-slides makes a PDF of. |
| **A page for every step, as the slide builds up** | A page whenever a step changes the picture, so a reader can follow how a slide is built. A step that changes nothing that shows makes no page, and neither does an empty screen, such as a slide clearing away the one before. |

The pages are drawn from what Manim showed after each step, the same pictures as the
canvas. They are not screenshots: every letter and every curve is a shape in the PDF, so
it stays sharp at any zoom and the file stays small. Pictures in the deck go in as they
are. The PDF has a bookmark for every slide, named by its title. A page is 7.5 inches
high, as wide as the deck's shape asks (13.33 inches for 16:9, PowerPoint's widescreen
size). Skipped slides have no page.

The PDF is made at once, from the slides as they are on the canvas. While Manim is still
working out the slides after an edit, the window says *Waiting for Manim to finish the
slides…* and Export waits with it.

## Exporting a website

**File → Export website…** turns the deck into a website: every slide rendered by Manim
as a video, and a player that runs them in the browser the way **Present** runs them in
Suit. It works on a computer, a tablet or a phone, from a web host or straight from the
disk.

| Choice | Meaning |
|---|---|
| **Size of the videos** | The deck's own size, or 1280×720 or 854×480. Smaller videos render sooner, make a smaller folder and load faster on a phone; the deck's size looks sharpest on a big screen. |
| **Add a PDF of the slides to download** | The page gets a download button for the PDF, with a page for every slide or every step. On unless you untick it. |
| **Put my notes on it, shown with N** | The slides' notes go with them, and N or a button shows them. Off unless you tick it, because anyone who opens the website can read them. |

**Export** starts Manim, and the top bar shows how far it has got: *Rendering the
slides*, then *Making the rewind videos*, then *Getting the videos ready for the web*.
Rendering takes as long as a manim-slides render of the deck: the sample deck, 22
seconds of slides, takes about a minute at full HD. You can go on editing meanwhile; the
website is of the deck as it was when you pressed Export. **Stop** in the bar stops
Manim and leaves the folder as it was. When the website is written, the bar says where
and how big it is, and **Watch it** opens it in your browser.

Exporting the same deck again is quicker: Manim keeps the animations it rendered (in
Suit's cache folder, for a day) and uses them again where nothing about them changed.
The sample deck takes half as long the second time.

### What is in the folder

| File | What it is |
|---|---|
| `index.html`, `player.js`, `player.css` | The page and the player: plain HTML, JavaScript and CSS, with nothing to install and no outside services. |
| `deck.js` | The slides: their titles, settings, notes if you chose them, and videos. |
| `videos/` | Every slide as Manim rendered it, forwards and backwards. |
| `slides.pdf` | The PDF, if you chose it. |
| `README.md` | How to watch it, and how to put it online. |
| `vercel.json` | Tells Vercel that a video never changes under the same name, so browsers keep it. |
| `icon.png` | The icon of the browser tab. Replace it with your own if you like. |

Exporting again rewrites the first five rows and leaves everything else in the folder
alone: your own `README.md`, `vercel.json` and `icon.png`, a `.git` repository, and the
`.vercel` folder Vercel keeps its link to the project in. So the website can be put
online once and updated with the same command after every export. Suit refuses to write
into a folder that has other files in it and that it did not write before.

### Watching it

Open `index.html` in a browser. The slide fills the window; the keys are those of
**Present**, and so of manim-slides:

| Key | What it does |
|---|---|
| → , Page Down, Enter, or a click | **Next.** Starts the next slide at once; on a held slide, lets it carry on. |
| ← , Page Up | **Previous.** The slide before, from its start. |
| V | **Rewind.** The slide before, backwards. Next then plays it forwards again. |
| R | **Replay.** This slide again, from its start. |
| Space | **Hold**, and carry on. |
| Home, End | The first slide, the last slide. |
| F | Full screen on or off. |
| N | The notes of this slide, if they were exported. |
| B or . | Blank the screen, and back: what the blank button of a presentation remote sends. |
| H | Hide or show the mouse pointer. |
| ? | The keys. |

Moving the mouse or touching the screen brings up a row of buttons for the same, the
number of the slide (click it for a list of all slides, to jump to one) and, if there is
one, the PDF. On a phone or tablet, tap the right of the slide for the next one and the
left third for the one before, or swipe. Looping slides loop, slides that move on by
themselves do, and where the browser allows it, the screen does not go to sleep while
the slides are shown.

The address follows the slide: `…/#5` is slide 5, so a link can point at one, and
reloading the page stays there. Three choices of manim-slides' presenter can be added to
the address: `?start-paused`, `?next-terminates-loop` and `?skip-all`, as in
`index.html?start-paused#1` (chapter 9 says what each does).

### Putting it online

The folder is a complete static website: there is nothing to build and no program to run
on the server, so any host that serves files will do. The folder's `README.md` has the
steps; in short:

- **Vercel**: with [Node.js](https://nodejs.org) installed, run `npx vercel --prod` in the
  folder. The first time it asks you to log in and to name the project; the other answers
  it suggests are right. It prints the address of the site. After the next export, the
  same command updates the same address. Or put the folder in a GitHub repository and
  import that at [vercel.com/new](https://vercel.com/new), with *Other* as the framework
  and no build command; every push then updates the site.
- **Netlify**: drag the folder onto [app.netlify.com/drop](https://app.netlify.com/drop).
- **GitHub Pages**: push the folder to a repository and choose *Settings → Pages → Deploy
  from a branch*.

Vercel's free plan takes up to 100 MB per upload from its command line. Manim's flat
pictures make small videos: the sample deck, 22 seconds of slides, comes to 1.3 MB at full
HD with its rewind videos, and 0.7 MB at 854×480. It takes a long deck, or one with a lot
of movement, to reach 100 MB. Suit says so when a website is bigger than that; export
with smaller videos to bring it down.

### In Docker

The website is written to `decks`, next to the deck, like everything else. **Watch it**
cannot open a browser from inside the container: open `index.html` from the `decks`
folder on your computer instead.

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
| `suit-cli pdf talk.suit.json` | Writes `talk.pdf` next to the deck, a page for every slide; `--every-step` for a page for every step, `-o` for another name. Needs Manim. |
| `suit-cli website talk.suit.json` | Writes the website to `talk-website` next to the deck, printing how far it has got. `--height 720` for smaller videos, `--notes` to put the notes on it, `--no-pdf` to leave the PDF out, `--every-step` for its pages, `-o` for another folder. Needs Manim. |
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
