//! Which area of a tour to take out: a window of anchors around a point of
//! the tour, sized in cutter spans.
//!
//! - squares of 2, 3 and 5 spans around a random position;
//! - bands 6 spans long and 1.5 high (or wide): a few neighbouring lanes;
//! - `Dead`: a 3 span square around a dead position, one whose footprint
//!   holds no cell only it sweeps (where `mow diagnose` puts the waste);
//! - `Transfer` (k > 1): a 3 span square of the longest tour, whose cells go
//!   to the nearest other tour.

use mow_core::Point;
use mow_coverage::{Scratch, VisitCounts};
use mow_polish::Window;
use rand::Rng;

/// Positions looked at for a dead one before settling for any.
const DEAD_TRIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destroy {
    Small,
    Medium,
    Large,
    BandH,
    BandV,
    Dead,
    Transfer,
}

impl Destroy {
    pub const ALL: [Self; 7] = [
        Self::Small,
        Self::Medium,
        Self::Large,
        Self::BandH,
        Self::BandV,
        Self::Dead,
        Self::Transfer,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "square-2",
            Self::Medium => "square-3",
            Self::Large => "square-5",
            Self::BandH => "band-h",
            Self::BandV => "band-v",
            Self::Dead => "dead-3",
            Self::Transfer => "transfer-3",
        }
    }

    /// Width and height in cutter spans.
    fn shape(self) -> (f64, f64) {
        match self {
            Self::Small => (2.0, 2.0),
            Self::Medium | Self::Dead | Self::Transfer => (3.0, 3.0),
            Self::Large => (5.0, 5.0),
            Self::BandH => (6.0, 1.5),
            Self::BandV => (1.5, 6.0),
        }
    }

    /// The window of this shape around a position of `tour`, chosen at
    /// random (a dead one for `Dead`).
    pub fn window(
        self,
        tour: &[Point],
        span: i32,
        counts: &VisitCounts,
        scratch: &mut Scratch,
        rng: &mut impl Rng,
    ) -> Window {
        let center = match self {
            Self::Dead => dead_position(tour, counts, scratch, rng),
            _ => tour[rng.gen_range(0..tour.len())],
        };

        let (w, h) = self.shape();
        let jitter = |rng: &mut dyn rand::RngCore| rng.gen_range(0.8..1.25);
        let width = ((w * f64::from(span) * jitter(rng)).round() as i32).max(2);
        let height = ((h * f64::from(span) * jitter(rng)).round() as i32).max(2);

        let x0 = center.x - rng.gen_range(0..width);
        let y0 = center.y - rng.gen_range(0..height);
        Window::new(x0, y0, x0 + width, y0 + height)
    }
}

/// A random position whose removal alone would leave no cell bare, or any
/// random position if none turns up within [`DEAD_TRIES`].
fn dead_position(tour: &[Point], counts: &VisitCounts, scratch: &mut Scratch, rng: &mut impl Rng) -> Point {
    let mut any = tour[rng.gen_range(0..tour.len())];

    for _ in 0..DEAD_TRIES {
        let p = tour[rng.gen_range(0..tour.len())];
        if counts.still_covered_without(scratch, [p]) {
            return p;
        }
        any = p;
    }

    any
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::rectangle;
    use mow_core::Route;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    fn names_are_distinct() {
        let names: std::collections::HashSet<&str> = Destroy::ALL.iter().map(|d| d.name()).collect();
        assert_eq!(names.len(), Destroy::ALL.len());
    }

    #[test]
    fn windows_touch_the_tour_and_follow_the_shape() {
        let problem = rectangle(40, 12, 3);
        let tour: Vec<Point> = (1..39).map(|x| Point::new(x, 7)).collect();
        let counts =
            VisitCounts::from_routes(&problem, &[Route::new(vec![tour[0], tour[37]]).unwrap()]).unwrap();
        let mut scratch = Scratch::for_counts(&counts);
        let mut rng = SmallRng::seed_from_u64(3);

        for destroy in Destroy::ALL {
            for _ in 0..50 {
                let w = destroy.window(&tour, 3, &counts, &mut scratch, &mut rng);

                assert!(tour.iter().any(|&p| w.contains(p)), "{destroy:?} {w:?}");
                if destroy == Destroy::BandH {
                    assert!(w.width() > 2 * w.height());
                }
            }
        }
    }
}
