//! Multi-page output: page tree, per-page content, and a cross-reference table that points at every object.

use tpt_canvas::{CanvasBackend, Color, FillRule, Paint, PathBuilder};
use tpt_canvas_pdf::PdfBackend;

fn square(p: &mut PdfBackend, x: f32) {
    let mut b = PathBuilder::new();
    b.rect(x, 0.0, 4.0, 4.0);
    p.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
}

/// Content of each page stream, in file order. Stream data starts after "\nstream\n".
fn content_streams(text: &str) -> Vec<&str> {
    text.split("\nstream\n")
        .skip(1)
        .map(|s| s.split("endstream").next().unwrap())
        .collect()
}

#[test]
fn two_pages_share_one_page_tree() {
    let mut p = PdfBackend::new(100.0, 50.0);
    square(&mut p, 1.0);
    p.new_page(200.0, 80.0);
    square(&mut p, 2.0);
    square(&mut p, 3.0);
    let text = String::from_utf8_lossy(&p.finish()).into_owned();

    assert!(text.contains("/Type /Pages /Kids [3 0 R 5 0 R] /Count 2"));
    assert!(text.contains("/MediaBox [0 0 100 50]"));
    assert!(text.contains("/MediaBox [0 0 200 80]"));
    assert!(text.contains("/Contents 4 0 R"));
    assert!(text.contains("/Contents 6 0 R"));
}

#[test]
fn each_page_gets_its_own_content() {
    let mut p = PdfBackend::new(10.0, 10.0);
    square(&mut p, 1.0);
    p.new_page(10.0, 10.0);
    square(&mut p, 2.0);
    square(&mut p, 3.0);
    let text = String::from_utf8_lossy(&p.finish()).into_owned();

    let streams = content_streams(&text);
    assert_eq!(streams.len(), 2);
    // Fill operators sit at the start of their own line.
    assert_eq!(streams[0].lines().filter(|l| *l == "f").count(), 1);
    assert_eq!(streams[1].lines().filter(|l| *l == "f").count(), 2);
}

#[test]
fn xref_offsets_point_at_each_object() {
    let mut p = PdfBackend::new(10.0, 10.0);
    square(&mut p, 1.0);
    p.new_page(20.0, 20.0);
    square(&mut p, 2.0);
    let bytes = p.finish();

    // The xref offsets are byte offsets, so search the raw bytes, not a lossy string.
    let find = |needle: &[u8], from: usize| {
        bytes[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|i| i + from)
            .unwrap()
    };
    let startxref = find(b"startxref\n", 0) + b"startxref\n".len();
    let xref_at: usize = std::str::from_utf8(&bytes[startxref..])
        .unwrap()
        .split('\n')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    let table = std::str::from_utf8(&bytes[xref_at..]).unwrap();
    let offsets: Vec<usize> = table
        .lines()
        .filter(|l| l.len() == 19 && l.ends_with(" n "))
        .map(|l| l[..10].parse().unwrap())
        .collect();
    // Objects 1..=6: catalog, pages, two pages and two content streams.
    assert_eq!(offsets.len(), 6);
    for (i, off) in offsets.iter().enumerate() {
        let header = format!("{} 0 obj", i + 1);
        assert!(
            bytes[*off..].starts_with(header.as_bytes()),
            "object {} at byte {off} does not start with {header:?}",
            i + 1
        );
    }
}
