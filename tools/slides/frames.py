"""Animation frames for the deck: each step or panel of a figure on its own,
the same size and style, so a slide can build them up one after another.

Writes to docs/slides/figures/frames/. Run after figures.py:

    python tools/slides/frames.py
"""

from figures import (
    FAINT,
    INK,
    MISSED,
    MOWED,
    NEW,
    OUT,
    SECOND,
    TOY,
    Lawn,
    Svg,
    check_covers,
    length,
    load_tours,
    snake,
    solved,
    swept_counts,
    unit_points,
)

FRAMES = OUT / "frames"


def stamp_sweep():
    """The cutter's first stamp, then one frame per step to the right."""
    strip = Lawn.rectangle(12, 5)

    for steps in range(7):
        svg = Svg(strip.width, strip.height)
        svg.grid(0, strip)
        for x, y in strip.stamp(2, 2):
            svg.cell(0, x, y, NEW)
        for x in range(4, 4 + steps):
            for y in range(1, 4):
                svg.cell(0, x, y, MOWED)

        if steps:
            svg.path(0, [(2, 2), (2 + steps, 2)], width=3.5)
        svg.box(0, 1 + steps, 1, 3 + steps, 3, colour=INK, dash="none")
        svg.save(f"stamp-sweep-{steps}", FRAMES)


def lawn_alone(toy):
    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    svg.save("lawn", FRAMES)


def two_cutters(toy):
    """Cutter 1's tour, then both."""
    tours = load_tours("toy_2s3.lns")
    check_covers(toy, tours, "two cutters")

    for shown in (1, 2):
        svg = Svg(toy.width, toy.height)
        svg.grid(0, toy)
        for tour, colour in list(zip(tours, [INK, SECOND], strict=True))[:shown]:
            svg.tour(0, tour, colour)
        svg.save(f"two-cutters-{shown}", FRAMES)


def one_per_panel(toy):
    """The multi-panel figures as single frames."""
    journey = ["toy_1s3.lanes-h", "toy_1s3.segments-h", "toy_1s3.turns", "toy_1s3.lns"]
    for i, file in enumerate(journey, 1):
        solved(toy, f"journey-{i}-{file.split('.')[1]}", file, FRAMES)

    mining = [
        "toy_1s3.lanes-v",
        "toy_1s3.segments-v",
        "toy_1s3.lanes-h",
        "toy_1s3.segments-h",
    ]
    for i, file in enumerate(mining, 1):
        solved(toy, f"mining-{i}-{file.split('.')[1]}", file, FRAMES)


def ruin_and_recreate(toy):
    """The tour with its window; the window torn out; the tour after search."""
    tours = load_tours("toy_1s3.segments-h")
    x0, y0, x1, y1 = 2, 9, 10, 17

    def in_window(p):
        return x0 <= p[0] <= x1 and y0 <= p[1] <= y1

    points = unit_points(tours[0])
    mowed = set()
    for p in points:
        if not in_window(p):
            mowed |= toy.stamp(*p)
    missed = toy.cells - mowed

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    svg.tour(0, tours[0])
    svg.save("ruin-1-tour", FRAMES)

    svg.box(0, x0 - 1, y0 - 1, x1 + 1, y1 + 1)
    svg.save("ruin-2-window", FRAMES)

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    for x, y in missed:
        svg.cell(0, x, y, MISSED, 0.85)
    svg.tour(0, tours[0], colour=FAINT, width=2.0, dash="3 5", start=False)
    for piece in pieces_outside([*points, points[0]], in_window):
        svg.path(0, piece)
    svg.box(0, x0 - 1, y0 - 1, x1 + 1, y1 + 1)
    svg.save("ruin-3-torn-out", FRAMES)

    svg = Svg(toy.width, toy.height)
    svg.grid(0, toy)
    svg.tour(0, load_tours("toy_1s3.lns")[0])
    svg.save("ruin-4-after-search", FRAMES)


def pieces_outside(points, in_window):
    """The runs of a tour's points outside the window."""
    runs, run = [], []
    for p in points:
        if in_window(p):
            if len(run) > 1:
                runs.append(run)
            run = []
        else:
            run.append(p)
    if len(run) > 1:
        runs.append(run)
    return runs


def single(lawn, tour, name, twice=True):
    check_covers(lawn, [tour], name)

    svg = Svg(lawn.width, lawn.height)
    svg.grid(0, lawn)
    svg.shade(0, lawn, swept_counts(lawn, [tour]), twice=twice)
    svg.tour(0, tour)
    svg.save(name, FRAMES)
    print(f"   {name}: {length(tour)}")


def slide_and_lanes():
    lawn = Lawn(13, 7, {(x, y) for x in range(10) for y in range(6)}, set())
    single(lawn, [(1, 1), (10, 1), (10, 4), (1, 4)], "slide-1-before", twice=False)
    single(lawn, [(1, 1), (8, 1), (8, 4), (1, 4)], "slide-2-after", twice=False)

    strip = Lawn.rectangle(12, 12)
    single(strip, snake(10, [1, 4, 7, 10]), "narrow-1-three-apart")
    single(strip, snake(10, [1, 3, 5, 7, 9, 10]), "narrow-2-two-apart")


def main():
    toy = Lawn.from_instance(TOY / "instances" / "toy_1s3.instance.json")

    stamp_sweep()
    lawn_alone(toy)
    two_cutters(toy)
    one_per_panel(toy)
    ruin_and_recreate(toy)
    slide_and_lanes()


if __name__ == "__main__":
    main()
