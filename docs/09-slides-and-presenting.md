# 9. Slides and presenting

## Slides

A deck is a row of slides. While presenting, a slide plays its steps and then rests on its
last picture until you move on.

- **Add slide** puts a new slide after the selected one.
- **↑** and **↓** above the list move the selected slide.
- A new slide begins with everything the slide before it left on the scene. Nothing is
  cleared automatically, as in a Manim scene. Begin a slide with FadeOut steps for what
  should go.

Because a slide builds on the ones before it, moving a slide can leave it without an
object it uses. Problems says so at once.

### A slide's settings

Select a slide and no step (click the slide, or *Start of the slide*). Properties shows:

| Setting | Meaning |
|---|---|
| **title** | The name in the list. It is not shown in the presentation. |
| **Loop until I move on** | The slide plays again and again until you press next. Meant for something that should keep moving while you talk. |
| **Move on by itself** | When the slide has played, the next one starts without a key. |
| **notes** | What you want to say. Shown in the notes window while presenting, and written into the export. |
| *On the slide when it starts / ends* | Which objects the slide finds, and which it leaves. For reading, not for changing. |
| **Delete this slide** | |

A looping slide should end the way it starts, or it will visibly jump on every round.
Suit warns when it does not. A slide that both loops and moves on by itself loops for
ever in Suit and in manim-slides, since a loop never ends.

Every slide that does not loop is followed by a short hold of 0.1 s, so that it rests on
the picture *after* its last animation has finished. The times in the slide list include
it.

## Playing one slide

**Play** under the canvas, or the space bar, plays the selected slide exactly as Manim
animates it.

- Space pauses and carries on.
- The bar under the canvas can be dragged to any moment.
- **Back to editing**, Esc, or selecting anything returns to the steps.
- A looping slide starts over at its end.

To play a slide, Manim has to render every frame of it once. Suit does that in the
background whenever the deck has been left alone for a moment, so Play usually starts at
once. If the slide is not ready, the canvas says *Manim is playing the slide through…*
and starts when it is. The recordings are kept in memory and only redone for slides that
changed.

This background work can be turned off under **▾** next to Present
(*Get the slides ready to play while I edit*). A slide is then played through when it is
first asked for.

## Presenting

**Present**, or F5, shows the deck from the selected slide. It takes the whole window and
goes full screen.

### Keys

| Key | What it does |
|---|---|
| → , Page Down, Enter, or a click | **Next.** Starts the next slide at once, even if this one is still playing. On a held slide it only lets the slide carry on. |
| ← , Page Up | **Previous.** Plays the slide before this one, from its start. |
| V | **Rewind.** Plays the *previous* slide backwards: from the picture this slide starts with, back to the one before. Next then plays it forwards again, so *V, →* is “show me that again”. |
| R | **Replay.** This slide again, from its start. |
| Space | **Hold**, and carry on. |
| H | Hide or show the mouse pointer. |
| F | Full screen on or off. |
| N | Your notes, in a second window. |
| Q, Esc | Leave, back to the editor at the slide that was showing. |

These are the keys and the behaviour of manim-slides' own presenter, so rehearsing in Suit
and presenting an exported deck with manim-slides feel the same.

### The notes window

**N** opens a second window with the notes of the slide that is showing, and the title of
the one that comes next. Put it on your own screen and the presentation on the projector.
The keys work from either window. N again puts it away.

### How a presentation runs

The **▾** next to Present has four choices, all off to begin with. They are the options of
`manim-slides present`:

| Choice | Effect |
|---|---|
| **Wait for a key before the first slide** | The first slide is held until you press next. |
| **Let a looping slide finish its round before moving on** | Next on a looping slide does not cut it off: the round is played to its end and rests there. A second Next moves on. |
| **Leave after the last slide** | Going past the last slide ends the presentation. |
| **Run through without waiting for keys** | Every slide moves on by itself and none loops: a rehearsal of the timing. |

and one of Suit's own: **Show my notes in a second window**.

### From the command line

```bash
./run.sh talk.suit.json --present
```

| Option | Meaning |
|---|---|
| `--present` | Present straight away |
| `--select SLIDE:STEP` | Start with this slide selected (and so present from it) |
| `--notes` | Show the notes window |
| `--start-paused` | Wait for a key before the first slide |
| `--next-terminates-loop` | Next lets a looping slide finish its round |
| `--exit-after-last-slide` | Going past the last slide ends the presentation |
| `--skip-all` | Run through without waiting |

### Presenting from Docker

In Docker the presentation is shown in the browser tab. Use the browser's own full
screen (usually F11) to fill the display. The notes window opens inside the same tab, on
top of the slides, so it is of little use there; rehearse with it, and present with the
notes on paper, or run Suit directly ([chapter 2](02-installing.md#without-docker-on-linux)).

For a talk in front of an audience, consider exporting the deck as a website and
presenting that ([chapter 10](10-saving-and-exporting.md#exporting-a-website)): its
videos play smoothly on any machine, whereas Suit draws each frame as it shows it. It
has the same keys, runs in any browser and on a phone, and can be put online so that
your audience can watch it again afterwards.

Next: [10. Saving and exporting](10-saving-and-exporting.md)
