//! The list of what the checker found, along the bottom.

use eframe::egui::{self, Sense};

use crate::{SuitApp, style};

impl SuitApp {
    pub(crate) fn problems(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let diagnostics = &session.checked.diagnostics;
        ui.add_space(6.0);
        ui.strong(match diagnostics.len() {
            0 => "Problems".to_owned(),
            count => format!("Problems ({count})"),
        });
        let locations = session.locations();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if diagnostics.is_empty() {
                    ui.weak("None. The deck is ready to export.");
                }
                for problem in diagnostics {
                    let step = problem
                        .node
                        .and_then(|id| locations.get(&id))
                        .map(|&(_, step)| step);
                    let place = match step {
                        Some(step) => format!("Slide {}, step {}", problem.slide + 1, step + 1),
                        None => format!("Slide {}", problem.slide + 1),
                    };
                    let row = ui.horizontal(|ui| {
                        ui.colored_label(style::severity_color(problem.severity), "⚠");
                        ui.weak(place);
                        ui.label(&problem.message);
                    });
                    if let Some(help) = &problem.help {
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.weak(help);
                        });
                    }
                    if row.response.interact(Sense::click()).clicked() {
                        chosen = Some((problem.slide, step));
                    }
                }
            });
        if let Some((slide, step)) = chosen {
            self.session.select(slide, step);
        }
    }
}
