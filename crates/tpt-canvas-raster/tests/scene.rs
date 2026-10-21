//! Scenes on the raster backend. The key property: after any edit, redrawing only the dirty
//! area gives exactly the pixels a full render gives.

use tpt_canvas::prelude::*;
use tpt_canvas::testkit::Rng;
use tpt_canvas::{BlendMode, DisplayList, Scene, Shadow};
use tpt_canvas_raster::RasterBackend;

const W: u32 = 64;
const H: u32 = 64;

/// A random item: a rectangle, a circle or a stroked rectangle, sometimes blended or shadowed.
fn random_item(rng: &mut Rng) -> DisplayList {
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    let color = Color::rgba(
        rng.next_f32(),
        rng.next_f32(),
        rng.next_f32(),
        0.4 + 0.6 * rng.next_f32(),
    );
    ctx.set_fill_paint(Paint::Solid(color));
    let x = rng.range(0.0, W as f32 - 8.0);
    let y = rng.range(0.0, H as f32 - 8.0);
    let size = rng.range(4.0, 24.0);
    match rng.index(3) {
        0 => ctx.fill_rect(x, y, size, size),
        1 => ctx.fill_circle(x + size / 2.0, y + size / 2.0, size / 2.0),
        _ => {
            ctx.set_stroke_paint(Paint::Solid(color));
            ctx.set_stroke_style(StrokeStyle {
                width: rng.range(1.0, 5.0),
                ..StrokeStyle::default()
            });
            ctx.stroke_rect(x, y, size, size);
        }
    }
    if rng.index(3) == 0 {
        let modes = [
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Difference,
            BlendMode::Hue,
        ];
        ctx.set_blend_mode(modes[rng.index(modes.len())]);
        ctx.fill_rect(x + 2.0, y + 2.0, size, size);
    }
    if rng.index(4) == 0 {
        ctx.set_shadow(Shadow {
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            offset: Vec2::new(rng.range(-3.0, 3.0), rng.range(-3.0, 3.0)),
            blur: rng.range(0.0, 6.0),
        });
        ctx.fill_rect(x, y, size, size);
    }
    ctx.restore();
    ctx.into_backend()
}

/// The pixels of a full render of `scene`.
fn full(scene: &Scene) -> Vec<[f32; 4]> {
    let mut canvas = RasterBackend::new(W, H);
    scene.render(&mut canvas);
    canvas.into_pixmap().data
}

#[test]
fn a_scene_renders_like_drawing_its_items_directly() {
    let mut rng = Rng::new(3);
    let items: Vec<DisplayList> = (0..8).map(|_| random_item(&mut rng)).collect();

    let mut scene = Scene::new();
    for item in &items {
        scene.add(item.clone());
    }
    let mut direct = RasterBackend::new(W, H);
    for item in &items {
        item.replay(&mut direct);
    }
    assert_eq!(full(&scene), direct.into_pixmap().data);
}

#[test]
fn random_edits_redraw_exactly_the_dirty_area() {
    let mut rng = Rng::new(20_260_922);
    let mut scene = Scene::new();
    let mut ids = Vec::new();
    for _ in 0..10 {
        ids.push(scene.add(random_item(&mut rng)));
    }
    // The persistent canvas, kept up to date only through dirty redraws.
    let mut canvas = RasterBackend::new(W, H);
    scene.render(&mut canvas);
    scene.take_dirty();

    for step in 0..60 {
        match rng.index(5) {
            0 | 1 => {
                let id = ids[rng.index(ids.len())];
                scene.replace(id, random_item(&mut rng));
            }
            2 => {
                let id = ids[rng.index(ids.len())];
                let shown = rng.index(2) == 0;
                scene.set_visible(id, shown);
            }
            3 => ids.push(scene.add(random_item(&mut rng))),
            _ => {
                if ids.len() > 3 {
                    let at = rng.index(ids.len());
                    scene.remove(ids.remove(at));
                }
            }
        }
        if let Some(area) = scene.take_dirty() {
            scene.render_dirty(&mut canvas, area);
        }
        assert_eq!(
            canvas.pixmap().data,
            full(&scene),
            "step {step}: incremental redraw differs from a full render"
        );
    }
}

#[test]
fn a_small_change_dirties_a_small_area() {
    let mut scene = Scene::new();
    let mut corner = Context::new(DisplayList::new());
    corner.fill_rect(2.0, 2.0, 6.0, 6.0);
    let id = scene.add(corner.into_backend());
    let mut far = Context::new(DisplayList::new());
    far.fill_rect(50.0, 50.0, 10.0, 10.0);
    scene.add(far.into_backend());
    scene.take_dirty();

    let mut moved = Context::new(DisplayList::new());
    moved.fill_rect(3.0, 3.0, 6.0, 6.0);
    scene.replace(id, moved.into_backend());
    let area = scene.take_dirty().expect("the change is dirty");
    assert!(
        area.w < 16.0 && area.h < 16.0,
        "dirty area too large: {area:?}"
    );
    assert!(
        area.x + area.w < 30.0,
        "dirty area reaches the far item: {area:?}"
    );
}

#[test]
fn glyph_runs_make_the_whole_canvas_dirty() {
    // Without a glyph source, the run stays as a glyph op, which has no bounds.
    let mut scene = Scene::new();
    let mut ctx = Context::new(DisplayList::new());
    let mut run = tpt_canvas::text::ShapedRun::new(0, 12.0);
    run.push(65, Vec2::new(4.0, 12.0));
    ctx.draw_glyphs(&run);
    let id = scene.add(ctx.into_backend());
    scene.take_dirty();
    scene.set_visible(id, false);
    let area = scene.take_dirty().expect("hiding it is a change");
    assert!(
        area.w >= W as f32 && area.h >= H as f32,
        "expected the whole canvas: {area:?}"
    );
}

#[test]
fn nothing_changed_means_nothing_dirty() {
    let mut scene = Scene::new();
    let id = scene.add(random_item(&mut Rng::new(1)));
    scene.take_dirty();
    assert!(scene.take_dirty().is_none());
    assert!(
        !scene.remove(tpt_canvas::ItemId(999)),
        "unknown ids are reported"
    );
    assert!(scene.set_visible(id, true));
    assert!(
        scene.take_dirty().is_none(),
        "showing an item that is shown changes nothing"
    );
}
