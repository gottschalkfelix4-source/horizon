# Toast Control (Horizon port)

Port of [Toast Control](https://github.com/Shadows-of-Fire/Toast-Control) 9.0.1 (ATM10 8.x, MIT)
as a native Horizon client mod. Written from the mod's observed behaviour and public description,
not from its code.

## What it does

| Feature | Status |
|---|---|
| Block all toasts (`global_disable`) | ported |
| Block advancement toasts; block only challenge toasts | ported (a challenge is told by its frame label, `advancements.toast.challenge`) |
| Block recipe, tutorial, system and other toasts (other: now-playing music, mods' own) | ported |
| Settings on Options > Mods, all off by default | ported |
| "Clear toasts" key: removes the toasts on screen and queued | needs API |
| Display options (translucent toasts, no slide-in, shorter lifetime, fewer visible at once) | needs API |
| Blocking toasts by their class/id (Toast Control's block list for mods' toasts) | needs API: `ToastKind::Other` carries no id |

How: `toast_add` cancels the candidate before it is queued.

## Needs from Horizon

- **Clearing the toast queue**: `fn clear_toasts(&mut self)` on a client host area
  (`crates/horizon-mod-vocab/src/host/client_view.rs`, `ClientViewClientHost`), as
  `ToastManager.clear`. The port would then register a key (`KeyRegistration::new("clear", "key.keyboard.j")`)
  and call it on `consume_click(MOD_ID, "clear")` from `client_tick_post`.
- **Toast display**: a `toast_render` hook (client, `contract = "listen"`) with
  `ToastRenderEvent { kind: ToastKind, visibility: &mut f32, slide: &mut bool, lifetime_ms: &mut u64 }`
  in `crates/horizon-mod-vocab/src/hooks/toast_render.rs`, or `ToastAddEvent` fields
  `display_ms: &mut u64` and `slide: &mut bool`, so a mod can shorten or stop the animation;
  and a client setting for how many toasts show at once (`ToastManager` visible slots).
- **The toast's id** in `ToastAddEvent` (`id: Option<&str>`, the class/registry name a mod's
  toast has), for block lists of mods' toasts.

## To check against the NeoForge original

1. Its default for each block (here everything is shown, as vanilla).
2. Whether "system" in the original covers the same toasts as Horizon's `ToastKind::System`.
3. The default key of "Clear toasts".

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
