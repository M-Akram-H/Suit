//! The window driven the way a person drives it: clicks, keys and drags, with no display and
//! no Manim. Where the canvas needs a picture of the scene, the test supplies one.

use std::collections::BTreeMap;

use eframe::egui::{self, Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use suit_codegen::pretty::Pretty;
use suit_core::{Anim, Command, Expr, Object, VarId};
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
        error: None,
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

/// A window whose slide has been recorded as `frames` frames at 60 fps. The harness moves
/// time on a quarter of a second, fifteen frames, with every frame it draws.
fn with_a_recording(looping: bool, frames: usize) -> Window {
    let (mut window, _) = with_a_circle([0.0, 0.0]);
    let session = &mut window.state_mut().session;
    session.edit_slide(|slide| slide.config.looping = looping);
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
    };
    session.install_recording(Clip {
        pool: vec![drawn],
        frames: vec![vec![0]; frames],
        marks: Vec::new(),
        error: None,
    });
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
