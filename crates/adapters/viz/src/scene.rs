//! One picture: the region, and whatever a step wants to show on top of it.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use mow_core::{Axis, BitGrid, Order, Point, Problem, Route, Segment};

use crate::legend::{self, Facts, Line};
use crate::palette;
use crate::region::{merged_rects, CellRect};
use crate::svg::{escape, Paths};
use crate::view::View;

/// What to draw, borrowed. Built with chained calls, then rendered:
///
/// ```no_run
/// # use mow_core::{Problem, Solution};
/// # use mow_viz::Scene;
/// # fn draw(problem: &Problem, solution: &Solution) -> std::io::Result<()> {
/// Scene::new(problem).routes(solution.routes()).save("out.svg")?;
/// # Ok(())
/// # }
/// ```
///
/// Layers, bottom to top: region, uncovered cells, segments, order links,
/// routes with a dot at each start, legend.
#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    problem: &'a Problem,
    title: Option<&'a str>,
    segments: Option<&'a [Segment]>,
    order: Option<&'a Order>,
    routes: &'a [Route],
    uncovered: Option<&'a BitGrid>,
}

impl<'a> Scene<'a> {
    /// The region alone.
    pub const fn new(problem: &'a Problem) -> Self {
        Self {
            problem,
            title: None,
            segments: None,
            order: None,
            routes: &[],
            uncovered: None,
        }
    }

    /// Text shown after the uid in the first legend line.
    pub const fn title(self, title: &'a str) -> Self {
        Self {
            title: Some(title),
            ..self
        }
    }

    pub const fn segments(self, segments: &'a [Segment]) -> Self {
        Self {
            segments: Some(segments),
            ..self
        }
    }

    /// The segments, and the links between them in `order`. Visits naming
    /// a segment outside `segments` are skipped.
    pub const fn order(self, segments: &'a [Segment], order: &'a Order) -> Self {
        Self {
            segments: Some(segments),
            order: Some(order),
            ..self
        }
    }

    /// One route per cutter, colored by index.
    pub const fn routes(self, routes: &'a [Route]) -> Self {
        Self { routes, ..self }
    }

    /// Cells the routes miss, as computed by the caller.
    pub const fn uncovered(self, cells: &'a BitGrid) -> Self {
        Self {
            uncovered: Some(cells),
            ..self
        }
    }

    /// Streams the SVG into `out`. Many small writes: pass a buffered writer.
    pub fn render<W: Write>(&self, mut out: W) -> io::Result<()> {
        let lines = legend::lines(&self.facts());
        let longest = lines.iter().map(|l| l.text.chars().count()).max().unwrap_or(0);
        let view = View::new(self.problem, self.anchors(), lines.len(), longest);

        open(&mut out, &view)?;
        cells(&mut out, &view, self.problem.region(), "region", palette::REGION)?;
        if let Some(grid) = self.uncovered {
            cells(&mut out, &view, grid, "uncovered", palette::UNCOVERED)?;
        }

        if let Some(segments) = self.segments {
            segment_lines(&mut out, &view, segments)?;
            if let Some(order) = self.order {
                links(&mut out, &view, segments, order)?;
            }
        }
        for (index, route) in self.routes.iter().enumerate() {
            route_path(&mut out, &view, index, route)?;
        }

        legend_text(&mut out, &view, &lines)?;
        out.write_all(b"</svg>\n")?;
        out.flush()
    }

    /// The whole SVG as text. For small pictures and tests.
    pub fn to_svg(&self) -> String {
        let mut bytes = Vec::new();
        self.render(&mut bytes)
            .expect("writing into memory does not fail");

        String::from_utf8(bytes).expect("the SVG is written as UTF-8")
    }

    /// Writes the SVG to the file `path`, replacing it.
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let file = File::create(path)?;
        self.render(BufWriter::with_capacity(1 << 16, file))
    }

    fn facts(&self) -> Facts<'a> {
        Facts {
            problem: self.problem,
            title: self.title,
            segments: self.segments.map(<[Segment]>::len),
            routes: self.routes,
            uncovered: self.uncovered.map_or(0, BitGrid::len),
        }
    }

    /// Every point drawn outside the region's cells: route corners and segment ends.
    fn anchors(&self) -> impl Iterator<Item = Point> + 'a {
        let corners = self.routes.iter().flat_map(|r| r.corners().iter().copied());
        let ends = self
            .segments
            .unwrap_or(&[])
            .iter()
            .flat_map(|s| [s.start(), s.end()]);

        corners.chain(ends)
    }
}

fn open<W: Write>(out: &mut W, view: &View) -> io::Result<()> {
    let (x, y, w, h) = view.view_box();
    let (pw, ph) = view.pixels();

    writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
    writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x} {y} {w} {h}\" width=\"{pw}\" height=\"{ph}\">"
    )?;
    writeln!(
        out,
        "<rect class=\"background\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
        palette::BACKGROUND
    )
}

/// The cells of `grid` as merged rectangles, one subpath each.
fn cells<W: Write>(out: &mut W, view: &View, grid: &BitGrid, class: &str, fill: &str) -> io::Result<()> {
    let attributes =
        format!("class=\"{class}\" fill=\"{fill}\" stroke=\"none\" shape-rendering=\"crispEdges\"");
    let mut paths = Paths::new(out, attributes);
    let mut result = Ok(());

    merged_rects(grid, |rect: CellRect| {
        if result.is_ok() {
            let (x, y) = (view.x(rect.x_min), view.y(rect.y_max + 1));
            let w = View::cells_to_units(i64::from(rect.x_max - rect.x_min) + 1);
            let h = View::cells_to_units(i64::from(rect.y_max - rect.y_min) + 1);
            result = paths.push(format_args!("M{x} {y}h{w}v{h}h-{w}z"));
        }
    });

    result?;
    paths.finish().map(drop)
}

fn segment_lines<W: Write>(out: &mut W, view: &View, segments: &[Segment]) -> io::Result<()> {
    let attributes = stroke("segments", palette::SEGMENT, "1.2", "");
    let mut paths = Paths::new(out, attributes);

    for segment in segments {
        let (x, y) = view.anchor(segment.start());
        let (ex, ey) = view.anchor(segment.end());
        match segment.axis() {
            Axis::Horizontal => paths.push(format_args!("M{x} {y}H{ex}"))?,
            Axis::Vertical => paths.push(format_args!("M{x} {y}V{ey}"))?,
        }
    }

    paths.finish().map(drop)
}

/// Straight lines from each visit's exit to the next visit's entry.
fn links<W: Write>(out: &mut W, view: &View, segments: &[Segment], order: &Order) -> io::Result<()> {
    let attributes = stroke("links", palette::LINK, "0.8", " stroke-dasharray=\"4 3\"");
    let mut paths = Paths::new(out, attributes);

    let ends: Vec<(Point, Point)> = order
        .visits()
        .iter()
        .filter_map(|v| segments.get(v.segment as usize).map(|s| s.ends(v.reversed)))
        .collect();

    for pair in ends.windows(2) {
        let (x, y) = view.anchor(pair[0].1);
        let (nx, ny) = view.anchor(pair[1].0);
        paths.push(format_args!("M{x} {y}L{nx} {ny}"))?;
    }

    paths.finish().map(drop)
}

/// One closed path through the route's corners, and a dot at its start.
fn route_path<W: Write>(out: &mut W, view: &View, index: usize, route: &Route) -> io::Result<()> {
    let color = palette::route_color(index);
    let attributes = stroke("route", color, "1.6", &format!(" data-cutter=\"{index}\""));

    let (sx, sy) = view.anchor(route.start());
    write!(out, "<path {attributes} d=\"M{sx} {sy}")?;

    for (a, b) in route.edges().take(route.corners().len().saturating_sub(1)) {
        let (x, y) = view.anchor(b);
        if b.x == a.x && b.y != a.y {
            write!(out, "V{y}")?;
        } else if b.x != a.x {
            write!(out, "H{x}")?;
        }
    }
    writeln!(out, "Z\"/>")?;

    let r = view.mark();
    writeln!(
        out,
        "<circle class=\"start\" cx=\"{sx}\" cy=\"{sy}\" r=\"{r}\" fill=\"{color}\"/>"
    )
}

fn stroke(class: &str, color: &str, width: &str, extra: &str) -> String {
    format!(
        "class=\"{class}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width}\" \
         stroke-linecap=\"round\" stroke-linejoin=\"round\" vector-effect=\"non-scaling-stroke\"{extra}"
    )
}

fn legend_text<W: Write>(out: &mut W, view: &View, lines: &[Line]) -> io::Result<()> {
    let (x, size) = (view.font(), view.font());

    for (index, line) in lines.iter().enumerate() {
        let y = view.line_y(index);
        writeln!(
            out,
            "<text class=\"legend\" x=\"{x}\" y=\"{y}\" font-size=\"{size}\" font-family=\"sans-serif\" \
             fill=\"{}\">{}</text>",
            line.color,
            escape(&line.text)
        )?;
    }

    Ok(())
}
