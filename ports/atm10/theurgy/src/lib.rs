//! Theurgy for Horizon: the Divination Rods, attuned to a block and then used to find the
//! nearest one of it around the player.
//!
//! Written from Theurgy's public description (its guide book, the wiki, how the mod plays),
//! never from its code. What still has to be confirmed against the NeoForge original is listed
//! in README.md.
//!
//! How it works:
//! - Sneak + right click on a block attunes the rod to that block: the block's id is written to
//!   the stack's custom data (`theurgy:linked_block`).
//! - Right click (not sneaking) with an attuned rod searches the cube of the rod's tier around the
//!   player, nearest shell first, for that block. A find is written to the stack
//!   (`theurgy:found`, `[x, y, z]`) and told on the action bar with its distance and compass
//!   direction; the rod loses one durability per search either way.
use std::sync::OnceLock;

use mod_api::{
    BlockKey, Component, Compound, HostSide, HostTransaction, InteractionResult, ItemKey, ItemPart,
    ItemPlace, ItemResource, ItemStack, LevelRef, Registrar, RegistrationContext, ServerHost, Tag,
    Text, UseItemEvent, UseOnEvent,
};

/// The rods, by tier: id and search radius in blocks along each axis (from memory).
pub const TIERS: [(&str, i32); 4] = [
    ("theurgy:divination_rod_t1", 16),
    ("theurgy:divination_rod_t2", 32),
    ("theurgy:divination_rod_t3", 48),
    ("theurgy:divination_rod_t4", 64),
];

/// Where the rod keeps what it knows: `minecraft:custom_data`.
struct CustomData;

impl Component for CustomData {
    type Value = Compound;
    fn id(&self) -> &str {
        "minecraft:custom_data"
    }
    fn decode(&self, value: &Tag) -> Option<Compound> {
        match value {
            Tag::Compound(c) => Some(c.clone()),
            _ => None,
        }
    }
    fn encode(&self, value: Compound) -> Tag {
        Tag::Compound(value)
    }
}

const LINKED: &str = "theurgy:linked_block";
const FOUND: &str = "theurgy:found";

/// The offsets at Chebyshev distance exactly `d` (the surface of the cube of side `2d + 1`).
pub fn shell(d: i32) -> impl Iterator<Item = [i32; 3]> {
    (-d..=d).flat_map(move |x| {
        (-d..=d).flat_map(move |y| {
            let face = x.abs() == d || y.abs() == d;
            let zs: Vec<i32> = if face {
                (-d..=d).collect()
            } else if d == 0 {
                vec![0]
            } else {
                vec![-d, d]
            };
            zs.into_iter().map(move |z| [x, y, z])
        })
    })
}

/// The offsets of a cube of `radius`, nearest Chebyshev shell first.
pub fn shell_offsets(radius: i32) -> impl Iterator<Item = [i32; 3]> {
    (0..=radius).flat_map(shell)
}

/// The nearest offset (by straight-line distance) among those `matches` accepts, searching shell
/// by shell: once a shell has a find, only the shells that could still hold a nearer one are
/// searched (up to `d * sqrt(3)`).
pub fn nearest(radius: i32, mut matches: impl FnMut([i32; 3]) -> bool) -> Option<[i32; 3]> {
    let dist2 = |o: [i32; 3]| o.iter().map(|v| (*v as i64) * (*v as i64)).sum::<i64>();
    let mut best: Option<[i32; 3]> = None;
    for d in 0..=radius {
        if let Some(b) = best {
            // Every offset of shell d is at least d away: stop once that is past the best.
            if (d as i64) * (d as i64) > dist2(b) {
                break;
            }
        }
        for o in shell(d) {
            if best.is_some_and(|b| dist2(o) >= dist2(b)) {
                continue;
            }
            if matches(o) {
                best = Some(o);
            }
        }
    }
    best
}

/// The eight-way compass direction of a horizontal offset, as a translation key suffix
/// (`north` is -z, `east` +x, as in vanilla).
pub fn compass(dx: i32, dz: i32) -> &'static str {
    if dx == 0 && dz == 0 {
        return "here";
    }
    // atan2 with north = 0, clockwise, in eighths.
    let angle = (dx as f64).atan2(-(dz as f64));
    let eighth = ((angle / std::f64::consts::FRAC_PI_4).round() as i32).rem_euclid(8);
    [
        "north",
        "north_east",
        "east",
        "south_east",
        "south",
        "south_west",
        "west",
        "north_west",
    ][eighth as usize]
}

struct Keys {
    rods: Vec<ItemKey>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn radius_of(item: ItemKey) -> Option<i32> {
    let keys = KEYS.get()?;
    let tier = keys.rods.iter().position(|k| *k == item)?;
    Some(TIERS[tier].1)
}

fn linked_block(stack: &ItemStack) -> Option<String> {
    match stack.get(&CustomData)?.get(&LINKED.into())? {
        Tag::String(s) => s.to_string().ok(),
        _ => None,
    }
}

/// Change the held rod: `edit` its custom data, then take `wear` durability; a rod worn out
/// breaks.
fn change_rod(
    host: &mut dyn ServerHost,
    player: u128,
    hand: mod_api::Hand,
    wear: u32,
    edit: impl FnOnce(&mut Compound),
) {
    let place = ItemPlace::PlayerHand { player, hand };
    let Some(stack) = place.stack(host) else {
        return;
    };
    let max = host.max_damage(stack.item()).unwrap_or(0);
    let infinite = host
        .player(player)
        .and_then(|p| p.has_infinite_materials(host))
        == Some(true);
    let wear = if infinite { 0 } else { wear };
    let broken = max > 0 && stack.damage() + wear >= max;
    let _: Result<(), ()> = host.transaction(|host, tx| {
        if broken {
            place.extract(host, tx, &ItemResource::of(&stack), 1);
            return Ok(());
        }
        place.edit(host, tx, |s| {
            let mut data = s.get(&CustomData).unwrap_or_default();
            edit(&mut data);
            let _ = s.set(&CustomData, data);
            if max > 0 && wear > 0 {
                s.set_damage(s.damage() + wear);
            }
        });
        Ok(())
    });
}

struct Rod;

impl Rod {
    fn attune(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Option<InteractionResult> {
        let player = host.player(event.player)?;
        if player.is_secondary_use_active(host) != Some(true) {
            return None;
        }
        let level = player.level(host)?;
        let state = level.block_state(host, event.pos)?;
        let id = host.block_state_string(host.block_state_default(state.block())?)?;
        let id = id.split('[').next().unwrap_or(&id).to_owned();
        change_rod(host, event.player, event.hand, 0, |data| {
            data.insert(LINKED.into(), Tag::String(id.as_str().into()));
            data.remove(&FOUND.into());
        });
        let _ = player.action_bar(
            host,
            Text::translate(
                "message.theurgy.divination_rod.linked",
                vec![Text::translate(block_translation_key(&id), vec![])],
            ),
        );
        Some(InteractionResult::SuccessServer)
    }

    fn search(host: &mut dyn ServerHost, event: &UseItemEvent<'_>) -> Option<InteractionResult> {
        let radius = radius_of(event.stack.item())?;
        let player = host.player(event.player)?;
        let Some(linked) = linked_block(event.stack) else {
            let _ = player.action_bar(
                host,
                Text::translate("message.theurgy.divination_rod.no_link", vec![]),
            );
            return Some(InteractionResult::Fail);
        };
        let level: LevelRef = player.level(host)?;
        let target: BlockKey = host.block_state_id(&linked)?.block();
        let origin = player.entity().block_position(host)?;
        let found = nearest(radius, |o| {
            let pos = [origin[0] + o[0], origin[1] + o[1], origin[2] + o[2]];
            level.block(host, pos) == Some(target)
        });
        let line = match found {
            Some(o) => {
                let pos = [origin[0] + o[0], origin[1] + o[1], origin[2] + o[2]];
                let distance = ((o[0] * o[0] + o[1] * o[1] + o[2] * o[2]) as f64).sqrt();
                change_rod(host, event.player, event.hand, 1, |data| {
                    data.insert(FOUND.into(), Tag::IntArray(pos.to_vec()));
                });
                Text::translate(
                    "message.theurgy.divination_rod.found",
                    vec![
                        Text::literal(format!("{}", distance.round() as i64)),
                        Text::translate(
                            format!("message.theurgy.direction.{}", compass(o[0], o[2])),
                            vec![],
                        ),
                    ],
                )
            }
            None => {
                change_rod(host, event.player, event.hand, 1, |data| {
                    data.remove(&FOUND.into());
                });
                Text::translate("message.theurgy.divination_rod.not_found", vec![])
            }
        };
        let _ = player.action_bar(host, line);
        Some(InteractionResult::SuccessServer)
    }
}

/// `minecraft:iron_ore` → `block.minecraft.iron_ore`.
fn block_translation_key(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("block.{ns}.{path}")
}

#[mod_api::horizon_part]
impl ItemPart for Rod {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => Rod::attune(host, event),
            HostSide::Client(_) => None,
        }
    }

    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => Rod::search(host, event),
            HostSide::Client(_) => None,
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let rods: Vec<ItemKey> = TIERS.iter().map(|(id, _)| r.item_key(id)).collect();
        for rod in &rods {
            r.item_part(*rod, Rod);
        }
        let _ = KEYS.set(Keys { rods });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_cover_the_cube_once_nearest_first() {
        let all: Vec<_> = shell_offsets(2).collect();
        assert_eq!(all.len(), 125);
        let mut sorted = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 125);
        let shell = |o: &[i32; 3]| o.iter().map(|v| v.abs()).max().unwrap();
        assert!(all.windows(2).all(|w| shell(&w[0]) <= shell(&w[1])));
    }

    #[test]
    fn the_nearest_find_wins_over_an_earlier_shell_corner() {
        // Shell 3 holds a corner at distance sqrt(27) ~ 5.2; shell 4 a face block at 4.
        let ores = [[3, 3, 3], [0, -4, 0]];
        let found = nearest(8, |o| ores.contains(&o));
        assert_eq!(found, Some([0, -4, 0]));
        assert_eq!(nearest(2, |o| ores.contains(&o)), None);
    }

    #[test]
    fn compass_follows_minecraft_axes() {
        assert_eq!(compass(0, -10), "north");
        assert_eq!(compass(10, 0), "east");
        assert_eq!(compass(0, 10), "south");
        assert_eq!(compass(-10, 0), "west");
        assert_eq!(compass(5, 5), "south_east");
        assert_eq!(compass(-7, -6), "north_west");
        assert_eq!(compass(0, 0), "here");
    }
}
