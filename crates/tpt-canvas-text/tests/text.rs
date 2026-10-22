//! Text from tpt-text, drawn through tpt-canvas. The font is Noto Sans (SIL Open Font
//! License, see `fonts/`).

use tpt_canvas::{Color, Context, Paint};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_text::TextRuns;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

const FONT: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");

/// Lays `text` out with Noto Sans at `size` pixels, wrapped to `width`.
fn runs_for(text: &str, size: i32, width: i32) -> TextRuns {
    let registry = FontRegistry::new();
    let face = registry
        .load_font(FONT.to_vec(), 0)
        .expect("the font loads");
    let style = TextStyle::new(face.family(), Fx::from_int(size));
    let layout =
        layout_text(&registry, text, &style, Fx::from_int(width)).expect("the text lays out");
    TextRuns::from_layout(&layout)
}

/// The glyph id of `c` in the font, through tpt-text.
fn glyph_of(c: char) -> u32 {
    let registry = FontRegistry::new();
    let face = registry
        .load_font(FONT.to_vec(), 0)
        .expect("the font loads");
    face.glyph_index(c).expect("the font has the glyph")
}

#[test]
fn one_run_holds_every_glyph_of_a_line() {
    let runs = runs_for("Hello", 24, 400);
    assert_eq!(runs.runs().len(), 1, "one font and one style make one run");
    let glyphs = &runs.runs()[0].glyphs;
    assert_eq!(glyphs.len(), 5, "one glyph per letter");
}

#[test]
fn glyphs_move_left_to_right_along_the_baseline() {
    let runs = runs_for("Hello", 24, 400);
    let glyphs = &runs.runs()[0].glyphs;
    let xs: Vec<f32> = glyphs.iter().map(|g| g.pos.x).collect();
    assert!(
        xs.windows(2).all(|w| w[1] > w[0]),
        "pen moves right: {xs:?}"
    );
    let y = glyphs[0].pos.y;
    assert!(
        glyphs.iter().all(|g| (g.pos.y - y).abs() < 1e-3),
        "one baseline"
    );
    assert!(y > 0.0, "the baseline sits below the top of the paragraph");
}

#[test]
fn outlines_are_y_up_with_capitals_above_the_baseline() {
    let runs = runs_for("H", 24, 400);
    let source = runs.glyph_source();
    let font_id = runs.runs()[0].font_id;
    let path = source
        .outline(font_id, glyph_of('H'))
        .expect("H has an outline");
    let bounds = path.bounding_box();
    // y up: the glyph's bottom sits on the baseline and its top is above it.
    assert!(bounds.y.abs() < 1.0, "H sits on the baseline: {bounds:?}");
    assert!(
        bounds.y + bounds.h > 500.0,
        "H is tall in font units: {bounds:?}"
    );
    assert_eq!(source.units_per_em(font_id), Some(1000.0));
}

#[test]
fn a_space_has_no_outline() {
    let runs = runs_for("a b", 24, 400);
    let source = runs.glyph_source();
    let font_id = runs.runs()[0].font_id;
    assert!(source.outline(font_id, glyph_of(' ')).is_none());
}

/// Ink (alpha) per pixel of a raster backend, row by row.
fn raster_ink(draw: impl FnOnce(&mut Context<RasterBackend>)) -> RasterBackend {
    let mut ctx = Context::new(RasterBackend::new(120, 60));
    ctx.set_fill_paint(Paint::Solid(Color::BLACK));
    draw(&mut ctx);
    ctx.into_backend()
}

#[test]
fn text_is_drawn_inside_its_layout_box() {
    let runs = runs_for("Hello", 24, 400);
    let backend = raster_ink(|ctx| {
        ctx.translate(10.0, 10.0);
        runs.draw(ctx);
    });
    let pixmap = backend.into_pixmap();
    let mut ink = 0usize;
    let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    for y in 0..60 {
        for x in 0..120 {
            if pixmap.pixel(x, y)[3] > 0.5 {
                ink += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    assert!(ink > 200, "text leaves ink on the canvas: {ink} pixels");
    assert!(min_x >= 10, "nothing left of the origin: {min_x}");
    assert!(min_y >= 10, "nothing above the origin: {min_y}");
    assert!(max_x < 110, "the text fits the width: {max_x}");
    assert!(
        max_y < 10 + 24 + 12,
        "the text stays within a line: {max_y}"
    );
}

#[test]
fn wrapped_text_takes_more_than_one_line() {
    let one = runs_for("Hello world", 24, 400);
    let wrapped = runs_for("Hello world", 24, 60);
    assert!(wrapped.runs().len() > one.runs().len());
    let top_one = one.runs()[0].glyphs[0].pos.y;
    let top_wrapped = wrapped.runs().last().unwrap().glyphs[0].pos.y;
    assert!(top_wrapped > top_one, "later lines sit lower");
}

#[test]
fn reference_ink_matches_the_raster_backend() {
    let runs = runs_for("Hello", 24, 400);
    let raster = raster_ink(|ctx| {
        ctx.translate(10.0, 10.0);
        runs.draw(ctx);
    })
    .into_pixmap();
    let raster_mass: f32 = raster.data.iter().map(|p| p[3]).sum();

    let mut ctx = Context::new(WebGpuBackend::new(120, 60));
    ctx.set_fill_paint(Paint::Solid(Color::BLACK));
    ctx.translate(10.0, 10.0);
    runs.draw(&mut ctx);
    let gpu = ctx.into_backend();
    assert_eq!(gpu.draw_list().unsupported, 0, "every glyph is drawn");
    let reference_mass: f32 = execute(gpu.draw_list()).data.iter().map(|p| p[3]).sum();

    // Raster antialiases the curves, and the reference samples pixel centres. Their coverage
    // differs at the edges and agrees in total.
    let ratio = reference_mass / raster_mass;
    assert!(
        (ratio - 1.0).abs() < 0.05,
        "ink totals differ: raster {raster_mass:.1}, reference {reference_mass:.1}"
    );
}
