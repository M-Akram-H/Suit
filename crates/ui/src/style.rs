//! Colours, sizes and small formatting helpers shared by the panels.

use eframe::egui::{self, Color32, Rect, pos2};
use suit_check::{Diagnostic, Severity};

pub const ERROR: Color32 = Color32::from_rgb(0xf0, 0x6a, 0x6a);
pub const WARNING: Color32 = Color32::from_rgb(0xe8, 0xb8, 0x4a);
pub const ACCENT: Color32 = Color32::from_rgb(0x58, 0xc4, 0xdd);

/// Height of a row in a list.
pub const ROW: f32 = 26.0;

/// The whole of a texture.
pub const UV: Rect = Rect {
    min: pos2(0.0, 0.0),
    max: pos2(1.0, 1.0),
};

/// A length of time for lists: at most two decimals, none if it is whole.
pub fn seconds(secs: f64) -> String {
    let text = format!("{secs:.2}");
    format!("{} s", text.trim_end_matches('0').trim_end_matches('.'))
}

pub fn severity_color(severity: Severity) -> Color32 {
    match severity {
        Severity::Error => ERROR,
        Severity::Warning => WARNING,
    }
}

/// The colour for the most serious of `diagnostics`; `None` if there are none.
pub fn worst<'d>(diagnostics: impl Iterator<Item = &'d Diagnostic>) -> Option<Color32> {
    let mut found = None;
    for diagnostic in diagnostics {
        if diagnostic.severity == Severity::Error {
            return Some(ERROR);
        }
        found = Some(WARNING);
    }
    found
}

pub fn diagnostic(ui: &mut egui::Ui, problem: &Diagnostic) {
    ui.colored_label(
        severity_color(problem.severity),
        format!("⚠ {}", problem.message),
    );
    if let Some(help) = &problem.help {
        ui.weak(help);
    }
}

/// Makes a hand-painted row behave like a button for assistive technology and for tests.
pub fn as_button(response: &egui::Response, selected: bool, label: &str) {
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
}

/// A button that shows only an arrow but says what it does: in its tooltip, and to
/// assistive technology and tests. Returns whether it was clicked.
pub fn arrow(ui: &mut egui::Ui, enabled: bool, glyph: &str, meaning: &str) -> bool {
    let response = ui
        .add_enabled(enabled, egui::Button::new(glyph))
        .on_hover_text(meaning);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, meaning));
    response.clicked()
}
