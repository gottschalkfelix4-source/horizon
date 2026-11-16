//! Iron Jetpacks for Horizon: energy-powered jetpacks worn in the chest slot. Hold jump to fly;
//! keys toggle the engine and hover mode and change the throttle.
//!
//! Written from Iron Jetpacks' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How: as in vanilla, the window's player moves itself, so the flight runs on the client
//! (`client_tick_pre` pushes the local player). Each flying tick the client tells the server
//! (`ironjetpacks:thrust`), which takes the energy from the worn stack and resets the fall
//! distance. Key presses go to the server as `ironjetpacks:toggle`, which flips the stack's
//! switches. A jetpack's tier, energy and switches are data components of the one
//! `ironjetpacks:jetpack` item.

pub mod flight;

use std::sync::{Mutex, OnceLock};

use mod_api::{
    BuildCreativeModeTabContentsEvent, ClientHost, ClientTickPreEvent, Component, ComponentId,
    ComponentTypesEvent, EquipmentSlot, Hooks, IPayloadContext, ITEM_ENERGY, ItemBarEvent,
    ItemComponentEnergy, ItemKey, ItemPart, ItemStack, KeyConflictContext, KeyRegistration,
    MovementInputEvent, PayloadCodec, PayloadId, Registrar, RegistrationContext, ServerHost, Tag,
    Text, ToServer, TooltipEvent,
};

use flight::{
    Input, Jetpack, Switches, THROTTLE_STEP, energy_bar, energy_use, flying, jetpack_by_id, thrust,
    world_push,
};

const MOD_ID: &str = "ironjetpacks";

/// A string component.
pub struct StrComponent(&'static str);
/// An int component.
pub struct IntComponent(&'static str);
/// A boolean component (a byte, as vanilla stores booleans).
pub struct BoolComponent(&'static str);

impl Component for StrComponent {
    type Value = String;
    fn id(&self) -> &str {
        self.0
    }
    fn decode(&self, value: &Tag) -> Option<String> {
        match value {
            Tag::String(s) => s.to_string().ok(),
            _ => None,
        }
    }
    fn encode(&self, value: String) -> Tag {
        Tag::String(value.as_str().into())
    }
}

impl Component for IntComponent {
    type Value = i32;
    fn id(&self) -> &str {
        self.0
    }
    fn decode(&self, value: &Tag) -> Option<i32> {
        match value {
            Tag::Int(v) => Some(*v),
            _ => None,
        }
    }
    fn encode(&self, value: i32) -> Tag {
        Tag::Int(value)
    }
}

impl Component for BoolComponent {
    type Value = bool;
    fn id(&self) -> &str {
        self.0
    }
    fn decode(&self, value: &Tag) -> Option<bool> {
        match value {
            Tag::Byte(v) => Some(*v != 0),
            _ => None,
        }
    }
    fn encode(&self, value: bool) -> Tag {
        Tag::Byte(i8::from(value))
    }
}

pub const JETPACK_ID: StrComponent = StrComponent("ironjetpacks:jetpack_id");
pub const ENERGY: IntComponent = IntComponent("ironjetpacks:energy");
pub const ENGINE: BoolComponent = BoolComponent("ironjetpacks:engine");
pub const HOVER: BoolComponent = BoolComponent("ironjetpacks:hover");
pub const THROTTLE: IntComponent = IntComponent("ironjetpacks:throttle");

/// The stack's jetpack definition, if it is a jetpack with a known id.
fn jetpack_of(stack: &ItemStack) -> Option<&'static Jetpack> {
    let item = *JETPACK_ITEM.get()?;
    if stack.item() != item {
        return None;
    }
    jetpack_by_id(&stack.get(&JETPACK_ID)?)
}

/// The stack's switches; absent components read as the defaults (engine on, hover off, full
/// throttle).
pub fn switches_of(stack: &ItemStack) -> Switches {
    let d = Switches::default();
    Switches {
        engine: stack.get(&ENGINE).unwrap_or(d.engine),
        hover: stack.get(&HOVER).unwrap_or(d.hover),
        throttle: stack
            .get(&THROTTLE)
            .map_or(d.throttle, |t| t.clamp(0, 100) as u8),
    }
}

pub fn energy_of(stack: &ItemStack) -> u32 {
    stack.get(&ENERGY).unwrap_or(0).max(0) as u32
}

/// A full jetpack of `jetpack`'s kind, as the creative tab lists it.
pub fn full_jetpack(jetpack: &Jetpack) -> Option<ItemStack> {
    let mut stack = ItemStack::of(*JETPACK_ITEM.get()?);
    stack.set(&JETPACK_ID, jetpack.id.to_string()).ok()?;
    if !jetpack.creative() {
        stack.set(&ENERGY, jetpack.capacity as i32).ok()?;
    }
    Some(stack)
}

/// What a toggle key asks the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Engine,
    Hover,
    ThrottleUp,
    ThrottleDown,
}

impl Toggle {
    const ALL: [Toggle; 4] = [
        Toggle::Engine,
        Toggle::Hover,
        Toggle::ThrottleUp,
        Toggle::ThrottleDown,
    ];
    fn key_id(self) -> &'static str {
        match self {
            Toggle::Engine => "engine",
            Toggle::Hover => "hover",
            Toggle::ThrottleUp => "throttle_up",
            Toggle::ThrottleDown => "throttle_down",
        }
    }
    /// Default keys (assumed: G and H as in older versions; throttle on the brackets).
    fn default_key(self) -> &'static str {
        match self {
            Toggle::Engine => "key.keyboard.g",
            Toggle::Hover => "key.keyboard.h",
            Toggle::ThrottleUp => "key.keyboard.right.bracket",
            Toggle::ThrottleDown => "key.keyboard.left.bracket",
        }
    }
    fn code(self) -> u8 {
        self as u8
    }
    fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }
}

/// The switches after `toggle`.
pub fn toggled(switches: Switches, toggle: Toggle) -> Switches {
    let mut s = switches;
    match toggle {
        Toggle::Engine => s.engine = !s.engine,
        Toggle::Hover => s.hover = !s.hover,
        Toggle::ThrottleUp => s.throttle = s.throttle.saturating_add(THROTTLE_STEP).min(100),
        Toggle::ThrottleDown => s.throttle = s.throttle.saturating_sub(THROTTLE_STEP),
    }
    s
}

/// The action-bar line that reports a toggle.
fn status_line(toggle: Toggle, s: Switches) -> Text {
    let key = match toggle {
        Toggle::Engine if s.engine => "ironjetpacks.status.engine_on",
        Toggle::Engine => "ironjetpacks.status.engine_off",
        Toggle::Hover if s.hover => "ironjetpacks.status.hover_on",
        Toggle::Hover => "ironjetpacks.status.hover_off",
        Toggle::ThrottleUp | Toggle::ThrottleDown => {
            return Text::translate(
                "ironjetpacks.status.throttle",
                vec![Text::literal(s.throttle.to_string())],
            );
        }
    };
    Text::translate(key, Vec::new())
}

#[derive(PayloadCodec)]
struct ThrustPacket {
    sprinting: bool,
}

#[derive(PayloadCodec)]
struct TogglePacket {
    toggle: u8,
}

static JETPACK_ITEM: OnceLock<ItemKey> = OnceLock::new();
static THRUST: OnceLock<ToServer<ThrustPacket>> = OnceLock::new();
static TOGGLE: OnceLock<ToServer<TogglePacket>> = OnceLock::new();
/// The local player's movement keys, as `movement_input` last read them.
static INPUT: Mutex<Input> = Mutex::new(Input {
    jump: false,
    sneak: false,
    sprint: false,
    forward: 0.0,
    strafe: 0.0,
});

/// Server: the sender's worn jetpack, with `edit` applied and written back.
fn edit_worn(
    cx: &mut dyn IPayloadContext,
    edit: impl FnOnce(&mut dyn ServerHost, &Jetpack, &mut ItemStack),
) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
    let Some(living) = host.living(player.entity()) else {
        return Ok(());
    };
    let Some(mut stack) = living.equipment(host, EquipmentSlot::Chest) else {
        return Ok(());
    };
    let Some(jetpack) = jetpack_of(&stack) else {
        return Ok(());
    };
    let before = stack.clone();
    edit(host, jetpack, &mut stack);
    if stack != before {
        let _ = living.set_equipment(host, EquipmentSlot::Chest, Some(stack));
    }
    Ok(())
}

fn on_thrust(packet: ThrustPacket, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    edit_worn(cx, |host, jetpack, stack| {
        player.entity().reset_fall_distance(host);
        let switches = switches_of(stack);
        if !switches.engine {
            return;
        }
        let used = energy_use(jetpack, switches, packet.sprinting);
        let left = energy_of(stack).saturating_sub(used);
        let _ = stack.set(&ENERGY, left as i32);
    })
}

fn on_toggle(packet: TogglePacket, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let Some(toggle) = Toggle::from_code(packet.toggle) else {
        return Err("unknown jetpack toggle".into());
    };
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    edit_worn(cx, |host, _, stack| {
        let s = toggled(switches_of(stack), toggle);
        let _ = stack.set(&ENGINE, s.engine);
        let _ = stack.set(&HOVER, s.hover);
        let _ = stack.set(&THROTTLE, i32::from(s.throttle));
        let _ = player.action_bar(host, status_line(toggle, s));
    })
}

/// The jetpack item: its tooltip and energy bar.
struct JetpackItem;

fn on_off(on: bool) -> Text {
    Text::translate(
        if on {
            "ironjetpacks.on"
        } else {
            "ironjetpacks.off"
        },
        Vec::new(),
    )
}

#[mod_api::horizon_part]
impl ItemPart for JetpackItem {
    fn tooltip(&self, _: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let Some(jetpack) = jetpack_of(event.stack) else {
            return;
        };
        let s = switches_of(event.stack);
        let energy = if jetpack.creative() {
            Text::translate("ironjetpacks.tooltip.infinite", Vec::new())
        } else {
            Text::translate(
                "ironjetpacks.tooltip.energy",
                vec![
                    Text::literal(energy_of(event.stack).to_string()),
                    Text::literal(jetpack.capacity.to_string()),
                ],
            )
        };
        let tier = Text::translate(
            "ironjetpacks.tooltip.tier",
            vec![Text::literal(jetpack.tier.to_string())],
        );
        let engine = Text::translate("ironjetpacks.tooltip.engine", vec![on_off(s.engine)]);
        let hover = Text::translate("ironjetpacks.tooltip.hover", vec![on_off(s.hover)]);
        for line in [tier, energy, engine, hover] {
            event.lines.push((line, 0xAA_AA_AA));
        }
    }

    fn item_bar(&self, _: &dyn ClientHost, event: &mut ItemBarEvent<'_>) {
        if let Some(jetpack) = jetpack_of(event.stack) {
            *event.bar = energy_bar(energy_of(event.stack), jetpack.capacity);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let item = r.item_key("ironjetpacks:jetpack");
        let _ = JETPACK_ITEM.set(item);
        r.item_part(item, JetpackItem);
        // Chargers of other mods fill a jetpack through the item energy capability.
        r.item_capability(&ITEM_ENERGY, "ironjetpacks:jetpack", |host, at| {
            let stack = at.stack(host)?;
            let jetpack = jetpack_of(&stack)?;
            if jetpack.creative() {
                return None;
            }
            let component = ComponentId::new(ENERGY.0)?;
            let cap = jetpack.capacity;
            Some(Box::new(ItemComponentEnergy::new(
                at.clone(),
                component,
                cap,
                cap,
                cap,
            )))
        });
        for toggle in Toggle::ALL {
            r.register_key_mapping(
                KeyRegistration::new(toggle.key_id(), toggle.default_key())
                    .label(&format!("key.ironjetpacks.{}", toggle.key_id()))
                    .context(KeyConflictContext::InGame),
            );
        }
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        let _ = THRUST.set(view.play_to_server_typed::<ThrustPacket>(
            PayloadId("ironjetpacks:thrust".into()),
            on_thrust,
        ));
        let _ = TOGGLE.set(view.play_to_server_typed::<TogglePacket>(
            PayloadId("ironjetpacks:toggle".into()),
            on_toggle,
        ));
    }

    /// Every jetpack, full, in the combat tab (the original has its own tab; assumed here).
    fn build_creative_mode_tab_contents(
        &self,
        _: &mut dyn ClientHost,
        event: &mut BuildCreativeModeTabContentsEvent<'_>,
    ) {
        if event.tab != "minecraft:combat" {
            return;
        }
        event
            .items
            .extend(flight::JETPACKS.iter().filter_map(full_jetpack));
    }

    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        for id in [JETPACK_ID.0, ENERGY.0, ENGINE.0, HOVER.0, THROTTLE.0] {
            event.types.push(id.to_string());
        }
    }

    fn movement_input(&self, _: &dyn ClientHost, event: &mut MovementInputEvent) {
        if let Ok(mut input) = INPUT.lock() {
            *input = Input {
                jump: event.jump,
                sneak: event.sneak,
                sprint: event.sprint,
                forward: event.forward,
                strafe: event.strafe,
            };
        }
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        for toggle in Toggle::ALL {
            if host.consume_click(MOD_ID, toggle.key_id())
                && let Some(token) = TOGGLE.get()
            {
                let _ = token.send(
                    host,
                    &TogglePacket {
                        toggle: toggle.code(),
                    },
                );
            }
        }
        let Some(player) = host.local_player() else {
            return;
        };
        let entity = player.entity();
        let Some(stack) = host
            .living(entity)
            .and_then(|l| l.equipment(host, EquipmentSlot::Chest))
        else {
            return;
        };
        let Some(jetpack) = jetpack_of(&stack) else {
            return;
        };
        let input = INPUT.lock().map(|i| *i).unwrap_or_default();
        let switches = switches_of(&stack);
        let on_ground = entity.on_ground(host).unwrap_or(true);
        if !flying(jetpack, switches, energy_of(&stack), input, on_ground) {
            return;
        }
        let Some(velocity) = entity.velocity(host) else {
            return;
        };
        let t = thrust(jetpack, switches, input, velocity[1]);
        let yaw = host.camera().map_or(0.0, |c| c.yaw);
        let [dx, dz] = world_push(t.forward, t.strafe, yaw);
        host.push_player([dx as f32, (t.vy - velocity[1]) as f32, dz as f32]);
        if let Some(token) = THRUST.get() {
            let _ = token.send(
                host,
                &ThrustPacket {
                    sprinting: input.sprint,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_flip_switches_and_step_the_throttle_within_bounds() {
        let s = Switches::default();
        assert!(!toggled(s, Toggle::Engine).engine);
        assert!(toggled(s, Toggle::Hover).hover);
        assert_eq!(toggled(s, Toggle::ThrottleUp).throttle, 100);
        assert_eq!(toggled(s, Toggle::ThrottleDown).throttle, 95);
        let low = Switches { throttle: 3, ..s };
        assert_eq!(toggled(low, Toggle::ThrottleDown).throttle, 0);
    }

    #[test]
    fn toggle_codes_round_trip() {
        for t in Toggle::ALL {
            assert_eq!(Toggle::from_code(t.code()), Some(t));
        }
        assert_eq!(Toggle::from_code(9), None);
    }
}
