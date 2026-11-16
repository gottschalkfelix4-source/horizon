//! The order a Ranged Pump looks for sources in: the layer right below it first, nearest the
//! pump outwards, then the next layer down, and so on to `depth` layers.

/// The (dx, dz) of one layer of radius `radius`, nearest the centre first.
pub fn layer_order(radius: i32) -> Vec<(i32, i32)> {
    let mut cells: Vec<(i32, i32)> = (-radius..=radius)
        .flat_map(|dx| (-radius..=radius).map(move |dz| (dx, dz)))
        .collect();
    cells.sort_by_key(|&(dx, dz)| (dx * dx + dz * dz, dx, dz));
    cells
}

/// The offset from the pump of the `index`th position in the scan, or none past its end.
pub fn offset(order: &[(i32, i32)], depth: i32, index: usize) -> Option<[i32; 3]> {
    let layer = index / order.len().max(1);
    if layer >= depth.max(0) as usize {
        return None;
    }
    let (dx, dz) = order[index % order.len()];
    Some([dx, -1 - layer as i32, dz])
}

/// How many positions a scan has.
pub fn len(order: &[(i32, i32)], depth: i32) -> usize {
    order.len() * depth.max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scan_starts_below_the_pump_and_goes_outwards_then_down() {
        let order = layer_order(2);
        assert_eq!(len(&order, 3), 25 * 3);
        assert_eq!(offset(&order, 3, 0), Some([0, -1, 0]));
        // Next come the four cells at distance one, then the diagonals at 2.
        let ring: Vec<[i32; 3]> = (1..=4).filter_map(|i| offset(&order, 3, i)).collect();
        assert!(ring.iter().all(|o| o[0].abs() + o[2].abs() == 1 && o[1] == -1));
        assert_eq!(offset(&order, 3, 25), Some([0, -2, 0]));
        assert_eq!(offset(&order, 3, 74).map(|o| o[1]), Some(-3));
        assert_eq!(offset(&order, 3, 75), None);
    }

    #[test]
    fn every_cell_of_the_square_is_visited_once_per_layer() {
        let order = layer_order(3);
        let mut seen = std::collections::HashSet::new();
        for i in 0..order.len() {
            let o = offset(&order, 1, i).unwrap();
            assert!(o[0].abs() <= 3 && o[2].abs() <= 3);
            assert!(seen.insert((o[0], o[2])));
        }
        assert_eq!(seen.len(), 49);
    }
}
