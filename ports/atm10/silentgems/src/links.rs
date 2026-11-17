//! Teleporter links: where a linked teleporter sends you, saved per chunk. Pure logic.

use mod_api::{Compound, Tag};

/// A block in a dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub dimension: String,
    pub pos: [i32; 3],
}

impl Place {
    pub fn to_nbt(&self) -> Compound {
        [
            (
                "dimension".into(),
                Tag::String(self.dimension.as_str().into()),
            ),
            ("pos".into(), Tag::IntArray(self.pos.to_vec())),
        ]
        .into()
    }

    pub fn from_nbt(data: &Compound) -> Option<Self> {
        let Some(Tag::String(dimension)) = data.get(&"dimension".into()) else {
            return None;
        };
        let Some(Tag::IntArray(pos)) = data.get(&"pos".into()) else {
            return None;
        };
        let [x, y, z] = pos.as_slice() else {
            return None;
        };
        Some(Self {
            dimension: dimension.to_string().ok()?,
            pos: [*x, *y, *z],
        })
    }

    /// Where a player arrives: standing on top of the teleporter, centred.
    pub fn arrival(&self) -> [f64; 3] {
        [
            f64::from(self.pos[0]) + 0.5,
            f64::from(self.pos[1]) + 1.0,
            f64::from(self.pos[2]) + 0.5,
        ]
    }
}

/// One teleporter's link: the teleporter at `from` (in the chunk's level) sends to `to`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub from: [i32; 3],
    pub to: Place,
}

/// `links` with the teleporter at `from` now sending to `to`, replacing an older link.
pub fn relink(mut links: Vec<Link>, from: [i32; 3], to: Place) -> Vec<Link> {
    links.retain(|l| l.from != from);
    links.push(Link { from, to });
    links
}

pub fn encode(links: &[Link]) -> Compound {
    let list = links
        .iter()
        .map(|l| {
            let mut c = l.to.to_nbt();
            c.insert("from".into(), Tag::IntArray(l.from.to_vec()));
            Tag::Compound(c)
        })
        .collect();
    [("links".into(), Tag::List(list))].into()
}

pub fn decode(data: &Compound) -> Option<Vec<Link>> {
    let Some(Tag::List(list)) = data.get(&"links".into()) else {
        return None;
    };
    list.iter()
        .map(|tag| {
            let Tag::Compound(c) = tag else {
                return None;
            };
            let Some(Tag::IntArray(from)) = c.get(&"from".into()) else {
                return None;
            };
            let [x, y, z] = from.as_slice() else {
                return None;
            };
            Some(Link {
                from: [*x, *y, *z],
                to: Place::from_nbt(c)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(dim: &str, pos: [i32; 3]) -> Place {
        Place {
            dimension: dim.into(),
            pos,
        }
    }

    #[test]
    fn links_round_trip_through_their_saved_form() {
        let links = vec![
            Link {
                from: [1, 64, 2],
                to: place("minecraft:the_nether", [-8, 40, 9]),
            },
            Link {
                from: [3, -50, 4],
                to: place("minecraft:overworld", [100, 70, -100]),
            },
        ];
        assert_eq!(decode(&encode(&links)), Some(links));
    }

    #[test]
    fn relinking_a_teleporter_replaces_its_old_destination() {
        let links = relink(
            Vec::new(),
            [0, 64, 0],
            place("minecraft:overworld", [5, 64, 5]),
        );
        let links = relink(links, [0, 64, 0], place("minecraft:the_end", [9, 50, 9]));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].to.dimension, "minecraft:the_end");
    }

    #[test]
    fn a_player_arrives_on_top_of_the_teleporter() {
        assert_eq!(place("x:y", [2, 64, -3]).arrival(), [2.5, 65.0, -2.5]);
    }
}
