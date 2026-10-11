//! The slides, down the left: a picture of what each rests on, its name and its length.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::{
    SuitApp,
    canvas::{Cached, Frame, rasterize},
    style::{self, ACCENT, UV},
};

impl SuitApp {
    pub(crate) fn slides(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.strong("Slides");
        ui.horizontal(|ui| {
            if ui.button("Add slide").clicked() {
                self.session.add_slide();
            }
            let (first, last) = (
                self.session.slide == 0,
                self.session.slide + 1 >= self.session.deck.slides.len(),
            );
            if style::arrow(ui, !first, "↑", "Move slide earlier") {
                self.session.move_slide(false);
            }
            if style::arrow(ui, !last, "↓", "Move slide later") {
                self.session.move_slide(true);
            }
        });
        ui.add_space(2.0);

        let Self {
            session,
            thumbnails,
            ..
        } = self;
        thumbnails.resize_with(session.deck.slides.len(), Cached::default);
        let frame = Frame::of(&session.deck.config);
        let fps = f64::from(session.deck.config.fps);
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let slides = session.deck.slides.iter().zip(thumbnails.iter_mut());
                for (index, (slide, thumbnail)) in slides.enumerate() {
                    let width = ui.available_width();
                    let picture = vec2(width - 12.0, (width - 12.0) / frame.aspect());
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, picture.y + 34.0), Sense::click());
                    let selected = index == session.slide;
                    style::as_button(
                        &response,
                        selected,
                        &format!("Slide {}: {}", index + 1, slide.title),
                    );
                    let painter = ui.painter_at(rect);
                    if selected {
                        painter.rect_filled(
                            rect,
                            6.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.45),
                        );
                    } else if response.hovered() {
                        painter.rect_filled(rect, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
                    }

                    // The picture is what the slide rests on once it has played.
                    let picture_rect = Rect::from_min_size(rect.min + vec2(6.0, 6.0), picture);
                    let pixels = ui.ctx().pixels_per_point();
                    let size =
                        [picture.x * pixels, picture.y * pixels].map(|side| side.round() as u32);
                    let texture = thumbnail.get(
                        ui.ctx(),
                        &format!("slide-{index}"),
                        (session.generation, size),
                        || {
                            let rest = session.snapshots.last_of(index);
                            let drawn = rest.into_iter().flat_map(|snapshot| &snapshot.drawn);
                            rasterize(drawn, frame, session.deck.config.background, size, None)
                        },
                    );
                    painter.image(texture, picture_rect, UV, Color32::WHITE);
                    let outline = match selected {
                        true => Stroke::new(1.5, ACCENT),
                        false => {
                            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                        }
                    };
                    painter.rect_stroke(picture_rect, 2.0, outline, StrokeKind::Outside);

                    let baseline = picture_rect.bottom() + 14.0;
                    let mut caption = format!("{}  {}", index + 1, slide.title);
                    if slide.config.looping {
                        caption.push_str("  ⟳");
                    }
                    let text = ui.visuals().text_color();
                    painter.text(
                        pos2(rect.left() + 8.0, baseline),
                        Align2::LEFT_CENTER,
                        caption,
                        FontId::proportional(13.0),
                        text,
                    );
                    let length = session
                        .checked
                        .slides
                        .get(index)
                        .map_or(0, |report| report.frames + report.gap_frames);
                    let weak = ui.visuals().weak_text_color();
                    let right = painter.text(
                        pos2(rect.right() - 8.0, baseline),
                        Align2::RIGHT_CENTER,
                        style::seconds(f64::from(length) / fps),
                        FontId::proportional(12.0),
                        weak,
                    );
                    let problems = session.problems().filter(|d| d.slide == index);
                    if let Some(color) = style::worst(problems) {
                        painter.text(
                            pos2(right.left() - 6.0, baseline),
                            Align2::RIGHT_CENTER,
                            "⚠",
                            FontId::proportional(13.0),
                            color,
                        );
                    }
                    if response.clicked() {
                        chosen = Some(index);
                    }
                    ui.add_space(2.0);
                }
            });
        if let Some(index) = chosen {
            session.select(index, None);
            self.picked = None;
        }
    }
}
