//! Better Advanced Tooltips for Horizon: with advanced tooltips on (F3+H), an item's tooltip
//! also tells its durability while undamaged, its food values, its burn time as fuel and the
//! size of its saved data.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a comparison with the NeoForge
//! original still has to confirm, is in README.md.
use mod_api::{
    ClientHost, Hooks, ItemStack, ItemTooltipEvent, Tag, Text,
    text::{TextContent, TextStyle},
};

pub const MOD_ID: &str = "betteradvancedtooltips";

/// Size in bytes of `tag`'s payload in Java's binary NBT (`NbtIo.write`).
pub fn payload_size(tag: &Tag) -> usize {
    match tag {
        Tag::End => 0,
        Tag::Byte(_) => 1,
        Tag::Short(_) => 2,
        Tag::Int(_) | Tag::Float(_) => 4,
        Tag::Long(_) | Tag::Double(_) => 8,
        Tag::ByteArray(v) => 4 + v.len(),
        Tag::String(s) => 2 + utf_len(&s.0),
        Tag::List(v) => 1 + 4 + v.iter().map(payload_size).sum::<usize>(),
        Tag::Compound(c) => {
            c.iter()
                .map(|(k, v)| 1 + 2 + utf_len(&k.0) + payload_size(v))
                .sum::<usize>()
                + 1
        }
        Tag::IntArray(v) => 4 + 4 * v.len(),
        Tag::LongArray(v) => 4 + 8 * v.len(),
    }
}

/// Bytes of Java's modified UTF-8 (`DataOutput.writeUTF`) for UTF-16 `units`.
fn utf_len(units: &[u16]) -> usize {
    units
        .iter()
        .map(|&u| match u {
            0x0001..=0x007F => 1,
            0x0000 | 0x0080..=0x07FF => 2,
            _ => 3,
        })
        .sum()
}

/// Size of a root compound as written to a file: type byte, empty name, payload.
pub fn root_size(tag: &Tag) -> usize {
    1 + 2 + payload_size(tag)
}

/// Reads a whole or decimal number from an NBT value.
fn number(tag: &Tag) -> Option<f64> {
    Some(match tag {
        Tag::Byte(v) => f64::from(*v),
        Tag::Short(v) => f64::from(*v),
        Tag::Int(v) => f64::from(*v),
        Tag::Long(v) => *v as f64,
        Tag::Float(bits) => f64::from(f32::from_bits(*bits)),
        Tag::Double(bits) => f64::from_bits(*bits),
        _ => return None,
    })
}

/// A component of `stack`: its own patch first (a `!id` removal hides it), else its item's default.
fn component(host: &dyn ClientHost, stack: &ItemStack, id: &str) -> Option<Tag> {
    if let Some(patch) = stack.components() {
        let patch = patch.as_nbt();
        if patch.contains_key(&format!("!{id}").as_str().into()) {
            return None;
        }
        if let Some(tag) = patch.get(&id.into()) {
            return Some(tag.clone());
        }
    }
    host.item_default_component(stack.item(), id)
}

/// `(nutrition, saturation)` of a `minecraft:food` value.
pub fn food(tag: &Tag) -> Option<(i32, f32)> {
    let Tag::Compound(c) = tag else {
        return None;
    };
    let nutrition = number(c.get(&"nutrition".into())?)? as i32;
    let saturation = c.get(&"saturation".into()).and_then(number).unwrap_or(0.0) as f32;
    Some((nutrition, saturation))
}

/// Formats a decimal the way the tooltip shows it: at most one decimal, none when whole.
pub fn decimal(v: f32) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

/// The extra lines, before vanilla's advanced lines (the item id and component count).
pub struct Extra {
    /// `(damage left, max)` for a damageable item vanilla doesn't already show (undamaged).
    pub durability: Option<(u32, u32)>,
    pub food: Option<(i32, f32)>,
    pub burn_time: Option<i32>,
    pub data_bytes: Option<usize>,
}

impl Extra {
    pub fn lines(&self) -> Vec<Text> {
        let arg = |s: String| Text::literal(s);
        let mut out = Vec::new();
        if let Some((left, max)) = self.durability {
            out.push(Text::translate(
                "item.durability",
                vec![arg(left.to_string()), arg(max.to_string())],
            ));
        }
        if let Some((nutrition, saturation)) = self.food {
            out.push(Text::translate(
                "betteradvancedtooltips.food",
                vec![arg(nutrition.to_string()), arg(decimal(saturation))],
            ));
        }
        if let Some(ticks) = self.burn_time {
            out.push(Text::translate(
                "betteradvancedtooltips.burn_time",
                vec![arg(ticks.to_string())],
            ));
        }
        if let Some(bytes) = self.data_bytes {
            out.push(Text::translate(
                "betteradvancedtooltips.nbt_size",
                vec![arg(bytes.to_string())],
            ));
        }
        let mut gray = TextStyle::default();
        gray.color = Some("dark_gray".into());
        out.into_iter()
            .map(|t| t.with_style(gray.clone()))
            .collect()
    }
}

/// Where vanilla's advanced block starts: the line that is the item's id, else the end.
fn insert_at(lines: &[(Text, u32)], id: &str) -> usize {
    lines
        .iter()
        .position(|(t, _)| matches!(&t.content, TextContent::Literal(s) if s == id))
        .unwrap_or(lines.len())
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        if !event.advanced || !host.on(MOD_ID) {
            return;
        }
        let stack = event.stack;
        let item = stack.item();
        let durability = host
            .max_damage(item)
            .filter(|&max| max > 0 && !stack.is_damaged())
            .map(|max| (max, max));
        let host_any: &dyn mod_api::Host = host;
        let extra = Extra {
            durability,
            food: component(host, stack, "minecraft:food")
                .as_ref()
                .and_then(food),
            burn_time: host.item_burn_time(item).filter(|&t| t > 0),
            data_bytes: stack.to_nbt(host_any).map(|c| root_size(&Tag::Compound(c))),
        };
        let at = insert_at(event.lines, &item.id(host));
        for (n, line) in extra.lines().into_iter().enumerate() {
            event.lines.insert(at + n, (line, u32::MAX));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nbt_size_follows_the_java_binary_format() {
        // {"minecraft:damage": 5}: type 1 + name length 2 + payload [type 1, name 2 + 16, int 4,
        // end 1] = 27 bytes.
        let tag = Tag::Compound([("minecraft:damage".into(), Tag::Int(5))].into());
        assert_eq!(root_size(&tag), 27);
        // A string's length is in modified UTF-8: "é" is two bytes, "€" three.
        assert_eq!(payload_size(&Tag::String("é€".into())), 2 + 2 + 3);
    }

    #[test]
    fn food_values_read_and_print_like_the_item_report() {
        let steak = Tag::Compound(
            [
                ("nutrition".into(), Tag::Int(8)),
                ("saturation".into(), Tag::Float(12.8f32.to_bits())),
            ]
            .into(),
        );
        let (n, s) = food(&steak).unwrap();
        assert_eq!((n, decimal(s)), (8, "12.8".to_string()));
        assert_eq!(decimal(6.0), "6");
    }

    #[test]
    fn extra_lines_go_before_the_item_id() {
        let lines = vec![
            (Text::literal("Diamond Sword"), u32::MAX),
            (Text::literal("minecraft:diamond_sword"), u32::MAX),
            (Text::literal("2 component(s)"), u32::MAX),
        ];
        assert_eq!(insert_at(&lines, "minecraft:diamond_sword"), 1);
        assert_eq!(insert_at(&lines, "minecraft:stone"), 3);
    }

    #[test]
    fn only_present_values_make_lines() {
        let extra = Extra {
            durability: Some((1561, 1561)),
            food: None,
            burn_time: Some(200),
            data_bytes: None,
        };
        let keys: Vec<_> = extra
            .lines()
            .into_iter()
            .map(|t| match t.content {
                TextContent::Translatable { key, .. } => key,
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            keys,
            ["item.durability", "betteradvancedtooltips.burn_time"]
        );
    }
}
