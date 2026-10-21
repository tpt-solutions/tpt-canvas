//! Snapshot tests for the helper itself. Goldens live in the crate's `snapshots/` directory.
//! Run with `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-canvas-snapshot` to write them.

use tpt_canvas::prelude::*;
use tpt_canvas_snapshot::{assert_render, render_bytes};

#[test]
fn solid_rect_matches_golden() {
    assert_render!(
        8,
        8,
        |ctx| {
            ctx.set_fill_paint(Paint::solid(named::RED));
            ctx.fill_rect(2.0, 2.0, 4.0, 4.0);
        },
        "solid_rect"
    );
}

#[test]
fn render_bytes_has_header_and_pixels() {
    let bytes = render_bytes(3, 2, |_| {});
    assert_eq!(&bytes[0..4], &3u32.to_le_bytes());
    assert_eq!(&bytes[4..8], &2u32.to_le_bytes());
    assert_eq!(bytes.len(), 8 + 3 * 2 * 4);
}

#[test]
fn different_scene_is_reported() {
    // In update mode the helper writes instead of comparing, so there is nothing to check.
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        return;
    }
    // Writes its own golden under the target tmp dir, so it never touches the shared snapshot.
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("different_scene.rgba");
    let red = render_bytes(8, 8, |ctx| {
        ctx.set_fill_paint(Paint::solid(named::RED));
        ctx.fill_rect(2.0, 2.0, 4.0, 4.0);
    });
    std::fs::write(&path, red).unwrap();

    let err = tpt_canvas_snapshot::check_render(&path, 8, 8, |ctx| {
        ctx.set_fill_paint(Paint::solid(named::BLUE));
        ctx.fill_rect(2.0, 2.0, 4.0, 4.0);
    })
    .unwrap_err();
    assert!(err.contains("differs"), "unexpected message: {err}");
}
