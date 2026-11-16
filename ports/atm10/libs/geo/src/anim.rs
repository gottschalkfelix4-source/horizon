//! `.animation.json` files: Bedrock's animation format (1.8.0) with GeckoLib's easing additions,
//! evaluated into a pose per bone.
//!
//! A channel is a constant, a Molang expression, or keyframes by time in seconds. Between two
//! keyframes the later one's `lerp_mode` or GeckoLib `easing` decides the curve; a keyframe may
//! have `pre` and `post` values (a jump at that time). Rotations become model space as the
//! geometry's do (`(−x, −y, z)`), positions mirror x.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::model::mirror_rotation;
use crate::molang::{self, Expr, Scope};

/// How an animation ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    /// Plays once, then the bones go back to rest.
    Once,
    /// Starts again from 0.
    Loop,
    /// Plays once and keeps its last pose.
    HoldOnLastFrame,
}

/// A curve between two keyframes.
#[derive(Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    Step,
    CatmullRom,
    /// GeckoLib's named easings (`easeInSine`, `easeOutBounce`...), with their optional argument.
    Named(String, Option<f64>),
}

/// Three values, each a constant or an expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Vec3Expr(pub [Expr; 3]);

impl Vec3Expr {
    fn eval(&self, scope: &dyn Scope) -> [f64; 3] {
        [self.0[0].eval(scope), self.0[1].eval(scope), self.0[2].eval(scope)]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f64,
    /// The value arriving at this time.
    pub pre: Vec3Expr,
    /// The value leaving it.
    pub post: Vec3Expr,
    /// The curve from the previous keyframe to this one.
    pub easing: Easing,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Channel {
    pub keyframes: Vec<Keyframe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Rotation,
    Position,
    Scale,
}

impl Channel {
    fn sample(&self, t: f64, scope: &dyn Scope) -> Option<[f64; 3]> {
        let ks = &self.keyframes;
        let first = ks.first()?;
        if ks.len() == 1 || t <= first.time {
            return Some(if t < first.time { first.pre.eval(scope) } else { first.post.eval(scope) });
        }
        let i = ks.iter().rposition(|k| k.time <= t).unwrap_or(0);
        let a = &ks[i];
        let Some(b) = ks.get(i + 1) else {
            return Some(a.post.eval(scope));
        };
        let span = b.time - a.time;
        let x = if span <= 0.0 { 1.0 } else { (t - a.time) / span };
        let from = a.post.eval(scope);
        let to = b.pre.eval(scope);
        Some(match &b.easing {
            Easing::Step => from,
            Easing::CatmullRom => {
                let before = if i > 0 { ks[i - 1].post.eval(scope) } else { from };
                let after = ks.get(i + 2).map_or(to, |k| k.pre.eval(scope));
                std::array::from_fn(|c| catmull_rom(before[c], from[c], to[c], after[c], x))
            }
            easing => {
                let e = ease(easing, x);
                std::array::from_fn(|c| from[c] + (to[c] - from[c]) * e)
            }
        })
    }
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

/// The easings from easings.net, by the names GeckoLib's files use.
pub fn ease(easing: &Easing, x: f64) -> f64 {
    use std::f64::consts::PI;
    let Easing::Named(name, arg) = easing else {
        return x;
    };
    let (dir, curve) = if let Some(c) = name.strip_prefix("easeInOut") {
        ("inout", c)
    } else if let Some(c) = name.strip_prefix("easeIn") {
        ("in", c)
    } else if let Some(c) = name.strip_prefix("easeOut") {
        ("out", c)
    } else {
        return x;
    };
    let f_in = |x: f64| -> f64 {
        match curve {
            "Sine" => 1.0 - (x * PI / 2.0).cos(),
            "Quad" => x * x,
            "Cubic" => x * x * x,
            "Quart" => x.powi(4),
            "Quint" => x.powi(5),
            "Expo" => {
                if x == 0.0 {
                    0.0
                } else {
                    2f64.powf(10.0 * x - 10.0)
                }
            }
            "Circ" => 1.0 - (1.0 - x * x).max(0.0).sqrt(),
            "Back" => {
                let c1 = arg.unwrap_or(1.70158);
                (c1 + 1.0) * x * x * x - c1 * x * x
            }
            "Elastic" => {
                if x == 0.0 || x == 1.0 {
                    x
                } else {
                    let c4 = 2.0 * PI / 3.0;
                    -(2f64.powf(10.0 * x - 10.0)) * ((x * 10.0 - 10.75) * c4).sin()
                }
            }
            "Bounce" => 1.0 - bounce_out(1.0 - x),
            _ => x,
        }
    };
    match dir {
        "in" => f_in(x),
        "out" => 1.0 - f_in(1.0 - x),
        _ => {
            if x < 0.5 {
                f_in(2.0 * x) / 2.0
            } else {
                1.0 - f_in(2.0 - 2.0 * x) / 2.0
            }
        }
    }
}

fn bounce_out(x: f64) -> f64 {
    let (n1, d1) = (7.5625, 2.75);
    if x < 1.0 / d1 {
        n1 * x * x
    } else if x < 2.0 / d1 {
        let x = x - 1.5 / d1;
        n1 * x * x + 0.75
    } else if x < 2.5 / d1 {
        let x = x - 2.25 / d1;
        n1 * x * x + 0.9375
    } else {
        let x = x - 2.625 / d1;
        n1 * x * x + 0.984375
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BoneTrack {
    pub rotation: Channel,
    pub position: Channel,
    pub scale: Channel,
}

/// A sound or particle keyframe, or a timeline instruction: the renderer hands these to the mod,
/// which plays them (a renderer has no host).
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub time: f64,
    pub kind: CueKind,
    /// The effect's name, or the instruction's text.
    pub value: String,
    /// A particle's locator bone, when it has one.
    pub locator: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueKind {
    Sound,
    Particle,
    Instruction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub length: f64,
    pub looping: LoopMode,
    pub bones: BTreeMap<String, BoneTrack>,
    pub cues: Vec<Cue>,
}

#[derive(Debug)]
pub enum AnimError {
    Json(serde_json::Error),
    Molang { animation: String, error: molang::ParseError },
}

impl std::fmt::Display for AnimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimError::Json(e) => write!(f, "animation file: {e}"),
            AnimError::Molang { animation, error } => write!(f, "animation {animation}: {error}"),
        }
    }
}

impl std::error::Error for AnimError {}

/// Reads every animation of an `.animation.json` file, by name.
pub fn parse_animations(bytes: &[u8]) -> Result<BTreeMap<String, Animation>, AnimError> {
    let root: Json = serde_json::from_slice(bytes).map_err(AnimError::Json)?;
    let mut out = BTreeMap::new();
    let Some(anims) = root.get("animations").and_then(Json::as_object) else {
        return Ok(out);
    };
    for (name, a) in anims {
        let err = |error| AnimError::Molang { animation: name.clone(), error };
        let looping = match a.get("loop") {
            Some(Json::Bool(true)) => LoopMode::Loop,
            Some(Json::String(s)) if s == "hold_on_last_frame" => LoopMode::HoldOnLastFrame,
            Some(Json::String(s)) if s == "true" => LoopMode::Loop,
            _ => LoopMode::Once,
        };
        let mut bones = BTreeMap::new();
        if let Some(map) = a.get("bones").and_then(Json::as_object) {
            for (bone, chans) in map {
                let track = BoneTrack {
                    rotation: channel(chans.get("rotation"), Kind::Rotation).map_err(err)?,
                    position: channel(chans.get("position"), Kind::Position).map_err(err)?,
                    scale: channel(chans.get("scale"), Kind::Scale).map_err(err)?,
                };
                bones.insert(bone.clone(), track);
            }
        }
        let mut cues = Vec::new();
        for (key, kind, field) in [
            ("sound_effects", CueKind::Sound, "effect"),
            ("particle_effects", CueKind::Particle, "effect"),
            ("timeline", CueKind::Instruction, ""),
        ] {
            let Some(map) = a.get(key).and_then(Json::as_object) else {
                continue;
            };
            for (time, v) in map {
                let Ok(time) = time.parse::<f64>() else {
                    continue;
                };
                let entries: Vec<&Json> = match v {
                    Json::Array(list) => list.iter().collect(),
                    v => vec![v],
                };
                for e in entries {
                    let value = match e {
                        Json::String(s) => s.clone(),
                        Json::Object(o) => o.get(field).and_then(Json::as_str).unwrap_or("").to_owned(),
                        _ => continue,
                    };
                    let locator = e.get("locator").and_then(Json::as_str).map(str::to_owned);
                    cues.push(Cue { time, kind, value, locator });
                }
            }
        }
        cues.sort_by(|a, b| a.time.total_cmp(&b.time));
        let last_key = bones
            .values()
            .flat_map(|t: &BoneTrack| [&t.rotation, &t.position, &t.scale])
            .filter_map(|c| c.keyframes.last().map(|k| k.time))
            .fold(0.0, f64::max);
        let length = a.get("animation_length").and_then(Json::as_f64).unwrap_or(last_key);
        out.insert(name.clone(), Animation { name: name.clone(), length, looping, bones, cues });
    }
    Ok(out)
}

fn expr(v: &Json) -> Result<Expr, molang::ParseError> {
    match v {
        Json::Number(n) => Ok(Expr::Num(n.as_f64().unwrap_or(0.0))),
        Json::String(s) if s.trim().is_empty() => Ok(Expr::Num(0.0)),
        Json::String(s) => molang::parse(s),
        Json::Bool(b) => Ok(Expr::Num(f64::from(u8::from(*b)))),
        _ => Ok(Expr::Num(0.0)),
    }
}

/// A value as model space wants it: rotations `(−x, −y, z)`, positions `(−x, y, z)`.
fn vec3(v: &Json, kind: Kind) -> Result<Vec3Expr, molang::ParseError> {
    let [x, y, z] = match v {
        Json::Array(a) => [
            expr(a.first().unwrap_or(&Json::Null))?,
            expr(a.get(1).unwrap_or(&Json::Null))?,
            expr(a.get(2).unwrap_or(&Json::Null))?,
        ],
        // One value stands for all three axes.
        other => {
            let e = expr(other)?;
            [e.clone(), e.clone(), e]
        }
    };
    let neg = |e: Expr| match e {
        Expr::Num(n) => Expr::Num(-n),
        e => Expr::Neg(Box::new(e)),
    };
    Ok(Vec3Expr(match kind {
        Kind::Rotation => {
            // The same sign change as `mirror_rotation`, kept in one place for constants.
            let signs = mirror_rotation([1.0, 1.0, 1.0]);
            let [x, y, z] = [x, y, z];
            [
                if signs[0] < 0.0 { neg(x) } else { x },
                if signs[1] < 0.0 { neg(y) } else { y },
                if signs[2] < 0.0 { neg(z) } else { z },
            ]
        }
        Kind::Position => [neg(x), y, z],
        Kind::Scale => [x, y, z],
    }))
}

fn channel(v: Option<&Json>, kind: Kind) -> Result<Channel, molang::ParseError> {
    let Some(v) = v else {
        return Ok(Channel::default());
    };
    let keyframes = match v {
        Json::Object(map) if !map.contains_key("vector") && !map.contains_key("pre") && !map.contains_key("post") => {
            let mut ks = Vec::new();
            for (time, frame) in map {
                let Ok(time) = time.parse::<f64>() else {
                    continue;
                };
                ks.push(keyframe(time, frame, kind)?);
            }
            ks.sort_by(|a, b| a.time.total_cmp(&b.time));
            ks
        }
        frame => vec![keyframe(0.0, frame, kind)?],
    };
    Ok(Channel { keyframes })
}

fn keyframe(time: f64, frame: &Json, kind: Kind) -> Result<Keyframe, molang::ParseError> {
    let (pre, post) = match frame {
        Json::Object(o) if o.contains_key("pre") || o.contains_key("post") => {
            let unwrap = |v: &Json| v.get("vector").cloned().unwrap_or_else(|| v.clone());
            let pre = o.get("pre").or_else(|| o.get("post")).map(unwrap).unwrap_or(Json::Null);
            let post = o.get("post").or_else(|| o.get("pre")).map(unwrap).unwrap_or(Json::Null);
            (vec3(&pre, kind)?, vec3(&post, kind)?)
        }
        Json::Object(o) => {
            let v = vec3(o.get("vector").unwrap_or(&Json::Null), kind)?;
            (v.clone(), v)
        }
        v => {
            let v = vec3(v, kind)?;
            (v.clone(), v)
        }
    };
    let easing = match frame {
        Json::Object(o) => {
            if let Some(name) = o.get("easing").and_then(Json::as_str) {
                let arg = o.get("easingArgs").and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_f64);
                match name {
                    "linear" => Easing::Linear,
                    "step" => Easing::Step,
                    "catmullrom" => Easing::CatmullRom,
                    n => Easing::Named(n.to_owned(), arg),
                }
            } else {
                match o.get("lerp_mode").and_then(Json::as_str) {
                    Some("catmullrom") => Easing::CatmullRom,
                    Some("step") => Easing::Step,
                    _ => Easing::Linear,
                }
            }
        }
        _ => Easing::Linear,
    };
    Ok(Keyframe { time, pre, post, easing })
}

/// A bone's change from rest at one moment: degrees and pixels in model space, scale as a factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    pub rotation: [f32; 3],
    pub position: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for BonePose {
    fn default() -> Self {
        Self { rotation: [0.0; 3], position: [0.0; 3], scale: [1.0; 3] }
    }
}

impl BonePose {
    pub fn lerp(&self, to: &BonePose, t: f32) -> BonePose {
        let l = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        BonePose { rotation: l(self.rotation, to.rotation), position: l(self.position, to.position), scale: l(self.scale, to.scale) }
    }
}

impl Animation {
    /// The time into the animation `seconds` after it started, or None once a `Once` animation
    /// has ended (its bones are back at rest).
    pub fn local_time(&self, seconds: f64) -> Option<f64> {
        match self.looping {
            LoopMode::Loop if self.length > 0.0 => Some(seconds.rem_euclid(self.length)),
            LoopMode::Loop => Some(0.0),
            LoopMode::HoldOnLastFrame => Some(seconds.min(self.length)),
            LoopMode::Once => (seconds <= self.length).then_some(seconds),
        }
    }

    /// Each animated bone's pose `seconds` after the animation started. `scope` answers the
    /// Molang reads; `query.anim_time` is set to the animation's own time on top of it.
    pub fn pose(&self, seconds: f64, scope: &dyn Scope) -> BTreeMap<String, BonePose> {
        let Some(t) = self.local_time(seconds) else {
            return BTreeMap::new();
        };
        let inner = |name: &str| -> Option<f64> {
            if name == "query.anim_time" {
                Some(t)
            } else {
                scope.get(name)
            }
        };
        let f = |v: [f64; 3]| v.map(|c| c as f32);
        self.bones
            .iter()
            .map(|(name, track)| {
                let mut pose = BonePose::default();
                if let Some(r) = track.rotation.sample(t, &inner) {
                    pose.rotation = f(r);
                }
                if let Some(p) = track.position.sample(t, &inner) {
                    pose.position = f(p);
                }
                if let Some(s) = track.scale.sample(t, &inner) {
                    pose.scale = f(s);
                }
                (name.clone(), pose)
            })
            .collect()
    }

    /// The cues whose time falls in `(from, to]` of the animation's own clock, for the mod to
    /// play; a loop that wrapped gives the end and the start.
    pub fn cues_between(&self, from: f64, to: f64) -> Vec<&Cue> {
        let (Some(a), Some(b)) = (self.local_time(from.max(0.0)), self.local_time(to)) else {
            return Vec::new();
        };
        self.cues
            .iter()
            .filter(|c| if a <= b { c.time > a && c.time <= b } else { c.time > a || c.time <= b })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const FILE: &str = r#"{
      "format_version": "1.8.0",
      "animations": {
        "animation.test.walk": {
          "loop": true,
          "animation_length": 2.0,
          "bones": {
            "leg": {
              "rotation": {"0.0": [0, 0, 0], "1.0": [90, 40, 20], "2.0": [0, 0, 0]},
              "position": [1, "math.sin(query.anim_time * 90) * 4", 0]
            },
            "tail": {"rotation": {"0.0": {"vector": [0, 0, 0]}, "1.0": {"vector": [0, 0, 100], "easing": "easeInQuad"}}},
            "jaw": {"scale": {"0.0": {"pre": [1, 1, 1], "post": [2, 2, 2]}, "1.0": [2, 2, 2]}}
          },
          "sound_effects": {"0.5": {"effect": "step"}, "1.5": {"effect": "step"}}
        },
        "animation.test.attack": {"loop": "hold_on_last_frame", "bones": {"arm": {"rotation": {"0.5": [10, 0, 0]}}}}
      }
    }"#;

    fn anims() -> BTreeMap<String, Animation> {
        parse_animations(FILE.as_bytes()).unwrap()
    }

    #[test]
    fn keyframes_interpolate_linearly_in_model_space() {
        let a = &anims()["animation.test.walk"];
        let p = a.pose(0.5, &HashMap::new());
        // halfway to (90, 40, 20), x and y negated
        assert_eq!(p["leg"].rotation, [-45.0, -20.0, 10.0]);
        let p = a.pose(2.5, &HashMap::new()); // looped back to 0.5
        assert_eq!(p["leg"].rotation, [-45.0, -20.0, 10.0]);
    }

    #[test]
    fn expressions_read_the_animation_time() {
        let a = &anims()["animation.test.walk"];
        let p = a.pose(1.0, &HashMap::new());
        assert_eq!(p["leg"].position[0], -1.0);
        assert!((p["leg"].position[1] - 4.0).abs() < 1e-5);
    }

    #[test]
    fn easing_belongs_to_the_later_keyframe() {
        let a = &anims()["animation.test.walk"];
        let p = a.pose(0.5, &HashMap::new());
        assert!((p["tail"].rotation[2] - 25.0).abs() < 1e-4, "{:?}", p["tail"]);
    }

    #[test]
    fn pre_and_post_make_a_jump() {
        let a = &anims()["animation.test.walk"];
        assert_eq!(a.pose(0.0, &HashMap::new())["jaw"].scale, [2.0; 3]);
        assert_eq!(a.pose(0.5, &HashMap::new())["jaw"].scale, [2.0; 3]);
    }

    #[test]
    fn hold_on_last_frame_keeps_the_pose_and_length_defaults_to_the_last_key() {
        let a = &anims()["animation.test.attack"];
        assert_eq!(a.length, 0.5);
        assert_eq!(a.pose(10.0, &HashMap::new())["arm"].rotation, [-10.0, 0.0, 0.0]);
    }

    #[test]
    fn cues_fire_once_across_a_loop() {
        let a = &anims()["animation.test.walk"];
        assert_eq!(a.cues_between(0.4, 0.6).len(), 1);
        assert_eq!(a.cues_between(1.9, 2.6).len(), 1);
        assert_eq!(a.cues_between(0.6, 1.4).len(), 0);
    }

    #[test]
    fn catmull_rom_passes_through_its_keys() {
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 0.0), 1.0);
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 1.0), 2.0);
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 0.5), 1.5);
    }

    #[test]
    fn named_easings_start_and_end_on_their_keys() {
        for name in ["easeInSine", "easeOutBounce", "easeInOutBack", "easeInElastic", "easeOutExpo", "easeInOutCirc"] {
            let e = Easing::Named(name.into(), None);
            assert!(ease(&e, 0.0).abs() < 1e-9, "{name}");
            assert!((ease(&e, 1.0) - 1.0).abs() < 1e-9, "{name}");
        }
    }
}
