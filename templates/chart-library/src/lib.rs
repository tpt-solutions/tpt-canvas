//! A small bar chart that draws onto any backend.
//!
//! The chart only uses `Context` calls, so the same `BarChart` can be written to SVG, PDF,
//! pixels, or a browser canvas. Labels use the built-in stroke font.

use tpt_canvas::prelude::*;
use tpt_canvas::text::StrokeFont;

/// A bar chart: a title-free list of labelled values.
#[derive(Clone, Debug, PartialEq)]
pub struct BarChart {
    /// `(label, value)` pairs, drawn left to right.
    pub bars: Vec<(String, f32)>,
    /// The bar colour.
    pub color: Color,
}

impl BarChart {
    /// Creates a chart from labels and values, with a default blue.
    pub fn new(bars: Vec<(String, f32)>) -> Self {
        BarChart {
            bars,
            color: Color::from_hex("#3366cc").unwrap_or(named::BLUE),
        }
    }

    /// Draws the chart into a `width` by `height` area at the origin. The largest value fills
    /// the plot height, and values below zero are drawn as nothing.
    pub fn draw<B: CanvasBackend>(&self, ctx: &mut Context<B>, width: f32, height: f32) {
        const LABEL_SIZE: f32 = 10.0;
        let label_band = LABEL_SIZE * 2.0;
        let plot_h = (height - label_band).max(1.0);
        let max = self
            .bars
            .iter()
            .map(|(_, v)| *v)
            .fold(0.0_f32, f32::max)
            .max(f32::MIN_POSITIVE);
        let slot = width / self.bars.len().max(1) as f32;

        ctx.set_glyph_source(Box::new(StrokeFont));
        for (i, (label, value)) in self.bars.iter().enumerate() {
            let x = slot * i as f32;
            let h = plot_h * value.max(0.0) / max;
            ctx.set_fill_paint(Paint::solid(self.color));
            ctx.fill_rect(x + slot * 0.15, plot_h - h, slot * 0.7, h);

            ctx.set_fill_paint(Paint::solid(named::BLACK));
            let mut run = StrokeFont::layout(label, LABEL_SIZE);
            for g in &mut run.glyphs {
                g.pos.x += x + slot * 0.15;
                g.pos.y += plot_h + LABEL_SIZE * 1.4;
            }
            ctx.draw_glyphs(&run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_canvas_raster::RasterBackend;

    #[test]
    fn tallest_bar_reaches_the_top_of_the_plot() {
        let chart = BarChart::new(vec![("A".into(), 1.0), ("B".into(), 4.0)]);
        let mut ctx = Context::new(RasterBackend::new(60, 60));
        chart.draw(&mut ctx, 60.0, 60.0);
        let pixmap = ctx.into_backend().into_pixmap();
        // The tallest bar's centre column is coloured near the top of the plot.
        let p = pixmap.pixel(42, 2);
        assert!(p[3] > 0.9, "tallest bar should reach the top: {p:?}");
    }

    #[test]
    fn empty_chart_draws_nothing() {
        let chart = BarChart::new(vec![]);
        let mut ctx = Context::new(RasterBackend::new(20, 20));
        chart.draw(&mut ctx, 20.0, 20.0);
        let ink: f32 = ctx.backend().pixmap().data.iter().map(|p| p[3]).sum();
        assert_eq!(ink, 0.0);
    }
}
