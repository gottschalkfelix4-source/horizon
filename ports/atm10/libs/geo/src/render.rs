//! Drawing a posed model: bones walked from the roots, each box's faces handed to the game's
//! draw context as textured quads (the same `quad` a block entity or entity renderer draws with).
//!
//! The transforms are GeckoLib's, per bone: move by the animation's position, go to the pivot,
//! turn z then y then x (rest rotation plus the animation's), scale, come back from the pivot.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use glam::{Mat4, Vec3};
use mod_api::{BlockEntityDrawContext, EntityRenderState, EntityRenderer};

use crate::anim::{Animation, BonePose};
use crate::model::{Face, GeoModel};
use crate::molang::Scope;

/// One face ready to draw: corners in blocks (top left, bottom left, bottom right, top right
/// seen from the front), UVs in texels of 16 per side of the texture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoQuad {
    pub corners: [[f32; 3]; 4],
    pub uv: [f32; 4],
    pub normal: [f32; 3],
}

fn rotation(deg: [f32; 3]) -> Mat4 {
    Mat4::from_rotation_z(deg[2].to_radians())
        * Mat4::from_rotation_y(deg[1].to_radians())
        * Mat4::from_rotation_x(deg[0].to_radians())
}

fn about(pivot_px: [f32; 3], inner: Mat4) -> Mat4 {
    let p = Vec3::from(pivot_px) / 16.0;
    Mat4::from_translation(p) * inner * Mat4::from_translation(-p)
}

/// A box face's corners in model space, seen from outside: sides with the image's top up, the
/// top and bottom with it towards north.
fn face_corners(face: Face, a: [f32; 3], b: [f32; 3]) -> [[f32; 3]; 4] {
    let ([x0, y0, z0], [x1, y1, z1]) = (a, b);
    match face {
        Face::North => [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]],
        Face::South => [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]],
        Face::East => [[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]],
        Face::West => [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
        Face::Up => [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        Face::Down => [[x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [x0, y0, z0]],
    }
}

/// Every visible face of `model` under `poses` (bone name to its change from rest), in blocks
/// relative to the model's origin.
pub fn bake_quads(model: &GeoModel, poses: &BTreeMap<String, BonePose>) -> Vec<GeoQuad> {
    let mut out = Vec::new();
    let texel = [16.0 / model.texture_width, 16.0 / model.texture_height];
    for &root in &model.roots {
        walk(model, root, Mat4::IDENTITY, poses, texel, &mut out);
    }
    out
}

fn walk(model: &GeoModel, i: usize, parent: Mat4, poses: &BTreeMap<String, BonePose>, texel: [f32; 2], out: &mut Vec<GeoQuad>) {
    let bone = &model.bones[i];
    let pose = poses.get(&bone.name).copied().unwrap_or_default();
    let rot = std::array::from_fn(|k| bone.rotation[k] + pose.rotation[k]);
    let m = parent
        * Mat4::from_translation(Vec3::from(pose.position) / 16.0)
        * about(bone.pivot, rotation(rot) * Mat4::from_scale(Vec3::from(pose.scale)));
    let hidden = bone.hidden || pose.scale.iter().all(|s| *s == 0.0);
    if !hidden {
        for cube in &bone.cubes {
            let cm = m * about(cube.pivot, rotation(cube.rotation));
            let a = cube.from.map(|c| c / 16.0);
            let b = cube.to.map(|c| c / 16.0);
            for (face, rect) in &cube.faces {
                let Some(r) = rect else { continue };
                let corners = face_corners(*face, a, b).map(|c| cm.transform_point3(Vec3::from(c)).to_array());
                let e1 = Vec3::from(corners[1]) - Vec3::from(corners[0]);
                let e2 = Vec3::from(corners[2]) - Vec3::from(corners[1]);
                let normal = e1.cross(e2).normalize_or_zero().to_array();
                let uv = [r[0] * texel[0], r[1] * texel[1], r[2] * texel[0], r[3] * texel[1]];
                out.push(GeoQuad { corners, uv, normal });
            }
        }
    }
    for &c in &bone.children {
        walk(model, c, m, poses, texel, out);
    }
}

/// Draws `model` posed by `poses` with texture `sprite` (`mymod:entity/robot`) under `draw`'s
/// current pose, times the 0xRRGGBB `tint`. False when the game doesn't know the sprite.
pub fn draw_model(
    draw: &mut dyn BlockEntityDrawContext,
    model: &GeoModel,
    poses: &BTreeMap<String, BonePose>,
    sprite: &str,
    tint: u32,
) -> bool {
    let mut ok = true;
    for q in bake_quads(model, poses) {
        ok &= draw.quad(sprite, q.corners, q.uv, tint);
    }
    ok
}

/// One animation an entity plays, from a moment of its age (in ticks).
#[derive(Clone, Debug, PartialEq)]
pub struct Playing {
    pub animation: String,
    pub since_tick: f32,
}

/// The animations a controller picks between, and how it blends: GeckoLib's
/// `AnimationController` reduced to what a renderer needs.
#[derive(Clone, Debug)]
pub struct Controller {
    /// Played while the entity stands still.
    pub idle: Option<String>,
    /// Played while it walks (its walk speed above `walk_threshold`).
    pub walk: Option<String>,
    pub walk_threshold: f32,
    /// Ticks to blend from one animation to the next; 0 switches at once.
    pub transition_ticks: f32,
}

impl Default for Controller {
    fn default() -> Self {
        Self { idle: None, walk: None, walk_threshold: 0.015, transition_ticks: 0.0 }
    }
}

#[derive(Clone, Debug)]
struct EntityAnim {
    playing: Option<Playing>,
    /// What the controller showed last and since when, for blending and for the loop's start.
    shown: Option<Playing>,
    previous: Option<(Playing, f32)>,
}

/// Draws every entity of a type with a GeckoLib model: its texture, its animations, and per
/// entity the one a mod's hook asked for ([`GeoEntityRenderer::play`]), else idle or walk.
pub struct GeoEntityRenderer {
    pub model: GeoModel,
    pub animations: BTreeMap<String, Animation>,
    pub sprite: String,
    pub controller: Controller,
    /// Extra Molang values for every entity (`variable.*`, `query.*` the game doesn't give).
    pub variables: HashMap<String, f64>,
    state: Mutex<HashMap<u32, EntityAnim>>,
}

impl GeoEntityRenderer {
    pub fn new(model: GeoModel, animations: BTreeMap<String, Animation>, sprite: &str) -> Arc<Self> {
        Arc::new(Self::with_controller(model, animations, sprite, Controller::default()))
    }

    pub fn with_controller(model: GeoModel, animations: BTreeMap<String, Animation>, sprite: &str, controller: Controller) -> Self {
        Self { model, animations, sprite: sprite.to_owned(), controller, variables: HashMap::new(), state: Mutex::new(HashMap::new()) }
    }

    /// Plays `animation` on entity `id` from its age `since_tick` (a mod sends this from a hook
    /// or a message, as GeckoLib's `triggerAnim` does). None goes back to idle and walk.
    pub fn play(&self, id: u32, animation: Option<&str>, since_tick: f32) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let e = state.entry(id).or_insert(EntityAnim { playing: None, shown: None, previous: None });
        e.playing = animation.map(|a| Playing { animation: a.to_owned(), since_tick });
    }

    /// Forgets entity `id` (it left the level).
    pub fn forget(&self, id: u32) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    }

    fn pose_of(&self, playing: &Playing, age: f32, scope: &dyn Scope) -> BTreeMap<String, BonePose> {
        match self.animations.get(&playing.animation) {
            Some(a) => a.pose(f64::from((age - playing.since_tick).max(0.0)) / 20.0, scope),
            None => BTreeMap::new(),
        }
    }

    /// The pose of entity `entity` this frame.
    pub fn poses(&self, entity: &EntityRenderState) -> BTreeMap<String, BonePose> {
        self.poses_of(entity.id, entity.age, entity.pitch, entity.living.as_ref())
    }

    fn poses_of(&self, id: u32, age: f32, pitch: f32, living: Option<&mod_api::LivingRenderState>) -> BTreeMap<String, BonePose> {
        let speed = living.map_or(0.0, |l| l.walk_animation_speed);
        let mut vars: HashMap<String, f64> = self.variables.clone();
        vars.insert("query.life_time".into(), f64::from(age) / 20.0);
        vars.insert("query.ground_speed".into(), f64::from(speed) * 20.0);
        vars.insert("query.modified_move_speed".into(), f64::from(speed));
        vars.insert("query.head_y_rotation".into(), f64::from(living.map_or(0.0, |l| l.y_rot)));
        vars.insert("query.head_x_rotation".into(), f64::from(living.map_or(pitch, |l| l.x_rot)));
        vars.insert("query.is_baby".into(), f64::from(u8::from(living.is_some_and(|l| l.is_baby))));
        vars.insert("query.is_in_water".into(), f64::from(u8::from(living.is_some_and(|l| l.is_in_water))));

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let e = state.entry(id).or_insert(EntityAnim { playing: None, shown: None, previous: None });
        // A played animation that ended goes back to idle and walk.
        if let Some(p) = &e.playing {
            let done = self.animations.get(&p.animation).is_none_or(|a| a.local_time(f64::from(age - p.since_tick) / 20.0).is_none());
            if done {
                e.playing = None;
            }
        }
        let want = e.playing.clone().or_else(|| {
            let name = if speed > self.controller.walk_threshold { self.controller.walk.as_ref() } else { None }
                .or(self.controller.idle.as_ref())?;
            Some(Playing { animation: name.clone(), since_tick: age })
        });
        let changed = match (&e.shown, &want) {
            (Some(a), Some(b)) => a.animation != b.animation || (e.playing.is_some() && a.since_tick != b.since_tick),
            (None, None) => false,
            _ => true,
        };
        if changed {
            if let Some(old) = e.shown.take() {
                e.previous = Some((old, age));
            }
            e.shown = want;
        }
        let current = e.shown.as_ref().map(|p| self.pose_of(p, age, &vars)).unwrap_or_default();
        let Some((prev, switched_at)) = e.previous.clone() else {
            return current;
        };
        let t = if self.controller.transition_ticks > 0.0 { (age - switched_at) / self.controller.transition_ticks } else { 1.0 };
        if t >= 1.0 {
            e.previous = None;
            return current;
        }
        let old = self.pose_of(&prev, switched_at, &vars);
        let names: std::collections::BTreeSet<&String> = old.keys().chain(current.keys()).collect();
        names
            .into_iter()
            .map(|n| {
                let a = old.get(n).copied().unwrap_or_default();
                let b = current.get(n).copied().unwrap_or_default();
                (n.clone(), a.lerp(&b, t))
            })
            .collect()
    }
}

impl EntityRenderer for GeoEntityRenderer {
    fn submit(&self, entity: &EntityRenderState, draw: &mut dyn BlockEntityDrawContext) -> Result<(), String> {
        let poses = self.poses(entity);
        draw.push();
        match &entity.living {
            Some(l) => {
                // As a living renderer: face the body's way, then its scale (a baby's too).
                draw.rotate_about([0.0, 1.0, 0.0], 180.0 - l.body_rot, [0.0; 3]);
                let s = l.scale * l.age_scale;
                draw.scale([s, s, s]);
            }
            None => draw.rotate_about([0.0, 1.0, 0.0], 180.0 - entity.yaw, [0.0; 3]),
        }
        let ok = draw_model(draw, &self.model, &poses, &self.sprite, 0xFFFFFF);
        draw.pop();
        if ok { Ok(()) } else { Err(format!("geo renderer: unknown sprite {}", self.sprite)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::parse_animations;
    use crate::model::parse_geo;

    const MODEL: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"identifier": "geometry.arm", "texture_width": 32, "texture_height": 32},
        "bones": [
          {"name": "arm", "pivot": [0, 16, 0],
           "cubes": [{"origin": [0, 0, 0], "size": [2, 16, 2], "uv": [0, 0]}]}
        ]}]}"#;

    fn approx(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn a_box_gives_six_faces_in_blocks_and_texture_units() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        let quads = bake_quads(&m, &BTreeMap::new());
        assert_eq!(quads.len(), 6);
        // The file's box spans x 0..2: mirrored, model space -2..0 pixels.
        let xs: Vec<f32> = quads.iter().flat_map(|q| q.corners.map(|c| c[0])).collect();
        assert!(xs.iter().all(|x| (-0.125..=0.0).contains(x)));
        // North face: uv (2, 2) to (4, 18) of a 32 texture, in 16ths.
        let north = quads.iter().find(|q| approx(q.normal, [0.0, 0.0, -1.0])).unwrap();
        assert_eq!(north.uv, [1.0, 1.0, 2.0, 9.0]);
    }

    #[test]
    fn every_face_faces_outwards() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        for q in bake_quads(&m, &BTreeMap::new()) {
            let centre = q.corners.iter().fold(Vec3::ZERO, |s, c| s + Vec3::from(*c)) / 4.0;
            let box_centre = Vec3::new(-1.0, 8.0, 1.0) / 16.0;
            assert!((centre - box_centre).dot(Vec3::from(q.normal)) > 0.0, "{q:?}");
        }
    }

    #[test]
    fn a_rotation_turns_the_bone_about_its_pivot() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        let mut poses = BTreeMap::new();
        // 180 degrees about z through (0, 16, 0) puts the arm above the pivot.
        poses.insert("arm".to_owned(), BonePose { rotation: [0.0, 0.0, 180.0], ..BonePose::default() });
        let ys: Vec<f32> = bake_quads(&m, &poses).iter().flat_map(|q| q.corners.map(|c| c[1])).collect();
        let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
        assert!((lo - 1.0).abs() < 1e-5 && (hi - 2.0).abs() < 1e-5, "{lo} {hi}");
    }

    impl GeoEntityRenderer {
        fn poses_t(&self, t: (u32, f32, f32, Option<&mod_api::LivingRenderState>)) -> BTreeMap<String, BonePose> {
            self.poses_of(t.0, t.1, t.2, t.3)
        }
    }

    #[test]
    fn a_played_animation_ends_and_the_controller_falls_back_to_idle() {
        let m = parse_geo(MODEL.as_bytes()).unwrap();
        let anims = parse_animations(
            br#"{"animations": {
                "idle": {"loop": true, "animation_length": 1, "bones": {"arm": {"rotation": [5, 0, 0]}}},
                "swing": {"animation_length": 0.5, "bones": {"arm": {"rotation": [90, 0, 0]}}}}}"#,
        )
        .unwrap();
        let r = GeoEntityRenderer::with_controller(m, anims, "test:entity/arm", Controller { idle: Some("idle".into()), ..Controller::default() });
        let at = |age: f32| (7u32, age, 0.0f32, None);
        assert_eq!(r.poses_t(at(0.0))["arm"].rotation[0], -5.0);
        r.play(7, Some("swing"), 2.0);
        assert_eq!(r.poses_t(at(4.0))["arm"].rotation[0], -90.0);
        // 0.5 s = 10 ticks after tick 2 it has ended.
        assert_eq!(r.poses_t(at(13.0))["arm"].rotation[0], -5.0);
    }
}
