# 13. The deck file

A deck is a JSON file whose name ends in `.suit.json`. It is the deck's tree written down
as it is: the types in `crates/core/src/ast.rs`, serialised. It is meant to be written by
Suit and read by people when they are curious, compare versions, or need to change
something the window does not offer yet.

Edit it by hand with care. Suit checks a deck when it opens it and reports what does not
fit under Problems, but a file that is not valid JSON, or not this shape, does not open.

## The deck

```json
{
  "config": {
    "width": 1920,
    "height": 1080,
    "fps": 60,
    "background": "#000000",
    "gap": 0.1,
    "gap_on_loops": false
  },
  "slides": [ … ],
  "names": { "2": "title", "5": "subtitle" },
  "next_id": 99
}
```

| Field | Meaning |
|---|---|
| `config.width`, `height` | The size of the picture in pixels. The frame is always 8 units high; its width follows from the proportions. |
| `config.fps` | Frames per second |
| `config.background` | The background colour |
| `config.gap` | The hold in seconds added after every slide, so that it rests on its finished picture. 0 for none. |
| `config.gap_on_loops` | Whether looping slides get that hold too |
| `slides` | The slides, in order |
| `names` | The name of every variable, by its number |
| `next_id` | The next unused number. Every node and variable gets one, and none is used twice. |

These six settings of `config` are the ones that are not in the window yet. To change
them, close the deck, edit the file, and open it again.

## A slide

```json
{
  "id": 1,
  "title": "Title",
  "config": {
    "looping": false,
    "auto_next": false,
    "skip": false,
    "playback_rate": 1.0,
    "reversed_playback_rate": 1.0,
    "notes": "Welcome everyone.",
    "direction": "horizontal"
  },
  "content": { "blueprint": { "body": [ … ] } }
}
```

| Field | Meaning |
|---|---|
| `looping`, `auto_next`, `notes` | *Loop until I move on*, *Move on by itself*, and the notes of [chapter 9](09-slides-and-presenting.md#a-slides-settings) |
| `skip` | The slide is run, because later slides build on it, but not shown |
| `playback_rate`, `reversed_playback_rate` | How fast it plays forwards and backwards. 1 is as recorded. |
| `direction` | `horizontal` or `vertical`: which way the HTML export moves to this slide |
| `content` | `blueprint`, a list of steps; or `media` with a `path`, a slide that plays a video file |

## A step

Each entry of `body` is a node: a number of its own and the step itself.

```json
{ "id": 5, "item": { "play": { "anims": [ … ] } } }
```

There are ten kinds of step:

| Key | Step | Holds |
|---|---|---|
| `let_object` | A new object | `var`, and `value`: how it is made |
| `let_value` | A named value | `var`, and `value`: a formula |
| `apply` | An instant change | `target`, and `call` |
| `add`, `remove` | Show, hide | `targets` |
| `play` | Animations | `anims` |
| `wait` | A pause | `duration` in seconds |
| `attach` | Keep doing | `target`, and `updater`: `always` a call, or `every` with a `dt` variable and a call |
| `detach` | Stop keeping on | `target` |
| `raw` | Python code | `python`, and `iface` with its `duration` |

### How an object is made

```json
{ "new": { "ctor": "Circle", "args": [ ["radius", { "expr": { "real": 1.2 } }] ] } }
```

- `new`: a constructor of the catalogue with its settings;
- `group`: `items`, and `vector` for a group of vector objects;
- `copy`: of another object;
- `chain`: a `base` made in one of these ways, and a `call` done to it at once. This is
  **Then also…**.

Settings are a list of `[name, value]` pairs, in the order they were given. A value is
one of:

| Key | Meaning |
|---|---|
| `expr` | A formula (below) |
| `fun` | A rule: a `param` variable and a `body` formula |
| `rate` | An easing, by name |
| `object` | An object made on the spot and not named |

### A place

An object is referred to by the number of its variable, and a part of it by the path to
the part:

```json
{ "root": 17 }
{ "root": 43, "path": [0, 2] }
```

### An animation

```json
{ "prim": { "ctor": "FadeIn", "args": [ ["mobject", { "expr": { "place": { "root": 17 } } }] ],
            "opts": { "run_time": 0.6 } } }
```

| Key | Meaning |
|---|---|
| `prim` | An animation of the catalogue, with its settings |
| `animate` | An animated change: `target` and one or more `calls` |
| `par`, `seq` | Several, together or one after another: `items` |

`opts` holds `run_time`, `rate` and `lag`, each only if it differs from the usual.

### A formula

| Written in Suit | In the file |
|---|---|
| `1.5` | `{ "real": 1.5 }` |
| `3` | `{ "int": 3 }` |
| `true` | `{ "bool": true }` |
| `"hello"` | `{ "str": "hello" }` |
| `(1, 2)` | `{ "vec3": [ {"real": 1.0}, {"real": 2.0}, {"real": 0.0} ] }` |
| `#58c4dd` | `{ "color": "#58c4dd" }` |
| `UP`, `BLUE` | `{ "const": "UP" }` |
| a named value, or the `x` of a rule | `{ "var": 22 }` |
| `circle` | `{ "place": { "root": 17 } }` |
| `2 * LEFT` | `{ "call": { "fun": "*", "args": [ {"real": 2.0}, {"const": "LEFT"} ] } }` |
| `if a then b else c` | `{ "if": { "cond": …, "then": …, "otherwise": … } }` |
| `t.value` | `{ "query": { "target": { "root": 91 }, "query": "get_value", "args": [] } }` |

## What the numbers are for

Names can be changed and can repeat; numbers cannot. Every reference in the file is by
number, so renaming an object touches one line of `names` and nothing else, and two
versions of a deck can be compared step by step.

The names become Python identifiers on export, with the number appended
(`title_2`, `circle_17`), so two objects called the same do not collide.

## Compatibility

The format has no version number yet. Suit is at version 0.1, and the format may still
change in ways that old files do not survive. Keep the exported Python of a talk you care
about: that file depends only on Manim.

Next: [14. Developing](14-developing.md)
