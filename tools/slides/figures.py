"""Draws the deck's figures, one clean SVG each, into docs/slides/figures/.

The tours come from real solver runs on the toy lawn in docs/slides/toy/
(each passed the official verifier). The small concept pictures are built
here, and each one is checked to mow its whole lawn before it is drawn.

    python tools/slides/figures.py
"""

import json
from itertools import pairwise
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOY = ROOT / "docs" / "slides" / "toy"
OUT = ROOT / "docs" / "slides" / "figures"

CELL = 20
MARGIN = 14

LAWN = "#d4ebc8"
LAWN_LINE = "#b9d9ab"
POND = "#a9d2ef"
MOWED = "#9fd08a"
NEW = "#5fae46"
TWICE = "#f5d76e"
OUTSIDE = "#f4a46a"
MISSED = "#e05a4f"
INK = "#1f3a5f"
SECOND = "#b8432f"
FAINT = "#9aa5b1"
WINDOW = "#1f3a5f"


# ---------------------------------------------------------------- geometry


def inside(px, py, ring):
    """Whether the point lies inside the polygon `ring` (ray casting)."""
    xs, ys = ring["x"], ring["y"]
    hit = False

    j = len(xs) - 1
    for i in range(len(xs)):
        if (ys[i] > py) != (ys[j] > py):
            cross = xs[i] + (py - ys[i]) * (xs[j] - xs[i]) / (ys[j] - ys[i])
            if px < cross:
                hit = not hit
        j = i

    return hit


class Lawn:
    """The cells to mow, the holes in it, and the square cutter."""

    def __init__(self, width, height, cells, holes, reach=1):
        self.width = width
        self.height = height
        self.cells = cells
        self.holes = holes
        self.reach = reach

    @classmethod
    def from_instance(cls, path):
        data = json.loads(path.read_text())
        region = data["region_to_cover"]
        outer, inner = region["outer_boundary"], region["inner_boundaries"]

        width, height = max(outer["x"]), max(outer["y"])
        cells, holes = set(), set()
        for x in range(width):
            for y in range(height):
                centre = (x + 0.5, y + 0.5)
                if not inside(*centre, outer):
                    continue
                if any(inside(*centre, hole) for hole in inner):
                    holes.add((x, y))
                else:
                    cells.add((x, y))

        return cls(width, height, cells, holes)

    @classmethod
    def rectangle(cls, width, height):
        cells = {(x, y) for x in range(width) for y in range(height)}
        return cls(width, height, cells, set())

    def stamp(self, x, y):
        r = self.reach
        return {(x + dx, y + dy) for dx in range(-r, r + 1) for dy in range(-r, r + 1)}


def unit_points(tour):
    """Every grid point a closed tour passes, corner to corner."""
    points = []

    for (x0, y0), (x1, y1) in pairwise(closed(tour)):
        steps = abs(x1 - x0) + abs(y1 - y0)
        dx = (x1 > x0) - (x1 < x0)
        dy = (y1 > y0) - (y1 < y0)
        points.extend((x0 + dx * s, y0 + dy * s) for s in range(steps))

    return points or list(tour)


def closed(tour):
    """The corners with the first one again at the end."""
    return [*tour, tour[0]]


def length(tour):
    return sum(
        abs(x1 - x0) + abs(y1 - y0) for (x0, y0), (x1, y1) in pairwise(closed(tour))
    )


def swept_counts(lawn, tours):
    """How many separate times each cell comes under the cutter, over all
    tours: a visit starts when the cutter reaches a cell it was not on one
    step before."""
    visits = {}
    for tour in tours:
        points = unit_points(tour)
        before = lawn.stamp(*points[-1])

        for point in points:
            now = lawn.stamp(*point)
            for cell in now - before:
                visits[cell] = visits.get(cell, 0) + 1
            before = now

        # a tour that never moves still mows its one stamp
        if len(points) == 1:
            for cell in before:
                visits[cell] = visits.get(cell, 0) + 1
    return visits


def check_covers(lawn, tours, name):
    missed = lawn.cells - set(swept_counts(lawn, tours))
    assert not missed, f"{name}: {len(missed)} cells not mowed"


def load_tours(name):
    data = json.loads((TOY / "solutions" / f"{name}.solution.json").read_text())
    return [list(zip(t["x"], t["y"], strict=True)) for t in data["tours"]]


# ---------------------------------------------------------------- drawing


class Svg:
    """One figure: a lawn's grid, in cells, y pointing up."""

    def __init__(self, width, height, panels=1, gap=2):
        self.cols = width
        self.rows = height
        self.gap = gap
        self.panels = panels
        self.parts = []

    def size(self):
        w = (
            self.panels * self.cols * CELL
            + (self.panels - 1) * self.gap * CELL
            + 2 * MARGIN
        )
        h = self.rows * CELL + 2 * MARGIN
        return w, h

    def origin(self, panel):
        return MARGIN + panel * (self.cols + self.gap) * CELL

    def xy(self, panel, x, y):
        """The pixel of the grid point (x, y) of a panel."""
        return self.origin(panel) + x * CELL, MARGIN + (self.rows - y) * CELL

    def cell(self, panel, x, y, fill, opacity=1.0):
        if not (0 <= x < self.cols and 0 <= y < self.rows):
            return
        px, py = self.xy(panel, x, y + 1)
        self.parts.append(
            f'<rect x="{px}" y="{py}" width="{CELL}" height="{CELL}" fill="{fill}" fill-opacity="{opacity}"/>'
        )

    def grid(self, panel, lawn):
        for x, y in lawn.cells:
            self.cell(panel, x, y, LAWN)
        for x, y in lawn.holes:
            self.cell(panel, x, y, POND)

        x0, y0 = self.xy(panel, 0, self.rows)
        lines = [
            f"M{x0 + x * CELL} {y0}v{self.rows * CELL}" for x in range(self.cols + 1)
        ]
        lines += [
            f"M{x0} {y0 + y * CELL}h{self.cols * CELL}" for y in range(self.rows + 1)
        ]
        self.parts.append(
            f'<path d="{" ".join(lines)}" stroke="{LAWN_LINE}" stroke-width="0.6" fill="none"/>'
        )

    def shade(self, panel, lawn, counts, outside=True, twice=True):
        """Cells off the lawn under the cutter, and lawn cells mowed twice."""
        for (x, y), n in counts.items():
            on_lawn = (x, y) in lawn.cells
            if outside and not on_lawn:
                self.cell(panel, x, y, OUTSIDE, 0.85)
            elif twice and on_lawn and n >= 2:
                self.cell(panel, x, y, TWICE, 0.9)

    def tour(self, panel, tour, colour=INK, width=3.0, dash=None, start=True):
        pixels = [self.xy(panel, x + 0.5, y + 0.5) for x, y in closed(tour)]
        d = "M" + " L".join(f"{px:.1f} {py:.1f}" for px, py in pixels)
        dashed = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<path d="{d}" stroke="{colour}" stroke-width="{width}" fill="none" '
            f'stroke-linejoin="round" stroke-linecap="round"{dashed}/>'
        )
        if start:
            px, py = pixels[0]
            self.parts.append(
                f'<circle cx="{px:.1f}" cy="{py:.1f}" r="5" fill="{colour}"/>'
            )

    def path(self, panel, points, colour=INK, width=3.0, dash=None):
        pixels = [self.xy(panel, x + 0.5, y + 0.5) for x, y in points]
        d = "M" + " L".join(f"{px:.1f} {py:.1f}" for px, py in pixels)
        dashed = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<path d="{d}" stroke="{colour}" stroke-width="{width}" fill="none" '
            f'stroke-linejoin="round" stroke-linecap="round"{dashed}/>'
        )

    def box(self, panel, x0, y0, x1, y1, colour=WINDOW, dash="6 4"):
        """A frame around cells x0..x1, y0..y1 (inclusive)."""
        px, py = self.xy(panel, x0, y1 + 1)
        w, h = (x1 - x0 + 1) * CELL, (y1 - y0 + 1) * CELL
        self.parts.append(
            f'<rect x="{px}" y="{py}" width="{w}" height="{h}" fill="none" stroke="{colour}" '
            f'stroke-width="2.5" stroke-dasharray="{dash}"/>'
        )

    def label(self, panel, x, y, text, size=15, colour=INK, anchor="middle"):
        px, py = self.xy(panel, x, y)
        self.parts.append(
            f'<text x="{px:.1f}" y="{py:.1f}" font-family="Segoe UI, Helvetica, Arial, sans-serif" '
            f'font-size="{size}" font-weight="600" fill="{colour}" text-anchor="{anchor}">{text}</text>'
        )

    def save(self, name, folder=OUT):
        w, h = self.size()
        body = "\n".join(self.parts)
        text = (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">\n'
            f'<rect width="{w}" height="{h}" fill="#ffffff"/>\n{body}\n</svg>\n'
        )
        folder.mkdir(parents=True, exist_ok=True)
        (folder / f"{name}.svg").write_text(text, encoding="utf-8")
        print(f"{name}.svg")


# ---------------------------------------------------------------- the problem


def lawn_and_cutter(toy):
    svg = Svg(toy.width + 6, toy.height)
    svg.grid(0, toy)

    for x, y in toy.stamp(toy.width + 3, 10):
        svg.cell(0, x, y, NEW)
    svg.label(0, toy.width + 3.5, 7.6, "cutter", size=14)
    svg.save("01-lawn-and-cutter")


def stamp_and_sweep():
    strip = Lawn.rectangle(12, 5)
    svg = Svg(strip.width, strip.height + 2)
    svg.grid(0, strip)

    for x, y in strip.stamp(2, 2):
        svg.cell(0, x, y, NEW)

    # six steps right: each brings one new column of three cells
    for x in range(4, 10):
        for y in range(1, 4):
            svg.cell(0, x, y, MOWED)
    svg.path(0, [(2, 2), (8, 2)], width=3.5)
    svg.label(0, 2.5, 6.2, "start: 9 cells", size=13)
    svg.label(0, 8.0, 5.4, "each step: 3 new cells", size=13)
    svg.save("02-stamp-and-sweep")


def one_tour(toy):
    tours = load_tours("toy_1s3.lns")
    check_covers(toy, tours, "lns")

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    svg.tour(0, tours[0])
    svg.save("03-one-tour")


def two_cutters(toy):
    tours = load_tours("toy_2s3.lns")
    check_covers(toy, tours, "lns k = 2")

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    svg.tour(0, tours[0], INK)
    svg.tour(0, tours[1], SECOND)
    svg.save("04-two-cutters")
    print("   lengths", [length(t) for t in tours])


# ---------------------------------------------------------------- approaches


def solved(toy, name, file, folder=OUT):
    tours = load_tours(file)
    check_covers(toy, tours, name)

    svg = Svg(toy.width + 2, toy.height + 2)
    shifted = Lawn(
        toy.width + 2,
        toy.height + 2,
        {(x + 1, y + 1) for x, y in toy.cells},
        {(x + 1, y + 1) for x, y in toy.holes},
    )
    svg.grid(0, shifted)
    moved = [[(x + 1, y + 1) for x, y in t] for t in tours]
    svg.shade(0, shifted, swept_counts(shifted, moved))
    for tour, colour in zip(moved, [INK, SECOND], strict=False):
        svg.tour(0, tour, colour)
    svg.save(name, folder)
    print("   longest", max(length(t) for t in tours))


def journey(toy):
    """The k = 1 tours of each stage side by side, the same lawn each time."""
    tours_in_a_row(
        toy,
        "15-journey",
        ["toy_1s3.lanes-h", "toy_1s3.segments-h", "toy_1s3.turns", "toy_1s3.lns"],
    )


def mining(toy):
    """Four settings of the construction; the miner keeps the shortest."""
    tours_in_a_row(
        toy,
        "09-mining",
        [
            "toy_1s3.lanes-v",
            "toy_1s3.segments-v",
            "toy_1s3.lanes-h",
            "toy_1s3.segments-h",
        ],
    )


def tours_in_a_row(toy, name, files):
    svg = Svg(toy.width + 2, toy.height + 2, panels=len(files), gap=1)

    for panel, file in enumerate(files):
        tours = load_tours(file)
        check_covers(toy, tours, file)
        shifted = Lawn(
            toy.width + 2,
            toy.height + 2,
            {(x + 1, y + 1) for x, y in toy.cells},
            {(x + 1, y + 1) for x, y in toy.holes},
        )
        moved = [[(x + 1, y + 1) for x, y in t] for t in tours]
        svg.grid(panel, shifted)
        svg.shade(panel, shifted, swept_counts(shifted, moved))
        svg.tour(panel, moved[0], width=2.6)
    svg.save(name)
    print("   longest", [max(length(t) for t in load_tours(f)) for f in files])


def narrow_lanes():
    """Lanes three apart sweep separate strips; two apart, every lane overlaps."""
    strip = Lawn.rectangle(12, 12)
    svg = Svg(strip.width, strip.height, panels=2)

    good = snake(10, [1, 4, 7, 10])
    tight = snake(10, [1, 3, 5, 7, 9, 10])
    for panel, tour in enumerate([good, tight]):
        check_covers(strip, [tour], f"lanes {panel}")
        svg.grid(panel, strip)
        svg.shade(panel, strip, swept_counts(strip, [tour]), outside=False)
        svg.tour(panel, tour)
    svg.save("20-narrow-lanes")
    print("   lengths", length(good), length(tight))


def snake(right, rows):
    """Lanes along `rows`, from x = 1 to `right` and back, alternating."""
    corners = []
    for i, y in enumerate(rows):
        ends = [(1, y), (right, y)]
        corners.extend(ends if i % 2 == 0 else ends[::-1])
    return corners


def cover_then_tour(toy):
    """Stamp points on a 3-grid that touch the lawn, visited nearest first."""
    points = [
        (x, y)
        for x in range(1, toy.width + 1, 3)
        for y in range(1, toy.height + 1, 3)
        if toy.stamp(x, y) & toy.cells
    ]

    order = [points.pop(0)]
    while points:
        last = order[-1]
        nearest = min(points, key=lambda p: abs(p[0] - last[0]) + abs(p[1] - last[1]))
        points.remove(nearest)
        order.append(nearest)
    order = two_opt(order)

    corners = []
    for (x0, y0), (x1, y1) in pairwise(closed(order)):
        corners.append((x0, y0))
        if x0 != x1 and y0 != y1:
            corners.append((x1, y0))
    check_covers(toy, [corners], "cover then tour")

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    for x, y in order:
        px, py = svg.xy(0, x + 0.5, y + 0.5)
        svg.parts.append(f'<circle cx="{px}" cy="{py}" r="4" fill="{SECOND}"/>')
    svg.tour(0, corners, width=2.4, start=False)
    svg.save("21-cover-then-tour")
    print("   longest", length(corners))


def two_opt(order):
    def cost(route):
        return sum(
            abs(a[0] - b[0]) + abs(a[1] - b[1]) for a, b in pairwise(closed(route))
        )

    best, improved = list(order), True
    while improved:
        improved = False
        for i in range(1, len(best) - 1):
            for j in range(i + 1, len(best)):
                candidate = best[:i] + best[i : j + 1][::-1] + best[j + 1 :]
                if cost(candidate) < cost(best):
                    best, improved = candidate, True
    return best


# ---------------------------------------------------------------- searches


def slide():
    """A U turn past the lawn's edge, then slid back in by two steps."""
    lawn = Lawn(13, 7, {(x, y) for x in range(10) for y in range(6)}, set())
    before = [(1, 1), (10, 1), (10, 4), (1, 4)]
    after = [(1, 1), (8, 1), (8, 4), (1, 4)]

    svg = Svg(lawn.width, lawn.height, panels=2)
    for panel, tour in enumerate([before, after]):
        check_covers(lawn, [tour], f"slide {panel}")
        svg.grid(panel, lawn)
        svg.shade(panel, lawn, swept_counts(lawn, [tour]), twice=False)
        svg.tour(panel, tour)
    svg.save("16-slide")
    print("   lengths", length(before), length(after))


def ruin_and_recreate(toy):
    """The segments tour; the part inside a window taken out; the cells it
    leaves unmowed. Then the tour after the search."""
    tours = load_tours("toy_1s3.segments-h")
    window = (2, 9, 10, 17)
    x0, y0, x1, y1 = window

    def in_window(p):
        return x0 <= p[0] <= x1 and y0 <= p[1] <= y1

    kept = [p for p in unit_points(tours[0]) if not in_window(p)]
    mowed = set()
    for p in kept:
        mowed |= toy.stamp(*p)
    missed = toy.cells - mowed

    svg = Svg(toy.width, toy.height, panels=3, gap=1)
    svg.grid(0, toy)
    svg.tour(0, tours[0])
    svg.box(0, x0 - 1, y0 - 1, x1 + 1, y1 + 1)

    svg.grid(1, toy)
    for x, y in missed:
        svg.cell(1, x, y, MISSED, 0.85)
    svg.tour(1, tours[0], colour=FAINT, width=2.0, dash="3 5", start=False)
    runs, run = [], []
    for p in [*unit_points(tours[0]), tours[0][0]]:
        if in_window(p):
            if len(run) > 1:
                runs.append(run)
            run = []
        else:
            run.append(p)
    if len(run) > 1:
        runs.append(run)
    for piece in runs:
        svg.path(1, piece)
    svg.box(1, x0 - 1, y0 - 1, x1 + 1, y1 + 1)

    after = load_tours("toy_1s3.lns")
    svg.grid(2, toy)
    svg.tour(2, after[0])
    svg.save("17-ruin-and-recreate")
    print("   missed cells", len(missed))


def main():
    toy = Lawn.from_instance(TOY / "instances" / "toy_1s3.instance.json")
    print(f"toy lawn: {len(toy.cells)} cells, pond {len(toy.holes)}")

    lawn_and_cutter(toy)
    stamp_and_sweep()
    one_tour(toy)
    two_cutters(toy)

    solved(toy, "10-plain-lanes", "toy_1s3.lanes-h")
    solved(toy, "11-lane-segments", "toy_1s3.segments-h")
    solved(toy, "12-split-walk", "toy_2s3.segments-h")
    solved(toy, "13-turns", "toy_1s3.turns")
    solved(toy, "14-lns", "toy_1s3.lns")
    solved(toy, "18-lns-two-cutters", "toy_2s3.lns")
    mining(toy)
    journey(toy)
    slide()
    ruin_and_recreate(toy)
    narrow_lanes()
    cover_then_tour(toy)


if __name__ == "__main__":
    main()
