//! GeckoLib's model format for Horizon ports: `.geo.json` models (Bedrock geometry), their
//! `.animation.json` animations with Molang values and GeckoLib's easings, and a renderer that
//! draws them through the game's entity and block-entity draw contexts.
//!
//! A port loads its files in a client hook (`Host::resource`, or `include_bytes!` for its own),
//! parses them once, and registers a [`GeoEntityRenderer`] for its entity type; a block entity's
//! submit hook calls [`draw_model`] with the pose it wants.
//!
//! Written from the public Bedrock geometry, animation and Molang documentation and GeckoLib's
//! public wiki, not from GeckoLib's code. What still needs checking against GeckoLib in the game
//! is listed in README.md.

pub mod anim;
pub mod model;
pub mod molang;
pub mod render;

pub use anim::{parse_animations, Animation, BonePose, LoopMode};
pub use model::{parse_geo, GeoModel};
pub use render::{bake_quads, draw_model, Controller, GeoEntityRenderer, GeoQuad};
