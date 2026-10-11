//! The whole chain at once — clicks, the engine, Manim, the canvas — drawn off screen and
//! saved as a picture to look at. Needs Manim and a graphics driver, so it is ignored
//! unless asked for: `cargo test -p suit-ui --test end_to_end -- --ignored`

use std::{
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
