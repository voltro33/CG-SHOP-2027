//! Colors. Routes use the Okabe-Ito palette, which stays distinct under the
//! common forms of color blindness.

/// One color per cutter, cycling if there are more routes than colors.
const ROUTES: [&str; 6] = ["#0072B2", "#E69F00", "#CC79A7", "#009E73", "#56B4E9", "#000000"];

pub const BACKGROUND: &str = "#ffffff";
pub const REGION: &str = "#cfe3c7";
pub const UNCOVERED: &str = "#D55E00";
pub const SEGMENT: &str = "#333333";
pub const LINK: &str = "#9a9a9a";
pub const TEXT: &str = "#222222";

/// The color of route `index`.
pub const fn route_color(index: usize) -> &'static str {
    ROUTES[index % ROUTES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_cutters_get_three_colors_and_colors_cycle() {
        assert_ne!(route_color(0), route_color(1));
        assert_ne!(route_color(1), route_color(2));
        assert_eq!(route_color(0), route_color(ROUTES.len()));
    }
}
