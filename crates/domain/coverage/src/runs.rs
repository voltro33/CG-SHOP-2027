//! Stamping a straight run of anchors in one go.
//!
//! Neighbouring anchors on a lane share almost all of their footprint, so
//! stamping them one by one touches the same cells over and over: a run of
//! `n` anchors costs `n` times the cutter's area. Cut the footprint into
//! strips along the run instead (its rows for a run along a row, its columns
//! for a run along a column). The `n` copies of one strip of `r` cells then
//! cover `n + r - 1` cells, each by a count known from where it sits, so the
//! run costs about `n + r` per strip.

/// Which way a run of anchors goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    Row,
    Column,
}

/// Neighbouring anchors, `len` of them from the anchor index `first`, one
/// apart along a row or a frame width apart along a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub first: usize,
    pub len: usize,
    pub along: Along,
}

/// `len` cover indices from `start`, one apart in a row strip or a cover
/// width apart in a column strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Strip {
    start: usize,
    len: usize,
}

/// The footprint cut into row strips and into column strips.
#[derive(Debug, Clone)]
pub struct Strips {
    rows: Vec<Strip>,
    columns: Vec<Strip>,
    cover_width: usize,
}

impl Strips {
    /// From the footprint's cover offsets, relative to the cutter box's low
    /// corner in a cover `cover_width` wide.
    pub fn new(footprint: &[usize], cover_width: usize) -> Self {
        let by_row = |d: &usize| (d / cover_width, d % cover_width);
        let by_column = |d: &usize| (d % cover_width, d / cover_width);

        Self {
            rows: strips(footprint, by_row),
            columns: strips(footprint, by_column),
            cover_width,
        }
    }

    /// Calls `f(cover index, anchors of the run over it)` for every cell
    /// the run's footprints sweep, each once. `base` is the cover base of the
    /// run's first anchor.
    pub fn for_each_cell(&self, run: Run, base: usize, mut f: impl FnMut(usize, u16)) {
        let (strips, step) = match run.along {
            Along::Row => (&self.rows, 1),
            Along::Column => (&self.columns, self.cover_width),
        };

        for strip in strips {
            let first = base + strip.start;
            for t in 0..run.len + strip.len - 1 {
                f(first + t * step, overlap(t, run.len, strip.len));
            }
        }
    }
}

/// The strips of `cells` keyed `(line, place on the line)` by `key`: cells
/// on one line at consecutive places. Each strip starts at its lowest cell.
fn strips(cells: &[usize], key: impl Fn(&usize) -> (usize, usize)) -> Vec<Strip> {
    let mut sorted = cells.to_vec();
    sorted.sort_unstable_by_key(&key);

    let mut out: Vec<Strip> = Vec::new();
    let mut last: Option<(usize, usize)> = None;
    for d in sorted {
        let (line, at) = key(&d);
        match (last, out.last_mut()) {
            (Some((l, a)), Some(strip)) if l == line && a + 1 == at => strip.len += 1,
            _ => out.push(Strip { start: d, len: 1 }),
        }
        last = Some((line, at));
    }

    out
}

/// How many of `n` copies of a strip of `r` cells, each shifted one further,
/// cover the cell `t` steps from the first copy's start.
fn overlap(t: usize, n: usize, r: usize) -> u16 {
    let high = t.min(n - 1);
    let low = (t + 1).saturating_sub(r);
    (high - low + 1) as u16
}

/// The anchor indices `anchors`, distinct, grouped into runs: consecutive
/// entries that extend a row or a column run at either end join it. A
/// frame row is `frame_width` anchors.
pub fn runs(anchors: &[usize], frame_width: usize) -> Vec<Run> {
    let mut out = Vec::new();
    let mut current: Option<Building> = None;

    for &i in anchors {
        current = match current.and_then(|b| b.extended(i, frame_width)) {
            Some(b) => Some(b),
            None => {
                out.extend(current.map(Building::run));
                Some(Building::single(i))
            }
        };
    }

    out.extend(current.map(Building::run));
    out
}

/// A run being collected: `len` anchors from `low` to `high` along
/// `along`, unknown while it holds one.
#[derive(Debug, Clone, Copy)]
struct Building {
    low: usize,
    high: usize,
    len: usize,
    along: Option<Along>,
}

impl Building {
    fn single(i: usize) -> Self {
        Self {
            low: i,
            high: i,
            len: 1,
            along: None,
        }
    }

    /// The run with `i` added at either end, if `i` lies there.
    fn extended(self, i: usize, frame_width: usize) -> Option<Self> {
        let same_row = |a: usize, b: usize| a / frame_width == b / frame_width;
        let row_ok = self.along != Some(Along::Column);
        let column_ok = self.along != Some(Along::Row);

        let (low, high, along) = if row_ok && i == self.high + 1 && same_row(i, self.high) {
            (self.low, i, Along::Row)
        } else if row_ok && i + 1 == self.low && same_row(i, self.low) {
            (i, self.high, Along::Row)
        } else if column_ok && i == self.high + frame_width {
            (self.low, i, Along::Column)
        } else if column_ok && i + frame_width == self.low {
            (i, self.high, Along::Column)
        } else {
            return None;
        };

        Some(Self {
            low,
            high,
            len: self.len + 1,
            along: Some(along),
        })
    }

    fn run(self) -> Run {
        Run {
            first: self.low,
            len: self.len,
            along: self.along.unwrap_or(Along::Row),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_in_either_direction_join_one_run() {
        // a frame 10 wide: 23 24 25 along a row, then 35 45 down a column
        // from 25, then 13 alone, then 44 43 leftwards
        let got = runs(&[24, 25, 23, 35, 45, 55, 13, 44, 43], 10);

        assert_eq!(
            got,
            vec![
                Run {
                    first: 23,
                    len: 3,
                    along: Along::Row
                },
                Run {
                    first: 35,
                    len: 3,
                    along: Along::Column
                },
                Run {
                    first: 13,
                    len: 1,
                    along: Along::Row
                },
                Run {
                    first: 43,
                    len: 2,
                    along: Along::Row
                },
            ]
        );
    }

    #[test]
    fn a_run_never_wraps_to_the_next_row() {
        let got = runs(&[8, 9, 10, 11], 10);

        assert_eq!(
            got[0],
            Run {
                first: 8,
                len: 2,
                along: Along::Row
            }
        );
        assert_eq!(
            got[1],
            Run {
                first: 10,
                len: 2,
                along: Along::Row
            }
        );
    }

    #[test]
    fn overlaps_rise_hold_and_fall() {
        // 4 copies of a 3 cell strip: cover 1 2 3 3 2 1
        let got: Vec<u16> = (0..6).map(|t| overlap(t, 4, 3)).collect();
        assert_eq!(got, vec![1, 2, 3, 3, 2, 1]);

        // 2 copies of a 5 cell strip: 1 2 2 2 2 1
        let got: Vec<u16> = (0..6).map(|t| overlap(t, 2, 5)).collect();
        assert_eq!(got, vec![1, 2, 2, 2, 2, 1]);
    }

    #[test]
    fn a_strip_run_visits_every_cell_once_with_its_count() {
        // a plus in a cover 10 wide: offsets of (1,0) (0,1) (1,1) (2,1) (1,2)
        let footprint = [1, 10, 11, 12, 21];
        let strips = Strips::new(&footprint, 10);
        let mut cover = [0u16; 60];

        let run = Run {
            first: 0,
            len: 3,
            along: Along::Row,
        };
        strips.for_each_cell(run, 0, |c, n| cover[c] += n);

        let mut one_by_one = [0u16; 60];
        for base in 0..3 {
            footprint.iter().for_each(|d| one_by_one[base + d] += 1);
        }
        assert_eq!(cover, one_by_one);

        let mut cover = [0u16; 60];
        let run = Run {
            first: 0,
            len: 3,
            along: Along::Column,
        };
        strips.for_each_cell(run, 0, |c, n| cover[c] += n);

        let mut one_by_one = [0u16; 60];
        for base in [0, 10, 20] {
            footprint.iter().for_each(|d| one_by_one[base + d] += 1);
        }
        assert_eq!(cover, one_by_one);
    }
}
