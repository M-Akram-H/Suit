# The .suit.json format

For changing a deck that was edited in Suit after it was built, when rebuilding from a
script would lose those edits. For new decks use `suitdeck.py`: it writes this format.

Check every hand edit with `python3 scripts/suitdeck.py check NAME.suit.json`. Keep a
copy of the original until the check passes.

## The whole file

```json
{
  "config": {"width": 1920, "height": 1080, "fps": 60, "background": "#000000",
             "gap": 0.1, "gap_on_loops": false},
  "slides": [ SLIDE, ... ],
  "names": {"2": "title", "5": "subtitle"},
  "next_id": 99
}
```

- `names` maps variable numbers (as strings) to the names shown in Suit.
- Every number in the file (node ids, variable numbers, slide ids) comes from one counter
  and is unique. `next_id` is the last one handed out: give new things numbers above
  it, and raise it to the largest you used.
- Times are seconds, as numbers. Colours are `"#rrggbb"`.

## A slide

```json
{"id": 1, "title": "Title",
 "config": {"looping": false, "auto_next": false, "skip": false, "playback_rate": 1.0,
            "reversed_playback_rate": 1.0, "notes": "", "direction": "horizontal"},
 "content": {"blueprint": {"body": [ STEP, ... ]}}}
```

## A step

`{"id": N, "item": COMMAND}`, where COMMAND is one of:

| Command | Holds |
|---|---|
| `{"let_object": {"var": V, "value": {"id": N, "item": OBJECT}}}` | a new object |
| `{"let_value": {"var": V, "value": EXPR}}` | a named value |
| `{"apply": {"target": PLACE, "call": CALL}}` | an instant change |
| `{"add": {"targets": [PLACE, ...]}}`, `{"remove": {...}}` | show, hide |
| `{"play": {"anims": [{"id": N, "item": ANIM}, ...]}}` | animations, together |
| `{"wait": {"duration": 1.0}}` | a pause |
| `{"attach": {"target": PLACE, "updater": {"always": CALL}}}` | keep doing |
| `{"attach": {"target": PLACE, "updater": {"every": {"dt": V, "call": CALL}}}}` | keep doing, with the frame time `{"var": V}` |
| `{"detach": {"target": PLACE}}` | stop keeping on |
| `{"raw": {"python": "...", "iface": {"requires": [], "ensures": [], "defines": [], "duration": 0.0}}}` | Python |

## Objects, calls, places

```json
{"new": {"ctor": "Circle", "args": [["radius", ARG], ["color", ARG]]}}
{"group": {"vector": true, "items": [PLACE, ...]}}
{"copy": PLACE}
{"chain": {"base": {"id": N, "item": OBJECT}, "call": CALL}}
```

`chain` is **Then also…**: the base object, then a call done to it as it is made.

A CALL is `{"method": "next_to", "args": [["mobject_or_point", ARG], ["direction", ARG]]}`.
A PLACE is `{"root": V}`, or `{"root": V, "path": [2]}` for a part.

Settings (`args`) are `[name, ARG]` pairs, named as in the catalogue
(`suitdeck.py catalogue NAME`). An ARG is one of `{"expr": EXPR}`,
`{"fun": {"param": V, "body": EXPR}}` (a rule; `V` is in `names`, usually as `x`), or
`{"rate": "smooth"}`.

## Animations

```json
{"prim": {"ctor": "FadeIn", "args": [["mobject", {"expr": {"place": {"root": 7}}}]], "opts": {}}}
{"animate": {"target": PLACE, "calls": [CALL, ...], "opts": {}}}
{"par": {"items": [{"id": N, "item": ANIM}, ...], "opts": {"lag": 0.3}}}
{"seq": {"items": [...], "opts": {}}}
```

`opts` may hold `run_time` (seconds), `rate` (an easing name) and `lag`.

## Expressions

| In Suit | EXPR |
|---|---|
| `1.5`, `3`, `true`, `"text"` | `{"real": 1.5}`, `{"int": 3}`, `{"bool": true}`, `{"str": "text"}` |
| `(1, 2)` | `{"vec3": [{"real": 1.0}, {"real": 2.0}, {"real": 0.0}]}` |
| `#58c4dd` | `{"color": "#58c4dd"}` |
| `UP`, `BLUE`, `PI` | `{"const": "UP"}` |
| a named value, a rule's `x` | `{"var": V}` |
| an object | `{"place": {"root": V}}` |
| `2 * LEFT` | `{"call": {"fun": "*", "args": [{"int": 2}, {"const": "LEFT"}]}}` |
| `if a then b else c` | `{"if": {"cond": A, "then": B, "otherwise": C}}` |
| `circle.center` | `{"query": {"target": {"root": V}, "query": "get_center", "args": []}}` |

Functions are `+ - * / pow neg < <= > >= == != and or not sin cos tan exp log sqrt abs
min max norm x y z` (the last three are a point's parts).
