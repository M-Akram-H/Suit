//! The whole chain at once — clicks, the engine, Manim, the canvas — drawn off screen and
//! saved as a picture to look at. Needs Manim and a graphics driver, so it is ignored
//! unless asked for: `cargo test -p suit-ui --test end_to_end -- --ignored`

use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use eframe::egui::{self, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use suit_engine::Session;
use suit_render::Sidecar;
use suit_ui::SuitApp;

type Window = Harness<'static, SuitApp>;

/// Draws frames until Manim has shown the deck as it now is.
fn settle(window: &mut Window) {
    let started = Instant::now();
    loop {
        window.step();
        if window.state().session.preview_is_current() {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "no preview within a minute"
        );
        thread::sleep(Duration::from_millis(10));
    }
    window.step();
    window.step();
}

fn press(window: &mut Window, button: &str) {
    window.get_by_role_and_label(Role::Button, button).click();
    settle(window);
}

#[test]
#[ignore = "needs Manim and a graphics driver"]
fn building_a_slide_by_hand() {
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut window: Window = Harness::builder()
        .with_size(egui::vec2(1480.0, 920.0))
        .with_max_steps(200)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut SuitApp| app.show(ui),
            SuitApp::new(Session::blank(), Some(sidecar)),
        );
    SuitApp::install_fonts(&window.ctx);
    settle(&mut window);

    press(&mut window, "Formula");
    press(&mut window, "Circle");
    // Drag the circle down and to the left; it has not moved yet, so this places it.
    let canvas = window.get_by_label("Canvas").rect();
    let centre = canvas.center();
    window.hover_at(centre);
    window.step();
    window.drag_at(centre);
    window.step();
    for part in [0.3, 0.7, 1.0] {
        window.hover_at(centre + egui::vec2(-220.0, 120.0) * part);
        window.step();
    }
    window.drop_at(centre + egui::vec2(-220.0, 120.0));
    settle(&mut window);
    press(&mut window, "Animate");

    let session = &window.state().session;
    assert!(
        session.checked.diagnostics.is_empty(),
        "{:?}",
        session.checked.diagnostics
    );
    assert_eq!(session.steps().len(), 5);
    let shown = session.snapshot().expect("the last step has a picture");
    assert_eq!(shown.boxes.values().filter(|bounds| bounds.on).count(), 2);

    let picture = window.render().expect("the window can be drawn");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("building_a_slide_by_hand.png");
    picture.save(&path).unwrap();
    println!("saved {}", path.display());
}

#[test]
#[ignore = "needs Manim and a graphics driver"]
fn playing_a_slide_of_the_sample() {
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut window: Window = Harness::builder()
        .with_size(egui::vec2(1480.0, 920.0))
        .with_max_steps(200)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut SuitApp| app.show(ui),
            SuitApp::new(Session::sample(), Some(sidecar)),
        );
    SuitApp::install_fonts(&window.ctx);
    settle(&mut window);
    window
        .get_by_role_and_label(Role::Button, "Slide 2: Shapes")
        .click();
    settle(&mut window);

    // Ask for the slide to be played, and wait for Manim to have gone through it.
    window.get_by_role_and_label(Role::Button, "Play").click();
    let started = Instant::now();
    while window.state().playing_frame().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "no recording within two minutes"
        );
        thread::sleep(Duration::from_millis(10));
        window.step();
    }
    println!("recorded in {:?}", started.elapsed());
    assert_eq!(window.state().playing_frame(), Some(0));

    // Every step of the harness is a quarter of a second: fifteen frames.
    for _ in 0..14 {
        window.step();
    }
    assert_eq!(
        window.state().playing_frame(),
        Some(210),
        "half-way through the morph"
    );
    let session = &window.state().session;
    let report = &session.checked.slides[1];
    assert_eq!(
        session.clip().unwrap().frames.len() as u32,
        report.frames + report.gap_frames
    );

    let picture = window.render().expect("the window can be drawn");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("playing_a_slide.png");
    picture.save(&path).unwrap();
    println!("saved {}", path.display());
}

#[test]
#[ignore = "needs Manim and a graphics driver"]
fn presenting_the_sample() {
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut window: Window = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .with_max_steps(200)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut SuitApp| app.show(ui),
            SuitApp::new(Session::sample(), Some(sidecar)),
        );
    SuitApp::install_fonts(&window.ctx);
    settle(&mut window);
    window
        .get_by_role_and_label(Role::Button, "Present")
        .click();
    window.step();
    assert!(window.state().is_presenting());

    // Draws frames until Manim has recorded the slide that is showing.
    let showing = |window: &mut Window, slide: usize| {
        let started = Instant::now();
        loop {
            window.step();
            if let Some((shown, Some(frame))) = window.state().presenting() {
                assert_eq!(shown, slide);
                return frame;
            }
            let session = &window.state().session;
            assert_eq!(session.recording_problem_of(slide), None);
            assert!(
                started.elapsed() < Duration::from_secs(120),
                "no recording of slide {} within two minutes; asked for: {}, shown: {:?}",
                slide + 1,
                session.is_recording_slide(slide),
                window.state().presenting(),
            );
            thread::sleep(Duration::from_millis(10));
        }
    };
    assert_eq!(
        showing(&mut window, 0),
        0,
        "the first slide starts when its recording arrives"
    );

    // On to the fifth slide, the graph, and three seconds into it.
    for slide in 1..=4 {
        window.key_press(egui::Key::ArrowRight);
        showing(&mut window, slide);
    }
    window.key_press(egui::Key::R);
    // The key going down starts the slide over; the key coming up is a quarter second later.
    assert_eq!(showing(&mut window, 4), 15);
    // Eleven more quarter seconds make three seconds.
    for _ in 0..11 {
        window.step();
    }
    assert_eq!(window.state().presenting(), Some((4, Some(180))));

    // Every slide the presentation asked for is recorded as long as the checker says.
    let session = &window.state().session;
    for slide in 0..=4 {
        let report = &session.checked.slides[slide];
        let frames = session.clip_of(slide).expect("recorded").frames.len() as u32;
        assert_eq!(
            frames,
            report.frames + report.gap_frames,
            "slide {}",
            slide + 1
        );
    }

    let picture = window.render().expect("the window can be drawn");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("presenting.png");
    picture.save(&path).unwrap();
    println!("saved {}", path.display());
}

#[test]
#[ignore = "needs Manim and a graphics driver"]
fn a_picture_from_a_file() {
    // A picture twice as wide as high: red on the left, blue on the right.
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pictures");
    fs::create_dir_all(&folder).unwrap();
    let mut pixels = tiny_skia::Pixmap::new(40, 20).unwrap();
    for (index, pixel) in pixels.pixels_mut().iter_mut().enumerate() {
        let red = index % 40 < 20;
        let (r, b) = if red { (255, 0) } else { (0, 255) };
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(r, 0, b, 255).unwrap();
    }
    pixels.save_png(folder.join("flag.png")).unwrap();
    fs::write(folder.join("readme.txt"), "not a picture").unwrap();

    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::blank();
    session
        .save_as(&folder.join("deck.suit.json"))
        .expect("the folder can be written to");
    let mut window: Window = Harness::builder()
        .with_size(egui::vec2(1480.0, 920.0))
        .with_max_steps(200)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut SuitApp| app.show(ui),
            SuitApp::new(session, Some(sidecar)),
        );
    SuitApp::install_fonts(&window.ctx);
    settle(&mut window);

    // Chosen by walking to it: the browser starts where the deck is.
    press(&mut window, "Picture");
    assert!(window.query_by_label("readme.txt").is_none());
    window.get_by_label("flag.png").click();
    window.step();
    press(&mut window, "Use this picture");

    let session = &window.state().session;
    assert_eq!(session.checked.diagnostics, []);
    assert_eq!(session.preview, suit_engine::Preview::Ready);
    let var = window.state().picked.expect("the picture is picked");
    let shown = session.snapshot().expect("Manim showed the step");
    // Four units high as it was put there, so eight wide, and on the slide.
    let bounds = &shown.boxes[&var];
    assert!(bounds.on);
    let [left, bottom, right, top] = bounds.rect;
    assert!((left + 4.0).abs() < 0.01 && (right - 4.0).abs() < 0.01);
    assert!((bottom + 2.0).abs() < 0.01 && (top - 2.0).abs() < 0.01);
    let picture = shown.drawn[0].picture.as_ref().expect("it is a picture");
    assert_eq!(picture.corners, [[-4.0, 2.0], [4.0, 2.0], [-4.0, -2.0]]);
    assert_eq!(picture.opacity, 1.0);
    assert!(!picture.png.is_empty(), "its pixels came along");

    // And it is drawn: red left of the middle of the canvas, blue right of it.
    let canvas = window.get_by_label("Canvas").rect();
    let image = window.render().expect("the window can be drawn");
    let at = |dx: f32| {
        let point = canvas.center() + egui::vec2(dx, 0.0);
        image.get_pixel(point.x as u32, point.y as u32).0
    };
    let (red, blue) = (at(-60.0), at(60.0));
    assert!(red[0] > 200 && red[2] < 60, "{red:?}");
    assert!(blue[2] > 200 && blue[0] < 60, "{blue:?}");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("picture.png");
    image.save(&path).unwrap();
    println!("saved {}", path.display());

    // It fades in when the slide plays: barely there at first, whole at the end.
    let started = Instant::now();
    while window.state().session.clip().is_none() {
        assert_eq!(window.state().session.recording_problem(), None);
        assert!(started.elapsed() < Duration::from_secs(60), "no recording");
        window.step();
        thread::sleep(Duration::from_millis(10));
    }
    let clip = window.state().session.clip().unwrap();
    let opacity = |frame: usize| {
        let mut drawn = clip.frame(frame);
        drawn
            .next()
            .and_then(|drawn| drawn.picture.as_ref())
            .map(|picture| picture.opacity)
    };
    assert!(
        opacity(1).is_some_and(|opacity| opacity < 0.05),
        "{:?}",
        opacity(1)
    );
    assert_eq!(opacity(clip.frames.len() - 1), Some(1.0));
}
