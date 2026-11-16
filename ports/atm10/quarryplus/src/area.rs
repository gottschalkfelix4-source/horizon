//! The quarry's area and the order it digs it in, independent of the game.

/// How far from the corner marker the other two are looked for.
pub const MAX_SPAN: i32 = 64;

/// The columns to dig: x and z ranges, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x0: i32,
    pub x1: i32,
    pub z0: i32,
    pub z1: i32,
}

impl Area {
    pub fn width(&self) -> i64 {
        i64::from(self.x1 - self.x0 + 1)
    }
    pub fn depth(&self) -> i64 {
        i64::from(self.z1 - self.z0 + 1)
    }
    /// Columns in the area.
    pub fn cells(&self) -> i64 {
        self.width() * self.depth()
    }
    /// The `index`th position of a dig whose first layer is `top`: a layer at a time, downwards,
    /// each from its low x and z, x fastest.
    pub fn position(&self, index: i64, top: i32) -> [i32; 3] {
        let layer = index / self.cells();
        let in_layer = index % self.cells();
        [
            self.x0 + (in_layer % self.width()) as i32,
            top - layer as i32,
            self.z0 + (in_layer / self.width()) as i32,
        ]
    }
}

/// The area between the marker at `corner` and the nearest markers in line with it along x and
/// along z (up to [`MAX_SPAN`] away): the blocks strictly between them. None without both, or
/// when the markers leave no room between them.
pub fn find_area(corner: [i32; 3], mut is_marker: impl FnMut([i32; 3]) -> bool) -> Option<Area> {
    let along = |axis: usize, is_marker: &mut dyn FnMut([i32; 3]) -> bool| -> Option<i32> {
        (1..=MAX_SPAN).find_map(|d| {
            [d, -d].into_iter().find(|s| {
                let mut p = corner;
                p[axis] += s;
                is_marker(p)
            })
        })
    };
    let dx = along(0, &mut is_marker)?;
    let dz = along(2, &mut is_marker)?;
    let (xa, xb) = (corner[0].min(corner[0] + dx), corner[0].max(corner[0] + dx));
    let (za, zb) = (corner[2].min(corner[2] + dz), corner[2].max(corner[2] + dz));
    let area = Area { x0: xa + 1, x1: xb - 1, z0: za + 1, z1: zb - 1 };
    (area.x1 >= area.x0 && area.z1 >= area.z0).then_some(area)
}

/// Whether the quarry leaves the block alone: nothing there, a fluid, an unbreakable block
/// (`hardness` below zero), or one of the quarry's own.
pub fn skips(id: &str, hardness: f32) -> bool {
    hardness < 0.0
        || matches!(
            id,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:lava"
        )
        || id == "quarryplus:marker"
        || id == "quarryplus:quarry"
}

/// Energy to break one block of `hardness`.
pub fn break_cost(hardness: f32) -> i64 {
    200 + (hardness.max(0.0) * 400.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_area_is_what_lies_between_three_markers_in_an_l() {
        let markers: HashSet<[i32; 3]> = [[0, 64, 0], [6, 64, 0], [0, 64, -4]].into();
        let area = find_area([0, 64, 0], |p| markers.contains(&p)).unwrap();
        assert_eq!(area, Area { x0: 1, x1: 5, z0: -3, z1: -1 });
        assert_eq!(area.cells(), 15);
    }

    #[test]
    fn two_markers_or_markers_with_no_room_make_no_area() {
        let two: HashSet<[i32; 3]> = [[0, 64, 0], [6, 64, 0]].into();
        assert_eq!(find_area([0, 64, 0], |p| two.contains(&p)), None);
        let close: HashSet<[i32; 3]> = [[0, 64, 0], [1, 64, 0], [0, 64, 5]].into();
        assert_eq!(find_area([0, 64, 0], |p| close.contains(&p)), None);
        let far: HashSet<[i32; 3]> = [[0, 64, 0], [65, 64, 0], [0, 64, 5]].into();
        assert_eq!(find_area([0, 64, 0], |p| far.contains(&p)), None);
    }

    #[test]
    fn the_dig_goes_a_layer_at_a_time_downwards() {
        let a = Area { x0: 10, x1: 12, z0: 20, z1: 21 };
        assert_eq!(a.position(0, 63), [10, 63, 20]);
        assert_eq!(a.position(2, 63), [12, 63, 20]);
        assert_eq!(a.position(3, 63), [10, 63, 21]);
        assert_eq!(a.position(5, 63), [12, 63, 21]);
        assert_eq!(a.position(6, 63), [10, 62, 20]);
    }

    #[test]
    fn air_fluids_bedrock_and_own_blocks_are_skipped_and_harder_blocks_cost_more() {
        assert!(skips("minecraft:air", 0.0) && skips("minecraft:water", 100.0) && skips("minecraft:bedrock", -1.0));
        assert!(skips("quarryplus:marker", 0.5));
        assert!(!skips("minecraft:stone", 1.5));
        assert!(break_cost(50.0) > break_cost(1.5) && break_cost(1.5) > break_cost(0.0));
    }
}
