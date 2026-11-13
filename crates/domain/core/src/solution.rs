//! One route per cutter for one instance.

use crate::route::Route;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solution {
    uid: String,
    routes: Vec<Route>,
}

impl Solution {
    pub fn new(uid: impl Into<String>, routes: Vec<Route>) -> Self {
        Self {
            uid: uid.into(),
            routes,
        }
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn into_routes(self) -> Vec<Route> {
        self.routes
    }

    /// What the contest scores: the length of the longest route.
    pub fn objective(&self) -> u64 {
        self.routes.iter().map(Route::length).max().unwrap_or(0)
    }

    pub fn total_length(&self) -> u64 {
        self.routes.iter().map(Route::length).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Point;

    #[test]
    fn objective_is_the_longest_route() {
        let short = Route::new(vec![Point::new(0, 0), Point::new(1, 0)]).unwrap();
        let long = Route::new(vec![Point::new(0, 0), Point::new(4, 0)]).unwrap();
        let solution = Solution::new("x", vec![short, long]);

        assert_eq!(solution.objective(), 8);
        assert_eq!(solution.total_length(), 10);
    }
}
