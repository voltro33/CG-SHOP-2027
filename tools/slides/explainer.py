"""The extra figures of docs/how-it-works.md, written to docs/figures/explained/.

- why the order of the pieces matters (two real tours, checked to mow all);
- the idea behind accepting a worse move (a drawn landscape, an illustration);
- three charts of real project numbers: where the length goes, the score
  over time, and the points each kind of run paid per CPU hour.

    python tools/slides/explainer.py
"""

import math

from figures import (
    INK,
    NEW,
    OUTSIDE,
    ROOT,
    SECOND,
    TWICE,
    Lawn,
    Svg,
    check_covers,
    length,
)

OUT = ROOT / "docs" / "figures" / "explained"
FONT = 'font-family="Segoe UI, Helvetica, Arial, sans-serif"'
MUTED = "#66707a"
GRIDLINE = "#e3e6ea"


def text(x, y, words, size=14, colour=INK, anchor="start", weight=400):
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" {FONT} font-size="{size}" font-weight="{weight}" '
        f'fill="{colour}" text-anchor="{anchor}">{words}</text>'
    )


def save(name, width, height, parts):
    OUT.mkdir(parents=True, exist_ok=True)
    body = "\n".join(parts)
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
        f'width="{width}" height="{height}">\n<rect width="{width}" height="{height}" fill="#fff"/>\n'
        f"{body}\n</svg>\n"
    )
    (OUT / f"{name}.svg").write_text(svg, encoding="utf-8")
    print(f"{name}.svg")


# ---------------------------------------------------------------- order


def order_matters():
    """Two islands of lawn, four lanes each. Hopping between the islands
    after every lane, against finishing one island first."""
    left = {(x, y) for x in range(9) for y in range(12)}
    right = {(x, y) for x in range(21, 30) for y in range(12)}
    lawn = Lawn(30, 12, left | right, set())
    rows = [1, 4, 7, 10]

    # one lane on the left island, then the same lane on the right, and back
    hopping = []
    for i, y in enumerate(rows):
        ends = [(1, y), (7, y), (22, y), (28, y)]
        hopping.extend(ends if i % 2 == 0 else ends[::-1])

    # the whole left island, one crossing, the whole right island, one back
    island_first = []
    for i, y in enumerate(rows):
        island_first.extend([(1, y), (7, y)] if i % 2 == 0 else [(7, y), (1, y)])
    for i, y in enumerate(reversed(rows)):
        island_first.extend([(22, y), (28, y)] if i % 2 == 0 else [(28, y), (22, y)])

    svg = Svg(lawn.width, lawn.height, panels=2, gap=2)
    for panel, tour in enumerate([hopping, island_first]):
        check_covers(lawn, [tour], f"order {panel}")
        svg.grid(panel, lawn)
        svg.tour(panel, tour)
    svg.save("order-matters", OUT)
    print("   lengths", length(hopping), length(island_first))


# ---------------------------------------------------------------- landscape


def landscape():
    """Tour length drawn as hills: only going downhill stops in the first
    dip; accepting small climbs reaches a deeper valley. An illustration."""
    width, height = 760, 320
    left, right, top, bottom = 40, 730, 40, 250

    def ground(t):
        """Tour length: a shallow valley, a hill, then a deep valley."""
        shallow = 0.30 * math.exp(-(((t - 0.24) / 0.09) ** 2))
        deep = 0.60 * math.exp(-(((t - 0.72) / 0.11) ** 2))
        return 0.85 - shallow - deep

    def point(t):
        """Longer tours are drawn higher up."""
        return left + t * (right - left), bottom - ground(t) * (bottom - top)

    samples = [i / 200 for i in range(201)]
    curve = " L".join(f"{x:.1f} {y:.1f}" for x, y in map(point, samples))

    # the first dip from the start, and the deepest point overall
    start = 0.04
    stuck = min((t for t in samples if t <= 0.45), key=ground)
    deepest = min(samples, key=ground)

    parts = [
        f'<path d="M{curve}" fill="none" stroke="{MUTED}" stroke-width="3"/>',
        text(
            left,
            22,
            "illustration: each point is a tour; height = its length",
            13,
            MUTED,
        ),
    ]
    sx, sy = point(start)
    parts.append(f'<circle cx="{sx:.1f}" cy="{sy:.1f}" r="8" fill="{INK}"/>')
    parts.append(text(sx, sy - 16, "start", 14, INK, "middle", 600))

    tx, ty = point(stuck)
    parts.append(f'<circle cx="{tx:.1f}" cy="{ty:.1f}" r="8" fill="{OUTSIDE}"/>')
    parts.append(
        text(tx, ty + 30, "only downhill: stuck here", 14, "#b45309", "middle", 600)
    )

    dx, dy = point(deepest)
    parts.append(f'<circle cx="{dx:.1f}" cy="{dy:.1f}" r="8" fill="{NEW}"/>')
    parts.append(
        text(
            dx, dy + 30, "small climbs allowed: gets here", 14, "#2f7d1f", "middle", 600
        )
    )
    parts.append(text(right, top + 8, "longer tour", 12, MUTED, "end"))
    parts.append(text(right, bottom + 40, "shorter tour", 12, MUTED, "end"))

    hop = [
        point(t)
        for t in [
            i / 100 for i in range(round(stuck * 100), round(deepest * 100) + 1, 3)
        ]
    ]
    hops = " L".join(f"{x:.1f} {y - 14:.1f}" for x, y in hop)
    parts.append(
        f'<path d="M{hops}" fill="none" stroke="{NEW}" stroke-width="2.5" stroke-dasharray="6 5"/>'
    )

    save("landscape", width, height, parts)


# ---------------------------------------------------------------- charts


def waste_bar():
    """Where the length of all 250 best tours goes (measured 2026-11-15)."""
    shares = [
        ("mows new lawn", 75.4, NEW),
        ("mows past the edge", 11.1, OUTSIDE),
        ("mows lawn again", 6.8, TWICE),
        ("drives over mowed lawn", 6.7, "#c9ced4"),
    ]
    width, height, left, bar_w = 760, 150, 20, 720
    parts = []

    x = left
    for _name, share, colour in shares:
        w = bar_w * share / 100
        parts.append(
            f'<rect x="{x:.1f}" y="30" width="{w:.1f}" height="44" fill="{colour}"/>'
        )
        if share > 20:
            parts.append(text(x + w / 2, 58, f"{share}%", 16, INK, "middle", 700))
        x += w

    x = left
    for name, share, colour in shares:
        parts.append(f'<rect x="{x}" y="100" width="14" height="14" fill="{colour}"/>')
        label = f"{name} {share}%"
        parts.append(text(x + 20, 112, label, 13, INK))
        x += 22 + 7.2 * len(label)

    save("waste-bar", width, height, parts)


def score_line():
    """The pessimistic score after each step (docs/timeline.md)."""
    points = [
        ("10-29", 110.0, "first mining"),
        ("10-30", 112.17, ""),
        ("10-31", 113.10, ""),
        ("11-13", 113.57, "Rust port"),
        ("11-13", 132.29, "corner moves + ruin and recreate"),
        ("11-14", 136.34, "faster search"),
        ("11-15", 137.91, ""),
        ("11-16", 140.05, ""),
        ("11-17", 141.06, "temperature cap"),
        ("11-18", 141.18, ""),
    ]
    width, height = 760, 340
    left, right, top, bottom = 60, 730, 30, 280
    low, high = 105, 145

    def xy(i, score):
        x = left + i * (right - left) / (len(points) - 1)
        y = bottom - (score - low) / (high - low) * (bottom - top)
        return x, y

    parts = []
    for tick in range(low, high + 1, 10):
        _, y = xy(0, tick)
        parts.append(
            f'<line x1="{left}" x2="{right}" y1="{y:.1f}" y2="{y:.1f}" stroke="{GRIDLINE}"/>'
        )
        parts.append(text(left - 8, y + 5, str(tick), 12, MUTED, "end"))

    line = " L".join(
        f"{x:.1f} {y:.1f}" for x, y in (xy(i, s) for i, (_, s, _) in enumerate(points))
    )
    parts.append(f'<path d="M{line}" fill="none" stroke="{INK}" stroke-width="3"/>')

    for i, (date, score, label) in enumerate(points):
        x, y = xy(i, score)
        parts.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="5" fill="{INK}"/>')
        parts.append(text(x, bottom + 20, date, 11, MUTED, "middle"))
        if label:
            if i == 4:
                # the big jump: the label sits left of its point, clear of the line
                parts.append(text(x - 12, y + 4, label, 12, SECOND, "end", 600))
                continue
            above = i in (5, 8)
            parts.append(
                text(x, y - 12 if above else y + 22, label, 12, SECOND, "middle", 600)
            )
    parts.append(text(left, 18, "score (of 250), pessimistic", 12, MUTED))

    save("score-over-time", width, height, parts)


def points_per_hour():
    """Points each kind of run paid per CPU hour, log scale (docs/timeline.md)."""
    rows = [
        ("first ruin and recreate pass (60 s)", 2.28),
        ("corner moves on today's bests", 0.61),
        ("ruin and recreate, 300 s pass", 0.16),
        ("mining: groups round", 0.073),
        ("ruin and recreate, pass 4", 0.06),
        ("polish (exact solver on windows)", 0.007),
        ("ruin and recreate, pass 6", 0.004),
        ("ruin and recreate, pass 8", 0.0015),
    ]
    width, height = 760, 40 + 34 * len(rows)
    label_w, left, right = 300, 310, 700
    low, high = math.log10(0.001), math.log10(3)

    parts = []
    for i, (name, value) in enumerate(rows):
        y = 20 + i * 34
        w = (math.log10(value) - low) / (high - low) * (right - left)
        colour = INK if value >= 0.002 else MUTED
        parts.append(text(label_w, y + 17, name, 13, INK, "end"))
        parts.append(
            f'<rect x="{left}" y="{y}" width="{w:.1f}" height="22" fill="{colour}"/>'
        )
        parts.append(text(left + w + 8, y + 17, f"{value:g}", 13, INK, "start", 600))

    stop = left + (math.log10(0.002) - low) / (high - low) * (right - left)
    parts.append(
        f'<line x1="{stop:.1f}" x2="{stop:.1f}" y1="10" y2="{height - 12}" stroke="{SECOND}" stroke-dasharray="5 4"/>'
    )
    parts.append(text(stop + 4, height - 4, "stop line: 0.002", 12, SECOND))

    save("points-per-hour", width, height, parts)


def main():
    order_matters()
    landscape()
    waste_bar()
    score_line()
    points_per_hour()


if __name__ == "__main__":
    main()
