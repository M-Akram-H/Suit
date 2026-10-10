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
