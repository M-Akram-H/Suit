//! The list of what is the matter with the deck, along the bottom: what the checker found,
//! what Manim remarked on when it ran the deck, and Manim not getting through it at all.

use eframe::egui::{self, Sense};
use suit_engine::Preview;

use crate::{SuitApp, style};

impl SuitApp {
    pub(crate) fn problems(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let stopped = match &session.preview {
            Preview::Failed(reason) => Some(reason.as_str()),
            _ => None,
        };
        let count = session.problems().count() + usize::from(stopped.is_some());
        ui.add_space(6.0);
        ui.strong(match count {
            0 => "Problems".to_owned(),
            count => format!("Problems ({count})"),
        });
        let locations = session.locations();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if count == 0 {
                    ui.weak("None. The deck is ready to export.");
                }
                if let Some(reason) = stopped {
                    ui.horizontal(|ui| {
                        ui.colored_label(style::WARNING, "⚠");
                        ui.weak("Manim");
                        ui.label(format!("stopped before the end of the deck: {reason}"));
                    });
                }
                for (index, problem) in session.problems().enumerate() {
                    let step = problem
                        .node
                        .and_then(|id| locations.get(&id))
                        .map(|&(_, step)| step);
                    let place = match step {
                        Some(step) => format!("Slide {}, step {}", problem.slide + 1, step + 1),
                        None => format!("Slide {}", problem.slide + 1),
                    };
                    let row = ui.scope(|ui| {
                        // A problem is clicked to go to it: its words are not there to be
                        // selected, which would take the click.
                        ui.style_mut().interaction.selectable_labels = false;
                        ui.horizontal(|ui| {
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
                    });
                    let whole = ui.interact(
                        row.response.rect,
                        ui.id().with(("problem", index)),
                        Sense::click(),
                    );
                    if whole
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Go there")
                        .clicked()
                    {
                        chosen = Some((problem.slide, step));
                    }
                }
            });
        if let Some((slide, step)) = chosen {
            self.session.select(slide, step);
        }
    }
}
