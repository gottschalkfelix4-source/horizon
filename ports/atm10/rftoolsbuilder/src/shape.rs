//! Shape cards' shapes: which blocks of a card's box belong to its shape, and the order a quarry
//! goes through them (top layer first, then north to south, then west to east).

/// A card's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Box,
    Sphere,
    /// A vertical cylinder filling the box's height.
    Cylinder,
    /// The upper half of a sphere standing on the box's floor.
    TopDome,
    /// The lower half of a sphere hanging from the box's ceiling.
    BottomDome,
}

impl Shape {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "box" => Shape::Box,
            "sphere" => Shape::Sphere,
            "cylinder" => Shape::Cylinder,
            "topdome" => Shape::TopDome,
            "bottomdome" => Shape::BottomDome,
            _ => return None,
        })
    }
    pub fn code(self) -> i32 {
        self as i32
    }
    pub fn from_code(code: i32) -> Option<Self> {
        [
            Shape::Box,
            Shape::Sphere,
            Shape::Cylinder,
            Shape::TopDome,
            Shape::BottomDome,
        ]
        .get(usize::try_from(code).ok()?)
        .copied()
    }
}

/// A shape in a box of `dim` blocks (x, y, z) whose centre block is `center`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub shape: Shape,
    /// Solid, or only the shell (one block thick).
    pub solid: bool,
    pub dim: [i32; 3],
    pub center: [i32; 3],
}

impl Area {
    /// The box's lowest corner.
    fn low(&self) -> [i32; 3] {
        std::array::from_fn(|i| self.center[i] - self.dim[i] / 2)
    }

    /// How many blocks the box holds: the quarry's cursor runs from 0 to this.
    pub fn volume(&self) -> u64 {
        self.dim.iter().map(|d| (*d).max(0) as u64).product()
    }

    /// The `index`th block of the box in the quarry's order, whether or not the shape holds it.
    pub fn nth(&self, index: u64) -> Option<[i32; 3]> {
        if index >= self.volume() {
            return None;
        }
        let [w, h, l] = self.dim.map(|d| d as u64);
        let layer = index / (w * l);
        let row = index % (w * l);
        let low = self.low();
        Some([
            low[0] + (row % w) as i32,
            low[1] + (h - 1 - layer) as i32,
            low[2] + (row / w) as i32,
        ])
    }

    /// Whether `pos` lies in the solid shape.
    fn inside(&self, pos: [i32; 3]) -> bool {
        let low = self.low();
        let rel: [i32; 3] = std::array::from_fn(|i| pos[i] - low[i]);
        if (0..3).any(|i| rel[i] < 0 || rel[i] >= self.dim[i]) {
            return false;
        }
        // Each axis scaled to -1..1 over the box, measured at block centres.
        let unit = |i: usize| (f64::from(rel[i]) + 0.5) / f64::from(self.dim[i]) * 2.0 - 1.0;
        let (x, y, z) = (unit(0), unit(1), unit(2));
        let half = |y: f64| (y + 1.0) / 2.0;
        match self.shape {
            Shape::Box => true,
            Shape::Sphere => x * x + y * y + z * z <= 1.0,
            Shape::Cylinder => x * x + z * z <= 1.0,
            Shape::TopDome => {
                let y = half(y);
                x * x + y * y + z * z <= 1.0
            }
            Shape::BottomDome => {
                let y = 1.0 - half(y);
                x * x + y * y + z * z <= 1.0
            }
        }
    }

    /// Whether the card's shape holds `pos`: inside it and, for a hollow card, on its shell.
    pub fn contains(&self, pos: [i32; 3]) -> bool {
        if !self.inside(pos) {
            return false;
        }
        if self.solid {
            return true;
        }
        let around: &[[i32; 3]] = match self.shape {
            // A hollow cylinder is its wall: open at the top and bottom.
            Shape::Cylinder => &[[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]],
            _ => &[
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ],
        };
        around
            .iter()
            .any(|d| !self.inside([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]]))
    }
}
