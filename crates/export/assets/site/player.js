// Suit's web player: a deck's slides as Manim rendered them, run the way Suit's presenter
// and manim-slides run them (Suit's specification, §6.6). Plain JavaScript and no
// libraries, so the folder works from any web server, or opened straight from the disk.
//
// Each slide has two videos: forwards, and backwards for the rewind. A slide plays, then
// rests on its last picture until the next key. The next video is loaded while one
// plays, and a new one is only shown once its first picture is there, so that moving on
// never flashes the background.

(() => {
  "use strict";

  const deck = window.SUIT_DECK;
  const $ = (selector) => document.querySelector(selector);
  const body = document.body;
  const screen = $("#screen");
  const slides = (deck && deck.slides) || [];
  const count = slides.length;

  if (count === 0) {
    tell("This presentation has no slides.");
    return;
  }

  // The choices of manim-slides' own presenter, in the address: index.html?start-paused
  const asked = new URLSearchParams(location.search);
  const options = {
    startPaused: asked.has("start-paused"),
    nextEndsLoops: asked.has("next-terminates-loop"),
    runThrough: asked.has("skip-all"),
  };

  const looping = (i) => slides[i].loop && !options.runThrough;
  const movesOn = (i) => slides[i].auto_next || options.runThrough;

  // ── State ─────────────────────────────────────────────────────────────
  // Which slide is showing, whether it runs backwards, and how: "playing", "held" (stopped
  // by hand), or "ended" (resting on the last picture of its run).
  let at = 0;
  let reversed = false;
  let run = "playing";
  // NEXT was pressed on a loop that is to finish its round first.
  let endingLoop = false;
  // The element of the run that is showing, and the one that can be seen.
  let current = null;
  let visible = null;
  // Counts the runs started, so that what an earlier one waited for is ignored.
  let generation = 0;

  // ── Videos ────────────────────────────────────────────────────────────
  const clips = new Map();

  function clip(i, back) {
    const key = `${i}${back ? "b" : "f"}`;
    let element = clips.get(key);
    if (element) return element;
    const slide = slides[i];
    const source = back ? slide.back : slide.video;
    if (slide.kind === "image") {
      element = new Image();
      element.decoding = "async";
      element.alt = slide.title || "";
    } else {
      element = document.createElement("video");
      element.muted = true;
      element.defaultMuted = true;
      element.playsInline = true;
      element.setAttribute("playsinline", "");
      element.setAttribute("muted", "");
      element.disablePictureInPicture = true;
      element.disableRemotePlayback = true;
      element.preload = "auto";
      element.addEventListener("ended", () => element === current && runEnded());
      element.addEventListener("waiting", () => element === current && waiting(true));
      element.addEventListener("playing", () => element === current && waiting(false));
      element.addEventListener("canplay", () => element === current && waiting(false));
    }
    element.addEventListener("error", () => {
      if (element === current) tell(`Slide ${i + 1} could not be loaded (${source}).`);
    });
    element.dataset.key = key;
    element.src = source;
    screen.append(element);
    clips.set(key, element);
    return element;
  }

  /** Lets go of the videos far from the slide showing, so a long deck does not fill the
      memory of a phone; and starts loading the ones likely to be wanted next. */
  function tidy() {
    for (const [key, element] of clips) {
      const slide = parseInt(key, 10);
      if (Math.abs(slide - at) > 2 && element !== visible && element !== current) {
        element.removeAttribute("src");
        if (element.load) element.load();
        element.remove();
        clips.delete(key);
      }
    }
    if (at + 1 < count) clip(at + 1, false);
    if (at > 0) clip(at - 1, false);
  }

  /** Puts `element` on top once its first picture can be seen, and the old one away. */
  function reveal(element, mine) {
    const swap = () => {
      if (mine !== generation) return;
      for (const other of screen.children) {
        if (other !== element) {
          other.classList.remove("shown");
          if (other.pause) other.pause();
        }
      }
      element.classList.add("shown");
      visible = element;
      tidy();
    };
    if (element.tagName === "IMG") {
      if (element.complete) swap();
      else element.addEventListener("load", swap, { once: true });
      return;
    }
    // A held video does not play, so it has a picture to show once it has any data.
    if (run === "held") {
      if (element.readyState >= 2) swap();
      else element.addEventListener("loadeddata", swap, { once: true });
    } else {
      // At its first picture, where the browser says when that is shown; and in any case
      // once it is playing past its start, for a browser that does not say, or does not
      // while the page is out of sight.
      if (element.requestVideoFrameCallback) element.requestVideoFrameCallback(swap);
      const playing = () => {
        if (mine !== generation || visible === element) return;
        if (element.readyState >= 2 && element.currentTime > 0) swap();
        else setTimeout(playing, 100);
      };
      setTimeout(playing, 250);
    }
  }

  /** Loads slide `i`, forwards or backwards, from its start. */
  function load(i, back, held = false) {
    const mine = ++generation;
    at = i;
    reversed = back;
    endingLoop = false;
    run = held ? "held" : "playing";
    tell(null);
    const slide = slides[i];
    const element = clip(i, back);
    current = element;
    if (element.tagName === "IMG") {
      run = "ended";
      reveal(element, mine);
      update();
      if (movesOn(i) && !held) setTimeout(() => mine === generation && runEnded(true), 0);
      return;
    }
    element.loop = looping(i);
    const rate = back ? slide.reversed_playback_rate : slide.playback_rate;
    element.defaultPlaybackRate = rate || 1;
    element.playbackRate = rate || 1;
    if (element.currentTime !== 0) {
      try {
        element.currentTime = 0;
      } catch (_) {
        // Not loaded far enough to be told where to be; it starts at the start anyway.
      }
    }
    reveal(element, mine);
    if (run === "playing") start(element, mine);
    else element.pause();
    waiting(element.readyState < 3);
    update();
  }

  function start(element, mine) {
    const playing = element.play();
    if (!playing) return;
    playing.catch((error) => {
      if (mine !== generation) return;
      // Some browsers start nothing by themselves, a phone saving power among them: the
      // first slide then waits for a tap.
      if (error && error.name === "NotAllowedError") {
        run = "held";
        $("#start").hidden = false;
        update();
      }
    });
  }

  let spinner = 0;
  function waiting(yes) {
    clearTimeout(spinner);
    if (yes) spinner = setTimeout(() => ($("#loading").hidden = false), 350);
    else $("#loading").hidden = true;
  }

  // ── What the keys do (the table of §6.6) ──────────────────────────────

  function runEnded(image = false) {
    if (run !== "playing" && !image) return;
    if (movesOn(at)) {
      if (reversed) return load(at, false);
      if (at + 1 < count) return load(at + 1, false);
    }
    run = "ended";
    update();
  }

  function next() {
    if (run === "held") return carryOn();
    if (run === "playing" && options.nextEndsLoops && looping(at) && !endingLoop) {
      // The round finishes, and the slide rests or moves on as its settings say.
      endingLoop = true;
      current.loop = false;
      return;
    }
    if (reversed) return load(at, false);
    if (at + 1 < count) load(at + 1, false);
  }

  function previous() {
    if (at > 0) load(at - 1, false);
  }

  function reverse() {
    if (at > 0) load(at - 1, true);
  }

  function replay() {
    load(at, reversed);
  }

  function hold() {
    if (run === "playing" && current.pause) {
      current.pause();
      run = "held";
      update();
    } else if (run === "held") {
      carryOn();
    }
  }

  function carryOn() {
    $("#start").hidden = true;
    run = "playing";
    if (current.play) start(current, generation);
    update();
  }

  function go(i) {
    const slide = Math.max(0, Math.min(count - 1, i));
    load(slide, false);
  }

  // ── What can be seen ──────────────────────────────────────────────────

  function update() {
    const slide = slides[at];
    body.classList.toggle("held", run === "held");
    $("#place span").textContent = `${at + 1} / ${count}`;
    $("#place").title = slide.title ? `${slide.title} · all slides` : "All slides";
    for (const [what, enabled] of [
      ["previous", at > 0],
      ["reverse", at > 0],
      ["next", reversed || at + 1 < count || run === "held"],
      // A slide that has played through is replayed with R, not set going again.
      ["hold", run !== "ended"],
    ]) {
      $(`[data-do="${what}"]`).disabled = !enabled;
    }
    $('[data-do="hold"]').setAttribute("aria-label", run === "held" ? "Carry on" : "Hold");
    const notes = $("#notes");
    notes.querySelector("h2").textContent = slide.title || `Slide ${at + 1}`;
    const text = notes.querySelector(".text");
    text.textContent = slide.notes || "No notes for this slide.";
    text.classList.toggle("empty", !slide.notes);
    for (const item of $("#list ol").children) {
      item.firstChild.setAttribute("aria-current", String(Number(item.dataset.slide) === at));
    }
    $("#said").textContent = `Slide ${at + 1} of ${count}${slide.title ? `: ${slide.title}` : ""}`;
    const place = `#${at + 1}`;
    if (location.hash !== place) history.replaceState(null, "", place);
  }

  function tell(problem) {
    const element = $("#problem");
    element.hidden = !problem;
    element.textContent = problem || "";
  }

  function toggle(panel, show) {
    const element = $(panel);
    const open = show === undefined ? element.hidden : show;
    for (const other of ["#notes", "#list", "#help"]) {
      if (other !== panel && open) $(other).hidden = true;
    }
    element.hidden = !open;
    if (open && panel === "#list") {
      const here = element.querySelector('[aria-current="true"]');
      if (here) here.scrollIntoView({ block: "nearest" });
    }
  }

  const root = document.documentElement;
  const canFill = Boolean(root.requestFullscreen || root.webkitRequestFullscreen);
  function fullscreen() {
    const filling = document.fullscreenElement || document.webkitFullscreenElement;
    if (filling) (document.exitFullscreen || document.webkitExitFullscreen).call(document);
    else if (canFill) {
      const asked = (root.requestFullscreen || root.webkitRequestFullscreen).call(root);
      if (asked && asked.catch) asked.catch(() => {});
    }
  }

  // The screen stays on while the slides are shown, where the browser allows it.
  let awake = null;
  async function stayAwake() {
    if (awake || !navigator.wakeLock || document.visibilityState !== "visible") return;
    try {
      awake = await navigator.wakeLock.request("screen");
      awake.addEventListener("release", () => (awake = null));
    } catch (_) {
      awake = null;
    }
  }
  document.addEventListener("visibilitychange", stayAwake);

  // The controls come up when the pointer moves or the screen is touched, and go again
  // when it rests. Keys do not bring them up: they are how a talk is given.
  let resting = 0;
  function stir() {
    body.classList.remove("idle");
    clearTimeout(resting);
    resting = setTimeout(() => {
      if (!$("#controls").matches(":hover")) body.classList.add("idle");
    }, 2600);
  }

  // ── Keys ──────────────────────────────────────────────────────────────

  const doing = {
    next,
    previous,
    reverse,
    replay,
    hold,
    list: () => toggle("#list"),
    notes: () => toggle("#notes"),
    help: () => toggle("#help"),
    fullscreen,
  };

  document.addEventListener("keydown", (event) => {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const onButton = event.target.closest && event.target.closest("button, a");
    if (onButton && (event.key === "Enter" || event.key === " ")) return;
    stayAwake();
    const key = event.key;
    let done = true;
    switch (key) {
      case "ArrowRight":
      case "PageDown":
      case "Enter":
        next();
        break;
      case "ArrowLeft":
      case "PageUp":
        previous();
        break;
      case " ":
      case "Spacebar":
        hold();
        break;
      case "Home":
        go(0);
        break;
      case "End":
        go(count - 1);
        break;
      case "Escape":
        for (const panel of ["#notes", "#list", "#help"]) $(panel).hidden = true;
        $("#blank").hidden = true;
        done = false;
        break;
      case "?":
      case "/":
        toggle("#help");
        break;
      default:
        switch (key.toLowerCase()) {
          case "v":
            reverse();
            break;
          case "r":
            replay();
            break;
          case "f":
            fullscreen();
            break;
          case "n":
            if (deck.notes) toggle("#notes");
            break;
          case "h":
            body.classList.toggle("no-pointer");
            break;
          case "b":
          case ".":
            $("#blank").hidden = !$("#blank").hidden;
            break;
          case "q":
            if (document.fullscreenElement || document.webkitFullscreenElement) fullscreen();
            break;
          default:
            done = false;
        }
    }
    if (done) event.preventDefault();
  });

  // ── Pointer and touch ─────────────────────────────────────────────────
  // A click is NEXT, as in Suit. On a touch screen the left third of the slide goes back
  // and the rest goes on; a swipe goes either way.

  let down = null;
  const stage = $("#stage");
  stage.addEventListener("pointerdown", (event) => {
    if (event.target.closest("button")) return;
    down = { x: event.clientX, y: event.clientY, type: event.pointerType, button: event.button };
  });
  stage.addEventListener("pointerup", (event) => {
    if (!down) return;
    const { x, y, type, button } = down;
    down = null;
    stayAwake();
    stir();
    const [dx, dy] = [event.clientX - x, event.clientY - y];
    const panelsOpen = ["#notes", "#list", "#help"].some((panel) => !$(panel).hidden);
    if (panelsOpen && Math.hypot(dx, dy) < 10) {
      for (const panel of ["#list", "#help"]) $(panel).hidden = true;
      return;
    }
    if (Math.abs(dx) > 50 && Math.abs(dx) > 1.5 * Math.abs(dy)) {
      return dx < 0 ? next() : previous();
    }
    if (Math.hypot(dx, dy) > 12) return;
    if (type === "mouse") {
      if (button === 0) next();
      return;
    }
    const third = stage.getBoundingClientRect().width / 3;
    return event.clientX < third ? previous() : next();
  });
  stage.addEventListener("pointercancel", () => (down = null));
  document.addEventListener("pointermove", (event) => {
    if (event.pointerType === "mouse") stir();
  });
  document.addEventListener("touchstart", stir, { passive: true });

  for (const button of document.querySelectorAll("#controls [data-do]")) {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      stayAwake();
      doing[button.dataset.do]();
      stir();
      // So that Space and Enter go on meaning what they mean for the slides.
      if (event.detail > 0) button.blur();
    });
  }
  $("#start").addEventListener("click", () => {
    stayAwake();
    carryOn();
  });

  // ── Setting up ────────────────────────────────────────────────────────

  body.classList.toggle("has-notes", Boolean(deck.notes));
  if (deck.pdf) {
    $("#pdf").href = deck.pdf;
    $("#pdf").hidden = false;
  }
  if (!canFill) $("#full").hidden = true;

  const list = $("#list ol");
  slides.forEach((slide, i) => {
    const item = document.createElement("li");
    item.dataset.slide = String(i);
    const button = document.createElement("button");
    button.type = "button";
    const number = document.createElement("span");
    number.className = "number";
    number.textContent = String(i + 1);
    const title = document.createElement("span");
    title.textContent = slide.title || `Slide ${i + 1}`;
    button.append(number, title);
    button.addEventListener("click", () => {
      $("#list").hidden = true;
      go(i);
    });
    item.append(button);
    list.append(item);
  });

  // A slide named in the address (…#3) is where to begin; editing it moves there.
  const named = () => {
    const number = parseInt(location.hash.replace(/^#\/?/, ""), 10);
    return Number.isFinite(number) ? Math.max(1, Math.min(count, number)) - 1 : null;
  };
  window.addEventListener("hashchange", () => {
    const slide = named();
    if (slide !== null && slide !== at) go(slide);
  });

  stir();
  load(named() ?? 0, false, options.startPaused);
})();
