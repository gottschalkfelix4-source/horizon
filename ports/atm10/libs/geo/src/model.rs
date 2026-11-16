//! `.geo.json` models: Bedrock's geometry format (1.12.0, and the older 1.8.0 form) as GeckoLib
//! reads it, baked into bones and boxes in model space.
//!
//! Model space here is the one GeckoLib draws in: y up, the model's front towards −z, units of
//! pixels (16 per block). A file's x axis is mirrored on the way in (GeckoLib negates a pivot's x
//! and puts a box at `-(origin.x + size.x)`), and its rotations become `(−x, −y, z)` degrees,
//! applied z, then y, then x.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value as Json;

/// A face of a box, by the side of model space it faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::Down, Face::Up, Face::North, Face::South, Face::West, Face::East];

    /// The name a file gives the same face, before the x mirror: model space's west is the file's
    /// east.
    fn file_name(self) -> &'static str {
        match self {
            Face::Down => "down",
            Face::Up => "up",
            Face::North => "north",
            Face::South => "south",
            Face::West => "east",
            Face::East => "west",
        }
    }
}

/// One box, in model space (pixels).
#[derive(Clone, Debug, PartialEq)]
pub struct GeoCube {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub pivot: [f32; 3],
    /// Degrees, model space.
    pub rotation: [f32; 3],
    /// Each face's texture rectangle `[u0, v0, u1, v1]` in pixels of the texture, or None for a
    /// face the file leaves out. `u0 > u1` draws it mirrored.
    pub faces: [(Face, Option<[f32; 4]>); 6],
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoBone {
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Model space, pixels.
    pub pivot: [f32; 3],
    /// Degrees, model space: the bone's rest rotation, to which animations add.
    pub rotation: [f32; 3],
    pub cubes: Vec<GeoCube>,
    /// Hidden from the file (`neverRender`) or by an animation.
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoModel {
    pub identifier: String,
    pub texture_width: f32,
    pub texture_height: f32,
    pub bones: Vec<GeoBone>,
    /// Bones without a parent, in file order.
    pub roots: Vec<usize>,
    by_name: HashMap<String, usize>,
}

impl GeoModel {
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }
}

#[derive(Debug)]
pub enum GeoError {
    Json(serde_json::Error),
    /// The file holds no geometry.
    Empty,
    /// A bone names a parent the file doesn't have.
    MissingParent { bone: String, parent: String },
}

impl std::fmt::Display for GeoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeoError::Json(e) => write!(f, "geo model: {e}"),
            GeoError::Empty => write!(f, "geo model: no geometry"),
            GeoError::MissingParent { bone, parent } => write!(f, "geo model: bone {bone} has no parent {parent}"),
        }
    }
}

impl std::error::Error for GeoError {}

#[derive(Deserialize)]
struct RawBone {
    name: String,
    parent: Option<String>,
    #[serde(default)]
    pivot: [f32; 3],
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default)]
    mirror: bool,
    #[serde(default)]
    inflate: f32,
    #[serde(default, rename = "neverRender", alias = "never_render")]
    never_render: bool,
    #[serde(default)]
    cubes: Vec<RawCube>,
}

#[derive(Deserialize)]
struct RawCube {
    origin: [f32; 3],
    size: [f32; 3],
    pivot: Option<[f32; 3]>,
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default)]
    inflate: Option<f32>,
    #[serde(default)]
    mirror: Option<bool>,
    #[serde(default)]
    uv: Json,
}

/// Reads a `.geo.json` file's first model.
pub fn parse_geo(bytes: &[u8]) -> Result<GeoModel, GeoError> {
    let root: Json = serde_json::from_slice(bytes).map_err(GeoError::Json)?;
    let (description, bones) = if let Some(list) = root.get("minecraft:geometry").and_then(Json::as_array) {
        let geo = list.first().ok_or(GeoError::Empty)?;
        (geo.get("description").cloned().unwrap_or(Json::Null), geo.get("bones").cloned().unwrap_or(Json::Null))
    } else {
        // 1.8.0: `"geometry.name": { "texturewidth": .., "bones": [..] }`
        let (name, geo) = root
            .as_object()
            .and_then(|o| o.iter().find(|(k, _)| k.starts_with("geometry.")))
            .ok_or(GeoError::Empty)?;
        let mut d = serde_json::Map::new();
        d.insert("identifier".into(), Json::String(name.clone()));
        if let Some(w) = geo.get("texturewidth") {
            d.insert("texture_width".into(), w.clone());
        }
        if let Some(h) = geo.get("textureheight") {
            d.insert("texture_height".into(), h.clone());
        }
        (Json::Object(d), geo.get("bones").cloned().unwrap_or(Json::Null))
    };
    let identifier = description.get("identifier").and_then(Json::as_str).unwrap_or("").to_owned();
    let tw = description.get("texture_width").and_then(Json::as_f64).unwrap_or(64.0) as f32;
    let th = description.get("texture_height").and_then(Json::as_f64).unwrap_or(64.0) as f32;
    let raw: Vec<RawBone> = if bones.is_null() { Vec::new() } else { serde_json::from_value(bones).map_err(GeoError::Json)? };

    let by_name: HashMap<String, usize> = raw.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
    let mut out: Vec<GeoBone> = Vec::with_capacity(raw.len());
    let mut roots = Vec::new();
    for (i, b) in raw.iter().enumerate() {
        let parent = match &b.parent {
            Some(p) => Some(*by_name.get(p).ok_or_else(|| GeoError::MissingParent { bone: b.name.clone(), parent: p.clone() })?),
            None => None,
        };
        if parent.is_none() {
            roots.push(i);
        }
        out.push(GeoBone {
            name: b.name.clone(),
            parent,
            children: Vec::new(),
            pivot: mirror_point(b.pivot),
            rotation: mirror_rotation(b.rotation),
            cubes: b.cubes.iter().map(|c| bake_cube(c, b, [tw, th])).collect(),
            hidden: b.never_render,
        });
    }
    for i in 0..out.len() {
        if let Some(p) = out[i].parent {
            out[p].children.push(i);
        }
    }
    Ok(GeoModel { identifier, texture_width: tw, texture_height: th, bones: out, roots, by_name })
}

fn mirror_point(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// A file's rotation in model space.
pub(crate) fn mirror_rotation(r: [f32; 3]) -> [f32; 3] {
    [-r[0], -r[1], r[2]]
}

fn bake_cube(c: &RawCube, bone: &RawBone, _texture: [f32; 2]) -> GeoCube {
    let inflate = c.inflate.unwrap_or(bone.inflate);
    let mirror = c.mirror.unwrap_or(bone.mirror);
    let x0 = -(c.origin[0] + c.size[0]);
    let from = [x0 - inflate, c.origin[1] - inflate, c.origin[2] - inflate];
    let to = [x0 + c.size[0] + inflate, c.origin[1] + c.size[1] + inflate, c.origin[2] + c.size[2] + inflate];
    let faces = match &c.uv {
        Json::Array(uv) => {
            let u = uv.first().and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let v = uv.get(1).and_then(Json::as_f64).unwrap_or(0.0) as f32;
            box_uv([u, v], c.size, mirror)
        }
        Json::Object(map) => Face::ALL.map(|face| {
            let rect = map.get(face.file_name()).and_then(|f| {
                let uv = f.get("uv")?.as_array()?;
                let size = f.get("uv_size").and_then(Json::as_array);
                let n = |a: &Vec<Json>, i: usize| a.get(i).and_then(Json::as_f64).unwrap_or(0.0) as f32;
                let (u, v) = (n(uv, 0), n(uv, 1));
                let (w, h) = size.map_or((0.0, 0.0), |s| (n(s, 0), n(s, 1)));
                Some([u, v, u + w, v + h])
            });
            (face, rect)
        }),
        _ => Face::ALL.map(|f| (f, None)),
    };
    GeoCube {
        from,
        to,
        pivot: c.pivot.map(mirror_point).unwrap_or([0.0; 3]),
        rotation: mirror_rotation(c.rotation),
        faces,
    }
}

/// Box UV: the layout Blockbench and Java's `ModelPart` share. The side strip at `v + d` holds
/// the right side (east), the front (north), the left side (west) and the back (south); the top
/// and bottom sit above the front and the left side, the bottom flipped.
pub fn box_uv(uv: [f32; 2], size: [f32; 3], mirror: bool) -> [(Face, Option<[f32; 4]>); 6] {
    let [u, v] = uv;
    let [w, h, d] = size.map(|s| s.floor());
    let mut east = [u, v + d, u + d, v + d + h];
    let north = [u + d, v + d, u + d + w, v + d + h];
    let mut west = [u + d + w, v + d, u + 2.0 * d + w, v + d + h];
    let south = [u + 2.0 * d + w, v + d, u + 2.0 * d + 2.0 * w, v + d + h];
    let up = [u + d, v, u + d + w, v + d];
    let down = [u + d + w, v + d, u + d + 2.0 * w, v];
    let mut faces = [up, down, north, south, west, east];
    if mirror {
        std::mem::swap(&mut east, &mut west);
        faces = [up, down, north, south, west, east];
        for f in &mut faces {
            f.swap(0, 2);
        }
    }
    let [up, down, north, south, west, east] = faces;
    [
        (Face::Down, Some(down)),
        (Face::Up, Some(up)),
        (Face::North, Some(north)),
        (Face::South, Some(south)),
        (Face::West, Some(west)),
        (Face::East, Some(east)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = r#"{
      "format_version": "1.12.0",
      "minecraft:geometry": [{
        "description": {"identifier": "geometry.test", "texture_width": 64, "texture_height": 32},
        "bones": [
          {"name": "root", "pivot": [0, 0, 0]},
          {"name": "body", "parent": "root", "pivot": [2, 12, 0], "rotation": [10, 20, 30],
           "cubes": [{"origin": [-4, 12, -2], "size": [8, 12, 4], "uv": [16, 16]}]},
          {"name": "head", "parent": "body", "pivot": [0, 24, 0],
           "cubes": [{"origin": [-4, 24, -4], "size": [8, 8, 8], "inflate": 0.5,
                      "uv": {"north": {"uv": [8, 8], "uv_size": [8, 8]}}}]}
        ]
      }]
    }"#;

    #[test]
    fn bones_keep_their_tree_and_mirror_x() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        assert_eq!(m.identifier, "geometry.test");
        assert_eq!((m.texture_width, m.texture_height), (64.0, 32.0));
        assert_eq!(m.roots, vec![0]);
        let body = &m.bones[m.bone("body").unwrap()];
        assert_eq!(body.parent, Some(0));
        assert_eq!(body.children, vec![m.bone("head").unwrap()]);
        assert_eq!(body.pivot, [-2.0, 12.0, 0.0]);
        assert_eq!(body.rotation, [-10.0, -20.0, 30.0]);
        // origin x -4, size 8: spans -4..4 either way; a lopsided box shows the mirror.
        assert_eq!(body.cubes[0].from, [-4.0, 12.0, -2.0]);
        assert_eq!(body.cubes[0].to, [4.0, 24.0, 2.0]);
    }

    #[test]
    fn a_box_is_inflated_and_per_face_uv_kept() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        let head = &m.bones[m.bone("head").unwrap()].cubes[0];
        assert_eq!(head.from, [-4.5, 23.5, -4.5]);
        assert_eq!(head.to, [4.5, 32.5, 4.5]);
        let north = head.faces.iter().find(|(f, _)| *f == Face::North).unwrap().1;
        assert_eq!(north, Some([8.0, 8.0, 16.0, 16.0]));
        assert!(head.faces.iter().filter(|(_, r)| r.is_some()).count() == 1);
    }

    #[test]
    fn box_uv_follows_the_skin_layout() {
        // A player's head: uv (0, 0), 8x8x8. Right side at (0, 8), face at (8, 8), left side
        // at (16, 8), back at (24, 8), top at (8, 0), bottom at (16, 0).
        let faces: HashMap<Face, [f32; 4]> = box_uv([0.0, 0.0], [8.0, 8.0, 8.0], false)
            .into_iter()
            .map(|(f, r)| (f, r.unwrap()))
            .collect();
        assert_eq!(faces[&Face::East], [0.0, 8.0, 8.0, 16.0]);
        assert_eq!(faces[&Face::North], [8.0, 8.0, 16.0, 16.0]);
        assert_eq!(faces[&Face::West], [16.0, 8.0, 24.0, 16.0]);
        assert_eq!(faces[&Face::South], [24.0, 8.0, 32.0, 16.0]);
        assert_eq!(faces[&Face::Up], [8.0, 0.0, 16.0, 8.0]);
        assert_eq!(faces[&Face::Down], [16.0, 8.0, 24.0, 0.0]);
    }

    #[test]
    fn the_old_format_reads_too() {
        let old = r#"{"format_version": "1.8.0", "geometry.old": {"texturewidth": 32, "textureheight": 32,
            "bones": [{"name": "a", "pivot": [1, 2, 3]}]}}"#;
        let m = parse_geo(old.as_bytes()).unwrap();
        assert_eq!(m.identifier, "geometry.old");
        assert_eq!(m.texture_width, 32.0);
        assert_eq!(m.bones[0].pivot, [-1.0, 2.0, 3.0]);
    }
}
