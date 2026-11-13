//! Where the world lands in the picture.
//!
//! SVG units are half cells, so cell corners and anchor centers (which sit
//! on half cells for odd cutters) are both whole numbers. SVG `y` grows
//! downward, world `y` upward: `svg_y = top - 2 y`. The legend sits in a
//! band above the plot, at negative `svg_y`.

use mow_core::{Point, Problem};

/// SVG units per cell.
const UNIT: i64 = 2;

/// The long side of the picture on screen, in pixels.
const PIXELS: i64 = 1000;

/// Legend font size as a fraction of the plot's long side.
const FONT_DIVISOR: i64 = 45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    left: i64,
    top: i64,
    width: i64,
    height: i64,
    header: i64,
    font: i64,
    center: (i64, i64),
}

impl View {
    /// A view of the region and every point in `anchors`, with room for
    /// `lines` legend lines of at most `chars` characters.
    pub fn new(problem: &Problem, anchors: impl Iterator<Item = Point>, lines: usize, chars: usize) -> Self {
        let c = problem.cutter().bounds();
        let center = (i64::from(c.x_min + c.x_max + 1), i64::from(c.y_min + c.y_max + 1));

        let r = problem.region_bounds();
        let start = [
            UNIT * i64::from(r.x_min),
            UNIT * i64::from(r.y_min),
            UNIT * (i64::from(r.x_max) + 1),
            UNIT * (i64::from(r.y_max) + 1),
        ];
        let [x0, y0, x1, y1] = anchors.fold(start, |[x0, y0, x1, y1], p| {
            let (x, y) = (UNIT * i64::from(p.x) + center.0, UNIT * i64::from(p.y) + center.1);
            [x0.min(x), y0.min(y), x1.max(x), y1.max(y)]
        });

        let long = (x1 - x0).max(y1 - y0);
        let margin = long / 40 + UNIT;
        let font = (long / FONT_DIVISOR).max(1);

        let text_width = (chars as i64 * font * 6 + 9) / 10;
        let width = (x1 - x0 + 2 * margin).max(text_width + 2 * margin);
        let header = lines as i64 * font * 14 / 10 + font / 2;

        Self {
            left: x0 - margin,
            top: y1 + margin,
            width,
            height: y1 - y0 + 2 * margin,
            header,
            font,
            center,
        }
    }

    /// The SVG `x` of the world line `x`, the left edge of cell column `x`.
    pub const fn x(&self, x: i32) -> i64 {
        UNIT * x as i64 - self.left
    }

    /// The SVG `y` of the world line `y`, the bottom edge of cell row `y`.
    pub const fn y(&self, y: i32) -> i64 {
        self.top - UNIT * y as i64
    }

    /// Where an anchor is drawn: the center of its cutter's box.
    pub const fn anchor(&self, p: Point) -> (i64, i64) {
        (self.x(p.x) + self.center.0, self.y(p.y) - self.center.1)
    }

    pub const fn cells_to_units(cells: i64) -> i64 {
        UNIT * cells
    }

    pub const fn font(&self) -> i64 {
        self.font
    }

    /// The `y` of legend line `index`, counted from the top, at its baseline.
    pub const fn line_y(&self, index: usize) -> i64 {
        -self.header + self.font * (index as i64 + 1) * 14 / 10
    }

    /// A size for small marks, like the start of a route.
    pub fn mark(&self) -> i64 {
        (self.width.max(self.height) / 150).max(1)
    }

    /// `(min_x, min_y, width, height)` of the SVG `viewBox`.
    pub const fn view_box(&self) -> (i64, i64, i64, i64) {
        (0, -self.header, self.width, self.height + self.header)
    }

    /// `(width, height)` on screen, in pixels.
    pub fn pixels(&self) -> (i64, i64) {
        let (_, _, w, h) = self.view_box();
        let long = w.max(h).max(1);

        ((PIXELS * w / long).max(1), (PIXELS * h / long).max(1))
    }
}
