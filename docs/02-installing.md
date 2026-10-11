# 2. Installing

There are two ways to run Suit.

| Way | For whom | What you install |
|---|---|---|
| [**Docker**](#with-docker-on-any-machine) | Everyone, on Linux, macOS or Windows | Git and Docker. Manim, LaTeX and Suit itself come in the image. Suit is shown in a web browser. |
| [**Directly**](#without-docker-on-linux) | Linux users who want a real window, and developers | Rust, Python, Manim, LaTeX |

## With Docker, on any machine

### What you need

- **Git**, to get the code: [git-scm.com](https://git-scm.com/downloads).
- **Docker** with Compose:
  - Windows and macOS: [Docker Desktop](https://www.docker.com/products/docker-desktop/).
    Start it, and wait until it says it is running.
  - Linux: [Docker Engine](https://docs.docker.com/engine/install/) with the Compose
    plugin (or Docker Desktop).
- About **4 GB of free disk space**, and for the first start an internet connection.

### From nothing to a running Suit

Open a terminal (on Windows: PowerShell) and run these three lines:

```bash
git clone https://github.com/M-Akram-H/Suit.git
cd Suit
docker compose up
```

The first time, Docker builds the image: it downloads LaTeX and Manim (about 2 GB) and
compiles Suit. This takes from fifteen minutes to three quarters of an hour, depending
mostly on the internet connection. It happens once.

When the terminal says

```
Suit is starting. Open http://localhost:6080 in a web browser.
```

open **<http://localhost:6080>** in a browser. Suit is there, with its sample deck:

![The editor with the sample deck open](images/editor.png)

Click once inside the picture so that the keyboard goes to Suit, and carry on with
[chapter 3](03-first-deck.md).

### Stopping and starting again

- **Stop**: press Ctrl+C in the terminal, or run `docker compose down` in the Suit
  folder from another terminal.
- **Start again**: `docker compose up`. From the second time on this takes a few seconds.
- **In the background**: `docker compose up -d` gives the terminal back.
  `docker compose logs` shows what Suit said, `docker compose down` stops it.

### Where your decks are

The container sees one folder of your computer: **`decks`**, inside the Suit folder.
In the container it is called `/decks`, and Suit's file browser opens there.

- Save your decks there (**File → Save as…**).
- Put the pictures you want to use there, before choosing them with **Picture**.
- Exports appear there.

They are ordinary files on your computer, and stay when the container is stopped or the
image rebuilt. **Anything saved to another folder inside the container is lost** when it
stops.

To start straight on one of your decks instead of the sample:

```bash
docker compose run --rm --service-ports suit talk.suit.json
```

### Settings

Two things can be set, as environment variables or, more comfortably, in a file called
`.env` next to `docker-compose.yml`:

```
SUIT_SCREEN=1920x1080
SUIT_PORT=6080
```

| Setting | Meaning | Usual value |
|---|---|---|
| `SUIT_SCREEN` | The size of the screen inside the container. The browser scales it to its window, so choose the size of your browser's window, or of the projector, for a sharp picture. A smaller size makes everything in Suit larger. | `1600x900` |
| `SUIT_PORT` | The port on your computer. Change it if 6080 is taken; the address is then `http://localhost:` and that number. | `6080` |

### Things to know about the browser

- The page is [noVNC](https://novnc.com/): a remote screen in a browser tab. A small
  handle at the left edge opens its menu, with **full screen**, the clipboard, and
  settings.
- **Some keys belong to the browser.** F5 reloads the page, Ctrl+W closes the tab, and
  Ctrl+S may ask to save the page. Use Suit's buttons and menus for those: **Present**,
  **File → Save**. A reload is harmless, by the way: Suit keeps running in the container
  and the page connects again.
- **Copy and paste** between your computer and Suit goes through the clipboard panel of
  the handle.
- **It is drawn by the processor.** There is no graphics card in the container. Editing
  is fine; playing a slide is less smooth than in a window of its own.
- **Only your computer can reach it.** The port is bound to `127.0.0.1`, and there is no
  password in front of the screen. Think before you change that in `docker-compose.yml`.

### Other programs in the container

The image also has `suit-cli`, `manim` and `manim-slides`. They are run next to your
decks:

```bash
docker compose run --rm suit suit-cli check talk.suit.json
docker compose run --rm suit manim-slides render talk.py Deck
docker compose run --rm suit manim-slides convert Deck talk.html
docker compose run --rm suit bash
```

[Chapter 10](10-saving-and-exporting.md) explains what they are for.

### Updating and removing

```bash
git pull                 # get the new version
docker compose build     # rebuild the image; only what changed is redone
```

```bash
docker compose down --volumes --rmi all    # remove the container, its cache and the image
```

Your `decks` folder is not touched by either.

### What it was tested on

The image was built and run on Linux (x86-64) with Docker 29 and Compose 5: drawing,
typing, saving into `decks`, presenting, and the command line tools. It is made of parts
that exist for Windows, macOS and ARM processors alike, but it has **not been run on
those yet**. If it fails for you, [chapter 15](15-troubleshooting.md) says what to look
at and where to report it.

## Without Docker, on Linux

Suit then runs in a window of its own, drawn by your graphics card.

### 1. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Version 1.90 or newer is needed.

### 2. What Manim needs from the system

On Ubuntu and Debian:

```bash
sudo apt install build-essential pkg-config python3-dev python3-venv \
    libcairo2-dev libpango1.0-dev \
    texlive-latex-extra texlive-fonts-recommended texlive-science cm-super dvisvgm
```

Other distributions have the same things under other names: a C compiler, the
development files of Cairo and Pango, and a LaTeX with the `latex-extra` and `science`
collections and `dvisvgm`. [Manim's own guide](https://docs.manim.community/en/stable/installation.html)
lists them.

### 3. Suit and Manim

```bash
git clone https://github.com/M-Akram-H/Suit.git
cd Suit
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

### 4. Run it

```bash
./run.sh
```

The first run compiles Suit, which takes some minutes. After that it starts at once.

```bash
./run.sh --new                       # an empty deck
./run.sh talk.suit.json              # a deck of your own
./run.sh talk.suit.json --present    # straight into presenting it
./run.sh --help                      # everything it can be started with
```

`run.sh` can be called from any folder. It checks that its Python has Manim before it
starts Suit, and says what is missing if not. To use another Python than `.venv`, name it
in `SUIT_PYTHON`:

```bash
SUIT_PYTHON=/path/to/python ./run.sh
```

If the `.venv` was made inside a [distrobox](https://distrobox.it/) container and
`run.sh` is called from outside it, the script notices and starts Suit in that container.

### macOS and Windows without Docker

Suit is plain Rust and needs nothing from Linux in particular, so it may well build
there, given a Python with Manim. This has not been tried. Docker is the way that is
known to work.

Next: [3. Your first deck](03-first-deck.md)
