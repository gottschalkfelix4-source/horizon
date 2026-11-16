//! Dungeon Crawl for Horizon: multi-floor underground dungeons.
//!
//! Written from Dungeon Crawl's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: the structure type `dungeoncrawl:dungeon` (placed by the data pack structure
//! set in `data/dungeoncrawl/worldgen/`) lays out `FLOORS` floors under the surface. Each floor
//! is a grid of cells joined by a random spanning tree, so every cell is reachable; dead ends
//! and some other cells are rooms, the rest corridors. Rooms hold a mob spawner or a loot chest.
//! A ladder shaft leads from the surface to the first floor, and from each floor to the next.
//! Every floor is one structure piece, saved with its layout, so a chunk generated later builds
//! the same dungeon.
use std::sync::Arc;

use mod_api::{
    BlockKey, BlockMetadata, BlockState, BoundingBox, Compound, FoundStructure, GenerationContext,
    HeightmapType, Hooks, PieceContext, Registrar, RegistrationContext, StructureError,
    StructurePiece, StructureRandom, StructureType, Tag,
};

/// Floors per dungeon.
pub const FLOORS: i32 = 4;
/// Cells along each side of a floor.
pub const GRID: i32 = 7;
/// Blocks along each side of a cell.
pub const CELL: i32 = 9;
/// Blocks from one floor's floor to the next's: floor, five open, ceiling.
pub const STOREY: i32 = 7;
/// Blocks of earth between the surface and the first floor's ceiling.
pub const COVER: i32 = 8;

const N: u16 = 1;
const E: u16 = 2;
const S: u16 = 4;
const W: u16 = 8;
const ROOM: u16 = 16;
const SPAWNER: u16 = 32;
const CHEST: u16 = 64;
/// A ladder down to the next floor starts here.
const DOWN: u16 = 128;
/// A ladder from the floor above (or the surface) ends here.
const UP: u16 = 256;

/// The four sides of a cell: its bit, the step to the neighbour and the opposite side's bit.
const SIDES: [(u16, [i32; 2], u16); 4] = [
    (N, [0, -1], S),
    (E, [1, 0], W),
    (S, [0, 1], N),
    (W, [-1, 0], E),
];

/// One floor's cells, row by row (z then x), as flag bits.
pub type Cells = Vec<u16>;

fn index(x: i32, z: i32) -> usize {
    (z * GRID + x) as usize
}

/// A floor's layout: a spanning tree over the grid from `start` (random depth-first search),
/// rooms at the dead ends and on about one other cell in four, and `up` and `down` cells marked.
/// `down` is None on the last floor.
pub fn floor_layout(
    random: &mut dyn StructureRandom,
    up: [i32; 2],
    down: Option<[i32; 2]>,
) -> Cells {
    let mut cells = vec![0u16; (GRID * GRID) as usize];
    let mut seen = vec![false; cells.len()];
    let mut stack = vec![up];
    seen[index(up[0], up[1])] = true;
    while let Some(&[x, z]) = stack.last() {
        let open: Vec<_> = SIDES
            .iter()
            .filter(|(_, d, _)| {
                let (nx, nz) = (x + d[0], z + d[1]);
                (0..GRID).contains(&nx) && (0..GRID).contains(&nz) && !seen[index(nx, nz)]
            })
            .collect();
        if open.is_empty() {
            stack.pop();
            continue;
        }
        let &&(bit, d, back) = &open[random.next_int_bound(open.len() as i32) as usize];
        let (nx, nz) = (x + d[0], z + d[1]);
        cells[index(x, z)] |= bit;
        cells[index(nx, nz)] |= back;
        seen[index(nx, nz)] = true;
        stack.push([nx, nz]);
    }
    for cell in cells.iter_mut() {
        let dead_end = (*cell & (N | E | S | W)).count_ones() == 1;
        if dead_end || random.next_int_bound(4) == 0 {
            *cell |= ROOM;
            *cell |= if random.next_int_bound(3) == 0 {
                CHEST
            } else {
                SPAWNER
            };
        }
    }
    let mark = |cells: &mut Cells, at: [i32; 2], flag: u16| {
        let c = &mut cells[index(at[0], at[1])];
        *c = (*c & !(SPAWNER | CHEST)) | ROOM | flag;
    };
    mark(&mut cells, up, UP);
    if let Some(down) = down {
        mark(&mut cells, down, DOWN);
    }
    cells
}

/// What one block of a floor is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wall,
    Floor,
    Air,
    /// A ladder against the wall to its north.
    Ladder,
    Spawner,
    Chest,
}

/// The block at `local` (x, y, z from the floor's lowest corner) of a floor with `cells`.
pub fn floor_block(cells: &Cells, local: [i32; 3]) -> Kind {
    let [x, y, z] = local;
    let cell = cells[index(x / CELL, z / CELL)];
    let (cx, cz) = (x % CELL, z % CELL);
    let middle = |v: i32| (3..=5).contains(&v);
    let ladder_column = cx == 4 && cz == 4;
    // The ladder shafts: down through this floor's floor, up through its ceiling.
    if ladder_column && ((cell & DOWN != 0 && y < STOREY - 1) || (cell & UP != 0 && y > 0)) {
        return Kind::Ladder;
    }
    if y == 0 {
        return Kind::Floor;
    }
    if y == STOREY - 1 {
        return Kind::Wall;
    }
    // The ladder's backing.
    if cx == 4 && cz == 3 && cell & (UP | DOWN) != 0 {
        return Kind::Wall;
    }
    let door = |bit: u16, edge: bool| cell & bit != 0 && edge;
    let open = if cell & ROOM != 0 {
        ((1..CELL - 1).contains(&cx) && (1..CELL - 1).contains(&cz))
            || (middle(cx) && (door(N, cz == 0) || door(S, cz == CELL - 1)))
            || (middle(cz) && (door(W, cx == 0) || door(E, cx == CELL - 1)))
    } else {
        (middle(cx) && middle(cz))
            || (middle(cx) && ((cell & N != 0 && cz < 3) || (cell & S != 0 && cz > 5)))
            || (middle(cz) && ((cell & W != 0 && cx < 3) || (cell & E != 0 && cx > 5)))
    };
    if !open {
        return Kind::Wall;
    }
    if y == 1 && cell & SPAWNER != 0 && ladder_column {
        return Kind::Spawner;
    }
    if y == 1 && cell & CHEST != 0 && cx == 2 && cz == 2 {
        return Kind::Chest;
    }
    Kind::Air
}

/// What a floor places at `local`: its block where it is open or borders the open space, None
/// where the ground stays as it is (inside the rock between rooms).
pub fn placed(cells: &Cells, local: [i32; 3]) -> Option<Kind> {
    let kind = floor_block(cells, local);
    if !matches!(kind, Kind::Wall | Kind::Floor) {
        return Some(kind);
    }
    let side = GRID * CELL;
    let borders_open = [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .into_iter()
    .map(|d| [local[0] + d[0], local[1] + d[1], local[2] + d[2]])
    .filter(|n| {
        (0..side).contains(&n[0]) && (0..STOREY).contains(&n[1]) && (0..side).contains(&n[2])
    })
    .any(|n| !matches!(floor_block(cells, n), Kind::Wall | Kind::Floor));
    borders_open.then_some(kind)
}

/// The mob a floor's spawners hold: deeper floors hold harder mobs.
pub fn spawner_mob(floor: i32) -> &'static str {
    match floor {
        0 => "minecraft:zombie",
        1 => "minecraft:skeleton",
        2 => "minecraft:spider",
        _ => "minecraft:cave_spider",
    }
}

/// The blocks a piece places, resolved once per piece.
struct Palette {
    wall: [BlockState; 3],
    floor: BlockState,
    air: BlockState,
    ladder: BlockState,
    spawner: BlockState,
    chest: BlockState,
}

#[derive(Clone, Copy)]
struct Blocks {
    stone_bricks: BlockKey,
    mossy: BlockKey,
    cracked: BlockKey,
    cobblestone: BlockKey,
    air: BlockKey,
    ladder: BlockKey,
    spawner: BlockKey,
    chest: BlockKey,
}

impl Blocks {
    fn palette(&self, ctx: &dyn PieceContext) -> Palette {
        Palette {
            wall: [
                ctx.state(self.stone_bricks),
                ctx.state(self.mossy),
                ctx.state(self.cracked),
            ],
            floor: ctx.state(self.cobblestone),
            air: ctx.state(self.air),
            ladder: ctx.state_with(self.ladder, &[("facing", "south")]),
            spawner: ctx.state(self.spawner),
            chest: ctx.state_with(self.chest, &[("facing", "south")]),
        }
    }
}

/// One floor of a dungeon.
#[derive(Debug)]
struct Floor {
    blocks: BlocksDebug,
    origin: [i32; 3],
    floor: i32,
    cells: Cells,
}

/// The ladder shaft from the surface down to the first floor's ceiling.
#[derive(Debug)]
struct Entrance {
    blocks: BlocksDebug,
    /// The ladder column's lowest block (in the first floor's ceiling's row, above it).
    bottom: [i32; 3],
    top: i32,
}

/// `Blocks` with a `Debug` the piece trait needs.
#[derive(Clone, Copy)]
struct BlocksDebug(Blocks);

impl std::fmt::Debug for BlocksDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blocks")
    }
}

fn clip(a: BoundingBox, b: BoundingBox) -> Option<([i32; 3], [i32; 3])> {
    let min = std::array::from_fn(|i| a.min[i].max(b.min[i]));
    let max = std::array::from_fn(|i| a.max[i].min(b.max[i]));
    (0..3).all(|i| min[i] <= max[i]).then_some((min, max))
}

impl StructurePiece for Floor {
    fn bounding_box(&self) -> BoundingBox {
        let o = self.origin;
        BoundingBox::new(
            o,
            [
                o[0] + GRID * CELL - 1,
                o[1] + STOREY - 1,
                o[2] + GRID * CELL - 1,
            ],
        )
    }

    fn post_process(&self, ctx: &mut dyn PieceContext) -> Result<(), StructureError> {
        let Some((min, max)) = clip(self.bounding_box(), ctx.chunk_box()) else {
            return Ok(());
        };
        let p = self.blocks.0.palette(ctx);
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                for y in min[1]..=max[1] {
                    let local = [x - self.origin[0], y - self.origin[1], z - self.origin[2]];
                    let pos = [x, y, z];
                    match placed(&self.cells, local) {
                        None => {}
                        Some(Kind::Wall) => {
                            let worn = ctx.random().next_int_bound(10);
                            let state = p.wall[if worn < 7 {
                                0
                            } else {
                                (worn - 6).min(2) as usize
                            }];
                            ctx.set_block(pos, state);
                        }
                        Some(Kind::Floor) => {
                            ctx.set_block(pos, p.floor);
                        }
                        Some(Kind::Air) => {
                            ctx.set_block(pos, p.air);
                        }
                        Some(Kind::Ladder) => {
                            ctx.set_block(pos, p.ladder);
                        }
                        Some(Kind::Spawner) => {
                            if ctx.set_block(pos, p.spawner) {
                                ctx.block_entity(pos, spawner_nbt(spawner_mob(self.floor)));
                            }
                        }
                        Some(Kind::Chest) => {
                            let seed = ctx.random().next_long();
                            if ctx.set_block(pos, p.chest) {
                                ctx.block_entity(pos, chest_nbt(seed));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn add_additional_save_data(&self, tag: &mut Compound) {
        tag.insert("origin".into(), Tag::IntArray(self.origin.to_vec()));
        tag.insert("floor".into(), Tag::Int(self.floor));
        let cells = self.cells.iter().map(|&c| i32::from(c)).collect();
        tag.insert("cells".into(), Tag::IntArray(cells));
    }
}

impl StructurePiece for Entrance {
    fn bounding_box(&self) -> BoundingBox {
        let b = self.bottom;
        BoundingBox::new([b[0] - 1, b[1], b[2] - 1], [b[0] + 1, self.top, b[2] + 1])
    }

    fn post_process(&self, ctx: &mut dyn PieceContext) -> Result<(), StructureError> {
        let Some((min, max)) = clip(self.bounding_box(), ctx.chunk_box()) else {
            return Ok(());
        };
        let p = self.blocks.0.palette(ctx);
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                for y in min[1]..=max[1] {
                    let centre = x == self.bottom[0] && z == self.bottom[2];
                    let state = if centre { p.ladder } else { p.wall[0] };
                    ctx.set_block([x, y, z], state);
                }
            }
        }
        Ok(())
    }

    fn add_additional_save_data(&self, tag: &mut Compound) {
        tag.insert("bottom".into(), Tag::IntArray(self.bottom.to_vec()));
        tag.insert("top".into(), Tag::Int(self.top));
    }
}

fn spawner_nbt(mob: &str) -> Compound {
    let entity: Compound = [("id".into(), Tag::String(mob.into()))].into();
    let data: Compound = [("entity".into(), Tag::Compound(entity))].into();
    [
        ("id".into(), Tag::String("minecraft:mob_spawner".into())),
        ("SpawnData".into(), Tag::Compound(data)),
    ]
    .into()
}

fn chest_nbt(seed: i64) -> Compound {
    [
        ("id".into(), Tag::String("minecraft:chest".into())),
        (
            "LootTable".into(),
            Tag::String("minecraft:chests/simple_dungeon".into()),
        ),
        ("LootTableSeed".into(), Tag::Long(seed)),
    ]
    .into()
}

/// The structure type.
struct Dungeon {
    blocks: Blocks,
}

impl StructureType for Dungeon {
    fn find_generation_point(&self, ctx: &mut dyn GenerationContext) -> Option<FoundStructure> {
        let chunk = ctx.chunk();
        let [x0, z0] = [chunk.x * 16, chunk.z * 16];
        let random = ctx.random();
        let entrance = [random.next_int_bound(GRID), random.next_int_bound(GRID)];
        // The surface over the entrance cell's ladder.
        let ladder = [x0 + entrance[0] * CELL + 4, z0 + entrance[1] * CELL + 4];
        let surface = ctx.base_height(ladder[0], ladder[1], HeightmapType::WorldSurfaceWg);
        let top = surface - COVER;
        let bottom = top - FLOORS * STOREY;
        if bottom < ctx.min_y() + 4 || surface <= ctx.sea_level() {
            return None;
        }
        let blocks = BlocksDebug(self.blocks);
        let mut pieces: Vec<Arc<dyn StructurePiece>> = vec![Arc::new(Entrance {
            blocks,
            bottom: [ladder[0], top, ladder[1]],
            top: surface,
        })];
        let mut up = entrance;
        for floor in 0..FLOORS {
            let random = ctx.random();
            let down = (floor + 1 < FLOORS).then(|| {
                loop {
                    let d = [random.next_int_bound(GRID), random.next_int_bound(GRID)];
                    if d != up {
                        break d;
                    }
                }
            });
            let cells = floor_layout(random, up, down);
            pieces.push(Arc::new(Floor {
                blocks,
                origin: [x0, top - (floor + 1) * STOREY, z0],
                floor,
                cells,
            }));
            if let Some(d) = down {
                up = d;
            }
        }
        Some(FoundStructure {
            position: [x0, top, z0],
            pieces,
        })
    }

    fn load_piece(
        &self,
        _: &dyn BlockMetadata,
        tag: &Compound,
    ) -> Result<Arc<dyn StructurePiece>, StructureError> {
        let blocks = BlocksDebug(self.blocks);
        let ints = |key: &str| match tag.get(&key.into()) {
            Some(Tag::IntArray(v)) => Ok(v.clone()),
            _ => Err(StructureError(format!("missing saved {key}"))),
        };
        let int = |key: &str| match tag.get(&key.into()) {
            Some(Tag::Int(v)) => Ok(*v),
            _ => Err(StructureError(format!("missing saved {key}"))),
        };
        let pos = |v: Vec<i32>| -> Result<[i32; 3], StructureError> {
            v.try_into()
                .map_err(|_| StructureError("a saved position is not 3 numbers".into()))
        };
        if tag.contains_key(&"cells".into()) {
            let cells: Cells = ints("cells")?.into_iter().map(|c| c as u16).collect();
            if cells.len() != (GRID * GRID) as usize {
                return Err(StructureError("saved cells are not a full grid".into()));
            }
            Ok(Arc::new(Floor {
                blocks,
                origin: pos(ints("origin")?)?,
                floor: int("floor")?,
                cells,
            }))
        } else {
            Ok(Arc::new(Entrance {
                blocks,
                bottom: pos(ints("bottom")?)?,
                top: int("top")?,
            }))
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let blocks = Blocks {
            stone_bricks: r.block_key("minecraft:stone_bricks"),
            mossy: r.block_key("minecraft:mossy_stone_bricks"),
            cracked: r.block_key("minecraft:cracked_stone_bricks"),
            cobblestone: r.block_key("minecraft:cobblestone"),
            air: r.block_key("minecraft:air"),
            ladder: r.block_key("minecraft:ladder"),
            spawner: r.block_key("minecraft:spawner"),
            chest: r.block_key("minecraft:chest"),
        };
        r.register_structure_type("dungeon", Dungeon { blocks });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Java's LCG, enough to drive the layout deterministically.
    struct Lcg(i64);
    impl StructureRandom for Lcg {
        fn next_int_bound(&mut self, bound: i32) -> i32 {
            self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
            ((self.0 >> 17) as i32).rem_euclid(bound.max(1))
        }
        fn next_long(&mut self) -> i64 {
            i64::from(self.next_int_bound(i32::MAX))
        }
        fn next_float(&mut self) -> f32 {
            self.next_int_bound(1 << 24) as f32 / (1 << 24) as f32
        }
        fn next_double(&mut self) -> f64 {
            f64::from(self.next_float())
        }
        fn next_boolean(&mut self) -> bool {
            self.next_int_bound(2) == 1
        }
    }

    /// The cells reached from `from` by walking the floor's open blocks at y = 1.
    fn reachable(cells: &Cells, from: [i32; 2]) -> usize {
        let side = GRID * CELL;
        let open = |x: i32, z: i32| {
            (0..side).contains(&x)
                && (0..side).contains(&z)
                && !matches!(floor_block(cells, [x, 1, z]), Kind::Wall)
        };
        let start = [from[0] * CELL + 4, from[1] * CELL + 4];
        let mut seen = vec![false; (side * side) as usize];
        let mut queue = VecDeque::from([start]);
        seen[(start[1] * side + start[0]) as usize] = true;
        let mut cells_seen = std::collections::HashSet::new();
        while let Some([x, z]) = queue.pop_front() {
            cells_seen.insert((x / CELL, z / CELL));
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                let (nx, nz) = (x + dx, z + dz);
                if open(nx, nz) && !seen[(nz * side + nx) as usize] {
                    seen[(nz * side + nx) as usize] = true;
                    queue.push_back([nx, nz]);
                }
            }
        }
        cells_seen.len()
    }

    #[test]
    fn every_cell_of_a_floor_can_be_walked_to() {
        for seed in [1, 42, 9_000_001] {
            let cells = floor_layout(&mut Lcg(seed), [3, 3], Some([0, 6]));
            assert_eq!(
                reachable(&cells, [3, 3]),
                (GRID * GRID) as usize,
                "seed {seed}"
            );
        }
    }

    #[test]
    fn the_ladders_meet_between_floors() {
        let above = floor_layout(&mut Lcg(7), [1, 1], Some([5, 2]));
        let below = floor_layout(&mut Lcg(8), [5, 2], None);
        let column = [5 * CELL + 4, 2 * CELL + 4];
        // Down through the floor above's floor, up through the floor below's ceiling.
        assert_eq!(floor_block(&above, [column[0], 0, column[1]]), Kind::Ladder);
        assert_eq!(
            floor_block(&below, [column[0], STOREY - 1, column[1]]),
            Kind::Ladder
        );
        // Each ladder has a wall to hang on.
        assert_eq!(
            floor_block(&below, [column[0], 2, column[1] - 1]),
            Kind::Wall
        );
        // The last floor has no way further down.
        assert!(below.iter().all(|c| c & DOWN == 0));
    }

    #[test]
    fn the_rock_between_rooms_is_left_alone() {
        let cells = floor_layout(&mut Lcg(11), [2, 2], Some([4, 4]));
        // A cell corner is never open, and the block two in from it borders nothing open.
        let corner = [CELL - 1, 3, CELL - 1];
        assert_eq!(floor_block(&cells, corner), Kind::Wall);
        let walls_kept = (0..GRID * CELL)
            .flat_map(|x| (0..GRID * CELL).map(move |z| [x, 3, z]))
            .filter(|&p| floor_block(&cells, p) == Kind::Wall)
            .filter(|&p| placed(&cells, p).is_none())
            .count();
        assert!(walls_kept > 0);
        // An open block is always placed (as air, so the rock is carved).
        assert_eq!(
            placed(&cells, [2 * CELL + 4, 3, 2 * CELL + 4]),
            Some(Kind::Ladder)
        );
    }

    #[test]
    fn rooms_hold_one_spawner_or_one_chest_and_corridors_neither() {
        let cells = floor_layout(&mut Lcg(3), [0, 0], Some([6, 6]));
        for (i, &cell) in cells.iter().enumerate() {
            let (x, z) = (i as i32 % GRID * CELL, i as i32 / GRID * CELL);
            let mut found = Vec::new();
            for dx in 0..CELL {
                for dz in 0..CELL {
                    match floor_block(&cells, [x + dx, 1, z + dz]) {
                        k @ (Kind::Spawner | Kind::Chest) => found.push(k),
                        _ => {}
                    }
                }
            }
            if cell & ROOM == 0 || cell & (UP | DOWN) != 0 {
                assert!(found.is_empty(), "cell {i}");
            } else {
                assert_eq!(found.len(), 1, "cell {i}");
            }
        }
    }
}
