# Bad Wither No Cookie - Reloaded (Horizon port)

Port of [Bad Wither No Cookie - Reloaded](https://www.curseforge.com/minecraft/mc-mods/bad-wither-no-cookie-reloaded)
3.20.4 (ATM10 8.x, MIT, by Kreezxil) as a native Horizon mod. Written from the mod's public
description (its options, defaults and commands), not from its code.

## What it does

| Option (`config/bwncr-common.toml`) | Default | Silences | Status |
|---|---|---|---|
| `silenceWither` | true | `entity.wither.death` (and `entity.wither.spawn` if it is ever played as a sound) | ported; the spawn roar is a global level event, see below |
| `silenceDragon` | true | `entity.ender_dragon.death`, `.growl`, `.flap` | ported for the sounds the server plays; death roar and wings: see below |
| `silenceLightning` | true | `entity.lightning_bolt.thunder` | ported where the server plays it; vanilla's client plays thunder itself |
| `silenceTrader` | true | `entity.wandering_trader.ambient` | ported; trader llamas: not yet (see below) |
| `silenceExplosion` | false | `entity.generic.explode`, `entity.tnt.primed`, `entity.creeper.primed`, `entity.wind_charge.wind_burst` | ported |
| `silenceUs` | `[]` | any listed sound id (namespace defaults to `minecraft`) | ported |
| `debugMode` | false | logs each silenced sound | ported |

How: `play_level_sound_at_position` and `play_level_sound_at_entity` cancel a matching sound, so
no player hears it. The hooks run on both sides, so a client with the mod also drops the
matching sounds the server sends it.

Commands (operator level 2), changes saved to the config at once:

| Original | Here | Status |
|---|---|---|
| `/bwncr show` | `/bwncr show` | ported |
| `/bwncr <option> <true\|false>` | `/bwncr set <option> <true\|false>` (`all` sets the five groups) | ported, different shape: Horizon's command literals are lowercase, so options are a word argument |
| `/bwncr toggle <option>` | `/bwncr toggle <option>` | ported |
| `/bwncr silenceUs add\|remove\|clear` | `/bwncr silenceus add <sound>`, `remove <sound>`, `clear` | ported (lowercase literal) |
| `/bad_wither_no_cookie_reloaded ...` alias | | not yet: only `/bwncr` is registered |
| `/bwncr listen`, `listen mute <sound>` | | not yet: needs the receiving player in the sound hooks |

Command feedback is translatable (`assets/bwncr/lang/en_us.json`); from the console it is logged
as the key and its values.

## Needs from Horizon

- **Global level events.** The wither's spawn roar (`1023`), the dragon's death (`1028`) and the
  End portal opening (`1038`) reach every player as `ServerLevel.globalLevelEvent`, not as a
  sound, so the sound hooks never see them. Needs a cancellable server hook
  `global_level_event(host: &mut dyn ServerHost, event: { level: LevelRef, id: i32, pos: [i32; 3],
  data: i32, cancelled: bool })`, fired in `ServerLevel.globalLevelEvent` before it sends, so the
  mod can drop it (or turn it into a local `level_event`). Horizon's 26.3 world has vanilla's
  `global_sound_events` game rule, which turns all three into local events; this port does not
  flip it, since that would change a saved world rule behind the operator's back.
- **The emitter of a positional sound.** Trader llamas play `entity.llama.ambient` like every
  llama; telling them apart needs `emitter: Option<EntityRef>` on
  `PlayLevelSoundAtPositionEvent` (mobs' own sounds are positional).
- **`/bwncr listen`.** Needs the players a sound goes to (`receivers: &[PlayerRef]`) on the sound
  events, or a per-player sound packet hook.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The exact sound ids in each group (does `silenceWither` also cover the wither's ambient,
   hurt and shoot sounds? does `silenceDragon` cover `entity.ender_dragon.hurt`?).
2. Whether "silenced" means dropped for everyone, or only no longer heard server-wide (heard
   near the source).
3. The config's real layout (section names, comments) so a file written by the original loads
   unchanged here; this port reads `key = value` lines under any section.
4. Whether `all` also sets `silenceExplosion` and `debugMode` (here: the five groups, not debug).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
