//! The window driven the way a person drives it: clicks, keys and drags, with no display and
//! no Manim. Where the canvas needs a picture of the scene, the test supplies one.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use eframe::egui::{
    self, Key, Modifiers, ViewportCommand, ViewportEvent, ViewportId, accesskit::Role,
};
use egui_kittest::{
    Harness,
    kittest::{By, Queryable},
};
use suit_codegen::pretty::Pretty;
use suit_core::{Anim, Command, Expr, Object, Time, VarId};
use suit_engine::Session;
use suit_render::{Bounds, Clip, Drawn, Outline, Snapshot, Snapshots};
use suit_ui::SuitApp;

type Window = Harness<'static, SuitApp>;

fn open(session: Session) -> Window {
    let mut window = Harness::builder()
        .with_size(egui::vec2(1480.0, 920.0))
        .with_max_steps(40)
        .build_ui_state(
            |ui, app: &mut SuitApp| app.show(ui),
            SuitApp::new(session, None),
        );
    window.run_ok();
    window
}

fn press(window: &mut Window, button: &str) {
    window.get_by_role_and_label(Role::Button, button).click();
    window.run_ok();
}

fn headlines(window: &Window) -> Vec<String> {
    let session = &window.state().session;
    let pretty = Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    session
        .steps()
        .iter()
        .map(|node| pretty.headline(&node.item))
        .collect()
}

fn text_of_step(window: &Window, step: usize) -> String {
    let session = &window.state().session;
    Pretty {
        deck: &session.deck,
        sig: &session.sig,
    }
    .command(&session.steps()[step].item)
}

#[test]
fn an_object_from_the_bar_lands_on_the_slide() {
    let mut window = open(Session::blank());
    press(&mut window, "Circle");
    assert_eq!(headlines(&window), ["New circle (Circle)", "Create circle"]);
    assert_eq!(window.state().session.step, Some(1));
    assert!(
        window.state().picked.is_some(),
        "the new object is the one further steps are about"
    );

    press(&mut window, "Formula");
    assert_eq!(
        headlines(&window)[2..],
        ["New formula (MathTex)", "Write formula"]
    );
    assert!(window.state().session.checked.diagnostics.is_empty());
}

#[test]
fn steps_are_selected_deleted_and_brought_back() {
    let mut window = open(Session::blank());
    press(&mut window, "Square");
    press(&mut window, "New square (Square)");
    assert_eq!(window.state().session.step, Some(0));

    window.key_press(Key::Delete);
    window.run_ok();
    assert_eq!(headlines(&window), ["Create square"]);
    // Creating a square that was never made is a problem, and the window says so.
    assert!(
        window
            .query_by_label_contains("does not exist yet")
            .is_some()
    );

    press(&mut window, "Undo");
    assert_eq!(headlines(&window), ["New square (Square)", "Create square"]);
    press(&mut window, "Redo");
    assert_eq!(headlines(&window), ["Create square"]);
}

#[test]
fn arrow_keys_walk_through_the_steps() {
    let mut window = open(Session::blank());
    press(&mut window, "Dot");
    for expected in [Some(0), None, None] {
        window.key_press(Key::ArrowUp);
        window.run_ok();
        assert_eq!(window.state().session.step, expected);
    }
    window.key_press(Key::ArrowDown);
    window.run_ok();
    assert_eq!(window.state().session.step, Some(0));
}

#[test]
fn steps_are_added_and_reordered_with_the_buttons() {
    let mut window = open(Session::blank());
    press(&mut window, "Star");
    press(&mut window, "Pause");
    press(&mut window, "Animate");
    assert_eq!(headlines(&window)[2..], ["Pause 1 s", "Indicate star"]);

    press(&mut window, "Move step earlier");
    assert_eq!(headlines(&window)[2..], ["Indicate star", "Pause 1 s"]);
    assert_eq!(window.state().session.step, Some(2));
    press(&mut window, "Hide");
    press(&mut window, "Show");
    assert_eq!(headlines(&window)[3..5], ["Hide star", "Show star"]);
    assert!(window.state().session.checked.diagnostics.is_empty());

    press(&mut window, "Delete");
    assert_eq!(headlines(&window)[3..], ["Hide star", "Pause 1 s"]);
}

#[test]
fn the_step_buttons_wait_for_an_object() {
    let mut window = open(Session::blank());
    window
        .get_by_role_and_label(Role::Button, "Animate")
        .click();
    window.run_ok();
    assert_eq!(
        headlines(&window),
        [] as [&str; 0],
        "there is nothing to animate yet"
    );
    press(&mut window, "Pause");
    assert_eq!(headlines(&window), ["Pause 1 s"]);
}

#[test]
fn slides_are_added_configured_and_moved() {
    let mut window = open(Session::blank());
    press(&mut window, "Add slide");
    assert_eq!(window.state().session.deck.slides.len(), 2);
    assert_eq!(window.state().session.slide, 1);

    window
        .get_by_role_and_label(Role::CheckBox, "Loop until I move on")
        .click();
    window.run_ok();
    assert!(window.state().session.deck.slides[1].config.looping);

    press(&mut window, "Move slide earlier");
    let titles: Vec<&str> = window
        .state()
        .session
        .deck
        .slides
        .iter()
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(titles, ["Slide 2", "Slide 1"]);
    press(&mut window, "Slide 2: Slide 1");
    assert_eq!(window.state().session.slide, 1);

    press(&mut window, "Delete this slide");
    assert_eq!(window.state().session.deck.slides.len(), 1);
}

#[test]
fn an_animation_can_be_changed_into_another() {
    let mut window = open(Session::blank());
    press(&mut window, "Text");
    assert_eq!(text_of_step(&window, 1), "play(Write(text))");

    // The first drop-down of the step chooses the animation.
    window.get_by_value("Write").click();
    window.run_ok();
    press(&mut window, "FadeIn");
    // FadeIn calls its object `mobject` where Write says `vmobject`; the object carries over.
    assert_eq!(text_of_step(&window, 1), "play(FadeIn(text))");
    let Command::Play { anims } = &window.state().session.steps()[1].item else {
        panic!()
    };
    let Anim::Prim { args, .. } = &anims[0].item else {
        panic!()
    };
    assert_eq!(args[0].0.as_str(), "mobject");
    assert!(window.state().session.checked.diagnostics.is_empty());
}

// ── The canvas ───────────────────────────────────────────────────────────────

/// A deck with one circle, and a picture of it as Manim would report: made at the origin,
/// two units across, and after the last step centred on `now`.
fn with_a_circle(now: [f32; 2]) -> (Window, VarId) {
    let mut session = Session::blank();
    let var = session.add_object("Circle").unwrap();
    let at = |[x, y]: [f32; 2], on: bool| {
        let rect = [x - 1.0, y - 1.0, x + 1.0, y + 1.0];
        let square = [
            [rect[0], rect[1]],
            [rect[2], rect[1]],
            [rect[2], rect[3]],
            [rect[0], rect[3]],
        ];
        // Four straight sides, each written as a cubic curve.
        let mut points = vec![square[0]];
        for side in 0..4 {
            let (from, to) = (square[side], square[(side + 1) % 4]);
            points.extend([from, to, to]);
        }
        let outline = Outline {
            points,
            closed: true,
        };
        let drawn = Drawn {
            owners: vec![(var, vec![])],
            paths: vec![outline],
            fill: [1.0; 4],
            stroke: [1.0; 4],
            stroke_width: 4.0,
            picture: None,
        };
        (
            if on { vec![drawn] } else { Vec::new() },
            BTreeMap::from([(var, Bounds { rect, on })]),
        )
    };
    let ids: Vec<_> = session.steps().iter().map(|node| node.id).collect();
    let snapshot = |node, (drawn, boxes)| Snapshot {
        slide: 0,
        node,
        drawn,
        boxes,
    };
    session.snapshots = Snapshots {
        steps: vec![
            snapshot(None, (Vec::new(), BTreeMap::new())),
            snapshot(Some(ids[0]), at([0.0, 0.0], false)),
            snapshot(Some(ids[1]), at(now, true)),
        ],
        ..Snapshots::default()
    };
    (open(session), var)
}

/// Where a scene point is on screen, and how many scene units one screen point is.
fn on_canvas(window: &Window, [x, y]: [f32; 2]) -> (egui::Pos2, f32) {
    let canvas = window.get_by_label("Canvas").rect().shrink(18.0);
    let height = match canvas.width() / canvas.height() > 16.0 / 9.0 {
        true => canvas.height(),
        false => canvas.width() * 9.0 / 16.0,
    };
    let scale = 8.0 / height;
    (canvas.center() + egui::vec2(x / scale, -y / scale), scale)
}

fn drag(window: &mut Window, from: egui::Pos2, by: egui::Vec2) {
    window.hover_at(from);
    window.step();
    window.drag_at(from);
    window.step();
    for part in [0.3, 0.7, 1.0] {
        window.hover_at(from + by * part);
        window.step();
    }
    window.drop_at(from + by);
    window.run_ok();
}

/// The point the step moves its object to, if it is a `move_to` of some kind.
fn destination(window: &Window, step: usize) -> [f64; 2] {
    let call = match &window.state().session.steps()[step].item {
        Command::LetObject { value, .. } => match &value.item {
            Object::Chain { call, .. } => call,
            other => panic!("not placed: {other:?}"),
        },
        Command::Play { anims } => match &anims[0].item {
            Anim::Animate { calls, .. } => &calls[0],
            other => panic!("not a change: {other:?}"),
        },
        Command::Apply { call, .. } => call,
        other => panic!("not a move: {other:?}"),
    };
    assert_eq!(call.method.as_str(), "move_to");
    let suit_core::Arg::Expr(Expr::Vec3(parts)) = &call.args[0].1 else {
        panic!()
    };
    let number = |part: &Expr| match part {
        Expr::Real(number) => *number,
        other => panic!("not a number: {other:?}"),
    };
    [number(&parts[0]), number(&parts[1])]
}

#[test]
fn clicking_the_canvas_picks_what_is_there() {
    let (mut window, var) = with_a_circle([0.0, 0.0]);
    window.state_mut().picked = None;
    let (centre, scale) = on_canvas(&window, [0.0, 0.0]);
    window.get_by_label("Canvas").click();
    window.run_ok();
    assert_eq!(
        window.state().picked,
        Some(var),
        "the canvas's centre is inside the circle"
    );

    // Three units to the right there is nothing.
    window.hover_at(centre + egui::vec2(3.0 / scale, 0.0));
    window.step();
    window.drag_at(centre + egui::vec2(3.0 / scale, 0.0));
    window.step();
    window.drop_at(centre + egui::vec2(3.0 / scale, 0.0));
    window.run_ok();
    assert_eq!(window.state().picked, None);
}

#[test]
fn dragging_an_object_that_has_not_moved_says_where_it_starts() {
    let (mut window, _) = with_a_circle([0.0, 0.0]);
    let (centre, scale) = on_canvas(&window, [0.0, 0.0]);
    drag(&mut window, centre, egui::vec2(2.0 / scale, -1.0 / scale));

    assert_eq!(
        headlines(&window),
        ["New circle (Circle)", "Create circle"],
        "no step was added"
    );
    let [x, y] = destination(&window, 0);
    assert!(
        (x - 2.0).abs() < 0.02 && (y - 1.0).abs() < 0.02,
        "moved to ({x}, {y})"
    );
    assert!(
        window.query_by_label("Glide there").is_none(),
        "and nothing was asked"
    );

    press(&mut window, "Undo");
    assert_eq!(text_of_step(&window, 0), "circle = Circle()");
}

#[test]
fn dragging_an_object_that_has_moved_asks_what_is_meant() {
    // Something moved the circle two units to the right after it was made.
    let (mut window, _) = with_a_circle([2.0, 0.0]);
    let (grab, scale) = on_canvas(&window, [2.0, 0.0]);
    drag(&mut window, grab, egui::vec2(-3.0 / scale, -2.0 / scale));
    assert_eq!(
        headlines(&window).len(),
        2,
        "nothing changes until the question is answered"
    );

    press(&mut window, "Glide there");
    assert_eq!(headlines(&window)[2..], ["Change circle: move_to"]);
    let [x, y] = destination(&window, 2);
    assert!(
        (x + 1.0).abs() < 0.02 && (y - 2.0).abs() < 0.02,
        "moved to ({x}, {y})"
    );
    let session = &window.state().session;
    assert_eq!(
        session.checked.notes[&session.steps()[2].id].frames,
        60,
        "gliding takes a second"
    );
}

#[test]
fn a_drag_can_be_called_off() {
    let (mut window, _) = with_a_circle([2.0, 0.0]);
    let (grab, scale) = on_canvas(&window, [2.0, 0.0]);
    drag(&mut window, grab, egui::vec2(1.0 / scale, 0.0));
    press(&mut window, "Cancel");
    assert_eq!(headlines(&window), ["New circle (Circle)", "Create circle"]);
    assert_eq!(window.state().session.can_undo(), Some("Add object"));
}

// ── Playing ──────────────────────────────────────────────────────────────────

/// What Manim would report for a slide of `frames` frames in which nothing changes.
fn recording(frames: usize) -> Clip {
    let dot = Outline {
        points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        closed: false,
    };
    let drawn = Drawn {
        owners: Vec::new(),
        paths: vec![dot],
        fill: [1.0; 4],
        stroke: [1.0; 4],
        stroke_width: 4.0,
        picture: None,
    };
    Clip {
        pool: vec![drawn],
        frames: vec![vec![0]; frames],
        marks: Vec::new(),
        error: None,
    }
}

/// A window whose slide has been recorded as `frames` frames at 60 fps. The harness moves
/// time on a quarter of a second, fifteen frames, with every frame it draws.
fn with_a_recording(looping: bool, frames: usize) -> Window {
    let (mut window, _) = with_a_circle([0.0, 0.0]);
    let session = &mut window.state_mut().session;
    session.edit_slide(|slide| slide.config.looping = looping);
    session.install_recording(recording(frames));
    window.run_ok();
    window
}

fn play(window: &mut Window) {
    window.get_by_role_and_label(Role::Button, "Play").click();
    window.step();
}

#[test]
fn a_slide_plays_through_and_rests_on_its_last_frame() {
    let mut window = with_a_recording(false, 30);
    assert_eq!(
        window.state().playing_frame(),
        None,
        "nothing plays until asked"
    );

    play(&mut window);
    assert_eq!(window.state().playing_frame(), Some(0));
    assert!(window.state().is_playing());
    window.step();
    assert_eq!(window.state().playing_frame(), Some(15));
    window.step();
    assert_eq!(
        window.state().playing_frame(),
        Some(29),
        "it stops on the last frame"
    );
    assert!(!window.state().is_playing());
    window.step();
    assert_eq!(window.state().playing_frame(), Some(29));

    // Playing again starts from the top.
    play(&mut window);
    assert_eq!(window.state().playing_frame(), Some(0));
}

#[test]
fn space_pauses_and_carries_on() {
    let mut window = with_a_recording(false, 90);
    // A key press is two frames to the harness: the key going down, which starts the
    // slide, and the key coming up a quarter of a second later.
    window.key_press(Key::Space);
    window.step();
    assert_eq!(window.state().playing_frame(), Some(15));

    window.key_press(Key::Space);
    window.step();
    assert!(!window.state().is_playing());
    assert_eq!(
        window.state().playing_frame(),
        Some(15),
        "paused before the clock moved it on"
    );
    window.step();
    assert_eq!(
        window.state().playing_frame(),
        Some(15),
        "time passes, the frame stays"
    );

    // It carries on from where it paused, not from where the clock has got to meanwhile.
    window.key_press(Key::Space);
    window.step();
    assert_eq!(window.state().playing_frame(), Some(30));
    assert!(window.state().is_playing());
}

#[test]
fn a_looping_slide_starts_over() {
    let mut window = with_a_recording(true, 30);
    play(&mut window);
    let mut seen = vec![window.state().playing_frame().unwrap()];
    for _ in 0..4 {
        window.step();
        seen.push(window.state().playing_frame().unwrap());
    }
    assert_eq!(seen, [0, 15, 0, 15, 0]);
    assert!(window.state().is_playing());
}

#[test]
fn selecting_or_editing_goes_back_to_the_steps() {
    let mut window = with_a_recording(false, 30);
    play(&mut window);
    press(&mut window, "New circle (Circle)");
    assert_eq!(
        window.state().playing_frame(),
        None,
        "choosing a step ends the playback"
    );

    play(&mut window);
    assert!(
        window.state().playing_frame().is_some(),
        "the recording is still good"
    );
    press(&mut window, "Back to editing");
    assert_eq!(window.state().playing_frame(), None);

    // An edit makes the recording stale, so there is nothing left to play from.
    play(&mut window);
    press(&mut window, "Pause");
    assert!(window.state().session.clip().is_none());
    assert_eq!(window.state().playing_frame(), None);
}

// ── Presenting ───────────────────────────────────────────────────────────────

/// A deck of three slides, the first two recorded as thirty frames each, the third not
/// recorded yet. Presenting starts with the first.
fn presenting_three_slides() -> Window {
    let mut session = Session::blank();
    session.add_object("Circle");
    for _ in 0..2 {
        session.add_slide();
        session.insert_step(Command::Wait {
            duration: Time::from_secs(0.5),
        });
    }
    session.install_recording_for(0, recording(30));
    session.install_recording_for(1, recording(30));
    session.select(0, None);
    let mut window = open(session);
    window
        .get_by_role_and_label(Role::Button, "Present")
        .click();
    window.step();
    assert!(window.state().is_presenting());
    window
}

/// A key press is two frames to the harness, a quarter of a second apart: the key going
/// down, which is acted on, and the key coming up.
fn key(window: &mut Window, key: Key) -> Option<(usize, Option<usize>)> {
    window.key_press(key);
    window.step();
    window.state().presenting()
}

fn wait(window: &mut Window) -> Option<(usize, Option<usize>)> {
    window.step();
    window.state().presenting()
}

#[test]
fn presenting_plays_a_slide_and_waits_for_a_key() {
    let mut window = presenting_three_slides();
    assert_eq!(wait(&mut window), Some((0, Some(0))));
    assert_eq!(wait(&mut window), Some((0, Some(15))));
    assert_eq!(wait(&mut window), Some((0, Some(29))));
    assert_eq!(
        wait(&mut window),
        Some((0, Some(29))),
        "it rests on its last frame"
    );
    // None of the editor is there while presenting.
    assert!(window.query_by_label("Slides").is_none());

    assert_eq!(key(&mut window, Key::ArrowRight), Some((1, Some(15))));
    assert_eq!(
        key(&mut window, Key::ArrowLeft),
        Some((0, Some(15))),
        "previous starts that slide over"
    );
}

#[test]
fn reverse_rewinds_and_next_plays_it_forwards_again() {
    let mut window = presenting_three_slides();
    wait(&mut window);
    assert_eq!(key(&mut window, Key::ArrowRight), Some((1, Some(15))));
    // The slide before, backwards: thirty frames, of which fifteen are gone by key-up.
    assert_eq!(key(&mut window, Key::V), Some((0, Some(14))));
    assert_eq!(wait(&mut window), Some((0, Some(0))));
    assert_eq!(
        key(&mut window, Key::ArrowRight),
        Some((0, Some(15))),
        "the same slide, forwards"
    );
    assert_eq!(
        key(&mut window, Key::R),
        Some((0, Some(15))),
        "replay starts it over"
    );
}

#[test]
fn space_holds_the_slide_and_next_lets_it_go() {
    let mut window = presenting_three_slides();
    wait(&mut window);
    assert_eq!(
        key(&mut window, Key::Space),
        Some((0, Some(0))),
        "held where it was"
    );
    assert_eq!(wait(&mut window), Some((0, Some(0))));
    // While held, next means carry on, not the next slide.
    assert_eq!(key(&mut window, Key::ArrowRight), Some((0, Some(15))));
}

#[test]
fn a_click_is_next_and_an_unrecorded_slide_is_waited_for() {
    let mut window = presenting_three_slides();
    wait(&mut window);
    window.get_by_label("Presentation").click();
    window.step();
    assert_eq!(window.state().presenting().map(|(slide, _)| slide), Some(1));
    // The third slide has no recording yet: nothing to show, and the clock does not run.
    assert_eq!(key(&mut window, Key::ArrowRight), Some((2, None)));
    assert_eq!(wait(&mut window), Some((2, None)));
    window
        .state_mut()
        .session
        .install_recording_for(2, recording(30));
    assert_eq!(
        wait(&mut window),
        Some((2, Some(0))),
        "it starts when the recording arrives"
    );
    assert_eq!(
        key(&mut window, Key::ArrowRight),
        Some((2, Some(29))),
        "there is no slide after the last"
    );
}

#[test]
fn leaving_returns_to_the_slide_that_was_showing() {
    let mut window = presenting_three_slides();
    wait(&mut window);
    key(&mut window, Key::ArrowRight);
    window.key_press(Key::Escape);
    window.run_ok();
    assert!(!window.state().is_presenting());
    assert_eq!(window.state().session.slide, 1);
    assert!(
        window.query_by_label("Slides").is_some(),
        "the editor is back"
    );

    // F5 starts from the selected slide.
    window.key_press(Key::F5);
    window.step();
    assert_eq!(wait(&mut window).map(|(slide, _)| slide), Some(1));
}

// ── Files and leaving ────────────────────────────────────────────────────────

/// An empty folder that belongs to one test.
fn scratch(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    folder.canonicalize().unwrap()
}

/// Replaces what is in the text field called `label`.
fn retype(window: &mut Window, label: &str, text: &str) {
    window.get_by_label(label).click();
    window.run_ok();
    window.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    window.get_by_label(label).type_text(text);
    window.run_ok();
}

fn menu(window: &mut Window, menu: &str, item: &str) {
    press(window, menu);
    press(window, item);
}

#[test]
fn a_deck_is_saved_and_opened_by_walking_through_folders() {
    let folder = scratch("browsing");
    fs::create_dir_all(folder.join("talks")).unwrap();
    fs::write(folder.join("notes.txt"), "not a deck").unwrap();
    let mut session = Session::blank();
    session.save_as(&folder.join("first.suit.json")).unwrap();
    let mut window = open(session);
    press(&mut window, "Circle");
    assert!(window.state().session.is_dirty());

    // Saving under another name, one folder down.
    menu(&mut window, "File", "Save as…");
    assert!(
        window.query_by_label("notes.txt").is_none(),
        "only decks and folders are listed"
    );
    press(&mut window, "talks");
    retype(&mut window, "File name", "second");
    press(&mut window, "Save");
    let second = folder.join("talks/second.suit.json");
    assert!(second.is_file(), "the ending was added");
    assert_eq!(
        window.state().session.path.as_deref(),
        Some(second.as_path())
    );
    assert!(!window.state().session.is_dirty());

    // Saving over a file takes saying so twice.
    press(&mut window, "Square");
    menu(&mut window, "File", "Save as…");
    press(&mut window, "Save");
    assert!(window.query_by_label_contains("already there").is_some());
    assert!(window.state().session.is_dirty(), "nothing was written yet");
    press(&mut window, "Save");
    assert!(!window.state().session.is_dirty());

    // Back up to the first deck, by walking there.
    menu(&mut window, "File", "Open…");
    press(&mut window, "↑ Up");
    window.get_by_label("first.suit.json").click();
    window.run_ok();
    press(&mut window, "Open");
    let first = folder.join("first.suit.json");
    assert_eq!(
        window.state().session.path.as_deref(),
        Some(first.as_path())
    );
    assert!(
        headlines(&window).is_empty(),
        "the deck that was saved empty"
    );

    // A file that is not a deck is refused where it is chosen.
    fs::write(folder.join("broken.suit.json"), "{").unwrap();
    menu(&mut window, "File", "Open…");
    window.get_by_label("broken.suit.json").click();
    window.run_ok();
    press(&mut window, "Open");
    assert!(window.query_by_label_contains("is not a deck").is_some());
    press(&mut window, "Cancel");
    assert_eq!(
        window.state().session.path.as_deref(),
        Some(first.as_path())
    );
}

/// What the window asked of the system in the last frame.
fn asked_of_the_window(window: &Window) -> Vec<ViewportCommand> {
    let output = window.output().viewport_output.get(&ViewportId::ROOT);
    output.map_or(Vec::new(), |output| output.commands.clone())
}

fn close_the_window(window: &mut Window) {
    let viewports = &mut window.input_mut().viewports;
    let root = viewports.entry(ViewportId::ROOT).or_default();
    root.events.push(ViewportEvent::Close);
    window.step();
}

#[test]
fn closing_with_unsaved_changes_asks_first() {
    let folder = scratch("closing");
    let mut window = open(Session::blank());
    close_the_window(&mut window);
    assert!(
        !asked_of_the_window(&window).contains(&ViewportCommand::CancelClose),
        "nothing to lose, nothing to ask"
    );

    press(&mut window, "Circle");
    close_the_window(&mut window);
    assert!(asked_of_the_window(&window).contains(&ViewportCommand::CancelClose));
    window.run_ok();
    assert!(window.query_by_label_contains("not saved").is_some());
    press(&mut window, "Keep editing");
    assert!(window.query_by_label_contains("not saved").is_none());

    // Saving first: the deck has no file yet, so one is asked for, and then the window goes.
    close_the_window(&mut window);
    window.run_ok();
    press(&mut window, "Save them");
    retype(&mut window, "Folder", &folder.display().to_string());
    window.key_press(Key::Enter);
    window.run_ok();
    retype(&mut window, "File name", "kept");
    window.get_by_role_and_label(Role::Button, "Save").click();
    // A click is three frames; the window is told to close in one of them.
    let mut asked = Vec::new();
    for _ in 0..3 {
        window.step();
        asked.extend(asked_of_the_window(&window));
    }
    assert!(folder.join("kept.suit.json").is_file());
    assert!(asked.contains(&ViewportCommand::Close));
}

#[test]
fn the_notes_say_what_to_say_and_what_comes_next() {
    let mut session = Session::sample();
    session.install_recording_for(0, recording(30));
    session.install_recording_for(1, recording(30));
    let mut window = open(session);
    press(&mut window, "Present");
    assert!(window.query_by_label_contains("Welcome everyone").is_none());

    window.key_press(Key::N);
    window.step();
    assert!(window.query_by_label_contains("Welcome everyone").is_some());
    assert!(window.query_by_label("Slide 1 of 6").is_some());
    assert!(window.query_by_label("Next: Shapes").is_some());

    // The keys still belong to the presentation, once.
    assert_eq!(
        key(&mut window, Key::ArrowRight).map(|(slide, _)| slide),
        Some(1)
    );
    assert!(window.query_by_label("Slide 2 of 6").is_some());
    assert!(window.query_by_label("No notes for this slide.").is_some());
    assert!(window.query_by_label("Next: Dots").is_some());

    window.key_press(Key::N);
    window.step();
    assert!(
        window.query_by_label("Slide 2 of 6").is_none(),
        "N puts them away again"
    );
}

#[test]
fn how_a_presentation_runs_is_chosen_next_to_the_button() {
    let mut session = Session::blank();
    session.add_object("Circle");
    session.install_recording(recording(30));
    let mut window = open(session);
    press(&mut window, "How to present");
    for choice in [
        "Wait for a key before the first slide",
        "Leave after the last slide",
    ] {
        window.get_by_label(choice).click();
        window.run_ok();
    }
    window.key_press(Key::Escape);
    window.run_ok();
    assert!(window.state().present_options.start_paused);

    press(&mut window, "Present");
    assert_eq!(wait(&mut window), Some((0, Some(0))));
    assert_eq!(wait(&mut window), Some((0, Some(0))), "it waits for a key");
    assert_eq!(key(&mut window, Key::ArrowRight), Some((0, Some(15))));
    assert_eq!(wait(&mut window), Some((0, Some(29))));
    // Past the only slide: back in the editor.
    window.key_press(Key::ArrowRight);
    window.run_ok();
    assert!(!window.state().is_presenting());
}

// ── Building blocks ──────────────────────────────────────────────────────────

/// Replaces what is in the text field that now says `shown`.
fn rewrite(window: &mut Window, shown: &str, text: &str) {
    let field = |window: &Window| {
        let says = By::new().role(Role::TextInput).value(shown);
        window.get(says).rect().center()
    };
    let at = field(window);
    window.hover_at(at);
    window.drag_at(at);
    window.drop_at(at);
    window.run_ok();
    window.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    window.event(egui::Event::Text(text.to_owned()));
    window.run_ok();
}

#[test]
fn a_setting_can_be_a_formula() {
    let mut window = open(Session::blank());
    menu(&mut window, "More objects", "Named value");
    assert_eq!(headlines(&window), ["Value value"]);
    rewrite(&mut window, "1.0", "3 / 4");
    assert_eq!(text_of_step(&window, 0), "value = 3 / 4");

    press(&mut window, "Circle");
    press(&mut window, "New circle (Circle)");
    menu(&mut window, "Add setting", "radius");
    assert_eq!(text_of_step(&window, 1), "circle = Circle(radius = 1.0)");
    // A plain number has a plain field; fx turns it into a formula.
    press(&mut window, "fx");
    rewrite(&mut window, "1.0", "2 * value");
    assert_eq!(
        text_of_step(&window, 1),
        "circle = Circle(radius = 2 * value)"
    );
    assert_eq!(window.state().session.checked.diagnostics, []);

    // What is not a formula is said to be so, and changes nothing.
    rewrite(&mut window, "2 * value", "2 * valu");
    assert!(
        window
            .query_by_label_contains("nothing called “valu”")
            .is_some()
    );
    assert_eq!(
        text_of_step(&window, 1),
        "circle = Circle(radius = 2 * value)"
    );
    // What is one but makes no sense goes to the checker, like everything else.
    rewrite(&mut window, "2 * valu", "\"big\"");
    assert_eq!(
        text_of_step(&window, 1),
        "circle = Circle(radius = \"big\")"
    );
    assert_eq!(window.state().session.checked.diagnostics.len(), 1);
}

#[test]
fn a_graph_is_made_and_its_rule_rewritten() {
    let mut window = open(Session::blank());
    menu(&mut window, "More objects", "Graph of a function");
    assert_eq!(headlines(&window), ["New graph (Plot)", "Create graph"]);
    assert_eq!(text_of_step(&window, 0), "graph = Plot(x ↦ sin(x))");

    // Its rule is at hand without going to the step that makes it.
    rewrite(&mut window, "sin(x)", "log(x)");
    assert_eq!(text_of_step(&window, 0), "graph = Plot(x ↦ log(x))");
    assert_eq!(window.state().session.checked.diagnostics, []);

    // What the graph found when Manim made it: where the rule has a value, and that it
    // runs off at zero. Left to itself it drew there, so there is nothing to put right.
    let found = |window: &mut Window, asked: [f64; 2], chosen: bool| {
        let session = &mut window.state_mut().session;
        let remark = suit_render::Remark {
            node: Some(session.steps()[0].id),
            var: session.objects()[0].var,
            what: "plot".to_owned(),
            asked: vec![asked],
            pieces: vec![[0.0, asked[1]]],
            chosen,
            missing: true,
            vertical: vec![0.0],
            ..Default::default()
        };
        session.install_snapshots(Snapshots {
            remarks: vec![remark],
            ..Snapshots::default()
        });
        window.run_ok();
    };
    found(&mut window, [-7.11, 7.11], false);
    assert!(
        window
            .query_by_label("Drawn on [0, 7.11]: where the rule has a value near the picture.")
            .is_some()
    );
    assert!(window.query_by_label("found: x = 0").is_some());
    assert!(window.query_by_label_contains("ready to export").is_some());

    // The box may be below the edge of the panel, so it is pressed without being pointed at.
    window.get_by_label("Show asymptotes").click_accesskit();
    window.run_ok();
    assert_eq!(
        text_of_step(&window, 0),
        "graph = Plot(x ↦ log(x), asymptotes = true)"
    );

    // Where to draw can be said by hand, several stretches at once.
    rewrite(&mut window, "", "[-2; -1] union [1; 2]");
    assert_eq!(
        text_of_step(&window, 0),
        "graph = Plot(x ↦ log(x), asymptotes = true, on = (-2, -1), on = (1, 2))"
    );
    assert_eq!(window.state().session.checked.diagnostics, []);
    rewrite(&mut window, "[-2; -1] union [1; 2]", "[-7, 7");
    assert!(
        window
            .query_by_label("This stretch has no closing ].")
            .is_some()
    );
    rewrite(&mut window, "[-7, 7", "[-7, 7]");
    assert_eq!(
        text_of_step(&window, 0),
        "graph = Plot(x ↦ log(x), asymptotes = true, on = (-7, 7))"
    );

    // Told to draw where the rule has no value, the editor says so, in so many words.
    found(&mut window, [-7.0, 7.0], true);
    assert!(window.query_by_label("Problems (1)").is_some());
    assert!(
        window
            .query_by_label("The function “log(x)” is not defined on all of [-7, 7].")
            .is_some()
    );
    assert!(
        window
            .query_by_label("It is drawn where it has a value: [0, 7].")
            .is_some()
    );

    // What goes around another object says so when there is none.
    let mut empty = open(Session::blank());
    menu(&mut empty, "More objects", "Brace under…");
    assert!(headlines(&empty).is_empty());
    assert!(empty.query_by_label_contains("needs an object").is_some());
}

#[test]
fn an_object_keeps_doing_something() {
    let mut window = open(Session::blank());
    press(&mut window, "Circle");
    press(&mut window, "Dot");
    menu(&mut window, "More steps", "Keep doing…");
    assert_eq!(text_of_step(&window, 4), "always dot.next_to(circle, DOWN)");
    assert_eq!(window.state().session.checked.diagnostics, []);

    // With the time passed, it can also keep moving.
    window
        .get_by_label("by an amount that depends on the time passed")
        .click();
    window.run_ok();
    window.get_by_value("next to").click();
    window.run_ok();
    press(&mut window, "rotate");
    press(&mut window, "fx");
    rewrite(&mut window, "1.0", "90 * DEGREES * dt");
    assert_eq!(
        text_of_step(&window, 4),
        "every dt. dot.rotate(90 * DEGREES * dt)"
    );
    assert_eq!(window.state().session.checked.diagnostics, []);

    menu(&mut window, "More steps", "Stop keeping on");
    assert_eq!(headlines(&window)[5], "dot stops updating");
}

#[test]
fn objects_are_grouped_and_copied_from_the_steps() {
    let mut window = open(Session::blank());
    press(&mut window, "Circle");
    press(&mut window, "Square");
    menu(&mut window, "More steps", "Group…");
    // The square was picked last, so it is ticked already; one object is not a group.
    assert!(
        window
            .query_by_label_contains("Tick what belongs together")
            .is_some()
    );
    window.get_by_label("circle").click();
    window.run_ok();
    press(&mut window, "Group these");
    assert_eq!(headlines(&window)[4], "New group (group of 2)");
    assert_eq!(text_of_step(&window, 4), "group = vgroup(circle, square)");
    assert_eq!(window.state().session.checked.diagnostics, []);

    // The group is what further steps are about.
    press(&mut window, "Animate");
    assert_eq!(headlines(&window)[5], "Indicate group");
    menu(&mut window, "More steps", "Duplicate");
    assert_eq!(
        headlines(&window)[6..],
        ["New group copy (copy)", "Create group copy"]
    );
    assert_eq!(window.state().session.checked.diagnostics, []);

    // Its members are spoken for.
    menu(&mut window, "More steps", "Group…");
    assert!(window.query_by_label("circle").is_none());
    press(&mut window, "Cancel");
}

#[test]
fn the_animations_on_offer_suit_the_object() {
    let mut window = open(Session::blank());
    press(&mut window, "Circle");
    press(&mut window, "Other animations");
    assert!(window.query_by_label("Take it away").is_some());
    assert!(
        window
            .query_by_role_and_label(Role::Button, "Create")
            .is_none(),
        "it is on the slide already"
    );
    press(&mut window, "FadeOut");
    assert_eq!(headlines(&window)[2], "FadeOut circle");

    // Gone from the slide, it can only be brought back.
    press(&mut window, "Other animations");
    assert!(window.query_by_label("Take it away").is_none());
    press(&mut window, "GrowFromCenter");
    assert_eq!(headlines(&window)[3], "GrowFromCenter circle");
    assert_eq!(window.state().session.checked.diagnostics, []);
}

#[test]
fn shift_picks_several_objects_to_group() {
    // A circle three units left of the middle and a square in the middle, as Manim would
    // report them once both are on the slide.
    let mut session = Session::blank();
    let circle = session.add_object("Circle").unwrap();
    let square = session.add_object("Square").unwrap();
    let shape = |var: VarId, x: f32| {
        let rect = [x - 1.0, -1.0, x + 1.0, 1.0];
        let corners = [
            [rect[0], -1.0],
            [rect[2], -1.0],
            [rect[2], 1.0],
            [rect[0], 1.0],
        ];
        let mut points = vec![corners[0]];
        for side in 0..4 {
            let (from, to) = (corners[side], corners[(side + 1) % 4]);
            points.extend([from, to, to]);
        }
        let drawn = Drawn {
            owners: vec![(var, vec![])],
            paths: vec![Outline {
                points,
                closed: true,
            }],
            fill: [1.0; 4],
            stroke: [1.0; 4],
            stroke_width: 4.0,
            picture: None,
        };
        (drawn, (var, Bounds { rect, on: true }))
    };
    let (round, round_box) = shape(circle, -3.0);
    let (straight, straight_box) = shape(square, 0.0);
    let last = session.steps().last().map(|node| node.id);
    session.snapshots = Snapshots {
        steps: vec![Snapshot {
            slide: 0,
            node: last,
            drawn: vec![round, straight],
            boxes: BTreeMap::from([round_box, straight_box]),
        }],
        ..Snapshots::default()
    };
    let mut window = open(session);
    window.state_mut().picked = None;

    let (centre, scale) = on_canvas(&window, [0.0, 0.0]);
    let on_circle = centre - egui::vec2(3.0 / scale, 0.0);
    window.hover_at(on_circle);
    window.drag_at(on_circle);
    window.drop_at(on_circle);
    window.run_ok();
    assert_eq!(window.state().picked, Some(circle));
    assert!(window.query_by_label_contains("Group these").is_none());

    // The square, with Shift: both are picked now, and can be grouped from right there.
    window
        .get_by_label("Canvas")
        .click_modifiers(Modifiers::SHIFT);
    window.run_ok();
    assert_eq!(window.state().picked, Some(circle));
    assert_eq!(window.state().picked_too, [square]);
    press(&mut window, "Group these 2…");
    press(&mut window, "Group these");
    assert_eq!(text_of_step(&window, 4), "group = vgroup(circle, square)");

    // A plain click picks one object again.
    window.get_by_label("Canvas").click();
    window.run_ok();
    assert!(window.state().picked_too.is_empty());
}

#[test]
fn animations_take_turns_and_python_fills_the_gaps() {
    let mut window = open(Session::blank());
    press(&mut window, "Circle");
    press(&mut window, "Animate");
    press(&mut window, "Add animation");
    assert_eq!(headlines(&window)[2], "Indicate circle + Indicate circle");
    press(&mut window, "Play these one after another");
    assert_eq!(headlines(&window)[2], "2 in a row");
    assert!(window.state().session.checked.diagnostics.is_empty());
    // With two animations in a row the properties are longer than their panel, and the
    // button is below its edge; it is pressed without being pointed at.
    window
        .get_by_role_and_label(Role::Button, "Play these together")
        .click_accesskit();
    window.run_ok();
    assert_eq!(headlines(&window)[2], "Indicate circle + Indicate circle");

    menu(&mut window, "More steps", "Python code");
    assert_eq!(headlines(&window)[3], "Python code");
    assert!(matches!(
        window.state().session.steps()[3].item,
        Command::Raw(_)
    ));
}

#[test]
fn pictures_are_named_from_where_the_deck_is() {
    let folder = scratch("portable");
    let elsewhere = scratch("elsewhere");
    fs::create_dir_all(folder.join("art")).unwrap();
    fs::write(folder.join("art/inside.png"), b"one picture").unwrap();
    fs::write(elsewhere.join("outside.png"), b"another picture").unwrap();
    let file_of = |session: &Session, step: usize| {
        let Command::LetObject { value, .. } = &session.steps()[step].item else {
            panic!("not an object")
        };
        let Object::Chain { base, .. } = &value.item else {
            panic!("not placed")
        };
        let Object::New { args, .. } = &base.item else {
            panic!("not made")
        };
        let suit_core::Arg::Expr(Expr::Str(file)) = &args[0].1 else {
            panic!("no file")
        };
        file.clone()
    };

    let mut session = Session::blank();
    session.save_as(&folder.join("deck.suit.json")).unwrap();
    session.add_picture(&folder.join("art/inside.png")).unwrap();
    session.add_picture(&elsewhere.join("outside.png")).unwrap();
    assert_eq!(file_of(&session, 0), "art/inside.png");
    assert_eq!(
        file_of(&session, 2),
        elsewhere.join("outside.png").display().to_string()
    );
    assert_eq!(
        session.pictures_elsewhere(),
        [elsewhere.join("outside.png")]
    );

    // Gathering copies what is elsewhere next to the deck; then the folder has it all.
    let mut window = open(session);
    menu(&mut window, "File", "Gather pictures");
    assert!(
        window
            .query_by_label("Copied 1 picture next to the deck.")
            .is_some()
    );
    let session = &mut window.state_mut().session;
    assert_eq!(file_of(session, 2), "pictures/outside.png");
    assert_eq!(
        fs::read(folder.join("pictures/outside.png")).unwrap(),
        b"another picture"
    );
    assert!(session.pictures_elsewhere().is_empty());

    // Saved somewhere else, the deck names the same files from there.
    let moved = scratch("moved");
    session.save_as(&moved.join("deck.suit.json")).unwrap();
    assert_eq!(
        file_of(session, 0),
        folder.join("art/inside.png").display().to_string()
    );
    assert!(!session.is_dirty());
    session.save_as(&folder.join("art/deck.suit.json")).unwrap();
    assert_eq!(file_of(session, 0), "inside.png");
}

#[test]
fn a_new_text_is_typed_into_straight_away() {
    let mut window = open(Session::blank());
    press(&mut window, "Text");
    assert_eq!(
        window.state().session.step,
        Some(1),
        "its entrance is the selected step"
    );
    // The words are there to be replaced: no step to find, nothing to click.
    window.event(egui::Event::Text("Hello there".to_owned()));
    window.run_ok();
    assert_eq!(text_of_step(&window, 0), "text = Text(\"Hello there\")");
    assert_eq!(window.state().session.step, Some(1));

    // Whatever step is selected, a picked object's own settings are at hand.
    press(&mut window, "Circle");
    assert_eq!(window.state().session.step, Some(3));
    menu(&mut window, "Add setting", "radius");
    assert_eq!(text_of_step(&window, 2), "circle = Circle(radius = 1.0)");
    press(&mut window, "Go to that step");
    assert_eq!(window.state().session.step, Some(2));
}
