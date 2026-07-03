#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    // Measures the total grid path between two points by adding the horizontal and
    // vertical differences
    pub fn manhattan_distance(self, other: Self) -> u64 {
        u64::from(self.x.abs_diff(other.x)) + u64::from(self.y.abs_diff(other.y))
    }

    pub fn is_adjacent(self, other: Self) -> bool {
        self.manhattan_distance(other) == 1
    }

    pub fn is_same_or_adjacent(self, other: Self) -> bool {
        self.manhattan_distance(other) <= 1
    }
}
