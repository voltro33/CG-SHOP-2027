//! Which way each tile sweeps: a hill climb over single tile flips
//! (`tile_directions` in Python).

use super::tiles::Directions;
use super::view::View;

/// Flips must pay by a little more than rounding, as in Python.
const MIN_GAIN: f64 = 1e-9;

/// Costs per tile row (of `horizontal`) and per tile column (the tile rows
/// of `vertical`, the mirrored view).
struct Costs {
    rows: Vec<f64>,
    columns: Vec<f64>,
}

impl Costs {
    fn of(horizontal: &View, vertical: &View, directions: &Directions, segment_cost: f64) -> Self {
        let rows = (0..horizontal.tiles().rows.count)
            .map(|t| horizontal.strip_cost(t, directions, segment_cost))
            .collect();
        let columns = (0..vertical.tiles().rows.count)
            .map(|s| vertical.strip_cost(s, directions, segment_cost))
            .collect();

        Self { rows, columns }
    }

    /// Summed like Python's `sum(rows) + sum(columns)`, in the same order.
    fn total(&self) -> f64 {
        self.rows.iter().sum::<f64>() + self.columns.iter().sum::<f64>()
    }
}

/// Starts from the cheaper of all horizontal and all vertical, then flips
/// single tiles while that lowers the cost, for at most `flip_rounds` rounds.
/// A flip only changes its own tile row and tile column, so each try costs
/// two strips.
pub(super) fn tile_directions(
    horizontal: &View,
    vertical: &View,
    segment_cost: f64,
    flip_rounds: u32,
) -> Directions {
    let occupied = occupied_tiles(horizontal);
    let all_horizontal = Directions::horizontal(horizontal.tiles());

    let costs_h = Costs::of(horizontal, vertical, &all_horizontal, segment_cost);
    let costs_v = Costs::of(horizontal, vertical, &occupied, segment_cost);

    let (mut directions, mut costs) = if costs_v.total() < costs_h.total() {
        (occupied.clone(), costs_v)
    } else {
        (all_horizontal, costs_h)
    };

    let tiles = occupied.marked();
    for _ in 0..flip_rounds {
        let mut improved = false;

        for &(t, s) in &tiles {
            let was = directions.get(t, s);
            directions.set(t, s, !was);

            let new_row = horizontal.strip_cost(t, &directions, segment_cost);
            let new_column = vertical.strip_cost(s, &directions, segment_cost);
            let old = costs.rows[t] + costs.columns[s];

            if new_row + new_column < old - MIN_GAIN {
                costs.rows[t] = new_row;
                costs.columns[s] = new_column;
                improved = true;
            } else {
                directions.set(t, s, was);
            }
        }

        if !improved {
            break;
        }
    }

    directions
}

/// The tiles holding at least one region cell, marked as vertical.
fn occupied_tiles(view: &View) -> Directions {
    let grid = view.tiles();
    let mut occupied = Directions::horizontal(grid);

    for (t, s) in view.cell_tiles() {
        occupied.set(t, s, true);
    }

    occupied
}
