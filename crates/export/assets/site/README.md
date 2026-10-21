# {{title}}

A presentation made with [Suit](https://github.com/M-Akram-H/Suit) and Manim, as a website.
Open `index.html` in a browser to watch it on this computer.

## Watching

| Key | What it does |
|---|---|
| → , Page Down, Enter, or a click | Next. Starts the next slide at once; on a held slide, lets it carry on. |
| ← , Page Up | Previous: the slide before, from its start. |
| V | Rewind: the slide before, backwards. Next plays it forwards again. |
| R | This slide again, from its start. |
| Space | Hold, and carry on. |
| Home, End | The first slide, the last slide. |
| F | Full screen on or off. |
| N | The notes of this slide, if they were exported. |
| B or . | Blank the screen, and back. |
| H | Hide or show the mouse pointer. |
| ? | The keys. |

On a phone or tablet, tap the right of the slide for the next one and the left for the one
before, or swipe. Moving the mouse or touching the screen brings up buttons for the same.

The address remembers the slide: `…/#5` opens slide 5, so a link can point at one. Three
choices of manim-slides' own presenter can be added to the address too:
`?start-paused` (the first slide waits for a key), `?next-terminates-loop` (Next lets a
looping slide finish its round) and `?skip-all` (every slide moves on by itself), as in
`index.html?start-paused#1`.

## Putting it online

This folder is a complete static website: no build step, no server code. Any host that
serves files will do. Exporting again from Suit rewrites the slides here and leaves
everything else alone, including this file and the hosts' own settings, so the same steps
put the new version online.

### Vercel

With [Node.js](https://nodejs.org) installed, run this in this folder:

```bash
npx vercel --prod
```

The first time it asks you to log in and to name the project; accept the other answers it
suggests. It then prints the address of the site. Run it again after the next export to
update the same address.

Or put the folder in a GitHub repository, and import that repository at
[vercel.com/new](https://vercel.com/new): framework preset *Other*, no build command, and
this folder as the root directory. Every push then updates the site.

Vercel's free plan takes up to 100 MB per upload from the command line. If the folder is
bigger, export again from Suit with smaller videos.

### Netlify

Drag this folder onto [app.netlify.com/drop](https://app.netlify.com/drop).

### GitHub Pages

Push this folder to a GitHub repository, then choose *Settings → Pages → Deploy from a
branch*.

## What is in the folder

| File | What it is |
|---|---|
| `index.html`, `player.js`, `player.css` | The page and the player. Plain HTML, JavaScript and CSS. |
| `deck.js` | The slides: their titles, settings and videos. |
| `videos/` | Every slide as Manim rendered it, forwards and backwards. |
| `slides.pdf` | The slides as a PDF, if it was exported with them. |
| `icon.png` | The icon of the browser tab; replace it with your own if you like. |
| `vercel.json` | Tells Vercel that a video never changes under the same name. |

Suit writes the first four rows again on every export, so make changes to the deck in Suit
rather than in those files.
