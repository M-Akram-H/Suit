# 2. Installing

There are four ways to run Suit.

| Way | For whom | What you install |
|---|---|---|
| [**The Windows installer**](#with-the-installer-on-windows) | Windows 10 and 11 | One setup program. Manim is set up for your account afterwards. |
| [**The Fedora package**](#with-the-package-on-fedora) | Fedora Linux | One RPM, which brings Python and LaTeX with it. Manim is set up for your account afterwards. |
| [**Docker**](#with-docker-on-any-machine) | Everyone, on Linux, macOS or Windows | Git and Docker. Manim, LaTeX and Suit itself come in the image. Suit is shown in a web browser. |
| [**From the source**](#without-docker-on-linux) | Linux users on other distributions, and developers | Rust, Python, Manim, LaTeX |

The installers are on the [Releases page](https://github.com/M-Akram-H/Suit/releases).

## With the installer, on Windows

### What you need

Windows 10 or 11 (64-bit), an internet connection, and about 1.5 GB of free space once
Manim and LaTeX are there. You do not need to be an administrator: everything goes into
your own account.

### Installing

1. Download `Suit-0.1.0-Setup.exe` and run it.
2. Windows may say ***Windows protected your PC***. That is what it says about any
   program whose maker has not bought a signing certificate. Choose **More info**, then
   **Run anyway**.
3. Go through the pages. On the last one, leave **Set up Manim now** ticked.

### Setting up Manim

A window opens and does the rest, asking before it installs anything for the whole
computer:

- It looks for **Python 3.12**. If there is none, it offers to install it with `winget`,
  which comes with Windows.
- It makes a Python environment for Suit alone and installs **Manim** into it
  (about 300 MB, a few minutes).
- It looks for **LaTeX**, which formulas need. If there is none, it offers to install
  [MiKTeX](https://miktex.org/). Texts, shapes and graphs work without it.
- It tells MiKTeX to fetch the LaTeX packages a formula needs by itself. The first
  formula you make therefore takes a while.

Then start **Suit** from the Start menu. If you skipped this step, or it failed, run it
again at any time: **Start menu → Suit → Set up Manim for Suit**.

### What goes where

| What | Where |
|---|---|
| Suit, its examples, the uninstaller | `%LOCALAPPDATA%\Programs\Suit` |
| Manim, and what it has typeset | `%LOCALAPPDATA%\Suit` |
| Shortcuts | The Start menu, under *Suit*, and the desktop |

**Uninstalling**: Settings → Apps → Suit, or **Start menu → Suit → Uninstall Suit**. It
asks whether Manim should go as well.

### What it was tested on

The Windows build is made on Linux, and there has been no Windows PC to try it on. Under
[Wine](https://www.winehq.org/), which runs Windows programs on Linux, the setup program
installs and uninstalls cleanly, Suit starts, finds the Python that was set up for it and
starts it, and the command line tool checks and exports decks. Drawing with Manim could
not be tried there, and the set-up window (a PowerShell script) not at all. If something
fails on your PC, please [report it](15-troubleshooting.md#reporting-a-problem).

## With the package, on Fedora

### Installing

Download `suit-0.1.0-1.fc44.x86_64.rpm` and install it:

```bash
sudo dnf install ./suit-0.1.0-1.fc44.x86_64.rpm
```

`dnf` brings what Suit needs from Fedora's own packages: Python 3.12, the compiler and
libraries two of Manim's parts are built with, and the LaTeX packages Manim's formulas
use. On a system that has none of it yet, that is a few hundred megabytes.

### Setting up Manim

Manim is not a Fedora package, so it is installed for your user account, without `sudo`:

```bash
suit-setup
```

or choose **Set up Manim for Suit** in the applications overview. It makes a Python
environment for Suit alone in `~/.local/share/suit` and installs Manim into it (about
300 MB, a few minutes).

Then start **Suit** from the applications overview, or type `suit`. A deck
(`something.suit.json`) can also be opened by double-clicking it.

### What goes where

| What | Where |
|---|---|
| `suit`, `suit-cli`, `suit-setup` | `/usr/bin` |
| Example decks | `/usr/share/suit/examples` |
| Manim | `~/.local/share/suit/venv` |
| What Manim has typeset | `~/.cache/suit` |

**More LaTeX.** The package brings what Manim's ordinary formulas need. If a formula uses
a LaTeX package that is missing, the top bar names the file LaTeX could not find; install
it with `sudo dnf install 'tex(name.sty)'`, or everything at once with
`sudo dnf install texlive-scheme-medium`.

**Removing**: `suit-setup --remove`, then `sudo dnf remove suit`.

**Another Fedora version.** The package is built for Fedora 44. For another one, build
it yourself: [chapter 14](14-developing.md#building-the-installers).

### What it was tested on

The package was installed on a clean Fedora 44 (in a container): `dnf` found everything
it requires, `suit-setup` installed Manim as an ordinary user, formulas typeset with the
LaTeX packages it brings, and the installed Suit opened and drew the example decks. It
has not yet been tried on a Fedora desktop with a real screen, where the launcher entry,
the icon and opening a deck by double click come into play.

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

### macOS

Suit is plain Rust and needs nothing from Linux in particular, so it may well build on
macOS, given a Python with Manim. This has not been tried. Docker is the way that is
known to work there.

Next: [3. Your first deck](03-first-deck.md)
