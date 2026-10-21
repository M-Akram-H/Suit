//! A deck as a PDF: a page for every slide, or for every step, drawn from the outlines
//! Manim reports, so that the pages are as sharp as the reader's zoom.
//!
//! The pictures are those of the canvas (spec §7.1): the scene after a step, filled and
//! stroked the way Manim's Cairo camera does it.

use std::collections::BTreeMap;

use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::{
    Content, Filter, Finish, Name, Pdf, Rect, Ref, TextStr,
    types::{LineCapStyle, LineJoinStyle},
    writers::Resources,
};
use suit_core::{Deck, SlideContent};
use suit_render::{Drawn, Outline, Picture, Snapshots};

/// Manim's frame is eight scene units high whatever the resolution.
const FRAME_HEIGHT: f32 = 8.0;
/// Manim's stroke widths are hundredths of a scene unit.
const STROKE_UNIT: f32 = 0.01;
/// Points further out than this, in scene units, are drawn as if they were this far.
/// What can be seen of a stroke that leaves the page is the same wherever it ends.
const FAR: f32 = 100.0;
/// A page is 7.5 inches high: a 16:9 deck gets PowerPoint's widescreen page.
const PAGE_HEIGHT: f32 = 540.0;
/// How hard streams are compressed, from 0 to 10.
const SQUEEZE: u8 = 6;

/// Which pictures of the deck become pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pages {
    /// The picture each slide ends on, as manim-slides makes a PDF.
    #[default]
    Slides,
    /// The picture after every step that changes it: how each slide builds up.
    Steps,
}

/// One page: what is on it, back to front, and the slide it belongs to.
pub struct Page<'s> {
    pub slide: usize,
    pub drawn: &'s [Drawn],
}

/// The pages of `deck`, from `snapshots`, which must show every slide as it is. Skipped
/// slides have none, and neither do slides that show a file rather than steps.
pub fn pages<'s>(
    deck: &Deck,
    snapshots: &'s Snapshots,
    which: Pages,
) -> Result<Vec<Page<'s>>, String> {
    let mut pages: Vec<Page<'s>> = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        if slide.config.skip || !matches!(slide.content, SlideContent::Blueprint(_)) {
            continue;
        }
        // The scene the slide starts with, then the scene after each of its steps.
        let states: Vec<&[Drawn]> = snapshots
            .steps
            .iter()
            .filter(|state| state.slide == index)
            .map(|state| state.drawn.as_slice())
            .collect();
        let end = snapshots
            .last_of(index)
            .map(|state| state.drawn.as_slice())
            .ok_or_else(|| format!("Manim has not shown slide {} yet.", index + 1))?;
        let before = pages.len();
        if which == Pages::Steps {
            // A step that changes what shows makes a page, unless what it leaves is an
            // empty screen: the slide clearing away the one before.
            for pair in states.windows(2) {
                let [was, drawn] = [pair[0], pair[1]];
                if !drawn.is_empty() && !same(was, drawn) {
                    pages.push(Page {
                        slide: index,
                        drawn,
                    });
                }
            }
        }
        // Every slide has a page, even one whose steps change nothing that shows.
        if pages.len() == before {
            pages.push(Page {
                slide: index,
                drawn: end,
            });
        }
    }
    if pages.is_empty() {
        return Err("The deck has no slide to show.".to_owned());
    }
    Ok(pages)
}

/// Whether two pictures look the same.
fn same(a: &[Drawn], b: &[Drawn]) -> bool {
    let picture = |p: &Option<Picture>| {
        p.as_ref()
            .map(|p| (p.file.clone(), p.corners, p.opacity.to_bits()))
    };
    let outline = |o: &Outline| (o.closed, o.points.clone());
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.fill == b.fill
                && a.stroke == b.stroke
                && a.stroke_width == b.stroke_width
                && picture(&a.picture) == picture(&b.picture)
                && a.paths.len() == b.paths.len()
                && a.paths.iter().map(outline).eq(b.paths.iter().map(outline))
        })
}

/// Hands out the numbers of a PDF's objects.
struct Refs(i32);

impl Refs {
    fn next(&mut self) -> Ref {
        self.0 += 1;
        Ref::new(self.0)
    }
}

/// What the pages share: pictures, and the levels of see-through they are drawn with.
#[derive(Default)]
struct Shared {
    /// Per picture file: its name on the pages, and its pixels if they could be read.
    pictures: BTreeMap<std::path::PathBuf, (String, Option<Ref>)>,
    /// Per pair of fill and stroke opacity, in 255ths: its name on the pages.
    alphas: BTreeMap<(u8, u8), String>,
}

/// The PDF of `pages` of `deck`, called `title`.
pub fn write(deck: &Deck, pages: &[Page], title: &str) -> Vec<u8> {
    let config = &deck.config;
    let aspect = config.width as f32 / config.height.max(1) as f32;
    let (width, height) = (PAGE_HEIGHT * aspect, PAGE_HEIGHT);
    let frame_width = FRAME_HEIGHT * aspect;

    let mut refs = Refs(0);
    let (catalog, tree, info, outline, resources) = (
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
    );
    let page_refs: Vec<(Ref, Ref)> = pages.iter().map(|_| (refs.next(), refs.next())).collect();

    let mut pdf = Pdf::new();
    let mut shared = Shared::default();
    let mut images: Vec<(Ref, &Picture)> = Vec::new();
    for (page, &(page_ref, content_ref)) in pages.iter().zip(&page_refs) {
        let mut content = Content::new();
        // From scene units, centred and pointing up, to points from the bottom left.
        let scale = height / FRAME_HEIGHT;
        content.transform([scale, 0.0, 0.0, scale, width / 2.0, height / 2.0]);
        let [r, g, b, _] = config.background.0.map(|c| f32::from(c) / 255.0);
        content.set_fill_rgb(r, g, b);
        content.rect(
            -frame_width / 2.0,
            -FRAME_HEIGHT / 2.0,
            frame_width,
            FRAME_HEIGHT,
        );
        content.fill_nonzero();
        for drawn in page.drawn {
            match &drawn.picture {
                Some(picture) => {
                    draw_picture(&mut content, picture, &mut shared, &mut refs, &mut images)
                }
                None => draw_outlines(&mut content, drawn, &mut shared),
            }
        }
        let squeezed = compress_to_vec_zlib(&content.finish(), SQUEEZE);
        pdf.stream(content_ref, &squeezed)
            .filter(Filter::FlateDecode);
        let mut writer = pdf.page(page_ref);
        writer
            .parent(tree)
            .media_box(Rect::new(0.0, 0.0, width, height))
            .contents(content_ref);
        writer.pair(Name(b"Resources"), resources);
        writer.finish();
    }

    // The resources every page draws on, all in one dictionary.
    let mut writer = pdf.indirect(resources).start::<Resources>();
    let mut objects = writer.x_objects();
    for (name, image) in shared.pictures.values() {
        if let Some(image) = image {
            objects.pair(Name(name.as_bytes()), *image);
        }
    }
    objects.finish();
    let mut states = writer.ext_g_states();
    for (&(fill, stroke), name) in &shared.alphas {
        states
            .insert(Name(name.as_bytes()))
            .start::<pdf_writer::writers::ExtGraphicsState>()
            .non_stroking_alpha(f32::from(fill) / 255.0)
            .stroking_alpha(f32::from(stroke) / 255.0);
    }
    states.finish();
    writer.finish();
    for (image, picture) in images {
        embed(&mut pdf, &mut refs, image, picture);
    }

    // A bookmark for every slide, at its first page.
    let mut marks: Vec<(usize, Ref)> = Vec::new();
    for (page, &(page_ref, _)) in pages.iter().zip(&page_refs) {
        if marks.last().is_none_or(|&(slide, _)| slide != page.slide) {
            marks.push((page.slide, page_ref));
        }
    }
    let items: Vec<Ref> = marks.iter().map(|_| refs.next()).collect();
    for (index, (&(slide, page_ref), &item)) in marks.iter().zip(&items).enumerate() {
        let named = deck.slides[slide].title.trim();
        let label = match named.is_empty() {
            true => format!("Slide {}", slide + 1),
            false => named.to_owned(),
        };
        let mut writer = pdf.outline_item(item);
        writer.title(TextStr(&label)).parent(outline);
        if let Some(&previous) = index.checked_sub(1).and_then(|i| items.get(i)) {
            writer.prev(previous);
        }
        if let Some(&next) = items.get(index + 1) {
            writer.next(next);
        }
        writer.dest().page(page_ref).fit();
    }
    let mut writer = pdf.outline(outline);
    if let (Some(&first), Some(&last)) = (items.first(), items.last()) {
        writer.first(first).last(last);
    }
    writer.count(items.len() as i32);
    writer.finish();

    pdf.pages(tree)
        .kids(page_refs.iter().map(|&(page, _)| page))
        .count(pages.len() as i32);
    pdf.catalog(catalog).pages(tree).outlines(outline);
    pdf.document_info(info)
        .title(TextStr(title))
        .creator(TextStr("Suit"))
        .producer(TextStr("Suit"));
    pdf.set_file_id((title.as_bytes().to_vec(), title.as_bytes().to_vec()));
    pdf.finish()
}

/// The name of the graphics state that draws with these opacities, if they need one.
fn alpha(shared: &mut Shared, fill: f32, stroke: f32) -> Option<String> {
    let level = |alpha: f32| (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let key = (level(fill), level(stroke));
    if key == (255, 255) {
        return None;
    }
    let count = shared.alphas.len();
    let name = shared
        .alphas
        .entry(key)
        .or_insert_with(|| format!("A{count}"));
    Some(name.clone())
}

/// Fills, then strokes, the outlines of `drawn`: the order Manim's Cairo camera uses.
fn draw_outlines(content: &mut Content, drawn: &Drawn, shared: &mut Shared) {
    let [fr, fg, fb, fa] = drawn.fill.map(|c| c.clamp(0.0, 1.0));
    let [sr, sg, sb, sa] = drawn.stroke.map(|c| c.clamp(0.0, 1.0));
    let fills = fa > 0.0;
    let strokes = sa > 0.0 && drawn.stroke_width > 0.0;
    if !fills && !strokes {
        return;
    }
    content.save_state();
    let state = alpha(
        shared,
        if fills { fa } else { 1.0 },
        if strokes { sa } else { 1.0 },
    );
    if let Some(state) = &state {
        content.set_parameters(Name(state.as_bytes()));
    }
    if fills {
        content.set_fill_rgb(fr, fg, fb);
    }
    if strokes {
        // Cairo's defaults, which Manim leaves alone unless a joint or cap style is set.
        content
            .set_stroke_rgb(sr, sg, sb)
            .set_line_width(drawn.stroke_width * STROKE_UNIT)
            .set_line_cap(LineCapStyle::ButtCap)
            .set_line_join(LineJoinStyle::MiterJoin)
            .set_miter_limit(10.0);
    }
    if trace(content, &drawn.paths) {
        match (fills, strokes) {
            (true, true) => content.fill_nonzero_and_stroke(),
            (true, false) => content.fill_nonzero(),
            (false, _) => content.stroke(),
        };
    }
    content.restore_state();
}

/// Writes the outlines as a path; false if none of them can be drawn.
fn trace(content: &mut Content, outlines: &[Outline]) -> bool {
    let near = |[x, y]: [f32; 2]| [x.clamp(-FAR, FAR), y.clamp(-FAR, FAR)];
    let mut any = false;
    for outline in outlines {
        // Manim draws nothing of an outline with a point that is not a number.
        let numbers = |point: &[f32; 2]| point[0].is_finite() && point[1].is_finite();
        if !outline.points.iter().all(numbers) {
            continue;
        }
        let Some((&start, curves)) = outline.points.split_first() else {
            continue;
        };
        let [x, y] = near(start);
        content.move_to(x, y);
        for &[first, second, end] in curves.as_chunks::<3>().0 {
            let ([x1, y1], [x2, y2], [x, y]) = (near(first), near(second), near(end));
            content.cubic_to(x1, y1, x2, y2, x, y);
        }
        if outline.closed {
            content.close_path();
        }
        any = true;
    }
    any
}

/// Draws a picture where Manim has it: any parallelogram, given by three of its corners.
fn draw_picture<'p>(
    content: &mut Content,
    picture: &'p Picture,
    shared: &mut Shared,
    refs: &mut Refs,
    images: &mut Vec<(Ref, &'p Picture)>,
) {
    if picture.png.is_empty() || picture.opacity <= 0.0 {
        return;
    }
    let count = shared.pictures.len();
    let (name, image) = shared
        .pictures
        .entry(picture.file.clone())
        .or_insert_with(|| {
            let image = refs.next();
            images.push((image, picture));
            (format!("P{count}"), Some(image))
        })
        .clone();
    if image.is_none() {
        return;
    }
    let [top_left, top_right, bottom_left] = picture.corners;
    content.save_state();
    if let Some(state) = alpha(shared, picture.opacity, 1.0) {
        content.set_parameters(Name(state.as_bytes()));
    }
    // An image fills the unit square, its first row at the top.
    content.transform([
        top_right[0] - top_left[0],
        top_right[1] - top_left[1],
        top_left[0] - bottom_left[0],
        top_left[1] - bottom_left[1],
        bottom_left[0],
        bottom_left[1],
    ]);
    content.x_object(Name(name.as_bytes()));
    content.restore_state();
}

/// Writes the pixels of `picture` as image `image`, with a mask if it is see-through.
fn embed(pdf: &mut Pdf, refs: &mut Refs, image: Ref, picture: &Picture) {
    let decoded = tiny_skia::Pixmap::decode_png(&picture.png).ok();
    // A file that is not a picture after all is drawn as one transparent pixel.
    let (width, height, colour, alpha) = match &decoded {
        Some(pixmap) => {
            let pixels: Vec<_> = pixmap.pixels().iter().map(|p| p.demultiply()).collect();
            let colour: Vec<u8> = pixels
                .iter()
                .flat_map(|p| [p.red(), p.green(), p.blue()])
                .collect();
            let alpha: Vec<u8> = pixels.iter().map(|p| p.alpha()).collect();
            (pixmap.width(), pixmap.height(), colour, alpha)
        }
        None => (1, 1, vec![0, 0, 0], vec![0]),
    };
    let mask = alpha
        .iter()
        .any(|&a| a < 255)
        .then(|| (refs.next(), compress_to_vec_zlib(&alpha, SQUEEZE)));
    let squeezed = compress_to_vec_zlib(&colour, SQUEEZE);
    let mut writer = pdf.image_xobject(image, &squeezed);
    writer
        .width(width as i32)
        .height(height as i32)
        .bits_per_component(8)
        .interpolate(true);
    writer.filter(Filter::FlateDecode);
    writer.color_space().device_rgb();
    if let Some((mask, _)) = &mask {
        writer.s_mask(*mask);
    }
    writer.finish();
    if let Some((mask, squeezed)) = &mask {
        let mut writer = pdf.image_xobject(*mask, squeezed);
        writer
            .width(width as i32)
            .height(height as i32)
            .bits_per_component(8)
            .interpolate(true);
        writer.filter(Filter::FlateDecode);
        writer.color_space().device_gray();
        writer.finish();
    }
}

#[cfg(test)]
mod tests {
    use suit_core::DeckConfig;
    use suit_render::Snapshot;

    use super::*;

    fn square(at: f32, fill: [f32; 4]) -> Drawn {
        let corner = |x: f32, y: f32| [at + x, y];
        let line = |from: [f32; 2], to: [f32; 2]| [from, to, to];
        let (a, b, c, d) = (
            corner(0.0, 0.0),
            corner(1.0, 0.0),
            corner(1.0, 1.0),
            corner(0.0, 1.0),
        );
        let mut points = vec![a];
        for (from, to) in [(a, b), (b, c), (c, d), (d, a)] {
            points.extend(line(from, to));
        }
        Drawn {
            owners: Vec::new(),
            paths: vec![Outline {
                points,
                closed: true,
            }],
            fill,
            stroke: [1.0; 4],
            stroke_width: 4.0,
            picture: None,
        }
    }

    /// A deck of three slides; the second is skipped. Each step adds a square.
    fn deck_and_snapshots() -> (Deck, Snapshots) {
        let mut deck = Deck::new(DeckConfig::default());
        for title in ["One", "Two", "Three"] {
            deck.add_slide(title);
        }
        deck.slides[1].config.skip = true;
        let ids: Vec<_> = (0..6).map(|n| Some(suit_core::NodeId(n))).collect();
        let state = |slide: usize, node: usize, squares: usize| Snapshot {
            slide,
            node: ids[node],
            drawn: (0..squares)
                .map(|n| square(n as f32 * 2.0, [0.2, 0.4, 0.8, 0.5]))
                .collect(),
            boxes: BTreeMap::new(),
        };
        let start = |slide: usize, squares: usize| Snapshot {
            node: None,
            ..state(slide, 0, squares)
        };
        let steps = vec![
            start(0, 0),
            state(0, 0, 1),
            state(0, 1, 1), // a step that changes nothing that shows
            state(0, 2, 2),
            start(1, 2),
            state(1, 3, 3),
            start(2, 3),
            state(2, 4, 0), // the slide clears the screen first
            state(2, 5, 4),
        ];
        let snapshots = Snapshots {
            steps,
            ..Snapshots::default()
        };
        (deck, snapshots)
    }

    #[test]
    fn a_page_per_slide_or_per_step_that_shows() {
        let (deck, snapshots) = deck_and_snapshots();
        let count = |pages: &[Page]| -> Vec<(usize, usize)> {
            pages.iter().map(|p| (p.slide, p.drawn.len())).collect()
        };
        let slides = pages(&deck, &snapshots, Pages::Slides).unwrap();
        assert_eq!(
            count(&slides),
            [(0, 2), (2, 4)],
            "the skipped slide has none"
        );
        let steps = pages(&deck, &snapshots, Pages::Steps).unwrap();
        assert_eq!(
            count(&steps),
            [(0, 1), (0, 2), (2, 4)],
            "neither a step that changes nothing nor an empty screen makes a page"
        );
    }

    #[test]
    fn the_pdf_has_its_pages_bookmarks_and_see_through_fills() {
        let (deck, snapshots) = deck_and_snapshots();
        let steps = pages(&deck, &snapshots, Pages::Steps).unwrap();
        let pdf = write(&deck, &steps, "Squares");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-"));
        assert_eq!(
            text.matches("/Type /Page\n").count() + text.matches("/Type /Page ").count(),
            3
        );
        assert!(text.contains("/Count 3"));
        assert!(text.contains("(One)") && text.contains("(Three)"));
        assert!(!text.contains("(Two)"), "no bookmark for a skipped slide");
        // Half see-through fills, opaque strokes.
        assert!(text.contains("/ca 0.5") && text.contains("/CA 1"));
        // A 16:9 page is 960 by 540 points.
        assert!(text.contains("/MediaBox [0 0 960 540]"));
    }
}
