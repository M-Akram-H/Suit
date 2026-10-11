//! Drawing a snapshot of the scene: Manim's outlines, rasterised the way Cairo would.

use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

use eframe::egui;
use suit_core::{DeckConfig, Rgba, VarId};
use suit_render::{Drawn, Outline, Picture};
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pixmap,
    PixmapPaint, Stroke, Transform,
};

/// Manim's frame is eight scene units high whatever the resolution.
const FRAME_HEIGHT: f32 = 8.0;
/// Manim's stroke widths are hundredths of a scene unit.
const STROKE_UNIT: f32 = 0.01;
/// Points further from the middle than this, in scene units, are drawn as if they were
/// this far: the frame is eight units high, and the rasteriser counts in sixteen bits.
const FAR: f32 = 100.0;

/// The visible part of the scene, centred on the origin, in scene units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub width: f32,
    pub height: f32,
}

impl Frame {
    pub fn of(config: &DeckConfig) -> Frame {
        let aspect = config.width as f32 / config.height as f32;
        Frame {
            width: FRAME_HEIGHT * aspect,
            height: FRAME_HEIGHT,
        }
    }

    pub fn aspect(self) -> f32 {
        self.width / self.height
    }

    /// Scene point to a position inside `rect`. Scene `y` points up, screen `y` down.
    pub fn to_screen(self, rect: egui::Rect, [x, y]: [f32; 2]) -> egui::Pos2 {
        let scale = rect.height() / self.height;
        rect.center() + egui::vec2(x * scale, -y * scale)
    }

    /// The inverse of [`Frame::to_screen`].
    pub fn to_scene(self, rect: egui::Rect, pos: egui::Pos2) -> [f32; 2] {
        let scale = rect.height() / self.height;
        let offset = pos - rect.center();
        [offset.x / scale, -offset.y / scale]
    }

    /// The largest rectangle of the frame's shape that fits in `available`, centred.
    pub fn fit(self, available: egui::Rect) -> egui::Rect {
        let size = match available.width() / available.height() > self.aspect() {
            true => egui::vec2(available.height() * self.aspect(), available.height()),
            false => egui::vec2(available.width(), available.width() / self.aspect()),
        };
        egui::Rect::from_center_size(available.center(), size)
    }
}

/// A surface to paint frames on, kept between frames so that playing a slide does not
/// allocate a full-screen buffer sixty times a second.
#[derive(Default)]
pub struct Raster {
    pixmap: Option<Pixmap>,
}

impl Raster {
    /// Paints `drawn`, back to front, on `background` into an image of `size` pixels.
    /// `moved` shifts the outlines of one object, in scene units: an object being dragged,
    /// before Manim has been asked where everything ends up.
    pub fn draw<'d>(
        &mut self,
        drawn: impl IntoIterator<Item = &'d Drawn>,
        frame: Frame,
        background: Rgba,
        size: [u32; 2],
        moved: Option<(VarId, [f32; 2])>,
    ) -> egui::ColorImage {
        let [width, height] = size.map(|side| side.max(1));
        let fits = |pixmap: &Pixmap| pixmap.width() == width && pixmap.height() == height;
        let pixmap = match self.pixmap.take().filter(fits) {
            Some(pixmap) => self.pixmap.insert(pixmap),
            None => self
                .pixmap
                .insert(Pixmap::new(width, height).expect("the size is not zero")),
        };
        let [r, g, b, _] = background.0;
        pixmap.fill(Color::from_rgba8(r, g, b, 255));

        let scale = height as f32 / frame.height;
        let to_pixels = Transform::from_scale(scale, -scale)
            .post_translate(width as f32 / 2.0, height as f32 / 2.0);
        for drawn in drawn {
            let shift = moved.filter(|(var, _)| drawn.owners.iter().any(|(owner, _)| owner == var));
            let transform = match shift {
                Some((_, [dx, dy])) => to_pixels.pre_translate(dx, dy),
                None => to_pixels,
            };
            // A shape the rasteriser gives up on is one shape missing from the picture.
            // It must never be the end of the window, and of what was not saved in it.
            let painted = catch_unwind(AssertUnwindSafe(|| draw(pixmap, drawn, transform)));
            if painted.is_err() {
                eprintln!("suit: a shape could not be drawn and was left out");
            }
        }
        egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], pixmap.data())
    }
}

/// Paints one picture on a surface of its own; see [`Raster::draw`].
pub fn rasterize<'d>(
    drawn: impl IntoIterator<Item = &'d Drawn>,
    frame: Frame,
    background: Rgba,
    size: [u32; 2],
    moved: Option<(VarId, [f32; 2])>,
) -> egui::ColorImage {
    Raster::default().draw(drawn, frame, background, size, moved)
}

/// The pixels of `picture`, decoded once and kept: a slide that plays shows the same few
/// pictures in every frame. `None` if they are not a picture after all.
fn pixels(picture: &Picture) -> Option<Arc<Pixmap>> {
    type Decoded = HashMap<PathBuf, Option<Arc<Pixmap>>>;
    static KEPT: LazyLock<Mutex<Decoded>> = LazyLock::new(Mutex::default);
    let mut kept = KEPT.lock().ok()?;
    if let Some(known) = kept.get(&picture.file) {
        return known.clone();
    }
    // Nothing clever: a deck has a handful of pictures, and when it has had many, the
    // ones still in use are decoded again.
    if kept.len() >= 64 {
        kept.clear();
    }
    let decoded = Pixmap::decode_png(&picture.png).ok().map(Arc::new);
    kept.insert(picture.file.clone(), decoded.clone());
    decoded
}

/// Paints a picture where Manim has it: any parallelogram, given by three of its corners.
fn draw_picture(pixmap: &mut Pixmap, picture: &Picture, to_pixels: Transform) {
    let Some(source) = pixels(picture) else {
        return;
    };
    let [top_left, top_right, bottom_left] = picture.corners;
    let (width, height) = (source.width() as f32, source.height() as f32);
    // From the picture's own pixels to the scene: one pixel right is a step along the top
    // edge, one pixel down a step along the left edge.
    let place = Transform::from_row(
        (top_right[0] - top_left[0]) / width,
        (top_right[1] - top_left[1]) / width,
        (bottom_left[0] - top_left[0]) / height,
        (bottom_left[1] - top_left[1]) / height,
        top_left[0],
        top_left[1],
    );
    let paint = PixmapPaint {
        opacity: picture.opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Bilinear,
    };
    let source = source.as_ref().as_ref();
    pixmap.draw_pixmap(0, 0, source, &paint, place.post_concat(to_pixels), None);
}

fn draw(pixmap: &mut Pixmap, drawn: &Drawn, to_pixels: Transform) {
    if let Some(picture) = &drawn.picture {
        draw_picture(pixmap, picture, to_pixels);
        return;
    }
    let Some(path) = path(&drawn.paths) else {
        return;
    };
    // Fill first, stroke on top: the order Manim's Cairo camera uses.
    if let Some(paint) = paint(drawn.fill) {
        pixmap.fill_path(&path, &paint, FillRule::Winding, to_pixels, None);
    }
    if drawn.stroke_width > 0.0
        && let Some(paint) = paint(drawn.stroke)
    {
        // Cairo's defaults, which Manim leaves alone unless a joint or cap style is set.
        let stroke = Stroke {
            width: drawn.stroke_width * STROKE_UNIT,
            miter_limit: 10.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash: None,
        };
        pixmap.stroke_path(&path, &paint, &stroke, to_pixels, None);
    }
}

fn paint(rgba: [f32; 4]) -> Option<Paint<'static>> {
    let [r, g, b, a] = rgba.map(|channel| channel.clamp(0.0, 1.0));
    if a <= 0.0 {
        return None;
    }
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba(r, g, b, a)?);
    paint.anti_alias = true;
    Some(paint)
}

fn path(outlines: &[Outline]) -> Option<tiny_skia::Path> {
    let mut builder = PathBuilder::new();
    for outline in outlines {
        // Manim draws nothing of an outline with a point that is not a number.
        let numbers = |point: &[f32; 2]| point[0].is_finite() && point[1].is_finite();
        if !outline.points.iter().all(numbers) {
            continue;
        }
        // A graph near a pole has points astronomically far away. What can be seen of a
        // stroke that leaves the picture is the same wherever beyond its edge it ends.
        let near = |[x, y]: [f32; 2]| [x.clamp(-FAR, FAR), y.clamp(-FAR, FAR)];
        let Some((&start, curves)) = outline.points.split_first() else {
            continue;
        };
        let [x, y] = near(start);
        builder.move_to(x, y);
        for &[first, second, end] in curves.as_chunks::<3>().0 {
            let ([x1, y1], [x2, y2], [x, y]) = (near(first), near(second), near(end));
            builder.cubic_to(x1, y1, x2, y2, x, y);
        }
        if outline.closed {
            builder.close();
        }
    }
    builder.finish()
}

/// A texture that is repainted only when what it shows changes.
#[derive(Default)]
pub struct Cached<K> {
    key: Option<K>,
    texture: Option<egui::TextureHandle>,
}

impl<K: PartialEq> Cached<K> {
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        key: K,
        paint: impl FnOnce() -> egui::ColorImage,
    ) -> egui::TextureId {
        if self.key.as_ref() != Some(&key) || self.texture.is_none() {
            self.texture = Some(ctx.load_texture(name, paint(), egui::TextureOptions::LINEAR));
            self.key = Some(key);
        }
        self.texture.as_ref().expect("just set").id()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use suit_render::{Drawn, Outline};

    use super::*;

    /// The graph of 1/x, joined up across its pole the way Manim joins it, reaches a
    /// million million units up and down. That used to end the program.
    #[test]
    fn points_far_outside_the_picture_or_not_numbers_at_all_do_no_harm() {
        let stroke = |points: Vec<[f32; 2]>| Drawn {
            owners: Vec::new(),
            paths: vec![Outline {
                points,
                closed: false,
            }],
            fill: [0.0; 4],
            stroke: [1.0; 4],
            stroke_width: 4.0,
            picture: None,
        };
        let huge = 1.0e15;
        let line = |from: [f32; 2], to: [f32; 2]| vec![from, from, to, to];
        let drawn = [
            stroke(line([-0.05, -20.0], [0.0, -huge])),
            stroke(line([0.0, -huge], [0.0, huge])),
            stroke(line([0.0, huge], [0.05, 20.0])),
            stroke(line([1.0, f32::NAN], [2.0, 1.0])),
            stroke(line([f32::INFINITY, 0.0], [2.0, 1.0])),
            stroke(line([-3.0, 2.0], [3.0, 2.0])),
        ];
        let frame = Frame {
            width: 8.0 * 16.0 / 9.0,
            height: 8.0,
        };
        let image = rasterize(&drawn, frame, Rgba::BLACK, [640, 360], None);
        let lit = |x: usize, y: usize| image.pixels[y * 640 + x].r() > 128;
        // The stroke through the pole runs down the middle of the picture, top to bottom.
        assert!(lit(320, 20) && lit(320, 340));
        // The ordinary line is where it belongs: two units up, three to either side.
        assert!(lit(320 - 100, 180 - 90) && lit(320 + 100, 180 - 90));
        assert!(!lit(100, 300));
    }

    /// Not a check, a measurement: `cargo test -p suit-ui full_hd -- --ignored --nocapture`
    #[test]
    #[ignore = "a timing, not a test"]
    fn full_hd_frame_time() {
        // Forty letter-sized closed shapes of thirty curves each: a slide with two lines of text.
        let shape = |index: usize| {
            let (cx, cy) = (
                -6.0 + (index % 20) as f32 * 0.6,
                1.0 - (index / 20) as f32 * 1.2,
            );
            let mut points = vec![[cx + 0.25, cy]];
            for curve in 1..=30 {
                let angle = |part: f32| (curve as f32 - 1.0 + part) / 30.0 * std::f32::consts::TAU;
                let at = |a: f32, r: f32| [cx + r * a.cos(), cy + r * a.sin()];
                points.extend([
                    at(angle(0.33), 0.27),
                    at(angle(0.66), 0.23),
                    at(angle(1.0), 0.25),
                ]);
            }
            let outline = Outline {
                points,
                closed: true,
            };
            Drawn {
                owners: Vec::new(),
                paths: vec![outline],
                fill: [1.0; 4],
                stroke: [0.3, 0.8, 0.9, 1.0],
                stroke_width: 4.0,
                picture: None,
            }
        };
        let drawn: Vec<Drawn> = (0..40).map(shape).collect();
        let frame = Frame {
            width: 8.0 * 16.0 / 9.0,
            height: 8.0,
        };
        // Where the time goes, at full HD.
        let (width, height) = (1920u32, 1080u32);
        let scale = height as f32 / frame.height;
        let to_pixels = Transform::from_scale(scale, -scale)
            .post_translate(width as f32 / 2.0, height as f32 / 2.0);
        let timed = |what: &str, work: &mut dyn FnMut()| {
            let started = Instant::now();
            for _ in 0..10 {
                work();
            }
            println!(
                "  {what}: {:.2} ms",
                started.elapsed().as_secs_f64() * 100.0
            );
        };
        let mut pixmap = Pixmap::new(width, height).unwrap();
        timed("new pixmap + clear", &mut || {
            let mut fresh = Pixmap::new(width, height).unwrap();
            fresh.fill(Color::from_rgba8(0, 0, 0, 255));
        });
        timed("build paths", &mut || {
            for drawn in &drawn {
                std::hint::black_box(path(&drawn.paths));
            }
        });
        let paths: Vec<tiny_skia::Path> = drawn.iter().filter_map(|d| path(&d.paths)).collect();
        let white = paint([1.0; 4]).unwrap();
        timed("fill 40 shapes", &mut || {
            for path in &paths {
                pixmap.fill_path(path, &white, FillRule::Winding, to_pixels, None);
            }
        });
        let stroke = Stroke {
            width: 0.04,
            miter_limit: 10.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash: None,
        };
        timed("stroke 40 shapes", &mut || {
            for path in &paths {
                pixmap.stroke_path(path, &white, &stroke, to_pixels, None);
            }
        });
        timed("to egui image", &mut || {
            std::hint::black_box(egui::ColorImage::from_rgba_premultiplied(
                [width as usize, height as usize],
                pixmap.data(),
            ));
        });
        for size in [[960, 540], [1920, 1080], [2560, 1440]] {
            let started = Instant::now();
            for _ in 0..10 {
                rasterize(&drawn, frame, Rgba::BLACK, size, None);
            }
            println!(
                "{}x{}: {:.1} ms per frame",
                size[0],
                size[1],
                started.elapsed().as_secs_f64() * 100.0
            );
        }
    }
}
